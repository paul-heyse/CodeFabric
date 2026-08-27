//! Canonical Arrow projection for application-owned Ruff semantic observations.
//!
//! Ruff-owned arenas and indices have already been removed by [`crate::ruff_adapter`].
//! This boundary re-keys adapter-local identifiers into one owner-scoped canonical
//! namespace, materializes explicit unknowns, and validates every generated batch
//! before it can reach publication.

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow_array::{ArrayRef, BinaryArray, RecordBatch, StringArray};
use serde_json::{Value, json};

use crate::core_facts::{
    registered_provider_observation_arrow_schema, validate_provider_semantic_observation,
};
use crate::fact_ingest::{
    BindingDetailRow, CanonicalIngestOutput, CanonicalReconciliationEngine, CapabilityStatusRow,
    EntityRow, FactEvidenceRow, FactIngestError, FactScope, OwnerRow, ProviderFactBatch,
    ProviderFactManifest, ProviderFactStream, ReferenceDetailRow, RelationRow, ScopeDetailRow,
    StreamTerminal, ValidatedFactBatch, encode_binding_details, encode_capability_statuses,
    encode_entities, encode_evidence, encode_owners, encode_reference_details, encode_relations,
    encode_scope_details,
};
use crate::registries::{
    CompletenessState, Directness, EvidenceCertainty, Language, OwnerCapabilityState, OwnerKind,
    ProviderCode, ResolutionClass, capability_code, capability_mask, entity_kind, fact_kind_code,
    relation_kind,
};
use crate::ruff_adapter::{
    PythonBindingKind, PythonFrontendBatch, PythonReferenceClass, PythonResolution,
    PythonScopeKind, PythonSemanticEdgeKind, PythonTargetForm,
};

const OBSERVATION_SCHEMA_ID: &str = "codefabric.ruff.semantic.v1";
const PROVIDER_VERSION: &str = "0.0.7";

#[derive(Clone, Copy)]
struct DerivedIdentity {
    id: [u8; 16],
    digest: [u8; 32],
}

/// Fully validated owner-scoped result of one Ruff semantic observation.
#[derive(Debug)]
pub struct PythonSemanticProjection {
    pub provider_run_id: [u8; 16],
    pub observation: RecordBatch,
    pub canonical: CanonicalIngestOutput,
}

impl PythonSemanticProjection {
    /// Read one validated generated table by stable table code.
    #[must_use]
    pub fn batch(&self, table_code: i16) -> Option<&ValidatedFactBatch> {
        self.canonical.batches.get(&table_code)
    }
}

