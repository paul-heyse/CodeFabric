//! Admission of exact provider-native Arrow relations into a candidate session.
//!
//! Provider adapters expose typed Arrow batches. This module does not reinterpret those rows as
//! canonical facts: it joins observed relations to an independently accepted
//! [`ProviderBoundaryContract`], derives coverage from typed coverage relations, and registers
//! only accepted raw relations. All workspace partitions enter through one consumed
//! [`ProgrammaticFabricEpochBuilder`]. Missing output or coverage is an explicit unknown; schema,
//! pin, authority, or unexpected-output contradictions fail the whole consumed candidate builder.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use std::sync::Arc;

use arrow_array::{
    Array as _, BooleanArray, FixedSizeBinaryArray, RecordBatch, StringArray, UInt64Array,
};
use arrow_ipc::reader::StreamReader;
use arrow_schema::SchemaRef;
use datafusion::common::{DataFusionError, TableReference};
use datafusion::datasource::MemTable;
use thiserror::Error;

use crate::fabric::epoch_runtime::{FABRIC_CATALOG, FabricEpochId, FabricSchemaRole};
use crate::fabric::programmatic_epoch::ProgrammaticFabricEpochBuilder;
use crate::fabric::programmatic_schema::{
    ProgrammaticRelationId, ProgrammaticSchemaAssembly, ProgrammaticSchemaError, ProviderInput,
};
use crate::fabric::proof::ProofRelations;
use crate::provider_boundary::{
    ContractDisposition, InstalledProviderSurface, ProviderApiFamily, ProviderAuthorityRole,
    ProviderBoundaryContract, ProviderBoundaryError, ProviderBoundaryEvidence,
    ProviderBoundaryReport, ProviderFamilyCoverage, ProviderFamilyRequest,
    ProviderFamilyRunOutcome, ProviderHandlerId, ProviderInstallerIdentity,
    evaluate_provider_boundary, validate_provider_boundary_contract,
};
use crate::provider_capability::{
    ProviderCapabilityError, ProviderCapabilityRelation, ProviderOracleProofBinding,
    derive_provider_capability_relation, provider_oracle_proofs_from_executable_relations,
};
use crate::provider_native_syntax::{
    NativeSyntaxRelation, ProviderNativeSyntaxRun, RUFF_COMPONENT_RELEASE,
    TREE_SITTER_PYTHON_GRAMMAR_RELEASE, TREE_SITTER_RUNTIME_RELEASE,
};
use crate::pyrefly_service::{AcceptedPyreflyModule, AcceptedPyreflyRun, PyreflyRelation};
use crate::relation_ipc::{
    AssembledRelation, ContextPin, CoverageRemainder, CoverageScope, CoverageTrailer, RelationId,
    RemainderReason, SourcePin, StreamId, StreamIdentity, TerminalStatus,
};
use crate::rustc_relation_schema::RustcRelation;
use crate::rustc_service::{
    AcceptedRustcCompilation, AcceptedRustcOwner, AcceptedRustcRelation,
    TrustQualifiedRustcCompilation, arrow_ipc_digest,
};
use crate::schema_contract::{FieldIndexMapping, SchemaContract, SchemaContractError, SchemaRole};

const MAX_ADMISSION_BINDINGS: usize = 4_096;
const MAX_PROVIDER_WORKSPACE_PARTITIONS: usize = 4_096;
const MAX_RELATION_NAME_BYTES: usize = 512;

/// Provider-emitted relation name used only to join output to application-owned bindings.
///
/// This is deliberately open data rather than a generated target relation registry.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProviderRelationIdentity(Arc<str>);

impl ProviderRelationIdentity {
    /// Construct a bounded provider relation identity.
    ///
    /// # Errors
    ///
    /// Rejects empty, surrounding-whitespace, control-character, or oversized identities.
    pub fn try_new(value: impl Into<Arc<str>>) -> Result<Self, ProviderAdmissionError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_RELATION_NAME_BYTES
            || value.trim() != value.as_ref()
            || value.chars().any(char::is_control)
        {
            return Err(ProviderAdmissionError::InvalidPlan(
                "provider relation identity is invalid".into(),
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Exact provider lane that produced one observed raw relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderNativeLane {
    TreeSitter,
    Ruff,
    Pyrefly,
    Rustc,
}

impl ProviderNativeLane {
    const fn raw_role(self) -> FabricSchemaRole {
        match self {
            Self::TreeSitter => FabricSchemaRole::RawTreeSitter,
            Self::Ruff => FabricSchemaRole::RawRuff,
            Self::Pyrefly => FabricSchemaRole::RawPyrefly,
            Self::Rustc => FabricSchemaRole::RawRustc,
        }
    }
}

/// Application classification used to prevent an analysis result from impersonating provider output.
///
/// These are semantic classes, not relation identifiers. The model remains authoritative for
/// assigning a relation to a class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderAuthorityClass {
    ProviderNative,
    PythonCfg,
    PythonDataflow,
    PythonAlias,
    PythonEffect,
    PythonSummary,
    RustApplicationDerived,
}

impl ProviderAuthorityClass {
    const fn forbids_provider_native(self) -> bool {
        !matches!(self, Self::ProviderNative)
    }
}

/// Typed column routing for one provider-declared coverage relation.
///
/// Different exact providers use different field names. The binding is model/contract input, so
/// admission contains no provider-family target registry and no semantic capability booleans.
#[derive(Clone, Debug)]
pub struct DeclaredCoverageBinding {
    pub relation_identity: ProviderRelationIdentity,
    pub family_value: String,
    pub family_column: String,
    pub requested_units_column: String,
    pub completed_units_column: String,
    pub status_column: String,
    pub remainder_reason_column: Option<String>,
    pub unknown_semantics_column: Option<String>,
    pub remainder_reason_map: BTreeMap<String, RemainderReason>,
}

/// Source of coverage for one requested raw relation.
#[derive(Clone, Debug)]
pub enum ProviderCoverageSource {
    /// The requested item is the raw control relation itself. Presence plus exact schema is the
    /// complete structural observation; it says nothing about semantic-family support.
    StructuralPresence,
    /// Coverage is read from typed provider rows using an accepted contract binding.
    ProviderDeclared(DeclaredCoverageBinding),
}

/// Whether coverage describes a semantic provider family or the control/evidence relation that
/// carries the observation. Structural presence is legal only for `ControlEvidence`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderRelationPurpose {
    SemanticFact,
    ControlEvidence,
}

/// Application-owned binding from provider relation identity to one epoch table.
#[derive(Clone, Debug)]
pub struct ProviderRelationBinding {
    pub provider_relation: ProviderRelationIdentity,
    pub api_family: ProviderApiFamily,
    pub lane: ProviderNativeLane,
    pub role: FabricSchemaRole,
    pub table_name: String,
    pub source_schema_identity: Arc<str>,
    pub handler_id: ProviderHandlerId,
    pub authority_class: ProviderAuthorityClass,
    pub purpose: ProviderRelationPurpose,
    pub requested_units: u64,
    pub coverage: ProviderCoverageSource,
}

/// Independently accepted admission plan for one exact provider installer.
#[derive(Clone, Debug)]
pub struct ProviderAdmissionPlan {
    pub provider_kind: Arc<str>,
    pub expected_source_pin: SourcePin,
    pub expected_context_pin: ContextPin,
    pub installer: ProviderInstallerIdentity,
    pub contract: ProviderBoundaryContract,
    pub bindings: Vec<ProviderRelationBinding>,
}

/// One provider relation and its typed batches before boundary evaluation.
///
/// This carrier stays private to the exclusive programmatic admission transaction. Exposing it
/// would let a caller assemble a provider-shaped relation set without first presenting the exact
/// provider run that owns its source, context, coverage, and terminal evidence.
#[derive(Clone, Debug)]
struct ObservedProviderRelation {
    identity: ProviderRelationIdentity,
    lane: ProviderNativeLane,
    batches: Vec<RecordBatch>,
}

/// Exact provider output carrying independently observable source and context pins.
#[derive(Clone, Debug)]
struct AcceptedProviderRelationSet {
    source_pin: SourcePin,
    context_pin: ContextPin,
    relations: BTreeMap<ProviderRelationIdentity, ObservedProviderRelation>,
    gap: Option<ProviderLaneGap>,
}

impl AcceptedProviderRelationSet {
    /// Construct one transaction-private relation set after an exact provider adapter has
    /// validated its native run. This deliberately is not a public admission overload.
    ///
    /// # Errors
    ///
    /// Rejects zero pins, duplicate relations, empty batch vectors, or schema drift within one
    /// relation.
    fn try_new(
        source_pin: SourcePin,
        context_pin: ContextPin,
        observed: Vec<ObservedProviderRelation>,
    ) -> Result<Self, ProviderAdmissionError> {
        require_nonzero(source_pin.0, "source pin")?;
        require_nonzero(context_pin.0, "context pin")?;
        let mut relations = BTreeMap::new();
        for relation in observed {
            if relation.batches.is_empty() {
                return Err(ProviderAdmissionError::InvalidObservedRelation {
                    relation: relation.identity.as_str().to_owned(),
                    detail: "relation has no Arrow batch/schema carrier".into(),
                });
            }
            let schema = relation.batches[0].schema();
            if relation
                .batches
                .iter()
                .any(|batch| batch.schema().as_ref() != schema.as_ref())
            {
                return Err(ProviderAdmissionError::InvalidObservedRelation {
                    relation: relation.identity.as_str().to_owned(),
                    detail: "batches have different schemas".into(),
                });
            }
            let identity = relation.identity.clone();
            if relations.insert(identity.clone(), relation).is_some() {
                return Err(ProviderAdmissionError::DuplicateObservedRelation(
                    identity.as_str().to_owned(),
                ));
            }
        }
        Ok(Self {
            source_pin,
            context_pin,
            relations,
            gap: None,
        })
    }

    fn from_gap(
        source_pin: SourcePin,
        context_pin: ContextPin,
        gap: ProviderLaneGap,
    ) -> Result<Self, ProviderAdmissionError> {
        require_nonzero(source_pin.0, "source pin")?;
        require_nonzero(context_pin.0, "context pin")?;
        Ok(Self {
            source_pin,
            context_pin,
            relations: BTreeMap::new(),
            gap: Some(gap),
        })
    }

    /// Validate and project the exact WP08 Tree-sitter/Ruff result without hiding empty
    /// relations.
    ///
    /// # Errors
    ///
    /// Rejects missing/malformed native pin fields, mixed source/context pins, relation metadata
    /// drift, or a release/provider identifier that differs from the exact compiled adapter.
    fn from_native_syntax(run: &ProviderNativeSyntaxRun) -> Result<Self, ProviderAdmissionError> {
        let mut observed = Vec::with_capacity(run.relations.len());
        let mut source_pin = None;
        let mut context_pin = None;
        for (relation, batch) in &run.relations {
            let lane = syntax_lane(*relation);
            validate_syntax_batch(*relation, lane, batch, &mut source_pin, &mut context_pin)?;
            observed.push(ObservedProviderRelation {
                identity: ProviderRelationIdentity::try_new(relation.as_str())?,
                lane,
                batches: vec![batch.clone()],
            });
        }
        let source_pin = source_pin.ok_or_else(|| ProviderAdmissionError::MissingObservedPin {
            pin: "content_digest",
            provider: "Tree-sitter/Ruff",
        })?;
        let context_pin =
            context_pin.ok_or_else(|| ProviderAdmissionError::MissingObservedPin {
                pin: "analysis_context_id",
                provider: "Tree-sitter/Ruff",
            })?;
        Self::try_new(SourcePin(source_pin), ContextPin(context_pin), observed)
    }

    /// Validate and project the exact WP09 accepted Pyrefly relation streams. Batches for the
    /// same relation across modules become partitions of one raw epoch table.
    ///
    /// # Errors
    ///
    /// Rejects schema/relation/digest/correlation drift, duplicate module relations, or mixed
    /// semantic-environment pins.
    fn from_pyrefly(run: &AcceptedPyreflyRun) -> Result<Self, ProviderAdmissionError> {
        if run.modules.is_empty() {
            return Err(ProviderAdmissionError::InvalidObservedRelation {
                relation: "provider.pyrefly.run".into(),
                detail: "accepted run has no modules".into(),
            });
        }
        let source_pin = pyrefly_source_pin(run);
        let mut context_pin = None;
        let mut grouped =
            BTreeMap::<ProviderRelationIdentity, (ProviderNativeLane, Vec<RecordBatch>)>::new();
        for module in &run.modules {
            let expected_digest = *blake3::hash(&module.source_bytes).as_bytes();
            let mut module_relations = BTreeSet::new();
            for relation in &module.relations {
                if !module_relations.insert(relation.relation) {
                    return Err(ProviderAdmissionError::DuplicateObservedRelation(format!(
                        "{}:{}",
                        module.module_id,
                        relation.relation.relation_id()
                    )));
                }
                validate_pyrefly_batch(
                    run,
                    module,
                    relation.relation,
                    &relation.batch,
                    relation.row_count,
                    &relation.schema_digest,
                    expected_digest,
                    &mut context_pin,
                )?;
                let identity = ProviderRelationIdentity::try_new(relation.relation.relation_id())?;
                grouped
                    .entry(identity)
                    .or_insert_with(|| (ProviderNativeLane::Pyrefly, Vec::new()))
                    .1
                    .push(relation.batch.clone());
            }
        }
        let context_pin =
            context_pin.ok_or_else(|| ProviderAdmissionError::MissingObservedPin {
                pin: "semantic_environment_id",
                provider: "Pyrefly",
            })?;
        let observed = grouped
            .into_iter()
            .map(|(identity, (lane, batches))| ObservedProviderRelation {
                identity,
                lane,
                batches,
            })
            .collect();
        Self::try_new(source_pin, ContextPin(context_pin), observed)
    }

    /// Decode the exact WP10 rustc relation streams already accepted by the daemon protocol.
    ///
    /// The source-snapshot and compiler-context digests bind the complete relation set. Each
    /// relation is decoded again at this boundary so protocol acceptance cannot substitute a schema,
    /// owner, run, compilation unit, or source generation before catalog registration. Zero-row
    /// batches remain present and therefore prove an available family had no facts for an owner.
    ///
    /// # Errors
    ///
    /// Rejects malformed digest pins, predecessor/unknown family codes on the target route,
    /// Arrow decode/schema/row drift, mixed owner/run/unit/generation columns, or duplicate
    /// owner-family relations.
    fn from_rustc(run: &AcceptedRustcCompilation) -> Result<Self, ProviderAdmissionError> {
        let source_pin = parse_b3_pin(
            &run.admission.source_snapshot_manifest_digest,
            "rustc source snapshot manifest",
        )?;
        let context_pin = parse_b3_pin(
            &run.admission.context_manifest_digest,
            "rustc context manifest",
        )?;
        if run.begin.compilation_unit_id.trim().is_empty() {
            return Err(ProviderAdmissionError::InvalidObservedRelation {
                relation: "provider.rustc.compilation.v1".into(),
                detail: "accepted compilation unit identity is empty".into(),
            });
        }

        let mut grouped =
            BTreeMap::<ProviderRelationIdentity, (ProviderNativeLane, Vec<RecordBatch>)>::new();
        for owner in &run.owners {
            append_rustc_owner(run, owner, &mut grouped)?;
        }
        if grouped.is_empty() {
            return Err(ProviderAdmissionError::InvalidObservedRelation {
                relation: "provider.rustc.compilation.v1".into(),
                detail: "accepted compiler stream contains no target Arrow relations".into(),
            });
        }
        let observed = grouped
            .into_iter()
            .map(|(identity, (lane, batches))| ObservedProviderRelation {
                identity,
                lane,
                batches,
            })
            .collect();
        Self::try_new(SourcePin(source_pin), ContextPin(context_pin), observed)
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) const fn source_pin(&self) -> SourcePin {
        self.source_pin
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) const fn context_pin(&self) -> ContextPin {
        self.context_pin
    }
}

fn append_rustc_owner(
    run: &AcceptedRustcCompilation,
    owner: &AcceptedRustcOwner,
    grouped: &mut BTreeMap<ProviderRelationIdentity, (ProviderNativeLane, Vec<RecordBatch>)>,
) -> Result<(), ProviderAdmissionError> {
    let owner_id = owner
        .begin
        .owner
        .as_ref()
        .map(|key| key.owner_id.as_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ProviderAdmissionError::InvalidObservedRelation {
            relation: "provider.rustc.owner".into(),
            detail: "accepted rustc owner identity is absent".into(),
        })?;
    let mut families = BTreeSet::new();
    for accepted_relation in &owner.relations {
        let relation = accepted_relation.relation;
        if !families.insert(relation) {
            return Err(ProviderAdmissionError::DuplicateObservedRelation(format!(
                "{owner_id}:{}",
                relation.relation_id()
            )));
        }
        let batch = decode_rustc_relation(run, owner_id, relation, accepted_relation)?;
        let identity = ProviderRelationIdentity::try_new(relation.relation_id())?;
        grouped
            .entry(identity)
            .or_insert_with(|| (ProviderNativeLane::Rustc, Vec::new()))
            .1
            .push(batch);
    }
    Ok(())
}

fn decode_rustc_relation(
    run: &AcceptedRustcCompilation,
    owner_id: &str,
    relation: RustcRelation,
    accepted: &AcceptedRustcRelation,
) -> Result<RecordBatch, ProviderAdmissionError> {
    let relation_name = relation.relation_id();
    if accepted.relation != relation
        || accepted.schema_digest != relation.schema_digest()
        || accepted.arrow_ipc_digest != arrow_ipc_digest(&accepted.arrow_ipc)
    {
        return Err(ProviderAdmissionError::InvalidObservedRelation {
            relation: relation_name.to_owned(),
            detail: "rustc relation-stream identity differs from the accepted run".into(),
        });
    }
    let mut reader =
        StreamReader::try_new(Cursor::new(&accepted.arrow_ipc), None).map_err(|error| {
            ProviderAdmissionError::InvalidObservedRelation {
                relation: relation_name.to_owned(),
                detail: format!("rustc Arrow stream is invalid: {error}"),
            }
        })?;
    let expected_schema = relation.schema();
    if reader.schema().as_ref() != expected_schema.as_ref() {
        return Err(ProviderAdmissionError::SchemaMismatch {
            relation: relation_name.to_owned(),
        });
    }
    let batch = reader
        .next()
        .transpose()
        .map_err(|error| ProviderAdmissionError::InvalidObservedRelation {
            relation: relation_name.to_owned(),
            detail: format!("rustc Arrow batch is invalid: {error}"),
        })?
        .ok_or_else(|| ProviderAdmissionError::InvalidObservedRelation {
            relation: relation_name.to_owned(),
            detail: "rustc Arrow stream contains no schema-carrying batch".into(),
        })?;
    if reader
        .next()
        .transpose()
        .map_err(|error| ProviderAdmissionError::InvalidObservedRelation {
            relation: relation_name.to_owned(),
            detail: format!("rustc Arrow batch is invalid: {error}"),
        })?
        .is_some()
        || u64::try_from(batch.num_rows()).ok() != Some(accepted.row_count)
        || batch != accepted.batch
    {
        return Err(ProviderAdmissionError::InvalidObservedRelation {
            relation: relation_name.to_owned(),
            detail: "rustc Arrow batch count/row declaration differs".into(),
        });
    }

    validate_rustc_common_columns(run, owner_id, relation_name, &batch)?;
    Ok(batch)
}

fn validate_rustc_common_columns(
    run: &AcceptedRustcCompilation,
    owner_id: &str,
    relation: &str,
    batch: &RecordBatch,
) -> Result<(), ProviderAdmissionError> {
    let provider_runs = utf8_column(batch, "provider_run_id", relation)?;
    let compilation_units = utf8_column(batch, "compilation_unit_id", relation)?;
    let owners = utf8_column(batch, "owner_id", relation)?;
    let source_generations = u64_column(batch, "source_generation", relation)?;
    for row in 0..batch.num_rows() {
        if provider_runs.is_null(row)
            || compilation_units.is_null(row)
            || owners.is_null(row)
            || source_generations.is_null(row)
            || provider_runs.value(row) != run.admission.provider_run_id
            || compilation_units.value(row) != run.begin.compilation_unit_id
            || owners.value(row) != owner_id
            || source_generations.value(row) != run.admission.source_generation
        {
            return Err(ProviderAdmissionError::InvalidObservedRelation {
                relation: relation.to_owned(),
                detail: "rustc Arrow row identity differs from accepted control pins".into(),
            });
        }
    }
    Ok(())
}

fn parse_b3_pin(value: &str, label: &'static str) -> Result<[u8; 32], ProviderAdmissionError> {
    let encoded = value
        .strip_prefix("b3:")
        .filter(|encoded| encoded.len() == 64)
        .ok_or_else(|| ProviderAdmissionError::InvalidObservedRelation {
            relation: "provider.rustc.compilation.v1".into(),
            detail: format!("{label} is not a b3-32 digest"),
        })?;
    let mut result = [0_u8; 32];
    for (index, chunk) in encoded.as_bytes().chunks_exact(2).enumerate() {
        result[index] = u8::from_str_radix(
            std::str::from_utf8(chunk).map_err(|_| {
                ProviderAdmissionError::InvalidObservedRelation {
                    relation: "provider.rustc.compilation.v1".into(),
                    detail: format!("{label} is not UTF-8 hexadecimal"),
                }
            })?,
            16,
        )
        .map_err(|_| ProviderAdmissionError::InvalidObservedRelation {
            relation: "provider.rustc.compilation.v1".into(),
            detail: format!("{label} is not hexadecimal"),
        })?;
    }
    Ok(result)
}

/// Why a requested relation was intentionally not registered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderAdmissionUnknownCause {
    MissingRelation,
    MissingCoverage,
    ProviderDeclared,
}

/// Application-owned cause for a provider lane that produced no accepted relation set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderLaneGap {
    RequiredInputAbsent,
    OptionalInputAbsent,
    ProviderFailure,
    CompilationFailure,
    TrustUnavailable,
    ResourceLimit,
    TimedOut,
    Cancelled,
    InvalidSource,
    Unsupported,
}

