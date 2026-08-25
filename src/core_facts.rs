//! Wave-4 source admission, capability evidence, and canonical fact orchestration.
//!
//! This module owns policy decisions above provider libraries. Tree-sitter and Ruff values
//! remain behind their adapters; generated registries and the deployment profile provide the
//! data that drives admission and completeness.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow_array::{Array as _, BooleanArray, ListArray, StringArray, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use thiserror::Error;

use crate::contracts::models::DeploymentProfileDocument;
use crate::fabric::{
    EmptySnapshotOverlay, FabricError, MutationJournal, OwnerMutationRequest,
    OwnerPublicationWrite, PublicationOutcome, PublicationRequest, SnapshotOverlayProviderFactory,
    SnapshotProviderCatalog, WorkspaceFabric,
};
use crate::fact_ingest::{
    CanonicalIngestOutput, CanonicalReconciliationEngine, CapabilityStatusRow, EntityRow,
    FactEvidenceRow, FactIngestError, FactScope, OwnerRow, PropertyFactRow, PropertyValue,
    ProviderFactBatch, ProviderFactManifest, ProviderFactStream, StreamTerminal,
    decode_validated_arrow_ipc_chunks, encode_capability_statuses, encode_entities,
    encode_evidence, encode_owners, encode_properties, encode_relations,
};
use crate::identity::{
    semantic_entity_identity, semantic_owner_identity, text_property_fact_identity,
};
use crate::model_generated::schema_tables::{
    PROVIDER_OBSERVATION_SCHEMAS, ProviderObservationLogicalType, ProviderObservationSchema,
};
use crate::provider_raw_kinds::{
    PROVIDER_GRAMMAR_INVENTORIES, ProviderRawKindDisposition, RUFF_PYTHON_FRONTEND,
};
use crate::registries::{
    CAPABILITY_IDS, Completeness, Directness, EvidenceCertainty, Language, OwnerCapabilityState,
    OwnerKind, ProviderCode, ProviderObservationFamily, ResolutionClass, UNKNOWN_IDS,
    capability_code, capability_mask, entity_kind, property_kind,
};
use crate::ruff_adapter::RuffSnapshot;
use crate::ruff_adapter::{NeverRuffCancelled, RuffAdapter, RuffAdapterError};
use crate::rustc_service::{AcceptedRustcCompilation, AcceptedRustcOwner};
use crate::schema_registry::table_spec;
use crate::snapshot::ServingSnapshotManifestBody;
use crate::snapshot_runtime::{ServingSnapshotCandidate, SnapshotRuntimeError};
use crate::source_image::{SourceImage, SourceLanguage};
use crate::source_syntax::SourceSyntaxProviderRuns;
use crate::tree_sitter_adapter::{
    NeverCancelled, TreeSitterAdapter, TreeSitterAdapterError, TreeSitterLanguage,
    TreeSitterSnapshot,
};

const DEPLOYMENT_PROFILE: &[u8] =
    include_bytes!("../contracts/deployment/local-workstation-v1.yaml");

/// Stable source classification. Inventory identity is retained for every variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionDisposition {
    Analyze,
    InventoryOnly,
    EndpointOnly,
    GeneratedCaptureRequired,
    BlockedContext,
}

/// One admission request detached from filesystem and parser library types.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // These flags are independent admission facts from the deployment profile.
pub struct AdmissionInput {
    pub path_components: Vec<Vec<u8>>,
    pub bytes: Vec<u8>,
    pub explicit_language: Option<SourceLanguage>,
    pub recognized_encoding: bool,
    pub explicitly_authorized_large_file: bool,
    pub generated_capture: bool,
    pub vendored_body_authorized: bool,
    pub required_configuration: bool,
}

/// Deterministic AC-G-43 classification and its capability-facing reason.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionDecision {
    pub disposition: AdmissionDisposition,
    pub language: Option<SourceLanguage>,
    pub reason_code: &'static str,
    pub diagnostics: Vec<&'static str>,
    pub observed_bytes: u64,
}

/// Model-owned admission policy decoded from the released deployment authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceAdmissionPolicy {
    ordinary_maximum_bytes: u64,
    explicit_maximum_bytes: u64,
    binary_sample_bytes: usize,
    maximum_single_line_bytes: usize,
    maximum_path_components: usize,
    maximum_path_bytes: usize,
    excluded_directories: BTreeSet<Vec<u8>>,
    vendored_directories: BTreeSet<Vec<u8>>,
    generated_directories: BTreeSet<Vec<u8>>,
    binary_signatures: Vec<(String, Vec<u8>)>,
}