/// Project a complete application-owned Ruff batch into canonical facts.
///
/// The caller supplies the authoritative module owner and source-file identity. Adapter-local
/// IDs are re-keyed with that owner, preventing identical files from colliding while retaining
/// deterministic identity for unchanged owners.
///
/// # Errors
///
/// Returns a protocol or generated batch-validation error if the observation is incomplete,
/// references an absent target, overflows canonical coordinates, or diverges from any generated
/// schema/registry contract.
#[allow(clippy::too_many_lines)] // The projection deliberately keeps the full evidence closure visible.
pub fn project_ruff_semantic_batch(
    scope: FactScope,
    file_id: [u8; 16],
    batch: &PythonFrontendBatch,
) -> Result<PythonSemanticProjection, FactIngestError> {
    if batch.terminal.terminal_state != "completed" || batch.terminal.failure_code.is_some() {
        return Err(FactIngestError::Protocol(format!(
            "Ruff semantic observation is not complete: terminal_state={} failure_code={:?}",
            batch.terminal.terminal_state, batch.terminal.failure_code
        )));
    }
    validate_reference_edges(batch)?;

    let observation_payloads = observation_payloads(batch)?;
    let observation = observation_batch(batch, &observation_payloads)?;
    validate_provider_semantic_observation(OBSERVATION_SCHEMA_ID, &observation)?;

    let provider_run = derived_identity(
        b"provider-run",
        &[
            &scope.workspace_id,
            &scope.analysis_context_id,
            &scope.owner_id,
            &scope.source_generation.to_be_bytes(),
            batch.module_name.as_bytes(),
            batch.provider_image_fingerprint.as_bytes(),
        ],
    );

    let mut canonical_ids = BTreeMap::new();
    for semantic in batch
        .scopes
        .iter()
        .map(|fact| (b"scope".as_slice(), fact.scope_id))
        .chain(
            batch
                .bindings
                .iter()
                .map(|fact| (b"binding".as_slice(), fact.binding_id)),
        )
        .chain(
            batch
                .references
                .iter()
                .map(|fact| (b"reference".as_slice(), fact.reference_id)),
        )
        .chain(
            batch
                .unknown_symbols
                .iter()
                .map(|fact| (b"unknown-symbol".as_slice(), fact.unknown_symbol_id)),
        )
    {
        let identity = derived_identity(semantic.0, &[&scope.owner_id, &semantic.1]);
        if canonical_ids.insert(semantic.1, identity).is_some() {
            return Err(FactIngestError::Protocol(
                "Ruff semantic observation reused one ID across fact forms".into(),
            ));
        }
    }

    let scope_kind = required_entity_kind("SCOPE")?;
    let declaration_kind = required_entity_kind("DECLARATION")?;
    let reference_kind = required_entity_kind("REFERENCE")?;
    let unknown_kind = required_entity_kind("UNKNOWN")?;
    let contains_kind = required_relation_kind("CONTAINS")?;
    let declares_kind = required_relation_kind("DECLARES")?;
    let refers_to_kind = required_relation_kind("REFERS_TO")?;

    let mut entities = Vec::with_capacity(
        batch.scopes.len()
            + batch.bindings.len()
            + batch.references.len()
            + batch.unknown_symbols.len(),
    );
    let mut scope_details = Vec::with_capacity(batch.scopes.len());
    let mut binding_details = Vec::with_capacity(batch.bindings.len());
    let mut reference_details = Vec::with_capacity(batch.references.len());

    for fact in &batch.scopes {
        let identity = canonical_identity(&canonical_ids, fact.scope_id, "scope")?;
        let parent = fact
            .parent_scope_id
            .map(|id| canonical_identity(&canonical_ids, id, "parent scope").map(|item| item.id))
            .transpose()?;
        let start = coordinate(fact.start_byte)?;
        let end = coordinate(fact.end_byte)?;
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: scope_kind.family_code,
            entity_kind_code: scope_kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: Some(start),
            end_byte: Some(end),
            name: fact.name.clone(),
            qualified_name: fact.name.clone(),
            parent_entity_id: parent,
            type_id: None,
            flags: 0,
            fact_hash64: digest_hash64(identity.digest),
        });
        scope_details.push(ScopeDetailRow {
            scope,
            scope_id: identity.id,
            parent_scope_id: parent,
            scope_kind: scope_kind_name(fact.kind).into(),
            name: fact.name.clone(),
            start_byte: start,
            end_byte: end,
        });
    }

    for fact in &batch.bindings {
        let identity = canonical_identity(&canonical_ids, fact.binding_id, "binding")?;
        let owner_scope = canonical_identity(&canonical_ids, fact.scope_id, "binding scope")?.id;
        let start = coordinate(fact.start_byte)?;
        let end = coordinate(fact.end_byte)?;
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: declaration_kind.family_code,
            entity_kind_code: declaration_kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: Some(start),
            end_byte: Some(end),
            name: Some(fact.name.clone()),
            qualified_name: None,
            parent_entity_id: Some(owner_scope),
            type_id: None,
            flags: 0,
            fact_hash64: digest_hash64(identity.digest),
        });
        binding_details.push(BindingDetailRow {
            scope,
            binding_id: identity.id,
            scope_id: owner_scope,
            name: fact.name.clone(),
            binding_kind: binding_kind_name(fact.kind).into(),
            target_form: target_form_name(fact.target_form).into(),
            start_byte: start,
            end_byte: end,
        });
    }

    for fact in &batch.references {
        let identity = canonical_identity(&canonical_ids, fact.reference_id, "reference")?;
        let owner_scope = canonical_identity(&canonical_ids, fact.scope_id, "reference scope")?.id;
        let target = canonical_identity(&canonical_ids, fact.target_id, "reference target")?.id;
        let start = coordinate(fact.start_byte)?;
        let end = coordinate(fact.end_byte)?;
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: reference_kind.family_code,
            entity_kind_code: reference_kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: Some(start),
            end_byte: Some(end),
            name: Some(fact.name.clone()),
            qualified_name: None,
            parent_entity_id: Some(owner_scope),
            type_id: None,
            flags: 0,
            fact_hash64: digest_hash64(identity.digest),
        });
        reference_details.push(ReferenceDetailRow {
            scope,
            reference_id: identity.id,
            scope_id: owner_scope,
            target_id: target,
            name: fact.name.clone(),
            reference_class: reference_class_name(fact.class).into(),
            resolution: resolution_name(fact.resolution).into(),
            start_byte: start,
            end_byte: end,
            unknown_reason_code: fact.unknown_reason_code.clone(),
        });
    }

    for fact in &batch.unknown_symbols {
        let identity =
            canonical_identity(&canonical_ids, fact.unknown_symbol_id, "unknown symbol")?;
        let owner_scope =
            canonical_identity(&canonical_ids, fact.scope_id, "unknown-symbol scope")?.id;
        entities.push(EntityRow {
            scope,
            entity_id: identity.id,
            language: Language::Python as i16,
            entity_family_code: unknown_kind.family_code,
            entity_kind_code: unknown_kind.code,
            raw_kind_code: None,
            file_id: Some(file_id),
            start_byte: None,
            end_byte: None,
            name: Some(fact.name.clone()),
            qualified_name: None,
            parent_entity_id: Some(owner_scope),
            type_id: None,
            flags: 0,
            fact_hash64: digest_hash64(identity.digest),
        });
    }

    let mut relations = Vec::new();
    for fact in &batch.scopes {
        if let Some(parent) = fact.parent_scope_id {
            push_relation(
                &mut relations,
                scope,
                file_id,
                contains_kind,
                canonical_identity(&canonical_ids, parent, "parent scope")?.id,
                canonical_identity(&canonical_ids, fact.scope_id, "child scope")?.id,
                None,
                None,
                EvidenceCertainty::StaticSemantic as i16,
                ResolutionClass::StaticallyResolved as i16,
            );
        }
    }
    for fact in &batch.bindings {
        push_relation(
            &mut relations,
            scope,
            file_id,
            declares_kind,
            canonical_identity(&canonical_ids, fact.scope_id, "binding scope")?.id,
            canonical_identity(&canonical_ids, fact.binding_id, "binding")?.id,
            Some(coordinate(fact.start_byte)?),
            Some(coordinate(fact.end_byte)?),
            EvidenceCertainty::StaticSemantic as i16,
            ResolutionClass::StaticallyResolved as i16,
        );
    }
    for fact in &batch.references {
        let (certainty, resolution) = canonical_resolution(fact.resolution);
        push_relation(
            &mut relations,
            scope,
            file_id,
            refers_to_kind,
            canonical_identity(&canonical_ids, fact.reference_id, "reference")?.id,
            canonical_identity(&canonical_ids, fact.target_id, "reference target")?.id,
            Some(coordinate(fact.start_byte)?),
            Some(coordinate(fact.end_byte)?),
            certainty,
            resolution,
        );
    }

    entities.sort_by_key(|row| row.entity_id);
    relations.sort_by_key(|row| row.fact_id);
    scope_details.sort_by_key(|row| row.scope_id);
    binding_details.sort_by_key(|row| row.binding_id);
    reference_details.sort_by_key(|row| row.reference_id);

    let entity_form = required_fact_form("ENTITY_EXISTENCE")?;
    let relation_form = required_fact_form("RELATION")?;
    let mut evidence = Vec::with_capacity(entities.len() + relations.len());
    for entity in &entities {
        let unresolved = entity.entity_kind_code == unknown_kind.code;
        evidence.push(evidence_row(
            scope,
            provider_run.id,
            entity.entity_id,
            entity_form,
            entity.file_id,
            entity.start_byte,
            entity.end_byte,
            if unresolved {
                EvidenceCertainty::Unresolved as i16
            } else {
                EvidenceCertainty::StaticSemantic as i16
            },
            if unresolved {
                ResolutionClass::Unresolved as i16
            } else {
                ResolutionClass::NotApplicable as i16
            },
        ));
    }
    for relation in &relations {
        evidence.push(evidence_row(
            scope,
            provider_run.id,
            relation.fact_id,
            relation_form,
            relation.file_id,
            relation.start_byte,
            relation.end_byte,
            relation.certainty_code,
            relation.resolution_code,
        ));
    }
    evidence.sort_by_key(|row| row.evidence_id);

    let mut coverage_hasher = blake3::Hasher::new_derive_key("codefabric.ruff-coverage.v1");
    coverage_hasher.update(batch.provider_image_fingerprint.as_bytes());
    for payload in &observation_payloads {
        coverage_hasher.update(payload);
    }
    let coverage_scope_fingerprint = *coverage_hasher.finalize().as_bytes();
    let scopes_bindings = capability_code("SCOPES_BINDINGS")
        .and_then(|code| i16::try_from(code).ok())
        .ok_or_else(|| FactIngestError::Protocol("SCOPES_BINDINGS capability is absent".into()))?;
    let capability = CapabilityStatusRow {
        scope,
        snapshot_id: None,
        capability_code: scopes_bindings,
        owner_capability_state_code: OwnerCapabilityState::Current as i16,
        completeness_state_code: CompletenessState::Complete as i16,
        provider_run_id: Some(provider_run.id),
        producer_code: Some(ProviderCode::RuffPython as i16),
        reason_code: None,
        diagnostic_id: None,
        fallback_source_available: false,
        coverage_scope_fingerprint,
    };

    let module = batch
        .scopes
        .iter()
        .find(|fact| fact.kind == PythonScopeKind::Module)
        .ok_or_else(|| FactIngestError::Protocol("Ruff module scope is absent".into()))?;
    let module_identity = canonical_identity(&canonical_ids, module.scope_id, "module scope")?.id;
    let capability_bits = capability_mask(&["SCOPES_BINDINGS"])
        .and_then(|mask| i64::try_from(mask).ok())
        .ok_or_else(|| FactIngestError::Protocol("SCOPES_BINDINGS mask is absent".into()))?;
    let owner = OwnerRow {
        scope,
        parent_owner_id: None,
        owner_kind_code: OwnerKind::Module as i16,
        language: Language::Python as i16,
        file_id: Some(file_id),
        semantic_entity_id: Some(module_identity),
        start_byte: Some(coordinate(module.start_byte)?),
        end_byte: Some(coordinate(module.end_byte)?),
        source_fingerprint: Some(
            *blake3::hash(batch.provider_image_fingerprint.as_bytes()).as_bytes(),
        ),
        semantic_fingerprint: Some(coverage_scope_fingerprint),
        capability_mask: capability_bits,
    };

    let encoded = [
        (8, encode_owners(&[owner])?),
        (9, encode_capability_statuses(&[capability])?),
        (100, encode_entities(&entities)?),
        (110, encode_relations(&relations)?),
        (130, encode_evidence(&evidence)?),
        (200, encode_scope_details(&scope_details)?),
        (210, encode_binding_details(&binding_details)?),
        (220, encode_reference_details(&reference_details)?),
    ];
    let provider_batches = encoded
        .into_iter()
        .map(|(table_code, batch)| ProviderFactBatch { table_code, batch })
        .collect::<Vec<_>>();
    let declared_rows = provider_batches
        .iter()
        .map(|batch| batch.batch.num_rows())
        .sum();
    let schema_fingerprints = provider_batches
        .iter()
        .map(|batch| {
            crate::schema_registry::table_spec(batch.table_code)
                .map(|spec| (batch.table_code, spec.schema_digest.clone()))
                .ok_or_else(|| {
                    FactIngestError::Protocol(format!(
                        "generated table {} is absent",
                        batch.table_code
                    ))
                })
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let stream = ProviderFactStream {
        manifest: ProviderFactManifest {
            stream_id: derived_identity(b"stream", &[&provider_run.id, &scope.owner_id]).id,
            workspace_id: scope.workspace_id,
            analysis_context_id: scope.analysis_context_id,
            source_generation: scope.source_generation,
            provider_code: ProviderCode::RuffPython as i16,
            provider_version: PROVIDER_VERSION.into(),
            provider_run_id: provider_run.id,
            emitted_at_micros: 0,
            schema_fingerprints,
            declared_rows,
        },
        batches: provider_batches,
        terminal: StreamTerminal::Completed,
    };
    let canonical = CanonicalReconciliationEngine::default().ingest(
        scope,
        &[stream],
        &BTreeMap::from([(ProviderCode::RuffPython as i16, 0)]),
    )?;

    Ok(PythonSemanticProjection {
        provider_run_id: provider_run.id,
        observation,
        canonical,
    })
}

fn validate_reference_edges(batch: &PythonFrontendBatch) -> Result<(), FactIngestError> {
    for reference in &batch.references {
        let expected = if reference.resolution == PythonResolution::MayReferTo {
            PythonSemanticEdgeKind::MayReferTo
        } else {
            PythonSemanticEdgeKind::RefersTo
        };
        if !batch.edges.iter().any(|edge| {
            edge.subject_id == reference.reference_id
                && edge.object_id == reference.target_id
                && edge.kind == expected
        }) {
            return Err(FactIngestError::Protocol(format!(
                "Ruff reference {} lacks its explicit {expected:?} edge",
                reference.name
            )));
        }
        match reference.resolution {
            PythonResolution::Resolved if reference.unknown_reason_code.is_some() => {
                return Err(FactIngestError::Protocol(format!(
                    "resolved Ruff reference {} carries an unknown reason",
                    reference.name
                )));
            }
            PythonResolution::MayReferTo | PythonResolution::UnknownSymbol
                if reference.unknown_reason_code.is_none() =>
            {
                return Err(FactIngestError::Protocol(format!(
                    "non-exact Ruff reference {} lacks an unknown reason",
                    reference.name
                )));
            }
            PythonResolution::Resolved
            | PythonResolution::MayReferTo
            | PythonResolution::UnknownSymbol
            | PythonResolution::UnboundLocal => {}
        }
    }
    Ok(())
}

fn observation_batch(
    batch: &PythonFrontendBatch,
    payloads: &[Vec<u8>; 5],
) -> Result<RecordBatch, FactIngestError> {
    let schema = Arc::new(registered_provider_observation_arrow_schema(
        OBSERVATION_SCHEMA_ID,
    )?);
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(vec![Some(batch.module_name.as_str())])),
        Arc::new(StringArray::from(vec![Some(
            batch.provider_image_fingerprint.as_str(),
        )])),
        Arc::new(BinaryArray::from(vec![Some(payloads[0].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[1].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[2].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[3].as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(payloads[4].as_slice())])),
    ];
    RecordBatch::try_new(schema, columns).map_err(FactIngestError::from)
}