impl ProviderLaneGap {
    const fn terminal_status(self) -> TerminalStatus {
        match self {
            Self::RequiredInputAbsent | Self::OptionalInputAbsent => TerminalStatus::Unknown,
            Self::ProviderFailure
            | Self::CompilationFailure
            | Self::TrustUnavailable
            | Self::ResourceLimit
            | Self::TimedOut
            | Self::Cancelled
            | Self::InvalidSource
            | Self::Unsupported => TerminalStatus::Partial,
        }
    }

    const fn remainder_reason(self) -> RemainderReason {
        match self {
            Self::RequiredInputAbsent | Self::OptionalInputAbsent => RemainderReason::Unknown,
            Self::ProviderFailure | Self::CompilationFailure | Self::TrustUnavailable => {
                RemainderReason::ProviderUnavailable
            }
            Self::ResourceLimit | Self::TimedOut => RemainderReason::ResourceLimit,
            Self::Cancelled => RemainderReason::Cancelled,
            Self::InvalidSource => RemainderReason::InvalidSource,
            Self::Unsupported => RemainderReason::Unsupported,
        }
    }
}

/// One provider lane supplies either at least one accepted application DTO or one exact gap.
pub enum ExactProviderLaneRuns<'a, T> {
    Accepted(&'a [T]),
    Gap(ProviderLaneGap),
}

impl<T> Copy for ExactProviderLaneRuns<'_, T> {}

impl<T> Clone for ExactProviderLaneRuns<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Registration disposition derived from the executable boundary report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderRegistrationDisposition {
    Registered {
        row_count: usize,
        coverage: TerminalStatus,
    },
    RegisteredUnknown {
        row_count: usize,
        cause: ProviderAdmissionUnknownCause,
    },
    Unknown {
        cause: ProviderAdmissionUnknownCause,
    },
    Remainder {
        trailer: CoverageTrailer,
    },
}

/// Relation-level admission observation. It is not a semantic capability boolean.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRelationAdmission {
    pub provider_relation: ProviderRelationIdentity,
    pub api_family: ProviderApiFamily,
    pub disposition: ProviderRegistrationDisposition,
    pub lane_gap: Option<ProviderLaneGap>,
}

/// Boundary proof and exact relation dispositions for one provider admission.
#[derive(Clone, Debug)]
pub struct ProviderAdmissionReport {
    pub boundary: ProviderBoundaryReport,
    pub relations: Vec<ProviderRelationAdmission>,
}

/// The exact accepted provider runs and independently accepted admission plans
/// that must enter one programmatic candidate together.
///
/// Each Python source partition carries a paired in-process Tree-sitter/Ruff
/// run. The lanes retain separate admission plans, catalog roles, boundary
/// reports, and authority pins after workspace aggregation.
pub struct ExactProgrammaticProviderRuns<'a> {
    tree_sitter_plan: &'a ProviderAdmissionPlan,
    ruff_plan: &'a ProviderAdmissionPlan,
    native_syntax_runs: ExactProviderLaneRuns<'a, ProviderNativeSyntaxRun>,
    pyrefly_plan: &'a ProviderAdmissionPlan,
    pyrefly_runs: ExactProviderLaneRuns<'a, AcceptedPyreflyRun>,
    rustc_plan: &'a ProviderAdmissionPlan,
    rustc_runs: ExactProviderLaneRuns<'a, TrustQualifiedRustcCompilation>,
}

impl<'a> ExactProgrammaticProviderRuns<'a> {
    #[must_use]
    pub(crate) fn try_new(
        tree_sitter_plan: &'a ProviderAdmissionPlan,
        ruff_plan: &'a ProviderAdmissionPlan,
        native_syntax_runs: ExactProviderLaneRuns<'a, ProviderNativeSyntaxRun>,
        pyrefly_plan: &'a ProviderAdmissionPlan,
        pyrefly_runs: ExactProviderLaneRuns<'a, AcceptedPyreflyRun>,
        rustc_plan: &'a ProviderAdmissionPlan,
        rustc_runs: ExactProviderLaneRuns<'a, TrustQualifiedRustcCompilation>,
    ) -> Result<Self, ProviderAdmissionError> {
        for (lane, empty) in [
            (
                ProviderNativeLane::TreeSitter,
                matches!(native_syntax_runs, ExactProviderLaneRuns::Accepted(runs) if runs.is_empty()),
            ),
            (
                ProviderNativeLane::Pyrefly,
                matches!(pyrefly_runs, ExactProviderLaneRuns::Accepted(runs) if runs.is_empty()),
            ),
            (
                ProviderNativeLane::Rustc,
                matches!(rustc_runs, ExactProviderLaneRuns::Accepted(runs) if runs.is_empty()),
            ),
        ] {
            if empty {
                return Err(ProviderAdmissionError::EmptyAcceptedProviderLane { lane });
            }
        }
        Ok(Self {
            tree_sitter_plan,
            ruff_plan,
            native_syntax_runs,
            pyrefly_plan,
            pyrefly_runs,
            rustc_plan,
            rustc_runs,
        })
    }
}

/// Provider-specific reports retained from one atomic programmatic admission.
#[derive(Clone, Debug)]
pub struct ExactProgrammaticProviderReports {
    tree_sitter: ProviderAdmissionReport,
    ruff: ProviderAdmissionReport,
    pyrefly: ProviderAdmissionReport,
    rustc: ProviderAdmissionReport,
}

impl ExactProgrammaticProviderReports {
    #[must_use]
    pub const fn tree_sitter(&self) -> &ProviderAdmissionReport {
        &self.tree_sitter
    }

    #[must_use]
    pub const fn ruff(&self) -> &ProviderAdmissionReport {
        &self.ruff
    }

    #[must_use]
    pub const fn pyrefly(&self) -> &ProviderAdmissionReport {
        &self.pyrefly
    }

    #[must_use]
    pub const fn rustc(&self) -> &ProviderAdmissionReport {
        &self.rustc
    }
}

/// Successful all-provider admission into the same programmatic candidate
/// session that will build every downstream CPG transformation.
pub struct ProgrammaticProviderAdmissionOutcome {
    builder: ProgrammaticFabricEpochBuilder,
    reports: ExactProgrammaticProviderReports,
}

impl ProgrammaticProviderAdmissionOutcome {
    #[must_use]
    pub const fn reports(&self) -> &ExactProgrammaticProviderReports {
        &self.reports
    }

    /// Exact candidate epoch that owns the admitted provider catalogs.
    #[must_use]
    pub const fn candidate_epoch_id(&self) -> &FabricEpochId {
        self.builder.identity()
    }

    #[must_use]
    pub(crate) fn into_parts(
        self,
    ) -> (
        ProgrammaticFabricEpochBuilder,
        ExactProgrammaticProviderReports,
    ) {
        (self.builder, self.reports)
    }
}

/// Application-owned catalog binding for the capability relation derived from exact provider runs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCapabilityCatalogBinding {
    pub table_name: String,
    pub source_schema_identity: Arc<str>,
}

/// Successful provider admission plus its registered, proof-qualified capability relation.
pub struct ProviderCapabilityAdmissionOutcome {
    builder: ProgrammaticFabricEpochBuilder,
    provider_reports: ExactProgrammaticProviderReports,
    capabilities: Vec<ProviderCapabilityRelation>,
}

impl ProviderCapabilityAdmissionOutcome {
    #[must_use]
    pub const fn provider_reports(&self) -> &ExactProgrammaticProviderReports {
        &self.provider_reports
    }

    #[must_use]
    pub fn capabilities(&self) -> &[ProviderCapabilityRelation] {
        &self.capabilities
    }

    /// Exact candidate epoch that owns both raw providers and capability evidence.
    #[must_use]
    pub const fn candidate_epoch_id(&self) -> &FabricEpochId {
        self.builder.identity()
    }

    #[must_use]
    pub(crate) fn into_parts(
        self,
    ) -> (
        ProgrammaticFabricEpochBuilder,
        ExactProgrammaticProviderReports,
        Vec<ProviderCapabilityRelation>,
    ) {
        (self.builder, self.provider_reports, self.capabilities)
    }
}

/// Closed admission failures. Any error drops the consumed candidate builder.
#[derive(Debug, Error)]
pub enum ProviderAdmissionError {
    #[error("provider admission plan is invalid: {0}")]
    InvalidPlan(String),
    #[error("provider relation {relation} is invalid: {detail}")]
    InvalidObservedRelation { relation: String, detail: String },
    #[error("duplicate observed provider relation {0}")]
    DuplicateObservedRelation(String),
    #[error("provider output contains unbound relation {0}")]
    UnexpectedProviderRelation(String),
    #[error("provider relation {relation} was produced by {actual:?}, expected {expected:?}")]
    ProviderLaneMismatch {
        relation: String,
        expected: ProviderNativeLane,
        actual: ProviderNativeLane,
    },
    #[error("exact {lane:?} provider run omitted required relation {relation}")]
    IncompleteExactProviderRun {
        lane: ProviderNativeLane,
        relation: String,
    },
    #[error("exact {lane:?} provider lane supplied an empty accepted-run set instead of a gap")]
    EmptyAcceptedProviderLane { lane: ProviderNativeLane },
    #[error("exact {lane:?} provider workspace contains duplicate partition {partition}")]
    DuplicateProviderPartition {
        lane: ProviderNativeLane,
        partition: String,
    },
    #[error("exact {lane:?} provider partitions do not share one workspace authority")]
    InconsistentProviderWorkspaceAuthority { lane: ProviderNativeLane },
    #[error(
        "exact {lane:?} provider workspace has {actual} partitions, exceeding the bound {maximum}"
    )]
    ProviderWorkspacePartitionLimit {
        lane: ProviderNativeLane,
        actual: usize,
        maximum: usize,
    },
    #[error(
        "provider relation {relation} is bound by both {first:?} and {second:?} admission plans"
    )]
    CrossProviderRelationIdentity {
        relation: String,
        first: ProviderNativeLane,
        second: ProviderNativeLane,
    },
    #[error(
        "epoch table {role:?}.{table} is bound by both {first:?} and {second:?} admission plans"
    )]
    CrossProviderTableBinding {
        role: FabricSchemaRole,
        table: String,
        first: ProviderNativeLane,
        second: ProviderNativeLane,
    },
    #[error("provider source/context pin differs from the accepted admission plan")]
    AdmissionPinMismatch,
    #[error("provider capability proof epoch differs from the candidate epoch")]
    CapabilityProofEpochMismatch,
    #[error("{provider} output does not expose required {pin} pin")]
    MissingObservedPin {
        pin: &'static str,
        provider: &'static str,
    },
    #[error("provider relation {relation} schema differs from the accepted boundary row")]
    SchemaMismatch { relation: String },
    #[error("provider relation {relation} has malformed typed coverage: {detail}")]
    InvalidCoverage { relation: String, detail: String },
    #[error("provider-native authority is forbidden for {class:?} ({family})")]
    ForbiddenAuthorityClaim {
        class: ProviderAuthorityClass,
        family: String,
    },
    #[error("provider emitted forbidden application-derived family {0}")]
    ForbiddenProviderOutput(String),
    #[error("rustc trust evidence cannot enter exact provider admission: {0}")]
    RustcTrustEvidence(String),
    #[error(transparent)]
    Boundary(#[from] ProviderBoundaryError),
    #[error(transparent)]
    Capability(#[from] ProviderCapabilityError),
    #[error(transparent)]
    SchemaContract(#[from] SchemaContractError),
    #[error(transparent)]
    DataFusion(#[from] DataFusionError),
    #[error(transparent)]
    ProgrammaticSchema(#[from] ProgrammaticSchemaError),
}

const fn syntax_lane(relation: NativeSyntaxRelation) -> ProviderNativeLane {
    match relation {
        NativeSyntaxRelation::TreeSitterRun
        | NativeSyntaxRelation::TreeSitterCoverage
        | NativeSyntaxRelation::TreeSitterRemainder
        | NativeSyntaxRelation::TreeSitterCstNode
        | NativeSyntaxRelation::TreeSitterChangedRange
        | NativeSyntaxRelation::TreeSitterRecoveryDiagnostic => ProviderNativeLane::TreeSitter,
        NativeSyntaxRelation::RuffRun
        | NativeSyntaxRelation::RuffCoverage
        | NativeSyntaxRelation::RuffRemainder
        | NativeSyntaxRelation::RuffToken
        | NativeSyntaxRelation::RuffComment
        | NativeSyntaxRelation::RuffDirective
        | NativeSyntaxRelation::RuffStringRegion
        | NativeSyntaxRelation::RuffDocstring
        | NativeSyntaxRelation::RuffContinuationLine
        | NativeSyntaxRelation::RuffAstNode
        | NativeSyntaxRelation::RuffParseDiagnostic
        | NativeSyntaxRelation::RuffDiagnosticRecoveryEvidence
        | NativeSyntaxRelation::RuffScope
        | NativeSyntaxRelation::RuffBinding
        | NativeSyntaxRelation::RuffReference
        | NativeSyntaxRelation::RuffUnknownSymbol
        | NativeSyntaxRelation::RuffSemanticEdge
        | NativeSyntaxRelation::RuffImport
        | NativeSyntaxRelation::RuffExport => ProviderNativeLane::Ruff,
    }
}

fn validate_syntax_batch(
    relation: NativeSyntaxRelation,
    lane: ProviderNativeLane,
    batch: &RecordBatch,
    source_pin: &mut Option<[u8; 32]>,
    context_pin: &mut Option<[u8; 32]>,
) -> Result<(), ProviderAdmissionError> {
    let relation_name = relation.as_str();
    if batch
        .schema_ref()
        .metadata()
        .get("codefabric.relation")
        .map(String::as_str)
        != Some(relation_name)
    {
        return Err(ProviderAdmissionError::InvalidObservedRelation {
            relation: relation_name.to_owned(),
            detail: "schema relation identity differs".into(),
        });
    }

    let provider_ids = utf8_column(batch, "provider_id", relation_name)?;
    let provider_releases = utf8_column(batch, "provider_release", relation_name)?;
    let content_digests = fixed32_column(batch, "content_digest", relation_name)?;
    let context_ids = fixed32_column(batch, "analysis_context_id", relation_name)?;
    // This column is required even when this particular fact relation has zero rows. Its type
    // makes the exact provider-context contract executable without inventing an empty-row
    // capability. Fabric-epoch identity is assigned only after the provider transaction admits.
    let _semantic_environment_ids =
        fixed32_column(batch, "semantic_environment_id", relation_name)?;

    let (expected_provider, expected_release) = match lane {
        ProviderNativeLane::TreeSitter => (
            "tree-sitter-python".to_owned(),
            format!(
                "tree-sitter={TREE_SITTER_RUNTIME_RELEASE};tree-sitter-python={TREE_SITTER_PYTHON_GRAMMAR_RELEASE}"
            ),
        ),
        ProviderNativeLane::Ruff => (
            "ruff-python".to_owned(),
            format!(
                "ruff-python-ast={RUFF_COMPONENT_RELEASE};ruff-python-parser={RUFF_COMPONENT_RELEASE};python-target=3.14"
            ),
        ),
        ProviderNativeLane::Pyrefly | ProviderNativeLane::Rustc => {
            unreachable!("syntax run contains only Tree-sitter and Ruff")
        }
    };
    for row in 0..batch.num_rows() {
        if provider_ids.is_null(row)
            || provider_releases.is_null(row)
            || provider_ids.value(row) != expected_provider
            || provider_releases.value(row) != expected_release
        {
            return Err(ProviderAdmissionError::InvalidObservedRelation {
                relation: relation_name.to_owned(),
                detail: "provider identity/release differs from the exact adapter".into(),
            });
        }
        observe_fixed32(
            source_pin,
            content_digests,
            row,
            relation_name,
            "content_digest",
        )?;
        observe_fixed32(
            context_pin,
            context_ids,
            row,
            relation_name,
            "analysis_context_id",
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_pyrefly_batch(
    run: &AcceptedPyreflyRun,
    module: &AcceptedPyreflyModule,
    relation: PyreflyRelation,
    batch: &RecordBatch,
    declared_rows: u64,
    declared_schema_digest: &str,
    expected_content_digest: [u8; 32],
    context_pin: &mut Option<[u8; 32]>,
) -> Result<(), ProviderAdmissionError> {
    let relation_name = relation.relation_id();
    let expected_schema = relation.schema();
    if batch.schema_ref().as_ref() != expected_schema.as_ref()
        || usize::try_from(declared_rows).ok() != Some(batch.num_rows())
        || declared_schema_digest != relation.schema_digest()
        || batch
            .schema_ref()
            .metadata()
            .get("codefabric.relation_id")
            .map(String::as_str)
            != Some(relation_name)
        || batch
            .schema_ref()
            .metadata()
            .get("codefabric.schema_digest")
            .map(String::as_str)
            != Some(declared_schema_digest)
    {
        return Err(ProviderAdmissionError::InvalidObservedRelation {
            relation: relation_name.to_owned(),
            detail: "accepted relation schema, digest, identity, or row count differs".into(),
        });
    }

    let provider_run_ids = utf8_column(batch, "provider_run_id", relation_name)?;
    let analysis_context_ids = utf8_column(batch, "analysis_context_id", relation_name)?;
    let module_ids = utf8_column(batch, "module_id", relation_name)?;
    let file_ids = utf8_column(batch, "file_id", relation_name)?;
    let content_digests = fixed32_column(batch, "content_digest", relation_name)?;
    let environment_ids = fixed32_column(batch, "semantic_environment_id", relation_name)?;
    let generations = u64_column(batch, "source_generation", relation_name)?;
    for row in 0..batch.num_rows() {
        if provider_run_ids.is_null(row)
            || analysis_context_ids.is_null(row)
            || module_ids.is_null(row)
            || file_ids.is_null(row)
            || generations.is_null(row)
            || provider_run_ids.value(row) != run.provider_run_id
            || analysis_context_ids.value(row) != run.analysis_context_id
            || module_ids.value(row) != module.module_id
            || file_ids.value(row).is_empty()
            || generations.value(row) != run.source_generation
            || fixed32_value(content_digests, row, relation_name, "content_digest")?
                != expected_content_digest
        {
            return Err(ProviderAdmissionError::InvalidObservedRelation {
                relation: relation_name.to_owned(),
                detail: "accepted row correlation/source pin differs from the run".into(),
            });
        }
        observe_fixed32(
            context_pin,
            environment_ids,
            row,
            relation_name,
            "semantic_environment_id",
        )?;
    }
    Ok(())
}

fn pyrefly_source_pin(run: &AcceptedPyreflyRun) -> SourcePin {
    let mut modules = run
        .modules
        .iter()
        .map(|module| {
            (
                module.module_id.as_bytes(),
                module.canonical_file_id,
                *blake3::hash(&module.source_bytes).as_bytes(),
            )
        })
        .collect::<Vec<_>>();
    modules.sort_by(|left, right| left.0.cmp(right.0).then(left.1.cmp(&right.1)));
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.pyrefly.source-pin.v1\0");
    hasher.update(&run.source_generation.to_be_bytes());
    for (module_id, file_id, digest) in modules {
        hasher.update(
            &u64::try_from(module_id.len())
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
        hasher.update(module_id);
        hasher.update(&file_id);
        hasher.update(&digest);
    }
    SourcePin(*hasher.finalize().as_bytes())
}

fn utf8_column<'a>(
    batch: &'a RecordBatch,
    field: &str,
    relation: &str,
) -> Result<&'a StringArray, ProviderAdmissionError> {
    typed_column::<StringArray>(batch, field, relation, "Utf8")
}

fn u64_column<'a>(
    batch: &'a RecordBatch,
    field: &str,
    relation: &str,
) -> Result<&'a UInt64Array, ProviderAdmissionError> {
    typed_column::<UInt64Array>(batch, field, relation, "UInt64")
}

fn bool_column<'a>(
    batch: &'a RecordBatch,
    field: &str,
    relation: &str,
) -> Result<&'a BooleanArray, ProviderAdmissionError> {
    typed_column::<BooleanArray>(batch, field, relation, "Boolean")
}

fn fixed32_column<'a>(
    batch: &'a RecordBatch,
    field: &str,
    relation: &str,
) -> Result<&'a FixedSizeBinaryArray, ProviderAdmissionError> {
    let column =
        typed_column::<FixedSizeBinaryArray>(batch, field, relation, "FixedSizeBinary(32)")?;
    if column.value_length() != 32 {
        return Err(ProviderAdmissionError::InvalidObservedRelation {
            relation: relation.to_owned(),
            detail: format!("{field} is not FixedSizeBinary(32)"),
        });
    }
    Ok(column)
}

