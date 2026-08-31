//! Admission of exact provider-native Arrow relations into a candidate session.
//!
//! Provider adapters expose typed Arrow batches. This module does not reinterpret those rows as
//! canonical facts: it joins observed relations to an independently accepted
//! [`ProviderBoundaryContract`], derives coverage from typed coverage relations, and registers
//! only accepted raw relations in a candidate [`FabricEpochBuilder`]. Missing output or coverage
//! is an explicit unknown; schema, pin, authority, or unexpected-output contradictions fail the
//! whole consumed candidate builder.

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

use crate::fabric::epoch::{
    FABRIC_CATALOG, FabricEpochBuilder, FabricEpochError, FabricEpochId, FabricSchemaRole,
};
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
    ProviderCapabilityError, ProviderCapabilityRelation, ProviderOracleProof,
    ProviderOracleProofBinding, derive_provider_capability_relation,
    provider_oracle_proofs_from_executable_relations,
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
use crate::rustc_service::{AcceptedRustcCompilation, AcceptedRustcOwner};
use crate::schema_contract::{FieldIndexMapping, SchemaContract, SchemaContractError};

const MAX_ADMISSION_BINDINGS: usize = 4_096;
const MAX_RELATION_NAME_BYTES: usize = 512;

/// Provider-emitted relation name used only to join output to model-owned bindings.
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

/// Model classification used to prevent an analysis result from impersonating provider output.
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
    /// Coverage is read from typed provider rows using a model-supplied column binding.
    ProviderDeclared(DeclaredCoverageBinding),
}

/// Whether coverage describes a semantic provider family or the control/evidence relation that
/// carries the observation. Structural presence is legal only for `ControlEvidence`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderRelationPurpose {
    SemanticFact,
    ControlEvidence,
}

/// Model/contract-supplied binding from provider relation identity to one epoch table.
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
#[derive(Clone, Debug)]
pub struct ObservedProviderRelation {
    pub identity: ProviderRelationIdentity,
    pub lane: ProviderNativeLane,
    pub batches: Vec<RecordBatch>,
}

/// Exact provider output carrying independently observable source and context pins.
#[derive(Clone, Debug)]
pub struct AcceptedProviderRelationSet {
    source_pin: SourcePin,
    context_pin: ContextPin,
    relations: BTreeMap<ProviderRelationIdentity, ObservedProviderRelation>,
}

