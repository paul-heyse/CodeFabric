//! Canonical source/syntax projection and deterministic range reconciliation.

use std::collections::{BTreeMap, BTreeSet};

use crate::fact_ingest::{
    CanonicalIngestOutput, CapabilityStatusRow, ConflictRecord, EntityRow, FactEvidenceRow,
    FactIngestError, FactScope, IngestDiagnostic, IngestMetrics, OwnerRow, PropertyFactRow,
    RelationRow, SourceAnnotationRow, SourceFileRow, SourceTokenRow, SyntaxDetailRow,
    ValidatedFactBatch, encode_capability_statuses, encode_entities, encode_evidence,
    encode_owners, encode_properties, encode_relations, encode_source_annotations,
    encode_source_files, encode_source_tokens, encode_syntax_details,
};
use crate::identity::{
    PlatformCode, SOURCE_CONTEXT_ID, SourceOccurrenceIdentityInput, SourceRelationIdentityInput,
    source_file_identity, source_occurrence_identity, source_relation_identity,
};
use crate::model_generated::registries::{OccurrenceFamily, ProviderNodeFlags, RawKindDisposition};
use crate::provider_raw_kinds::ProviderRawKindDisposition;
use crate::registries::{
    AnnotationKind, CompletenessState, Language, NewlineKind as RegistryNewlineKind,
    OwnerCapabilityState, OwnerKind, PathEncoding, ProviderCode, SYNTAX_KIND_VALUES,
    SyntaxFieldRole, SyntaxKind, TokenKind, capability_code, capability_mask, entity_kind,
    registry_state_name, relation_kind,
};
use crate::ruff_adapter::{
    RuffAstCategory, RuffAstFact, RuffChildRole, RuffDirectiveKind, RuffOccurrenceId, RuffSnapshot,
    RuffTokenClass, RuffTokenFact, RuffTokenSpelling,
};
use crate::source_image::{NewlineKind, SourceEncoding, SourceImage, SourceLanguage};
use crate::tree_sitter_adapter::{RawSyntaxFact, SyntaxOccurrenceId, TreeSitterSnapshot};

const SOURCE_PROVIDER_VERSION: &str = "source-image-v1";

/// Runtime provider-run identities associated with one complete source projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSyntaxProviderRuns {
    pub tree_sitter: [u8; 16],
    pub ruff_python: Option<[u8; 16]>,
}

/// The exact GEN §80 rule that selected a canonical syntax anchor.
pub use crate::model_generated::registries::RangeReconciliationStep as ReconciliationStep;

/// Provider-independent candidate supplied to the five-step range ladder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RangeAnchor {
    pub entity_id: [u8; 16],
    pub start_byte: u64,
    pub end_byte: u64,
    pub normalized_kind_code: u16,
    pub declaration_name_span: Option<(u64, u64)>,
}

/// One source-ranged observation seeking a canonical syntax anchor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RangeObservation {
    pub start_byte: u64,
    pub end_byte: u64,
    pub normalized_kind_code: u16,
    pub declaration_name_span: Option<(u64, u64)>,
}

/// Result of the deterministic GEN §80 range ladder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReconciliationOutcome {
    pub entity_id: Option<[u8; 16]>,
    pub step: ReconciliationStep,
    /// More than one distinct canonical anchor had the same best governed rank.
    pub ambiguous: bool,
}

/// Apply the five-step range ladder without arbitrary-overlap fallback.
#[must_use]
pub fn reconcile_range(
    observation: RangeObservation,
    candidates: &[RangeAnchor],
) -> ReconciliationOutcome {
    reconcile_range_with_preference(observation, candidates, None)
}

fn reconcile_range_with_preference(
    observation: RangeObservation,
    candidates: &[RangeAnchor],
    preferred_entity_id: Option<[u8; 16]>,
) -> ReconciliationOutcome {
    let (candidate, ambiguous) = select_unique_by_key(
        candidates.iter().filter(|candidate| {
            candidate.start_byte == observation.start_byte
                && candidate.end_byte == observation.end_byte
                && candidate.normalized_kind_code == observation.normalized_kind_code
        }),
        |candidate| preference_rank(candidate, preferred_entity_id),
    );
    if let Some(candidate) = candidate {
        return outcome(candidate, ReconciliationStep::ExactRangeAndKind, ambiguous);
    }
    if observation.normalized_kind_code == SyntaxKind::DeclarationSyntax as u16
        && let Some(name_span) = observation.declaration_name_span
    {
        let (candidate, ambiguous) = select_unique_by_key(
            candidates.iter().filter(|candidate| {
                candidate.normalized_kind_code == SyntaxKind::DeclarationSyntax as u16
                    && candidate.declaration_name_span == Some(name_span)
                    && candidate.start_byte <= name_span.0
                    && candidate.end_byte >= name_span.1
            }),
            |candidate| {
                (
                    candidate.end_byte.saturating_sub(candidate.start_byte),
                    preference_rank(candidate, preferred_entity_id),
                )
            },
        );
        if let Some(candidate) = candidate {
            return outcome(
                candidate,
                ReconciliationStep::ExactDeclarationName,
                ambiguous,
            );
        }
    }
    let (candidate, ambiguous) = select_unique_by_key(
        candidates.iter().filter(|candidate| {
            candidate.start_byte <= observation.start_byte
                && candidate.end_byte >= observation.end_byte
                && compatible_kind(
                    candidate.normalized_kind_code,
                    observation.normalized_kind_code,
                )
        }),
        |candidate| {
            (
                candidate.end_byte.saturating_sub(candidate.start_byte),
                u8::from(candidate.normalized_kind_code != observation.normalized_kind_code),
                preference_rank(candidate, preferred_entity_id),
            )
        },
    );
    if let Some(candidate) = candidate {
        return outcome(
            candidate,
            ReconciliationStep::SmallestEnclosingCompatible,
            ambiguous,
        );
    }
    let (candidate, ambiguous) = select_unique_by_key(
        candidates.iter().filter(|candidate| {
            candidate.start_byte == observation.start_byte
                && compatible_kind(
                    candidate.normalized_kind_code,
                    observation.normalized_kind_code,
                )
        }),
        |candidate| {
            (
                candidate.end_byte.abs_diff(observation.end_byte),
                u8::from(candidate.normalized_kind_code != observation.normalized_kind_code),
                candidate.end_byte.saturating_sub(candidate.start_byte),
                preference_rank(candidate, preferred_entity_id),
            )
        },
    );
    if let Some(candidate) = candidate {
        return outcome(
            candidate,
            ReconciliationStep::SameStartCompatible,
            ambiguous,
        );
    }
    ReconciliationOutcome {
        entity_id: None,
        step: ReconciliationStep::ProviderOnlySynthetic,
        ambiguous: false,
    }
}

fn preference_rank(candidate: &RangeAnchor, preferred_entity_id: Option<[u8; 16]>) -> u8 {
    match preferred_entity_id {
        Some(preferred) if candidate.entity_id == preferred => 0,
        Some(_) => 1,
        None => 0,
    }
}