fn fixed16_column<'a>(
    batch: &'a RecordBatch,
    field: &str,
    relation: &str,
) -> Result<&'a FixedSizeBinaryArray, ProviderAdmissionError> {
    let column =
        typed_column::<FixedSizeBinaryArray>(batch, field, relation, "FixedSizeBinary(16)")?;
    if column.value_length() != 16 {
        return Err(ProviderAdmissionError::InvalidObservedRelation {
            relation: relation.to_owned(),
            detail: format!("{field} is not FixedSizeBinary(16)"),
        });
    }
    Ok(column)
}

fn typed_column<'a, T: 'static>(
    batch: &'a RecordBatch,
    field: &str,
    relation: &str,
    expected: &str,
) -> Result<&'a T, ProviderAdmissionError> {
    let index = batch.schema_ref().index_of(field).map_err(|_| {
        ProviderAdmissionError::InvalidObservedRelation {
            relation: relation.to_owned(),
            detail: format!("missing {field} column"),
        }
    })?;
    batch
        .column(index)
        .as_any()
        .downcast_ref::<T>()
        .ok_or_else(|| ProviderAdmissionError::InvalidObservedRelation {
            relation: relation.to_owned(),
            detail: format!("{field} is not {expected}"),
        })
}

fn fixed32_value(
    column: &FixedSizeBinaryArray,
    row: usize,
    relation: &str,
    field: &str,
) -> Result<[u8; 32], ProviderAdmissionError> {
    if column.is_null(row) {
        return Err(ProviderAdmissionError::InvalidObservedRelation {
            relation: relation.to_owned(),
            detail: format!("{field} is null"),
        });
    }
    column
        .value(row)
        .try_into()
        .map_err(|_| ProviderAdmissionError::InvalidObservedRelation {
            relation: relation.to_owned(),
            detail: format!("{field} has the wrong width"),
        })
}

fn fixed16_value(
    column: &FixedSizeBinaryArray,
    row: usize,
    relation: &str,
    field: &str,
) -> Result<[u8; 16], ProviderAdmissionError> {
    if column.is_null(row) {
        return Err(ProviderAdmissionError::InvalidObservedRelation {
            relation: relation.to_owned(),
            detail: format!("{field} is null"),
        });
    }
    column
        .value(row)
        .try_into()
        .map_err(|_| ProviderAdmissionError::InvalidObservedRelation {
            relation: relation.to_owned(),
            detail: format!("{field} has the wrong width"),
        })
}

fn observe_fixed32(
    observed: &mut Option<[u8; 32]>,
    column: &FixedSizeBinaryArray,
    row: usize,
    relation: &str,
    field: &str,
) -> Result<(), ProviderAdmissionError> {
    let value = fixed32_value(column, row, relation, field)?;
    if observed.is_some_and(|expected| expected != value) {
        return Err(ProviderAdmissionError::InvalidObservedRelation {
            relation: relation.to_owned(),
            detail: format!("{field} differs across the accepted provider run"),
        });
    }
    *observed = Some(value);
    Ok(())
}