impl SourceAdmissionPolicy {
    /// Decode and validate the sole local-workstation source policy.
    ///
    /// # Errors
    ///
    /// Rejects malformed typed deployment data, zero bounds, invalid signature hex, duplicate
    /// signature names, or a non-monotonic ordinary/explicit byte limit.
    pub fn local_workstation() -> Result<Self, CoreFactError> {
        let profile: DeploymentProfileDocument = serde_yaml_ng::from_slice(DEPLOYMENT_PROFILE)
            .map_err(|error| CoreFactError::Policy(error.to_string()))?;
        let source = profile.source_admission;
        let mut names = BTreeSet::new();
        let binary_signatures = source
            .binary_signatures
            .into_iter()
            .map(|signature| {
                if !names.insert(signature.name.clone()) {
                    return Err(CoreFactError::Policy(
                        "binary signature names must be unique".into(),
                    ));
                }
                Ok((signature.name, decode_hex(&signature.prefix_hex)?))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if profile.source_image_limits.ordinary_maximum_bytes == 0
            || profile.source_image_limits.explicit_maximum_bytes
                < profile.source_image_limits.ordinary_maximum_bytes
            || source.binary_sample_bytes == 0
            || source.maximum_single_line_bytes == 0
            || source.maximum_path_components == 0
            || source.maximum_path_bytes == 0
            || binary_signatures.is_empty()
        {
            return Err(CoreFactError::Policy(
                "source admission limits are incomplete".into(),
            ));
        }
        Ok(Self {
            ordinary_maximum_bytes: profile.source_image_limits.ordinary_maximum_bytes,
            explicit_maximum_bytes: profile.source_image_limits.explicit_maximum_bytes,
            binary_sample_bytes: usize::try_from(source.binary_sample_bytes)
                .map_err(|_| CoreFactError::Policy("binary sample does not fit host".into()))?,
            maximum_single_line_bytes: usize::try_from(source.maximum_single_line_bytes)
                .map_err(|_| CoreFactError::Policy("line limit does not fit host".into()))?,
            maximum_path_components: usize::from(source.maximum_path_components),
            maximum_path_bytes: usize::from(source.maximum_path_bytes),
            excluded_directories: bytes_set(source.excluded_directory_names),
            vendored_directories: bytes_set(source.vendored_directory_names),
            generated_directories: bytes_set(source.generated_directory_names),
            binary_signatures,
        })
    }

    /// Apply the complete AC-G-43 precedence ladder without reading the filesystem.
    #[must_use]
    #[allow(clippy::too_many_lines)] // One ordered ladder makes every mutually exclusive admission outcome auditable.
    pub fn classify(&self, input: &AdmissionInput) -> AdmissionDecision {
        let observed_bytes = u64::try_from(input.bytes.len()).unwrap_or(u64::MAX);
        if input.path_components.len() > self.maximum_path_components
            || path_length(&input.path_components) > self.maximum_path_bytes
        {
            return decision(
                AdmissionDisposition::InventoryOnly,
                None,
                "PATH_LIMIT_EXCEEDED",
                observed_bytes,
            );
        }
        if contains_component(&input.path_components, &self.excluded_directories) {
            return decision(
                AdmissionDisposition::InventoryOnly,
                None,
                "EXCLUDED_POLICY",
                observed_bytes,
            );
        }
        if observed_bytes > self.explicit_maximum_bytes {
            return decision(
                if input.required_configuration {
                    AdmissionDisposition::BlockedContext
                } else {
                    AdmissionDisposition::InventoryOnly
                },
                None,
                if input.required_configuration {
                    "CONFIG_ARTIFACT_TOO_LARGE"
                } else {
                    "SOURCE_HARD_LIMIT_EXCEEDED"
                },
                observed_bytes,
            );
        }
        if observed_bytes > self.ordinary_maximum_bytes && !input.explicitly_authorized_large_file {
            return decision(
                AdmissionDisposition::InventoryOnly,
                None,
                "SOURCE_ORDINARY_LIMIT_EXCEEDED",
                observed_bytes,
            );
        }
        if contains_component(&input.path_components, &self.generated_directories)
            && !input.generated_capture
        {
            return decision(
                AdmissionDisposition::GeneratedCaptureRequired,
                None,
                "GENERATED_CAPTURE_REQUIRED",
                observed_bytes,
            );
        }
        if contains_component(&input.path_components, &self.vendored_directories)
            && !input.vendored_body_authorized
        {
            return decision(
                AdmissionDisposition::EndpointOnly,
                None,
                "VENDORED_ENDPOINT_ONLY",
                observed_bytes,
            );
        }
        let sample = &input.bytes[..input.bytes.len().min(self.binary_sample_bytes)];
        if sample.contains(&0)
            || self
                .binary_signatures
                .iter()
                .any(|(_, prefix)| sample.starts_with(prefix))
            || !input.recognized_encoding
        {
            return decision(
                AdmissionDisposition::InventoryOnly,
                None,
                "UNSUPPORTED_BINARY",
                observed_bytes,
            );
        }
        let (language, mut diagnostics) = language(input);
        if input
            .bytes
            .split(|byte| *byte == b'\n' || *byte == b'\r')
            .any(|line| line.len() > self.maximum_single_line_bytes)
        {
            diagnostics.push("SOURCE_LINE_LIMIT_WARNING");
        }
        match language {
            Some(language) => AdmissionDecision {
                disposition: AdmissionDisposition::Analyze,
                language: Some(language),
                reason_code: "SUPPORTED_SOURCE",
                diagnostics,
                observed_bytes,
            },
            None => AdmissionDecision {
                disposition: AdmissionDisposition::InventoryOnly,
                language: None,
                reason_code: "UNSUPPORTED_CONTENT",
                diagnostics,
                observed_bytes,
            },
        }
    }
}

fn bytes_set(values: BTreeSet<String>) -> BTreeSet<Vec<u8>> {
    values.into_iter().map(String::into_bytes).collect()
}

fn path_length(components: &[Vec<u8>]) -> usize {
    components
        .iter()
        .map(Vec::len)
        .sum::<usize>()
        .saturating_add(components.len().saturating_sub(1))
}

fn contains_component(components: &[Vec<u8>], values: &BTreeSet<Vec<u8>>) -> bool {
    components
        .iter()
        .any(|component| values.contains(component))
}

fn decision(
    disposition: AdmissionDisposition,
    language: Option<SourceLanguage>,
    reason_code: &'static str,
    observed_bytes: u64,
) -> AdmissionDecision {
    AdmissionDecision {
        disposition,
        language,
        reason_code,
        diagnostics: Vec::new(),
        observed_bytes,
    }
}

fn language(input: &AdmissionInput) -> (Option<SourceLanguage>, Vec<&'static str>) {
    let extension = input
        .path_components
        .last()
        .and_then(|name| name.rsplit(|byte| *byte == b'.').next())
        .and_then(|extension| match extension {
            b"py" | b"pyi" => Some(SourceLanguage::Python),
            b"rs" => Some(SourceLanguage::Rust),
            _ => None,
        });
    let first_line = input
        .bytes
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or_default();
    let shebang = if first_line.starts_with(b"#!") && first_line.windows(6).any(|v| v == b"python")
    {
        Some(SourceLanguage::Python)
    } else {
        None
    };
    let selected = input.explicit_language.or(extension).or(shebang);
    let conflict = [input.explicit_language, extension, shebang]
        .into_iter()
        .flatten()
        .any(|candidate| Some(candidate) != selected);
    (
        selected,
        if conflict {
            vec!["LANGUAGE_EVIDENCE_CONFLICT"]
        } else {
            Vec::new()
        },
    )
}

fn decode_hex(value: &str) -> Result<Vec<u8>, CoreFactError> {
    if value.is_empty() || !value.len().is_multiple_of(2) {
        return Err(CoreFactError::Policy(
            "binary signature hex is invalid".into(),
        ));
    }
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let high = char::from(pair[0])
                .to_digit(16)
                .ok_or_else(|| CoreFactError::Policy("binary signature hex is invalid".into()))?;
            let low = char::from(pair[1])
                .to_digit(16)
                .ok_or_else(|| CoreFactError::Policy("binary signature hex is invalid".into()))?;
            u8::try_from((high << 4) | low)
                .map_err(|_| CoreFactError::Policy("binary signature hex is invalid".into()))
        })
        .collect()
}

/// One child record supplied to the formal AC-G-36 aggregation algebra.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // The aggregation algebra requires these independent closure predicates.
pub struct CapabilityChild {
    pub applicable: bool,
    pub completeness: Completeness,
    pub has_facts: bool,
    pub missing_remainder_characterized: bool,
    pub required_context_covered: bool,
    pub external_policy_allows_closure: bool,
}

/// Aggregate capability completeness without provider-specific status strings.
#[must_use]
pub fn aggregate_capability(children: &[CapabilityChild]) -> Completeness {
    let applicable = children
        .iter()
        .filter(|child| child.applicable)
        .collect::<Vec<_>>();
    if applicable.is_empty() {
        return Completeness::NotApplicable;
    }
    if applicable
        .iter()
        .any(|child| child.completeness == Completeness::Indeterminate)
    {
        return Completeness::Indeterminate;
    }
    if applicable.iter().all(|child| {
        child.completeness == Completeness::Complete
            && child.required_context_covered
            && child.external_policy_allows_closure
    }) {
        return Completeness::Complete;
    }
    if applicable.iter().any(|child| child.has_facts) {
        if applicable
            .iter()
            .all(|child| child.missing_remainder_characterized)
        {
            Completeness::Partial
        } else {
            Completeness::Indeterminate
        }
    } else {
        Completeness::Unavailable
    }
}

/// Canonical owner-capability record retained independently from provider-run state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityEvidence {
    pub workspace_id: [u8; 16],
    pub analysis_context_id: [u8; 16],
    pub owner_id: [u8; 16],
    pub capability_code: &'static str,
    pub source_generation: u64,
    pub state: OwnerCapabilityState,
    pub completeness: Completeness,
    pub provider_run_id: Option<[u8; 16]>,
    pub reason_code: &'static str,
    pub diagnostic_id: Option<[u8; 16]>,
    pub fallback_source_available: bool,
    pub coverage_scope_fingerprint: [u8; 32],
    pub external_remainder: bool,
    pub unknown_remainder: bool,
}

impl CapabilityEvidence {
    /// Construct capability evidence only for a generated capability identifier.
    ///
    /// # Errors
    ///
    /// Rejects a capability code absent from the generated registry.
    #[allow(clippy::too_many_arguments)] // Generated capability evidence exposes the complete governed row contract.
    pub fn new(
        scope: FactScope,
        capability_code: &'static str,
        state: OwnerCapabilityState,
        completeness: Completeness,
        reason_code: &'static str,
        provider_run_id: Option<[u8; 16]>,
        external_remainder: bool,
        unknown_remainder: bool,
    ) -> Result<Self, CoreFactError> {
        if !CAPABILITY_IDS.contains(&capability_code) {
            return Err(CoreFactError::UnknownCapability(capability_code.to_owned()));
        }
        Ok(Self {
            workspace_id: scope.workspace_id,
            analysis_context_id: scope.analysis_context_id,
            owner_id: scope.owner_id,
            capability_code,
            source_generation: u64::try_from(scope.source_generation)
                .map_err(|_| CoreFactError::Policy("negative source generation".into()))?,
            state,
            completeness,
            provider_run_id,
            reason_code,
            diagnostic_id: None,
            fallback_source_available: true,
            coverage_scope_fingerprint: crate::identity::capability_scope_fingerprint(
                scope.workspace_id,
                scope.analysis_context_id,
                scope.owner_id,
                scope.source_generation,
                capability_code,
            ),
            external_remainder,
            unknown_remainder,
        })
    }
}

/// Explicit unknown value accepted only when both kind and reason are governed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnknownEvidence {
    pub unknown_kind: &'static str,
    pub reason_class: &'static str,
    pub candidate_set_digest: Option<[u8; 32]>,
}

impl UnknownEvidence {
    /// Validate the two identifiers against the generated unknown registry projection.
    ///
    /// # Errors
    ///
    /// Rejects unregistered kind/reason identifiers.
    pub fn new(
        unknown_kind: &'static str,
        reason_class: &'static str,
        candidate_set_digest: Option<[u8; 32]>,
    ) -> Result<Self, CoreFactError> {
        if !UNKNOWN_IDS.contains(&unknown_kind) || !UNKNOWN_IDS.contains(&reason_class) {
            return Err(CoreFactError::UnknownEvidence);
        }
        Ok(Self {
            unknown_kind,
            reason_class,
            candidate_set_digest,
        })
    }
}