fn select_unique_by_key<'a, I, K>(
    candidates: I,
    mut key: impl FnMut(&RangeAnchor) -> K,
) -> (Option<&'a RangeAnchor>, bool)
where
    I: Iterator<Item = &'a RangeAnchor>,
    K: Ord,
{
    let mut best: Option<(&RangeAnchor, K)> = None;
    let mut ambiguous = false;
    for candidate in candidates {
        let candidate_key = key(candidate);
        match &best {
            None => best = Some((candidate, candidate_key)),
            Some((selected, selected_key)) => match candidate_key.cmp(selected_key) {
                std::cmp::Ordering::Less => {
                    best = Some((candidate, candidate_key));
                    ambiguous = false;
                }
                std::cmp::Ordering::Equal if candidate.entity_id != selected.entity_id => {
                    ambiguous = true;
                }
                std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => {}
            },
        }
    }
    (best.map(|(candidate, _)| candidate), ambiguous)
}

const fn outcome(
    candidate: &RangeAnchor,
    step: ReconciliationStep,
    ambiguous: bool,
) -> ReconciliationOutcome {
    ReconciliationOutcome {
        entity_id: Some(candidate.entity_id),
        step,
        ambiguous,
    }
}

const fn compatible_kind(left: u16, right: u16) -> bool {
    left == right || left == SyntaxKind::SyntaxNode as u16 || right == SyntaxKind::SyntaxNode as u16
}

fn reconciliation_anchors(
    facts: &[RawSyntaxFact],
    entity_ids: &BTreeMap<SyntaxOccurrenceId, [u8; 16]>,
    name_spans: &BTreeMap<SyntaxOccurrenceId, (u64, u64)>,
) -> Vec<RangeAnchor> {
    facts
        .iter()
        .filter(|fact| fact.named && !fact.extra && !fact.error && !fact.missing)
        .filter_map(|fact| {
            Some(RangeAnchor {
                entity_id: *entity_ids.get(&fact.id)?,
                start_byte: fact.start_byte,
                end_byte: fact.end_byte,
                normalized_kind_code: fact.normalized_kind.0,
                declaration_name_span: name_spans.get(&fact.id).copied(),
            })
        })
        .collect()
}

const fn governed_provider_node_flags() -> i64 {
    ProviderNodeFlags::empty().bits().cast_signed()
}

#[derive(Clone)]
struct EvidenceInput {
    fact_id: [u8; 16],
    provider_code: i16,
    provider_version: String,
    provider_run_id: [u8; 16],
    observation_id: [u8; 16],
    raw_kind_code: Option<i32>,
    file_id: Option<[u8; 16]>,
    start_byte: Option<i64>,
    end_byte: Option<i64>,
    cold_payload: Option<Vec<u8>>,
}

#[allow(clippy::too_many_lines)] // One ordered pass makes cross-table identity and endpoint construction auditable.
pub(crate) fn project(
    expected_scope: FactScope,
    source: &SourceImage,
    tree: &TreeSitterSnapshot,
    ruff: Option<&RuffSnapshot>,
    runs: SourceSyntaxProviderRuns,
) -> Result<CanonicalIngestOutput, FactIngestError> {
    validate_inputs(expected_scope, source, tree, ruff, runs)?;
    let language = language_code(source.language);
    let byte_len = source.byte_length;
    let mut entities = Vec::new();
    let mut relations = Vec::new();
    let properties: Vec<PropertyFactRow> = Vec::new();
    let mut source_tokens = Vec::new();
    let mut source_annotations = Vec::new();
    let mut syntax_details = Vec::new();
    let mut evidence_inputs = Vec::new();
    let mut conflicts = Vec::new();
    let mut diagnostics = Vec::new();
    let mut rejected_observations = BTreeSet::new();

    let file_kind = required_entity_kind("SOURCE_FILE")?;
    entities.push(EntityRow {
        scope: expected_scope,
        entity_id: source.file_id,
        language,
        entity_family_code: file_kind.family_code,
        entity_kind_code: file_kind.code,
        raw_kind_code: None,
        file_id: Some(source.file_id),
        start_byte: Some(0),
        end_byte: Some(to_i64(byte_len, "source byte length")?),
        name: None,
        qualified_name: None,
        parent_entity_id: None,
        type_id: None,
        flags: 0,
        fact_hash64: hash64(source.file_id),
    });
    let source_observation = observation_id(source.lease.lease_id, 1, 0);
    evidence_inputs.push(EvidenceInput {
        fact_id: source.file_id,
        provider_code: ProviderCode::SourceSubstrate as i16,
        provider_version: SOURCE_PROVIDER_VERSION.into(),
        provider_run_id: source.lease.lease_id,
        observation_id: source_observation,
        raw_kind_code: None,
        file_id: Some(source.file_id),
        start_byte: Some(0),
        end_byte: Some(to_i64(byte_len, "source byte length")?),
        cold_payload: None,
    });

    let mut tree_entity = BTreeMap::new();
    let tree_name_spans = tree_name_spans(&tree.facts);
    let mut syntax_index = BTreeMap::new();
    let mut entity_index = BTreeMap::from([(source.file_id, 0_usize)]);
    let mut tree_observation = BTreeMap::new();
    for fact in tree.facts.iter() {
        validate_span(source, fact.start_byte, fact.end_byte, "Tree-sitter")?;
        let parent_id = fact
            .parent
            .and_then(|parent| tree_entity.get(&parent).copied());
        let role = tree_field_role(fact.field_name.as_deref());
        let kind = syntax_entity_kind(fact.normalized_kind.0, fact.error, fact.missing)?;
        let identity = source_occurrence_identity(SourceOccurrenceIdentityInput {
            workspace_id: source.workspace_id,
            file_id: source.file_id,
            source_digest: source.digest,
            start_byte: fact.start_byte,
            end_byte: fact.end_byte,
            owner_id: expected_scope.owner_id,
            entity_kind_code: governed_code(kind.code, "syntax entity kind")?,
            occurrence_family_code: OccurrenceFamily::Syntax as u16,
            normalized_kind_code: u32::from(fact.normalized_kind.0),
            parent_id,
            role_code: role,
            ordinal: fact.ordinal,
        })?;
        let entity_id = identity.id;
        tree_entity.insert(fact.id, entity_id);
        let start = to_i64(fact.start_byte, "syntax start")?;
        let end = to_i64(fact.end_byte, "syntax end")?;
        entity_index.insert(entity_id, entities.len());
        entities.push(EntityRow {
            scope: expected_scope,
            entity_id,
            language,
            entity_family_code: kind.family_code,
            entity_kind_code: kind.code,
            raw_kind_code: Some(i32::from(fact.raw_kind_id)),
            file_id: Some(source.file_id),
            start_byte: Some(start),
            end_byte: Some(end),
            name: None,
            qualified_name: None,
            parent_entity_id: parent_id,
            type_id: None,
            flags: 0,
            fact_hash64: hash64(entity_id),
        });
        syntax_index.insert(entity_id, syntax_details.len());
        syntax_details.push(SyntaxDetailRow {
            scope: expected_scope,
            entity_id,
            raw_kind_code: i32::from(fact.raw_kind_id),
            occurrence_family_code: OccurrenceFamily::Syntax as i16,
            reconciliation_step_code: ReconciliationStep::ExactRangeAndKind as i16,
            raw_kind_disposition_code: raw_disposition_code(fact.disposition),
            normalized_kind_code: i32::from(fact.normalized_kind.0),
            parent_syntax_id: parent_id,
            field_role_code: role.map(u16::cast_signed),
            ordinal: Some(to_i32(u64::from(fact.ordinal), "syntax ordinal")?),
            named: fact.named,
            extra: fact.extra,
            error: fact.error,
            missing: fact.missing,
            explicitly_parenthesized: false,
            provider_node_flags: governed_provider_node_flags(),
        });
        let observed = observation_id(runs.tree_sitter, 2, fact.id.0);
        tree_observation.insert(fact.id, observed);
        evidence_inputs.push(EvidenceInput {
            fact_id: entity_id,
            provider_code: ProviderCode::TreeSitter as i16,
            provider_version: format!("{};{}", tree.catalog_id, tree.grammar_fingerprint),
            provider_run_id: runs.tree_sitter,
            observation_id: observed,
            raw_kind_code: Some(i32::from(fact.raw_kind_id)),
            file_id: Some(source.file_id),
            start_byte: Some(start),
            end_byte: Some(end),
            cold_payload: Some(fact.raw_kind.as_bytes().to_vec()),
        });
    }

    let anchors = reconciliation_anchors(&tree.facts, &tree_entity, &tree_name_spans);
    let mut ruff_entity = BTreeMap::new();
    let mut ruff_observation = BTreeMap::new();
    if let Some(ruff) = ruff {
        let run_id = runs.ruff_python.expect("validated Ruff run identity");
        let name_spans = ruff_name_spans(&ruff.ast);
        for fact in ruff.ast.iter() {
            validate_span(source, fact.start_byte, fact.end_byte, "Ruff")?;
            let observation = RangeObservation {
                start_byte: fact.start_byte,
                end_byte: fact.end_byte,
                normalized_kind_code: fact.category.registry_code(),
                declaration_name_span: name_spans.get(&fact.id).copied(),
            };
            let preferred_entity_id = ruff
                .correspondences
                .iter()
                .find(|correspondence| correspondence.ruff_id == fact.id)
                .and_then(|correspondence| {
                    tree_entity.get(&correspondence.tree_sitter_id).copied()
                });
            let reconciled =
                reconcile_range_with_preference(observation, &anchors, preferred_entity_id);
            if reconciled.ambiguous {
                return Err(FactIngestError::Protocol(format!(
                    "Ruff source-range reconciliation is ambiguous at {}..{}",
                    fact.start_byte, fact.end_byte
                )));
            }
            let observed = observation_id(run_id, 3, fact.id.0);
            ruff_observation.insert(fact.id, observed);
            let entity_id = if let Some(entity_id) = reconciled.entity_id {
                let detail = &mut syntax_details[syntax_index[&entity_id]];
                detail.explicitly_parenthesized |= fact.explicit_parenthesized;
                detail.reconciliation_step_code = reconciled.step as i16;
                if detail.normalized_kind_code == i32::from(SyntaxKind::SyntaxNode as u16)
                    && fact.category.registry_code() != SyntaxKind::SyntaxNode as u16
                {
                    detail.normalized_kind_code = i32::from(fact.category.registry_code());
                    detail.raw_kind_code = i32::from(fact.raw_kind_id);
                    let selected = syntax_entity_kind(fact.category.registry_code(), false, false)?;
                    let entity = &mut entities[entity_index[&entity_id]];
                    entity.entity_family_code = selected.family_code;
                    entity.entity_kind_code = selected.code;
                    entity.raw_kind_code = Some(i32::from(fact.raw_kind_id));
                }
                entity_id
            } else {
                let parent_id = fact
                    .parent
                    .and_then(|parent| ruff_entity.get(&parent).copied());
                let role = ruff_field_role(fact.child_role);
                let kind = syntax_entity_kind(fact.category.registry_code(), false, false)?;
                let identity = source_occurrence_identity(SourceOccurrenceIdentityInput {
                    workspace_id: source.workspace_id,
                    file_id: source.file_id,
                    source_digest: source.digest,
                    start_byte: fact.start_byte,
                    end_byte: fact.end_byte,
                    owner_id: expected_scope.owner_id,
                    entity_kind_code: governed_code(kind.code, "Ruff syntax entity kind")?,
                    occurrence_family_code: OccurrenceFamily::Syntax as u16,
                    normalized_kind_code: u32::from(fact.category.registry_code()),
                    parent_id,
                    role_code: role,
                    ordinal: fact.child_ordinal,
                })?;
                let entity_id = identity.id;
                let start = to_i64(fact.start_byte, "Ruff syntax start")?;
                let end = to_i64(fact.end_byte, "Ruff syntax end")?;
                entity_index.insert(entity_id, entities.len());
                entities.push(EntityRow {
                    scope: expected_scope,
                    entity_id,
                    language,
                    entity_family_code: kind.family_code,
                    entity_kind_code: kind.code,
                    raw_kind_code: Some(i32::from(fact.raw_kind_id)),
                    file_id: Some(source.file_id),
                    start_byte: Some(start),
                    end_byte: Some(end),
                    name: None,
                    qualified_name: None,
                    parent_entity_id: parent_id,
                    type_id: None,
                    flags: 0,
                    fact_hash64: hash64(entity_id),
                });
                syntax_index.insert(entity_id, syntax_details.len());
                syntax_details.push(SyntaxDetailRow {
                    scope: expected_scope,
                    entity_id,
                    raw_kind_code: i32::from(fact.raw_kind_id),
                    occurrence_family_code: OccurrenceFamily::Syntax as i16,
                    reconciliation_step_code: reconciled.step as i16,
                    raw_kind_disposition_code: raw_disposition_code(fact.disposition),
                    normalized_kind_code: i32::from(fact.category.registry_code()),
                    parent_syntax_id: parent_id,
                    field_role_code: role.map(u16::cast_signed),
                    ordinal: Some(to_i32(u64::from(fact.child_ordinal), "Ruff child ordinal")?),
                    named: true,
                    extra: false,
                    error: false,
                    missing: false,
                    explicitly_parenthesized: fact.explicit_parenthesized,
                    provider_node_flags: governed_provider_node_flags(),
                });
                if let Some(tree_fact) = tree.facts.iter().find(|candidate| {
                    candidate.start_byte == fact.start_byte
                        && candidate.end_byte == fact.end_byte
                        && !compatible_kind(
                            candidate.normalized_kind.0,
                            fact.category.registry_code(),
                        )
                }) {
                    let rejected = tree_observation[&tree_fact.id];
                    rejected_observations.insert(rejected);
                    conflicts.push(ConflictRecord {
                        table_code: 170,
                        fact_id: entity_id,
                        selected_provider_code: ProviderCode::RuffPython as i16,
                        rejected_provider_code: ProviderCode::TreeSitter as i16,
                        selected_observation_id: observed,
                        rejected_observation_id: rejected,
                    });
                    diagnostics.push(IngestDiagnostic {
                        code: "SOURCE_RANGE_RECONCILIATION_CONFLICT",
                        detail: format!(
                            "Ruff {} conflicts with Tree-sitter {} at {}..{}; both occurrences retained",
                            fact.raw_kind, tree_fact.raw_kind, fact.start_byte, fact.end_byte
                        ),
                        file_id: Some(source.file_id),
                        start_byte: Some(start),
                        end_byte: Some(end),
                    });
                }
                entity_id
            };
            ruff_entity.insert(fact.id, entity_id);
            if fact.category == RuffAstCategory::DeclarationSyntax
                && let Some((start, end)) = name_spans.get(&fact.id).copied()
            {
                entities[entity_index[&entity_id]].name = source_text(source, start, end);
            }
            evidence_inputs.push(EvidenceInput {
                fact_id: entity_id,
                provider_code: ProviderCode::RuffPython as i16,
                provider_version: ruff.provider_version.into(),
                provider_run_id: run_id,
                observation_id: observed,
                raw_kind_code: Some(i32::from(fact.raw_kind_id)),
                file_id: Some(source.file_id),
                start_byte: Some(to_i64(fact.start_byte, "Ruff evidence start")?),
                end_byte: Some(to_i64(fact.end_byte, "Ruff evidence end")?),
                cold_payload: Some(fact.raw_kind.as_bytes().to_vec()),
            });
        }
        project_ruff_tokens(
            expected_scope,
            source,
            ruff,
            run_id,
            language,
            &ruff_entity,
            &mut entities,
            &mut entity_index,
            &mut source_tokens,
            &mut evidence_inputs,
        )?;
        project_ruff_annotations(
            expected_scope,
            source,
            ruff,
            run_id,
            language,
            &ruff_entity,
            &tree_entity,
            &mut entities,
            &mut entity_index,
            &mut source_annotations,
            &mut evidence_inputs,
        )?;
    }
    project_tree_recovery_annotations(
        expected_scope,
        source,
        tree,
        runs.tree_sitter,
        language,
        &tree_entity,
        &mut entities,
        &mut entity_index,
        &mut source_annotations,
        &mut evidence_inputs,
    )?;

    derive_relations(
        expected_scope,
        source,
        language,
        &entities,
        &source_tokens,
        &source_annotations,
        &syntax_details,
        ruff,
        &ruff_entity,
        &mut relations,
    )?;

    validate_cross_table(
        source,
        &entities,
        &relations,
        &source_tokens,
        &source_annotations,
        &syntax_details,
    )?;
    let provided_capabilities = ["SOURCE_BYTES", "TOKENS", "CST"];
    let owner_capability_mask = capability_mask(&provided_capabilities)
        .and_then(|mask| i64::try_from(mask).ok())
        .ok_or_else(|| FactIngestError::Protocol("generated capability mask overflow".into()))?;
    let owners = vec![OwnerRow {
        scope: expected_scope,
        parent_owner_id: None,
        owner_kind_code: OwnerKind::SourceFile as i16,
        language,
        file_id: Some(source.file_id),
        semantic_entity_id: Some(source.file_id),
        start_byte: Some(0),
        end_byte: Some(to_i64(byte_len, "source byte length")?),
        source_fingerprint: Some(source.digest),
        semantic_fingerprint: None,
        capability_mask: owner_capability_mask,
    }];
    let capability_statuses = provided_capabilities
        .into_iter()
        .map(|capability| {
            let (provider_run_id, producer_code) = if capability == "SOURCE_BYTES" {
                (source.lease.lease_id, ProviderCode::SourceSubstrate as i16)
            } else {
                (runs.tree_sitter, ProviderCode::TreeSitter as i16)
            };
            let capability_code = capability_code(capability)
                .and_then(|code| i16::try_from(code).ok())
                .ok_or_else(|| {
                    FactIngestError::Protocol(format!(
                        "unknown generated capability identifier {capability}"
                    ))
                })?;
            Ok(CapabilityStatusRow {
                scope: expected_scope,
                snapshot_id: None,
                capability_code,
                owner_capability_state_code: OwnerCapabilityState::Current as i16,
                completeness_state_code: CompletenessState::Complete as i16,
                provider_run_id: Some(provider_run_id),
                producer_code: Some(producer_code),
                reason_code: None,
                diagnostic_id: None,
                fallback_source_available: true,
                coverage_scope_fingerprint: capability_scope_fingerprint(
                    expected_scope,
                    capability,
                ),
            })
        })
        .collect::<Result<Vec<_>, FactIngestError>>()?;
    let source_files = vec![source_file_row(expected_scope, source)?];
    let mut evidence = evidence_inputs
        .into_iter()
        .map(|input| evidence_row(expected_scope, input, &rejected_observations))
        .collect::<Vec<_>>();
    entities.sort_by_key(|row| row.entity_id);
    relations.sort_by_key(|row| row.fact_id);
    source_tokens.sort_by_key(|row| row.token_id);
    source_annotations.sort_by_key(|row| row.annotation_id);
    syntax_details.sort_by_key(|row| row.entity_id);
    evidence.sort_by_key(|row| row.evidence_id);
    ensure_row_budgets(&[
        entities.len(),
        relations.len(),
        evidence.len(),
        source_tokens.len(),
        source_annotations.len(),
        syntax_details.len(),
    ])?;
    let batches = [
        (8, encode_owners(&owners)?),
        (9, encode_capability_statuses(&capability_statuses)?),
        (100, encode_entities(&entities)?),
        (110, encode_relations(&relations)?),
        (120, encode_properties(&properties)?),
        (130, encode_evidence(&evidence)?),
        (140, encode_source_files(&source_files)?),
        (150, encode_source_tokens(&source_tokens)?),
        (160, encode_source_annotations(&source_annotations)?),
        (170, encode_syntax_details(&syntax_details)?),
    ]
    .into_iter()
    .map(|(code, batch)| {
        ValidatedFactBatch::validate(code, batch, expected_scope).map(|batch| (code, batch))
    })
    .collect::<Result<BTreeMap<_, _>, _>>()?;
    let rows_encoded = batches
        .values()
        .map(ValidatedFactBatch::num_rows)
        .map(|rows| u64::try_from(rows).unwrap_or(u64::MAX))
        .sum();
    let rows_received = 1_u64
        .saturating_add(u64::try_from(tree.facts.len()).unwrap_or(u64::MAX))
        .saturating_add(ruff.map_or(0, |snapshot| snapshot.metrics.output_records));
    let metrics = IngestMetrics {
        streams_received: 2 + u64::from(ruff.is_some()),
        rows_received,
        rows_encoded,
        validation_failures: 0,
        conflicts: u64::try_from(conflicts.len()).unwrap_or(u64::MAX),
    };
    Ok(CanonicalIngestOutput {
        batches,
        conflicts,
        diagnostics,
        metrics,
    })
}

fn capability_scope_fingerprint(scope: FactScope, capability: &str) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric-capability-scope-v1\0");
    hasher.update(&scope.workspace_id);
    hasher.update(&scope.analysis_context_id);
    hasher.update(&scope.owner_id);
    hasher.update(&scope.source_generation.to_be_bytes());
    hasher.update(capability.as_bytes());
    *hasher.finalize().as_bytes()
}