fn observation_payloads(batch: &PythonFrontendBatch) -> Result<[Vec<u8>; 5], FactIngestError> {
    let scopes = batch
        .scopes
        .iter()
        .map(|fact| {
            json!({
                "scope_id": hex_id(fact.scope_id),
                "parent_scope_id": fact.parent_scope_id.map(hex_id),
                "kind": scope_kind_name(fact.kind),
                "name": fact.name,
                "start_byte": fact.start_byte,
                "end_byte": fact.end_byte,
            })
        })
        .collect::<Vec<_>>();
    let bindings = batch
        .bindings
        .iter()
        .map(|fact| {
            json!({
                "binding_id": hex_id(fact.binding_id),
                "scope_id": hex_id(fact.scope_id),
                "name": fact.name,
                "kind": binding_kind_name(fact.kind),
                "target_form": target_form_name(fact.target_form),
                "start_byte": fact.start_byte,
                "end_byte": fact.end_byte,
            })
        })
        .collect::<Vec<_>>();
    let references = batch
        .references
        .iter()
        .map(|fact| {
            json!({
                "reference_id": hex_id(fact.reference_id),
                "scope_id": hex_id(fact.scope_id),
                "target_id": hex_id(fact.target_id),
                "name": fact.name,
                "class": reference_class_name(fact.class),
                "resolution": resolution_name(fact.resolution),
                "start_byte": fact.start_byte,
                "end_byte": fact.end_byte,
                "unknown_reason_code": fact.unknown_reason_code,
            })
        })
        .collect::<Vec<_>>();
    let unknowns = batch
        .unknown_symbols
        .iter()
        .map(|fact| {
            json!({
                "unknown_symbol_id": hex_id(fact.unknown_symbol_id),
                "scope_id": hex_id(fact.scope_id),
                "name": fact.name,
                "reason_code": fact.reason_code,
            })
        })
        .collect::<Vec<_>>();
    let edges = batch
        .edges
        .iter()
        .map(|edge| {
            json!({
                "subject_id": hex_id(edge.subject_id),
                "object_id": hex_id(edge.object_id),
                "kind": edge_kind_name(edge.kind),
            })
        })
        .collect::<Vec<_>>();
    Ok([
        json_bytes(&scopes)?,
        json_bytes(&bindings)?,
        json_bytes(&references)?,
        json_bytes(&unknowns)?,
        json_bytes(&edges)?,
    ])
}