/// Programmatic raw-kind disposition census across every linked provider inventory.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProviderDispositionCensus {
    pub normalized: usize,
    pub ignored: usize,
    pub unsupported: usize,
}

/// Require every generated raw kind to have exactly one closed disposition.
///
/// # Errors
///
/// Rejects empty/duplicate provider raw-kind identities or an empty provider inventory.
pub fn provider_disposition_census() -> Result<ProviderDispositionCensus, CoreFactError> {
    let mut census = ProviderDispositionCensus::default();
    let mut identities = BTreeSet::new();
    for inventory in PROVIDER_GRAMMAR_INVENTORIES {
        for raw in inventory.raw_kinds {
            if raw.raw_name.is_empty()
                || !identities.insert((inventory.catalog_id, raw.raw_kind_id, raw.raw_name))
            {
                return Err(CoreFactError::ProviderInventory);
            }
            census.record(raw.disposition);
        }
    }
    for raw in RUFF_PYTHON_FRONTEND.node_kinds {
        if raw.raw_name.is_empty()
            || !identities.insert((
                RUFF_PYTHON_FRONTEND.catalog_id,
                raw.raw_kind_id,
                raw.raw_name,
            ))
        {
            return Err(CoreFactError::ProviderInventory);
        }
        census.record(raw.disposition);
    }
    census.normalized = census
        .normalized
        .saturating_add(RUFF_PYTHON_FRONTEND.token_kinds.len());
    if census.normalized + census.ignored + census.unsupported == 0 {
        return Err(CoreFactError::ProviderInventory);
    }
    Ok(census)
}

impl ProviderDispositionCensus {
    fn record(&mut self, disposition: ProviderRawKindDisposition) {
        match disposition {
            ProviderRawKindDisposition::Normalize => self.normalized += 1,
            ProviderRawKindDisposition::Ignore => self.ignored += 1,
            ProviderRawKindDisposition::Unsupported => self.unsupported += 1,
        }
    }
}

/// Sole Wave-4 orchestration boundary over source/syntax and generic provider observations.
#[derive(Clone, Debug, Default)]
pub struct CoreFactEngine {
    reconciliation: CanonicalReconciliationEngine,
}

/// Complete, application-owned result of the in-process Wave-4 source lane.
///
/// Provider snapshots are retained with the canonical result so diagnostics and
/// incremental callers can inspect the exact observations that produced the rows.
#[derive(Debug)]
pub struct CoreSourceAnalysis {
    pub tree_sitter: TreeSitterSnapshot,
    pub ruff_python: Option<RuffSnapshot>,
    pub canonical: CanonicalIngestOutput,
}

impl CoreFactEngine {
    /// Run the supported in-process provider stack and reconcile its output once.
    ///
    /// This is the sole production path from an immutable [`SourceImage`] to canonical
    /// source/syntax batches. Rust uses Tree-sitter; Python uses the same Tree-sitter
    /// snapshot as both canonical syntax input and Ruff's structural cross-check.
    ///
    /// # Errors
    ///
    /// Rejects an unsupported source language, absent provider text, any provider
    /// resource/version/parse failure, or canonical reconciliation failure.
    pub fn analyze_source(
        &self,
        scope: FactScope,
        source: &SourceImage,
        runs: SourceSyntaxProviderRuns,
    ) -> Result<CoreSourceAnalysis, CoreFactError> {
        let text = source
            .provider_text
            .clone()
            .ok_or(CoreFactError::ProviderTextUnavailable)?;
        let language = match source.language {
            SourceLanguage::Python => TreeSitterLanguage::Python,
            SourceLanguage::Rust => TreeSitterLanguage::Rust,
            SourceLanguage::Other => return Err(CoreFactError::UnsupportedLanguage),
        };
        let revision = source.source_generation;
        let mut tree_adapter = TreeSitterAdapter::new(language)?;
        let tree_sitter = tree_adapter.parse_full(revision, text.clone(), &NeverCancelled)?;
        let ruff_python = if source.language == SourceLanguage::Python {
            let mut ruff = RuffAdapter::new()?;
            Some(ruff.parse(revision, text, &tree_sitter, &NeverRuffCancelled)?)
        } else {
            None
        };
        let canonical =
            self.reconcile_source_syntax(scope, source, &tree_sitter, ruff_python.as_ref(), runs)?;
        Ok(CoreSourceAnalysis {
            tree_sitter,
            ruff_python,
            canonical,
        })
    }

    /// Reconcile an immutable source image and provider snapshots into validated Arrow batches.
    ///
    /// # Errors
    ///
    /// Propagates exact source/range/identity/provider/batch validation failures.
    pub fn reconcile_source_syntax(
        &self,
        scope: FactScope,
        source: &SourceImage,
        tree: &TreeSitterSnapshot,
        ruff: Option<&RuffSnapshot>,
        runs: SourceSyntaxProviderRuns,
    ) -> Result<CanonicalIngestOutput, CoreFactError> {
        Ok(self
            .reconciliation
            .ingest_source_syntax(scope, source, tree, ruff, runs)?)
    }

    /// Reconcile arbitrary typed provider streams through the same canonical authority.
    ///
    /// # Errors
    ///
    /// Propagates manifest, fingerprint, precedence, ontology, and Arrow validation failures.
    pub fn reconcile_observations(
        &self,
        scope: FactScope,
        streams: &[ProviderFactStream],
        provider_precedence: &BTreeMap<i16, u16>,
    ) -> Result<CanonicalIngestOutput, CoreFactError> {
        Ok(self
            .reconciliation
            .ingest(scope, streams, provider_precedence)?)
    }