fn require_nonzero<const N: usize>(
    value: [u8; N],
    label: &'static str,
) -> Result<(), ProviderAdmissionError> {
    if value == [0; N] {
        return Err(ProviderAdmissionError::InvalidPlan(format!(
            "{label} uses the zero sentinel"
        )));
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct PreparedProviderTable {
    binding: ProviderRelationBinding,
    schema: SchemaRef,
    batches: Vec<RecordBatch>,
}

#[derive(Clone, Debug)]
struct PreparedProviderAdmission {
    report: ProviderAdmissionReport,
    tables: Vec<PreparedProviderTable>,
}

/// Evaluate exact provider output without mutating a candidate epoch.
///
/// Kept transaction-private so callers cannot stop after validating one hand-assembled provider
/// set and treat that report as admission. The public route consumes the programmatic epoch
/// builder and all exact provider runs together.
///
/// # Errors
///
/// Rejects plan, ownership, schema, pin, coverage, authority, or unexpected-output
/// contradictions. Ordinary missing output/coverage is returned as an explicit unknown.
#[cfg(test)]
fn evaluate_provider_admission(
    plan: &ProviderAdmissionPlan,
    observed: &AcceptedProviderRelationSet,
) -> Result<ProviderAdmissionReport, ProviderAdmissionError> {
    Ok(prepare_provider_admission(plan, observed)?.report)
}

/// Admit the exact Tree-sitter, Ruff, Pyrefly, and rustc runs into one
/// programmatic epoch transaction.
///
/// The builder is consumed before registration and is returned only after all
/// four provider lanes succeed. Exact run conversion and every plan are
/// preflighted first; any later catalog failure drops the assembly containing
/// the earlier registrations instead of exposing partial state. Provider rows
/// retain their native schemas and source/context columns, while the returned
/// reports retain each provider's independently validated authority pins and
/// explicit unknown/remainder outcomes.
///
/// # Errors
///
/// Returns a typed exact-run, admission, cross-provider collision,
/// schema-contract, catalog, or DataFusion failure.
pub(crate) fn admit_provider_relations_programmatic(
    builder: ProgrammaticFabricEpochBuilder,
    runs: ExactProgrammaticProviderRuns<'_>,
) -> Result<ProgrammaticProviderAdmissionOutcome, ProviderAdmissionError> {
    if runs.tree_sitter_plan.expected_source_pin != runs.ruff_plan.expected_source_pin
        || runs.tree_sitter_plan.expected_context_pin != runs.ruff_plan.expected_context_pin
    {
        return Err(
            ProviderAdmissionError::InconsistentProviderWorkspaceAuthority {
                lane: ProviderNativeLane::TreeSitter,
            },
        );
    }
    let native_syntax = match runs.native_syntax_runs {
        ExactProviderLaneRuns::Accepted(accepted) => aggregate_native_syntax_runs(
            accepted,
            runs.tree_sitter_plan.expected_source_pin,
            runs.tree_sitter_plan.expected_context_pin,
        )?,
        ExactProviderLaneRuns::Gap(gap) => AcceptedProviderRelationSet::from_gap(
            runs.tree_sitter_plan.expected_source_pin,
            runs.tree_sitter_plan.expected_context_pin,
            gap,
        )?,
    };
    let tree_sitter = provider_lane_subset(&native_syntax, ProviderNativeLane::TreeSitter)?;
    let ruff = provider_lane_subset(&native_syntax, ProviderNativeLane::Ruff)?;
    let pyrefly = match runs.pyrefly_runs {
        ExactProviderLaneRuns::Accepted(accepted) => aggregate_pyrefly_runs(
            accepted,
            runs.pyrefly_plan.expected_source_pin,
            runs.pyrefly_plan.expected_context_pin,
        )?,
        ExactProviderLaneRuns::Gap(gap) => AcceptedProviderRelationSet::from_gap(
            runs.pyrefly_plan.expected_source_pin,
            runs.pyrefly_plan.expected_context_pin,
            gap,
        )?,
    };
    let rustc = match runs.rustc_runs {
        ExactProviderLaneRuns::Accepted(accepted) => aggregate_rustc_runs(
            accepted,
            runs.rustc_plan.expected_source_pin,
            runs.rustc_plan.expected_context_pin,
        )?,
        ExactProviderLaneRuns::Gap(gap) => AcceptedProviderRelationSet::from_gap(
            runs.rustc_plan.expected_source_pin,
            runs.rustc_plan.expected_context_pin,
            gap,
        )?,
    };

    let plans = [
        (ProviderNativeLane::TreeSitter, runs.tree_sitter_plan),
        (ProviderNativeLane::Ruff, runs.ruff_plan),
        (ProviderNativeLane::Pyrefly, runs.pyrefly_plan),
        (ProviderNativeLane::Rustc, runs.rustc_plan),
    ];
    validate_programmatic_transaction_plans(&plans)?;

    let tree_sitter = prepare_provider_admission(runs.tree_sitter_plan, &tree_sitter)?;
    let ruff = prepare_provider_admission(runs.ruff_plan, &ruff)?;
    let pyrefly = prepare_provider_admission(runs.pyrefly_plan, &pyrefly)?;
    let rustc = prepare_provider_admission(runs.rustc_plan, &rustc)?;

    let (identity, runtime_config, runtime_env, mut assembly) = builder.into_assembly_parts();
    let tree_sitter = register_prepared_programmatic(&mut assembly, tree_sitter)?;
    let ruff = register_prepared_programmatic(&mut assembly, ruff)?;
    let pyrefly = register_prepared_programmatic(&mut assembly, pyrefly)?;
    let rustc = register_prepared_programmatic(&mut assembly, rustc)?;

    Ok(ProgrammaticProviderAdmissionOutcome {
        builder: ProgrammaticFabricEpochBuilder::from_assembly_parts(
            identity,
            runtime_config,
            runtime_env,
            assembly,
        ),
        reports: ExactProgrammaticProviderReports {
            tree_sitter,
            ruff,
            pyrefly,
            rustc,
        },
    })
}

fn register_prepared_programmatic(
    assembly: &mut ProgrammaticSchemaAssembly,
    prepared: PreparedProviderAdmission,
) -> Result<ProviderAdmissionReport, ProviderAdmissionError> {
    for table in prepared.tables {
        let partitions = table.batches.into_iter().map(|batch| vec![batch]).collect();
        let provider = Arc::new(MemTable::try_new(Arc::clone(&table.schema), partitions)?);
        let table_reference = TableReference::full(
            FABRIC_CATALOG,
            table.binding.role.as_str(),
            table.binding.table_name.as_str(),
        );
        let contract = Arc::new(SchemaContract::try_new(
            Arc::clone(&table.binding.source_schema_identity),
            table_reference.clone(),
            Arc::clone(&table.schema),
            Arc::clone(&table.schema),
            (0..table.schema.fields().len())
                .map(|index| FieldIndexMapping::direct(index, index))
                .collect(),
        )?);
        assembly.register_provider(ProviderInput::new(
            ProgrammaticRelationId::new(table.binding.provider_relation.as_str()),
            table_reference,
            contract,
            provider,
        ))?;
    }
    Ok(prepared.report)
}

fn validate_native_syntax_census(
    observed: &AcceptedProviderRelationSet,
) -> Result<(), ProviderAdmissionError> {
    for relation in NativeSyntaxRelation::ALL {
        let identity = ProviderRelationIdentity::try_new(relation.as_str())?;
        if !observed.relations.contains_key(&identity) {
            return Err(ProviderAdmissionError::IncompleteExactProviderRun {
                lane: syntax_lane(relation),
                relation: relation.as_str().to_owned(),
            });
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct NativeSyntaxPartitionAuthority {
    file_id: [u8; 16],
    source_generation: u64,
    content_digest: [u8; 32],
    analysis_context_id: [u8; 32],
    semantic_environment_id: [u8; 32],
    tree_sitter_run_id: [u8; 16],
    ruff_run_id: [u8; 16],
}

fn aggregate_native_syntax_runs(
    runs: &[ProviderNativeSyntaxRun],
    empty_source_pin: SourcePin,
    empty_context_pin: ContextPin,
) -> Result<AcceptedProviderRelationSet, ProviderAdmissionError> {
    enforce_workspace_partition_limit(ProviderNativeLane::TreeSitter, runs.len())?;
    if runs.is_empty() {
        return AcceptedProviderRelationSet::try_new(
            empty_source_pin,
            empty_context_pin,
            Vec::new(),
        );
    }

    let mut authorities = Vec::with_capacity(runs.len());
    let mut relation_sets = Vec::with_capacity(runs.len());
    let mut source_partitions = BTreeSet::new();
    let mut provider_runs = BTreeSet::new();
    for run in runs {
        let relation_set = AcceptedProviderRelationSet::from_native_syntax(run)?;
        validate_native_syntax_census(&relation_set)?;
        let authority = native_syntax_partition_authority(run)?;
        let source_partition = (authority.file_id, authority.source_generation);
        if !source_partitions.insert(source_partition) {
            return Err(ProviderAdmissionError::DuplicateProviderPartition {
                lane: ProviderNativeLane::TreeSitter,
                partition: format!(
                    "{}:{}",
                    hex_bytes(&authority.file_id),
                    authority.source_generation
                ),
            });
        }
        for (lane, run_id) in [
            (ProviderNativeLane::TreeSitter, authority.tree_sitter_run_id),
            (ProviderNativeLane::Ruff, authority.ruff_run_id),
        ] {
            if !provider_runs.insert((lane_name(lane), run_id)) {
                return Err(ProviderAdmissionError::DuplicateProviderPartition {
                    lane,
                    partition: hex_bytes(&run_id),
                });
            }
        }
        authorities.push(authority);
        relation_sets.push(relation_set);
    }

    let first = authorities[0];
    if authorities.iter().any(|authority| {
        authority.analysis_context_id != first.analysis_context_id
            || authority.semantic_environment_id != first.semantic_environment_id
    }) {
        return Err(
            ProviderAdmissionError::InconsistentProviderWorkspaceAuthority {
                lane: ProviderNativeLane::TreeSitter,
            },
        );
    }
    authorities.sort_by_key(|authority| {
        (
            authority.file_id,
            authority.source_generation,
            authority.content_digest,
        )
    });
    let source_pin = native_syntax_workspace_source_pin(&authorities);
    merge_provider_relation_sets(
        &relation_sets,
        source_pin,
        ContextPin(first.analysis_context_id),
    )
}

fn native_syntax_partition_authority(
    run: &ProviderNativeSyntaxRun,
) -> Result<NativeSyntaxPartitionAuthority, ProviderAdmissionError> {
    let tree = syntax_run_authority(run.relation(NativeSyntaxRelation::TreeSitterRun))?;
    let ruff = syntax_run_authority(run.relation(NativeSyntaxRelation::RuffRun))?;
    if tree.file_id != ruff.file_id
        || tree.source_generation != ruff.source_generation
        || tree.content_digest != ruff.content_digest
        || tree.analysis_context_id != ruff.analysis_context_id
        || tree.semantic_environment_id != ruff.semantic_environment_id
    {
        return Err(
            ProviderAdmissionError::InconsistentProviderWorkspaceAuthority {
                lane: ProviderNativeLane::TreeSitter,
            },
        );
    }
    Ok(NativeSyntaxPartitionAuthority {
        file_id: tree.file_id,
        source_generation: tree.source_generation,
        content_digest: tree.content_digest,
        analysis_context_id: tree.analysis_context_id,
        semantic_environment_id: tree.semantic_environment_id,
        tree_sitter_run_id: tree.provider_run_id,
        ruff_run_id: ruff.provider_run_id,
    })
}

#[derive(Clone, Copy)]
struct SyntaxRunAuthority {
    provider_run_id: [u8; 16],
    file_id: [u8; 16],
    source_generation: u64,
    content_digest: [u8; 32],
    analysis_context_id: [u8; 32],
    semantic_environment_id: [u8; 32],
}

fn syntax_run_authority(batch: &RecordBatch) -> Result<SyntaxRunAuthority, ProviderAdmissionError> {
    let relation = batch
        .schema_ref()
        .metadata()
        .get("codefabric.relation_id")
        .cloned()
        .unwrap_or_else(|| "provider.native_syntax.run".to_owned());
    if batch.num_rows() != 1 {
        return Err(ProviderAdmissionError::InvalidObservedRelation {
            relation,
            detail: "run relation must contain exactly one authority row".to_owned(),
        });
    }
    let source_generations = u64_column(batch, "source_generation", &relation)?;
    if source_generations.is_null(0) || source_generations.value(0) == 0 {
        return Err(ProviderAdmissionError::InvalidObservedRelation {
            relation,
            detail: "source_generation is null or uses the zero sentinel".to_owned(),
        });
    }
    let authority = SyntaxRunAuthority {
        provider_run_id: fixed16_value(
            fixed16_column(batch, "provider_run_id", &relation)?,
            0,
            &relation,
            "provider_run_id",
        )?,
        file_id: fixed16_value(
            fixed16_column(batch, "file_id", &relation)?,
            0,
            &relation,
            "file_id",
        )?,
        source_generation: source_generations.value(0),
        content_digest: fixed32_value(
            fixed32_column(batch, "content_digest", &relation)?,
            0,
            &relation,
            "content_digest",
        )?,
        analysis_context_id: fixed32_value(
            fixed32_column(batch, "analysis_context_id", &relation)?,
            0,
            &relation,
            "analysis_context_id",
        )?,
        semantic_environment_id: fixed32_value(
            fixed32_column(batch, "semantic_environment_id", &relation)?,
            0,
            &relation,
            "semantic_environment_id",
        )?,
    };
    require_nonzero(authority.provider_run_id, "native syntax provider run")?;
    require_nonzero(authority.file_id, "native syntax file")?;
    require_nonzero(authority.content_digest, "native syntax content")?;
    require_nonzero(authority.analysis_context_id, "native syntax context")?;
    require_nonzero(
        authority.semantic_environment_id,
        "native syntax semantic environment",
    )?;
    Ok(authority)
}

fn native_syntax_workspace_source_pin(authorities: &[NativeSyntaxPartitionAuthority]) -> SourcePin {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.native-syntax-workspace-source.v1\0");
    hasher.update(
        &u64::try_from(authorities.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    for authority in authorities {
        hasher.update(&authority.file_id);
        hasher.update(&authority.source_generation.to_be_bytes());
        hasher.update(&authority.content_digest);
    }
    SourcePin(*hasher.finalize().as_bytes())
}

fn validate_pyrefly_census(
    run: &AcceptedPyreflyRun,
    observed: &AcceptedProviderRelationSet,
) -> Result<(), ProviderAdmissionError> {
    for module in &run.modules {
        let present = module
            .relations
            .iter()
            .map(|relation| relation.relation)
            .collect::<BTreeSet<_>>();
        for relation in PyreflyRelation::ALL {
            if !present.contains(&relation) {
                return Err(ProviderAdmissionError::IncompleteExactProviderRun {
                    lane: ProviderNativeLane::Pyrefly,
                    relation: format!("{}:{}", module.module_id, relation.relation_id()),
                });
            }
        }
    }
    for relation in PyreflyRelation::ALL {
        let identity = ProviderRelationIdentity::try_new(relation.relation_id())?;
        if !observed.relations.contains_key(&identity) {
            return Err(ProviderAdmissionError::IncompleteExactProviderRun {
                lane: ProviderNativeLane::Pyrefly,
                relation: relation.relation_id().to_owned(),
            });
        }
    }
    Ok(())
}

fn aggregate_pyrefly_runs(
    runs: &[AcceptedPyreflyRun],
    empty_source_pin: SourcePin,
    empty_context_pin: ContextPin,
) -> Result<AcceptedProviderRelationSet, ProviderAdmissionError> {
    enforce_workspace_partition_limit(ProviderNativeLane::Pyrefly, runs.len())?;
    if runs.is_empty() {
        return AcceptedProviderRelationSet::try_new(
            empty_source_pin,
            empty_context_pin,
            Vec::new(),
        );
    }

    let first = &runs[0];
    let mut provider_runs = BTreeSet::new();
    let mut modules = BTreeMap::<String, ([u8; 16], [u8; 32])>::new();
    let mut files = BTreeSet::new();
    let mut relation_sets = Vec::with_capacity(runs.len());
    let mut context_pin = None;
    for run in runs {
        if run.workspace_id != first.workspace_id
            || run.analysis_context_id != first.analysis_context_id
            || run.canonical_workspace_id != first.canonical_workspace_id
            || run.canonical_analysis_context_id != first.canonical_analysis_context_id
            || run.source_generation != first.source_generation
            || run.sandbox_profile_digest != first.sandbox_profile_digest
            || run.trust_profile != first.trust_profile
        {
            return Err(
                ProviderAdmissionError::InconsistentProviderWorkspaceAuthority {
                    lane: ProviderNativeLane::Pyrefly,
                },
            );
        }
        if !provider_runs.insert(run.provider_run_id.as_str()) {
            return Err(ProviderAdmissionError::DuplicateProviderPartition {
                lane: ProviderNativeLane::Pyrefly,
                partition: run.provider_run_id.clone(),
            });
        }
        let relation_set = AcceptedProviderRelationSet::from_pyrefly(run)?;
        validate_pyrefly_census(run, &relation_set)?;
        if context_pin.is_some_and(|pin| pin != relation_set.context_pin) {
            return Err(
                ProviderAdmissionError::InconsistentProviderWorkspaceAuthority {
                    lane: ProviderNativeLane::Pyrefly,
                },
            );
        }
        context_pin = Some(relation_set.context_pin);
        for module in &run.modules {
            if modules.len() == MAX_PROVIDER_WORKSPACE_PARTITIONS {
                return Err(ProviderAdmissionError::ProviderWorkspacePartitionLimit {
                    lane: ProviderNativeLane::Pyrefly,
                    actual: modules.len().saturating_add(1),
                    maximum: MAX_PROVIDER_WORKSPACE_PARTITIONS,
                });
            }
            let digest = *blake3::hash(&module.source_bytes).as_bytes();
            if !files.insert(module.canonical_file_id) {
                return Err(ProviderAdmissionError::DuplicateProviderPartition {
                    lane: ProviderNativeLane::Pyrefly,
                    partition: hex_bytes(&module.canonical_file_id),
                });
            }
            if modules
                .insert(module.module_id.clone(), (module.canonical_file_id, digest))
                .is_some()
            {
                return Err(ProviderAdmissionError::DuplicateProviderPartition {
                    lane: ProviderNativeLane::Pyrefly,
                    partition: module.module_id.clone(),
                });
            }
        }
        relation_sets.push(relation_set);
    }
    let source_pin = pyrefly_workspace_source_pin(first.source_generation, &modules);
    merge_provider_relation_sets(
        &relation_sets,
        source_pin,
        context_pin.expect("a non-empty accepted Pyrefly run has a context pin"),
    )
}

fn pyrefly_workspace_source_pin(
    source_generation: u64,
    modules: &BTreeMap<String, ([u8; 16], [u8; 32])>,
) -> SourcePin {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.pyrefly.source-pin.v1\0");
    hasher.update(&source_generation.to_be_bytes());
    for (module_id, (file_id, digest)) in modules {
        hasher.update(
            &u64::try_from(module_id.len())
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
        hasher.update(module_id.as_bytes());
        hasher.update(file_id);
        hasher.update(digest);
    }
    SourcePin(*hasher.finalize().as_bytes())
}

fn aggregate_rustc_runs(
    runs: &[TrustQualifiedRustcCompilation],
    empty_source_pin: SourcePin,
    empty_context_pin: ContextPin,
) -> Result<AcceptedProviderRelationSet, ProviderAdmissionError> {
    enforce_workspace_partition_limit(ProviderNativeLane::Rustc, runs.len())?;
    if runs.is_empty() {
        return AcceptedProviderRelationSet::try_new(
            empty_source_pin,
            empty_context_pin,
            Vec::new(),
        );
    }

    let first = &runs[0].accepted().admission;
    let mut provider_runs = BTreeSet::new();
    let mut compilation_units = BTreeSet::new();
    let mut relation_sets = Vec::with_capacity(runs.len());
    let mut source_pin = None;
    let mut context_pin = None;
    for qualified in runs {
        qualified
            .validate()
            .map_err(|error| ProviderAdmissionError::RustcTrustEvidence(error.to_string()))?;
        let run = qualified.accepted();
        if run.admission.workspace_id != first.workspace_id
            || run.admission.analysis_context_id != first.analysis_context_id
            || run.admission.canonical_workspace_id != first.canonical_workspace_id
            || run.admission.canonical_analysis_context_id != first.canonical_analysis_context_id
            || run.admission.source_generation != first.source_generation
            || run.admission.context_manifest_digest != first.context_manifest_digest
            || run.admission.source_snapshot_manifest_digest
                != first.source_snapshot_manifest_digest
            || run.admission.resource_profile_id != first.resource_profile_id
        {
            return Err(
                ProviderAdmissionError::InconsistentProviderWorkspaceAuthority {
                    lane: ProviderNativeLane::Rustc,
                },
            );
        }
        if !provider_runs.insert(run.admission.provider_run_id.as_str()) {
            return Err(ProviderAdmissionError::DuplicateProviderPartition {
                lane: ProviderNativeLane::Rustc,
                partition: run.admission.provider_run_id.clone(),
            });
        }
        if !compilation_units.insert(run.begin.compilation_unit_id.as_str()) {
            return Err(ProviderAdmissionError::DuplicateProviderPartition {
                lane: ProviderNativeLane::Rustc,
                partition: run.begin.compilation_unit_id.clone(),
            });
        }
        let relation_set = AcceptedProviderRelationSet::from_rustc(run)?;
        if source_pin.is_some_and(|pin| pin != relation_set.source_pin)
            || context_pin.is_some_and(|pin| pin != relation_set.context_pin)
        {
            return Err(
                ProviderAdmissionError::InconsistentProviderWorkspaceAuthority {
                    lane: ProviderNativeLane::Rustc,
                },
            );
        }
        source_pin = Some(relation_set.source_pin);
        context_pin = Some(relation_set.context_pin);
        relation_sets.push(relation_set);
    }
    merge_provider_relation_sets(
        &relation_sets,
        source_pin.expect("a non-empty rustc set has a source pin"),
        context_pin.expect("a non-empty rustc set has a context pin"),
    )
}

fn merge_provider_relation_sets(
    sets: &[AcceptedProviderRelationSet],
    source_pin: SourcePin,
    context_pin: ContextPin,
) -> Result<AcceptedProviderRelationSet, ProviderAdmissionError> {
    let mut grouped =
        BTreeMap::<ProviderRelationIdentity, (ProviderNativeLane, Vec<RecordBatch>)>::new();
    for set in sets {
        for relation in set.relations.values() {
            let entry = grouped
                .entry(relation.identity.clone())
                .or_insert_with(|| (relation.lane, Vec::new()));
            if entry.0 != relation.lane {
                return Err(ProviderAdmissionError::ProviderLaneMismatch {
                    relation: relation.identity.as_str().to_owned(),
                    expected: entry.0,
                    actual: relation.lane,
                });
            }
            let partition_count = entry.1.len().saturating_add(relation.batches.len());
            enforce_workspace_partition_limit(relation.lane, partition_count)?;
            entry.1.extend(relation.batches.iter().cloned());
        }
    }
    AcceptedProviderRelationSet::try_new(
        source_pin,
        context_pin,
        grouped
            .into_iter()
            .map(|(identity, (lane, batches))| ObservedProviderRelation {
                identity,
                lane,
                batches,
            })
            .collect(),
    )
}

fn enforce_workspace_partition_limit(
    lane: ProviderNativeLane,
    actual: usize,
) -> Result<(), ProviderAdmissionError> {
    if actual > MAX_PROVIDER_WORKSPACE_PARTITIONS {
        return Err(ProviderAdmissionError::ProviderWorkspacePartitionLimit {
            lane,
            actual,
            maximum: MAX_PROVIDER_WORKSPACE_PARTITIONS,
        });
    }
    Ok(())
}

fn provider_lane_subset(
    observed: &AcceptedProviderRelationSet,
    lane: ProviderNativeLane,
) -> Result<AcceptedProviderRelationSet, ProviderAdmissionError> {
    if let Some(gap) = observed.gap {
        return AcceptedProviderRelationSet::from_gap(
            observed.source_pin,
            observed.context_pin,
            gap,
        );
    }
    let relations = observed
        .relations
        .values()
        .filter(|relation| relation.lane == lane)
        .cloned()
        .collect::<Vec<_>>();
    AcceptedProviderRelationSet::try_new(observed.source_pin, observed.context_pin, relations)
}

const fn lane_name(lane: ProviderNativeLane) -> u8 {
    match lane {
        ProviderNativeLane::TreeSitter => 1,
        ProviderNativeLane::Ruff => 2,
        ProviderNativeLane::Pyrefly => 3,
        ProviderNativeLane::Rustc => 4,
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn validate_programmatic_transaction_plans(
    plans: &[(ProviderNativeLane, &ProviderAdmissionPlan)],
) -> Result<(), ProviderAdmissionError> {
    let mut relations = BTreeMap::<ProviderRelationIdentity, ProviderNativeLane>::new();
    let mut tables = BTreeMap::<(FabricSchemaRole, String), ProviderNativeLane>::new();
    for (lane, plan) in plans {
        validate_admission_plan(plan)?;
        for binding in &plan.bindings {
            if binding.lane != *lane {
                return Err(ProviderAdmissionError::ProviderLaneMismatch {
                    relation: binding.provider_relation.as_str().to_owned(),
                    expected: *lane,
                    actual: binding.lane,
                });
            }
            if let Some(first) = relations.insert(binding.provider_relation.clone(), *lane) {
                return Err(ProviderAdmissionError::CrossProviderRelationIdentity {
                    relation: binding.provider_relation.as_str().to_owned(),
                    first,
                    second: *lane,
                });
            }
            let table = (binding.role, binding.table_name.clone());
            if let Some(first) = tables.insert(table.clone(), *lane) {
                return Err(ProviderAdmissionError::CrossProviderTableBinding {
                    role: table.0,
                    table: table.1,
                    first,
                    second: *lane,
                });
            }
        }
    }
    Ok(())
}

fn validate_provider_capability_catalog_binding(
    binding: &ProviderCapabilityCatalogBinding,
) -> Result<(), ProviderAdmissionError> {
    if binding.table_name.is_empty()
        || binding.table_name.trim() != binding.table_name
        || binding.table_name.len() > MAX_RELATION_NAME_BYTES
        || binding.table_name.chars().any(char::is_control)
        || binding.source_schema_identity.trim().is_empty()
    {
        return Err(ProviderAdmissionError::InvalidPlan(
            "provider capability catalog binding is invalid".into(),
        ));
    }
    Ok(())
}

fn programmatic_provider_capabilities(
    reports: &ExactProgrammaticProviderReports,
    proof_relations: &ProofRelations,
    oracle_bindings: &[ProviderOracleProofBinding],
) -> Result<Vec<ProviderCapabilityRelation>, ProviderAdmissionError> {
    let reports = [
        reports.tree_sitter(),
        reports.ruff(),
        reports.pyrefly(),
        reports.rustc(),
    ];
    let mut owners = BTreeMap::new();
    for (report_index, report) in reports.iter().enumerate() {
        for family in &report.boundary.families {
            if owners
                .insert((family.oracle_id, family.relation_id), report_index)
                .is_some()
            {
                return Err(ProviderCapabilityError::DuplicateBoundaryFamily.into());
            }
        }
    }

    let mut seen_bindings = BTreeSet::new();
    let mut bindings_by_report: [Vec<ProviderOracleProofBinding>; 4] =
        std::array::from_fn(|_| Vec::new());
    for binding in oracle_bindings {
        let key = (binding.provider_oracle_id, binding.relation_id);
        if !seen_bindings.insert(key) {
            return Err(ProviderCapabilityError::DuplicateProofBinding.into());
        }
        let report_index = owners
            .get(&key)
            .copied()
            .ok_or(ProviderCapabilityError::UnboundProofBinding)?;
        bindings_by_report[report_index].push(*binding);
    }

    reports
        .into_iter()
        .zip(bindings_by_report)
        .map(|(report, bindings)| {
            let proofs = provider_oracle_proofs_from_executable_relations(
                &report.boundary,
                proof_relations,
                &bindings,
            )?;
            if proofs.iter().any(|proof| {
                proof.proof_epoch_id != *proof_relations.candidate_pins().epoch.as_bytes()
            }) {
                return Err(ProviderAdmissionError::CapabilityProofEpochMismatch);
            }
            Ok(derive_provider_capability_relation(
                &report.boundary,
                &proofs,
            )?)
        })
        .collect()
}

fn register_programmatic_provider_capabilities(
    outcome: ProgrammaticProviderAdmissionOutcome,
    binding: &ProviderCapabilityCatalogBinding,
    capabilities: Vec<ProviderCapabilityRelation>,
) -> Result<ProviderCapabilityAdmissionOutcome, ProviderAdmissionError> {
    validate_provider_capability_catalog_binding(binding)?;
    let schema = capabilities
        .first()
        .map(ProviderCapabilityRelation::schema)
        .cloned()
        .ok_or_else(|| {
            ProviderAdmissionError::InvalidPlan(
                "programmatic provider capability set is empty".into(),
            )
        })?;
    if capabilities
        .iter()
        .any(|capability| capability.schema().as_ref() != schema.as_ref())
    {
        return Err(ProviderAdmissionError::SchemaMismatch {
            relation: "system.provider_capability.v1".to_owned(),
        });
    }
    let provider = Arc::new(MemTable::try_new(
        Arc::clone(&schema),
        capabilities
            .iter()
            .map(|capability| vec![capability.batch().clone()])
            .collect(),
    )?);
    let table_reference = TableReference::full(
        FABRIC_CATALOG,
        FabricSchemaRole::System.as_str(),
        binding.table_name.as_str(),
    );
    let contract = Arc::new(SchemaContract::try_new(
        Arc::clone(&binding.source_schema_identity),
        table_reference.clone(),
        Arc::clone(&schema),
        Arc::clone(&schema),
        (0..schema.fields().len())
            .map(|index| FieldIndexMapping::direct(index, index))
            .collect(),
    )?);
    let relation_id =
        ProgrammaticRelationId::new(contract.relation_id(SchemaRole::Logical)?.to_owned());
    let ProgrammaticProviderAdmissionOutcome { builder, reports } = outcome;
    let (identity, runtime_config, runtime_env, mut assembly) = builder.into_assembly_parts();
    assembly.register_provider(ProviderInput::new(
        relation_id,
        table_reference,
        contract,
        provider,
    ))?;
    Ok(ProviderCapabilityAdmissionOutcome {
        builder: ProgrammaticFabricEpochBuilder::from_assembly_parts(
            identity,
            runtime_config,
            runtime_env,
            assembly,
        ),
        provider_reports: reports,
        capabilities,
    })
}

/// Register provider capability derived only from the executable proof engine's sealed output.
///
/// This is the production path: application-owned bindings join all four exact provider reports
/// to proof oracles, exact proof/candidate pins are carried into the receipt relation, and missing
/// oracle execution remains missing proof rather than an optimistic capability.
///
/// # Errors
///
/// Returns binding, proof-pin, schema, or catalog-registration failures without returning a
/// partially mutated epoch candidate.
pub fn admit_provider_capability_from_proof_relations(
    outcome: ProgrammaticProviderAdmissionOutcome,
    catalog_binding: &ProviderCapabilityCatalogBinding,
    proof_relations: &ProofRelations,
    oracle_bindings: &[ProviderOracleProofBinding],
) -> Result<ProviderCapabilityAdmissionOutcome, ProviderAdmissionError> {
    if proof_relations.candidate_pins().epoch != *outcome.candidate_epoch_id() {
        return Err(ProviderAdmissionError::CapabilityProofEpochMismatch);
    }
    let capabilities =
        programmatic_provider_capabilities(outcome.reports(), proof_relations, oracle_bindings)?;
    register_programmatic_provider_capabilities(outcome, catalog_binding, capabilities)
}

fn prepare_provider_admission(
    plan: &ProviderAdmissionPlan,
    observed: &AcceptedProviderRelationSet,
) -> Result<PreparedProviderAdmission, ProviderAdmissionError> {
    validate_admission_plan(plan)?;
    if observed.source_pin != plan.expected_source_pin
        || observed.context_pin != plan.expected_context_pin
    {
        return Err(ProviderAdmissionError::AdmissionPinMismatch);
    }

    let rows = plan
        .contract
        .rows
        .iter()
        .map(|row| (row.api_family.clone(), row))
        .collect::<BTreeMap<_, _>>();
    let bindings_by_relation = plan
        .bindings
        .iter()
        .map(|binding| (binding.provider_relation.clone(), binding))
        .collect::<BTreeMap<_, _>>();
    for identity in observed.relations.keys() {
        if !bindings_by_relation.contains_key(identity) {
            return Err(ProviderAdmissionError::UnexpectedProviderRelation(
                identity.as_str().to_owned(),
            ));
        }
    }

    let mut surfaces = Vec::new();
    let mut requests = Vec::with_capacity(plan.bindings.len());
    let mut coverage = Vec::new();
    let mut coverage_present = BTreeSet::new();
    let mut assembled = Vec::new();
    for binding in &plan.bindings {
        let row = rows.get(&binding.api_family).copied().ok_or_else(|| {
            ProviderAdmissionError::InvalidPlan(format!(
                "binding {} has no accepted boundary row",
                binding.api_family.as_str()
            ))
        })?;
        let actual = observed.relations.get(&binding.provider_relation);
        enforce_authority(binding, row.authority, row.disposition, actual.is_some())?;
        requests.push(ProviderFamilyRequest {
            api_family: binding.api_family.clone(),
            requested_units: binding.requested_units,
        });

        if let Some(actual) = actual {
            if actual.lane != binding.lane {
                return Err(ProviderAdmissionError::ProviderLaneMismatch {
                    relation: binding.provider_relation.as_str().to_owned(),
                    expected: binding.lane,
                    actual: actual.lane,
                });
            }
            if actual
                .batches
                .iter()
                .any(|batch| batch.schema_ref().as_ref() != row.relation.schema.as_ref())
            {
                return Err(ProviderAdmissionError::SchemaMismatch {
                    relation: binding.provider_relation.as_str().to_owned(),
                });
            }
            surfaces.push(InstalledProviderSurface {
                installer_id: plan.installer.installer_id,
                handler_id: binding.handler_id,
                api_family: binding.api_family.clone(),
                upstream_symbols: row.upstream_symbols.clone(),
                relation_id: row.relation.relation_id,
                schema_fingerprint: row.relation.schema_fingerprint,
                schema: actual.batches[0].schema(),
            });
        }

        // A missing required relation is always an explicit unknown. A provider's stale or
        // contradictory coverage row cannot turn absent Arrow output into completion.
        let family_coverage = if actual.is_none() {
            observed.gap.map(|gap| provider_gap_trailer(binding, gap))
        } else {
            coverage_for_binding(binding, observed)?
        };
        if let Some(trailer) = family_coverage.clone() {
            coverage_present.insert(binding.api_family.clone());
            coverage.push(ProviderFamilyCoverage {
                api_family: binding.api_family.clone(),
                trailer,
            });
        }

        if let Some(actual) = actual {
            let trailer = family_coverage
                .unwrap_or_else(|| conservative_unknown_trailer(binding.requested_units));
            assembled.push(AssembledRelation {
                identity: StreamIdentity {
                    relation_id: row.relation.relation_id,
                    stream_id: stream_id(
                        row.relation.relation_id,
                        observed.source_pin,
                        observed.context_pin,
                    ),
                    schema_fingerprint: row.relation.schema_fingerprint,
                    source_pin: observed.source_pin,
                    context_pin: observed.context_pin,
                },
                schema: actual.batches[0].schema(),
                batches: actual.batches.clone(),
                ipc_bytes: Vec::new(),
                trailer,
            });
        }
    }

    let boundary = evaluate_provider_boundary(
        &plan.contract,
        &plan.installer,
        ProviderBoundaryEvidence {
            expected_source_pin: plan.expected_source_pin,
            expected_context_pin: plan.expected_context_pin,
            installed_surfaces: &surfaces,
            requested_families: &requests,
            family_coverage: &coverage,
            relations: &assembled,
        },
    )?;

    let mut tables = Vec::new();
    let mut relation_reports = Vec::with_capacity(plan.bindings.len());
    for binding in &plan.bindings {
        let family = boundary
            .families
            .iter()
            .find(|family| family.api_family == binding.api_family)
            .ok_or_else(|| {
                ProviderAdmissionError::InvalidPlan(format!(
                    "boundary evaluator omitted requested family {}",
                    binding.api_family.as_str()
                ))
            })?;
        let actual = observed.relations.get(&binding.provider_relation);
        let (disposition, register) = match &family.run {
            ProviderFamilyRunOutcome::Complete { .. } => (
                ProviderRegistrationDisposition::Registered {
                    row_count: relation_row_count(actual),
                    coverage: TerminalStatus::Complete,
                },
                true,
            ),
            ProviderFamilyRunOutcome::Partial { .. } if actual.is_some() => (
                ProviderRegistrationDisposition::Registered {
                    row_count: relation_row_count(actual),
                    coverage: TerminalStatus::Partial,
                },
                true,
            ),
            ProviderFamilyRunOutcome::Partial { trailer } => (
                ProviderRegistrationDisposition::Remainder {
                    trailer: trailer.clone(),
                },
                false,
            ),
            ProviderFamilyRunOutcome::Unknown { .. } => {
                let cause = if actual.is_none() {
                    ProviderAdmissionUnknownCause::MissingRelation
                } else if !coverage_present.contains(&binding.api_family) {
                    ProviderAdmissionUnknownCause::MissingCoverage
                } else {
                    ProviderAdmissionUnknownCause::ProviderDeclared
                };
                if actual.is_some() && coverage_present.contains(&binding.api_family) {
                    (
                        ProviderRegistrationDisposition::RegisteredUnknown {
                            row_count: relation_row_count(actual),
                            cause,
                        },
                        true,
                    )
                } else {
                    (ProviderRegistrationDisposition::Unknown { cause }, false)
                }
            }
            ProviderFamilyRunOutcome::NotRequested => {
                return Err(ProviderAdmissionError::InvalidPlan(format!(
                    "binding {} was not evaluated as requested",
                    binding.api_family.as_str()
                )));
            }
        };
        if register {
            let actual = actual.ok_or_else(|| {
                ProviderAdmissionError::InvalidPlan(format!(
                    "completed family {} has no relation",
                    binding.api_family.as_str()
                ))
            })?;
            tables.push(PreparedProviderTable {
                binding: binding.clone(),
                schema: actual.batches[0].schema(),
                batches: actual.batches.clone(),
            });
        }
        relation_reports.push(ProviderRelationAdmission {
            provider_relation: binding.provider_relation.clone(),
            api_family: binding.api_family.clone(),
            disposition,
            lane_gap: observed.gap,
        });
    }

    Ok(PreparedProviderAdmission {
        report: ProviderAdmissionReport {
            boundary,
            relations: relation_reports,
        },
        tables,
    })
}

fn validate_admission_plan(plan: &ProviderAdmissionPlan) -> Result<(), ProviderAdmissionError> {
    if plan.provider_kind.is_empty()
        || plan.provider_kind.trim() != plan.provider_kind.as_ref()
        || plan.provider_kind.chars().any(char::is_control)
    {
        return Err(ProviderAdmissionError::InvalidPlan(
            "provider kind is invalid".into(),
        ));
    }
    require_nonzero(plan.expected_source_pin.0, "expected source pin")?;
    require_nonzero(plan.expected_context_pin.0, "expected context pin")?;
    validate_provider_boundary_contract(&plan.contract, &plan.installer)?;
    if plan.bindings.is_empty() || plan.bindings.len() > MAX_ADMISSION_BINDINGS {
        return Err(ProviderAdmissionError::InvalidPlan(
            "admission binding count is empty or exceeds its resource bound".into(),
        ));
    }

    let mut identities = BTreeSet::new();
    let mut families = BTreeSet::new();
    let mut tables = BTreeSet::new();
    for binding in &plan.bindings {
        if !identities.insert(binding.provider_relation.clone()) {
            return Err(ProviderAdmissionError::InvalidPlan(
                "duplicate provider relation binding".into(),
            ));
        }
        if !families.insert(binding.api_family.clone()) {
            return Err(ProviderAdmissionError::InvalidPlan(
                "duplicate provider family binding".into(),
            ));
        }
        if !tables.insert((binding.role, binding.table_name.clone())) {
            return Err(ProviderAdmissionError::InvalidPlan(
                "duplicate epoch table binding".into(),
            ));
        }
        if binding.table_name.is_empty()
            || binding.source_schema_identity.trim().is_empty()
            || binding.requested_units == 0
            || binding.handler_id.0 == [0; 16]
        {
            return Err(ProviderAdmissionError::InvalidPlan(format!(
                "binding {} has an empty name/identity, zero handler, or zero request",
                binding.provider_relation.as_str()
            )));
        }
        match (&binding.purpose, &binding.coverage) {
            (ProviderRelationPurpose::SemanticFact, ProviderCoverageSource::StructuralPresence) => {
                return Err(ProviderAdmissionError::InvalidPlan(format!(
                    "semantic family {} cannot claim coverage from relation presence",
                    binding.api_family.as_str()
                )));
            }
            (_, ProviderCoverageSource::ProviderDeclared(declared)) => {
                validate_declared_coverage_binding(declared)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn enforce_authority(
    binding: &ProviderRelationBinding,
    authority: ProviderAuthorityRole,
    disposition: ContractDisposition,
    emitted: bool,
) -> Result<(), ProviderAdmissionError> {
    if binding.authority_class.forbids_provider_native() {
        if authority != ProviderAuthorityRole::ForbiddenProviderNative
            || disposition
                != (ContractDisposition::IntentionalRemainder {
                    reason: RemainderReason::Unsupported,
                })
        {
            return Err(ProviderAdmissionError::ForbiddenAuthorityClaim {
                class: binding.authority_class,
                family: binding.api_family.as_str().to_owned(),
            });
        }
        if emitted {
            return Err(ProviderAdmissionError::ForbiddenProviderOutput(
                binding.api_family.as_str().to_owned(),
            ));
        }
    } else if binding.role != binding.lane.raw_role() {
        return Err(ProviderAdmissionError::InvalidPlan(format!(
            "provider-native relation {} must remain queryable in {}",
            binding.provider_relation.as_str(),
            binding.lane.raw_role().as_str()
        )));
    }
    Ok(())
}

fn validate_declared_coverage_binding(
    binding: &DeclaredCoverageBinding,
) -> Result<(), ProviderAdmissionError> {
    let names = [
        binding.family_column.as_str(),
        binding.requested_units_column.as_str(),
        binding.completed_units_column.as_str(),
        binding.status_column.as_str(),
    ]
    .into_iter()
    .chain(binding.remainder_reason_column.as_deref())
    .chain(binding.unknown_semantics_column.as_deref());
    if binding.family_value.trim().is_empty()
        || names.clone().any(|name| {
            name.is_empty()
                || name.trim() != name
                || name.len() > MAX_RELATION_NAME_BYTES
                || name.chars().any(char::is_control)
        })
        || binding.remainder_reason_map.keys().any(|reason| {
            reason.is_empty()
                || reason.trim() != reason
                || reason.len() > MAX_RELATION_NAME_BYTES
                || reason.chars().any(char::is_control)
        })
    {
        return Err(ProviderAdmissionError::InvalidPlan(
            "declared coverage binding contains an invalid family, field, or reason".into(),
        ));
    }
    Ok(())
}

fn coverage_for_binding(
    binding: &ProviderRelationBinding,
    observed: &AcceptedProviderRelationSet,
) -> Result<Option<CoverageTrailer>, ProviderAdmissionError> {
    match &binding.coverage {
        ProviderCoverageSource::StructuralPresence => Ok(observed
            .relations
            .contains_key(&binding.provider_relation)
            .then(|| CoverageTrailer::complete(binding.requested_units))),
        ProviderCoverageSource::ProviderDeclared(declared) => {
            declared_coverage(binding, declared, observed)
        }
    }
}

fn declared_coverage(
    binding: &ProviderRelationBinding,
    declared: &DeclaredCoverageBinding,
    observed: &AcceptedProviderRelationSet,
) -> Result<Option<CoverageTrailer>, ProviderAdmissionError> {
    let Some(coverage_relation) = observed.relations.get(&declared.relation_identity) else {
        return Ok(None);
    };
    let relation_name = declared.relation_identity.as_str();
    let mut matched = false;
    let mut requested_units = 0_u64;
    let mut completed_units = 0_u64;
    let mut remainders = Vec::new();
    let mut any_unknown = false;
    let mut any_partial = false;
    for (batch_index, batch) in coverage_relation.batches.iter().enumerate() {
        let families = coverage_utf8_column(batch, &declared.family_column, relation_name)?;
        let requested =
            coverage_u64_column(batch, &declared.requested_units_column, relation_name)?;
        let completed =
            coverage_u64_column(batch, &declared.completed_units_column, relation_name)?;
        let statuses = coverage_utf8_column(batch, &declared.status_column, relation_name)?;
        let reasons = declared
            .remainder_reason_column
            .as_deref()
            .map(|field| coverage_utf8_column(batch, field, relation_name))
            .transpose()?;
        let unknowns = declared
            .unknown_semantics_column
            .as_deref()
            .map(|field| coverage_bool_column(batch, field, relation_name))
            .transpose()?;

        for row in 0..batch.num_rows() {
            if families.is_null(row) || families.value(row) != declared.family_value {
                continue;
            }
            matched = true;
            if requested.is_null(row) || completed.is_null(row) || statuses.is_null(row) {
                return Err(coverage_error(
                    relation_name,
                    "family/requested/completed/status contains null",
                ));
            }
            let row_requested = requested.value(row);
            let provider_completed = completed.value(row);
            if row_requested == 0 || provider_completed > row_requested {
                return Err(coverage_error(
                    relation_name,
                    "requested/completed units are invalid",
                ));
            }
            // Provider-native rows retain their exact vocabulary in the registered Arrow
            // relation. Admission maps that vocabulary only into the application-owned terminal
            // state used by the boundary evaluator. rustc deliberately distinguishes
            // characterized incompleteness from a generic partial/unknown label.
            let raw_status = statuses.value(row);
            let status = provider_terminal_status(raw_status).ok_or_else(|| {
                coverage_error(
                    relation_name,
                    "terminal vocabulary is not registered by the application",
                )
            })?;
            let unknown_semantics =
                unknowns.is_some_and(|values| !values.is_null(row) && values.value(row));
            if unknowns.is_some_and(|values| values.is_null(row)) {
                return Err(coverage_error(
                    relation_name,
                    "unknown-semantics field is null",
                ));
            }
            let status_unknown = status == TerminalStatus::Unknown;
            let effective_unknown = status_unknown || unknown_semantics;
            if status == TerminalStatus::Complete
                && (provider_completed != row_requested || effective_unknown)
            {
                return Err(coverage_error(
                    relation_name,
                    "terminal vocabulary or complete claim is invalid",
                ));
            }
            let effective_completed = if effective_unknown {
                0
            } else {
                provider_completed
            };
            if status == TerminalStatus::Partial
                && !effective_unknown
                && effective_completed == row_requested
            {
                return Err(coverage_error(
                    relation_name,
                    "partial claim has no incomplete unit",
                ));
            }
            let missing = row_requested - effective_completed;
            let provider_reason =
                reasons.and_then(|values| (!values.is_null(row)).then(|| values.value(row)));
            if missing > 0 {
                let reason = if effective_unknown {
                    RemainderReason::Unknown
                } else {
                    let provider_reason = provider_reason.ok_or_else(|| {
                        coverage_error(relation_name, "partial row lacks a remainder reason")
                    })?;
                    *declared
                        .remainder_reason_map
                        .get(provider_reason)
                        .ok_or_else(|| {
                            coverage_error(
                                relation_name,
                                "provider remainder reason has no accepted model mapping",
                            )
                        })?
                };
                remainders.push(CoverageRemainder {
                    scope: coverage_scope(
                        &binding.api_family,
                        &declared.relation_identity,
                        batch_index,
                        row,
                    ),
                    unit_count: missing,
                    reason,
                });
            } else if provider_reason.is_some() {
                return Err(coverage_error(
                    relation_name,
                    "complete row carries an unexplained remainder reason",
                ));
            }
            requested_units = requested_units.checked_add(row_requested).ok_or_else(|| {
                coverage_error(relation_name, "requested-unit aggregation overflow")
            })?;
            completed_units = completed_units
                .checked_add(effective_completed)
                .ok_or_else(|| {
                    coverage_error(relation_name, "completed-unit aggregation overflow")
                })?;
            any_unknown |= effective_unknown;
            any_partial |= status == TerminalStatus::Partial;
        }
    }
    if !matched {
        return Ok(None);
    }
    if requested_units != binding.requested_units {
        return Err(coverage_error(
            relation_name,
            "provider requested units differ from the accepted model request",
        ));
    }
    let status = if any_unknown {
        TerminalStatus::Unknown
    } else if any_partial || !remainders.is_empty() {
        TerminalStatus::Partial
    } else {
        TerminalStatus::Complete
    };
    Ok(Some(CoverageTrailer {
        status,
        requested_units,
        completed_units,
        remainders,
    }))
}

/// Map a provider-native completeness label into the application terminal state without
/// rewriting the provider relation that carries the label.
///
/// The two characterized labels are part of the exact rustc adapter contract. An unregistered
/// label is a schema/protocol contradiction, never an implicit unknown or empty success.
fn provider_terminal_status(value: &str) -> Option<TerminalStatus> {
    match value {
        "complete" => Some(TerminalStatus::Complete),
        "partial" | "partial-characterized" => Some(TerminalStatus::Partial),
        "unknown" | "unavailable-characterized" => Some(TerminalStatus::Unknown),
        _ => None,
    }
}

fn coverage_utf8_column<'a>(
    batch: &'a RecordBatch,
    field: &str,
    relation: &str,
) -> Result<&'a StringArray, ProviderAdmissionError> {
    typed_column::<StringArray>(batch, field, relation, "Utf8")
        .map_err(|_| coverage_error(relation, &format!("{field} is missing or not Utf8")))
}

fn coverage_u64_column<'a>(
    batch: &'a RecordBatch,
    field: &str,
    relation: &str,
) -> Result<&'a UInt64Array, ProviderAdmissionError> {
    typed_column::<UInt64Array>(batch, field, relation, "UInt64")
        .map_err(|_| coverage_error(relation, &format!("{field} is missing or not UInt64")))
}

fn coverage_bool_column<'a>(
    batch: &'a RecordBatch,
    field: &str,
    relation: &str,
) -> Result<&'a BooleanArray, ProviderAdmissionError> {
    bool_column(batch, field, relation)
        .map_err(|_| coverage_error(relation, &format!("{field} is missing or not Boolean")))
}

fn coverage_error(relation: &str, detail: &str) -> ProviderAdmissionError {
    ProviderAdmissionError::InvalidCoverage {
        relation: relation.to_owned(),
        detail: detail.to_owned(),
    }
}

fn coverage_scope(
    family: &ProviderApiFamily,
    relation: &ProviderRelationIdentity,
    batch: usize,
    row: usize,
) -> CoverageScope {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.provider-coverage-scope.v1\0");
    hasher.update(family.as_str().as_bytes());
    hasher.update(&[0]);
    hasher.update(relation.as_str().as_bytes());
    hasher.update(&u64::try_from(batch).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(&u64::try_from(row).unwrap_or(u64::MAX).to_be_bytes());
    let digest = hasher.finalize();
    let mut scope = [0_u8; 16];
    scope.copy_from_slice(&digest.as_bytes()[..16]);
    CoverageScope(scope)
}

fn conservative_unknown_trailer(requested_units: u64) -> CoverageTrailer {
    CoverageTrailer {
        status: TerminalStatus::Unknown,
        requested_units,
        completed_units: 0,
        remainders: vec![CoverageRemainder {
            scope: CoverageScope([0xff; 16]),
            unit_count: requested_units,
            reason: RemainderReason::Unknown,
        }],
    }
}

fn provider_gap_trailer(
    binding: &ProviderRelationBinding,
    gap: ProviderLaneGap,
) -> CoverageTrailer {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.provider-lane-gap-scope.v1\0");
    hasher.update(binding.api_family.as_str().as_bytes());
    hasher.update(&[0]);
    hasher.update(binding.provider_relation.as_str().as_bytes());
    hasher.update(&[provider_lane_gap_code(gap)]);
    let digest = hasher.finalize();
    let mut scope = [0_u8; 16];
    scope.copy_from_slice(&digest.as_bytes()[..16]);
    CoverageTrailer {
        status: gap.terminal_status(),
        requested_units: binding.requested_units,
        completed_units: 0,
        remainders: vec![CoverageRemainder {
            scope: CoverageScope(scope),
            unit_count: binding.requested_units,
            reason: gap.remainder_reason(),
        }],
    }
}

const fn provider_lane_gap_code(gap: ProviderLaneGap) -> u8 {
    match gap {
        ProviderLaneGap::RequiredInputAbsent => 1,
        ProviderLaneGap::OptionalInputAbsent => 2,
        ProviderLaneGap::ProviderFailure => 3,
        ProviderLaneGap::CompilationFailure => 4,
        ProviderLaneGap::TrustUnavailable => 5,
        ProviderLaneGap::ResourceLimit => 6,
        ProviderLaneGap::TimedOut => 7,
        ProviderLaneGap::Cancelled => 8,
        ProviderLaneGap::InvalidSource => 9,
        ProviderLaneGap::Unsupported => 10,
    }
}

fn stream_id(relation: RelationId, source: SourcePin, context: ContextPin) -> StreamId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.provider-admission-stream.v1\0");
    hasher.update(&relation.0);
    hasher.update(&source.0);
    hasher.update(&context.0);
    let digest = hasher.finalize();
    let mut value = [0_u8; 16];
    value.copy_from_slice(&digest.as_bytes()[..16]);
    StreamId(value)
}

fn relation_row_count(relation: Option<&ObservedProviderRelation>) -> usize {
    relation
        .into_iter()
        .flat_map(|relation| &relation.batches)
        .map(RecordBatch::num_rows)
        .sum()
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::Path;

    use arrow_array::ArrayRef;
    use arrow_array::builder::FixedSizeBinaryBuilder;
    use arrow_ipc::writer::StreamWriter;
    use arrow_schema::{DataType, Field, Schema};

    use super::*;
    use crate::cancellation::Cancellation;
    use crate::fabric::epoch_runtime::{FabricEpochId, FabricEpochRuntimeConfig, FabricSchemaRole};
    use crate::fabric::production_kernel::CompiledSemanticRelease;
    use crate::fabric::proof::{
        OracleId, OracleImplementationRef, ProofRunId, ProofTerminalStatus,
        test_relations_with_oracle,
    };
    use crate::provider_boundary::{
        BoundaryContractId, BoundaryOwnerId, CanonicalIdentityRole, ContractDisposition,
        CoordinateRole, FieldMeaning, IndependentContractAcceptance, ProviderArrowRelationContract,
        ProviderBoundaryContractRow, ProviderBoundaryField, ProviderId, ProviderInstallerId,
        ProviderLocalIdentityRole, ProviderOracleId, ProviderRevision, RetentionPolicy,
        UnavailableBehavior, UpstreamApiSymbol,
    };
    use crate::provider_native_syntax::{
        ExactPythonSyntaxRunner, ProviderNativeSourceImage, PythonModuleInput, PythonSyntaxRunPins,
        SyntaxProviderRunPin,
    };
    use crate::provider_types::ProviderText;
    use crate::pyrefly_service::AcceptedPyreflyRelation;
    use crate::relation_ipc::{SchemaFingerprint, TerminalStatus};
    use crate::rpc::generated::codefabric::provider::v1::ProviderRunState;
    use crate::rpc::generated::codefabric::rustc::v1::{
        CompilationBegin, CompilationEnd, CompilerOwnerKey, OwnerBegin, OwnerEnd,
    };
    use crate::rustc_service::{
        AcceptedRustcCompilation, AcceptedRustcOwner, AcceptedRustcRelation, RustcRunAdmission,
        TrustQualifiedRustcCompilation,
    };

    fn data_schema() -> SchemaRef {
        Arc::new(Schema::new(vec![Field::new(
            "native_kind",
            DataType::Utf8,
            false,
        )]))
    }

    fn coverage_schema() -> SchemaRef {
        Arc::new(Schema::new(vec![
            Field::new("family", DataType::Utf8, false),
            Field::new("requested_units", DataType::UInt64, false),
            Field::new("completed_units", DataType::UInt64, false),
            Field::new("terminal_status", DataType::Utf8, false),
            Field::new("remainder_reason", DataType::Utf8, true),
        ]))
    }

    fn data_batch(schema: &SchemaRef) -> RecordBatch {
        RecordBatch::try_new(
            Arc::clone(schema),
            vec![Arc::new(StringArray::from(vec!["name"])) as ArrayRef],
        )
        .unwrap()
    }

    fn coverage_batch(schema: &SchemaRef) -> RecordBatch {
        RecordBatch::try_new(
            Arc::clone(schema),
            vec![
                Arc::new(StringArray::from(vec!["ruff.token"])) as ArrayRef,
                Arc::new(UInt64Array::from(vec![1])) as ArrayRef,
                Arc::new(UInt64Array::from(vec![1])) as ArrayRef,
                Arc::new(StringArray::from(vec!["complete"])) as ArrayRef,
                Arc::new(StringArray::from(vec![None::<&str>])) as ArrayRef,
            ],
        )
        .unwrap()
    }

    fn characterized_coverage_schema() -> SchemaRef {
        Arc::new(Schema::new(vec![
            Field::new("family", DataType::Utf8, false),
            Field::new("requested_units", DataType::UInt64, false),
            Field::new("completed_units", DataType::UInt64, false),
            Field::new("terminal_status", DataType::Utf8, false),
            Field::new("remainder_reason", DataType::Utf8, true),
            Field::new("unknown_semantics", DataType::Boolean, false),
        ]))
    }

    fn characterized_coverage_batch(
        schema: &SchemaRef,
        status: &str,
        completed_units: u64,
        remainder_reason: Option<&str>,
        unknown_semantics: bool,
    ) -> RecordBatch {
        RecordBatch::try_new(
            Arc::clone(schema),
            vec![
                Arc::new(StringArray::from(vec!["rustc.call"])) as ArrayRef,
                Arc::new(UInt64Array::from(vec![1])) as ArrayRef,
                Arc::new(UInt64Array::from(vec![completed_units])) as ArrayRef,
                Arc::new(StringArray::from(vec![status])) as ArrayRef,
                Arc::new(StringArray::from(vec![remainder_reason])) as ArrayRef,
                Arc::new(BooleanArray::from(vec![unknown_semantics])) as ArrayRef,
            ],
        )
        .unwrap()
    }

    fn provider_revision() -> ProviderRevision {
        ProviderRevision {
            provider_id: ProviderId([1; 16]),
            release: "ruff-python-0.0.7".into(),
            source_revision: [2; 32],
        }
    }

    fn installer() -> ProviderInstallerIdentity {
        ProviderInstallerIdentity {
            installer_id: ProviderInstallerId([3; 32]),
            owner: BoundaryOwnerId([4; 32]),
            provider_revision: provider_revision(),
        }
    }

    fn family(value: &str) -> ProviderApiFamily {
        ProviderApiFamily::new(value).unwrap()
    }

    fn boundary_row(
        marker: u8,
        family_name: &str,
        schema: SchemaRef,
    ) -> ProviderBoundaryContractRow {
        let fields = schema
            .fields()
            .iter()
            .enumerate()
            .map(|(ordinal, field)| ProviderBoundaryField {
                ordinal,
                field: Arc::clone(field),
                meaning: if field.name() == "native_kind" {
                    FieldMeaning::ProviderNativeKind
                } else {
                    FieldMeaning::TypedFact
                },
                provider_local_identity: ProviderLocalIdentityRole::None,
                canonical_identity: CanonicalIdentityRole::NotCanonical,
                coordinate: CoordinateRole::None,
                retention: RetentionPolicy::RetainProviderNative,
            })
            .collect();
        ProviderBoundaryContractRow {
            api_family: family(family_name),
            upstream_symbols: vec![UpstreamApiSymbol::new("ruff::exact_api").unwrap()],
            relation: ProviderArrowRelationContract {
                relation_id: RelationId([marker; 16]),
                schema_fingerprint: SchemaFingerprint([marker.wrapping_add(1); 32]),
                schema,
                fields,
            },
            authority: ProviderAuthorityRole::Primary,
            disposition: ContractDisposition::Required,
            unavailable_behavior: UnavailableBehavior {
                allowed_statuses: vec![TerminalStatus::Partial],
                allowed_reasons: vec![
                    RemainderReason::ProviderUnavailable,
                    RemainderReason::ResourceLimit,
                    RemainderReason::InvalidSource,
                    RemainderReason::Cancelled,
                    RemainderReason::Unsupported,
                ],
            },
            oracle_id: ProviderOracleId([marker.wrapping_add(2); 32]),
        }
    }

    fn contract(rows: Vec<ProviderBoundaryContractRow>) -> ProviderBoundaryContract {
        ProviderBoundaryContract {
            contract_id: BoundaryContractId([5; 32]),
            contract_revision: 1,
            provider_revision: provider_revision(),
            acceptance: IndependentContractAcceptance {
                author_owner: BoundaryOwnerId([6; 32]),
                reviewer_owner: BoundaryOwnerId([7; 32]),
                acceptance_authority: BoundaryOwnerId([8; 32]),
            },
            rows,
        }
    }

    fn declared_coverage() -> ProviderCoverageSource {
        ProviderCoverageSource::ProviderDeclared(DeclaredCoverageBinding {
            relation_identity: ProviderRelationIdentity::try_new("provider.ruff.coverage").unwrap(),
            family_value: "ruff.token".into(),
            family_column: "family".into(),
            requested_units_column: "requested_units".into(),
            completed_units_column: "completed_units".into(),
            status_column: "terminal_status".into(),
            remainder_reason_column: Some("remainder_reason".into()),
            unknown_semantics_column: None,
            remainder_reason_map: BTreeMap::new(),
        })
    }

    fn binding(
        identity: &str,
        family_name: &str,
        table_name: &str,
        handler: u8,
        purpose: ProviderRelationPurpose,
        coverage: ProviderCoverageSource,
    ) -> ProviderRelationBinding {
        ProviderRelationBinding {
            provider_relation: ProviderRelationIdentity::try_new(identity).unwrap(),
            api_family: family(family_name),
            lane: ProviderNativeLane::Ruff,
            role: FabricSchemaRole::RawRuff,
            table_name: table_name.into(),
            source_schema_identity: Arc::from(format!("model:test:{family_name}")),
            handler_id: ProviderHandlerId([handler; 16]),
            authority_class: ProviderAuthorityClass::ProviderNative,
            purpose,
            requested_units: 1,
            coverage,
        }
    }

    fn accepted_plan() -> (ProviderAdmissionPlan, AcceptedProviderRelationSet) {
        let data_schema = data_schema();
        let coverage_schema = coverage_schema();
        let data_row = boundary_row(20, "ruff.token", Arc::clone(&data_schema));
        let coverage_row = boundary_row(30, "ruff.coverage", Arc::clone(&coverage_schema));
        let plan = ProviderAdmissionPlan {
            provider_kind: Arc::from("ruff.exact-arrow"),
            expected_source_pin: SourcePin([11; 32]),
            expected_context_pin: ContextPin([12; 32]),
            installer: installer(),
            contract: contract(vec![data_row, coverage_row]),
            bindings: vec![
                binding(
                    "provider.ruff.token",
                    "ruff.token",
                    "token",
                    40,
                    ProviderRelationPurpose::SemanticFact,
                    declared_coverage(),
                ),
                binding(
                    "provider.ruff.coverage",
                    "ruff.coverage",
                    "coverage",
                    41,
                    ProviderRelationPurpose::ControlEvidence,
                    ProviderCoverageSource::StructuralPresence,
                ),
            ],
        };
        let observed = AcceptedProviderRelationSet::try_new(
            SourcePin([11; 32]),
            ContextPin([12; 32]),
            vec![
                ObservedProviderRelation {
                    identity: ProviderRelationIdentity::try_new("provider.ruff.token").unwrap(),
                    lane: ProviderNativeLane::Ruff,
                    batches: vec![data_batch(&data_schema)],
                },
                ObservedProviderRelation {
                    identity: ProviderRelationIdentity::try_new("provider.ruff.coverage").unwrap(),
                    lane: ProviderNativeLane::Ruff,
                    batches: vec![coverage_batch(&coverage_schema)],
                },
            ],
        )
        .unwrap();
        (plan, observed)
    }

    fn digest(byte: u8) -> String {
        format!("b3:{}", format!("{byte:02x}").repeat(32))
    }

    pub(crate) fn programmatic_epoch_builder() -> ProgrammaticFabricEpochBuilder {
        ProgrammaticFabricEpochBuilder::try_new(
            FabricEpochId::from_bytes([90; 16]),
            FabricEpochRuntimeConfig::default(),
        )
        .unwrap()
    }

    fn exact_native_syntax_run(marker: u8, source_text: &str) -> ProviderNativeSyntaxRun {
        let bytes = Arc::<[u8]>::from(source_text.as_bytes());
        let source = ProviderNativeSourceImage::new(
            [marker; 16],
            7,
            Arc::clone(&bytes),
            crate::integrity::digest_bytes(&bytes),
            ProviderText {
                text: Arc::from(source_text),
                original_byte_offsets: Arc::from(
                    source_text
                        .char_indices()
                        .map(|(offset, _)| u64::try_from(offset).unwrap())
                        .chain(std::iter::once(u64::try_from(source_text.len()).unwrap()))
                        .collect::<Vec<_>>(),
                ),
            },
        )
        .unwrap();
        let module_name = format!("fixture.module_{marker}");
        let module_path = format!("fixture/module_{marker}.py");
        let release = CompiledSemanticRelease::current();
        ExactPythonSyntaxRunner::new(release.provider_authority())
            .unwrap()
            .run_full(
                1,
                &source,
                PythonSyntaxRunPins {
                    tree_sitter: SyntaxProviderRunPin {
                        provider_run_id: [marker; 16],
                        analysis_context_id: [202; 32],
                        semantic_environment_id: [203; 32],
                    },
                    ruff: SyntaxProviderRunPin {
                        provider_run_id: [marker.wrapping_add(64); 16],
                        analysis_context_id: [202; 32],
                        semantic_environment_id: [203; 32],
                    },
                },
                PythonModuleInput {
                    module_name: &module_name,
                    module_path: Path::new(&module_path),
                },
                &Cancellation::default(),
            )
            .unwrap()
    }

    fn fixed_value(width: i32, value: &[u8]) -> ArrayRef {
        let mut builder = FixedSizeBinaryBuilder::with_capacity(1, width);
        builder.append_value(value).unwrap();
        Arc::new(builder.finish())
    }

    fn ipc_batch(batch: &RecordBatch) -> Vec<u8> {
        let mut ipc = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut ipc, &batch.schema()).unwrap();
            writer.write(batch).unwrap();
            writer.finish().unwrap();
        }
        ipc
    }

    fn exact_pyrefly_batch(
        relation: PyreflyRelation,
        provider_run_id: &str,
        module_id: &str,
        module_name: &str,
        source_bytes: &[u8],
    ) -> RecordBatch {
        let schema = relation.schema();
        let content_digest = *blake3::hash(source_bytes).as_bytes();
        let columns = schema
            .fields()
            .iter()
            .map(|field| match field.data_type() {
                DataType::Utf8 => {
                    let value = match field.name().as_str() {
                        "provider_run_id" => provider_run_id,
                        "analysis_context_id" => "context:pyrefly-workspace",
                        "module_id" => module_id,
                        "file_id" => "file:pyrefly-fixture",
                        "module_name" => module_name,
                        "qualified_target" => "fixture.target",
                        "callee_kind" => "function",
                        "resolution_state" => "resolved",
                        _ => "fixture",
                    };
                    Arc::new(StringArray::from(vec![value])) as ArrayRef
                }
                DataType::UInt64 => Arc::new(UInt64Array::from(vec![7])) as ArrayRef,
                DataType::Boolean => Arc::new(BooleanArray::from(vec![true])) as ArrayRef,
                DataType::FixedSizeBinary(32) => {
                    let value = if field.name() == "content_digest" {
                        content_digest
                    } else {
                        [77; 32]
                    };
                    fixed_value(32, &value)
                }
                other => panic!("unexpected Pyrefly fixture type {other:?}"),
            })
            .collect::<Vec<_>>();
        RecordBatch::try_new(schema, columns).unwrap()
    }

    fn exact_pyrefly_run(marker: u8, source_text: &str) -> AcceptedPyreflyRun {
        let provider_run_id = format!("run:pyrefly:{marker}");
        let module_id = format!("module:pyrefly:{marker}");
        let module_name = format!("fixture.pyrefly_{marker}");
        let source_bytes = source_text.as_bytes().to_vec();
        let relations = PyreflyRelation::ALL
            .into_iter()
            .map(|relation| {
                let batch = exact_pyrefly_batch(
                    relation,
                    &provider_run_id,
                    &module_id,
                    &module_name,
                    &source_bytes,
                );
                let arrow_ipc = ipc_batch(&batch);
                AcceptedPyreflyRelation {
                    relation,
                    schema_digest: relation.schema_digest(),
                    arrow_ipc_digest: crate::integrity::framed_digest(&arrow_ipc),
                    row_count: 1,
                    batch,
                    arrow_ipc,
                }
            })
            .collect::<Vec<_>>();
        AcceptedPyreflyRun {
            provider_run_id,
            workspace_id: "workspace:provider-admission".to_owned(),
            analysis_context_id: "context:pyrefly-workspace".to_owned(),
            canonical_workspace_id: [61; 16],
            canonical_analysis_context_id: [62; 16],
            source_generation: 7,
            modules: vec![AcceptedPyreflyModule {
                module_id,
                module_name,
                canonical_file_id: [marker; 16],
                source_bytes,
                module_digest: digest(marker.wrapping_add(1)),
                relations,
            }],
            capability_codes: Vec::new(),
            overall_digest: digest(marker.wrapping_add(2)),
            rechecked_module_ids: Vec::new(),
            sandbox_profile_digest: digest(63),
            trust_profile: "UNTRUSTED_SANDBOXED".to_owned(),
        }
    }

    fn exact_rustc_batch(
        relation: RustcRelation,
        marker: u8,
        provider_run_id: &str,
        compilation_unit_id: &str,
        owner_id: &str,
    ) -> RecordBatch {
        let schema = relation.schema();
        let columns = schema
            .fields()
            .iter()
            .map(|field| match field.data_type() {
                DataType::Utf8 => {
                    let value = match field.name().as_str() {
                        "provider_run_id" => provider_run_id,
                        "compilation_unit_id" => compilation_unit_id,
                        "owner_id" => owner_id,
                        "slot_kind" => "statement",
                        "projection_kind" => "BaseLocal",
                        "occurrence_role" => "fixture-place",
                        "local_role" => "temporary",
                        "mutability" if relation == RustcRelation::MirLocal => "mut",
                        "rvalue_kind" => "Ref",
                        "cast_kind" => "PtrToPtr",
                        "aggregate_kind" => "Coroutine",
                        "region_kind" => "fixture-region",
                        "raw_statement_kind" => "Assign",
                        "normalized_effect" => "WRITE",
                        "raw_terminator_kind" => "InlineAsm",
                        "access_kind" => "Drop",
                        "structured_evidence" => "StatementKind::Assign.destination",
                        "declared_target" => "fixture::foreign_call",
                        "dispatch_kind" => "direct",
                        "resolution_confidence" => "exact",
                        _ => "fixture",
                    };
                    Arc::new(StringArray::from(vec![value])) as ArrayRef
                }
                DataType::UInt64 => {
                    let value = match field.name().as_str() {
                        "source_block" => u64::from(marker),
                        "target_block" => u64::from(marker).saturating_add(1),
                        "stable_crate_id" => u64::from(marker).saturating_add(100),
                        _ => 7,
                    };
                    Arc::new(UInt64Array::from(vec![value])) as ArrayRef
                }
                DataType::Boolean => Arc::new(BooleanArray::from(vec![true])) as ArrayRef,
                DataType::FixedSizeBinary(width @ (16 | 32)) => {
                    fixed_value(*width, &vec![91; usize::try_from(*width).unwrap()])
                }
                other => panic!("unexpected rustc fixture type {other:?}"),
            })
            .collect::<Vec<_>>();
        RecordBatch::try_new(schema, columns).unwrap()
    }

    fn exact_rustc_run(marker: u8, manifest_marker: u8) -> TrustQualifiedRustcCompilation {
        let provider_run_id = format!("run:rustc:{marker}");
        let compilation_unit_id = format!("unit:rustc:{marker}");
        let owner_id = format!("owner:rustc:{marker}");
        let relations = RustcRelation::ALL
            .into_iter()
            .map(|relation| {
                let batch = exact_rustc_batch(
                    relation,
                    marker,
                    &provider_run_id,
                    &compilation_unit_id,
                    &owner_id,
                );
                let arrow_ipc = ipc_batch(&batch);
                (relation, batch, arrow_ipc)
            })
            .collect::<Vec<_>>();
        let accepted = AcceptedRustcCompilation::test_only(
            RustcRunAdmission {
                provider_run_id: provider_run_id.clone(),
                workspace_id: "workspace:provider-admission".to_owned(),
                analysis_context_id: "context:rustc-workspace".to_owned(),
                canonical_workspace_id: [61; 16],
                canonical_analysis_context_id: [64; 16],
                source_generation: 7,
                context_manifest_digest: digest(manifest_marker),
                source_snapshot_manifest_digest: digest(manifest_marker.wrapping_add(1)),
                resource_profile_id: "profile:rustc-provider-admission".to_owned(),
            },
            CompilationBegin {
                provider_run_id: provider_run_id.clone(),
                compilation_unit_id: compilation_unit_id.clone(),
                ..CompilationBegin::default()
            },
            vec![AcceptedRustcOwner {
                begin: OwnerBegin {
                    provider_run_id: provider_run_id.clone(),
                    compilation_unit_id: compilation_unit_id.clone(),
                    sequence: 1,
                    owner: Some(CompilerOwnerKey {
                        owner_id: owner_id.clone(),
                        owner_kind: "CRATE".to_owned(),
                        file_id: format!("file:rustc:{marker}"),
                        source_start: 0,
                        source_end: 1,
                    }),
                    expected_observation_family_codes: relations
                        .iter()
                        .map(|(relation, _, _)| relation.family_code())
                        .collect(),
                },
                relations: relations
                    .into_iter()
                    .enumerate()
                    .map(
                        |(index, (relation, batch, arrow_ipc))| AcceptedRustcRelation {
                            relation,
                            logical_sequence: 2 + u64::try_from(index).unwrap(),
                            schema_digest: relation.schema_digest(),
                            row_count: 1,
                            arrow_ipc_digest: arrow_ipc_digest(&arrow_ipc),
                            arrow_ipc,
                            batch,
                        },
                    )
                    .collect(),
                end: OwnerEnd::default(),
            }],
            CompilationEnd {
                compiler_exit_status: 0,
                terminal_state: ProviderRunState::Succeeded as i32,
                ..CompilationEnd::default()
            },
        );
        TrustQualifiedRustcCompilation::test_only(accepted)
    }

    fn exact_plan(
        lane: ProviderNativeLane,
        marker: u8,
        observed: &AcceptedProviderRelationSet,
    ) -> ProviderAdmissionPlan {
        let revision = ProviderRevision {
            provider_id: ProviderId([marker; 16]),
            release: format!("exact-provider-{marker}"),
            source_revision: [marker.wrapping_add(1); 32],
        };
        let installer = ProviderInstallerIdentity {
            installer_id: ProviderInstallerId([marker.wrapping_add(2); 32]),
            owner: BoundaryOwnerId([marker.wrapping_add(3); 32]),
            provider_revision: revision.clone(),
        };
        let mut rows = Vec::with_capacity(observed.relations.len());
        let mut bindings = Vec::with_capacity(observed.relations.len());
        for (index, relation) in observed.relations.values().enumerate() {
            let identity = marker.wrapping_add(u8::try_from(index).unwrap());
            let family = family(&format!("fixture.{}.{}", lane_name(lane), index));
            let schema = relation.batches[0].schema();
            rows.push(ProviderBoundaryContractRow {
                api_family: family.clone(),
                upstream_symbols: vec![
                    UpstreamApiSymbol::new(format!("fixture::lane_{}::{index}", lane_name(lane)))
                        .unwrap(),
                ],
                relation: ProviderArrowRelationContract {
                    relation_id: RelationId([identity; 16]),
                    schema_fingerprint: SchemaFingerprint([identity.wrapping_add(1); 32]),
                    fields: schema
                        .fields()
                        .iter()
                        .enumerate()
                        .map(|(ordinal, field)| ProviderBoundaryField {
                            ordinal,
                            field: Arc::clone(field),
                            meaning: FieldMeaning::TypedFact,
                            provider_local_identity: ProviderLocalIdentityRole::None,
                            canonical_identity: CanonicalIdentityRole::NotCanonical,
                            coordinate: CoordinateRole::None,
                            retention: RetentionPolicy::RetainProviderNative,
                        })
                        .collect(),
                    schema,
                },
                authority: ProviderAuthorityRole::Primary,
                disposition: ContractDisposition::Required,
                unavailable_behavior: UnavailableBehavior {
                    allowed_statuses: vec![TerminalStatus::Partial, TerminalStatus::Unknown],
                    allowed_reasons: vec![
                        RemainderReason::Unknown,
                        RemainderReason::ProviderUnavailable,
                        RemainderReason::ResourceLimit,
                        RemainderReason::Cancelled,
                        RemainderReason::InvalidSource,
                        RemainderReason::Unsupported,
                    ],
                },
                oracle_id: ProviderOracleId([identity.wrapping_add(2); 32]),
            });
            bindings.push(ProviderRelationBinding {
                provider_relation: relation.identity.clone(),
                api_family: family,
                lane,
                role: lane.raw_role(),
                table_name: format!("fixture_{}_{}", lane_name(lane), index),
                source_schema_identity: Arc::from(format!(
                    "provider:fixture:{}:{}",
                    lane_name(lane),
                    relation.identity.as_str()
                )),
                handler_id: ProviderHandlerId([identity.wrapping_add(3); 16]),
                authority_class: ProviderAuthorityClass::ProviderNative,
                purpose: ProviderRelationPurpose::ControlEvidence,
                requested_units: 1,
                coverage: ProviderCoverageSource::StructuralPresence,
            });
        }
        ProviderAdmissionPlan {
            provider_kind: Arc::from(format!("exact-provider-lane-{}", lane_name(lane))),
            expected_source_pin: observed.source_pin,
            expected_context_pin: observed.context_pin,
            installer,
            contract: ProviderBoundaryContract {
                contract_id: BoundaryContractId([marker.wrapping_add(4); 32]),
                contract_revision: 1,
                provider_revision: revision,
                acceptance: IndependentContractAcceptance {
                    author_owner: BoundaryOwnerId([marker.wrapping_add(5); 32]),
                    reviewer_owner: BoundaryOwnerId([marker.wrapping_add(6); 32]),
                    acceptance_authority: BoundaryOwnerId([marker.wrapping_add(7); 32]),
                },
                rows,
            },
            bindings,
        }
    }

    pub(crate) struct ExactWorkspaceFixture {
        native_syntax_runs: Vec<ProviderNativeSyntaxRun>,
        pyrefly_runs: Vec<AcceptedPyreflyRun>,
        rustc_runs: Vec<TrustQualifiedRustcCompilation>,
        tree_sitter_plan: ProviderAdmissionPlan,
        ruff_plan: ProviderAdmissionPlan,
        pyrefly_plan: ProviderAdmissionPlan,
        rustc_plan: ProviderAdmissionPlan,
    }

    impl ExactWorkspaceFixture {
        pub(crate) fn runs(&self) -> ExactProgrammaticProviderRuns<'_> {
            ExactProgrammaticProviderRuns::try_new(
                &self.tree_sitter_plan,
                &self.ruff_plan,
                ExactProviderLaneRuns::Accepted(&self.native_syntax_runs),
                &self.pyrefly_plan,
                ExactProviderLaneRuns::Accepted(&self.pyrefly_runs),
                &self.rustc_plan,
                ExactProviderLaneRuns::Accepted(&self.rustc_runs),
            )
            .unwrap()
        }
    }

    pub(crate) fn exact_workspace_fixture() -> ExactWorkspaceFixture {
        exact_workspace_fixture_from(
            [(21, "value = 1\n"), (22, "other = value + 1\n")],
            [(31, "value: int = 1\n"), (32, "other: int = 2\n")],
            [41, 42],
            65,
        )
    }

    pub(crate) fn changed_exact_workspace_fixture() -> ExactWorkspaceFixture {
        exact_workspace_fixture_from(
            [
                (23, "value = call(1)\nresult = value\n"),
                (24, "other = call(2)\nresult = other\n"),
            ],
            [
                (33, "value: int = int('1')\n"),
                (34, "other: int = int('2')\n"),
            ],
            [43, 44],
            75,
        )
    }

    fn exact_workspace_fixture_from(
        native_sources: [(u8, &str); 2],
        pyrefly_sources: [(u8, &str); 2],
        rustc_markers: [u8; 2],
        rustc_manifest_marker: u8,
    ) -> ExactWorkspaceFixture {
        let native_syntax_runs = vec![
            exact_native_syntax_run(native_sources[0].0, native_sources[0].1),
            exact_native_syntax_run(native_sources[1].0, native_sources[1].1),
        ];
        let pyrefly_runs = vec![
            exact_pyrefly_run(pyrefly_sources[0].0, pyrefly_sources[0].1),
            exact_pyrefly_run(pyrefly_sources[1].0, pyrefly_sources[1].1),
        ];
        let rustc_runs = vec![
            exact_rustc_run(rustc_markers[0], rustc_manifest_marker),
            exact_rustc_run(rustc_markers[1], rustc_manifest_marker),
        ];

        let native = aggregate_native_syntax_runs(
            &native_syntax_runs,
            SourcePin([1; 32]),
            ContextPin([2; 32]),
        )
        .unwrap();
        let tree_sitter = provider_lane_subset(&native, ProviderNativeLane::TreeSitter).unwrap();
        let ruff = provider_lane_subset(&native, ProviderNativeLane::Ruff).unwrap();
        let pyrefly =
            aggregate_pyrefly_runs(&pyrefly_runs, SourcePin([3; 32]), ContextPin([4; 32])).unwrap();
        let rustc =
            aggregate_rustc_runs(&rustc_runs, SourcePin([5; 32]), ContextPin([6; 32])).unwrap();

        ExactWorkspaceFixture {
            native_syntax_runs,
            pyrefly_runs,
            rustc_runs,
            tree_sitter_plan: exact_plan(ProviderNativeLane::TreeSitter, 10, &tree_sitter),
            ruff_plan: exact_plan(ProviderNativeLane::Ruff, 50, &ruff),
            pyrefly_plan: exact_plan(ProviderNativeLane::Pyrefly, 100, &pyrefly),
            rustc_plan: exact_plan(ProviderNativeLane::Rustc, 150, &rustc),
        }
    }

    #[tokio::test]
    async fn wp34_beh_workspace_transaction_aggregates_all_four_exact_provider_lanes() {
        let fixture = exact_workspace_fixture();
        let outcome =
            admit_provider_relations_programmatic(programmatic_epoch_builder(), fixture.runs())
                .unwrap();
        for report in [
            outcome.reports().tree_sitter(),
            outcome.reports().ruff(),
            outcome.reports().pyrefly(),
            outcome.reports().rustc(),
        ] {
            assert_eq!(report.boundary.status, TerminalStatus::Complete);
            assert!(report.relations.iter().all(|relation| matches!(
                relation.disposition,
                ProviderRegistrationDisposition::Registered {
                    coverage: TerminalStatus::Complete,
                    ..
                }
            )));
        }

        let (builder, _) = outcome.into_parts();
        let (_, _, _, assembly) = builder.into_assembly_parts();
        let sealed = assembly
            .seal(FabricEpochId::from_bytes([90; 16]))
            .await
            .unwrap();
        for relation_id in [
            NativeSyntaxRelation::TreeSitterRun.as_str(),
            NativeSyntaxRelation::RuffRun.as_str(),
            PyreflyRelation::ModuleContext.relation_id(),
            RustcRelation::Compilation.relation_id(),
        ] {
            let binding = sealed
                .relation(&ProgrammaticRelationId::new(relation_id))
                .unwrap_or_else(|| panic!("{relation_id} was not registered"));
            let rows = sealed
                .session()
                .table(binding.table_reference.clone())
                .await
                .unwrap()
                .count()
                .await
                .unwrap();
            assert_eq!(rows, 2, "{relation_id} did not retain both partitions");
        }
    }

    #[tokio::test]
    async fn wp34_beh_absent_language_lanes_return_explicit_unknowns_without_fake_tables() {
        let fixture = exact_workspace_fixture();
        let outcome = admit_provider_relations_programmatic(
            programmatic_epoch_builder(),
            ExactProgrammaticProviderRuns::try_new(
                &fixture.tree_sitter_plan,
                &fixture.ruff_plan,
                ExactProviderLaneRuns::Gap(ProviderLaneGap::RequiredInputAbsent),
                &fixture.pyrefly_plan,
                ExactProviderLaneRuns::Gap(ProviderLaneGap::RequiredInputAbsent),
                &fixture.rustc_plan,
                ExactProviderLaneRuns::Gap(ProviderLaneGap::RequiredInputAbsent),
            )
            .unwrap(),
        )
        .unwrap();
        for report in [
            outcome.reports().tree_sitter(),
            outcome.reports().ruff(),
            outcome.reports().pyrefly(),
            outcome.reports().rustc(),
        ] {
            assert_eq!(report.boundary.status, TerminalStatus::Unknown);
            assert!(report.relations.iter().all(|relation| {
                relation.lane_gap == Some(ProviderLaneGap::RequiredInputAbsent)
                    && relation.disposition
                        == ProviderRegistrationDisposition::Unknown {
                            cause: ProviderAdmissionUnknownCause::MissingRelation,
                        }
            }));
        }

        let (builder, _) = outcome.into_parts();
        let (_, _, _, assembly) = builder.into_assembly_parts();
        let sealed = assembly
            .seal(FabricEpochId::from_bytes([90; 16]))
            .await
            .unwrap();
        assert!(
            sealed
                .relation(&ProgrammaticRelationId::new(
                    NativeSyntaxRelation::TreeSitterRun.as_str()
                ))
                .is_none()
        );
    }

    #[test]
    fn wp34_beh_provider_lane_gaps_preserve_exact_cause_status_and_remainder() {
        let fixture = exact_workspace_fixture();
        for (gap, expected_status, expected_reason) in [
            (
                ProviderLaneGap::RequiredInputAbsent,
                TerminalStatus::Unknown,
                RemainderReason::Unknown,
            ),
            (
                ProviderLaneGap::OptionalInputAbsent,
                TerminalStatus::Unknown,
                RemainderReason::Unknown,
            ),
            (
                ProviderLaneGap::ProviderFailure,
                TerminalStatus::Partial,
                RemainderReason::ProviderUnavailable,
            ),
            (
                ProviderLaneGap::CompilationFailure,
                TerminalStatus::Partial,
                RemainderReason::ProviderUnavailable,
            ),
            (
                ProviderLaneGap::TrustUnavailable,
                TerminalStatus::Partial,
                RemainderReason::ProviderUnavailable,
            ),
            (
                ProviderLaneGap::ResourceLimit,
                TerminalStatus::Partial,
                RemainderReason::ResourceLimit,
            ),
            (
                ProviderLaneGap::TimedOut,
                TerminalStatus::Partial,
                RemainderReason::ResourceLimit,
            ),
            (
                ProviderLaneGap::Cancelled,
                TerminalStatus::Partial,
                RemainderReason::Cancelled,
            ),
            (
                ProviderLaneGap::InvalidSource,
                TerminalStatus::Partial,
                RemainderReason::InvalidSource,
            ),
            (
                ProviderLaneGap::Unsupported,
                TerminalStatus::Partial,
                RemainderReason::Unsupported,
            ),
        ] {
            let outcome = admit_provider_relations_programmatic(
                programmatic_epoch_builder(),
                ExactProgrammaticProviderRuns::try_new(
                    &fixture.tree_sitter_plan,
                    &fixture.ruff_plan,
                    ExactProviderLaneRuns::Accepted(&fixture.native_syntax_runs),
                    &fixture.pyrefly_plan,
                    ExactProviderLaneRuns::Gap(gap),
                    &fixture.rustc_plan,
                    ExactProviderLaneRuns::Accepted(&fixture.rustc_runs),
                )
                .unwrap(),
            )
            .unwrap();
            let report = outcome.reports().pyrefly();
            assert_eq!(report.boundary.status, expected_status);
            assert!(report.relations.iter().all(|relation| {
                if relation.lane_gap != Some(gap) {
                    return false;
                }
                match &relation.disposition {
                    ProviderRegistrationDisposition::Unknown {
                        cause: ProviderAdmissionUnknownCause::MissingRelation,
                    } => expected_status == TerminalStatus::Unknown,
                    ProviderRegistrationDisposition::Remainder { trailer } => {
                        expected_status == TerminalStatus::Partial
                            && trailer.status == expected_status
                            && trailer.completed_units == 0
                            && trailer.remainders.len() == 1
                            && trailer.remainders[0].reason == expected_reason
                            && trailer.remainders[0].unit_count == trailer.requested_units
                    }
                    _ => false,
                }
            }));
        }
    }

    #[test]
    fn wp34_neg_empty_accepted_provider_lane_is_rejected_before_admission() {
        let fixture = exact_workspace_fixture();
        for (lane, runs) in [
            (
                ProviderNativeLane::TreeSitter,
                ExactProgrammaticProviderRuns::try_new(
                    &fixture.tree_sitter_plan,
                    &fixture.ruff_plan,
                    ExactProviderLaneRuns::Accepted(&fixture.native_syntax_runs[..0]),
                    &fixture.pyrefly_plan,
                    ExactProviderLaneRuns::Gap(ProviderLaneGap::RequiredInputAbsent),
                    &fixture.rustc_plan,
                    ExactProviderLaneRuns::Gap(ProviderLaneGap::RequiredInputAbsent),
                ),
            ),
            (
                ProviderNativeLane::Pyrefly,
                ExactProgrammaticProviderRuns::try_new(
                    &fixture.tree_sitter_plan,
                    &fixture.ruff_plan,
                    ExactProviderLaneRuns::Gap(ProviderLaneGap::RequiredInputAbsent),
                    &fixture.pyrefly_plan,
                    ExactProviderLaneRuns::Accepted(&fixture.pyrefly_runs[..0]),
                    &fixture.rustc_plan,
                    ExactProviderLaneRuns::Gap(ProviderLaneGap::RequiredInputAbsent),
                ),
            ),
            (
                ProviderNativeLane::Rustc,
                ExactProgrammaticProviderRuns::try_new(
                    &fixture.tree_sitter_plan,
                    &fixture.ruff_plan,
                    ExactProviderLaneRuns::Gap(ProviderLaneGap::RequiredInputAbsent),
                    &fixture.pyrefly_plan,
                    ExactProviderLaneRuns::Gap(ProviderLaneGap::RequiredInputAbsent),
                    &fixture.rustc_plan,
                    ExactProviderLaneRuns::Accepted(&fixture.rustc_runs[..0]),
                ),
            ),
        ] {
            assert!(matches!(
                runs,
                Err(ProviderAdmissionError::EmptyAcceptedProviderLane { lane: actual })
                    if actual == lane
            ));
        }
    }

    #[test]
    fn wp34_neg_duplicate_workspace_partition_is_rejected_before_registration() {
        let fixture = exact_workspace_fixture();
        let duplicate_syntax = vec![
            exact_native_syntax_run(21, "value = 1\n"),
            exact_native_syntax_run(21, "value = 2\n"),
        ];
        let error = admit_provider_relations_programmatic(
            programmatic_epoch_builder(),
            ExactProgrammaticProviderRuns::try_new(
                &fixture.tree_sitter_plan,
                &fixture.ruff_plan,
                ExactProviderLaneRuns::Accepted(&duplicate_syntax),
                &fixture.pyrefly_plan,
                ExactProviderLaneRuns::Accepted(&fixture.pyrefly_runs),
                &fixture.rustc_plan,
                ExactProviderLaneRuns::Accepted(&fixture.rustc_runs),
            )
            .unwrap(),
        )
        .err()
        .expect("duplicate source partition must fail");
        assert!(matches!(
            error,
            ProviderAdmissionError::DuplicateProviderPartition {
                lane: ProviderNativeLane::TreeSitter,
                ..
            }
        ));
    }

    #[test]
    fn wp34_neg_exact_programmatic_admission_rejects_missing_pyrefly_coverage_relation() {
        let fixture = exact_workspace_fixture();
        let mut missing_coverage = fixture.pyrefly_runs.clone();
        missing_coverage[0].modules[0]
            .relations
            .retain(|relation| relation.relation != PyreflyRelation::Coverage);

        let error = admit_provider_relations_programmatic(
            programmatic_epoch_builder(),
            ExactProgrammaticProviderRuns::try_new(
                &fixture.tree_sitter_plan,
                &fixture.ruff_plan,
                ExactProviderLaneRuns::Accepted(&fixture.native_syntax_runs),
                &fixture.pyrefly_plan,
                ExactProviderLaneRuns::Accepted(&missing_coverage),
                &fixture.rustc_plan,
                ExactProviderLaneRuns::Accepted(&fixture.rustc_runs),
            )
            .unwrap(),
        )
        .err()
        .expect("an exact provider run cannot omit its typed coverage relation");

        assert!(matches!(
            error,
            ProviderAdmissionError::IncompleteExactProviderRun {
                lane: ProviderNativeLane::Pyrefly,
                relation,
            } if relation.ends_with(PyreflyRelation::Coverage.relation_id())
        ));
    }

    #[test]
    fn wp34_neg_changed_source_or_rustc_receipt_binding_invalidates_workspace_authority() {
        let fixture = exact_workspace_fixture();
        let changed_syntax = vec![
            exact_native_syntax_run(21, "value = 1\n"),
            exact_native_syntax_run(22, "other = value + 2\n"),
        ];
        let error = admit_provider_relations_programmatic(
            programmatic_epoch_builder(),
            ExactProgrammaticProviderRuns::try_new(
                &fixture.tree_sitter_plan,
                &fixture.ruff_plan,
                ExactProviderLaneRuns::Accepted(&changed_syntax),
                &fixture.pyrefly_plan,
                ExactProviderLaneRuns::Accepted(&fixture.pyrefly_runs),
                &fixture.rustc_plan,
                ExactProviderLaneRuns::Accepted(&fixture.rustc_runs),
            )
            .unwrap(),
        )
        .err()
        .expect("changed source partition must change workspace authority");
        assert!(matches!(
            error,
            ProviderAdmissionError::AdmissionPinMismatch
        ));

        let mut changed_rustc = fixture.rustc_runs.clone();
        changed_rustc[1]
            .accepted_mut_for_test()
            .admission
            .source_snapshot_manifest_digest = digest(99);
        let error = admit_provider_relations_programmatic(
            programmatic_epoch_builder(),
            ExactProgrammaticProviderRuns::try_new(
                &fixture.tree_sitter_plan,
                &fixture.ruff_plan,
                ExactProviderLaneRuns::Accepted(&fixture.native_syntax_runs),
                &fixture.pyrefly_plan,
                ExactProviderLaneRuns::Accepted(&fixture.pyrefly_runs),
                &fixture.rustc_plan,
                ExactProviderLaneRuns::Accepted(&changed_rustc),
            )
            .unwrap(),
        )
        .err()
        .expect("rustc source pins detached from the launcher receipt must fail");
        assert!(matches!(
            error,
            ProviderAdmissionError::RustcTrustEvidence(_)
        ));
    }

    #[test]
    fn wp34_neg_cross_provider_relation_and_table_collisions_are_rejected() {
        let fixture = exact_workspace_fixture();
        let mut relation_collision = fixture.ruff_plan.clone();
        relation_collision.bindings[0].provider_relation = fixture.tree_sitter_plan.bindings[0]
            .provider_relation
            .clone();
        let error = admit_provider_relations_programmatic(
            programmatic_epoch_builder(),
            ExactProgrammaticProviderRuns::try_new(
                &fixture.tree_sitter_plan,
                &relation_collision,
                ExactProviderLaneRuns::Accepted(&fixture.native_syntax_runs),
                &fixture.pyrefly_plan,
                ExactProviderLaneRuns::Accepted(&fixture.pyrefly_runs),
                &fixture.rustc_plan,
                ExactProviderLaneRuns::Accepted(&fixture.rustc_runs),
            )
            .unwrap(),
        )
        .err()
        .expect("cross-provider relation collision must fail");
        assert!(matches!(
            error,
            ProviderAdmissionError::CrossProviderRelationIdentity { .. }
        ));

        let mut table_collision = fixture.ruff_plan.clone();
        table_collision.bindings[0].role = fixture.tree_sitter_plan.bindings[0].role;
        table_collision.bindings[0].table_name =
            fixture.tree_sitter_plan.bindings[0].table_name.clone();
        let error = admit_provider_relations_programmatic(
            programmatic_epoch_builder(),
            ExactProgrammaticProviderRuns::try_new(
                &fixture.tree_sitter_plan,
                &table_collision,
                ExactProviderLaneRuns::Accepted(&fixture.native_syntax_runs),
                &fixture.pyrefly_plan,
                ExactProviderLaneRuns::Accepted(&fixture.pyrefly_runs),
                &fixture.rustc_plan,
                ExactProviderLaneRuns::Accepted(&fixture.rustc_runs),
            )
            .unwrap(),
        )
        .err()
        .expect("cross-provider table collision must fail");
        assert!(matches!(
            error,
            ProviderAdmissionError::CrossProviderTableBinding { .. }
        ));
    }

    #[test]
    fn later_provider_failure_drops_the_partially_registered_builder() {
        let fixture = exact_workspace_fixture();
        let rustc =
            aggregate_rustc_runs(&fixture.rustc_runs, SourcePin([1; 32]), ContextPin([2; 32]))
                .unwrap();
        let relation_id =
            ProviderRelationIdentity::try_new(RustcRelation::Compilation.relation_id()).unwrap();
        let relation = rustc.relations.get(&relation_id).unwrap();
        let schema = relation.batches[0].schema();
        let table_reference = TableReference::full(
            FABRIC_CATALOG,
            FabricSchemaRole::RawRustc.as_str(),
            "preexisting_rustc_collision",
        );
        let contract = Arc::new(
            SchemaContract::try_new(
                "provider:fixture:preexisting-rustc",
                table_reference.clone(),
                Arc::clone(&schema),
                Arc::clone(&schema),
                (0..schema.fields().len())
                    .map(|index| FieldIndexMapping::direct(index, index))
                    .collect(),
            )
            .unwrap(),
        );
        let provider = Arc::new(
            MemTable::try_new(Arc::clone(&schema), vec![relation.batches.clone()]).unwrap(),
        );
        let weak_provider = Arc::downgrade(&provider);
        let mut builder = programmatic_epoch_builder();
        builder
            .register_provider(ProviderInput::new(
                ProgrammaticRelationId::new(relation_id.as_str()),
                table_reference,
                contract,
                provider.clone(),
            ))
            .unwrap();
        drop(provider);

        let error = admit_provider_relations_programmatic(builder, fixture.runs())
            .err()
            .expect("the rustc duplicate must fail after earlier lane registration");
        assert!(matches!(
            error,
            ProviderAdmissionError::ProgrammaticSchema(
                ProgrammaticSchemaError::DuplicateRelation { .. }
            )
        ));
        assert!(
            weak_provider.upgrade().is_none(),
            "failed transaction retained a partially registered candidate"
        );
    }

    #[test]
    fn wp34_beh_accepted_rustc_zero_fact_relation_remains_a_proved_empty_batch() {
        let relation = RustcRelation::MirRvalue;
        let batch = RecordBatch::new_empty(relation.schema());
        let mut arrow_ipc = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut arrow_ipc, &batch.schema()).unwrap();
            writer.write(&batch).unwrap();
            writer.finish().unwrap();
        }
        let provider_run_id = "run:rustc-provider-admission".to_owned();
        let compilation_unit_id = "unit:rustc-provider-admission".to_owned();
        let owner_id = "owner:rustc-provider-admission".to_owned();
        let accepted_relation = AcceptedRustcRelation {
            relation,
            logical_sequence: 2,
            schema_digest: relation.schema_digest(),
            row_count: 0,
            arrow_ipc_digest: arrow_ipc_digest(&arrow_ipc),
            arrow_ipc,
            batch,
        };
        let owner = AcceptedRustcOwner {
            begin: OwnerBegin {
                provider_run_id: provider_run_id.clone(),
                compilation_unit_id: compilation_unit_id.clone(),
                sequence: 1,
                owner: Some(CompilerOwnerKey {
                    owner_id: owner_id.clone(),
                    owner_kind: "MIR_BODY".into(),
                    file_id: "file:rustc-provider-admission".into(),
                    source_start: 0,
                    source_end: 0,
                }),
                expected_observation_family_codes: vec![relation.family_code()],
            },
            relations: vec![accepted_relation],
            end: OwnerEnd::default(),
        };
        let accepted = AcceptedRustcCompilation::test_only(
            RustcRunAdmission {
                provider_run_id: provider_run_id.clone(),
                workspace_id: "workspace:rustc-provider-admission".into(),
                analysis_context_id: "context:rustc-provider-admission".into(),
                canonical_workspace_id: [1; 16],
                canonical_analysis_context_id: [2; 16],
                source_generation: 7,
                context_manifest_digest: digest(41),
                source_snapshot_manifest_digest: digest(42),
                resource_profile_id: "profile:rustc-provider-admission".into(),
            },
            CompilationBegin {
                provider_run_id,
                compilation_unit_id,
                ..CompilationBegin::default()
            },
            vec![owner],
            CompilationEnd {
                compiler_exit_status: 0,
                terminal_state: ProviderRunState::Succeeded as i32,
                ..CompilationEnd::default()
            },
        );

        let relations = AcceptedProviderRelationSet::from_rustc(&accepted).unwrap();
        let observed = relations
            .relations
            .get(&ProviderRelationIdentity::try_new(relation.relation_id()).unwrap())
            .expect("zero-fact rustc relation remains present");
        assert_eq!(observed.lane, ProviderNativeLane::Rustc);
        assert_eq!(observed.batches.len(), 1);
        assert_eq!(observed.batches[0].num_rows(), 0);
        assert_eq!(relations.source_pin(), SourcePin([42; 32]));
        assert_eq!(relations.context_pin(), ContextPin([41; 32]));
    }

    #[tokio::test]
    async fn proof_qualified_capabilities_register_in_the_exact_programmatic_candidate() {
        let fixture = exact_workspace_fixture();
        let admitted =
            admit_provider_relations_programmatic(programmatic_epoch_builder(), fixture.runs())
                .unwrap();
        let candidate_epoch_id = FabricEpochId::from_bytes([90; 16]);
        assert_eq!(admitted.candidate_epoch_id(), &candidate_epoch_id);

        let proof_oracle = OracleId::new([71; 16]).unwrap();
        let proof_relations = test_relations_with_oracle(
            candidate_epoch_id,
            proof_oracle,
            OracleImplementationRef::new([72; 32]).unwrap(),
            Some(ProofRunId::new([73; 16]).unwrap()),
            ProofTerminalStatus::Pass,
        );
        let reports = admitted.reports();
        let provider_reports = [
            reports.tree_sitter(),
            reports.ruff(),
            reports.pyrefly(),
            reports.rustc(),
        ];
        let expected_rows = provider_reports
            .iter()
            .map(|report| report.boundary.families.len())
            .sum::<usize>();
        let oracle_bindings = provider_reports
            .into_iter()
            .flat_map(|report| &report.boundary.families)
            .map(|family| ProviderOracleProofBinding {
                provider_oracle_id: family.oracle_id,
                relation_id: family.relation_id,
                proof_oracle_id: proof_oracle,
            })
            .collect::<Vec<_>>();
        let admitted = admit_provider_capability_from_proof_relations(
            admitted,
            &ProviderCapabilityCatalogBinding {
                table_name: "provider_capability".into(),
                source_schema_identity: Arc::from("programmatic:provider-capability:test"),
            },
            &proof_relations,
            &oracle_bindings,
        )
        .unwrap();
        assert_eq!(admitted.candidate_epoch_id(), &candidate_epoch_id);
        assert_eq!(admitted.capabilities().len(), 4);
        assert_eq!(
            admitted
                .capabilities()
                .iter()
                .map(|capability| capability.batch().num_rows())
                .sum::<usize>(),
            expected_rows
        );
        for capability in admitted.capabilities() {
            let states = capability
                .batch()
                .column(24)
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            assert!(
                states.iter().all(|state| state == Some("proved-complete")),
                "a complete accepted family with passing proof was not advertised"
            );
        }

        let (builder, reports, capabilities) = admitted.into_parts();
        assert_eq!(reports.rustc().boundary.status, TerminalStatus::Complete);
        assert_eq!(capabilities.len(), 4);
        let (_, _, _, assembly) = builder.into_assembly_parts();
        let sealed = assembly.seal(candidate_epoch_id).await.unwrap();
        let capability = sealed
            .relation(&ProgrammaticRelationId::new(
                "system.provider_capability.v1",
            ))
            .expect("provider capability relation was not registered");
        assert_eq!(
            capability.table_reference,
            TableReference::full(
                FABRIC_CATALOG,
                FabricSchemaRole::System.as_str(),
                "provider_capability",
            )
        );
        let row_count = sealed
            .session()
            .table(capability.table_reference.clone())
            .await
            .unwrap()
            .count()
            .await
            .unwrap();
        assert_eq!(row_count, expected_rows);
    }

    #[test]
    fn provider_capability_rejects_a_different_proof_candidate_epoch() {
        let fixture = exact_workspace_fixture();
        let admitted =
            admit_provider_relations_programmatic(programmatic_epoch_builder(), fixture.runs())
                .unwrap();
        let proof_relations = test_relations_with_oracle(
            FabricEpochId::from_bytes([91; 16]),
            OracleId::new([71; 16]).unwrap(),
            OracleImplementationRef::new([72; 32]).unwrap(),
            Some(ProofRunId::new([73; 16]).unwrap()),
            ProofTerminalStatus::Pass,
        );
        let error = admit_provider_capability_from_proof_relations(
            admitted,
            &ProviderCapabilityCatalogBinding {
                table_name: "provider_capability".into(),
                source_schema_identity: Arc::from("programmatic:provider-capability:test"),
            },
            &proof_relations,
            &[],
        )
        .err()
        .expect("a proof from another candidate epoch must fail");
        assert!(matches!(
            error,
            ProviderAdmissionError::CapabilityProofEpochMismatch
        ));
    }

    #[test]
    fn provider_capability_rejects_a_binding_outside_the_exact_reports() {
        let fixture = exact_workspace_fixture();
        let admitted =
            admit_provider_relations_programmatic(programmatic_epoch_builder(), fixture.runs())
                .unwrap();
        let proof_oracle = OracleId::new([71; 16]).unwrap();
        let proof_relations = test_relations_with_oracle(
            FabricEpochId::from_bytes([90; 16]),
            proof_oracle,
            OracleImplementationRef::new([72; 32]).unwrap(),
            Some(ProofRunId::new([73; 16]).unwrap()),
            ProofTerminalStatus::Pass,
        );
        let error = admit_provider_capability_from_proof_relations(
            admitted,
            &ProviderCapabilityCatalogBinding {
                table_name: "provider_capability".into(),
                source_schema_identity: Arc::from("programmatic:provider-capability:test"),
            },
            &proof_relations,
            &[ProviderOracleProofBinding {
                provider_oracle_id: ProviderOracleId([250; 32]),
                relation_id: RelationId([251; 16]),
                proof_oracle_id: proof_oracle,
            }],
        )
        .err()
        .expect("a binding outside the exact reports must fail");
        assert!(matches!(
            error,
            ProviderAdmissionError::Capability(ProviderCapabilityError::UnboundProofBinding)
        ));
    }

    #[test]
    fn missing_relation_and_missing_coverage_are_explicit_unknowns() {
        let data_schema = data_schema();
        let plan = ProviderAdmissionPlan {
            provider_kind: Arc::from("ruff.exact-arrow"),
            expected_source_pin: SourcePin([11; 32]),
            expected_context_pin: ContextPin([12; 32]),
            installer: installer(),
            contract: contract(vec![boundary_row(
                20,
                "ruff.token",
                Arc::clone(&data_schema),
            )]),
            bindings: vec![binding(
                "provider.ruff.token",
                "ruff.token",
                "token",
                40,
                ProviderRelationPurpose::SemanticFact,
                declared_coverage(),
            )],
        };
        let absent =
            AcceptedProviderRelationSet::try_new(SourcePin([11; 32]), ContextPin([12; 32]), vec![])
                .unwrap();
        let report = evaluate_provider_admission(&plan, &absent).unwrap();
        assert_eq!(report.boundary.status, TerminalStatus::Unknown);
        assert_eq!(
            report.relations[0].disposition,
            ProviderRegistrationDisposition::Unknown {
                cause: ProviderAdmissionUnknownCause::MissingRelation
            }
        );

        let without_coverage = AcceptedProviderRelationSet::try_new(
            SourcePin([11; 32]),
            ContextPin([12; 32]),
            vec![ObservedProviderRelation {
                identity: ProviderRelationIdentity::try_new("provider.ruff.token").unwrap(),
                lane: ProviderNativeLane::Ruff,
                batches: vec![data_batch(&data_schema)],
            }],
        )
        .unwrap();
        let report = evaluate_provider_admission(&plan, &without_coverage).unwrap();
        assert_eq!(report.boundary.status, TerminalStatus::Unknown);
        assert_eq!(
            report.relations[0].disposition,
            ProviderRegistrationDisposition::Unknown {
                cause: ProviderAdmissionUnknownCause::MissingCoverage
            }
        );
    }

    #[test]
    fn characterized_provider_statuses_map_without_rewriting_raw_coverage() {
        let evaluate = |raw_status: &str,
                        completed_units: u64,
                        remainder_reason: Option<&str>,
                        unknown_semantics: bool|
         -> Result<CoverageTrailer, ProviderAdmissionError> {
            let schema = characterized_coverage_schema();
            let batch = characterized_coverage_batch(
                &schema,
                raw_status,
                completed_units,
                remainder_reason,
                unknown_semantics,
            );
            let statuses = batch
                .column_by_name("terminal_status")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            assert_eq!(statuses.value(0), raw_status, "provider row was rewritten");
            let coverage_identity =
                ProviderRelationIdentity::try_new("provider.rustc.coverage.v1").unwrap();
            let observed = AcceptedProviderRelationSet::try_new(
                SourcePin([0x91; 32]),
                ContextPin([0x92; 32]),
                vec![ObservedProviderRelation {
                    identity: coverage_identity.clone(),
                    lane: ProviderNativeLane::Rustc,
                    batches: vec![batch],
                }],
            )
            .unwrap();
            let binding = ProviderRelationBinding {
                provider_relation: ProviderRelationIdentity::try_new("provider.rustc.call.v1")
                    .unwrap(),
                api_family: family("rustc.call"),
                lane: ProviderNativeLane::Rustc,
                role: FabricSchemaRole::RawRustc,
                table_name: "rustc_call".to_owned(),
                source_schema_identity: Arc::from("provider:rustc:call:v1"),
                handler_id: ProviderHandlerId([0x93; 16]),
                authority_class: ProviderAuthorityClass::ProviderNative,
                purpose: ProviderRelationPurpose::SemanticFact,
                requested_units: 1,
                coverage: ProviderCoverageSource::StructuralPresence,
            };
            let declared = DeclaredCoverageBinding {
                relation_identity: coverage_identity,
                family_value: "rustc.call".to_owned(),
                family_column: "family".to_owned(),
                requested_units_column: "requested_units".to_owned(),
                completed_units_column: "completed_units".to_owned(),
                status_column: "terminal_status".to_owned(),
                remainder_reason_column: Some("remainder_reason".to_owned()),
                unknown_semantics_column: Some("unknown_semantics".to_owned()),
                remainder_reason_map: BTreeMap::from([(
                    "RESOURCE_LIMIT".to_owned(),
                    RemainderReason::ResourceLimit,
                )]),
            };
            super::declared_coverage(&binding, &declared, &observed)?
                .ok_or_else(|| coverage_error("provider.rustc.coverage.v1", "missing row"))
        };

        let complete = evaluate("complete", 1, None, false).unwrap();
        assert_eq!(complete.status, TerminalStatus::Complete);
        assert_eq!(complete.completed_units, 1);

        let partial = evaluate("partial-characterized", 0, Some("RESOURCE_LIMIT"), false).unwrap();
        assert_eq!(partial.status, TerminalStatus::Partial);
        assert_eq!(partial.completed_units, 0);
        assert_eq!(partial.remainders[0].reason, RemainderReason::ResourceLimit);

        let unavailable = evaluate("unavailable-characterized", 0, None, true).unwrap();
        assert_eq!(unavailable.status, TerminalStatus::Unknown);
        assert_eq!(unavailable.completed_units, 0);
        assert_eq!(unavailable.remainders[0].reason, RemainderReason::Unknown);

        let error = evaluate("new-unregistered-provider-state", 0, None, true).unwrap_err();
        assert!(matches!(
            error,
            ProviderAdmissionError::InvalidCoverage { .. }
        ));
    }

    #[test]
    fn unbound_provider_output_is_rejected() {
        let (plan, mut observed) = accepted_plan();
        let extra_schema = data_schema();
        let extra_identity = ProviderRelationIdentity::try_new("provider.ruff.extra").unwrap();
        observed.relations.insert(
            extra_identity.clone(),
            ObservedProviderRelation {
                identity: extra_identity,
                lane: ProviderNativeLane::Ruff,
                batches: vec![data_batch(&extra_schema)],
            },
        );
        assert!(matches!(
            evaluate_provider_admission(&plan, &observed),
            Err(ProviderAdmissionError::UnexpectedProviderRelation(name))
                if name == "provider.ruff.extra"
        ));
    }

    #[test]
    fn python_analysis_and_application_rust_cannot_claim_provider_authority() {
        for authority_class in [
            ProviderAuthorityClass::PythonCfg,
            ProviderAuthorityClass::PythonDataflow,
            ProviderAuthorityClass::RustApplicationDerived,
        ] {
            let schema = data_schema();
            let mut forbidden_binding = binding(
                "provider.ruff.analysis",
                "application.analysis",
                "analysis",
                40,
                ProviderRelationPurpose::SemanticFact,
                declared_coverage(),
            );
            forbidden_binding.authority_class = authority_class;
            let plan = ProviderAdmissionPlan {
                provider_kind: Arc::from("ruff.exact-arrow"),
                expected_source_pin: SourcePin([11; 32]),
                expected_context_pin: ContextPin([12; 32]),
                installer: installer(),
                contract: contract(vec![boundary_row(
                    20,
                    "application.analysis",
                    Arc::clone(&schema),
                )]),
                bindings: vec![forbidden_binding],
            };
            let observed = AcceptedProviderRelationSet::try_new(
                SourcePin([11; 32]),
                ContextPin([12; 32]),
                vec![],
            )
            .unwrap();
            assert!(matches!(
                evaluate_provider_admission(&plan, &observed),
                Err(ProviderAdmissionError::ForbiddenAuthorityClaim { class, .. })
                    if class == authority_class
            ));
        }
    }
}