fn json_bytes(value: &[Value]) -> Result<Vec<u8>, FactIngestError> {
    serde_json::to_vec(value)
        .map_err(|error| FactIngestError::Protocol(format!("Ruff observation JSON: {error}")))
}

fn canonical_identity(
    identities: &BTreeMap<[u8; 16], DerivedIdentity>,
    semantic_id: [u8; 16],
    role: &str,
) -> Result<DerivedIdentity, FactIngestError> {
    identities.get(&semantic_id).copied().ok_or_else(|| {
        FactIngestError::Protocol(format!("Ruff {role} references an absent semantic ID"))
    })
}

fn required_entity_kind(
    name: &str,
) -> Result<crate::registries::OntologyCodeEntry, FactIngestError> {
    entity_kind(name)
        .ok_or_else(|| FactIngestError::Protocol(format!("entity kind {name} is absent")))
}

fn required_relation_kind(
    name: &str,
) -> Result<crate::registries::OntologyCodeEntry, FactIngestError> {
    relation_kind(name)
        .ok_or_else(|| FactIngestError::Protocol(format!("relation kind {name} is absent")))
}

fn required_fact_form(name: &str) -> Result<i16, FactIngestError> {
    fact_kind_code(name)
        .and_then(|code| i16::try_from(code).ok())
        .ok_or_else(|| FactIngestError::Protocol(format!("fact form {name} is absent")))
}