fn validate_inputs(
    scope: FactScope,
    source: &SourceImage,
    tree: &TreeSitterSnapshot,
    ruff: Option<&RuffSnapshot>,
    runs: SourceSyntaxProviderRuns,
) -> Result<(), FactIngestError> {
    if scope.analysis_context_id != SOURCE_CONTEXT_ID {
        return Err(FactIngestError::SourceSnapshotMismatch(
            "source/syntax facts require context:source".into(),
        ));
    }
    if scope.workspace_id != source.workspace_id || source.path.workspace_id != source.workspace_id
    {
        return Err(FactIngestError::SourceSnapshotMismatch(
            "source workspace".into(),
        ));
    }
    if scope.source_generation
        != i64::try_from(source.source_generation)
            .map_err(|_| FactIngestError::Protocol("source generation overflow".into()))?
    {
        return Err(FactIngestError::StaleResult("source image".into()));
    }
    if source.file_id != source_file_identity(&source.path)?.id
        || source.digest != *blake3::hash(&source.bytes).as_bytes()
        || source.byte_length != u64::try_from(source.bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(FactIngestError::SourceSnapshotMismatch(
            "source image identity".into(),
        ));
    }
    let text = source.provider_text.as_ref().ok_or_else(|| {
        FactIngestError::Protocol("provider-compatible source text is absent".into())
    })?;
    let fingerprint = text.provider_image_fingerprint();
    if tree.provider_image_fingerprint != fingerprint {
        return Err(FactIngestError::SourceSnapshotMismatch(
            "Tree-sitter provider image".into(),
        ));
    }
    match source.language {
        SourceLanguage::Python if !tree.catalog_id.contains("python") => {
            return Err(FactIngestError::SourceSnapshotMismatch(
                "Tree-sitter language catalog".into(),
            ));
        }
        SourceLanguage::Rust if !tree.catalog_id.contains("rust") => {
            return Err(FactIngestError::SourceSnapshotMismatch(
                "Tree-sitter language catalog".into(),
            ));
        }
        SourceLanguage::Other => {
            return Err(FactIngestError::Protocol(
                "unclassified source cannot enter syntax projection".into(),
            ));
        }
        _ => {}
    }
    if let Some(ruff) = ruff {
        if source.language != SourceLanguage::Python
            || ruff.source.provider_image_fingerprint != fingerprint
            || runs.ruff_python.is_none()
        {
            return Err(FactIngestError::SourceSnapshotMismatch(
                "Ruff provider image or run".into(),
            ));
        }
    } else if runs.ruff_python.is_some() {
        return Err(FactIngestError::Protocol(
            "Ruff run supplied without a complete snapshot".into(),
        ));
    }
    validate_provider_structure(source, tree, ruff)?;
    Ok(())
}

fn validate_provider_structure(
    source: &SourceImage,
    tree: &TreeSitterSnapshot,
    ruff: Option<&RuffSnapshot>,
) -> Result<(), FactIngestError> {
    let mut tree_by_id = BTreeMap::new();
    for fact in tree.facts.iter() {
        validate_span(source, fact.start_byte, fact.end_byte, "Tree-sitter")?;
        if tree_by_id.insert(fact.id, fact).is_some() {
            return Err(FactIngestError::Protocol(
                "Tree-sitter emitted a duplicate occurrence identity".into(),
            ));
        }
    }
    for fact in tree.facts.iter() {
        if let Some(parent_id) = fact.parent {
            let parent = tree_by_id.get(&parent_id).ok_or_else(|| {
                FactIngestError::Protocol("Tree-sitter parent occurrence is absent".into())
            })?;
            if parent.id == fact.id
                || parent.start_byte > fact.start_byte
                || parent.end_byte < fact.end_byte
            {
                return Err(FactIngestError::Protocol(
                    "Tree-sitter emitted an invalid parent/source-range relationship".into(),
                ));
            }
        }
    }

    let Some(ruff) = ruff else {
        return Ok(());
    };
    let mut ruff_by_id = BTreeMap::new();
    for fact in ruff.ast.iter() {
        validate_span(source, fact.start_byte, fact.end_byte, "Ruff AST")?;
        if ruff_by_id.insert(fact.id, fact).is_some() {
            return Err(FactIngestError::Protocol(
                "Ruff emitted a duplicate occurrence identity".into(),
            ));
        }
    }
    for fact in ruff.ast.iter() {
        if let Some(parent_id) = fact.parent {
            let parent = ruff_by_id.get(&parent_id).ok_or_else(|| {
                FactIngestError::Protocol("Ruff parent occurrence is absent".into())
            })?;
            if parent.id == fact.id
                || parent.start_byte > fact.start_byte
                || parent.end_byte < fact.end_byte
            {
                return Err(FactIngestError::Protocol(
                    "Ruff emitted an invalid parent/source-range relationship".into(),
                ));
            }
        }
    }

    let mut tokens = ruff.tokens.iter().collect::<Vec<_>>();
    tokens.sort_by_key(|token| (token.start_byte, token.end_byte, token.ordinal));
    let mut previous_nonempty_end = 0_u64;
    for token in tokens {
        validate_span(source, token.start_byte, token.end_byte, "Ruff token")?;
        if token.start_byte != token.end_byte && token.start_byte < previous_nonempty_end {
            return Err(FactIngestError::Protocol(
                "Ruff emitted overlapping token spans".into(),
            ));
        }
        if token.start_byte != token.end_byte {
            previous_nonempty_end = token.end_byte;
        }
    }
    for (start, end, label) in ruff
        .comments
        .iter()
        .map(|fact| (fact.start_byte, fact.end_byte, "Ruff comment"))
        .chain(
            ruff.directives
                .iter()
                .map(|fact| (fact.start_byte, fact.end_byte, "Ruff directive")),
        )
        .chain(
            ruff.strings
                .iter()
                .map(|fact| (fact.start_byte, fact.end_byte, "Ruff string")),
        )
        .chain(
            ruff.docstrings
                .iter()
                .map(|fact| (fact.start_byte, fact.end_byte, "Ruff docstring")),
        )
        .chain(
            ruff.diagnostics
                .iter()
                .map(|fact| (fact.start_byte, fact.end_byte, "Ruff diagnostic")),
        )
    {
        validate_span(source, start, end, label)?;
    }
    for offset in ruff.continuation_line_starts.iter().copied() {
        validate_span(source, offset, offset, "Ruff continuation line")?;
    }
    Ok(())
}

fn source_file_row(
    scope: FactScope,
    source: &SourceImage,
) -> Result<SourceFileRow, FactIngestError> {
    let encoding_name = match &source.encoding {
        SourceEncoding::Utf8 => Some("utf-8".into()),
        SourceEncoding::Utf8Bom => Some("utf-8-sig".into()),
        SourceEncoding::PythonLatin1 => Some("latin-1".into()),
        SourceEncoding::Unsupported { declared } => declared.clone(),
    };
    let line_start_offsets = source
        .line_index
        .offsets
        .iter()
        .map(|offset| to_i64(*offset, "line start"))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SourceFileRow {
        scope,
        file_id: source.file_id,
        path_bytes: source.path.raw_relative_path_bytes.clone(),
        path_display: source.path.display_string.clone(),
        path_encoding_code: path_encoding(source.path.platform_code),
        path_case_key: Some(source.path.comparison_key_bytes.clone()),
        path_display_is_lossy: source.path.display_is_lossy,
        language: language_code(source.language),
        source_digest: source.digest,
        byte_len: to_i64(source.byte_length, "source byte length")?,
        line_count: to_i32(
            u64::try_from(line_start_offsets.len()).unwrap_or(u64::MAX),
            "line count",
        )?,
        encoding_name,
        newline_kind_code: newline_kind(source.line_index.newline_kind),
        source_bytes: source.bytes.to_vec(),
        decoded_text: source
            .provider_text
            .as_ref()
            .map(|text| text.text.to_string()),
        line_start_offsets,
        module_entity_id: None,
        is_stub: source.path.raw_relative_path_bytes.ends_with(b".pyi"),
        flags: 0,
    })
}