impl AcceptedProviderRelationSet {
    /// Construct an application-owned accepted relation set.
    ///
    /// This is the generic seam used by the Rust extractor and focused contract fixtures. Exact
    /// Tree-sitter/Ruff and Pyrefly integrations should use [`Self::from_native_syntax`] and
    /// [`Self::from_pyrefly`] so their native pin carriers are checked.
    ///
    /// # Errors
    ///
    /// Rejects zero pins, duplicate relations, empty batch vectors, or schema drift within one
    /// relation.
    pub fn try_new(
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
        })
    }

    /// Validate and project the exact WP08 Tree-sitter/Ruff result without hiding empty
    /// relations.
    ///
    /// # Errors
    ///
    /// Rejects missing/malformed native pin fields, mixed source/context pins, relation metadata
    /// drift, or a release/provider identifier that differs from the exact compiled adapter.
    pub fn from_native_syntax(
        run: &ProviderNativeSyntaxRun,
    ) -> Result<Self, ProviderAdmissionError> {
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
    pub fn from_pyrefly(run: &AcceptedPyreflyRun) -> Result<Self, ProviderAdmissionError> {
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
    /// chunk is decoded again at this boundary so protocol acceptance cannot substitute a schema,
    /// owner, run, compilation unit, or source generation before catalog registration. Zero-row
    /// batches remain present and therefore prove an available family had no facts for an owner.
    ///
    /// # Errors
    ///
    /// Rejects malformed digest pins, predecessor/unknown family codes on the target route,
    /// Arrow decode/schema/row drift, mixed owner/run/unit/generation columns, or duplicate
    /// owner-family chunks.
    pub fn from_rustc(run: &AcceptedRustcCompilation) -> Result<Self, ProviderAdmissionError> {
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

    #[must_use]
    pub const fn source_pin(&self) -> SourcePin {
        self.source_pin
    }

    #[must_use]
    pub const fn context_pin(&self) -> ContextPin {
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
    for chunk in &owner.chunks {
        let relation =
            RustcRelation::from_family_code(chunk.observation_family_code).ok_or_else(|| {
                ProviderAdmissionError::InvalidObservedRelation {
                    relation: format!("provider.rustc.family.{}", chunk.observation_family_code),
                    detail: "predecessor or unknown family code reached target admission".into(),
                }
            })?;
        if !families.insert(relation) {
            return Err(ProviderAdmissionError::DuplicateObservedRelation(format!(
                "{owner_id}:{}",
                relation.relation_id()
            )));
        }
        let batch = decode_rustc_chunk(run, owner_id, relation, chunk)?;
        let identity = ProviderRelationIdentity::try_new(relation.relation_id())?;
        grouped
            .entry(identity)
            .or_insert_with(|| (ProviderNativeLane::Rustc, Vec::new()))
            .1
            .push(batch);
    }
    Ok(())
}

fn decode_rustc_chunk(
    run: &AcceptedRustcCompilation,
    owner_id: &str,
    relation: RustcRelation,
    chunk: &crate::rpc::generated::codefabric::rustc::v1::OwnerObservationChunk,
) -> Result<RecordBatch, ProviderAdmissionError> {
    let relation_name = relation.relation_id();
    if chunk.provider_run_id != run.admission.provider_run_id
        || chunk.compilation_unit_id != run.begin.compilation_unit_id
        || chunk.owner_id != owner_id
        || chunk.schema_digest != relation.schema_digest()
        || chunk.payload_reference.is_some()
    {
        return Err(ProviderAdmissionError::InvalidObservedRelation {
            relation: relation_name.to_owned(),
            detail: "rustc chunk control identity differs from the accepted run".into(),
        });
    }
    let mut reader =
        StreamReader::try_new(Cursor::new(&chunk.arrow_ipc), None).map_err(|error| {
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
        || u64::try_from(batch.num_rows()).ok() != Some(chunk.row_count)
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
}

/// Boundary proof and exact relation dispositions for one provider admission.
#[derive(Clone, Debug)]
pub struct ProviderAdmissionReport {
    pub boundary: ProviderBoundaryReport,
    pub relations: Vec<ProviderRelationAdmission>,
}

/// Successful registration keeps the still-mutable builder owned by the caller.
pub struct ProviderAdmissionOutcome {
    builder: FabricEpochBuilder,
    report: ProviderAdmissionReport,
}

/// Successful admission into the same programmatic candidate session that
/// will build every downstream CPG transformation.
pub struct ProgrammaticProviderAdmissionOutcome {
    assembly: ProgrammaticSchemaAssembly,
    report: ProviderAdmissionReport,
}

impl ProgrammaticProviderAdmissionOutcome {
    #[must_use]
    pub const fn report(&self) -> &ProviderAdmissionReport {
        &self.report
    }

    #[must_use]
    pub fn into_parts(self) -> (ProgrammaticSchemaAssembly, ProviderAdmissionReport) {
        (self.assembly, self.report)
    }
}

impl ProviderAdmissionOutcome {
    #[must_use]
    pub const fn report(&self) -> &ProviderAdmissionReport {
        &self.report
    }

    #[must_use]
    pub const fn candidate_epoch_id(&self) -> &FabricEpochId {
        self.builder.identity()
    }

    #[must_use]
    pub fn into_parts(self) -> (FabricEpochBuilder, ProviderAdmissionReport) {
        (self.builder, self.report)
    }
}

/// Model-supplied catalog binding for the capability relation derived from one accepted run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCapabilityCatalogBinding {
    pub table_name: String,
    pub source_schema_identity: Arc<str>,
}

/// Successful provider admission plus its registered, proof-qualified capability relation.
pub struct ProviderCapabilityAdmissionOutcome {
    builder: FabricEpochBuilder,
    provider_report: ProviderAdmissionReport,
    capability: ProviderCapabilityRelation,
}

impl ProviderCapabilityAdmissionOutcome {
    #[must_use]
    pub const fn provider_report(&self) -> &ProviderAdmissionReport {
        &self.provider_report
    }

    #[must_use]
    pub const fn capability(&self) -> &ProviderCapabilityRelation {
        &self.capability
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        FabricEpochBuilder,
        ProviderAdmissionReport,
        ProviderCapabilityRelation,
    ) {
        (self.builder, self.provider_report, self.capability)
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
    #[error(transparent)]
    Boundary(#[from] ProviderBoundaryError),
    #[error(transparent)]
    Capability(#[from] ProviderCapabilityError),
    #[error(transparent)]
    SchemaContract(#[from] SchemaContractError),
    #[error(transparent)]
    DataFusion(#[from] DataFusionError),
    #[error(transparent)]
    Epoch(#[from] FabricEpochError),
    #[error(transparent)]
    ProgrammaticSchema(#[from] ProgrammaticSchemaError),
}

fn syntax_lane(relation: NativeSyntaxRelation) -> ProviderNativeLane {
    if relation.as_str().starts_with("provider.tree_sitter.") {
        ProviderNativeLane::TreeSitter
    } else {
        ProviderNativeLane::Ruff
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
    // These columns are required even when this particular fact relation has zero rows. Their
    // types make the exact pin contract executable without inventing an empty-row capability.
    let _model_epoch_ids = fixed32_column(batch, "model_epoch_id", relation_name)?;
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
/// # Errors
///
/// Rejects plan, ownership, schema, pin, coverage, authority, or unexpected-output
/// contradictions. Ordinary missing output/coverage is returned as an explicit unknown.
pub fn evaluate_provider_admission(
    plan: &ProviderAdmissionPlan,
    observed: &AcceptedProviderRelationSet,
) -> Result<ProviderAdmissionReport, ProviderAdmissionError> {
    Ok(prepare_provider_admission(plan, observed)?.report)
}

/// Evaluate and register accepted raw provider relations in one consumed epoch candidate.
///
/// Consuming the builder is load-bearing: a registration failure drops the partially constructed
/// candidate instead of returning mutable partial state to the caller.
///
/// # Errors
///
/// Returns a typed admission, DataFusion, schema-contract, or epoch-registration failure.
pub fn admit_provider_relations(
    mut builder: FabricEpochBuilder,
    plan: &ProviderAdmissionPlan,
    observed: &AcceptedProviderRelationSet,
) -> Result<ProviderAdmissionOutcome, ProviderAdmissionError> {
    let prepared = prepare_provider_admission(plan, observed)?;
    for table in prepared.tables {
        let provider = Arc::new(MemTable::try_new(
            Arc::clone(&table.schema),
            vec![table.batches],
        )?);
        let contract = Arc::new(SchemaContract::try_new(
            Arc::clone(&table.binding.source_schema_identity),
            TableReference::full(
                FABRIC_CATALOG,
                table.binding.role.as_str(),
                table.binding.table_name.as_str(),
            ),
            Arc::clone(&table.schema),
            Arc::clone(&table.schema),
            (0..table.schema.fields().len())
                .map(|index| FieldIndexMapping::direct(index, index))
                .collect(),
        )?);
        builder.register_provider(
            table.binding.role,
            table.binding.table_name,
            Arc::clone(&plan.provider_kind),
            provider,
            contract,
        )?;
    }
    Ok(ProviderAdmissionOutcome {
        builder,
        report: prepared.report,
    })
}

/// Evaluate and register accepted raw provider relations directly into the
/// programmatic candidate session that owns downstream transformations.
///
/// The assembly is consumed so no partially registered candidate escapes on
/// failure. Arrow types, nullability, field identities, and relation identity
/// all come from the admitted provider schema itself.
///
/// # Errors
///
/// Returns a typed admission, schema-contract, catalog, or DataFusion failure.
pub fn admit_provider_relations_programmatic(
    mut assembly: ProgrammaticSchemaAssembly,
    plan: &ProviderAdmissionPlan,
    observed: &AcceptedProviderRelationSet,
) -> Result<ProgrammaticProviderAdmissionOutcome, ProviderAdmissionError> {
    let prepared = prepare_provider_admission(plan, observed)?;
    for table in prepared.tables {
        let provider = Arc::new(MemTable::try_new(
            Arc::clone(&table.schema),
            vec![table.batches],
        )?);
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
    Ok(ProgrammaticProviderAdmissionOutcome {
        assembly,
        report: prepared.report,
    })
}

/// Register the capability relation computed from one successfully admitted provider run.
///
/// The admission outcome is consumed so capability registration cannot be detached from the
/// exact boundary report that admitted the raw relations. Catalog naming and source-schema
/// identity remain model data. Any proof or registration contradiction drops the partially
/// constructed epoch candidate.
///
/// # Errors
///
/// Rejects invalid catalog bindings, ambiguous/unbound proof evidence, Arrow construction,
/// schema-contract drift, or epoch registration failure.
pub(crate) fn admit_provider_capability(
    outcome: ProviderAdmissionOutcome,
    binding: &ProviderCapabilityCatalogBinding,
    proofs: &[ProviderOracleProof],
) -> Result<ProviderCapabilityAdmissionOutcome, ProviderAdmissionError> {
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

    let ProviderAdmissionOutcome {
        mut builder,
        report,
    } = outcome;
    if proofs
        .iter()
        .any(|proof| proof.proof_epoch_id != *builder.identity().as_bytes())
    {
        return Err(ProviderAdmissionError::CapabilityProofEpochMismatch);
    }
    let capability = derive_provider_capability_relation(&report.boundary, proofs)?;
    let schema = Arc::clone(capability.schema());
    let provider = Arc::new(MemTable::try_new(
        Arc::clone(&schema),
        vec![vec![capability.batch().clone()]],
    )?);
    let contract = Arc::new(SchemaContract::try_new(
        Arc::clone(&binding.source_schema_identity),
        TableReference::full(
            FABRIC_CATALOG,
            FabricSchemaRole::System.as_str(),
            binding.table_name.as_str(),
        ),
        Arc::clone(&schema),
        Arc::clone(&schema),
        (0..schema.fields().len())
            .map(|index| FieldIndexMapping::direct(index, index))
            .collect(),
    )?);
    builder.register_provider(
        FabricSchemaRole::System,
        binding.table_name.clone(),
        "datafusion.mem_table.provider_capability",
        provider,
        contract,
    )?;
    Ok(ProviderCapabilityAdmissionOutcome {
        builder,
        provider_report: report,
        capability,
    })
}

/// Register provider capability derived only from the executable proof engine's sealed output.
///
/// This is the production path: model-owned bindings join provider contract families to proof
/// oracles, exact proof/candidate pins are carried into the receipt relation, and missing oracle
/// execution remains missing proof rather than an optimistic capability.
///
/// # Errors
///
/// Returns binding, proof-pin, schema, or catalog-registration failures without returning a
/// partially mutated epoch candidate.
pub fn admit_provider_capability_from_proof_relations(
    outcome: ProviderAdmissionOutcome,
    catalog_binding: &ProviderCapabilityCatalogBinding,
    proof_relations: &ProofRelations,
    oracle_bindings: &[ProviderOracleProofBinding],
) -> Result<ProviderCapabilityAdmissionOutcome, ProviderAdmissionError> {
    if proof_relations.candidate_pins().epoch != *outcome.candidate_epoch_id() {
        return Err(ProviderAdmissionError::CapabilityProofEpochMismatch);
    }
    let proofs = provider_oracle_proofs_from_executable_relations(
        &outcome.report.boundary,
        proof_relations,
        oracle_bindings,
    )?;
    admit_provider_capability(outcome, catalog_binding, &proofs)
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
        let family_coverage =
            if actual.is_none() && matches!(row.disposition, ContractDisposition::Required) {
                None
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
            let status = statuses.value(row);
            let unknown_semantics =
                unknowns.is_some_and(|values| !values.is_null(row) && values.value(row));
            if unknowns.is_some_and(|values| values.is_null(row)) {
                return Err(coverage_error(
                    relation_name,
                    "unknown-semantics field is null",
                ));
            }
            let status_unknown = status == "unknown";
            let effective_unknown = status_unknown || unknown_semantics;
            if !matches!(status, "complete" | "partial" | "unknown")
                || (status == "complete"
                    && (provider_completed != row_requested || effective_unknown))
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
            if status == "partial" && !effective_unknown && effective_completed == row_requested {
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
            any_partial |= status == "partial";
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
mod tests {
    use arrow_array::ArrayRef;
    use arrow_ipc::writer::StreamWriter;
    use arrow_schema::{DataType, Field, Schema};

    use super::*;
    use crate::fabric::epoch::{FabricEpochId, FabricEpochRuntimeConfig, FabricSchemaRole};
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
    use crate::relation_ipc::{SchemaFingerprint, TerminalStatus};
    use crate::relational_model::{FabricCompilerRelease, IntrinsicInstaller, ReplayEngine};
    use crate::rpc::generated::codefabric::rustc::v1::{
        CompilationBegin, CompilationEnd, CompilerOwnerKey, OwnerBegin, OwnerEnd,
        OwnerObservationChunk,
    };
    use crate::rustc_service::{AcceptedRustcCompilation, AcceptedRustcOwner, RustcRunAdmission};

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
                status: TerminalStatus::Partial,
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

    fn epoch_builder() -> FabricEpochBuilder {
        let runtime = FabricEpochRuntimeConfig::default();
        let release = FabricCompilerRelease::builder(
            "provider-admission-test-release",
            "source:provider-admission-test",
            "build:provider-admission-test",
        )
        .with_abis(1, 1, 1)
        .with_intrinsic_package("intrinsics-v1")
        .add_dependency("arrow", "59.2.0")
        .unwrap()
        .add_dependency("datafusion", "55.0.0")
        .unwrap()
        .add_dependency("deltalake", "43a0cf10")
        .unwrap()
        .add_provider_schema("ruff", "python-0.0.7")
        .unwrap()
        .with_policy_and_configuration("policy-v2", runtime.identity())
        .add_toolchain("rust", "1.95.0")
        .unwrap()
        .add_wire_contract("codefabric.rpc.provider-admission-test")
        .unwrap()
        .build()
        .unwrap();
        let model = Arc::new(
            ReplayEngine::new(
                release,
                IntrinsicInstaller::new("intrinsics-v1", "implementation-v1").unwrap(),
            )
            .unwrap()
            .replay(&[])
            .unwrap(),
        );
        FabricEpochBuilder::try_new(FabricEpochId::from_bytes([50; 16]), model, runtime).unwrap()
    }

    fn digest(byte: u8) -> String {
        format!("b3:{}", format!("{byte:02x}").repeat(32))
    }

    #[test]
    fn accepted_rustc_zero_fact_relation_remains_a_proved_empty_batch() {
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
        let chunk = OwnerObservationChunk {
            provider_run_id: provider_run_id.clone(),
            compilation_unit_id: compilation_unit_id.clone(),
            sequence: 2,
            owner_id: owner_id.clone(),
            observation_family_code: relation.family_code(),
            chunk_digest: digest(31),
            arrow_ipc,
            payload_reference: None,
            schema_digest: relation.schema_digest(),
            row_count: 0,
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
            chunks: vec![chunk],
            end: OwnerEnd::default(),
        };
        let accepted = AcceptedRustcCompilation {
            admission: RustcRunAdmission {
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
            begin: CompilationBegin {
                provider_run_id,
                compilation_unit_id,
                ..CompilationBegin::default()
            },
            owners: vec![owner],
            end: CompilationEnd::default(),
        };

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
    async fn typed_coverage_admits_raw_relations_and_registers_exact_contracts() {
        let (plan, observed) = accepted_plan();
        let outcome = admit_provider_relations(epoch_builder(), &plan, &observed).unwrap();
        assert_eq!(outcome.report().boundary.status, TerminalStatus::Complete);
        assert!(outcome.report().relations.iter().all(|relation| matches!(
            relation.disposition,
            ProviderRegistrationDisposition::Registered {
                coverage: TerminalStatus::Complete,
                ..
            }
        )));

        let (builder, _) = outcome.into_parts();
        let epoch = builder.seal().await.unwrap();
        let schemas = epoch
            .catalog_observation()
            .column(2)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let tables = epoch
            .catalog_observation()
            .column(3)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let admitted = (0..epoch.catalog_observation().num_rows()).any(|row| {
            !schemas.is_null(row)
                && !tables.is_null(row)
                && schemas.value(row) == FabricSchemaRole::RawRuff.as_str()
                && tables.value(row) == "token"
        });
        assert!(
            admitted,
            "raw provider relation was not queryable in the sealed catalog"
        );
    }

    #[tokio::test]
    async fn accepted_provider_capability_is_registered_only_after_oracle_proof() {
        let (plan, observed) = accepted_plan();
        let admitted = admit_provider_relations(epoch_builder(), &plan, &observed).unwrap();
        let boundary = &admitted.report().boundary;
        let proof_epoch_id = *admitted.candidate_epoch_id().as_bytes();
        let proof_oracle = OracleId::new([71; 16]).unwrap();
        let proof_relations = test_relations_with_oracle(
            FabricEpochId::from_bytes(proof_epoch_id),
            proof_oracle,
            OracleImplementationRef::new([72; 32]).unwrap(),
            Some(ProofRunId::new([73; 16]).unwrap()),
            ProofTerminalStatus::Pass,
        );
        let oracle_bindings = boundary
            .families
            .iter()
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
                source_schema_identity: Arc::from("model:provider-capability:test"),
            },
            &proof_relations,
            &oracle_bindings,
        )
        .unwrap();
        let states = admitted
            .capability()
            .batch()
            .column(24)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert!(
            states.iter().all(|state| state == Some("proved-complete")),
            "a complete accepted family with passing proof was not advertised"
        );

        let (builder, _, _) = admitted.into_parts();
        let epoch = builder.seal().await.unwrap();
        let schemas = epoch
            .catalog_observation()
            .column(2)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let tables = epoch
            .catalog_observation()
            .column(3)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert!((0..epoch.catalog_observation().num_rows()).any(|row| {
            !schemas.is_null(row)
                && !tables.is_null(row)
                && schemas.value(row) == FabricSchemaRole::System.as_str()
                && tables.value(row) == "provider_capability"
        }));
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