#[allow(clippy::too_many_arguments)]
fn push_relation(
    output: &mut Vec<RelationRow>,
    scope: FactScope,
    file_id: [u8; 16],
    kind: crate::registries::OntologyCodeEntry,
    source_id: [u8; 16],
    target_id: [u8; 16],
    start_byte: Option<i64>,
    end_byte: Option<i64>,
    certainty_code: i16,
    resolution_code: i16,
) {
    let identity = derived_identity(
        b"relation",
        &[
            &scope.owner_id,
            &kind.code.to_be_bytes(),
            &source_id,
            &target_id,
        ],
    );
    output.push(RelationRow {
        scope,
        fact_id: identity.id,
        language: Language::Python as i16,
        relation_family_code: kind.family_code,
        relation_kind_code: kind.code,
        source_id,
        target_id,
        ordinal: None,
        role_code: None,
        distance: None,
        directness_code: Directness::Direct as i16,
        file_id: Some(file_id),
        start_byte,
        end_byte,
        certainty_code,
        resolution_code,
        producer_code: ProviderCode::RuffPython as i16,
        derivation_code: None,
        flags: 0,
        fact_hash64: digest_hash64(identity.digest),
    });
}

#[allow(clippy::too_many_arguments)]
fn evidence_row(
    scope: FactScope,
    provider_run_id: [u8; 16],
    fact_id: [u8; 16],
    fact_form_code: i16,
    file_id: Option<[u8; 16]>,
    start_byte: Option<i64>,
    end_byte: Option<i64>,
    certainty_code: i16,
    resolution_code: i16,
) -> FactEvidenceRow {
    let observation_id = derived_identity(b"observation", &[&provider_run_id, &fact_id]).id;
    FactEvidenceRow {
        evidence_id: crate::identity::fact_evidence_id(provider_run_id, observation_id, fact_id),
        scope,
        fact_id,
        fact_form_code,
        provider_code: ProviderCode::RuffPython as i16,
        provider_version: PROVIDER_VERSION.into(),
        provider_run_id,
        observation_id,
        raw_kind_code: None,
        file_id,
        start_byte,
        end_byte,
        certainty_code,
        resolution_code,
        conflict_disposition_code: 10,
        cold_payload: None,
    }
}