#[allow(clippy::too_many_arguments)]
fn project_ruff_tokens(
    scope: FactScope,
    source: &SourceImage,
    ruff: &RuffSnapshot,
    run_id: [u8; 16],
    language: i16,
    ruff_entity: &BTreeMap<RuffOccurrenceId, [u8; 16]>,
    entities: &mut Vec<EntityRow>,
    entity_index: &mut BTreeMap<[u8; 16], usize>,
    rows: &mut Vec<SourceTokenRow>,
    evidence: &mut Vec<EvidenceInput>,
) -> Result<(), FactIngestError> {
    for token in ruff.tokens.iter() {
        validate_span(source, token.start_byte, token.end_byte, "Ruff token")?;
        let normalized = token_kind(token);
        let entity_kind = required_entity_kind(token_entity_kind(normalized))?;
        let identity = source_occurrence_identity(SourceOccurrenceIdentityInput {
            workspace_id: source.workspace_id,
            file_id: source.file_id,
            source_digest: source.digest,
            start_byte: token.start_byte,
            end_byte: token.end_byte,
            owner_id: scope.owner_id,
            entity_kind_code: governed_code(entity_kind.code, "token entity kind")?,
            occurrence_family_code: OccurrenceFamily::Token as u16,
            normalized_kind_code: normalized as u32,
            parent_id: token.syntax_id.and_then(|id| ruff_entity.get(&id).copied()),
            role_code: Some(normalized as u16),
            ordinal: token.ordinal,
        })?;
        let token_id = identity.id;
        let start = to_i64(token.start_byte, "token start")?;
        let end = to_i64(token.end_byte, "token end")?;
        entity_index.insert(token_id, entities.len());
        entities.push(EntityRow {
            scope,
            entity_id: token_id,
            language,
            entity_family_code: entity_kind.family_code,
            entity_kind_code: entity_kind.code,
            raw_kind_code: Some(i32::from(token.raw_kind_id)),
            file_id: Some(source.file_id),
            start_byte: Some(start),
            end_byte: Some(end),
            name: None,
            qualified_name: None,
            parent_entity_id: token.syntax_id.and_then(|id| ruff_entity.get(&id).copied()),
            type_id: None,
            flags: 0,
            fact_hash64: hash64(token_id),
        });
        rows.push(SourceTokenRow {
            scope,
            token_id,
            file_id: source.file_id,
            ordinal: to_i32(u64::from(token.ordinal), "token ordinal")?,
            token_kind_code: i32::from(normalized as u16),
            start_byte: start,
            end_byte: end,
            normalized_value: match &token.spelling {
                Some(RuffTokenSpelling::Slice(value) | RuffTokenSpelling::Blake3(value)) => {
                    Some(value.clone())
                }
                None => None,
            },
            flags: 0,
        });
        evidence.push(EvidenceInput {
            fact_id: token_id,
            provider_code: ProviderCode::RuffPython as i16,
            provider_version: ruff.provider_version.into(),
            provider_run_id: run_id,
            observation_id: observation_id(run_id, 4, u64::from(token.ordinal)),
            raw_kind_code: Some(i32::from(token.raw_kind_id)),
            file_id: Some(source.file_id),
            start_byte: Some(start),
            end_byte: Some(end),
            cold_payload: Some(token.raw_kind.as_bytes().to_vec()),
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn project_ruff_annotations(
    scope: FactScope,
    source: &SourceImage,
    ruff: &RuffSnapshot,
    run_id: [u8; 16],
    language: i16,
    ruff_entity: &BTreeMap<RuffOccurrenceId, [u8; 16]>,
    tree_entity: &BTreeMap<SyntaxOccurrenceId, [u8; 16]>,
    entities: &mut Vec<EntityRow>,
    entity_index: &mut BTreeMap<[u8; 16], usize>,
    rows: &mut Vec<SourceAnnotationRow>,
    evidence: &mut Vec<EvidenceInput>,
) -> Result<(), FactIngestError> {
    for (ordinal, comment) in ruff.comments.iter().enumerate() {
        push_annotation(
            scope,
            source,
            language,
            AnnotationKind::Comment,
            comment.start_byte,
            comment.end_byte,
            None,
            source_text(source, comment.start_byte, comment.end_byte),
            None,
            0,
            ProviderCode::RuffPython as i16,
            ruff.provider_version,
            run_id,
            observation_id(run_id, 5, u64::try_from(ordinal).unwrap_or(u64::MAX)),
            Some(
                format!(
                    "placement={:?};block_member={}",
                    comment.placement, comment.block_member
                )
                .into_bytes(),
            ),
            entities,
            entity_index,
            rows,
            evidence,
        )?;
    }
    for (ordinal, directive) in ruff.directives.iter().enumerate() {
        let target = directive
            .target
            .and_then(|id| ruff_entity.get(&id).copied());
        push_annotation(
            scope,
            source,
            language,
            AnnotationKind::PragmaOrDirective,
            directive.start_byte,
            directive.end_byte,
            target,
            source_text(source, directive.start_byte, directive.end_byte),
            Some(directive_code(directive.kind)),
            0,
            ProviderCode::RuffPython as i16,
            ruff.provider_version,
            run_id,
            observation_id(run_id, 6, u64::try_from(ordinal).unwrap_or(u64::MAX)),
            None,
            entities,
            entity_index,
            rows,
            evidence,
        )?;
    }
    for (ordinal, docstring) in ruff.docstrings.iter().enumerate() {
        let target = ruff_entity.get(&docstring.owner).copied();
        push_annotation(
            scope,
            source,
            language,
            AnnotationKind::Documentation,
            docstring.start_byte,
            docstring.end_byte,
            target,
            source_text(source, docstring.start_byte, docstring.end_byte),
            None,
            0,
            ProviderCode::RuffPython as i16,
            ruff.provider_version,
            run_id,
            observation_id(run_id, 7, u64::try_from(ordinal).unwrap_or(u64::MAX)),
            None,
            entities,
            entity_index,
            rows,
            evidence,
        )?;
    }
    for (ordinal, diagnostic) in ruff.diagnostics.iter().enumerate() {
        let target = diagnostic
            .tree_sitter_recovery_ids
            .iter()
            .find_map(|id| tree_entity.get(id).copied());
        let code = match diagnostic.kind {
            crate::ruff_adapter::RuffDiagnosticKind::Parse => 10,
            crate::ruff_adapter::RuffDiagnosticKind::UnsupportedSyntax => 20,
        };
        push_annotation(
            scope,
            source,
            language,
            AnnotationKind::ParseError,
            diagnostic.start_byte,
            diagnostic.end_byte,
            target,
            Some(diagnostic.message.clone()),
            Some(code),
            0,
            ProviderCode::RuffPython as i16,
            ruff.provider_version,
            run_id,
            observation_id(run_id, 8, u64::try_from(ordinal).unwrap_or(u64::MAX)),
            None,
            entities,
            entity_index,
            rows,
            evidence,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn project_tree_recovery_annotations(
    scope: FactScope,
    source: &SourceImage,
    tree: &TreeSitterSnapshot,
    run_id: [u8; 16],
    language: i16,
    tree_entity: &BTreeMap<SyntaxOccurrenceId, [u8; 16]>,
    entities: &mut Vec<EntityRow>,
    entity_index: &mut BTreeMap<[u8; 16], usize>,
    rows: &mut Vec<SourceAnnotationRow>,
    evidence: &mut Vec<EvidenceInput>,
) -> Result<(), FactIngestError> {
    for fact in tree.facts.iter().filter(|fact| fact.error || fact.missing) {
        let kind = if fact.missing {
            AnnotationKind::MissingSyntax
        } else {
            AnnotationKind::ParseError
        };
        push_annotation(
            scope,
            source,
            language,
            kind,
            fact.start_byte,
            fact.end_byte,
            tree_entity.get(&fact.id).copied(),
            None,
            Some(if fact.missing { 40 } else { 30 }),
            0,
            ProviderCode::TreeSitter as i16,
            &format!("{};{}", tree.catalog_id, tree.grammar_fingerprint),
            run_id,
            observation_id(run_id, 9, fact.id.0),
            Some(fact.raw_kind.as_bytes().to_vec()),
            entities,
            entity_index,
            rows,
            evidence,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push_annotation(
    scope: FactScope,
    source: &SourceImage,
    language: i16,
    annotation_kind: AnnotationKind,
    start_byte: u64,
    end_byte: u64,
    target_entity_id: Option<[u8; 16]>,
    text: Option<String>,
    diagnostic_code: Option<i32>,
    flags: i64,
    provider_code: i16,
    provider_version: &str,
    provider_run_id: [u8; 16],
    observed: [u8; 16],
    cold_payload: Option<Vec<u8>>,
    entities: &mut Vec<EntityRow>,
    entity_index: &mut BTreeMap<[u8; 16], usize>,
    rows: &mut Vec<SourceAnnotationRow>,
    evidence: &mut Vec<EvidenceInput>,
) -> Result<[u8; 16], FactIngestError> {
    validate_span(source, start_byte, end_byte, "source annotation")?;
    let ordinal = u32::try_from(rows.len())
        .map_err(|_| FactIngestError::Protocol("annotation ordinal overflow".into()))?;
    let entity_kind = required_entity_kind(annotation_entity_kind(annotation_kind))?;
    let identity = source_occurrence_identity(SourceOccurrenceIdentityInput {
        workspace_id: source.workspace_id,
        file_id: source.file_id,
        source_digest: source.digest,
        start_byte,
        end_byte,
        owner_id: scope.owner_id,
        entity_kind_code: governed_code(entity_kind.code, "annotation entity kind")?,
        occurrence_family_code: OccurrenceFamily::Annotation as u16,
        normalized_kind_code: annotation_kind as u32,
        parent_id: target_entity_id,
        role_code: Some(annotation_kind as u16),
        ordinal,
    })?;
    let id = identity.id;
    let start = to_i64(start_byte, "annotation start")?;
    let end = to_i64(end_byte, "annotation end")?;
    entity_index.insert(id, entities.len());
    entities.push(EntityRow {
        scope,
        entity_id: id,
        language,
        entity_family_code: entity_kind.family_code,
        entity_kind_code: entity_kind.code,
        raw_kind_code: None,
        file_id: Some(source.file_id),
        start_byte: Some(start),
        end_byte: Some(end),
        name: None,
        qualified_name: None,
        parent_entity_id: target_entity_id,
        type_id: None,
        flags,
        fact_hash64: hash64(id),
    });
    rows.push(SourceAnnotationRow {
        scope,
        annotation_id: id,
        file_id: source.file_id,
        annotation_kind_code: i32::from(annotation_kind as u16),
        start_byte: start,
        end_byte: end,
        target_entity_id,
        text,
        diagnostic_code,
        flags,
    });
    evidence.push(EvidenceInput {
        fact_id: id,
        provider_code,
        provider_version: provider_version.into(),
        provider_run_id,
        observation_id: observed,
        raw_kind_code: None,
        file_id: Some(source.file_id),
        start_byte: Some(start),
        end_byte: Some(end),
        cold_payload,
    });
    Ok(id)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn derive_relations(
    scope: FactScope,
    source: &SourceImage,
    language: i16,
    entities: &[EntityRow],
    tokens: &[SourceTokenRow],
    annotations: &[SourceAnnotationRow],
    syntax: &[SyntaxDetailRow],
    ruff: Option<&RuffSnapshot>,
    ruff_entity: &BTreeMap<RuffOccurrenceId, [u8; 16]>,
    output: &mut Vec<RelationRow>,
) -> Result<(), FactIngestError> {
    let mut unique = BTreeMap::new();
    let mut ranged = entities
        .iter()
        .filter(|entity| entity.entity_id != source.file_id)
        .filter_map(|entity| Some((entity.entity_id, entity.start_byte?, entity.end_byte?)))
        .collect::<Vec<_>>();
    ranged.sort_by_key(|(id, start, end)| (*start, *end, *id));
    for (ordinal, (target, start, end)) in ranged.into_iter().enumerate() {
        insert_relation(
            &mut unique,
            scope,
            source,
            language,
            "CONTAINS_SPAN",
            source.file_id,
            target,
            Some(u32::try_from(ordinal).unwrap_or(u32::MAX)),
            None,
            Some((start, end)),
        )?;
    }
    for detail in syntax {
        if let Some(parent) = detail.parent_syntax_id {
            insert_relation(
                &mut unique,
                scope,
                source,
                language,
                "AST_CHILD",
                parent,
                detail.entity_id,
                detail.ordinal.and_then(|value| u32::try_from(value).ok()),
                detail
                    .field_role_code
                    .and_then(|value| u16::try_from(value).ok()),
                None,
            )?;
        }
    }
    for (index, token) in tokens.iter().enumerate() {
        let target = ruff
            .and_then(|snapshot| snapshot.tokens.get(index))
            .and_then(|fact| fact.syntax_id)
            .and_then(|id| ruff_entity.get(&id).copied())
            .unwrap_or(source.file_id);
        insert_relation(
            &mut unique,
            scope,
            source,
            language,
            "TOKEN_OF",
            token.token_id,
            target,
            Some(u32::try_from(index).unwrap_or(u32::MAX)),
            None,
            Some((token.start_byte, token.end_byte)),
        )?;
    }
    for pair in tokens.windows(2) {
        insert_relation(
            &mut unique,
            scope,
            source,
            language,
            "LEXICALLY_PRECEDES",
            pair[0].token_id,
            pair[1].token_id,
            None,
            None,
            None,
        )?;
    }
    for annotation in annotations {
        let relation = match AnnotationKind::try_from(
            u16::try_from(annotation.annotation_kind_code).unwrap_or_default(),
        ) {
            Ok(AnnotationKind::Documentation) => Some("DOCUMENTS"),
            Ok(AnnotationKind::PragmaOrDirective) => Some("DIRECTIVE_APPLIES_TO"),
            _ => None,
        };
        if let (Some(relation), Some(target)) = (relation, annotation.target_entity_id) {
            insert_relation(
                &mut unique,
                scope,
                source,
                language,
                relation,
                annotation.annotation_id,
                target,
                None,
                None,
                Some((annotation.start_byte, annotation.end_byte)),
            )?;
        }
    }
    output.extend(unique.into_values());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_relation(
    output: &mut BTreeMap<[u8; 16], RelationRow>,
    scope: FactScope,
    source: &SourceImage,
    language: i16,
    name: &str,
    source_id: [u8; 16],
    target_id: [u8; 16],
    ordinal: Option<u32>,
    role_code: Option<u16>,
    span: Option<(i64, i64)>,
) -> Result<(), FactIngestError> {
    let kind = relation_kind(name)
        .ok_or_else(|| FactIngestError::Protocol(format!("relation kind {name} is absent")))?;
    let identity = source_relation_identity(SourceRelationIdentityInput {
        workspace_id: source.workspace_id,
        owner_id: scope.owner_id,
        relation_kind_code: kind.code,
        source_id,
        target_id,
        ordinal,
        role_code,
    })?;
    output.entry(identity.id).or_insert(RelationRow {
        scope,
        fact_id: identity.id,
        language,
        relation_family_code: kind.family_code,
        relation_kind_code: kind.code,
        source_id,
        target_id,
        ordinal: ordinal.map(|value| i32::try_from(value).unwrap_or(i32::MAX)),
        role_code: role_code.map(u16::cast_signed),
        distance: None,
        directness_code: 10,
        file_id: span.map(|_| source.file_id),
        start_byte: span.map(|value| value.0),
        end_byte: span.map(|value| value.1),
        certainty_code: 10,
        resolution_code: 10,
        producer_code: ProviderCode::CodefabricDerivation as i16,
        derivation_code: None,
        flags: 0,
        fact_hash64: hash64(identity.id),
    });
    Ok(())
}

fn validate_cross_table(
    source: &SourceImage,
    entities: &[EntityRow],
    relations: &[RelationRow],
    tokens: &[SourceTokenRow],
    annotations: &[SourceAnnotationRow],
    syntax: &[SyntaxDetailRow],
) -> Result<(), FactIngestError> {
    let ids = entities
        .iter()
        .map(|entity| entity.entity_id)
        .collect::<BTreeSet<_>>();
    if ids.len() != entities.len() {
        return Err(batch_error(
            100,
            "entity-identity",
            "duplicate canonical entity",
        ));
    }
    if relations
        .iter()
        .any(|relation| !ids.contains(&relation.source_id) || !ids.contains(&relation.target_id))
    {
        return Err(batch_error(
            110,
            "edge-endpoint",
            "relation endpoint is absent",
        ));
    }
    if syntax.iter().any(|row| {
        !ids.contains(&row.entity_id)
            || row
                .parent_syntax_id
                .is_some_and(|parent| !ids.contains(&parent))
    }) {
        return Err(batch_error(
            170,
            "syntax-endpoint",
            "syntax entity or parent is absent",
        ));
    }
    if annotations.iter().any(|row| {
        !ids.contains(&row.annotation_id)
            || row
                .target_entity_id
                .is_some_and(|target| !ids.contains(&target))
    }) {
        return Err(batch_error(
            160,
            "annotation-endpoint",
            "annotation entity or target is absent",
        ));
    }
    let max = i64::try_from(source.byte_length).unwrap_or(i64::MAX);
    let spans = tokens
        .iter()
        .map(|row| (row.start_byte, row.end_byte))
        .chain(annotations.iter().map(|row| (row.start_byte, row.end_byte)));
    if spans
        .into_iter()
        .any(|(start, end)| !valid_span(start, end, max))
    {
        return Err(batch_error(
            140,
            "source-boundary",
            "span exceeds source bytes",
        ));
    }
    Ok(())
}

const fn valid_span(start: i64, end: i64, maximum: i64) -> bool {
    start >= 0 && end >= start && end <= maximum
}

fn evidence_row(
    scope: FactScope,
    input: EvidenceInput,
    rejected: &BTreeSet<[u8; 16]>,
) -> FactEvidenceRow {
    FactEvidenceRow {
        evidence_id: evidence_id(input.provider_run_id, input.observation_id, input.fact_id),
        scope,
        fact_id: input.fact_id,
        fact_form_code: 10,
        provider_code: input.provider_code,
        provider_version: input.provider_version,
        provider_run_id: input.provider_run_id,
        observation_id: input.observation_id,
        raw_kind_code: input.raw_kind_code,
        file_id: input.file_id,
        start_byte: input.start_byte,
        end_byte: input.end_byte,
        certainty_code: 10,
        resolution_code: 10,
        conflict_disposition_code: if rejected.contains(&input.observation_id) {
            20
        } else {
            10
        },
        cold_payload: input.cold_payload,
    }
}

fn evidence_id(run: [u8; 16], observation: [u8; 16], fact: [u8; 16]) -> [u8; 16] {
    digest16(&[b"codefabric-fact-evidence-v1\0", &run, &observation, &fact])
}

fn observation_id(run: [u8; 16], family: u8, ordinal: u64) -> [u8; 16] {
    digest16(&[
        b"codefabric-source-observation-v1\0",
        &run,
        &[family],
        &ordinal.to_be_bytes(),
    ])
}

fn digest16(parts: &[&[u8]]) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    for part in parts {
        hasher.update(part);
    }
    let mut output = [0; 16];
    output.copy_from_slice(&hasher.finalize().as_bytes()[..16]);
    output
}

fn hash64(id: [u8; 16]) -> i64 {
    i64::from_be_bytes(id[..8].try_into().expect("eight-byte identity prefix"))
}

fn language_code(language: SourceLanguage) -> i16 {
    match language {
        SourceLanguage::Python => Language::Python as i16,
        SourceLanguage::Rust => Language::Rust as i16,
        SourceLanguage::Other => Language::Unknown as i16,
    }
}

fn path_encoding(platform: PlatformCode) -> i16 {
    match platform {
        PlatformCode::Unix => PathEncoding::UnixBytes as i16,
        PlatformCode::MacOs => PathEncoding::MacosBytes as i16,
        PlatformCode::WindowsWtf8 => PathEncoding::WindowsWtf8 as i16,
    }
}

fn newline_kind(kind: NewlineKind) -> i16 {
    match kind {
        NewlineKind::None => RegistryNewlineKind::None as i16,
        NewlineKind::Lf => RegistryNewlineKind::Lf as i16,
        NewlineKind::CrLf => RegistryNewlineKind::Crlf as i16,
        NewlineKind::Cr => RegistryNewlineKind::Cr as i16,
        NewlineKind::Mixed => RegistryNewlineKind::Mixed as i16,
    }
}

fn required_entity_kind(
    name: &str,
) -> Result<crate::registries::OntologyCodeEntry, FactIngestError> {
    entity_kind(name)
        .ok_or_else(|| FactIngestError::Protocol(format!("entity kind {name} is absent")))
}

fn syntax_entity_kind(
    normalized: u16,
    error: bool,
    missing: bool,
) -> Result<crate::registries::OntologyCodeEntry, FactIngestError> {
    if missing {
        return required_entity_kind("MISSING_SYNTAX");
    }
    if error {
        return required_entity_kind("PARSE_ERROR");
    }
    let name = registry_state_name(SYNTAX_KIND_VALUES, normalized)
        .ok_or_else(|| FactIngestError::Protocol(format!("syntax kind {normalized} is absent")))?;
    required_entity_kind(name)
}

fn tree_name_spans(facts: &[RawSyntaxFact]) -> BTreeMap<SyntaxOccurrenceId, (u64, u64)> {
    facts
        .iter()
        .filter(|fact| fact.field_name.as_deref() == Some("name"))
        .filter_map(|fact| {
            fact.parent
                .map(|parent| (parent, (fact.start_byte, fact.end_byte)))
        })
        .collect()
}

fn ruff_name_spans(facts: &[RuffAstFact]) -> BTreeMap<RuffOccurrenceId, (u64, u64)> {
    facts
        .iter()
        .filter(|fact| fact.child_role == Some(RuffChildRole::Name))
        .filter_map(|fact| {
            fact.parent
                .map(|parent| (parent, (fact.start_byte, fact.end_byte)))
        })
        .collect()
}

fn tree_field_role(field: Option<&str>) -> Option<u16> {
    Some(match field? {
        "name" => SyntaxFieldRole::Name,
        "parameters" | "type_parameters" => SyntaxFieldRole::Parameters,
        "decorator" => SyntaxFieldRole::Decorator,
        "return_type" => SyntaxFieldRole::Returns,
        "body" | "consequence" | "alternative" => SyntaxFieldRole::Body,
        "condition" => SyntaxFieldRole::Condition,
        "left" | "target" => SyntaxFieldRole::Target,
        "right" | "value" => SyntaxFieldRole::Value,
        "object" => SyntaxFieldRole::Receiver,
        "function" => SyntaxFieldRole::Callee,
        "arguments" => SyntaxFieldRole::Argument,
        "iterable" => SyntaxFieldRole::Iterable,
        "guard" => SyntaxFieldRole::Guard,
        "pattern" => SyntaxFieldRole::Pattern,
        "handler" => SyntaxFieldRole::Handler,
        "finally" => SyntaxFieldRole::FinallyBody,
        _ => return None,
    } as u16)
}

fn ruff_field_role(role: Option<RuffChildRole>) -> Option<u16> {
    Some(match role? {
        RuffChildRole::Body => SyntaxFieldRole::Body,
        RuffChildRole::Decorator => SyntaxFieldRole::Decorator,
        RuffChildRole::Name => SyntaxFieldRole::Name,
        RuffChildRole::TypeParameter | RuffChildRole::Parameter => SyntaxFieldRole::Parameters,
        RuffChildRole::Argument => SyntaxFieldRole::Argument,
        RuffChildRole::KeywordArgument => SyntaxFieldRole::KeywordArgument,
        RuffChildRole::Callee => SyntaxFieldRole::Callee,
        RuffChildRole::Condition => SyntaxFieldRole::Condition,
        RuffChildRole::Target => SyntaxFieldRole::Target,
        RuffChildRole::Value => SyntaxFieldRole::Value,
        RuffChildRole::Annotation => SyntaxFieldRole::Returns,
        RuffChildRole::Iterable => SyntaxFieldRole::Iterable,
        RuffChildRole::Pattern => SyntaxFieldRole::Pattern,
        RuffChildRole::Handler => SyntaxFieldRole::Handler,
        RuffChildRole::Clause => SyntaxFieldRole::FinallyBody,
        RuffChildRole::Item | RuffChildRole::Segment | RuffChildRole::Child => return None,
    } as u16)
}

fn token_kind(token: &RuffTokenFact) -> TokenKind {
    match token.class {
        RuffTokenClass::Identifier => TokenKind::Identifier,
        RuffTokenClass::Keyword => TokenKind::Keyword,
        RuffTokenClass::Literal if token.raw_kind.contains("String") => TokenKind::String,
        RuffTokenClass::Literal if matches!(token.raw_kind, "Int" | "Float" | "Complex") => {
            TokenKind::Number
        }
        RuffTokenClass::Literal => TokenKind::Literal,
        RuffTokenClass::Operator if punctuation(token.raw_kind) => TokenKind::Punctuation,
        RuffTokenClass::Operator => TokenKind::Operator,
        RuffTokenClass::Newline | RuffTokenClass::Indentation | RuffTokenClass::EndOfFile => {
            TokenKind::Punctuation
        }
        RuffTokenClass::Comment | RuffTokenClass::Unknown => TokenKind::Unknown,
    }
}

fn punctuation(raw: &str) -> bool {
    matches!(
        raw,
        "Lpar"
            | "Rpar"
            | "Lsqb"
            | "Rsqb"
            | "Lbrace"
            | "Rbrace"
            | "Colon"
            | "Comma"
            | "Semi"
            | "Dot"
    )
}

const fn token_entity_kind(kind: TokenKind) -> &'static str {
    match kind {
        TokenKind::Identifier => "IDENTIFIER_TOKEN",
        TokenKind::Keyword => "KEYWORD_TOKEN",
        TokenKind::Operator => "OPERATOR_TOKEN",
        TokenKind::Punctuation => "PUNCTUATION_TOKEN",
        TokenKind::Literal => "LITERAL_TOKEN",
        TokenKind::String => "STRING_TOKEN",
        TokenKind::Number => "NUMBER_TOKEN",
        TokenKind::Unknown => "TOKEN",
    }
}

const fn annotation_entity_kind(kind: AnnotationKind) -> &'static str {
    match kind {
        AnnotationKind::Comment => "COMMENT",
        AnnotationKind::Documentation => "DOCUMENTATION",
        AnnotationKind::PragmaOrDirective => "PRAGMA_OR_DIRECTIVE",
        AnnotationKind::ParseError => "PARSE_ERROR",
        AnnotationKind::MissingSyntax => "MISSING_SYNTAX",
    }
}

const fn directive_code(kind: RuffDirectiveKind) -> i32 {
    match kind {
        RuffDirectiveKind::Noqa => 10,
        RuffDirectiveKind::TypeIgnore => 20,
        RuffDirectiveKind::TypeComment => 30,
        RuffDirectiveKind::Formatter => 40,
        RuffDirectiveKind::OtherPragma => 50,
    }
}

const fn raw_disposition_code(disposition: ProviderRawKindDisposition) -> i16 {
    match disposition {
        ProviderRawKindDisposition::Normalize => RawKindDisposition::Normalize as i16,
        ProviderRawKindDisposition::Ignore => RawKindDisposition::Ignore as i16,
        ProviderRawKindDisposition::Unsupported => RawKindDisposition::Unsupported as i16,
    }
}

fn governed_code(code: i32, label: &str) -> Result<u16, FactIngestError> {
    u16::try_from(code).map_err(|_| FactIngestError::Protocol(format!("{label} is outside code16")))
}

fn source_text(source: &SourceImage, start: u64, end: u64) -> Option<String> {
    let start = usize::try_from(start).ok()?;
    let end = usize::try_from(end).ok()?;
    std::str::from_utf8(source.bytes.get(start..end)?)
        .ok()
        .map(str::to_owned)
}

fn validate_span(
    source: &SourceImage,
    start: u64,
    end: u64,
    provider: &str,
) -> Result<(), FactIngestError> {
    if start > end || end > source.byte_length {
        return Err(FactIngestError::Protocol(format!(
            "{provider} emitted an invalid source span"
        )));
    }
    let boundaries = &source
        .provider_text
        .as_ref()
        .ok_or_else(|| FactIngestError::Protocol("provider source text is absent".into()))?
        .original_byte_offsets;
    if boundaries.binary_search(&start).is_err() || boundaries.binary_search(&end).is_err() {
        return Err(FactIngestError::Protocol(format!(
            "{provider} emitted a non-boundary source span"
        )));
    }
    Ok(())
}

fn to_i64(value: u64, name: &str) -> Result<i64, FactIngestError> {
    i64::try_from(value).map_err(|_| FactIngestError::Protocol(format!("{name} overflow")))
}

fn to_i32(value: u64, name: &str) -> Result<i32, FactIngestError> {
    i32::try_from(value).map_err(|_| FactIngestError::Protocol(format!("{name} overflow")))
}

fn batch_error(table_code: i16, check: &'static str, detail: &str) -> FactIngestError {
    let table = crate::schema_registry::table_spec(table_code)
        .map_or_else(|| table_code.to_string(), |spec| spec.name.into());
    FactIngestError::BatchInvalid {
        table,
        check,
        detail: detail.into(),
    }
}

fn ensure_row_budgets(counts: &[usize]) -> Result<(), FactIngestError> {
    if counts.iter().any(|count| *count > 65_536) {
        return Err(FactIngestError::Protocol(
            "source/syntax owner batch exceeds 65,536 rows".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::ipc::writer::StreamWriter;
    use arrow_array::{Array as _, BinaryArray, Int32Array};
    use serde::Deserialize;

    use super::*;
    use crate::fact_ingest::{CanonicalReconciliationEngine, validate_fact_batch};
    use crate::identity::{CaseSensitivityMode, WorkspacePath};
    use crate::provider_types::ProviderText;
    use crate::ruff_adapter::{NeverRuffCancelled, RuffAdapter};
    use crate::secure_path::StableFileMetadata;
    use crate::source_image::{BlobReference, LineIndex, SourceBlobLease, SourceFileKind};
    use crate::tree_sitter_adapter::{
        NeverCancelled, NormalizedSyntaxKind, TreeSitterAdapter, TreeSitterLanguage,
    };

    fn provider_text(text: &str) -> ProviderText {
        ProviderText {
            text: Arc::from(text),
            original_byte_offsets: Arc::from(
                text.char_indices()
                    .map(|(offset, _)| u64::try_from(offset).unwrap())
                    .chain(std::iter::once(u64::try_from(text.len()).unwrap()))
                    .collect::<Vec<_>>(),
            ),
        }
    }

    fn source_image_for(text: &str, language: SourceLanguage, file_name: &[u8]) -> SourceImage {
        let workspace_id = [1; 16];
        let path = WorkspacePath::from_components(
            workspace_id,
            PlatformCode::Unix,
            CaseSensitivityMode::Sensitive,
            &[b"pkg".to_vec(), file_name.to_vec()],
        )
        .unwrap();
        let bytes = text.as_bytes().to_vec();
        let digest = *blake3::hash(&bytes).as_bytes();
        let mut offsets = vec![0_u64];
        offsets.extend(
            bytes
                .iter()
                .enumerate()
                .filter(|(_, byte)| **byte == b'\n')
                .map(|(index, _)| u64::try_from(index + 1).unwrap()),
        );
        let serialized = offsets
            .iter()
            .flat_map(|offset| offset.to_le_bytes())
            .collect::<Vec<_>>();
        SourceImage {
            workspace_id,
            worktree_id: None,
            source_generation: 7,
            file_id: source_file_identity(&path).unwrap().id,
            path,
            language,
            bytes: Arc::from(bytes.clone()),
            digest,
            byte_length: u64::try_from(bytes.len()).unwrap(),
            file_kind: SourceFileKind::Regular,
            blob: BlobReference {
                digest,
                relative_name: "fixture".into(),
                byte_length: u64::try_from(bytes.len()).unwrap(),
            },
            lease: SourceBlobLease {
                lease_id: [6; 16],
                blob_digest: digest,
                expires_at: u64::MAX,
            },
            encoding: SourceEncoding::Utf8,
            provider_text: Some(provider_text(text)),
            line_index: LineIndex {
                offsets: offsets.into(),
                serialized: Arc::from(serialized.clone()),
                digest: *blake3::hash(&serialized).as_bytes(),
                format_version: 1,
                newline_kind: if text.contains('\n') {
                    NewlineKind::Lf
                } else {
                    NewlineKind::None
                },
            },
            metadata: StableFileMetadata {
                device: 1,
                inode: 2,
                size: u64::try_from(bytes.len()).unwrap(),
                mode: 0o100_600,
                modified_seconds: 0,
                modified_nanoseconds: 0,
                changed_seconds: 0,
                changed_nanoseconds: 0,
            },
        }
    }

    fn source_image(text: &str) -> SourceImage {
        source_image_for(text, SourceLanguage::Python, b"sample.py")
    }

    fn scope(source: &SourceImage) -> FactScope {
        FactScope {
            workspace_id: source.workspace_id,
            analysis_context_id: SOURCE_CONTEXT_ID,
            source_generation: i64::try_from(source.source_generation).unwrap(),
            owner_id: [9; 16],
        }
    }

    fn snapshots(source: &SourceImage) -> (TreeSitterSnapshot, RuffSnapshot) {
        let text = source.provider_text.clone().unwrap();
        let mut tree = TreeSitterAdapter::new(TreeSitterLanguage::Python).unwrap();
        let tree = tree.parse_full(1, text.clone(), &NeverCancelled).unwrap();
        let mut ruff = RuffAdapter::new().unwrap();
        let ruff = ruff.parse(1, text, &tree, &NeverRuffCancelled).unwrap();
        (tree, ruff)
    }

    fn output(
        source: &SourceImage,
        tree: &TreeSitterSnapshot,
        ruff: &RuffSnapshot,
    ) -> CanonicalIngestOutput {
        CanonicalReconciliationEngine::default()
            .ingest_source_syntax(
                scope(source),
                source,
                tree,
                Some(ruff),
                SourceSyntaxProviderRuns {
                    tree_sitter: [10; 16],
                    ruff_python: Some([20; 16]),
                },
            )
            .unwrap()
    }

    fn batch_digest(batch: &arrow_array::RecordBatch) -> String {
        let mut encoded = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut encoded, &batch.schema()).unwrap();
            writer.write(batch).unwrap();
            writer.finish().unwrap();
        }
        format!("b3:{}", blake3::hash(&encoded).to_hex())
    }

    #[derive(Debug, Deserialize)]
    struct SourceSyntaxFixture {
        arrow_version: String,
        source: String,
        required_reconciliation_steps: Vec<String>,
    }

    fn canonical_fixture() -> SourceSyntaxFixture {
        serde_json::from_str(include_str!(
            "../contracts/fixtures/synthetic/source-syntax-canonicalization-v1.json"
        ))
        .unwrap()
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One acceptance case proves all four tables and all five ladder steps.
    fn wp32_behavioral_acceptance() {
        let fixture = canonical_fixture();
        assert_eq!(fixture.arrow_version, arrow::ARROW_VERSION);
        let source = source_image(&fixture.source);
        let (tree, ruff) = snapshots(&source);
        let actual_output = output(&source, &tree, &ruff);
        let replay = output(&source, &tree, &ruff);
        assert_eq!(
            actual_output.batches.keys().copied().collect::<Vec<_>>(),
            [8, 9, 100, 110, 120, 130, 140, 150, 160, 170]
        );
        for (table_code, actual) in &actual_output.batches {
            let replayed = &replay.batches[table_code];
            assert_eq!(actual.num_rows(), replayed.num_rows(), "table {table_code}");
            assert_eq!(
                batch_digest(actual.batch()),
                batch_digest(replayed.batch()),
                "table {table_code}"
            );
        }
        assert_eq!(actual_output.batches[&140].num_rows(), 1);
        assert_eq!(actual_output.batches[&150].num_rows(), ruff.tokens.len());
        assert!(
            actual_output.batches[&160].num_rows() >= ruff.comments.len() + ruff.docstrings.len()
        );
        assert!(actual_output.batches[&170].num_rows() >= tree.facts.len());

        let relation_spec = crate::schema_registry::table_spec(110).unwrap();
        let kinds = actual_output.batches[&110]
            .batch()
            .column(
                relation_spec
                    .arrow_schema
                    .index_of("relation_kind_code")
                    .unwrap(),
            )
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();
        for required in [
            "CONTAINS_SPAN",
            "TOKEN_OF",
            "LEXICALLY_PRECEDES",
            "DOCUMENTS",
            "DIRECTIVE_APPLIES_TO",
            "AST_CHILD",
        ] {
            let code = relation_kind(required).unwrap().code;
            assert!(
                kinds.iter().flatten().any(|actual| actual == code),
                "missing {required}"
            );
        }

        let candidates = [
            RangeAnchor {
                entity_id: [1; 16],
                start_byte: 10,
                end_byte: 20,
                normalized_kind_code: SyntaxKind::Expression as u16,
                declaration_name_span: None,
            },
            RangeAnchor {
                entity_id: [2; 16],
                start_byte: 30,
                end_byte: 60,
                normalized_kind_code: SyntaxKind::DeclarationSyntax as u16,
                declaration_name_span: Some((34, 38)),
            },
            RangeAnchor {
                entity_id: [3; 16],
                start_byte: 70,
                end_byte: 100,
                normalized_kind_code: SyntaxKind::SyntaxNode as u16,
                declaration_name_span: None,
            },
            RangeAnchor {
                entity_id: [4; 16],
                start_byte: 110,
                end_byte: 115,
                normalized_kind_code: SyntaxKind::SyntaxNode as u16,
                declaration_name_span: None,
            },
        ];
        let cases = [
            (
                RangeObservation {
                    start_byte: 10,
                    end_byte: 20,
                    normalized_kind_code: SyntaxKind::Expression as u16,
                    declaration_name_span: None,
                },
                ReconciliationStep::ExactRangeAndKind,
            ),
            (
                RangeObservation {
                    start_byte: 31,
                    end_byte: 59,
                    normalized_kind_code: SyntaxKind::DeclarationSyntax as u16,
                    declaration_name_span: Some((34, 38)),
                },
                ReconciliationStep::ExactDeclarationName,
            ),
            (
                RangeObservation {
                    start_byte: 75,
                    end_byte: 80,
                    normalized_kind_code: SyntaxKind::Expression as u16,
                    declaration_name_span: None,
                },
                ReconciliationStep::SmallestEnclosingCompatible,
            ),
            (
                RangeObservation {
                    start_byte: 110,
                    end_byte: 120,
                    normalized_kind_code: SyntaxKind::Expression as u16,
                    declaration_name_span: None,
                },
                ReconciliationStep::SameStartCompatible,
            ),
            (
                RangeObservation {
                    start_byte: 200,
                    end_byte: 210,
                    normalized_kind_code: SyntaxKind::Expression as u16,
                    declaration_name_span: None,
                },
                ReconciliationStep::ProviderOnlySynthetic,
            ),
        ];
        for (observation, expected) in cases {
            let reconciled = reconcile_range(observation, &candidates);
            assert_eq!(reconciled.step, expected);
            assert!(!reconciled.ambiguous);
        }
        let ambiguous = reconcile_range(
            cases[0].0,
            &[
                candidates[0],
                RangeAnchor {
                    entity_id: [9; 16],
                    ..candidates[0]
                },
            ],
        );
        assert_eq!(ambiguous.step, ReconciliationStep::ExactRangeAndKind);
        assert!(ambiguous.ambiguous);
        let declaration = RangeObservation {
            start_byte: 31,
            end_byte: 59,
            normalized_kind_code: SyntaxKind::DeclarationSyntax as u16,
            declaration_name_span: Some((34, 38)),
        };
        let correct = RangeAnchor {
            entity_id: [9; 16],
            start_byte: 30,
            end_byte: 60,
            normalized_kind_code: SyntaxKind::DeclarationSyntax as u16,
            declaration_name_span: Some((34, 38)),
        };
        let invalid_candidates = [
            RangeAnchor {
                entity_id: [5; 16],
                start_byte: 33,
                end_byte: 39,
                normalized_kind_code: SyntaxKind::Expression as u16,
                declaration_name_span: Some((34, 38)),
            },
            RangeAnchor {
                entity_id: [6; 16],
                start_byte: 33,
                end_byte: 39,
                normalized_kind_code: SyntaxKind::DeclarationSyntax as u16,
                declaration_name_span: Some((35, 38)),
            },
            RangeAnchor {
                entity_id: [7; 16],
                start_byte: 35,
                end_byte: 39,
                normalized_kind_code: SyntaxKind::DeclarationSyntax as u16,
                declaration_name_span: Some((34, 38)),
            },
            RangeAnchor {
                entity_id: [8; 16],
                start_byte: 33,
                end_byte: 37,
                normalized_kind_code: SyntaxKind::DeclarationSyntax as u16,
                declaration_name_span: Some((34, 38)),
            },
        ];
        for invalid in invalid_candidates {
            let reconciled = reconcile_range(declaration, &[correct, invalid]);
            assert_eq!(reconciled.entity_id, Some(correct.entity_id));
            assert_eq!(reconciled.step, ReconciliationStep::ExactDeclarationName);
        }
        assert_eq!(ReconciliationStep::ExactRangeAndKind as u16, 10);
        assert_eq!(ReconciliationStep::ExactDeclarationName as u16, 20);
        assert_eq!(ReconciliationStep::SmallestEnclosingCompatible as u16, 30);
        assert_eq!(ReconciliationStep::SameStartCompatible as u16, 40);
        assert_eq!(ReconciliationStep::ProviderOnlySynthetic as u16, 50);
        assert!(valid_span(0, 0, 1));
        assert!(valid_span(0, 1, 1));
        assert!(!valid_span(-1, 0, 1));
        assert!(!valid_span(1, 0, 1));
        assert!(!valid_span(0, 2, 1));
        assert_eq!(
            fixture.required_reconciliation_steps,
            [
                "EXACT_RANGE_AND_KIND",
                "EXACT_DECLARATION_NAME",
                "SMALLEST_ENCLOSING_COMPATIBLE",
                "SAME_START_COMPATIBLE",
                "PROVIDER_ONLY_SYNTHETIC",
            ]
        );
    }

    #[test]
    fn wp32_structural_acceptance() {
        let source = source_image("def f(value):\n    return (value + 1)\n");
        let (tree, ruff) = snapshots(&source);
        let first = output(&source, &tree, &ruff);
        let second = output(&source, &tree, &ruff);
        for code in first.batches.keys() {
            let left = first.batches[code]
                .batch()
                .columns()
                .iter()
                .map(arrow_array::Array::to_data)
                .collect::<Vec<_>>();
            let right = second.batches[code]
                .batch()
                .columns()
                .iter()
                .map(arrow_array::Array::to_data)
                .collect::<Vec<_>>();
            assert_eq!(left, right, "table {code} is nondeterministic");
        }
        let entity_spec = crate::schema_registry::table_spec(100).unwrap();
        let entity_ids = first.batches[&100]
            .batch()
            .column(entity_spec.arrow_schema.index_of("entity_id").unwrap())
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap()
            .iter()
            .flatten()
            .map(<[u8; 16]>::try_from)
            .collect::<Result<BTreeSet<_>, _>>()
            .unwrap();
        let relation_spec = crate::schema_registry::table_spec(110).unwrap();
        for name in ["source_id", "target_id"] {
            let endpoints = first.batches[&110]
                .batch()
                .column(relation_spec.arrow_schema.index_of(name).unwrap())
                .as_any()
                .downcast_ref::<BinaryArray>()
                .unwrap();
            assert!(endpoints.iter().flatten().all(|id| entity_ids.contains(id)));
        }

        let mut facts = tree.facts.to_vec();
        let template = facts[0].clone();
        for (id, state) in [
            (100_u64, (false, false, false, false)),
            (101, (true, true, false, false)),
            (102, (true, false, true, false)),
            (103, (true, false, false, true)),
        ] {
            let mut fact = template.clone();
            fact.id = SyntaxOccurrenceId(id);
            (fact.named, fact.extra, fact.error, fact.missing) = state;
            facts.push(fact);
        }
        let anchor_ids = facts
            .iter()
            .map(|fact| (fact.id, [u8::try_from(fact.id.0).unwrap_or(255); 16]))
            .collect::<BTreeMap<_, _>>();
        let anchors = reconciliation_anchors(&facts, &anchor_ids, &BTreeMap::new());
        assert!(anchors.iter().all(|anchor| {
            let fact = facts
                .iter()
                .find(|fact| anchor_ids[&fact.id] == anchor.entity_id)
                .unwrap();
            fact.named && !fact.extra && !fact.error && !fact.missing
        }));
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One zero-state matrix isolates every governed ingress predicate.
    fn wp32_negative_zero_state() {
        let source = source_image("value = 1\n");
        let (mut tree, mut ruff) = snapshots(&source);
        let mut tree_facts = tree.facts.to_vec();
        for fact in &mut tree_facts {
            fact.normalized_kind = NormalizedSyntaxKind(SyntaxKind::Return as u16);
        }
        tree.facts = tree_facts.into();
        let mut ast = ruff.ast.to_vec();
        ast[3].category = RuffAstCategory::Assignment;
        ruff.ast = ast.into();
        let conflict_output = output(&source, &tree, &ruff);
        assert!(!conflict_output.conflicts.is_empty());
        let selected = observation_id([20; 16], 3, 3);
        let exact_conflict = conflict_output
            .conflicts
            .iter()
            .find(|conflict| conflict.selected_observation_id == selected)
            .unwrap();
        assert_eq!(
            exact_conflict.rejected_observation_id,
            observation_id([10; 16], 2, 6)
        );
        assert!(
            conflict_output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "SOURCE_RANGE_RECONCILIATION_CONFLICT")
        );
        assert!(
            conflict_output.batches[&130].num_rows() > conflict_output.batches[&170].num_rows()
        );

        let mut wrong_scope = scope(&source);
        wrong_scope.analysis_context_id = [3; 16];
        assert!(matches!(
            CanonicalReconciliationEngine::default().ingest_source_syntax(
                wrong_scope,
                &source,
                &tree,
                Some(&ruff),
                SourceSyntaxProviderRuns {
                    tree_sitter: [10; 16],
                    ruff_python: Some([20; 16])
                },
            ),
            Err(FactIngestError::SourceSnapshotMismatch(_))
        ));

        let admission_source = source_image("value = 1\n");
        let (admission_tree, admission_ruff) = snapshots(&admission_source);
        let valid_runs = SourceSyntaxProviderRuns {
            tree_sitter: [10; 16],
            ruff_python: Some([20; 16]),
        };
        let mut wrong_workspace_scope = scope(&admission_source);
        wrong_workspace_scope.workspace_id = [2; 16];
        assert!(
            validate_inputs(
                wrong_workspace_scope,
                &admission_source,
                &admission_tree,
                Some(&admission_ruff),
                valid_runs,
            )
            .is_err()
        );

        let unicode_source = source_image("é = 1\n");
        let (mut non_boundary_tree, unicode_ruff) = snapshots(&unicode_source);
        let mut non_boundary_facts = non_boundary_tree.facts.to_vec();
        non_boundary_facts[0].start_byte = 1;
        non_boundary_tree.facts = non_boundary_facts.into();
        assert!(matches!(
            CanonicalReconciliationEngine::default().ingest_source_syntax(
                scope(&unicode_source),
                &unicode_source,
                &non_boundary_tree,
                Some(&unicode_ruff),
                valid_runs,
            ),
            Err(FactIngestError::Protocol(detail)) if detail.contains("non-boundary")
        ));

        let mut overlapping_ruff = admission_ruff.clone();
        let mut overlapping_tokens = overlapping_ruff.tokens.to_vec();
        let nonempty = overlapping_tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| token.start_byte < token.end_byte)
            .map(|(index, _)| index)
            .take(2)
            .collect::<Vec<_>>();
        overlapping_tokens[nonempty[1]].start_byte =
            overlapping_tokens[nonempty[0]].end_byte.saturating_sub(1);
        overlapping_ruff.tokens = overlapping_tokens.into();
        assert!(matches!(
            CanonicalReconciliationEngine::default().ingest_source_syntax(
                scope(&admission_source),
                &admission_source,
                &admission_tree,
                Some(&overlapping_ruff),
                valid_runs,
            ),
            Err(FactIngestError::Protocol(detail)) if detail.contains("overlapping token")
        ));

        let mut ambiguous_tree = admission_tree.clone();
        let mut ambiguous_facts = ambiguous_tree.facts.to_vec();
        let ruff_anchor = &admission_ruff.ast[0];
        let mut first = ambiguous_facts[0].clone();
        first.id = SyntaxOccurrenceId(10_000);
        first.start_byte = ruff_anchor.start_byte;
        first.end_byte = ruff_anchor.end_byte;
        first.normalized_kind = NormalizedSyntaxKind(ruff_anchor.category.registry_code());
        first.named = true;
        first.extra = false;
        first.error = false;
        first.missing = false;
        first.parent = None;
        first.ordinal = 10_000;
        let mut second = first.clone();
        second.id = SyntaxOccurrenceId(10_001);
        second.ordinal = 10_001;
        ambiguous_facts.extend([first, second]);
        ambiguous_tree.facts = ambiguous_facts.into();
        let mut ambiguous_ruff = admission_ruff.clone();
        ambiguous_ruff.correspondences = Arc::from([]);
        assert!(matches!(
            CanonicalReconciliationEngine::default().ingest_source_syntax(
                scope(&admission_source),
                &admission_source,
                &ambiguous_tree,
                Some(&ambiguous_ruff),
                valid_runs,
            ),
            Err(FactIngestError::Protocol(detail)) if detail.contains("ambiguous")
        ));
        let mut wrong_path_workspace = admission_source.clone();
        wrong_path_workspace.path.workspace_id = [2; 16];
        wrong_path_workspace.file_id = source_file_identity(&wrong_path_workspace.path).unwrap().id;
        assert!(
            validate_inputs(
                scope(&admission_source),
                &wrong_path_workspace,
                &admission_tree,
                Some(&admission_ruff),
                valid_runs,
            )
            .is_err()
        );
        let mut invalid_images = Vec::new();
        let mut wrong_file_id = admission_source.clone();
        wrong_file_id.file_id = [0; 16];
        invalid_images.push(wrong_file_id);
        let mut wrong_digest = admission_source.clone();
        wrong_digest.digest = [0; 32];
        invalid_images.push(wrong_digest);
        let mut wrong_length = admission_source.clone();
        wrong_length.byte_length += 1;
        invalid_images.push(wrong_length);
        for invalid in invalid_images {
            assert!(
                validate_inputs(
                    scope(&admission_source),
                    &invalid,
                    &admission_tree,
                    Some(&admission_ruff),
                    valid_runs,
                )
                .is_err()
            );
        }
        let mut unclassified = admission_source.clone();
        unclassified.language = SourceLanguage::Other;
        assert!(
            validate_inputs(
                scope(&admission_source),
                &unclassified,
                &admission_tree,
                None,
                SourceSyntaxProviderRuns {
                    tree_sitter: [10; 16],
                    ruff_python: None,
                },
            )
            .is_err()
        );
        let mut wrong_python_catalog = admission_tree.clone();
        wrong_python_catalog.catalog_id = "tree-sitter-rust:test";
        assert!(
            validate_inputs(
                scope(&admission_source),
                &admission_source,
                &wrong_python_catalog,
                Some(&admission_ruff),
                valid_runs,
            )
            .is_err()
        );
        let mut drifted_ruff = admission_ruff.clone();
        drifted_ruff.source.provider_image_fingerprint.push('x');
        assert!(
            validate_inputs(
                scope(&admission_source),
                &admission_source,
                &admission_tree,
                Some(&drifted_ruff),
                SourceSyntaxProviderRuns {
                    tree_sitter: [10; 16],
                    ruff_python: Some([20; 16]),
                },
            )
            .is_err()
        );
        assert!(
            validate_inputs(
                scope(&admission_source),
                &admission_source,
                &admission_tree,
                Some(&admission_ruff),
                SourceSyntaxProviderRuns {
                    tree_sitter: [10; 16],
                    ruff_python: None,
                },
            )
            .is_err()
        );

        let recovery_source = source_image("def broken(:\n    pass\n");
        let (recovery_tree, recovery_ruff) = snapshots(&recovery_source);
        let recovery = output(&recovery_source, &recovery_tree, &recovery_ruff);
        let annotation_spec = crate::schema_registry::table_spec(160).unwrap();
        let annotation_kinds = recovery.batches[&160]
            .batch()
            .column(
                annotation_spec
                    .arrow_schema
                    .index_of("annotation_kind_code")
                    .unwrap(),
            )
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();
        assert!(annotation_kinds.iter().flatten().any(|code| {
            code == AnnotationKind::ParseError as i32
                || code == AnnotationKind::MissingSyntax as i32
        }));

        let rust_source = source_image_for(
            "fn main() { let value = 1; }\n",
            SourceLanguage::Rust,
            b"sample.rs",
        );
        let mut rust_adapter = TreeSitterAdapter::new(TreeSitterLanguage::Rust).unwrap();
        let rust_tree = rust_adapter
            .parse_full(
                1,
                rust_source.provider_text.clone().unwrap(),
                &NeverCancelled,
            )
            .unwrap();
        let mut wrong_rust_catalog = rust_tree.clone();
        wrong_rust_catalog.catalog_id = "tree-sitter-python:test";
        assert!(
            validate_inputs(
                scope(&rust_source),
                &rust_source,
                &wrong_rust_catalog,
                None,
                SourceSyntaxProviderRuns {
                    tree_sitter: [30; 16],
                    ruff_python: None,
                },
            )
            .is_err()
        );
        let rust_output = CanonicalReconciliationEngine::default()
            .ingest_source_syntax(
                scope(&rust_source),
                &rust_source,
                &rust_tree,
                None,
                SourceSyntaxProviderRuns {
                    tree_sitter: [30; 16],
                    ruff_python: None,
                },
            )
            .unwrap();
        assert!(rust_output.batches[&170].num_rows() > 0);

        let rust_recovery_source = source_image_for(
            "fn broken( { let value = ;\n",
            SourceLanguage::Rust,
            b"broken.rs",
        );
        let mut rust_recovery_adapter = TreeSitterAdapter::new(TreeSitterLanguage::Rust).unwrap();
        let rust_recovery_tree = rust_recovery_adapter
            .parse_full(
                1,
                rust_recovery_source.provider_text.clone().unwrap(),
                &NeverCancelled,
            )
            .unwrap();
        let rust_recovery = CanonicalReconciliationEngine::default()
            .ingest_source_syntax(
                scope(&rust_recovery_source),
                &rust_recovery_source,
                &rust_recovery_tree,
                None,
                SourceSyntaxProviderRuns {
                    tree_sitter: [31; 16],
                    ruff_python: None,
                },
            )
            .unwrap();
        let rust_annotation_kinds = rust_recovery.batches[&160]
            .batch()
            .column(
                annotation_spec
                    .arrow_schema
                    .index_of("annotation_kind_code")
                    .unwrap(),
            )
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();
        assert!(rust_annotation_kinds.iter().flatten().any(|code| {
            code == AnnotationKind::ParseError as i32
                || code == AnnotationKind::MissingSyntax as i32
        }));

        let endpoint_entity = EntityRow {
            scope: scope(&admission_source),
            entity_id: [41; 16],
            language: Language::Python as i16,
            entity_family_code: 10,
            entity_kind_code: 10,
            raw_kind_code: None,
            file_id: Some(admission_source.file_id),
            start_byte: Some(0),
            end_byte: Some(1),
            name: None,
            qualified_name: None,
            parent_entity_id: None,
            type_id: None,
            flags: 0,
            fact_hash64: 0,
        };
        let missing_relation_endpoint = RelationRow {
            scope: scope(&admission_source),
            fact_id: [42; 16],
            language: Language::Python as i16,
            relation_family_code: 10,
            relation_kind_code: 10,
            source_id: [43; 16],
            target_id: endpoint_entity.entity_id,
            ordinal: None,
            role_code: None,
            distance: None,
            directness_code: 10,
            file_id: Some(admission_source.file_id),
            start_byte: Some(0),
            end_byte: Some(1),
            certainty_code: 10,
            resolution_code: 10,
            producer_code: 10,
            derivation_code: None,
            flags: 0,
            fact_hash64: 0,
        };
        assert!(
            validate_cross_table(
                &admission_source,
                std::slice::from_ref(&endpoint_entity),
                &[missing_relation_endpoint],
                &[],
                &[],
                &[],
            )
            .is_err()
        );
        let missing_syntax_entity = SyntaxDetailRow {
            scope: scope(&admission_source),
            entity_id: [44; 16],
            raw_kind_code: 10,
            occurrence_family_code: OccurrenceFamily::Syntax as i16,
            reconciliation_step_code: ReconciliationStep::ExactRangeAndKind as i16,
            raw_kind_disposition_code: RawKindDisposition::Normalize as i16,
            normalized_kind_code: SyntaxKind::SyntaxNode as i32,
            parent_syntax_id: None,
            field_role_code: None,
            ordinal: None,
            named: true,
            extra: false,
            error: false,
            missing: false,
            explicitly_parenthesized: false,
            provider_node_flags: governed_provider_node_flags(),
        };
        assert!(
            validate_cross_table(
                &admission_source,
                std::slice::from_ref(&endpoint_entity),
                &[],
                &[],
                &[],
                &[missing_syntax_entity],
            )
            .is_err()
        );
        let missing_annotation_target = SourceAnnotationRow {
            scope: scope(&admission_source),
            annotation_id: endpoint_entity.entity_id,
            file_id: admission_source.file_id,
            annotation_kind_code: AnnotationKind::Comment as i32,
            start_byte: 0,
            end_byte: 1,
            target_entity_id: Some([45; 16]),
            text: None,
            diagnostic_code: None,
            flags: 0,
        };
        assert!(
            validate_cross_table(
                &admission_source,
                &[endpoint_entity],
                &[],
                &[],
                &[missing_annotation_target],
                &[],
            )
            .is_err()
        );

        let valid_row = source_file_row(scope(&admission_source), &admission_source).unwrap();
        let mut invalid_rows = Vec::new();
        let mut wrong_batch_length = valid_row.clone();
        wrong_batch_length.byte_len += 1;
        invalid_rows.push(wrong_batch_length);
        let mut wrong_batch_digest = valid_row.clone();
        wrong_batch_digest.source_digest = [0; 32];
        invalid_rows.push(wrong_batch_digest);
        let mut empty = valid_row.clone();
        empty.line_start_offsets.clear();
        empty.line_count = 0;
        invalid_rows.push(empty);
        let mut nonzero_start = valid_row.clone();
        nonzero_start.line_start_offsets = vec![1];
        nonzero_start.line_count = 1;
        invalid_rows.push(nonzero_start);
        let mut out_of_bounds = valid_row.clone();
        out_of_bounds.line_start_offsets = vec![0, out_of_bounds.byte_len + 1];
        out_of_bounds.line_count = 2;
        invalid_rows.push(out_of_bounds);
        let mut not_increasing = valid_row.clone();
        not_increasing.line_start_offsets = vec![0, 0];
        not_increasing.line_count = 2;
        invalid_rows.push(not_increasing);
        let mut wrong_count = valid_row;
        wrong_count.line_count += 1;
        invalid_rows.push(wrong_count);
        for row in invalid_rows {
            let batch = encode_source_files(&[row]).unwrap();
            assert!(validate_fact_batch(&batch, 140, scope(&admission_source)).is_err());
        }
    }

    #[test]
    fn wp32_operational_acceptance() {
        let source = source_image("x = 1\ny = x\n");
        let (tree, ruff) = snapshots(&source);
        let ingress = CanonicalReconciliationEngine::default();
        let output = ingress
            .ingest_source_syntax(
                scope(&source),
                &source,
                &tree,
                Some(&ruff),
                SourceSyntaxProviderRuns {
                    tree_sitter: [10; 16],
                    ruff_python: Some([20; 16]),
                },
            )
            .unwrap();
        assert_eq!(ingress.metrics(), output.metrics);
        assert_eq!(output.metrics.streams_received, 3);
        assert!(output.metrics.rows_received > 0);
        assert!(output.metrics.rows_encoded > output.metrics.rows_received);
        assert!(ensure_row_budgets(&[65_536]).is_ok());
        assert!(ensure_row_budgets(&[65_537]).is_err());
        for (code, batch) in &output.batches {
            validate_fact_batch(batch.batch(), *code, scope(&source)).unwrap();
            assert!(batch.num_rows() <= 65_536);
        }
    }
}