    /// Decode a completely verified compiler stream and reconcile each MIR owner through
    /// the same canonical fact ingress used by in-process providers.
    ///
    /// The thin Wave-5 profile emits one callable entity and its canonical name property per
    /// compiler owner. The complete typed MIR Arrow row remains attached as evidence, so body
    /// changes alter canonical state even while the callable identity stays stable.
    ///
    /// # Errors
    ///
    /// Rejects a non-WP35 Arrow schema, row-count drift, non-singleton owner payload, invalid
    /// CBEF identity, stale generation, or any generated canonical-batch validation failure.
    #[allow(clippy::too_many_lines)] // One accepted compiler transaction keeps all emitted fact families atomic.
    pub fn reconcile_rustc_compilation(
        &self,
        compilation: &AcceptedRustcCompilation,
    ) -> Result<Vec<CanonicalIngestOutput>, CoreFactError> {
        let source_generation = i64::try_from(compilation.begin.source_generation)
            .map_err(|_| FactIngestError::Protocol("source generation exceeds i64".into()))?;
        let provider_run_id = stable_id16(compilation.begin.provider_run_id.as_bytes());
        let provider_code = ProviderCode::RustcMir as i16;
        let capability = i16::try_from(capability_code("RUST_MIR").ok_or_else(|| {
            FactIngestError::Protocol("RUST_MIR capability allocation is absent".into())
        })?)
        .map_err(|_| FactIngestError::Protocol("RUST_MIR capability exceeds i16".into()))?;
        let capability_bits = capability_mask(&["RUST_MIR"])
            .and_then(|mask| i64::try_from(mask).ok())
            .ok_or_else(|| {
                FactIngestError::Protocol("RUST_MIR capability mask is invalid".into())
            })?;
        let precedence = BTreeMap::from([(provider_code, 0)]);
        let callable = entity_kind("CALLABLE")
            .ok_or_else(|| FactIngestError::Protocol("CALLABLE allocation is absent".into()))?;
        let name_property = property_kind("NAME")
            .ok_or_else(|| FactIngestError::Protocol("NAME allocation is absent".into()))?;
        let callable_code = u16::try_from(callable.code)
            .map_err(|_| FactIngestError::Protocol("CALLABLE code is invalid".into()))?;
        let name_property_code = u16::try_from(name_property.code)
            .map_err(|_| FactIngestError::Protocol("NAME code is invalid".into()))?;
        let mut outputs = Vec::with_capacity(compilation.owners.len());
        for owner in &compilation.owners {
            let mir = decode_rustc_owner(owner)?;
            let owner_key = semantic_key(
                crate::identity::SemanticFingerprintDomain::RustcCanonicalOwner,
                &[
                    compilation.begin.compilation_unit_id.as_bytes(),
                    mir.protocol_owner_id.as_bytes(),
                    mir.name.as_bytes(),
                ],
            );
            let owner_identity = semantic_owner_identity(
                compilation.admission.canonical_workspace_id,
                compilation.admission.canonical_analysis_context_id,
                "mir-body",
                owner_key,
            )
            .map_err(FactIngestError::from)?;
            let scope = FactScope {
                workspace_id: compilation.admission.canonical_workspace_id,
                analysis_context_id: compilation.admission.canonical_analysis_context_id,
                source_generation,
                owner_id: owner_identity.id,
            };
            let entity_identity = semantic_entity_identity(
                scope.workspace_id,
                scope.analysis_context_id,
                callable_code,
                scope.owner_id,
                mir.name.as_bytes().to_vec(),
            )
            .map_err(FactIngestError::from)?;
            let property_identity = text_property_fact_identity(
                scope.workspace_id,
                scope.analysis_context_id,
                name_property_code,
                entity_identity.id,
                &mir.name,
            )
            .map_err(FactIngestError::from)?;
            let entity_observation = stable_id16(
                [mir.chunk_digest.as_bytes(), b"\0entity"]
                    .concat()
                    .as_slice(),
            );
            let property_observation = stable_id16(
                [mir.chunk_digest.as_bytes(), b"\0property:name"]
                    .concat()
                    .as_slice(),
            );
            let entity = EntityRow {
                scope,
                entity_id: entity_identity.id,
                language: Language::Rust as i16,
                entity_family_code: callable.family_code,
                entity_kind_code: callable.code,
                raw_kind_code: None,
                file_id: None,
                start_byte: None,
                end_byte: None,
                name: Some(mir.name.clone()),
                qualified_name: Some(mir.name.clone()),
                parent_entity_id: None,
                type_id: None,
                flags: 0,
                fact_hash64: digest_hash64(entity_identity.full_digest),
            };
            let property = PropertyFactRow {
                scope,
                fact_id: property_identity.id,
                subject_entity_id: entity_identity.id,
                property_kind_code: name_property.code,
                program_point_entity_id: None,
                value: PropertyValue::Text(mir.name.clone()),
                directness_code: Directness::Direct as i16,
                certainty_code: EvidenceCertainty::CompilerExact as i16,
                resolution_code: ResolutionClass::NotApplicable as i16,
                producer_code: provider_code,
                derivation_code: None,
                file_id: None,
                start_byte: None,
                end_byte: None,
                fact_hash64: digest_hash64(property_identity.full_digest),
            };
            let entity_form = crate::registries::fact_kind_code("ENTITY_EXISTENCE")
                .and_then(|code| i16::try_from(code).ok())
                .ok_or_else(|| FactIngestError::Protocol("entity fact form is absent".into()))?;
            let property_form = crate::registries::fact_kind_code("PROPERTY")
                .and_then(|code| i16::try_from(code).ok())
                .ok_or_else(|| FactIngestError::Protocol("property fact form is absent".into()))?;
            let evidence = vec![
                FactEvidenceRow {
                    evidence_id: crate::identity::fact_evidence_id(
                        provider_run_id,
                        entity_observation,
                        entity_identity.id,
                    ),
                    scope,
                    fact_id: entity_identity.id,
                    fact_form_code: entity_form,
                    provider_code,
                    provider_version: format!(
                        "{}+{}",
                        compilation.begin.rustc_version, compilation.begin.rustc_commit
                    ),
                    provider_run_id,
                    observation_id: entity_observation,
                    raw_kind_code: None,
                    file_id: None,
                    start_byte: None,
                    end_byte: None,
                    certainty_code: EvidenceCertainty::CompilerExact as i16,
                    resolution_code: ResolutionClass::NotApplicable as i16,
                    conflict_disposition_code: 10,
                    cold_payload: Some(mir.arrow_ipc.clone()),
                },
                FactEvidenceRow {
                    evidence_id: crate::identity::fact_evidence_id(
                        provider_run_id,
                        property_observation,
                        property_identity.id,
                    ),
                    scope,
                    fact_id: property_identity.id,
                    fact_form_code: property_form,
                    provider_code,
                    provider_version: format!(
                        "{}+{}",
                        compilation.begin.rustc_version, compilation.begin.rustc_commit
                    ),
                    provider_run_id,
                    observation_id: property_observation,
                    raw_kind_code: None,
                    file_id: None,
                    start_byte: None,
                    end_byte: None,
                    certainty_code: EvidenceCertainty::CompilerExact as i16,
                    resolution_code: ResolutionClass::NotApplicable as i16,
                    conflict_disposition_code: 10,
                    cold_payload: None,
                },
            ];
            let owner_batch = encode_owners(&[OwnerRow {
                scope,
                parent_owner_id: None,
                owner_kind_code: OwnerKind::MirBody as i16,
                language: Language::Rust as i16,
                file_id: None,
                semantic_entity_id: Some(entity_identity.id),
                start_byte: None,
                end_byte: None,
                source_fingerprint: None,
                semantic_fingerprint: Some(digest32(&owner.end.owner_content_digest)?),
                capability_mask: capability_bits,
            }])?;
            let capability_batch = encode_capability_statuses(&[CapabilityStatusRow {
                scope,
                snapshot_id: None,
                capability_code: capability,
                owner_capability_state_code: OwnerCapabilityState::Current as i16,
                completeness_state_code: Completeness::Complete as i16,
                provider_run_id: Some(provider_run_id),
                producer_code: Some(provider_code),
                reason_code: None,
                diagnostic_id: None,
                fallback_source_available: true,
                coverage_scope_fingerprint: coverage_fingerprint(scope, &mir.chunk_digest),
            }])?;
            let batches = vec![
                ProviderFactBatch {
                    table_code: 8,
                    batch: owner_batch,
                },
                ProviderFactBatch {
                    table_code: 9,
                    batch: capability_batch,
                },
                ProviderFactBatch {
                    table_code: 100,
                    batch: encode_entities(&[entity])?,
                },
                ProviderFactBatch {
                    table_code: 110,
                    batch: encode_relations(&[])?,
                },
                ProviderFactBatch {
                    table_code: 120,
                    batch: encode_properties(&[property])?,
                },
                ProviderFactBatch {
                    table_code: 130,
                    batch: encode_evidence(&evidence)?,
                },
            ];
            let stream = ProviderFactStream {
                manifest: ProviderFactManifest {
                    stream_id: digest_id16(&owner.end.owner_content_digest)?,
                    workspace_id: scope.workspace_id,
                    analysis_context_id: scope.analysis_context_id,
                    source_generation,
                    provider_code,
                    provider_version: format!(
                        "{}+{}",
                        compilation.begin.rustc_version, compilation.begin.rustc_commit
                    ),
                    provider_run_id,
                    emitted_at_micros: 0,
                    schema_fingerprints: batches
                        .iter()
                        .map(|batch| {
                            required_table_digest(batch.table_code)
                                .map(|digest| (batch.table_code, digest))
                        })
                        .collect::<Result<BTreeMap<_, _>, _>>()?,
                    declared_rows: 6,
                },
                batches,
                terminal: StreamTerminal::Completed,
            };
            let output = self.reconciliation.ingest(scope, &[stream], &precedence)?;
            outputs.push(output);
        }
        Ok(outputs)
    }

    /// Atomically publish all validated canonical batches through the generated
    /// owner-replacement policy and durable publication protocol.
    ///
    /// The canonical result is consumed so callers cannot accidentally publish a
    /// partially reused batch set after a successful commit.
    ///
    /// # Errors
    ///
    /// Rejects missing generated tables, scope/pin drift, mutation conflicts, or
    /// publication validation and pointer-CAS failures.
    pub async fn publish_canonical<J: MutationJournal>(
        &self,
        fabric: &mut WorkspaceFabric,
        journal: &mut J,
        request: &PublicationRequest,
        canonical: CanonicalIngestOutput,
    ) -> Result<PublicationOutcome, CoreFactError> {
        self.publish_canonical_set(fabric, journal, request, vec![canonical])
            .await
    }

    /// Atomically publish canonical outputs for multiple independent owners.
    ///
    /// # Errors
    ///
    /// Rejects duplicate owner/table replacements, missing generated tables, scope/pin drift,
    /// mutation conflicts, or publication validation and pointer-CAS failures.
    pub async fn publish_canonical_set<J: MutationJournal>(
        &self,
        fabric: &mut WorkspaceFabric,
        journal: &mut J,
        request: &PublicationRequest,
        canonicals: Vec<CanonicalIngestOutput>,
    ) -> Result<PublicationOutcome, CoreFactError> {
        let capacity = canonicals
            .iter()
            .map(|canonical| canonical.batches.len())
            .sum();
        let mut owner_tables = BTreeSet::new();
        let mut writes = Vec::with_capacity(capacity);
        for canonical in canonicals {
            for (table_code, batch) in canonical.batches {
                let expected_predecessor = fabric
                    .table(table_code)
                    .ok_or(CoreFactError::MissingFactTable(table_code))?
                    .version();
                let scope = batch.scope();
                if !owner_tables.insert((table_code, scope.owner_id)) {
                    return Err(CoreFactError::DuplicateOwnerTable {
                        table_code,
                        owner_id: scope.owner_id,
                    });
                }
                writes.push(OwnerPublicationWrite {
                    request: OwnerMutationRequest {
                        scope: scope.batch_scope(),
                        publication_id: request.pins.publication_id,
                        operation_id: request.operation_id,
                        table_code,
                        owner_ids: vec![scope.owner_id],
                        expected_predecessor,
                    },
                    batch,
                });
            }
        }
        writes.sort_by_key(|write| (write.request.table_code, write.request.owner_ids[0]));
        Ok(fabric.publish(journal, request, &writes).await?)
    }