fn derived_identity(domain: &[u8], parts: &[&[u8]]) -> DerivedIdentity {
    let mut hasher = blake3::Hasher::new_derive_key("codefabric.python-canonical-fact.v1");
    hasher.update(&(domain.len() as u64).to_be_bytes());
    hasher.update(domain);
    for part in parts {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    let digest = *hasher.finalize().as_bytes();
    let mut id = [0_u8; 16];
    id.copy_from_slice(&digest[..16]);
    DerivedIdentity { id, digest }
}

fn digest_hash64(digest: [u8; 32]) -> i64 {
    i64::from_be_bytes(digest[..8].try_into().expect("eight digest bytes"))
}

fn coordinate(value: u64) -> Result<i64, FactIngestError> {
    i64::try_from(value)
        .map_err(|_| FactIngestError::Protocol("Ruff byte coordinate exceeds Int64".into()))
}

fn canonical_resolution(resolution: PythonResolution) -> (i16, i16) {
    match resolution {
        PythonResolution::Resolved => (
            EvidenceCertainty::StaticSemantic as i16,
            ResolutionClass::StaticallyResolved as i16,
        ),
        PythonResolution::MayReferTo => (
            EvidenceCertainty::SoundMay as i16,
            ResolutionClass::SoundPossible as i16,
        ),
        PythonResolution::UnknownSymbol | PythonResolution::UnboundLocal => (
            EvidenceCertainty::Unresolved as i16,
            ResolutionClass::Unresolved as i16,
        ),
    }
}

fn hex_id(id: [u8; 16]) -> String {
    let mut output = String::with_capacity(32);
    for byte in id {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

const fn scope_kind_name(kind: PythonScopeKind) -> &'static str {
    match kind {
        PythonScopeKind::Module => "MODULE",
        PythonScopeKind::Function => "FUNCTION",
        PythonScopeKind::Class => "CLASS",
        PythonScopeKind::Lambda => "LAMBDA",
        PythonScopeKind::Comprehension => "COMPREHENSION",
        PythonScopeKind::Annotation => "ANNOTATION",
        PythonScopeKind::TypeParameter => "TYPE_PARAMETER",
    }
}

const fn binding_kind_name(kind: PythonBindingKind) -> &'static str {
    match kind {
        PythonBindingKind::Local => "LOCAL",
        PythonBindingKind::Parameter => "PARAMETER",
        PythonBindingKind::Global => "GLOBAL",
        PythonBindingKind::Nonlocal => "NONLOCAL",
        PythonBindingKind::Import => "IMPORT",
        PythonBindingKind::ClassAttribute => "CLASS_ATTRIBUTE",
        PythonBindingKind::InstanceAttribute => "INSTANCE_ATTRIBUTE",
        PythonBindingKind::Comprehension => "COMPREHENSION",
        PythonBindingKind::Loop => "LOOP",
        PythonBindingKind::With => "WITH",
        PythonBindingKind::Exception => "EXCEPTION",
        PythonBindingKind::Match => "MATCH",
        PythonBindingKind::Walrus => "WALRUS",
        PythonBindingKind::TypeParameter => "TYPE_PARAMETER",
        PythonBindingKind::TypeAlias => "TYPE_ALIAS",
        PythonBindingKind::Free => "FREE",
        PythonBindingKind::Cell => "CELL",
        PythonBindingKind::Builtin => "BUILTIN",
        PythonBindingKind::Function => "FUNCTION",
        PythonBindingKind::Class => "CLASS",
    }
}

const fn target_form_name(form: PythonTargetForm) -> &'static str {
    match form {
        PythonTargetForm::FunctionName => "FUNCTION_NAME",
        PythonTargetForm::ClassName => "CLASS_NAME",
        PythonTargetForm::Parameter => "PARAMETER",
        PythonTargetForm::Assignment => "ASSIGNMENT",
        PythonTargetForm::AnnotatedAssignment => "ANNOTATED_ASSIGNMENT",
        PythonTargetForm::AugmentedAssignment => "AUGMENTED_ASSIGNMENT",
        PythonTargetForm::NamedExpression => "NAMED_EXPRESSION",
        PythonTargetForm::ImportAlias => "IMPORT_ALIAS",
        PythonTargetForm::LoopTarget => "LOOP_TARGET",
        PythonTargetForm::WithTarget => "WITH_TARGET",
        PythonTargetForm::ExceptionTarget => "EXCEPTION_TARGET",
        PythonTargetForm::MatchCapture => "MATCH_CAPTURE",
        PythonTargetForm::ComprehensionTarget => "COMPREHENSION_TARGET",
        PythonTargetForm::GlobalDeclaration => "GLOBAL_DECLARATION",
        PythonTargetForm::NonlocalDeclaration => "NONLOCAL_DECLARATION",
        PythonTargetForm::TypeParameter => "TYPE_PARAMETER",
        PythonTargetForm::TypeAlias => "TYPE_ALIAS",
    }
}

const fn reference_class_name(class: PythonReferenceClass) -> &'static str {
    match class {
        PythonReferenceClass::Read => "READ",
        PythonReferenceClass::Write => "WRITE",
        PythonReferenceClass::ReadWrite => "READ_WRITE",
        PythonReferenceClass::Delete => "DELETE",
        PythonReferenceClass::TypeReference => "TYPE_REFERENCE",
        PythonReferenceClass::CallReference => "CALL_REFERENCE",
        PythonReferenceClass::ImportReference => "IMPORT_REFERENCE",
    }
}

const fn resolution_name(resolution: PythonResolution) -> &'static str {
    match resolution {
        PythonResolution::Resolved => "RESOLVED",
        PythonResolution::MayReferTo => "MAY_REFER_TO",
        PythonResolution::UnknownSymbol => "UNKNOWN_SYMBOL",
        PythonResolution::UnboundLocal => "UNBOUND_LOCAL",
    }
}

const fn edge_kind_name(kind: PythonSemanticEdgeKind) -> &'static str {
    match kind {
        PythonSemanticEdgeKind::RefersTo => "REFERS_TO",
        PythonSemanticEdgeKind::MayReferTo => "MAY_REFER_TO",
        PythonSemanticEdgeKind::Shadows => "SHADOWS",
        PythonSemanticEdgeKind::Rebinds => "REBINDS",
        PythonSemanticEdgeKind::GlobalResolution => "GLOBAL_RESOLUTION",
        PythonSemanticEdgeKind::NonlocalResolution => "NONLOCAL_RESOLUTION",
        PythonSemanticEdgeKind::Captures => "CAPTURES",
        PythonSemanticEdgeKind::CapturedFrom => "CAPTURED_FROM",
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::fabric::batch_checksum;
    use crate::ruff_adapter::{
        PythonBindingFact, PythonReferenceFact, PythonScopeFact, PythonSemanticEdge,
        PythonSemanticMetrics, PythonSemanticTerminal,
    };

    fn scope(owner: u8) -> FactScope {
        FactScope {
            workspace_id: [1; 16],
            analysis_context_id: [2; 16],
            source_generation: 7,
            owner_id: [owner; 16],
        }
    }

    fn fixture(module: &str, extra_binding: bool) -> PythonFrontendBatch {
        let module_scope = [10; 16];
        let binding = [20; 16];
        let reference = [30; 16];
        let mut bindings = vec![PythonBindingFact {
            binding_id: binding,
            scope_id: module_scope,
            name: "value".into(),
            kind: PythonBindingKind::Local,
            target_form: PythonTargetForm::Assignment,
            start_byte: 0,
            end_byte: 5,
        }];
        if extra_binding {
            bindings.push(PythonBindingFact {
                binding_id: [21; 16],
                scope_id: module_scope,
                name: "changed".into(),
                kind: PythonBindingKind::Local,
                target_form: PythonTargetForm::Assignment,
                start_byte: 12,
                end_byte: 19,
            });
        }
        PythonFrontendBatch {
            module_name: module.into(),
            provider_image_fingerprint: format!("b3:{module}"),
            scopes: vec![PythonScopeFact {
                scope_id: module_scope,
                parent_scope_id: None,
                kind: PythonScopeKind::Module,
                name: Some(module.into()),
                start_byte: 0,
                end_byte: if extra_binding { 20 } else { 11 },
            }],
            bindings,
            references: vec![PythonReferenceFact {
                reference_id: reference,
                scope_id: module_scope,
                name: "value".into(),
                class: PythonReferenceClass::Read,
                resolution: PythonResolution::Resolved,
                target_id: binding,
                start_byte: 6,
                end_byte: 11,
                unknown_reason_code: None,
            }],
            unknown_symbols: Vec::new(),
            edges: vec![PythonSemanticEdge {
                subject_id: reference,
                object_id: binding,
                kind: PythonSemanticEdgeKind::RefersTo,
            }],
            metrics: PythonSemanticMetrics {
                binding_pass_duration: Duration::ZERO,
                traversal_pass_duration: Duration::ZERO,
                cleanup_duration: Duration::ZERO,
                visited_nodes: 3,
                scope_count: 1,
                binding_count: if extra_binding { 2 } else { 1 },
                reference_count: 1,
                unresolved_reference_count: 0,
            },
            terminal: PythonSemanticTerminal {
                pass_id: "PASS_RUFF_SCOPE_BINDING_V1",
                provider_id: "ruff-python",
                terminal_state: "completed",
                failure_code: None,
            },
        }
    }

    #[test]
    fn py_scope_binding_owner_replacement_gate() {
        let changed_before =
            project_ruff_semantic_batch(scope(10), [40; 16], &fixture("owner_a", false)).unwrap();
        let changed_after =
            project_ruff_semantic_batch(scope(10), [40; 16], &fixture("owner_a", true)).unwrap();
        let stable_before =
            project_ruff_semantic_batch(scope(11), [41; 16], &fixture("owner_b", false)).unwrap();
        let stable_after =
            project_ruff_semantic_batch(scope(11), [41; 16], &fixture("owner_b", false)).unwrap();

        assert_eq!(
            changed_before.observation.schema(),
            stable_before.observation.schema()
        );
        for table_code in [200, 210, 220] {
            let a_before =
                batch_checksum(changed_before.batch(table_code).unwrap().batch()).unwrap();
            let a_after = batch_checksum(changed_after.batch(table_code).unwrap().batch()).unwrap();
            let b_before =
                batch_checksum(stable_before.batch(table_code).unwrap().batch()).unwrap();
            let b_after = batch_checksum(stable_after.batch(table_code).unwrap().batch()).unwrap();
            assert_eq!(b_before, b_after, "unchanged owner table {table_code}");
            if table_code == 220 {
                assert_eq!(a_before, a_after, "unchanged reference detail table");
            } else {
                assert_ne!(a_before, a_after, "changed owner table {table_code}");
            }
        }
        assert_ne!(
            changed_before.provider_run_id,
            stable_before.provider_run_id
        );
    }
}