    /// Freeze one durable publication into the exact DataFusion/Delta provider graph
    /// and bind it to an immutable serving-snapshot manifest.
    ///
    /// # Errors
    ///
    /// Rejects incomplete publication census, unresolved Delta versions, provider
    /// schema/content drift, or manifest identity/scope mismatch.
    pub async fn freeze_publication(
        &self,
        publication: &PublicationOutcome,
        body: ServingSnapshotManifestBody,
        source_blob_digests: &[[u8; 32]],
    ) -> Result<ServingSnapshotCandidate, CoreFactError> {
        self.freeze_publication_with_overlay(
            publication,
            &EmptySnapshotOverlay,
            body,
            source_blob_digests,
        )
        .await
    }

    /// Freeze one durable publication together with one immutable hot-overlay generation.
    ///
    /// # Errors
    ///
    /// Rejects incomplete publication census, unresolved Delta versions, overlay policy/schema
    /// drift, provider schema/content drift, or manifest identity/scope mismatch.
    pub async fn freeze_publication_with_overlay<O: SnapshotOverlayProviderFactory>(
        &self,
        publication: &PublicationOutcome,
        overlay: &O,
        body: ServingSnapshotManifestBody,
        source_blob_digests: &[[u8; 32]],
    ) -> Result<ServingSnapshotCandidate, CoreFactError> {
        let providers = Arc::new(SnapshotProviderCatalog::build(publication, overlay).await?);
        Ok(ServingSnapshotCandidate::build(
            body,
            providers,
            source_blob_digests,
        )?)
    }
}

#[derive(Debug)]
struct RustcMirObservation {
    protocol_owner_id: String,
    name: String,
    arrow_ipc: Vec<u8>,
    chunk_digest: String,
}

fn decode_rustc_owner(owner: &AcceptedRustcOwner) -> Result<RustcMirObservation, FactIngestError> {
    if owner.chunks.len() != 1 {
        return Err(FactIngestError::Protocol(
            "WP35 MIR owner must contain exactly one Arrow chunk".into(),
        ));
    }
    let protocol_owner_id = owner
        .begin
        .owner
        .as_ref()
        .map(|key| key.owner_id.clone())
        .ok_or_else(|| FactIngestError::Protocol("compiler owner key is absent".into()))?;
    let chunk = &owner.chunks[0];
    let contract = rustc_mir_observation_contract()?;
    if chunk.observation_family_code != u32::from(ProviderObservationFamily::RustMirOwner as u16)
        || chunk.schema_digest != contract.schema_digest
        || chunk.chunk_digest != crate::integrity::framed_digest(&chunk.arrow_ipc)
        || chunk.payload_reference.is_some()
        || chunk.arrow_ipc.is_empty()
    {
        return Err(FactIngestError::Protocol(
            "WP35 MIR chunk profile differs".into(),
        ));
    }
    let expected_schema = Arc::new(provider_observation_arrow_schema(contract));
    let batches = decode_validated_arrow_ipc_chunks(
        Arc::clone(&expected_schema),
        usize::try_from(chunk.row_count)
            .map_err(|_| FactIngestError::Protocol("MIR row count exceeds usize".into()))?,
        chunk.arrow_ipc.len(),
        [chunk.arrow_ipc.as_slice()],
    )?;
    let mut rows = Vec::new();
    for batch in batches {
        let names = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| FactIngestError::Protocol("MIR name column is not UTF-8".into()))?;
        let item_kinds = required_string_column(&batch, 1, "item_kind")?;
        let type_descriptions = required_string_column(&batch, 2, "type_description")?;
        let monomorphization = batch
            .column(3)
            .as_any()
            .downcast_ref::<BooleanArray>()
            .ok_or_else(|| {
                FactIngestError::Protocol("MIR monomorphization column differs".into())
            })?;
        let basic_blocks = required_u64_column(&batch, 4, "basic_block_count")?;
        let locals = required_u64_column(&batch, 5, "local_count")?;
        let statements = required_string_list_column(&batch, 6, "statement_kinds")?;
        let terminators = required_string_list_column(&batch, 7, "terminator_kinds")?;
        let successors = required_u64_column(&batch, 8, "successor_count")?;
        for row in 0..batch.num_rows() {
            if names.is_null(row)
                || item_kinds.is_null(row)
                || type_descriptions.is_null(row)
                || monomorphization.is_null(row)
                || basic_blocks.is_null(row)
                || locals.is_null(row)
                || statements.is_null(row)
                || terminators.is_null(row)
                || successors.is_null(row)
            {
                return Err(FactIngestError::Protocol(
                    "WP35 MIR Arrow row contains nulls".into(),
                ));
            }
            validate_string_list(statements, row)?;
            validate_string_list(terminators, row)?;
            rows.push(names.value(row).to_owned());
        }
    }
    if rows.len() != 1 || chunk.row_count != 1 || rows[0].is_empty() {
        return Err(FactIngestError::Protocol(
            "WP35 MIR owner row census differs".into(),
        ));
    }
    Ok(RustcMirObservation {
        protocol_owner_id,
        name: rows.remove(0),
        arrow_ipc: chunk.arrow_ipc.clone(),
        chunk_digest: chunk.chunk_digest.clone(),
    })
}

fn rustc_mir_observation_contract() -> Result<&'static ProviderObservationSchema, FactIngestError> {
    PROVIDER_OBSERVATION_SCHEMAS
        .iter()
        .find(|schema| schema.provider_id == "rustc-mir")
        .ok_or_else(|| FactIngestError::Protocol("rustc MIR observation schema is absent".into()))
}

fn provider_observation_arrow_schema(contract: &ProviderObservationSchema) -> Schema {
    Schema::new(
        contract
            .fields
            .iter()
            .map(|field| {
                let data_type = match field.logical_type {
                    ProviderObservationLogicalType::Utf8 => DataType::Utf8,
                    ProviderObservationLogicalType::Boolean => DataType::Boolean,
                    ProviderObservationLogicalType::UInt64 => DataType::UInt64,
                    ProviderObservationLogicalType::Utf8List => {
                        DataType::List(Arc::new(Field::new_list_field(DataType::Utf8, false)))
                    }
                };
                Field::new(field.name, data_type, field.nullable)
            })
            .collect::<Vec<_>>(),
    )
}

fn required_string_column<'a>(
    batch: &'a arrow_array::RecordBatch,
    index: usize,
    name: &str,
) -> Result<&'a StringArray, FactIngestError> {
    batch
        .column(index)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| FactIngestError::Protocol(format!("MIR {name} column differs")))
}

fn required_u64_column<'a>(
    batch: &'a arrow_array::RecordBatch,
    index: usize,
    name: &str,
) -> Result<&'a UInt64Array, FactIngestError> {
    batch
        .column(index)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .ok_or_else(|| FactIngestError::Protocol(format!("MIR {name} column differs")))
}

fn required_string_list_column<'a>(
    batch: &'a arrow_array::RecordBatch,
    index: usize,
    name: &str,
) -> Result<&'a ListArray, FactIngestError> {
    batch
        .column(index)
        .as_any()
        .downcast_ref::<ListArray>()
        .ok_or_else(|| FactIngestError::Protocol(format!("MIR {name} column differs")))
}

fn validate_string_list(column: &ListArray, row: usize) -> Result<(), FactIngestError> {
    let values = column.value(row);
    let strings = values
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| FactIngestError::Protocol("MIR list member type differs".into()))?;
    if strings.null_count() != 0 {
        return Err(FactIngestError::Protocol(
            "MIR list contains a null member".into(),
        ));
    }
    Ok(())
}

fn required_table_digest(table_code: i16) -> Result<String, FactIngestError> {
    table_spec(table_code)
        .map(|spec| spec.schema_digest.clone())
        .ok_or_else(|| FactIngestError::Protocol(format!("table {table_code} is absent")))
}

fn semantic_key(domain: crate::identity::SemanticFingerprintDomain, fields: &[&[u8]]) -> Vec<u8> {
    let mut hasher = crate::identity::semantic_fingerprint(domain);
    for field in fields {
        hasher.update(&(field.len() as u64).to_be_bytes());
        hasher.update(field);
    }
    hasher.finalize().to_vec()
}

fn stable_id16(bytes: &[u8]) -> [u8; 16] {
    crate::identity::unframed_semantic_id(bytes)
}

fn digest32(value: &str) -> Result<[u8; 32], FactIngestError> {
    let payload = value
        .strip_prefix("b3:")
        .ok_or_else(|| FactIngestError::Protocol("compiler digest framing is invalid".into()))?;
    if payload.len() != 64 {
        return Err(FactIngestError::Protocol(
            "compiler digest width is invalid".into(),
        ));
    }
    let mut decoded = [0_u8; 32];
    for (index, pair) in payload.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let high = char::from(pair[0])
            .to_digit(16)
            .ok_or_else(|| FactIngestError::Protocol("compiler digest is invalid".into()))?;
        let low = char::from(pair[1])
            .to_digit(16)
            .ok_or_else(|| FactIngestError::Protocol("compiler digest is invalid".into()))?;
        decoded[index] = u8::try_from((high << 4) | low)
            .map_err(|_| FactIngestError::Protocol("compiler digest is invalid".into()))?;
    }
    Ok(decoded)
}

fn digest_id16(value: &str) -> Result<[u8; 16], FactIngestError> {
    let digest = digest32(value)?;
    let mut id = [0; 16];
    id.copy_from_slice(&digest[..16]);
    Ok(id)
}

const fn digest_hash64(digest: [u8; 32]) -> i64 {
    i64::from_be_bytes([
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
    ])
}

fn coverage_fingerprint(scope: FactScope, chunk_digest: &str) -> [u8; 32] {
    crate::identity::rustc_capability_scope_fingerprint(
        scope.workspace_id,
        scope.analysis_context_id,
        scope.source_generation,
        scope.owner_id,
        chunk_digest,
    )
}

/// Stable Wave-4 boundary errors.
#[derive(Debug, Error)]
pub enum CoreFactError {
    #[error("SOURCE_ADMISSION_POLICY:{0}")]
    Policy(String),
    #[error("UNKNOWN_CAPABILITY:{0}")]
    UnknownCapability(String),
    #[error("UNKNOWN_EVIDENCE_UNREGISTERED")]
    UnknownEvidence,
    #[error("PROVIDER_RAW_KIND_INVENTORY_INVALID")]
    ProviderInventory,
    #[error("PROVIDER_TEXT_UNAVAILABLE")]
    ProviderTextUnavailable,
    #[error("UNSUPPORTED_SOURCE_LANGUAGE")]
    UnsupportedLanguage,
    #[error("FACT_TABLE_MISSING:{0}")]
    MissingFactTable(i16),
    #[error("DUPLICATE_OWNER_TABLE:{table_code}:{owner_id:?}")]
    DuplicateOwnerTable { table_code: i16, owner_id: [u8; 16] },
    #[error(transparent)]
    TreeSitter(#[from] TreeSitterAdapterError),
    #[error(transparent)]
    Ruff(#[from] RuffAdapterError),
    #[error(transparent)]
    Fabric(#[from] FabricError),
    #[error(transparent)]
    Snapshot(#[from] SnapshotRuntimeError),
    #[error(transparent)]
    Ingest(#[from] FactIngestError),
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::ipc::writer::StreamWriter;
    use arrow_array::builder::{ListBuilder, StringBuilder};
    use arrow_array::{ArrayRef, BooleanArray, RecordBatch, StringArray, UInt64Array};

    use super::*;
    use crate::fabric::batch_checksum;
    use crate::identity::{
        CaseSensitivityMode, IdentityDomain, PlatformCode, SOURCE_CONTEXT_ID, WorkspacePath,
        context_set_identity, encode_public_id, source_file_identity,
    };
    use crate::operational_store::OperationalStore;
    use crate::provider_types::ProviderText;
    use crate::registries::{ProviderRunState, WorkspaceRegistryLifecycle};
    use crate::rpc::generated::codefabric::rustc::v1::{
        CompilationBegin, CompilationEnd, CompilerOwnerKey, DiagnosticSummary, OwnerBegin,
        OwnerEnd, OwnerObservationChunk, PackageTargetIdentity,
    };
    use crate::rustc_service::{AcceptedRustcCompilation, AcceptedRustcOwner, RustcRunAdmission};
    use crate::secure_path::StableFileMetadata;
    use crate::snapshot::{
        SnapshotBasePublication, SnapshotBundles, SnapshotContextRecord, SnapshotContexts,
        SnapshotIndexes, SnapshotOverlay, SnapshotSource,
    };
    use crate::source_image::{
        BlobReference, LineIndex, NewlineKind, SourceBlobLease, SourceEncoding, SourceFileKind,
    };
    use crate::workspace_registry::WorkspaceRecord;

    fn input(path: &[&[u8]], bytes: &[u8]) -> AdmissionInput {
        AdmissionInput {
            path_components: path.iter().map(|part| part.to_vec()).collect(),
            bytes: bytes.to_vec(),
            explicit_language: None,
            recognized_encoding: true,
            explicitly_authorized_large_file: false,
            generated_capture: false,
            vendored_body_authorized: false,
            required_configuration: false,
        }
    }

    fn source_image(text: &str) -> SourceImage {
        let workspace_id = [1; 16];
        let path = WorkspacePath::from_components(
            workspace_id,
            PlatformCode::Unix,
            CaseSensitivityMode::Sensitive,
            &[b"pkg".to_vec(), b"sample.py".to_vec()],
        )
        .unwrap();
        let bytes = text.as_bytes().to_vec();
        let digest = crate::integrity::digest_bytes(&bytes);
        let offsets = text
            .char_indices()
            .map(|(offset, _)| u64::try_from(offset).unwrap())
            .chain(std::iter::once(u64::try_from(text.len()).unwrap()))
            .collect::<Vec<_>>();
        let line_offsets = std::iter::once(0_u64)
            .chain(
                bytes
                    .iter()
                    .enumerate()
                    .filter(|(_, byte)| **byte == b'\n')
                    .map(|(index, _)| u64::try_from(index + 1).unwrap()),
            )
            .collect::<Vec<_>>();
        let serialized = line_offsets
            .iter()
            .flat_map(|offset| offset.to_le_bytes())
            .collect::<Vec<_>>();
        SourceImage {
            workspace_id,
            worktree_id: None,
            source_generation: 7,
            file_id: source_file_identity(&path).unwrap().id,
            path,
            language: SourceLanguage::Python,
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
            provider_text: Some(ProviderText {
                text: Arc::from(text),
                original_byte_offsets: Arc::from(offsets),
            }),
            line_index: LineIndex {
                offsets: Arc::from(line_offsets),
                serialized: Arc::from(serialized.clone()),
                digest: crate::integrity::digest_bytes(&serialized),
                format_version: 1,
                newline_kind: NewlineKind::Lf,
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

    fn workspace_record() -> WorkspaceRecord {
        WorkspaceRecord {
            workspace_id: [1; 16],
            workspace_registration_nonce: [2; 16],
            registration_revision: 1,
            administrative_key: vec![3],
            root_path_bytes: b"/workspace".to_vec(),
            root_path_display: "/workspace".into(),
            root_directory_file_identity: vec![4],
            platform_code: 2,
            case_sensitivity_mode: "sensitive".into(),
            authorization_revision: 1,
            allowed_source_disclosure_rules: Vec::new(),
            repository_id: None,
            worktree_id: None,
            authorization_fingerprint: [5; 32],
            context_fingerprint: [6; 32],
            status: WorkspaceRegistryLifecycle::Bootstrapping,
            created_at: "00000000000000001000".into(),
            updated_at: "00000000000000001000".into(),
        }
    }

    fn framed(byte: u8) -> String {
        use std::fmt::Write as _;

        let mut value = String::from("b3:");
        for byte in [byte; 32] {
            write!(value, "{byte:02x}").unwrap();
        }
        value
    }

    fn rustc_string_list(values: &[&str]) -> ArrayRef {
        let mut builder = ListBuilder::new(StringBuilder::new())
            .with_field(Field::new_list_field(DataType::Utf8, false));
        for value in values {
            builder.values().append_value(value);
        }
        builder.append(true);
        Arc::new(builder.finish())
    }

    fn rustc_mir_ipc(name: &str) -> Vec<u8> {
        let contract = rustc_mir_observation_contract().unwrap();
        let schema = Arc::new(provider_observation_arrow_schema(contract));
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![
                Arc::new(StringArray::from(vec![name])) as ArrayRef,
                Arc::new(StringArray::from(vec!["fn"])) as ArrayRef,
                Arc::new(StringArray::from(vec!["fn() -> ()"])) as ArrayRef,
                Arc::new(BooleanArray::from(vec![false])) as ArrayRef,
                Arc::new(UInt64Array::from(vec![1])) as ArrayRef,
                Arc::new(UInt64Array::from(vec![2])) as ArrayRef,
                rustc_string_list(&["Assign"]),
                rustc_string_list(&["Return"]),
                Arc::new(UInt64Array::from(vec![0])) as ArrayRef,
            ],
        )
        .unwrap();
        let mut bytes = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut bytes, &schema).unwrap();
            writer.write(&batch).unwrap();
            writer.finish().unwrap();
        }
        bytes
    }

    fn accepted_rustc_compilation(name: &str) -> AcceptedRustcCompilation {
        let provider_run_id = "run:wp36".to_owned();
        let compilation_unit_id = "unit:wp36".to_owned();
        let arrow_ipc = rustc_mir_ipc(name);
        let contract = rustc_mir_observation_contract().unwrap();
        let family = u32::from(contract.observation_family_code);
        AcceptedRustcCompilation {
            admission: RustcRunAdmission {
                provider_run_id: provider_run_id.clone(),
                workspace_id: "workspace:wp36".to_owned(),
                analysis_context_id: "context:wp36".to_owned(),
                canonical_workspace_id: [0x36; 16],
                canonical_analysis_context_id: [0x37; 16],
                source_generation: 7,
                context_manifest_digest: framed(1),
                source_snapshot_manifest_digest: framed(2),
                resource_profile_id: "compiler-semantic-standard".to_owned(),
            },
            begin: CompilationBegin {
                provider_run_id: provider_run_id.clone(),
                compilation_unit_id: compilation_unit_id.clone(),
                workspace_id: "workspace:wp36".to_owned(),
                analysis_context_id: "context:wp36".to_owned(),
                source_generation: 7,
                target: Some(PackageTargetIdentity {
                    package_id: "pkg:wp36".to_owned(),
                    package_name: "fixture".to_owned(),
                    target_name: "fixture".to_owned(),
                    target_kind: "lib".to_owned(),
                    crate_name: "fixture".to_owned(),
                    crate_type: "lib".to_owned(),
                    crate_disambiguator: "wp36".to_owned(),
                }),
                rustc_version: "1.100.0-nightly".to_owned(),
                rustc_commit: "8fa1c96cfd489e4c27654c144ae871ce2c4db6c6".to_owned(),
                normalized_rustc_invocation_digest: framed(3),
                cargo_metadata_digest: framed(4),
                cargo_lock_digest: framed(5),
                cargo_config_digest: framed(6),
                build_script_output_digests: Vec::new(),
                proc_macro_output_digests: Vec::new(),
                source_snapshot_manifest_digest: framed(2),
                requested_capability_codes: vec![u32::from(capability_code("RUST_MIR").unwrap())],
                context_manifest_digest: framed(1),
                resource_profile_id: "compiler-semantic-standard".to_owned(),
                toolchain_identity_digest: framed(7),
            },
            owners: vec![AcceptedRustcOwner {
                begin: OwnerBegin {
                    provider_run_id: provider_run_id.clone(),
                    compilation_unit_id: compilation_unit_id.clone(),
                    sequence: 1,
                    owner: Some(CompilerOwnerKey {
                        owner_id: "owner:wp36".to_owned(),
                        owner_kind: "MIR_BODY".to_owned(),
                        file_id: "file:wp36".to_owned(),
                        source_start: 0,
                        source_end: 16,
                    }),
                    expected_observation_family_codes: vec![family],
                },
                chunks: vec![OwnerObservationChunk {
                    provider_run_id: provider_run_id.clone(),
                    compilation_unit_id: compilation_unit_id.clone(),
                    sequence: 2,
                    owner_id: "owner:wp36".to_owned(),
                    observation_family_code: family,
                    chunk_digest: crate::integrity::framed_digest(&arrow_ipc),
                    arrow_ipc,
                    payload_reference: None,
                    schema_digest: contract.schema_digest.to_owned(),
                    row_count: 1,
                }],
                end: OwnerEnd {
                    provider_run_id: provider_run_id.clone(),
                    compilation_unit_id: compilation_unit_id.clone(),
                    sequence: 3,
                    owner_id: "owner:wp36".to_owned(),
                    family_counts: [(family, 1)].into_iter().collect(),
                    owner_content_digest: framed(8),
                },
            }],
            end: CompilationEnd {
                provider_run_id,
                compilation_unit_id,
                sequence: 4,
                compiler_exit_status: 0,
                closed_owner_set_digest: framed(9),
                capability_outcomes: Vec::new(),
                diagnostic_summary: Some(DiagnosticSummary {
                    error_count: 0,
                    warning_count: 0,
                    diagnostics_digest: framed(10),
                }),
                overall_stream_digest: framed(11),
                terminal_state: ProviderRunState::Succeeded as i32,
                rejection_error: None,
            },
        }
    }

    fn snapshot_body() -> ServingSnapshotManifestBody {
        let contexts = vec![SOURCE_CONTEXT_ID];
        ServingSnapshotManifestBody {
            manifest_version: "1.0".into(),
            workspace_id: encode_public_id(IdentityDomain::Workspace, None, [1; 16]).unwrap(),
            repository_id: None,
            worktree_id: None,
            registration_revision: 1,
            source: SnapshotSource {
                source_generation: 7,
                admitted_event_sequence: 7,
                reconciled_event_sequence: 7,
                inventory_digest: framed(1),
                authorization_fingerprint: framed(2),
                inclusion_policy_fingerprint: framed(3),
                path_profile_version: "1".into(),
                source_trust_state: "CURRENT_BYTES_VERIFIED".into(),
                event_stream_health: "HEALTHY".into(),
                git_acceleration_status: "NOT_REQUIRED".into(),
                git_state_fingerprint: None,
            },
            contexts: SnapshotContexts {
                context_set_id: encode_public_id(
                    IdentityDomain::ContextSet,
                    None,
                    context_set_identity([1; 16], &contexts).unwrap().id,
                )
                .unwrap(),
                default_python_context_id: Some(
                    encode_public_id(IdentityDomain::AnalysisContext, None, SOURCE_CONTEXT_ID)
                        .unwrap(),
                ),
                default_rust_context_id: None,
                records: vec![SnapshotContextRecord {
                    analysis_context_id: encode_public_id(
                        IdentityDomain::AnalysisContext,
                        None,
                        SOURCE_CONTEXT_ID,
                    )
                    .unwrap(),
                    context_manifest_digest: framed(4),
                    capability_partition_digest: framed(5),
                }],
            },
            base_publication: SnapshotBasePublication {
                publication_id: String::new(),
                tables: Vec::new(),
            },
            overlay: SnapshotOverlay {
                overlay_generation: 0,
                overlay_digest: framed(0),
                total_memory_bytes: 0,
                tables: Vec::new(),
            },
            indexes: SnapshotIndexes {
                capability_index_digest: framed(6),
                diagnostic_index_digest: framed(7),
                dependency_graph_digest: framed(8),
            },
            bundles: SnapshotBundles {
                ontology_bundle_id: "ontology:1.3".into(),
                schema_bundle_id: "schema:1.0".into(),
                provider_bundle_id: "provider:1.0".into(),
                derivation_bundle_id: "derivation:1.0".into(),
                query_language_bundle_id: "query:1.0".into(),
                model_pack_bundle_id: "model:1.0".into(),
                toolchain_bundle_id: "toolchain:1.0".into(),
            },
            limits_profile_digest: framed(9),
        }
    }

    #[test]
    fn wp36_compiler_arrow_reconciles_into_validated_canonical_batches() {
        let engine = CoreFactEngine::default();
        let compilation = accepted_rustc_compilation("fixture::answer");
        let first = engine.reconcile_rustc_compilation(&compilation).unwrap();
        let second = engine.reconcile_rustc_compilation(&compilation).unwrap();
        assert_eq!(first.len(), 1);
        let expected_rows =
            BTreeMap::from([(8, 1), (9, 1), (100, 1), (110, 0), (120, 1), (130, 2)]);
        assert_eq!(
            first[0]
                .batches
                .iter()
                .map(|(code, batch)| (*code, batch.batch().num_rows()))
                .collect::<BTreeMap<_, _>>(),
            expected_rows
        );
        assert_eq!(
            first[0]
                .batches
                .iter()
                .map(|(code, batch)| (*code, batch_checksum(batch.batch()).unwrap()))
                .collect::<BTreeMap<_, _>>(),
            second[0]
                .batches
                .iter()
                .map(|(code, batch)| (*code, batch_checksum(batch.batch()).unwrap()))
                .collect()
        );
    }

    #[test]
    fn wp36_negative_zero_state() {
        let engine = CoreFactEngine::default();
        let mut wrong_schema = accepted_rustc_compilation("fixture::answer");
        wrong_schema.owners[0].chunks[0].schema_digest = framed(0xee);
        assert!(engine.reconcile_rustc_compilation(&wrong_schema).is_err());

        let mut changed_payload = accepted_rustc_compilation("fixture::answer");
        changed_payload.owners[0].chunks[0].arrow_ipc.push(0);
        assert!(
            engine
                .reconcile_rustc_compilation(&changed_payload)
                .is_err()
        );
    }

    #[test]
    fn wp33_behavioral_acceptance() {
        let policy = SourceAdmissionPolicy::local_workstation().unwrap();
        assert_eq!(
            policy.classify(&input(&[b"src", b"lib.rs"], b"pub fn f() {}\n")),
            AdmissionDecision {
                disposition: AdmissionDisposition::Analyze,
                language: Some(SourceLanguage::Rust),
                reason_code: "SUPPORTED_SOURCE",
                diagnostics: Vec::new(),
                observed_bytes: 14,
            }
        );
        assert_eq!(
            policy
                .classify(&input(&[b"assets", b"image.png"], b"\x89PNG\r\n\x1a\n"))
                .reason_code,
            "UNSUPPORTED_BINARY"
        );
        assert_eq!(
            policy
                .classify(&input(&[b"third_party", b"lib.rs"], b"fn f() {}"))
                .disposition,
            AdmissionDisposition::EndpointOnly
        );
        assert_eq!(
            policy
                .classify(&input(&[b"generated", b"bindings.rs"], b"fn f() {}"))
                .disposition,
            AdmissionDisposition::GeneratedCaptureRequired
        );
        assert_eq!(
            policy
                .classify(&input(&[b".git", b"config"], b"[core]\n"))
                .reason_code,
            "EXCLUDED_POLICY"
        );
        assert_eq!(
            policy
                .classify(&input(&[b"notes.txt"], b"unknown source\n"))
                .reason_code,
            "UNSUPPORTED_CONTENT"
        );
        let oversized = vec![b'x'; usize::try_from(policy.ordinary_maximum_bytes).unwrap() + 1];
        assert_eq!(
            policy
                .classify(&input(&[b"src", b"large.rs"], &oversized))
                .reason_code,
            "SOURCE_ORDINARY_LIMIT_EXCEEDED"
        );
    }

    #[test]
    fn wp33_negative_zero_state() {
        assert_eq!(aggregate_capability(&[]), Completeness::NotApplicable);
        assert_eq!(
            aggregate_capability(&[CapabilityChild {
                applicable: true,
                completeness: Completeness::Partial,
                has_facts: true,
                missing_remainder_characterized: false,
                required_context_covered: true,
                external_policy_allows_closure: true,
            }]),
            Completeness::Indeterminate
        );
        assert!(UnknownEvidence::new("UNKNOWN_SYMBOL", "PROVIDER_UNAVAILABLE", None).is_ok());
        assert!(UnknownEvidence::new("UNKNOWN_SYMBOL", "NOT_REGISTERED", None).is_err());
    }

    #[test]
    fn wp33_structural_acceptance() {
        let census = provider_disposition_census().unwrap();
        assert!(census.normalized > 0);
        assert!(census.unsupported > 0);
        let total = PROVIDER_GRAMMAR_INVENTORIES
            .iter()
            .map(|inventory| inventory.raw_kinds.len())
            .sum::<usize>()
            + RUFF_PYTHON_FRONTEND.node_kinds.len()
            + RUFF_PYTHON_FRONTEND.token_kinds.len();
        assert_eq!(
            census.normalized + census.ignored + census.unsupported,
            total
        );
    }

    #[test]
    fn wp33_source_lane_selects_providers_once_and_replays_exactly() {
        let source = source_image("def answer(value: int) -> int:\n    return value + 42\n");
        let scope = FactScope {
            workspace_id: source.workspace_id,
            analysis_context_id: SOURCE_CONTEXT_ID,
            source_generation: 7,
            owner_id: source.file_id,
        };
        let runs = SourceSyntaxProviderRuns {
            tree_sitter: [10; 16],
            ruff_python: Some([20; 16]),
        };
        let first = CoreFactEngine::default()
            .analyze_source(scope, &source, runs)
            .unwrap();
        let second = CoreFactEngine::default()
            .analyze_source(scope, &source, runs)
            .unwrap();
        assert!(first.ruff_python.is_some());
        assert_eq!(first.canonical.batches.len(), 10);
        assert_eq!(
            first
                .canonical
                .batches
                .iter()
                .map(|(code, batch)| (*code, batch_checksum(batch.batch()).unwrap()))
                .collect::<BTreeMap<_, _>>(),
            second
                .canonical
                .batches
                .iter()
                .map(|(code, batch)| (*code, batch_checksum(batch.batch()).unwrap()))
                .collect()
        );
    }

    #[tokio::test]
    async fn wp33_operational_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let source = source_image("def answer(value: int) -> int:\n    return value + 42\n");
        let scope = FactScope {
            workspace_id: source.workspace_id,
            analysis_context_id: SOURCE_CONTEXT_ID,
            source_generation: 7,
            owner_id: source.file_id,
        };
        let engine = CoreFactEngine::default();
        let analysis = engine
            .analyze_source(
                scope,
                &source,
                SourceSyntaxProviderRuns {
                    tree_sitter: [10; 16],
                    ruff_python: Some([20; 16]),
                },
            )
            .unwrap();
        let mut fact_digest = crate::integrity::IntegrityHasher::for_domain(
            crate::integrity::IntegrityDomain::Wave4CanonicalState,
        );
        for (table_code, batch) in &analysis.canonical.batches {
            fact_digest.update(&table_code.to_be_bytes());
            fact_digest.update(&batch_checksum(batch.batch()).unwrap());
        }

        let mut fabric =
            crate::fabric::bootstrap_workspace(&root.path().join("fabric"), &workspace_record())
                .await
                .unwrap();
        let mut store = OperationalStore::open(&root.path().join("operational.sqlite")).unwrap();
        let contexts = vec![SOURCE_CONTEXT_ID];
        let request = PublicationRequest {
            operation_id: [0x78; 16],
            pins: crate::fabric::PublicationPins {
                publication_id: [0x77; 16],
                workspace_id: source.workspace_id,
                repository_id: None,
                worktree_id: None,
                source_generation: 7,
                source_inventory_digest: source.digest,
                analysis_context_set_id: context_set_identity(source.workspace_id, &contexts)
                    .unwrap()
                    .id,
                analysis_context_ids: contexts,
                git_state_fingerprint: None,
                inclusion_policy_fingerprint: [0x22; 32],
                base_fact_digest: fact_digest.finalize(),
                derived_fact_digest: None,
                ontology_version: "1.3".into(),
                schema_bundle_version: "1.0".into(),
                provider_bundle_version: "1.0".into(),
                derivation_bundle_version: "1.0".into(),
                toolchain_bundle_version: "1.0".into(),
            },
            expected_pointer: None,
            expected_publication_table_version: fabric.table(5).unwrap().version(),
            expected_manifest_table_version: fabric.table(6).unwrap().version(),
            expected_pointer_table_version: fabric.table(7).unwrap().version(),
            started_at_micros: 1_000,
            completed_at_micros: 1_500,
        };
        let publication = engine
            .publish_canonical(&mut fabric, &mut store, &request, analysis.canonical)
            .await
            .unwrap();
        let candidate = engine
            .freeze_publication(&publication, snapshot_body(), &[source.digest])
            .await
            .unwrap();
        assert_eq!(candidate.providers().publication_id(), [0x77; 16]);
        let context = datafusion::prelude::SessionContext::new();
        let entity_rows = context
            .read_table(candidate.providers().provider(100).unwrap())
            .unwrap()
            .count()
            .await
            .unwrap();
        assert!(entity_rows > 0);
    }

    #[test]
    fn wp36_structural_acceptance() {
        let forbidden = ["Synthetic", "Canonical", "Ingest"].concat();
        let output = std::process::Command::new("rg")
            .args(["--fixed-strings", "--glob", "*.rs", &forbidden, "src"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(1),
            "synthetic ingress remains:\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
        for table_code in [8, 9, 100, 110, 120, 130] {
            assert!(table_spec(table_code).is_some());
        }
    }
}
