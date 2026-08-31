//! Versioned application-owned analyses over accepted Rust MIR relations.
//!
//! The rustc extractor owns raw MIR coordinates and observations. This module consumes only the
//! application-owned Arrow boundary and owns the derived meaning. In particular, a raw `place_id`
//! is an occurrence identity, not a memory location: locations are reconstructed from the base
//! local and ordered projection path before any flow analysis is attempted. Public-MIR ownership,
//! alias, lifecycle, lowering, and unsafe/FFI observations are deliberately bounded
//! approximations. Exact private borrow-checker evidence remains a separate optional relation and
//! authority domain.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow_array::builder::FixedSizeBinaryBuilder;
use arrow_array::{
    Array as _, ArrayRef, BooleanArray, FixedSizeBinaryArray, RecordBatch, StringArray, UInt64Array,
};
use arrow_schema::{ArrowError, DataType, Field, Schema, SchemaRef};
use thiserror::Error;

use crate::provider_admission::ProviderAuthorityClass;
use crate::relational_program::RelationId;
use crate::rustc_relation_schema::{RUSTC_PUBLIC_RELEASE, RUSTC_TOOLCHAIN, RustcRelation};

/// Exact application algorithm release. Changing transfer semantics requires a new release.
pub const RUST_MIR_DERIVED_ANALYSIS_RELEASE: &str =
    "codefabric.rust-mir-public-derived.fixed-point.v2";
/// Precision contract of the implemented public-MIR analysis families.
pub const RUST_MIR_DERIVED_PRECISION_RELEASE: &str =
    "owner-local-may-flow-and-structural-observation.public-mir.v2";
/// Authority carried by every output row. Compiler authority is reserved for raw observations.
pub const RUST_MIR_DERIVED_AUTHORITY: &str = "application.rust-mir-derived-analysis";

const MAX_BLOCKS: usize = 131_072;
const MAX_EDGES: usize = 524_288;
const MAX_PLACES: usize = 1_048_576;
const MAX_ACCESSES: usize = 1_048_576;
const MAX_LOCALS: usize = 262_144;
const MAX_OPERANDS: usize = 1_048_576;
const MAX_RVALUES: usize = 1_048_576;
const MAX_STATEMENTS: usize = 1_048_576;
const MAX_TERMINATORS: usize = 131_072;
const MAX_CALLS: usize = 262_144;
const MAX_INSTANCES: usize = 262_144;

/// One stable owner key accepted from the compiler adapter's private stable-identity seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RustStableOwnerKey {
    pub stable_crate_id: u64,
    pub def_path_hash: [u8; 16],
}

/// Exact immutable pins repeated on every derived row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustMirAnalysisProvenance {
    pub model_epoch_id: [u8; 32],
    pub source_snapshot_pin: [u8; 32],
    pub analysis_context_pin: [u8; 32],
    pub source_generation: u64,
    pub provider_run_id: Arc<str>,
    pub compilation_unit_id: Arc<str>,
    pub owner_id: Arc<str>,
    pub source_file_id: Arc<str>,
    pub source_content_digest: [u8; 32],
    pub stable_owner_key: Option<RustStableOwnerKey>,
    pub rustc_release: Arc<str>,
    pub rustc_commit: Arc<str>,
    pub rustc_toolchain: Arc<str>,
    pub toolchain_identity_digest: [u8; 32],
    pub raw_schema_bundle_digest: [u8; 32],
}

/// Whether one accepted raw relation is complete for this owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustMirRelationCompleteness {
    Complete,
    Partial { reason: Arc<str> },
    Unavailable { reason: Arc<str> },
}

impl RustMirRelationCompleteness {
    const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete)
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Complete => None,
            Self::Partial { reason } | Self::Unavailable { reason } => Some(reason),
        }
    }

    const fn status(&self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial { .. } => "partial",
            Self::Unavailable { .. } => "unavailable",
        }
    }
}

/// Completeness of every raw relation consumed by this analysis release.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustMirInputCompleteness {
    pub blocks: RustMirRelationCompleteness,
    pub locals: RustMirRelationCompleteness,
    pub places: RustMirRelationCompleteness,
    pub operands: RustMirRelationCompleteness,
    pub rvalues: RustMirRelationCompleteness,
    pub statements: RustMirRelationCompleteness,
    pub terminators: RustMirRelationCompleteness,
    pub cfg_edges: RustMirRelationCompleteness,
    pub calls: RustMirRelationCompleteness,
    pub instances: RustMirRelationCompleteness,
    pub accesses: RustMirRelationCompleteness,
}

impl RustMirInputCompleteness {
    #[must_use]
    pub const fn complete() -> Self {
        Self {
            blocks: RustMirRelationCompleteness::Complete,
            locals: RustMirRelationCompleteness::Complete,
            places: RustMirRelationCompleteness::Complete,
            operands: RustMirRelationCompleteness::Complete,
            rvalues: RustMirRelationCompleteness::Complete,
            statements: RustMirRelationCompleteness::Complete,
            terminators: RustMirRelationCompleteness::Complete,
            cfg_edges: RustMirRelationCompleteness::Complete,
            calls: RustMirRelationCompleteness::Complete,
            instances: RustMirRelationCompleteness::Complete,
            accesses: RustMirRelationCompleteness::Complete,
        }
    }
}

/// Accepted application-owned rustc relation batches for one MIR owner.
#[derive(Clone, Debug)]
pub struct RustMirRawRelations {
    pub blocks: Vec<RecordBatch>,
    pub locals: Vec<RecordBatch>,
    pub places: Vec<RecordBatch>,
    pub operands: Vec<RecordBatch>,
    pub rvalues: Vec<RecordBatch>,
    pub statements: Vec<RecordBatch>,
    pub terminators: Vec<RecordBatch>,
    pub cfg_edges: Vec<RecordBatch>,
    pub calls: Vec<RecordBatch>,
    pub instances: Vec<RecordBatch>,
    pub accesses: Vec<RecordBatch>,
    pub completeness: RustMirInputCompleteness,
    pub private_enrichment: Option<RustMirPrivateEnrichmentInput>,
}

/// Static output roles for this algorithm contract. Semantic relation identities are model data.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RustMirDerivedRelation {
    CfgEdge,
    DefUse,
    ReachingDefinition,
    Liveness,
    OwnershipState,
    AliasPointsTo,
    ResourceLifecycle,
    AsyncLowering,
    UnsafeFfi,
    ControlDependenceInput,
    Unknown,
}

impl RustMirDerivedRelation {
    pub const ALL: [Self; 11] = [
        Self::CfgEdge,
        Self::DefUse,
        Self::ReachingDefinition,
        Self::Liveness,
        Self::OwnershipState,
        Self::AliasPointsTo,
        Self::ResourceLifecycle,
        Self::AsyncLowering,
        Self::UnsafeFfi,
        Self::ControlDependenceInput,
        Self::Unknown,
    ];

    fn schema_with_bindings(self, bindings: &RustMirAnalysisBindings) -> SchemaRef {
        let mut fields = common_output_fields();
        match self {
            Self::CfgEdge => fields.extend([
                Field::new("edge_id", DataType::FixedSizeBinary(32), false),
                Field::new("source_block", DataType::UInt64, false),
                Field::new("target_block", DataType::UInt64, false),
                Field::new("edge_kind", DataType::Utf8, false),
                Field::new("branch_value_u128", DataType::Utf8, true),
                Field::new("unwind_action", DataType::Utf8, true),
            ]),
            Self::DefUse => fields.extend([
                Field::new("memory_location_id", DataType::FixedSizeBinary(32), false),
                Field::new("base_local", DataType::UInt64, false),
                Field::new("projection_path", DataType::Utf8, false),
                Field::new("definition_event_id", DataType::FixedSizeBinary(32), false),
                Field::new("definition_place_id", DataType::FixedSizeBinary(32), false),
                Field::new("definition_block", DataType::UInt64, false),
                Field::new("definition_slot_kind", DataType::Utf8, false),
                Field::new("definition_slot_index", DataType::UInt64, false),
                Field::new("definition_access_ordinal", DataType::UInt64, false),
                Field::new("definition_access_kind", DataType::Utf8, false),
                Field::new("definition_structured_evidence", DataType::Utf8, false),
                Field::new("definition_runtime_effect", DataType::Boolean, false),
                Field::new("use_event_id", DataType::FixedSizeBinary(32), false),
                Field::new("use_place_id", DataType::FixedSizeBinary(32), false),
                Field::new("use_block", DataType::UInt64, false),
                Field::new("use_slot_kind", DataType::Utf8, false),
                Field::new("use_slot_index", DataType::UInt64, false),
                Field::new("use_access_ordinal", DataType::UInt64, false),
                Field::new("use_access_kind", DataType::Utf8, false),
                Field::new("use_structured_evidence", DataType::Utf8, false),
                Field::new("use_runtime_effect", DataType::Boolean, false),
                Field::new("certainty", DataType::Utf8, false),
            ]),
            Self::ReachingDefinition => fields.extend([
                Field::new("block_index", DataType::UInt64, false),
                Field::new("boundary", DataType::Utf8, false),
                Field::new("memory_location_id", DataType::FixedSizeBinary(32), false),
                Field::new("base_local", DataType::UInt64, false),
                Field::new("projection_path", DataType::Utf8, false),
                Field::new("definition_event_id", DataType::FixedSizeBinary(32), false),
                Field::new("definition_place_id", DataType::FixedSizeBinary(32), false),
                Field::new("definition_block", DataType::UInt64, false),
                Field::new("definition_slot_kind", DataType::Utf8, false),
                Field::new("definition_slot_index", DataType::UInt64, false),
                Field::new("definition_access_ordinal", DataType::UInt64, false),
                Field::new("definition_access_kind", DataType::Utf8, false),
                Field::new("definition_structured_evidence", DataType::Utf8, false),
                Field::new("definition_runtime_effect", DataType::Boolean, false),
            ]),
            Self::Liveness => fields.extend([
                Field::new("block_index", DataType::UInt64, false),
                Field::new("boundary", DataType::Utf8, false),
                Field::new("memory_location_id", DataType::FixedSizeBinary(32), false),
                Field::new("base_local", DataType::UInt64, false),
                Field::new("projection_path", DataType::Utf8, false),
            ]),
            Self::OwnershipState => fields.extend([
                Field::new("event_id", DataType::FixedSizeBinary(32), false),
                Field::new("place_id", DataType::FixedSizeBinary(32), false),
                Field::new("memory_location_id", DataType::FixedSizeBinary(32), false),
                Field::new("base_local", DataType::UInt64, false),
                Field::new("projection_path", DataType::Utf8, false),
                Field::new("local_role", DataType::Utf8, true),
                Field::new("local_type_key", DataType::FixedSizeBinary(32), true),
                Field::new("local_mutability", DataType::Utf8, true),
                Field::new("block_index", DataType::UInt64, false),
                Field::new("slot_kind", DataType::Utf8, false),
                Field::new("slot_index", DataType::UInt64, false),
                Field::new("access_ordinal", DataType::UInt64, false),
                Field::new("access_kind", DataType::Utf8, false),
                Field::new("ownership_observation", DataType::Utf8, false),
                Field::new("structured_evidence", DataType::Utf8, false),
            ]),
            Self::AliasPointsTo => fields.extend([
                Field::new("alias_observation_id", DataType::FixedSizeBinary(32), false),
                Field::new("pointer_place_id", DataType::FixedSizeBinary(32), false),
                Field::new("pointer_location_id", DataType::FixedSizeBinary(32), false),
                Field::new("pointee_place_id", DataType::FixedSizeBinary(32), false),
                Field::new("pointee_location_id", DataType::FixedSizeBinary(32), false),
                Field::new("block_index", DataType::UInt64, false),
                Field::new("statement_index", DataType::UInt64, false),
                Field::new("rvalue_kind", DataType::Utf8, false),
                Field::new("normalized_effect", DataType::Utf8, false),
                Field::new("source_scope", DataType::UInt64, false),
                Field::new("region_kind", DataType::Utf8, true),
                Field::new("mutability", DataType::Utf8, true),
                Field::new("relation_kind", DataType::Utf8, false),
            ]),
            Self::ResourceLifecycle => fields.extend([
                Field::new("lifecycle_event_id", DataType::FixedSizeBinary(32), false),
                Field::new("place_id", DataType::FixedSizeBinary(32), false),
                Field::new("memory_location_id", DataType::FixedSizeBinary(32), false),
                Field::new("base_local", DataType::UInt64, false),
                Field::new("projection_path", DataType::Utf8, false),
                Field::new("block_index", DataType::UInt64, false),
                Field::new("slot_kind", DataType::Utf8, false),
                Field::new("slot_index", DataType::UInt64, false),
                Field::new("lifecycle_event", DataType::Utf8, false),
                Field::new("structured_evidence", DataType::Utf8, false),
            ]),
            Self::AsyncLowering => fields.extend([
                Field::new("observation_id", DataType::FixedSizeBinary(32), false),
                Field::new("block_index", DataType::UInt64, false),
                Field::new("statement_index", DataType::UInt64, false),
                Field::new("source_scope", DataType::UInt64, false),
                Field::new("rvalue_kind", DataType::Utf8, false),
                Field::new("aggregate_kind", DataType::Utf8, false),
                Field::new("result_type_key", DataType::FixedSizeBinary(32), true),
                Field::new("observation_kind", DataType::Utf8, false),
            ]),
            Self::UnsafeFfi => fields.extend([
                Field::new("observation_id", DataType::FixedSizeBinary(32), false),
                Field::new("block_index", DataType::UInt64, false),
                Field::new("slot_kind", DataType::Utf8, false),
                Field::new("slot_index", DataType::UInt64, false),
                Field::new("source_scope", DataType::UInt64, false),
                Field::new("observation_kind", DataType::Utf8, false),
                Field::new("raw_kind", DataType::Utf8, false),
                Field::new("declared_target", DataType::Utf8, true),
                Field::new("resolved_instance_key", DataType::FixedSizeBinary(32), true),
                Field::new("is_foreign_item", DataType::Boolean, true),
                Field::new("structured_evidence", DataType::Utf8, false),
            ]),
            Self::ControlDependenceInput => fields.extend([
                Field::new("control_input_id", DataType::FixedSizeBinary(32), false),
                Field::new("controller_block", DataType::UInt64, false),
                Field::new("controller_kind", DataType::Utf8, false),
                Field::new("predicate_operand_id", DataType::FixedSizeBinary(32), true),
                Field::new("predicate_role", DataType::Utf8, true),
                Field::new("predicate_operand_kind", DataType::Utf8, true),
                Field::new("source_scope", DataType::UInt64, false),
                Field::new("normal_target_count", DataType::UInt64, false),
                Field::new("unwind_action", DataType::Utf8, true),
                Field::new("edge_id", DataType::FixedSizeBinary(32), false),
                Field::new("target_block", DataType::UInt64, false),
                Field::new("edge_kind", DataType::Utf8, false),
                Field::new("is_unwind", DataType::Boolean, false),
            ]),
            Self::Unknown => fields.extend([
                Field::new("family", DataType::Utf8, false),
                Field::new("reason_code", DataType::Utf8, false),
                Field::new("detail", DataType::Utf8, false),
                Field::new("bounded", DataType::Boolean, false),
                Field::new("input_relation", DataType::Utf8, true),
            ]),
        }
        Arc::new(Schema::new_with_metadata(
            fields,
            [
                (
                    "codefabric.relation_id".to_owned(),
                    bindings.relation_id(self).as_str().to_owned(),
                ),
                (
                    "codefabric.authority".to_owned(),
                    RUST_MIR_DERIVED_AUTHORITY.to_owned(),
                ),
                (
                    "codefabric.algorithm_release".to_owned(),
                    RUST_MIR_DERIVED_ANALYSIS_RELEASE.to_owned(),
                ),
                (
                    "codefabric.precision_release".to_owned(),
                    RUST_MIR_DERIVED_PRECISION_RELEASE.to_owned(),
                ),
                (
                    "codefabric.semantic_encoding".to_owned(),
                    "typed-arrow-application-analysis".to_owned(),
                ),
            ]
            .into_iter()
            .collect(),
        ))
    }

    /// Build the Arrow schema after validating model relation and authority bindings.
    ///
    /// # Errors
    ///
    /// Rejects duplicate semantic relation identities or non-application authority.
    pub fn schema(
        self,
        bindings: &RustMirAnalysisBindings,
    ) -> Result<SchemaRef, RustMirAnalysisError> {
        bindings.validate()?;
        Ok(self.schema_with_bindings(bindings))
    }
}

/// Model-selected target relation identities for each static algorithm output role.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustMirAnalysisRelations {
    pub cfg_edges: RelationId,
    pub def_use: RelationId,
    pub reaching_definitions: RelationId,
    pub liveness: RelationId,
    pub ownership_state: RelationId,
    pub alias_points_to: RelationId,
    pub resource_lifecycle: RelationId,
    pub async_lowering: RelationId,
    pub unsafe_ffi: RelationId,
    pub control_dependence_inputs: RelationId,
    pub unknowns: RelationId,
}

/// Complete model binding for one Rust MIR analysis compilation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustMirAnalysisBindings {
    pub relations: RustMirAnalysisRelations,
    pub authority_class: ProviderAuthorityClass,
    pub private_enrichment: Option<RustMirPrivateEnrichmentBinding>,
}

impl RustMirAnalysisBindings {
    #[must_use]
    pub fn relation_id(&self, role: RustMirDerivedRelation) -> &RelationId {
        match role {
            RustMirDerivedRelation::CfgEdge => &self.relations.cfg_edges,
            RustMirDerivedRelation::DefUse => &self.relations.def_use,
            RustMirDerivedRelation::ReachingDefinition => &self.relations.reaching_definitions,
            RustMirDerivedRelation::Liveness => &self.relations.liveness,
            RustMirDerivedRelation::OwnershipState => &self.relations.ownership_state,
            RustMirDerivedRelation::AliasPointsTo => &self.relations.alias_points_to,
            RustMirDerivedRelation::ResourceLifecycle => &self.relations.resource_lifecycle,
            RustMirDerivedRelation::AsyncLowering => &self.relations.async_lowering,
            RustMirDerivedRelation::UnsafeFfi => &self.relations.unsafe_ffi,
            RustMirDerivedRelation::ControlDependenceInput => {
                &self.relations.control_dependence_inputs
            }
            RustMirDerivedRelation::Unknown => &self.relations.unknowns,
        }
    }

    fn validate(&self) -> Result<(), RustMirAnalysisError> {
        if self.authority_class != ProviderAuthorityClass::RustApplicationDerived {
            return Err(RustMirAnalysisError::InvalidBinding(
                "Rust MIR derived relations require RustApplicationDerived authority".to_owned(),
            ));
        }
        let identities = RustMirDerivedRelation::ALL
            .into_iter()
            .map(|role| self.relation_id(role).as_str())
            .collect::<BTreeSet<_>>();
        if identities.len() != RustMirDerivedRelation::ALL.len() {
            return Err(RustMirAnalysisError::InvalidBinding(
                "Rust MIR output roles require distinct model relation identities".to_owned(),
            ));
        }
        if let Some(private) = &self.private_enrichment
            && identities.contains(private.relation_id.as_str())
        {
            return Err(RustMirAnalysisError::InvalidBinding(
                "private borrowck relation identity must be distinct from application outputs"
                    .to_owned(),
            ));
        }
        Ok(())
    }
}

/// Exact private-provider authority remains distinct from application approximations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RustMirPrivateAuthority {
    ExactBorrowck,
}

impl RustMirPrivateAuthority {
    const fn label(self) -> &'static str {
        match self {
            Self::ExactBorrowck => "provider.rustc-private.borrowck-exact",
        }
    }
}

/// Model binding for the optional exact private borrowck relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustMirPrivateEnrichmentBinding {
    pub relation_id: RelationId,
    pub authority: RustMirPrivateAuthority,
}

/// One exact loan/region observation supplied by a separately admitted private provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustMirExactBorrowckRow {
    pub loan_id: [u8; 32],
    pub place_id: [u8; 32],
    pub region_id: [u8; 32],
    pub loan_kind: Arc<str>,
    pub issued_block: u64,
    pub issued_slot_kind: Arc<str>,
    pub issued_slot_index: u64,
    pub killed_block: Option<u64>,
    pub killed_slot_kind: Option<Arc<str>>,
    pub killed_slot_index: Option<u64>,
}

/// Optional private-provider input. Its identity and completeness are not application authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustMirPrivateEnrichmentInput {
    pub provider_run_id: Arc<str>,
    pub provider_release: Arc<str>,
    pub source_generation: u64,
    pub stable_owner_key: RustStableOwnerKey,
    pub toolchain_identity_digest: [u8; 32],
    pub completeness: RustMirRelationCompleteness,
    pub rows: Vec<RustMirExactBorrowckRow>,
}

/// Distinct exact-private Arrow output. It is never inserted into application-derived relations.
#[derive(Clone, Debug)]
pub struct RustMirPrivateEnrichmentOutput {
    pub authority: RustMirPrivateAuthority,
    pub relation_id: RelationId,
    pub completeness: RustMirAnalysisCompleteness,
    pub batch: RecordBatch,
}

/// Causal observation of what this execution actually produced.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustMirAnalysisObservation {
    pub relations: RustMirAnalysisRelations,
    pub algorithm_release: Arc<str>,
    pub precision_release: Arc<str>,
    pub authority_class: ProviderAuthorityClass,
    pub cfg_complete: bool,
    pub dataflow_complete: bool,
    pub relation_completeness: BTreeMap<RustMirDerivedRelation, RustMirAnalysisCompleteness>,
    pub unsupported_families: BTreeSet<Arc<str>>,
    pub output_rows: BTreeMap<RustMirDerivedRelation, u64>,
    pub private_enrichment_completeness: Option<RustMirAnalysisCompleteness>,
}

/// Execution-derived completeness for one output role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RustMirAnalysisCompleteness {
    Complete,
    Partial,
    Unknown,
}

impl RustMirAnalysisCompleteness {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Unknown => "unknown",
        }
    }
}

/// Typed Arrow outputs and their execution-derived observation.
#[derive(Clone, Debug)]
pub struct RustMirAnalysisOutput {
    relations: BTreeMap<RustMirDerivedRelation, RecordBatch>,
    observation: RustMirAnalysisObservation,
    private_enrichment: Option<RustMirPrivateEnrichmentOutput>,
}

impl RustMirAnalysisOutput {
    #[must_use]
    pub fn relation(&self, relation: RustMirDerivedRelation) -> &RecordBatch {
        self.relations
            .get(&relation)
            .expect("all derived Rust MIR relations are materialized")
    }

    #[must_use]
    pub const fn observation(&self) -> &RustMirAnalysisObservation {
        &self.observation
    }

    #[must_use]
    pub const fn private_enrichment(&self) -> Option<&RustMirPrivateEnrichmentOutput> {
        self.private_enrichment.as_ref()
    }
}

#[derive(Debug, Error)]
pub enum RustMirAnalysisError {
    #[error(transparent)]
    Arrow(#[from] ArrowError),
    #[error("raw Rust MIR relation {relation} has a non-contract schema")]
    InvalidRawSchema { relation: &'static str },
    #[error("raw Rust MIR column {column} is missing or has the wrong type")]
    InvalidRawColumn { column: &'static str },
    #[error("raw Rust MIR provenance does not match the admitted analysis pin: {0}")]
    ProvenanceMismatch(String),
    #[error("raw Rust MIR structure is invalid: {0}")]
    InvalidStructure(String),
    #[error("Rust MIR analysis model binding is invalid: {0}")]
    InvalidBinding(String),
    #[error("Rust MIR analysis resource bound exceeded for {family}: {actual} > {maximum}")]
    ResourceBound {
        family: &'static str,
        actual: usize,
        maximum: usize,
    },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct BlockRow {
    block_index: u64,
    statement_count: u64,
    terminator_kind: Arc<str>,
    is_entry: bool,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct EdgeRow {
    edge_id: [u8; 32],
    source_block: u64,
    target_block: u64,
    edge_kind: Arc<str>,
    branch_value: Option<Arc<str>>,
    unwind_action: Option<Arc<str>>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ProjectionRow {
    ordinal: Option<u64>,
    kind: Arc<str>,
    local_or_field: Option<u64>,
    offset: Option<u64>,
    min_length: Option<u64>,
    slice_to: Option<u64>,
    from_end: Option<bool>,
    type_key: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MemoryLocation {
    id: [u8; 32],
    base_local: u64,
    projection_path: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PlaceObservation {
    location: MemoryLocation,
    block_index: u64,
    slot_kind: Arc<str>,
    slot_index: u64,
    occurrence_ordinal: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EventRow {
    event_id: [u8; 32],
    place_id: [u8; 32],
    location: MemoryLocation,
    block_index: u64,
    slot_kind: Arc<str>,
    slot_index: u64,
    access_ordinal: u64,
    access_kind: Arc<str>,
    structured_evidence: Arc<str>,
    runtime_effect: bool,
    is_use: bool,
    is_definition: bool,
    is_kill: bool,
}

impl EventRow {
    fn coordinate_key(&self) -> (u8, u64, u8, u64, [u8; 32]) {
        (
            slot_kind_rank(&self.slot_kind),
            self.slot_index,
            self.evaluation_phase_rank(),
            self.access_ordinal,
            self.place_id,
        )
    }

    /// MIR rvalue/call operands are evaluated before their destination write. Provider ordinals
    /// are role-local, so they cannot establish this cross-role order by themselves.
    const fn evaluation_phase_rank(&self) -> u8 {
        if self.is_use && !self.is_kill && !self.is_definition {
            0
        } else if self.is_use {
            1
        } else {
            2
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DefUseRow {
    location_id: [u8; 32],
    base_local: u64,
    projection_path: Arc<str>,
    definition: EventRef,
    use_event: EventRef,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct EventRef {
    event_id: [u8; 32],
    place_id: [u8; 32],
    block_index: u64,
    slot_kind: Arc<str>,
    slot_index: u64,
    access_ordinal: u64,
    access_kind: Arc<str>,
    structured_evidence: Arc<str>,
    runtime_effect: bool,
}

impl From<&EventRow> for EventRef {
    fn from(value: &EventRow) -> Self {
        Self {
            event_id: value.event_id,
            place_id: value.place_id,
            block_index: value.block_index,
            slot_kind: Arc::clone(&value.slot_kind),
            slot_index: value.slot_index,
            access_ordinal: value.access_ordinal,
            access_kind: Arc::clone(&value.access_kind),
            structured_evidence: Arc::clone(&value.structured_evidence),
            runtime_effect: value.runtime_effect,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ReachingRow {
    block_index: u64,
    boundary: &'static str,
    location_id: [u8; 32],
    base_local: u64,
    projection_path: Arc<str>,
    definition: EventRef,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LivenessRow {
    block_index: u64,
    boundary: &'static str,
    location_id: [u8; 32],
    base_local: u64,
    projection_path: Arc<str>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct UnknownRow {
    family: Arc<str>,
    reason_code: Arc<str>,
    detail: Arc<str>,
    bounded: bool,
    input_relation: Option<Arc<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LocalObservation {
    local_index: u64,
    local_role: Arc<str>,
    type_key: [u8; 32],
    mutability: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OperandObservation {
    operand_id: [u8; 32],
    block_index: u64,
    slot_kind: Arc<str>,
    slot_index: u64,
    parent_role: Arc<str>,
    operand_ordinal: u64,
    operand_kind: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RvalueObservation {
    block_index: u64,
    statement_index: u64,
    rvalue_kind: Arc<str>,
    result_type_key: Option<[u8; 32]>,
    cast_kind: Option<Arc<str>>,
    aggregate_kind: Option<Arc<str>>,
    source_place_id: Option<[u8; 32]>,
    region_kind: Option<Arc<str>>,
    mutability: Option<Arc<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StatementObservation {
    block_index: u64,
    statement_index: u64,
    raw_kind: Arc<str>,
    normalized_effect: Arc<str>,
    source_scope: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TerminatorObservation {
    block_index: u64,
    raw_kind: Arc<str>,
    source_scope: u64,
    normal_target_count: u64,
    unwind_action: Option<Arc<str>>,
    destination_place_id: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CallObservation {
    block_index: u64,
    destination_place_id: [u8; 32],
    declared_target: Option<Arc<str>>,
    resolved_instance_key: Option<[u8; 32]>,
    dispatch_kind: Arc<str>,
    resolution_confidence: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InstanceObservation {
    instance_key: [u8; 32],
    definition_path: Arc<str>,
    is_foreign_item: bool,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct OwnershipRow {
    event: EventRef,
    location_id: [u8; 32],
    base_local: u64,
    projection_path: Arc<str>,
    local_role: Option<Arc<str>>,
    local_type_key: Option<[u8; 32]>,
    local_mutability: Option<Arc<str>>,
    ownership_observation: Arc<str>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AliasRow {
    observation_id: [u8; 32],
    pointer_place_id: [u8; 32],
    pointer_location_id: [u8; 32],
    pointee_place_id: [u8; 32],
    pointee_location_id: [u8; 32],
    block_index: u64,
    statement_index: u64,
    rvalue_kind: Arc<str>,
    normalized_effect: Arc<str>,
    source_scope: u64,
    region_kind: Option<Arc<str>>,
    mutability: Option<Arc<str>>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ResourceRow {
    lifecycle_event_id: [u8; 32],
    event: EventRef,
    location_id: [u8; 32],
    base_local: u64,
    projection_path: Arc<str>,
    lifecycle_event: Arc<str>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AsyncRow {
    observation_id: [u8; 32],
    block_index: u64,
    statement_index: u64,
    source_scope: u64,
    rvalue_kind: Arc<str>,
    aggregate_kind: Arc<str>,
    result_type_key: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct UnsafeFfiRow {
    observation_id: [u8; 32],
    block_index: u64,
    slot_kind: Arc<str>,
    slot_index: u64,
    source_scope: u64,
    observation_kind: Arc<str>,
    raw_kind: Arc<str>,
    declared_target: Option<Arc<str>>,
    resolved_instance_key: Option<[u8; 32]>,
    is_foreign_item: Option<bool>,
    structured_evidence: Arc<str>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ControlInputRow {
    control_input_id: [u8; 32],
    controller_block: u64,
    controller_kind: Arc<str>,
    predicate_operand_id: Option<[u8; 32]>,
    predicate_role: Option<Arc<str>>,
    predicate_operand_kind: Option<Arc<str>>,
    source_scope: u64,
    normal_target_count: u64,
    unwind_action: Option<Arc<str>>,
    edge_id: [u8; 32],
    target_block: u64,
    edge_kind: Arc<str>,
    is_unwind: bool,
}

type DefinitionState = BTreeMap<[u8; 32], BTreeSet<[u8; 32]>>;

/// Digest of the exact application-owned raw relation schemas consumed by this release.
#[must_use]
pub fn expected_rust_mir_input_schema_bundle_digest() -> [u8; 32] {
    let digests = [
        RustcRelation::MirBlock.schema_digest(),
        RustcRelation::MirLocal.schema_digest(),
        RustcRelation::MirPlace.schema_digest(),
        RustcRelation::MirOperand.schema_digest(),
        RustcRelation::MirRvalue.schema_digest(),
        RustcRelation::MirStatement.schema_digest(),
        RustcRelation::MirTerminator.schema_digest(),
        RustcRelation::CfgEdge.schema_digest(),
        RustcRelation::Call.schema_digest(),
        RustcRelation::Instance.schema_digest(),
        RustcRelation::Access.schema_digest(),
    ];
    hash_parts(
        b"codefabric.rust-mir-derived-input-schemas.v1\0",
        digests.iter().map(String::as_bytes),
    )
}

/// Execute the versioned owner-local CFG, def-use, reaching-definition, and liveness program.
///
/// Structurally invalid relation batches fail closed. Declared partial/unavailable inputs and
/// semantically unclassified access variants instead produce explicit unknown rows, so an empty
/// derived relation never masquerades as proof of absence.
///
/// # Errors
///
/// Returns an error for a schema/pin mismatch, invalid CFG, duplicate provider coordinates, or a
/// configured resource-bound violation.
pub fn analyze_rust_mir_relations(
    provenance: &RustMirAnalysisProvenance,
    raw: &RustMirRawRelations,
    bindings: &RustMirAnalysisBindings,
) -> Result<RustMirAnalysisOutput, RustMirAnalysisError> {
    bindings.validate()?;
    validate_provenance(provenance)?;
    validate_completeness(&raw.completeness)?;
    validate_relation_batches(RustcRelation::MirBlock, &raw.blocks, provenance)?;
    validate_relation_batches(RustcRelation::MirLocal, &raw.locals, provenance)?;
    validate_relation_batches(RustcRelation::MirPlace, &raw.places, provenance)?;
    validate_relation_batches(RustcRelation::MirOperand, &raw.operands, provenance)?;
    validate_relation_batches(RustcRelation::MirRvalue, &raw.rvalues, provenance)?;
    validate_relation_batches(RustcRelation::MirStatement, &raw.statements, provenance)?;
    validate_relation_batches(RustcRelation::MirTerminator, &raw.terminators, provenance)?;
    validate_relation_batches(RustcRelation::CfgEdge, &raw.cfg_edges, provenance)?;
    validate_relation_batches(RustcRelation::Call, &raw.calls, provenance)?;
    validate_relation_batches(RustcRelation::Instance, &raw.instances, provenance)?;
    validate_relation_batches(RustcRelation::Access, &raw.accesses, provenance)?;

    bound("blocks", row_count(&raw.blocks), MAX_BLOCKS)?;
    bound("locals", row_count(&raw.locals), MAX_LOCALS)?;
    bound("cfg-edges", row_count(&raw.cfg_edges), MAX_EDGES)?;
    bound("places", row_count(&raw.places), MAX_PLACES)?;
    bound("operands", row_count(&raw.operands), MAX_OPERANDS)?;
    bound("rvalues", row_count(&raw.rvalues), MAX_RVALUES)?;
    bound("statements", row_count(&raw.statements), MAX_STATEMENTS)?;
    bound("terminators", row_count(&raw.terminators), MAX_TERMINATORS)?;
    bound("calls", row_count(&raw.calls), MAX_CALLS)?;
    bound("instances", row_count(&raw.instances), MAX_INSTANCES)?;
    bound("accesses", row_count(&raw.accesses), MAX_ACCESSES)?;

    let private_enrichment = materialize_private_enrichment(
        provenance,
        raw.private_enrichment.as_ref(),
        bindings.private_enrichment.as_ref(),
    )?;
    let private_completeness = private_enrichment
        .as_ref()
        .map(|output| output.completeness);
    let exact_private_complete =
        private_completeness == Some(RustMirAnalysisCompleteness::Complete);

    let blocks = parse_blocks(&raw.blocks, raw.completeness.blocks.is_complete())?;
    let mut unknowns = unsupported_remainders(exact_private_complete);
    append_input_unknowns(&mut unknowns, &raw.completeness);

    let stable_identity_available = provenance.stable_owner_key.is_some();
    if !stable_identity_available {
        for family in [
            "cfg",
            "def-use",
            "reaching-definition",
            "liveness",
            "ownership-state",
            "alias-points-to",
            "drop-resource",
            "async-lowering",
            "unsafe-ffi",
            "control-dependence-input",
        ] {
            unknowns.insert(UnknownRow {
                family: Arc::from(family),
                reason_code: Arc::from("STABLE_OWNER_IDENTITY_UNAVAILABLE"),
                detail: Arc::from(
                    "StableCrateId plus DefPathHash is absent; no canonical derived identity is invented",
                ),
                bounded: true,
                input_relation: Some(Arc::from(RustcRelation::MirBlock.relation_id())),
            });
        }
    }

    let cfg_status = derived_completeness(
        [&raw.completeness.blocks, &raw.completeness.cfg_edges],
        stable_identity_available,
    );
    let cfg_rows = if cfg_status == RustMirAnalysisCompleteness::Complete {
        parse_and_validate_edges(&raw.cfg_edges, &blocks, provenance)?
    } else {
        Vec::new()
    };

    let mut def_use_rows = Vec::new();
    let mut reaching_rows = Vec::new();
    let mut liveness_rows = Vec::new();
    let mut dataflow_status = derived_completeness(
        [
            &raw.completeness.blocks,
            &raw.completeness.places,
            &raw.completeness.cfg_edges,
            &raw.completeness.accesses,
        ],
        stable_identity_available,
    );

    if dataflow_status == RustMirAnalysisCompleteness::Complete {
        let locations = parse_places(&raw.places, provenance)?;
        let (events, access_unknowns) = parse_accesses(&raw.accesses, &locations, provenance)?;
        if !access_unknowns.is_empty() {
            dataflow_status = RustMirAnalysisCompleteness::Partial;
            unknowns.extend(access_unknowns);
        }
        let analysis = execute_dataflow(&blocks, &cfg_rows, &events, &locations)?;
        if !analysis.unknowns.is_empty() {
            dataflow_status = RustMirAnalysisCompleteness::Partial;
        }
        def_use_rows = analysis.def_use;
        reaching_rows = analysis.reaching;
        liveness_rows = analysis.liveness;
        unknowns.extend(analysis.unknowns);
    }

    let extended = derive_extended_analyses(provenance, raw, &blocks, &cfg_rows)?;
    unknowns.extend(extended.unknowns.iter().cloned());

    let mut relations = BTreeMap::new();
    relations.insert(
        RustMirDerivedRelation::CfgEdge,
        cfg_batch(provenance, &cfg_rows, bindings, cfg_status.as_str())?,
    );
    relations.insert(
        RustMirDerivedRelation::DefUse,
        def_use_batch(
            provenance,
            &def_use_rows,
            bindings,
            dataflow_status.as_str(),
        )?,
    );
    relations.insert(
        RustMirDerivedRelation::ReachingDefinition,
        reaching_batch(
            provenance,
            &reaching_rows,
            bindings,
            dataflow_status.as_str(),
        )?,
    );
    relations.insert(
        RustMirDerivedRelation::Liveness,
        liveness_batch(
            provenance,
            &liveness_rows,
            bindings,
            dataflow_status.as_str(),
        )?,
    );
    relations.insert(
        RustMirDerivedRelation::OwnershipState,
        ownership_batch(
            provenance,
            &extended.ownership,
            bindings,
            extended.statuses[&RustMirDerivedRelation::OwnershipState].as_str(),
        )?,
    );
    relations.insert(
        RustMirDerivedRelation::AliasPointsTo,
        alias_batch(
            provenance,
            &extended.aliases,
            bindings,
            extended.statuses[&RustMirDerivedRelation::AliasPointsTo].as_str(),
        )?,
    );
    relations.insert(
        RustMirDerivedRelation::ResourceLifecycle,
        resource_batch(
            provenance,
            &extended.resources,
            bindings,
            extended.statuses[&RustMirDerivedRelation::ResourceLifecycle].as_str(),
        )?,
    );
    relations.insert(
        RustMirDerivedRelation::AsyncLowering,
        async_batch(
            provenance,
            &extended.async_lowering,
            bindings,
            extended.statuses[&RustMirDerivedRelation::AsyncLowering].as_str(),
        )?,
    );
    relations.insert(
        RustMirDerivedRelation::UnsafeFfi,
        unsafe_ffi_batch(
            provenance,
            &extended.unsafe_ffi,
            bindings,
            extended.statuses[&RustMirDerivedRelation::UnsafeFfi].as_str(),
        )?,
    );
    relations.insert(
        RustMirDerivedRelation::ControlDependenceInput,
        control_input_batch(
            provenance,
            &extended.control_inputs,
            bindings,
            extended.statuses[&RustMirDerivedRelation::ControlDependenceInput].as_str(),
        )?,
    );
    let unknown_rows = unknowns.into_iter().collect::<Vec<_>>();
    relations.insert(
        RustMirDerivedRelation::Unknown,
        unknown_batch(provenance, &unknown_rows, bindings)?,
    );

    let output_rows = relations
        .iter()
        .map(|(relation, batch)| {
            (
                *relation,
                u64::try_from(batch.num_rows()).unwrap_or(u64::MAX),
            )
        })
        .collect();
    let mut relation_completeness = BTreeMap::from([
        (RustMirDerivedRelation::CfgEdge, cfg_status),
        (RustMirDerivedRelation::DefUse, dataflow_status),
        (RustMirDerivedRelation::ReachingDefinition, dataflow_status),
        (RustMirDerivedRelation::Liveness, dataflow_status),
        (
            RustMirDerivedRelation::Unknown,
            RustMirAnalysisCompleteness::Unknown,
        ),
    ]);
    relation_completeness.extend(extended.statuses);
    let mut unsupported_families = [
        "must-alias-no-alias-closure",
        "semantic-resource-kind",
        "exact-async-suspension-map",
        "lexical-unsafe-scope",
    ]
    .into_iter()
    .map(Arc::from)
    .collect::<BTreeSet<_>>();
    if !exact_private_complete {
        unsupported_families.insert(Arc::from("exact-loans-and-regions"));
    }

    Ok(RustMirAnalysisOutput {
        relations,
        observation: RustMirAnalysisObservation {
            relations: bindings.relations.clone(),
            algorithm_release: Arc::from(RUST_MIR_DERIVED_ANALYSIS_RELEASE),
            precision_release: Arc::from(RUST_MIR_DERIVED_PRECISION_RELEASE),
            authority_class: bindings.authority_class,
            cfg_complete: cfg_status == RustMirAnalysisCompleteness::Complete,
            dataflow_complete: dataflow_status == RustMirAnalysisCompleteness::Complete,
            relation_completeness,
            unsupported_families,
            output_rows,
            private_enrichment_completeness: private_completeness,
        },
        private_enrichment,
    })
}

fn validate_provenance(provenance: &RustMirAnalysisProvenance) -> Result<(), RustMirAnalysisError> {
    for (name, value) in [
        ("model_epoch_id", provenance.model_epoch_id),
        ("source_snapshot_pin", provenance.source_snapshot_pin),
        ("analysis_context_pin", provenance.analysis_context_pin),
        (
            "toolchain_identity_digest",
            provenance.toolchain_identity_digest,
        ),
        (
            "raw_schema_bundle_digest",
            provenance.raw_schema_bundle_digest,
        ),
    ] {
        if value == [0; 32] {
            return Err(RustMirAnalysisError::ProvenanceMismatch(format!(
                "{name} is the zero identity"
            )));
        }
    }
    for (name, value) in [
        ("provider_run_id", provenance.provider_run_id.as_ref()),
        (
            "compilation_unit_id",
            provenance.compilation_unit_id.as_ref(),
        ),
        ("owner_id", provenance.owner_id.as_ref()),
        ("source_file_id", provenance.source_file_id.as_ref()),
        ("rustc_commit", provenance.rustc_commit.as_ref()),
    ] {
        if value.is_empty() || value.len() > 1_024 || value.chars().any(char::is_control) {
            return Err(RustMirAnalysisError::ProvenanceMismatch(format!(
                "{name} is not a bounded identifier"
            )));
        }
    }
    if provenance.source_content_digest == [0; 32] {
        return Err(RustMirAnalysisError::ProvenanceMismatch(
            "source_content_digest is the zero identity".to_owned(),
        ));
    }
    if provenance.rustc_release.as_ref() != RUSTC_PUBLIC_RELEASE
        || provenance.rustc_toolchain.as_ref() != RUSTC_TOOLCHAIN
    {
        return Err(RustMirAnalysisError::ProvenanceMismatch(
            "compiler release/toolchain differs from the raw relation contract".to_owned(),
        ));
    }
    if provenance.raw_schema_bundle_digest != expected_rust_mir_input_schema_bundle_digest() {
        return Err(RustMirAnalysisError::ProvenanceMismatch(
            "raw relation schema bundle differs from this algorithm release".to_owned(),
        ));
    }
    Ok(())
}

fn validate_completeness(
    completeness: &RustMirInputCompleteness,
) -> Result<(), RustMirAnalysisError> {
    for (family, state) in [
        ("blocks", &completeness.blocks),
        ("locals", &completeness.locals),
        ("places", &completeness.places),
        ("operands", &completeness.operands),
        ("rvalues", &completeness.rvalues),
        ("statements", &completeness.statements),
        ("terminators", &completeness.terminators),
        ("cfg-edges", &completeness.cfg_edges),
        ("calls", &completeness.calls),
        ("instances", &completeness.instances),
        ("accesses", &completeness.accesses),
    ] {
        if let Some(reason) = state.reason()
            && (reason.is_empty() || reason.len() > 2_048 || reason.chars().any(char::is_control))
        {
            return Err(RustMirAnalysisError::InvalidStructure(format!(
                "{family} completeness reason is not bounded text"
            )));
        }
    }
    Ok(())
}

fn derived_completeness<const N: usize>(
    states: [&RustMirRelationCompleteness; N],
    stable_identity_available: bool,
) -> RustMirAnalysisCompleteness {
    if !stable_identity_available
        || states
            .iter()
            .any(|state| matches!(state, RustMirRelationCompleteness::Unavailable { .. }))
    {
        RustMirAnalysisCompleteness::Unknown
    } else if states
        .iter()
        .any(|state| matches!(state, RustMirRelationCompleteness::Partial { .. }))
    {
        RustMirAnalysisCompleteness::Partial
    } else {
        RustMirAnalysisCompleteness::Complete
    }
}

fn materialize_private_enrichment(
    provenance: &RustMirAnalysisProvenance,
    input: Option<&RustMirPrivateEnrichmentInput>,
    binding: Option<&RustMirPrivateEnrichmentBinding>,
) -> Result<Option<RustMirPrivateEnrichmentOutput>, RustMirAnalysisError> {
    let Some(binding) = binding else {
        if input.is_some() {
            return Err(RustMirAnalysisError::InvalidBinding(
                "exact private enrichment input requires a model-bound private relation".to_owned(),
            ));
        }
        return Ok(None);
    };

    let (completeness, rows, provider_run_id, provider_release) = match input {
        None => (
            RustMirAnalysisCompleteness::Unknown,
            &[][..],
            "unavailable",
            "unavailable",
        ),
        Some(input) => {
            for (name, value) in [
                ("provider_run_id", input.provider_run_id.as_ref()),
                ("provider_release", input.provider_release.as_ref()),
            ] {
                if value.is_empty() || value.len() > 2_048 || value.chars().any(char::is_control) {
                    return Err(RustMirAnalysisError::InvalidStructure(format!(
                        "private enrichment {name} is not bounded text"
                    )));
                }
            }
            if let Some(reason) = input.completeness.reason()
                && (reason.is_empty()
                    || reason.len() > 2_048
                    || reason.chars().any(char::is_control))
            {
                return Err(RustMirAnalysisError::InvalidStructure(
                    "private enrichment completeness reason is not bounded text".to_owned(),
                ));
            }
            if input.source_generation != provenance.source_generation
                || Some(input.stable_owner_key) != provenance.stable_owner_key
                || input.toolchain_identity_digest != provenance.toolchain_identity_digest
            {
                return Err(RustMirAnalysisError::ProvenanceMismatch(
                    "private enrichment owner/source/toolchain pins differ from the public MIR analysis pin"
                        .to_owned(),
                ));
            }
            bound("private-borrowck-loans", input.rows.len(), MAX_ACCESSES)?;
            validate_private_borrowck_rows(&input.rows)?;
            let completeness = match &input.completeness {
                RustMirRelationCompleteness::Complete => RustMirAnalysisCompleteness::Complete,
                RustMirRelationCompleteness::Partial { .. } => RustMirAnalysisCompleteness::Partial,
                RustMirRelationCompleteness::Unavailable { .. } => {
                    RustMirAnalysisCompleteness::Unknown
                }
            };
            let rows = if completeness == RustMirAnalysisCompleteness::Unknown {
                &[][..]
            } else {
                input.rows.as_slice()
            };
            (
                completeness,
                rows,
                input.provider_run_id.as_ref(),
                input.provider_release.as_ref(),
            )
        }
    };
    let batch = private_enrichment_batch(
        provenance,
        rows,
        binding,
        provider_run_id,
        provider_release,
        completeness.as_str(),
    )?;
    Ok(Some(RustMirPrivateEnrichmentOutput {
        authority: binding.authority,
        relation_id: binding.relation_id.clone(),
        completeness,
        batch,
    }))
}

fn validate_private_borrowck_rows(
    rows: &[RustMirExactBorrowckRow],
) -> Result<(), RustMirAnalysisError> {
    let mut loans = BTreeSet::new();
    for row in rows {
        if !loans.insert(row.loan_id) {
            return Err(RustMirAnalysisError::InvalidStructure(format!(
                "duplicate exact private loan {}",
                hex_prefix(&row.loan_id)
            )));
        }
        if row.loan_id == [0; 32] || row.place_id == [0; 32] || row.region_id == [0; 32] {
            return Err(RustMirAnalysisError::InvalidStructure(
                "exact private loan/place/region identities must be non-zero".to_owned(),
            ));
        }
        for (name, value) in [
            ("loan_kind", row.loan_kind.as_ref()),
            ("issued_slot_kind", row.issued_slot_kind.as_ref()),
        ] {
            if value.is_empty() || value.len() > 2_048 || value.chars().any(char::is_control) {
                return Err(RustMirAnalysisError::InvalidStructure(format!(
                    "exact private {name} is not bounded text"
                )));
            }
        }
        let killed_fields = [
            row.killed_block.is_some(),
            row.killed_slot_kind.is_some(),
            row.killed_slot_index.is_some(),
        ];
        if killed_fields.iter().any(|present| *present)
            && !killed_fields.iter().all(|present| *present)
        {
            return Err(RustMirAnalysisError::InvalidStructure(
                "exact private loan kill coordinate must be wholly present or absent".to_owned(),
            ));
        }
        if let Some(kind) = &row.killed_slot_kind
            && (kind.is_empty() || kind.len() > 2_048 || kind.chars().any(char::is_control))
        {
            return Err(RustMirAnalysisError::InvalidStructure(
                "exact private killed_slot_kind is not bounded text".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_relation_batches(
    relation: RustcRelation,
    batches: &[RecordBatch],
    provenance: &RustMirAnalysisProvenance,
) -> Result<(), RustMirAnalysisError> {
    let expected = relation.schema();
    for batch in batches {
        if batch.schema().as_ref() != expected.as_ref() {
            return Err(RustMirAnalysisError::InvalidRawSchema {
                relation: relation.relation_id(),
            });
        }
        validate_common_raw_pins(batch, provenance)?;
    }
    Ok(())
}

fn validate_common_raw_pins(
    batch: &RecordBatch,
    provenance: &RustMirAnalysisProvenance,
) -> Result<(), RustMirAnalysisError> {
    let provider_run_ids = strings(batch, "provider_run_id")?;
    let compilation_unit_ids = strings(batch, "compilation_unit_id")?;
    let owner_ids = strings(batch, "owner_id")?;
    let source_generations = u64s(batch, "source_generation")?;
    let source_file_ids = strings(batch, "source_file_id")?;
    let source_digests = fixed(batch, "source_content_digest", 32)?;
    let stable_crate_ids = u64s(batch, "stable_crate_id")?;
    let def_path_hashes = fixed(batch, "def_path_hash", 16)?;
    for row in 0..batch.num_rows() {
        if provider_run_ids.value(row) != provenance.provider_run_id.as_ref()
            || compilation_unit_ids.value(row) != provenance.compilation_unit_id.as_ref()
            || owner_ids.value(row) != provenance.owner_id.as_ref()
            || source_generations.value(row) != provenance.source_generation
            || source_file_ids.value(row) != provenance.source_file_id.as_ref()
            || source_digests.value(row) != provenance.source_content_digest
        {
            return Err(RustMirAnalysisError::ProvenanceMismatch(
                "raw row common pins differ from the analysis provenance".to_owned(),
            ));
        }
        match provenance.stable_owner_key {
            Some(key)
                if !stable_crate_ids.is_null(row)
                    && stable_crate_ids.value(row) == key.stable_crate_id
                    && !def_path_hashes.is_null(row)
                    && def_path_hashes.value(row) == key.def_path_hash => {}
            None if stable_crate_ids.is_null(row) && def_path_hashes.is_null(row) => {}
            _ => {
                return Err(RustMirAnalysisError::ProvenanceMismatch(
                    "raw row stable owner key differs from the analysis provenance".to_owned(),
                ));
            }
        }
    }
    Ok(())
}

fn parse_blocks(
    batches: &[RecordBatch],
    require_closed_body: bool,
) -> Result<Vec<BlockRow>, RustMirAnalysisError> {
    let mut blocks = Vec::with_capacity(row_count(batches));
    let mut seen = BTreeSet::new();
    for batch in batches {
        let indices = u64s(batch, "block_index")?;
        let statements = u64s(batch, "statement_count")?;
        let terminators = strings(batch, "terminator_kind")?;
        let entries = bools(batch, "is_entry")?;
        for row in 0..batch.num_rows() {
            let block_index = indices.value(row);
            if !seen.insert(block_index) {
                return Err(RustMirAnalysisError::InvalidStructure(format!(
                    "duplicate MIR block {block_index}"
                )));
            }
            blocks.push(BlockRow {
                block_index,
                statement_count: statements.value(row),
                terminator_kind: Arc::from(terminators.value(row)),
                is_entry: entries.value(row),
            });
        }
    }
    blocks.sort();
    let entry_count = blocks.iter().filter(|block| block.is_entry).count();
    if require_closed_body && (blocks.is_empty() || entry_count != 1) {
        return Err(RustMirAnalysisError::InvalidStructure(format!(
            "MIR body has {entry_count} entry blocks"
        )));
    }
    Ok(blocks)
}

fn parse_and_validate_edges(
    batches: &[RecordBatch],
    blocks: &[BlockRow],
    provenance: &RustMirAnalysisProvenance,
) -> Result<Vec<EdgeRow>, RustMirAnalysisError> {
    let block_ids = blocks
        .iter()
        .map(|block| block.block_index)
        .collect::<BTreeSet<_>>();
    let stable_key = provenance
        .stable_owner_key
        .expect("call site proved stable identity availability");
    let mut edges = Vec::with_capacity(row_count(batches));
    let mut seen = BTreeSet::new();
    for batch in batches {
        let sources = u64s(batch, "source_block")?;
        let targets = u64s(batch, "target_block")?;
        let kinds = strings(batch, "edge_kind")?;
        let branches = strings(batch, "branch_value_u128")?;
        let unwinds = strings(batch, "unwind_action")?;
        for row in 0..batch.num_rows() {
            let source = sources.value(row);
            let target = targets.value(row);
            if !block_ids.contains(&source) || !block_ids.contains(&target) {
                return Err(RustMirAnalysisError::InvalidStructure(format!(
                    "CFG edge {source}->{target} references an absent block"
                )));
            }
            let branch = optional_string(branches, row);
            let unwind = optional_string(unwinds, row);
            let key = (
                source,
                target,
                kinds.value(row).to_owned(),
                branch.clone(),
                unwind.clone(),
            );
            if !seen.insert(key) {
                return Err(RustMirAnalysisError::InvalidStructure(format!(
                    "duplicate CFG edge {source}->{target}"
                )));
            }
            let source_bytes = source.to_be_bytes();
            let target_bytes = target.to_be_bytes();
            let crate_bytes = stable_key.stable_crate_id.to_be_bytes();
            let edge_id = hash_parts(
                b"codefabric.rust-mir-derived-cfg-edge.v1\0",
                [
                    crate_bytes.as_slice(),
                    stable_key.def_path_hash.as_slice(),
                    source_bytes.as_slice(),
                    target_bytes.as_slice(),
                    kinds.value(row).as_bytes(),
                    branch.as_deref().unwrap_or("").as_bytes(),
                    unwind.as_deref().unwrap_or("").as_bytes(),
                ],
            );
            edges.push(EdgeRow {
                edge_id,
                source_block: source,
                target_block: target,
                edge_kind: Arc::from(kinds.value(row)),
                branch_value: branch,
                unwind_action: unwind,
            });
        }
    }
    edges.sort();
    Ok(edges)
}

#[derive(Clone, Debug)]
struct PlaceGroup {
    block_index: u64,
    slot_kind: Arc<str>,
    slot_index: u64,
    occurrence_role: Arc<str>,
    occurrence_ordinal: u64,
    base_local: u64,
    projections: Vec<ProjectionRow>,
}

fn parse_places(
    batches: &[RecordBatch],
    provenance: &RustMirAnalysisProvenance,
) -> Result<BTreeMap<[u8; 32], PlaceObservation>, RustMirAnalysisError> {
    let mut groups = BTreeMap::<[u8; 32], PlaceGroup>::new();
    for batch in batches {
        let place_ids = fixed(batch, "place_id", 32)?;
        let block_indices = u64s(batch, "block_index")?;
        let slot_kinds = strings(batch, "slot_kind")?;
        let slot_indices = u64s(batch, "slot_index")?;
        let occurrence_roles = strings(batch, "occurrence_role")?;
        let occurrence_ordinals = u64s(batch, "occurrence_ordinal")?;
        let base_locals = u64s(batch, "base_local")?;
        let projection_ordinals = u64s(batch, "projection_ordinal")?;
        let projection_kinds = strings(batch, "projection_kind")?;
        let local_or_fields = u64s(batch, "projection_local_or_field")?;
        let offsets = u64s(batch, "offset")?;
        let min_lengths = u64s(batch, "min_length")?;
        let slice_tos = u64s(batch, "slice_to")?;
        let from_ends = bools(batch, "from_end")?;
        let type_keys = fixed(batch, "projection_type_key", 32)?;
        for row in 0..batch.num_rows() {
            let place_id = array32(place_ids.value(row));
            let common = (
                block_indices.value(row),
                slot_kinds.value(row),
                slot_indices.value(row),
                occurrence_roles.value(row),
                occurrence_ordinals.value(row),
                base_locals.value(row),
            );
            let group = groups.entry(place_id).or_insert_with(|| PlaceGroup {
                block_index: common.0,
                slot_kind: Arc::from(common.1),
                slot_index: common.2,
                occurrence_role: Arc::from(common.3),
                occurrence_ordinal: common.4,
                base_local: common.5,
                projections: Vec::new(),
            });
            if group.block_index != common.0
                || group.slot_kind.as_ref() != common.1
                || group.slot_index != common.2
                || group.occurrence_role.as_ref() != common.3
                || group.occurrence_ordinal != common.4
                || group.base_local != common.5
            {
                return Err(RustMirAnalysisError::InvalidStructure(format!(
                    "place occurrence {} has inconsistent native coordinates",
                    hex_prefix(&place_id)
                )));
            }
            group.projections.push(ProjectionRow {
                ordinal: optional_u64(projection_ordinals, row),
                kind: Arc::from(projection_kinds.value(row)),
                local_or_field: optional_u64(local_or_fields, row),
                offset: optional_u64(offsets, row),
                min_length: optional_u64(min_lengths, row),
                slice_to: optional_u64(slice_tos, row),
                from_end: optional_bool(from_ends, row),
                type_key: optional_fixed32(type_keys, row),
            });
        }
    }

    let stable_key = provenance
        .stable_owner_key
        .expect("call site proved stable identity availability");
    groups
        .into_iter()
        .map(|(place_id, mut group)| {
            group.projections.sort();
            validate_projection_path(place_id, &group.projections)?;
            let path = projection_path(&group.projections);
            let projection_identity = projection_identity_digest(&group.projections);
            let crate_bytes = stable_key.stable_crate_id.to_be_bytes();
            let local_bytes = group.base_local.to_be_bytes();
            let location_id = hash_parts(
                b"codefabric.rust-mir-memory-location.v1\0",
                [
                    crate_bytes.as_slice(),
                    stable_key.def_path_hash.as_slice(),
                    local_bytes.as_slice(),
                    projection_identity.as_slice(),
                ],
            );
            Ok((
                place_id,
                PlaceObservation {
                    location: MemoryLocation {
                        id: location_id,
                        base_local: group.base_local,
                        projection_path: Arc::from(path),
                    },
                    block_index: group.block_index,
                    slot_kind: group.slot_kind,
                    slot_index: group.slot_index,
                    occurrence_ordinal: group.occurrence_ordinal,
                },
            ))
        })
        .collect()
}

fn validate_projection_path(
    place_id: [u8; 32],
    projections: &[ProjectionRow],
) -> Result<(), RustMirAnalysisError> {
    if projections.is_empty() {
        return Err(RustMirAnalysisError::InvalidStructure(format!(
            "place occurrence {} has no projection observation",
            hex_prefix(&place_id)
        )));
    }
    if projections.len() == 1
        && projections[0].ordinal.is_none()
        && projections[0].kind.as_ref() == "BaseLocal"
    {
        return Ok(());
    }
    let mut expected = 0_u64;
    for projection in projections {
        if projection.ordinal != Some(expected) || projection.kind.as_ref() == "BaseLocal" {
            return Err(RustMirAnalysisError::InvalidStructure(format!(
                "place occurrence {} has a non-contiguous projection path",
                hex_prefix(&place_id)
            )));
        }
        expected = expected.saturating_add(1);
    }
    Ok(())
}

fn projection_path(projections: &[ProjectionRow]) -> String {
    projections
        .iter()
        .map(|projection| {
            let type_key = projection
                .type_key
                .map_or_else(|| "-".to_owned(), |value| hex_full(&value));
            format!(
                "{}:{}:{}:{}:{}:{}:{}:{}",
                projection
                    .ordinal
                    .map_or_else(|| "base".to_owned(), |value| value.to_string()),
                projection.kind,
                optional_number(projection.local_or_field),
                optional_number(projection.offset),
                optional_number(projection.min_length),
                optional_number(projection.slice_to),
                projection
                    .from_end
                    .map_or("-", |value| if value { "1" } else { "0" }),
                type_key,
            )
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn projection_identity_digest(projections: &[ProjectionRow]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.rust-mir-projection-path.v1\0");
    hasher.update(&(projections.len() as u64).to_be_bytes());
    for projection in projections {
        hash_optional_u64(&mut hasher, projection.ordinal);
        hash_framed(&mut hasher, projection.kind.as_bytes());
        hash_optional_u64(&mut hasher, projection.local_or_field);
        hash_optional_u64(&mut hasher, projection.offset);
        hash_optional_u64(&mut hasher, projection.min_length);
        hash_optional_u64(&mut hasher, projection.slice_to);
        match projection.from_end {
            Some(value) => {
                hasher.update(&[1, u8::from(value)]);
            }
            None => {
                hasher.update(&[0]);
            }
        };
        match projection.type_key {
            Some(value) => {
                hasher.update(&[1]);
                hasher.update(&value);
            }
            None => {
                hasher.update(&[0]);
            }
        };
    }
    *hasher.finalize().as_bytes()
}

fn hash_optional_u64(hasher: &mut blake3::Hasher, value: Option<u64>) {
    match value {
        Some(value) => {
            hasher.update(&[1]);
            hasher.update(&value.to_be_bytes());
        }
        None => {
            hasher.update(&[0]);
        }
    }
}

fn hash_framed(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn parse_accesses(
    batches: &[RecordBatch],
    places: &BTreeMap<[u8; 32], PlaceObservation>,
    provenance: &RustMirAnalysisProvenance,
) -> Result<(Vec<EventRow>, BTreeSet<UnknownRow>), RustMirAnalysisError> {
    let mut events = Vec::with_capacity(row_count(batches));
    let mut unknowns = BTreeSet::new();
    let mut coordinates = BTreeSet::new();
    let stable_key = provenance
        .stable_owner_key
        .expect("call site proved stable identity availability");
    for batch in batches {
        let block_indices = u64s(batch, "block_index")?;
        let slot_kinds = strings(batch, "slot_kind")?;
        let slot_indices = u64s(batch, "slot_index")?;
        let ordinals = u64s(batch, "access_ordinal")?;
        let place_ids = fixed(batch, "place_id", 32)?;
        let access_kinds = strings(batch, "access_kind")?;
        let evidence = strings(batch, "structured_evidence")?;
        let runtime_effects = bools(batch, "runtime_effect")?;
        for row in 0..batch.num_rows() {
            let place_id = array32(place_ids.value(row));
            let place = places.get(&place_id).ok_or_else(|| {
                RustMirAnalysisError::InvalidStructure(format!(
                    "access references absent place occurrence {}",
                    hex_prefix(&place_id)
                ))
            })?;
            let block_index = block_indices.value(row);
            let slot_kind = slot_kinds.value(row);
            let slot_index = slot_indices.value(row);
            let access_ordinal = ordinals.value(row);
            if place.block_index != block_index
                || place.slot_kind.as_ref() != slot_kind
                || place.slot_index != slot_index
                || place.occurrence_ordinal != access_ordinal
            {
                return Err(RustMirAnalysisError::InvalidStructure(format!(
                    "access and place occurrence {} disagree on native MIR coordinates",
                    hex_prefix(&place_id)
                )));
            }
            if !coordinates.insert((
                block_index,
                slot_kind.to_owned(),
                slot_index,
                access_ordinal,
                place_id,
            )) {
                return Err(RustMirAnalysisError::InvalidStructure(format!(
                    "duplicate access occurrence {}",
                    hex_prefix(&place_id)
                )));
            }
            let access_kind = access_kinds.value(row);
            let Some((is_use, is_definition, is_kill)) = access_semantics(access_kind) else {
                unknowns.insert(UnknownRow {
                    family: Arc::from("dataflow"),
                    reason_code: Arc::from("UNCLASSIFIED_ACCESS_KIND"),
                    detail: Arc::from(format!(
                        "{access_kind} at block {block_index} {slot_kind} {slot_index}:{access_ordinal} has no transfer rule in {}",
                        RUST_MIR_DERIVED_ANALYSIS_RELEASE
                    )),
                    bounded: true,
                    input_relation: Some(Arc::from(RustcRelation::Access.relation_id())),
                });
                continue;
            };
            let block_bytes = block_index.to_be_bytes();
            let slot_bytes = slot_index.to_be_bytes();
            let ordinal_bytes = access_ordinal.to_be_bytes();
            let crate_bytes = stable_key.stable_crate_id.to_be_bytes();
            let event_id = hash_parts(
                b"codefabric.rust-mir-access-event.v1\0",
                [
                    crate_bytes.as_slice(),
                    stable_key.def_path_hash.as_slice(),
                    block_bytes.as_slice(),
                    slot_kind.as_bytes(),
                    slot_bytes.as_slice(),
                    ordinal_bytes.as_slice(),
                    place_id.as_slice(),
                    access_kind.as_bytes(),
                ],
            );
            events.push(EventRow {
                event_id,
                place_id,
                location: place.location.clone(),
                block_index,
                slot_kind: Arc::from(slot_kind),
                slot_index,
                access_ordinal,
                access_kind: Arc::from(access_kind),
                structured_evidence: Arc::from(evidence.value(row)),
                runtime_effect: runtime_effects.value(row),
                is_use,
                is_definition,
                is_kill,
            });
        }
    }
    events.sort_by(|left, right| {
        left.block_index
            .cmp(&right.block_index)
            .then_with(|| left.coordinate_key().cmp(&right.coordinate_key()))
    });
    Ok((events, unknowns))
}

fn access_semantics(kind: &str) -> Option<(bool, bool, bool)> {
    match kind.as_bytes() {
        b"Write" | b"DiscriminantWrite" => Some((false, true, true)),
        b"Move" | b"Drop" => Some((true, false, true)),
        b"StorageLive" | b"StorageDead" => Some((false, false, true)),
        b"Copy"
        | b"CopyForDeref"
        | b"DiscriminantRead"
        | b"LengthRead"
        | b"BorrowShared"
        | b"BorrowFake"
        | b"BorrowMut"
        | b"ReborrowMut"
        | b"ReborrowShared"
        | b"AddressOfMut"
        | b"AddressOfConst"
        | b"AddressOfMetadata"
        | b"FakeReadMatchGuard"
        | b"FakeReadMatchedPlace" => Some((true, false, false)),
        _ => None,
    }
}

fn parse_locals(
    batches: &[RecordBatch],
) -> Result<BTreeMap<u64, LocalObservation>, RustMirAnalysisError> {
    let mut locals = BTreeMap::new();
    for batch in batches {
        let indices = u64s(batch, "local_index")?;
        let roles = strings(batch, "local_role")?;
        let types = fixed(batch, "type_key", 32)?;
        let mutabilities = strings(batch, "mutability")?;
        for row in 0..batch.num_rows() {
            let local = LocalObservation {
                local_index: indices.value(row),
                local_role: Arc::from(roles.value(row)),
                type_key: array32(types.value(row)),
                mutability: Arc::from(mutabilities.value(row)),
            };
            if locals.insert(local.local_index, local).is_some() {
                return Err(RustMirAnalysisError::InvalidStructure(
                    "duplicate MIR local index".to_owned(),
                ));
            }
        }
    }
    Ok(locals)
}

fn parse_operands(
    batches: &[RecordBatch],
) -> Result<Vec<OperandObservation>, RustMirAnalysisError> {
    let mut operands = Vec::with_capacity(row_count(batches));
    let mut seen = BTreeSet::new();
    for batch in batches {
        let ids = fixed(batch, "operand_id", 32)?;
        let blocks = u64s(batch, "block_index")?;
        let slot_kinds = strings(batch, "slot_kind")?;
        let slot_indices = u64s(batch, "slot_index")?;
        let parent_roles = strings(batch, "parent_role")?;
        let ordinals = u64s(batch, "operand_ordinal")?;
        let kinds = strings(batch, "operand_kind")?;
        for row in 0..batch.num_rows() {
            let operand_id = array32(ids.value(row));
            if !seen.insert(operand_id) {
                return Err(RustMirAnalysisError::InvalidStructure(format!(
                    "duplicate MIR operand {}",
                    hex_prefix(&operand_id)
                )));
            }
            operands.push(OperandObservation {
                operand_id,
                block_index: blocks.value(row),
                slot_kind: Arc::from(slot_kinds.value(row)),
                slot_index: slot_indices.value(row),
                parent_role: Arc::from(parent_roles.value(row)),
                operand_ordinal: ordinals.value(row),
                operand_kind: Arc::from(kinds.value(row)),
            });
        }
    }
    operands.sort_by(|left, right| {
        (
            left.block_index,
            slot_kind_rank(&left.slot_kind),
            left.slot_index,
            left.parent_role.as_ref(),
            left.operand_ordinal,
            left.operand_id,
        )
            .cmp(&(
                right.block_index,
                slot_kind_rank(&right.slot_kind),
                right.slot_index,
                right.parent_role.as_ref(),
                right.operand_ordinal,
                right.operand_id,
            ))
    });
    Ok(operands)
}

fn parse_rvalues(batches: &[RecordBatch]) -> Result<Vec<RvalueObservation>, RustMirAnalysisError> {
    let mut rvalues = Vec::with_capacity(row_count(batches));
    let mut seen = BTreeSet::new();
    for batch in batches {
        let blocks = u64s(batch, "block_index")?;
        let statements = u64s(batch, "statement_index")?;
        let kinds = strings(batch, "rvalue_kind")?;
        let result_types = fixed(batch, "result_type_key", 32)?;
        let casts = strings(batch, "cast_kind")?;
        let aggregates = strings(batch, "aggregate_kind")?;
        let source_places = fixed(batch, "source_place_id", 32)?;
        let regions = strings(batch, "region_kind")?;
        let mutabilities = strings(batch, "mutability")?;
        for row in 0..batch.num_rows() {
            let key = (blocks.value(row), statements.value(row));
            if !seen.insert(key) {
                return Err(RustMirAnalysisError::InvalidStructure(format!(
                    "duplicate MIR rvalue at {}:{}",
                    key.0, key.1
                )));
            }
            rvalues.push(RvalueObservation {
                block_index: key.0,
                statement_index: key.1,
                rvalue_kind: Arc::from(kinds.value(row)),
                result_type_key: optional_fixed32(result_types, row),
                cast_kind: optional_string(casts, row),
                aggregate_kind: optional_string(aggregates, row),
                source_place_id: optional_fixed32(source_places, row),
                region_kind: optional_string(regions, row),
                mutability: optional_string(mutabilities, row),
            });
        }
    }
    rvalues.sort_by_key(|row| (row.block_index, row.statement_index));
    Ok(rvalues)
}

fn parse_statements(
    batches: &[RecordBatch],
) -> Result<BTreeMap<(u64, u64), StatementObservation>, RustMirAnalysisError> {
    let mut statements = BTreeMap::new();
    for batch in batches {
        let blocks = u64s(batch, "block_index")?;
        let indices = u64s(batch, "statement_index")?;
        let kinds = strings(batch, "raw_statement_kind")?;
        let effects = strings(batch, "normalized_effect")?;
        let scopes = u64s(batch, "source_scope")?;
        for row in 0..batch.num_rows() {
            let statement = StatementObservation {
                block_index: blocks.value(row),
                statement_index: indices.value(row),
                raw_kind: Arc::from(kinds.value(row)),
                normalized_effect: Arc::from(effects.value(row)),
                source_scope: scopes.value(row),
            };
            if statements
                .insert(
                    (statement.block_index, statement.statement_index),
                    statement,
                )
                .is_some()
            {
                return Err(RustMirAnalysisError::InvalidStructure(
                    "duplicate MIR statement coordinate".to_owned(),
                ));
            }
        }
    }
    Ok(statements)
}

fn parse_terminators(
    batches: &[RecordBatch],
) -> Result<BTreeMap<u64, TerminatorObservation>, RustMirAnalysisError> {
    let mut terminators = BTreeMap::new();
    for batch in batches {
        let blocks = u64s(batch, "block_index")?;
        let kinds = strings(batch, "raw_terminator_kind")?;
        let scopes = u64s(batch, "source_scope")?;
        let normal_counts = u64s(batch, "normal_target_count")?;
        let unwinds = strings(batch, "unwind_action")?;
        let destinations = fixed(batch, "destination_place_id", 32)?;
        for row in 0..batch.num_rows() {
            let terminator = TerminatorObservation {
                block_index: blocks.value(row),
                raw_kind: Arc::from(kinds.value(row)),
                source_scope: scopes.value(row),
                normal_target_count: normal_counts.value(row),
                unwind_action: optional_string(unwinds, row),
                destination_place_id: optional_fixed32(destinations, row),
            };
            if terminators
                .insert(terminator.block_index, terminator)
                .is_some()
            {
                return Err(RustMirAnalysisError::InvalidStructure(
                    "duplicate MIR terminator block".to_owned(),
                ));
            }
        }
    }
    Ok(terminators)
}

fn parse_calls(batches: &[RecordBatch]) -> Result<Vec<CallObservation>, RustMirAnalysisError> {
    let mut calls = Vec::with_capacity(row_count(batches));
    let mut seen = BTreeSet::new();
    for batch in batches {
        let blocks = u64s(batch, "block_index")?;
        let destinations = fixed(batch, "destination_place_id", 32)?;
        let targets = strings(batch, "declared_target")?;
        let instances = fixed(batch, "resolved_instance_key", 32)?;
        let dispatch = strings(batch, "dispatch_kind")?;
        let confidence = strings(batch, "resolution_confidence")?;
        for row in 0..batch.num_rows() {
            let block = blocks.value(row);
            if !seen.insert(block) {
                return Err(RustMirAnalysisError::InvalidStructure(format!(
                    "duplicate MIR call observation in block {block}"
                )));
            }
            calls.push(CallObservation {
                block_index: block,
                destination_place_id: array32(destinations.value(row)),
                declared_target: optional_string(targets, row),
                resolved_instance_key: optional_fixed32(instances, row),
                dispatch_kind: Arc::from(dispatch.value(row)),
                resolution_confidence: Arc::from(confidence.value(row)),
            });
        }
    }
    calls.sort_by_key(|row| row.block_index);
    Ok(calls)
}

fn parse_instances(
    batches: &[RecordBatch],
) -> Result<BTreeMap<[u8; 32], InstanceObservation>, RustMirAnalysisError> {
    let mut instances = BTreeMap::new();
    for batch in batches {
        let keys = fixed(batch, "instance_key", 32)?;
        let paths = strings(batch, "definition_path")?;
        let foreign = bools(batch, "is_foreign_item")?;
        for row in 0..batch.num_rows() {
            let instance = InstanceObservation {
                instance_key: array32(keys.value(row)),
                definition_path: Arc::from(paths.value(row)),
                is_foreign_item: foreign.value(row),
            };
            if instances.insert(instance.instance_key, instance).is_some() {
                return Err(RustMirAnalysisError::InvalidStructure(
                    "duplicate resolved MIR instance".to_owned(),
                ));
            }
        }
    }
    Ok(instances)
}

#[derive(Default)]
struct DataflowResult {
    def_use: Vec<DefUseRow>,
    reaching: Vec<ReachingRow>,
    liveness: Vec<LivenessRow>,
    unknowns: BTreeSet<UnknownRow>,
}

struct ExtendedAnalysisResult {
    ownership: Vec<OwnershipRow>,
    aliases: Vec<AliasRow>,
    resources: Vec<ResourceRow>,
    async_lowering: Vec<AsyncRow>,
    unsafe_ffi: Vec<UnsafeFfiRow>,
    control_inputs: Vec<ControlInputRow>,
    statuses: BTreeMap<RustMirDerivedRelation, RustMirAnalysisCompleteness>,
    unknowns: BTreeSet<UnknownRow>,
}

fn derive_extended_analyses(
    provenance: &RustMirAnalysisProvenance,
    raw: &RustMirRawRelations,
    blocks: &[BlockRow],
    cfg_rows: &[EdgeRow],
) -> Result<ExtendedAnalysisResult, RustMirAnalysisError> {
    let stable = provenance.stable_owner_key.is_some();
    let mut statuses = BTreeMap::from([
        (
            RustMirDerivedRelation::OwnershipState,
            derived_completeness(
                [
                    &raw.completeness.locals,
                    &raw.completeness.places,
                    &raw.completeness.accesses,
                ],
                stable,
            ),
        ),
        (
            RustMirDerivedRelation::AliasPointsTo,
            derived_completeness(
                [
                    &raw.completeness.places,
                    &raw.completeness.rvalues,
                    &raw.completeness.statements,
                    &raw.completeness.accesses,
                ],
                stable,
            ),
        ),
        (
            RustMirDerivedRelation::ResourceLifecycle,
            derived_completeness(
                [&raw.completeness.places, &raw.completeness.accesses],
                stable,
            ),
        ),
        (
            RustMirDerivedRelation::AsyncLowering,
            derived_completeness(
                [&raw.completeness.rvalues, &raw.completeness.statements],
                stable,
            ),
        ),
        (
            RustMirDerivedRelation::UnsafeFfi,
            derived_completeness(
                [
                    &raw.completeness.rvalues,
                    &raw.completeness.statements,
                    &raw.completeness.terminators,
                    &raw.completeness.calls,
                    &raw.completeness.instances,
                ],
                stable,
            ),
        ),
        (
            RustMirDerivedRelation::ControlDependenceInput,
            derived_completeness(
                [
                    &raw.completeness.blocks,
                    &raw.completeness.operands,
                    &raw.completeness.terminators,
                    &raw.completeness.cfg_edges,
                ],
                stable,
            ),
        ),
    ]);
    let mut unknowns = BTreeSet::new();
    let mut ownership = Vec::new();
    let mut aliases = Vec::new();
    let mut resources = Vec::new();
    let mut async_lowering = Vec::new();
    let mut unsafe_ffi = Vec::new();
    let mut control_inputs = Vec::new();

    let place_access_complete =
        stable && raw.completeness.places.is_complete() && raw.completeness.accesses.is_complete();
    let (places, events, access_unknowns) = if place_access_complete {
        let places = parse_places(&raw.places, provenance)?;
        let (events, access_unknowns) = parse_accesses(&raw.accesses, &places, provenance)?;
        (Some(places), Some(events), access_unknowns)
    } else {
        (None, None, BTreeSet::new())
    };
    if !access_unknowns.is_empty() {
        for role in [
            RustMirDerivedRelation::OwnershipState,
            RustMirDerivedRelation::AliasPointsTo,
            RustMirDerivedRelation::ResourceLifecycle,
        ] {
            if statuses[&role] == RustMirAnalysisCompleteness::Complete {
                statuses.insert(role, RustMirAnalysisCompleteness::Partial);
            }
        }
        unknowns.extend(access_unknowns);
    }

    if statuses[&RustMirDerivedRelation::OwnershipState] == RustMirAnalysisCompleteness::Complete {
        let locals = parse_locals(&raw.locals)?;
        for event in events.as_deref().unwrap_or_default() {
            let Some(observation) = ownership_observation(&event.access_kind) else {
                continue;
            };
            let local = locals.get(&event.location.base_local);
            if local.is_none() {
                statuses.insert(
                    RustMirDerivedRelation::OwnershipState,
                    RustMirAnalysisCompleteness::Partial,
                );
                unknowns.insert(UnknownRow {
                    family: Arc::from("ownership-state"),
                    reason_code: Arc::from("MIR_LOCAL_METADATA_MISSING"),
                    detail: Arc::from(format!(
                        "base local {} has an ownership event but no accepted MirLocal row",
                        event.location.base_local
                    )),
                    bounded: true,
                    input_relation: Some(Arc::from(RustcRelation::MirLocal.relation_id())),
                });
            }
            ownership.push(OwnershipRow {
                event: EventRef::from(event),
                location_id: event.location.id,
                base_local: event.location.base_local,
                projection_path: Arc::clone(&event.location.projection_path),
                local_role: local.map(|local| Arc::clone(&local.local_role)),
                local_type_key: local.map(|local| local.type_key),
                local_mutability: local.map(|local| Arc::clone(&local.mutability)),
                ownership_observation: Arc::from(observation),
            });
        }
        ownership.sort();
    }

    let rvalues = if raw.completeness.rvalues.is_complete() {
        Some(parse_rvalues(&raw.rvalues)?)
    } else {
        None
    };
    let statements = if raw.completeness.statements.is_complete() {
        Some(parse_statements(&raw.statements)?)
    } else {
        None
    };

    if statuses[&RustMirDerivedRelation::AliasPointsTo] == RustMirAnalysisCompleteness::Complete {
        let places = places
            .as_ref()
            .expect("complete alias input includes places");
        let events = events.as_deref().unwrap_or_default();
        let destinations = events
            .iter()
            .filter(|event| {
                event.is_definition
                    && event.slot_kind.as_ref() == "statement"
                    && event.structured_evidence.as_ref() == "StatementKind::Assign.destination"
            })
            .map(|event| ((event.block_index, event.slot_index), event))
            .collect::<BTreeMap<_, _>>();
        for rvalue in rvalues.as_deref().unwrap_or_default() {
            if !matches!(
                rvalue.rvalue_kind.as_ref(),
                "Ref" | "Reborrow" | "AddressOf"
            ) {
                continue;
            }
            let key = (rvalue.block_index, rvalue.statement_index);
            let statement_row = statements
                .as_ref()
                .and_then(|statements| statements.get(&key));
            let statement_is_assign =
                statement_row.is_some_and(|statement| statement.raw_kind.as_ref() == "Assign");
            let source = rvalue
                .source_place_id
                .and_then(|place_id| places.get(&place_id).map(|place| (place_id, place)));
            let destination = destinations.get(&key).copied();
            let (Some((source_place_id, source)), Some(destination)) = (source, destination) else {
                statuses.insert(
                    RustMirDerivedRelation::AliasPointsTo,
                    RustMirAnalysisCompleteness::Partial,
                );
                unknowns.insert(UnknownRow {
                    family: Arc::from("alias-points-to"),
                    reason_code: Arc::from("REFERENCE_ENDPOINT_NOT_JOINABLE"),
                    detail: Arc::from(format!(
                        "{} rvalue at {}:{} lacks a unique source place or assignment destination",
                        rvalue.rvalue_kind, rvalue.block_index, rvalue.statement_index
                    )),
                    bounded: true,
                    input_relation: Some(Arc::from(RustcRelation::MirRvalue.relation_id())),
                });
                continue;
            };
            if !statement_is_assign {
                statuses.insert(
                    RustMirDerivedRelation::AliasPointsTo,
                    RustMirAnalysisCompleteness::Partial,
                );
                continue;
            }
            let block = rvalue.block_index.to_be_bytes();
            let statement_bytes = rvalue.statement_index.to_be_bytes();
            let observation_id = hash_parts(
                b"codefabric.rust-mir-alias-observation.v1\0",
                [
                    block.as_slice(),
                    statement_bytes.as_slice(),
                    destination.place_id.as_slice(),
                    source_place_id.as_slice(),
                    rvalue.rvalue_kind.as_bytes(),
                ],
            );
            aliases.push(AliasRow {
                observation_id,
                pointer_place_id: destination.place_id,
                pointer_location_id: destination.location.id,
                pointee_place_id: source_place_id,
                pointee_location_id: source.location.id,
                block_index: rvalue.block_index,
                statement_index: rvalue.statement_index,
                rvalue_kind: Arc::clone(&rvalue.rvalue_kind),
                normalized_effect: statement_row.map_or_else(
                    || Arc::from("unknown"),
                    |row| Arc::clone(&row.normalized_effect),
                ),
                source_scope: statement_row.map_or(0, |row| row.source_scope),
                region_kind: rvalue.region_kind.clone(),
                mutability: rvalue.mutability.clone(),
            });
        }
        aliases.sort();
    }

    if statuses[&RustMirDerivedRelation::ResourceLifecycle] == RustMirAnalysisCompleteness::Complete
    {
        for event in events.as_deref().unwrap_or_default() {
            let lifecycle = match event.access_kind.as_ref() {
                "StorageLive" => "STORAGE_LIVE",
                "StorageDead" => "STORAGE_DEAD",
                "Drop" => "DROP_EXECUTED",
                _ => continue,
            };
            resources.push(ResourceRow {
                lifecycle_event_id: event.event_id,
                event: EventRef::from(event),
                location_id: event.location.id,
                base_local: event.location.base_local,
                projection_path: Arc::clone(&event.location.projection_path),
                lifecycle_event: Arc::from(lifecycle),
            });
        }
        resources.sort();
        statuses.insert(
            RustMirDerivedRelation::ResourceLifecycle,
            RustMirAnalysisCompleteness::Partial,
        );
        unknowns.insert(semantic_gap(
            "drop-resource",
            "RESOURCE_KIND_NOT_IN_PUBLIC_MIR",
            "storage and Drop lifecycle is structural; semantic resource kind/acquire/release is not exposed",
            RustcRelation::Access,
        ));
    }

    if statuses[&RustMirDerivedRelation::AsyncLowering] == RustMirAnalysisCompleteness::Complete {
        for rvalue in rvalues.as_deref().unwrap_or_default() {
            let Some(aggregate) = rvalue.aggregate_kind.as_deref() else {
                continue;
            };
            if !matches!(aggregate, "Coroutine" | "CoroutineClosure") {
                continue;
            }
            let Some(statement_row) = statements
                .as_ref()
                .and_then(|rows| rows.get(&(rvalue.block_index, rvalue.statement_index)))
            else {
                statuses.insert(
                    RustMirDerivedRelation::AsyncLowering,
                    RustMirAnalysisCompleteness::Partial,
                );
                unknowns.insert(semantic_gap(
                    "async-lowering",
                    "COROUTINE_STATEMENT_NOT_JOINABLE",
                    "a coroutine aggregate rvalue has no accepted MirStatement coordinate",
                    RustcRelation::MirStatement,
                ));
                continue;
            };
            let block = rvalue.block_index.to_be_bytes();
            let statement_bytes = rvalue.statement_index.to_be_bytes();
            async_lowering.push(AsyncRow {
                observation_id: hash_parts(
                    b"codefabric.rust-mir-async-lowering.v1\0",
                    [
                        block.as_slice(),
                        statement_bytes.as_slice(),
                        aggregate.as_bytes(),
                    ],
                ),
                block_index: rvalue.block_index,
                statement_index: rvalue.statement_index,
                source_scope: statement_row.source_scope,
                rvalue_kind: Arc::clone(&rvalue.rvalue_kind),
                aggregate_kind: Arc::from(aggregate),
                result_type_key: rvalue.result_type_key,
            });
        }
        async_lowering.sort();
        statuses.insert(
            RustMirDerivedRelation::AsyncLowering,
            RustMirAnalysisCompleteness::Partial,
        );
        unknowns.insert(semantic_gap(
            "async-lowering",
            "SUSPENSION_STATE_MAP_NOT_EXPOSED",
            "coroutine aggregates are observable, but the selected public MIR terminator surface has no exact suspension-state relation",
            RustcRelation::MirRvalue,
        ));
    }

    if statuses[&RustMirDerivedRelation::UnsafeFfi] == RustMirAnalysisCompleteness::Complete {
        derive_unsafe_ffi(
            provenance,
            raw,
            rvalues.as_deref().unwrap_or_default(),
            &mut unsafe_ffi,
            &mut unknowns,
        )?;
        statuses.insert(
            RustMirDerivedRelation::UnsafeFfi,
            RustMirAnalysisCompleteness::Partial,
        );
    }

    if statuses[&RustMirDerivedRelation::ControlDependenceInput]
        == RustMirAnalysisCompleteness::Complete
    {
        let operands = parse_operands(&raw.operands)?;
        let terminators = parse_terminators(&raw.terminators)?;
        derive_control_inputs(
            blocks,
            cfg_rows,
            &operands,
            &terminators,
            &mut control_inputs,
            &mut unknowns,
            &mut statuses,
        );
    }

    Ok(ExtendedAnalysisResult {
        ownership,
        aliases,
        resources,
        async_lowering,
        unsafe_ffi,
        control_inputs,
        statuses,
        unknowns,
    })
}

fn ownership_observation(access_kind: &str) -> Option<&'static str> {
    match access_kind {
        "BorrowShared" | "BorrowFake" => Some("SHARED_BORROW_OBSERVED"),
        "BorrowMut" => Some("MUTABLE_BORROW_OBSERVED"),
        "ReborrowShared" => Some("SHARED_REBORROW_OBSERVED"),
        "ReborrowMut" => Some("MUTABLE_REBORROW_OBSERVED"),
        "Move" => Some("MOVE_OBSERVED"),
        "Copy" | "CopyForDeref" => Some("COPY_OBSERVED"),
        "StorageLive" => Some("STORAGE_LIVE_OBSERVED"),
        "StorageDead" => Some("STORAGE_DEAD_OBSERVED"),
        "Drop" => Some("DROP_OBSERVED"),
        "AddressOfMut" | "AddressOfConst" | "AddressOfMetadata" => Some("RAW_ADDRESS_OBSERVED"),
        _ => None,
    }
}

fn semantic_gap(
    family: &'static str,
    reason: &'static str,
    detail: &'static str,
    relation: RustcRelation,
) -> UnknownRow {
    UnknownRow {
        family: Arc::from(family),
        reason_code: Arc::from(reason),
        detail: Arc::from(detail),
        bounded: true,
        input_relation: Some(Arc::from(relation.relation_id())),
    }
}

fn derive_unsafe_ffi(
    _provenance: &RustMirAnalysisProvenance,
    raw: &RustMirRawRelations,
    rvalues: &[RvalueObservation],
    rows: &mut Vec<UnsafeFfiRow>,
    unknowns: &mut BTreeSet<UnknownRow>,
) -> Result<(), RustMirAnalysisError> {
    let statements = parse_statements(&raw.statements)?;
    let terminators = parse_terminators(&raw.terminators)?;
    let calls = parse_calls(&raw.calls)?;
    let instances = parse_instances(&raw.instances)?;

    for terminator in terminators.values() {
        if terminator.raw_kind.as_ref() != "InlineAsm" {
            continue;
        }
        let block = terminator.block_index.to_be_bytes();
        rows.push(UnsafeFfiRow {
            observation_id: hash_parts(
                b"codefabric.rust-mir-unsafe-observation.v1\0",
                [block.as_slice(), b"INLINE_ASM"],
            ),
            block_index: terminator.block_index,
            slot_kind: Arc::from("terminator"),
            slot_index: 0,
            source_scope: terminator.source_scope,
            observation_kind: Arc::from("INLINE_ASM"),
            raw_kind: Arc::clone(&terminator.raw_kind),
            declared_target: None,
            resolved_instance_key: None,
            is_foreign_item: None,
            structured_evidence: Arc::from("MirTerminator::InlineAsm"),
        });
    }

    for rvalue in rvalues {
        let Some(cast) = rvalue.cast_kind.as_deref() else {
            continue;
        };
        if !matches!(
            cast,
            "PointerExposeAddress"
                | "PointerWithExposedProvenance"
                | "PtrToPtr"
                | "FnPtrToPtr"
                | "Transmute"
                | "BoxDerefTransmute"
        ) {
            continue;
        }
        let Some(statement_row) = statements.get(&(rvalue.block_index, rvalue.statement_index))
        else {
            unknowns.insert(semantic_gap(
                "unsafe-ffi",
                "UNSAFE_CAST_STATEMENT_NOT_JOINABLE",
                "an unsafe-relevant cast rvalue has no accepted MirStatement coordinate",
                RustcRelation::MirStatement,
            ));
            continue;
        };
        let block = rvalue.block_index.to_be_bytes();
        let statement = rvalue.statement_index.to_be_bytes();
        rows.push(UnsafeFfiRow {
            observation_id: hash_parts(
                b"codefabric.rust-mir-unsafe-observation.v1\0",
                [block.as_slice(), statement.as_slice(), cast.as_bytes()],
            ),
            block_index: rvalue.block_index,
            slot_kind: Arc::from("statement"),
            slot_index: rvalue.statement_index,
            source_scope: statement_row.source_scope,
            observation_kind: Arc::from("UNSAFE_RELEVANT_CAST"),
            raw_kind: Arc::from(cast),
            declared_target: None,
            resolved_instance_key: None,
            is_foreign_item: None,
            structured_evidence: Arc::from("MirRvalue::Cast"),
        });
    }

    for call in calls {
        let resolved = call
            .resolved_instance_key
            .and_then(|key| instances.get(&key));
        match resolved {
            Some(instance) if instance.is_foreign_item => {
                let Some(terminator) = terminators.get(&call.block_index) else {
                    unknowns.insert(semantic_gap(
                        "unsafe-ffi",
                        "FOREIGN_CALL_TERMINATOR_NOT_JOINABLE",
                        "a resolved foreign call has no accepted MirTerminator coordinate",
                        RustcRelation::MirTerminator,
                    ));
                    continue;
                };
                let block = call.block_index.to_be_bytes();
                let destination = call.destination_place_id;
                rows.push(UnsafeFfiRow {
                    observation_id: hash_parts(
                        b"codefabric.rust-mir-unsafe-observation.v1\0",
                        [
                            block.as_slice(),
                            instance.instance_key.as_slice(),
                            destination.as_slice(),
                            b"FOREIGN_CALL",
                        ],
                    ),
                    block_index: call.block_index,
                    slot_kind: Arc::from("terminator"),
                    slot_index: 0,
                    source_scope: terminator.source_scope,
                    observation_kind: Arc::from("FOREIGN_CALL"),
                    raw_kind: Arc::from("Call"),
                    declared_target: call
                        .declared_target
                        .clone()
                        .or_else(|| Some(Arc::clone(&instance.definition_path))),
                    resolved_instance_key: Some(instance.instance_key),
                    is_foreign_item: Some(true),
                    structured_evidence: Arc::from(format!(
                        "dispatch={};confidence={}",
                        call.dispatch_kind, call.resolution_confidence
                    )),
                });
            }
            None => {
                unknowns.insert(UnknownRow {
                    family: Arc::from("unsafe-ffi"),
                    reason_code: Arc::from("CALL_FOREIGN_STATUS_UNRESOLVED"),
                    detail: Arc::from(format!(
                        "call in block {} has no joined resolved Instance foreign-item evidence",
                        call.block_index
                    )),
                    bounded: true,
                    input_relation: Some(Arc::from(RustcRelation::Call.relation_id())),
                });
            }
            Some(_) => {}
        }
    }
    rows.sort();
    unknowns.insert(semantic_gap(
        "unsafe-ffi",
        "LEXICAL_UNSAFE_SCOPE_NOT_EXPOSED",
        "public MIR exposes unsafe-relevant operations but not a complete lexical unsafe-block relation",
        RustcRelation::MirTerminator,
    ));
    Ok(())
}

fn derive_control_inputs(
    blocks: &[BlockRow],
    edges: &[EdgeRow],
    operands: &[OperandObservation],
    terminators: &BTreeMap<u64, TerminatorObservation>,
    rows: &mut Vec<ControlInputRow>,
    unknowns: &mut BTreeSet<UnknownRow>,
    statuses: &mut BTreeMap<RustMirDerivedRelation, RustMirAnalysisCompleteness>,
) {
    let outgoing = edges
        .iter()
        .fold(BTreeMap::<u64, Vec<&EdgeRow>>::new(), |mut map, edge| {
            map.entry(edge.source_block).or_default().push(edge);
            map
        });
    for block in blocks {
        let Some(terminator) = terminators.get(&block.block_index) else {
            statuses.insert(
                RustMirDerivedRelation::ControlDependenceInput,
                RustMirAnalysisCompleteness::Partial,
            );
            unknowns.insert(semantic_gap(
                "control-dependence-input",
                "TERMINATOR_OBSERVATION_MISSING",
                "a MirBlock has no joined MirTerminator row",
                RustcRelation::MirTerminator,
            ));
            continue;
        };
        if terminator.raw_kind != block.terminator_kind {
            statuses.insert(
                RustMirDerivedRelation::ControlDependenceInput,
                RustMirAnalysisCompleteness::Partial,
            );
            unknowns.insert(semantic_gap(
                "control-dependence-input",
                "TERMINATOR_KIND_MISMATCH",
                "MirBlock and MirTerminator disagree on the native terminator kind",
                RustcRelation::MirTerminator,
            ));
        }
        let block_edges = outgoing
            .get(&block.block_index)
            .map_or(&[][..], Vec::as_slice);
        let is_controller = block_edges.len() > 1
            || block_edges
                .iter()
                .any(|edge| edge.edge_kind.as_ref() == "Unwind");
        if !is_controller {
            continue;
        }
        let predicate_role = match terminator.raw_kind.as_ref() {
            "SwitchInt" => Some("switch-discriminant"),
            "Assert" => Some("assert-condition"),
            _ => None,
        };
        let predicate = predicate_role.and_then(|role| {
            operands.iter().find(|operand| {
                operand.block_index == block.block_index
                    && operand.slot_kind.as_ref() == "terminator"
                    && operand.slot_index == 0
                    && operand.parent_role.as_ref() == role
            })
        });
        if predicate_role.is_some() && predicate.is_none() {
            statuses.insert(
                RustMirDerivedRelation::ControlDependenceInput,
                RustMirAnalysisCompleteness::Partial,
            );
            unknowns.insert(semantic_gap(
                "control-dependence-input",
                "CONTROL_PREDICATE_OPERAND_MISSING",
                "branch terminator lacks its accepted predicate operand row",
                RustcRelation::MirOperand,
            ));
        }
        let observed_normal = block_edges
            .iter()
            .filter(|edge| edge.edge_kind.as_ref() != "Unwind")
            .count() as u64;
        if observed_normal != terminator.normal_target_count {
            statuses.insert(
                RustMirDerivedRelation::ControlDependenceInput,
                RustMirAnalysisCompleteness::Partial,
            );
            unknowns.insert(semantic_gap(
                "control-dependence-input",
                "NORMAL_TARGET_COUNT_MISMATCH",
                "MirTerminator normal target count differs from accepted CFG edges",
                RustcRelation::CfgEdge,
            ));
        }
        for edge in block_edges {
            let block_bytes = block.block_index.to_be_bytes();
            rows.push(ControlInputRow {
                control_input_id: hash_parts(
                    b"codefabric.rust-mir-control-input.v1\0",
                    [
                        block_bytes.as_slice(),
                        edge.edge_id.as_slice(),
                        predicate.map_or(&[][..], |operand| operand.operand_id.as_slice()),
                    ],
                ),
                controller_block: block.block_index,
                controller_kind: Arc::clone(&terminator.raw_kind),
                predicate_operand_id: predicate.map(|operand| operand.operand_id),
                predicate_role: predicate.map(|operand| Arc::clone(&operand.parent_role)),
                predicate_operand_kind: predicate.map(|operand| Arc::clone(&operand.operand_kind)),
                source_scope: terminator.source_scope,
                normal_target_count: terminator.normal_target_count,
                unwind_action: terminator.unwind_action.clone(),
                edge_id: edge.edge_id,
                target_block: edge.target_block,
                edge_kind: Arc::clone(&edge.edge_kind),
                is_unwind: edge.edge_kind.as_ref() == "Unwind",
            });
        }
    }
    rows.sort();
}

fn execute_dataflow(
    blocks: &[BlockRow],
    edges: &[EdgeRow],
    events: &[EventRow],
    places: &BTreeMap<[u8; 32], PlaceObservation>,
) -> Result<DataflowResult, RustMirAnalysisError> {
    let block_by_id = blocks
        .iter()
        .map(|block| (block.block_index, block))
        .collect::<BTreeMap<_, _>>();
    let mut events_by_block = blocks
        .iter()
        .map(|block| (block.block_index, Vec::new()))
        .collect::<BTreeMap<_, Vec<&EventRow>>>();
    let mut definition_by_id = BTreeMap::<[u8; 32], &EventRow>::new();
    let mut location_by_id = places
        .values()
        .map(|place| (place.location.id, place.location.clone()))
        .collect::<BTreeMap<_, _>>();
    for event in events {
        let Some(block) = block_by_id.get(&event.block_index) else {
            return Err(RustMirAnalysisError::InvalidStructure(format!(
                "access event references absent block {}",
                event.block_index
            )));
        };
        validate_event_coordinate(event, block)?;
        events_by_block
            .get_mut(&event.block_index)
            .expect("block map was initialized from the block relation")
            .push(event);
        location_by_id
            .entry(event.location.id)
            .or_insert_with(|| event.location.clone());
        if event.is_definition && definition_by_id.insert(event.event_id, event).is_some() {
            return Err(RustMirAnalysisError::InvalidStructure(format!(
                "duplicate definition identity {}",
                hex_prefix(&event.event_id)
            )));
        }
    }
    for block_events in events_by_block.values_mut() {
        block_events.sort_by_key(|event| event.coordinate_key());
    }

    let mut incoming_edges = blocks
        .iter()
        .map(|block| (block.block_index, Vec::new()))
        .collect::<BTreeMap<_, Vec<&EdgeRow>>>();
    let mut outgoing_edges = incoming_edges.clone();
    for edge in edges {
        incoming_edges
            .get_mut(&edge.target_block)
            .expect("CFG validation proved target membership")
            .push(edge);
        outgoing_edges
            .get_mut(&edge.source_block)
            .expect("CFG validation proved source membership")
            .push(edge);
    }

    let (reaching_in, reaching_out) = reaching_fixed_point(
        blocks,
        &incoming_edges,
        &outgoing_edges,
        &events_by_block,
        definition_by_id.len(),
    )?;
    let mut result = DataflowResult::default();

    for block in blocks {
        let mut current = reaching_in
            .get(&block.block_index)
            .cloned()
            .unwrap_or_default();
        for event in &events_by_block[&block.block_index] {
            if event.is_use {
                match current.get(&event.location.id) {
                    Some(definitions) if !definitions.is_empty() => {
                        for definition_id in definitions {
                            let definition =
                                definition_by_id.get(definition_id).ok_or_else(|| {
                                    RustMirAnalysisError::InvalidStructure(format!(
                                        "reaching state references absent definition {}",
                                        hex_prefix(definition_id)
                                    ))
                                })?;
                            result.def_use.push(DefUseRow {
                                location_id: event.location.id,
                                base_local: event.location.base_local,
                                projection_path: Arc::clone(&event.location.projection_path),
                                definition: EventRef::from(*definition),
                                use_event: EventRef::from(*event),
                            });
                        }
                    }
                    _ => {
                        result.unknowns.insert(UnknownRow {
                            family: Arc::from("def-use"),
                            reason_code: Arc::from("NO_REACHING_DEFINITION_WITNESS"),
                            detail: Arc::from(format!(
                                "use {} at block {} {} {}:{} has no public-MIR definition witness",
                                hex_prefix(&event.event_id),
                                event.block_index,
                                event.slot_kind,
                                event.slot_index,
                                event.access_ordinal
                            )),
                            bounded: true,
                            input_relation: Some(Arc::from(RustcRelation::Access.relation_id())),
                        });
                    }
                }
            }
            apply_event_transfer(&mut current, event);
        }
    }
    result.def_use.sort();

    for block in blocks {
        for (boundary, state) in [
            ("ENTRY", &reaching_in[&block.block_index]),
            ("EXIT", &reaching_out[&block.block_index]),
        ] {
            for (location_id, definitions) in state {
                let location = &location_by_id[location_id];
                for definition_id in definitions {
                    let definition = definition_by_id.get(definition_id).ok_or_else(|| {
                        RustMirAnalysisError::InvalidStructure(format!(
                            "reaching boundary references absent definition {}",
                            hex_prefix(definition_id)
                        ))
                    })?;
                    result.reaching.push(ReachingRow {
                        block_index: block.block_index,
                        boundary,
                        location_id: *location_id,
                        base_local: location.base_local,
                        projection_path: Arc::clone(&location.projection_path),
                        definition: EventRef::from(*definition),
                    });
                }
            }
        }
    }
    result.reaching.sort();

    let (live_in, live_out) = liveness_fixed_point(
        blocks,
        &outgoing_edges,
        &events_by_block,
        location_by_id.len(),
    )?;
    for block in blocks {
        for (boundary, locations) in [
            ("ENTRY", &live_in[&block.block_index]),
            ("EXIT", &live_out[&block.block_index]),
        ] {
            for location_id in locations {
                let location = &location_by_id[location_id];
                result.liveness.push(LivenessRow {
                    block_index: block.block_index,
                    boundary,
                    location_id: *location_id,
                    base_local: location.base_local,
                    projection_path: Arc::clone(&location.projection_path),
                });
            }
        }
    }
    result.liveness.sort();
    Ok(result)
}

fn validate_event_coordinate(
    event: &EventRow,
    block: &BlockRow,
) -> Result<(), RustMirAnalysisError> {
    match event.slot_kind.as_ref() {
        "statement" if event.slot_index < block.statement_count => Ok(()),
        "terminator" if event.slot_index == 0 => Ok(()),
        kind => Err(RustMirAnalysisError::InvalidStructure(format!(
            "access event {} has invalid {kind} coordinate {} in block {} with {} statements",
            hex_prefix(&event.event_id),
            event.slot_index,
            block.block_index,
            block.statement_count
        ))),
    }
}

fn reaching_fixed_point(
    blocks: &[BlockRow],
    incoming_edges: &BTreeMap<u64, Vec<&EdgeRow>>,
    outgoing_edges: &BTreeMap<u64, Vec<&EdgeRow>>,
    events_by_block: &BTreeMap<u64, Vec<&EventRow>>,
    definition_count: usize,
) -> Result<
    (
        BTreeMap<u64, DefinitionState>,
        BTreeMap<u64, DefinitionState>,
    ),
    RustMirAnalysisError,
> {
    let mut inputs = blocks
        .iter()
        .map(|block| (block.block_index, DefinitionState::new()))
        .collect::<BTreeMap<_, _>>();
    let mut outputs = inputs.clone();
    let mut edge_outputs = incoming_edges
        .values()
        .flatten()
        .map(|edge| (edge.edge_id, DefinitionState::new()))
        .collect::<BTreeMap<_, _>>();
    let maximum_iterations = blocks
        .len()
        .saturating_mul(definition_count.saturating_add(1))
        .saturating_add(1);
    for _ in 0..maximum_iterations {
        let mut changed = false;
        for block in blocks {
            let mut input = DefinitionState::new();
            for edge in &incoming_edges[&block.block_index] {
                union_definition_state(&mut input, &edge_outputs[&edge.edge_id]);
            }
            let mut output = DefinitionState::new();
            if outgoing_edges[&block.block_index].is_empty() {
                output = input.clone();
                for event in &events_by_block[&block.block_index] {
                    apply_event_transfer(&mut output, event);
                }
            } else {
                for edge in &outgoing_edges[&block.block_index] {
                    let mut edge_output = input.clone();
                    for event in &events_by_block[&block.block_index] {
                        if event_executes_on_edge(event, edge) {
                            apply_event_transfer(&mut edge_output, event);
                        }
                    }
                    union_definition_state(&mut output, &edge_output);
                    if edge_outputs[&edge.edge_id] != edge_output {
                        edge_outputs.insert(edge.edge_id, edge_output);
                        changed = true;
                    }
                }
            }
            if inputs[&block.block_index] != input {
                inputs.insert(block.block_index, input);
                changed = true;
            }
            if outputs[&block.block_index] != output {
                outputs.insert(block.block_index, output);
                changed = true;
            }
        }
        if !changed {
            return Ok((inputs, outputs));
        }
    }
    Err(RustMirAnalysisError::InvalidStructure(
        "reaching-definition fixed point did not converge within the finite definition lattice"
            .to_owned(),
    ))
}

fn union_definition_state(target: &mut DefinitionState, source: &DefinitionState) {
    for (location, definitions) in source {
        target
            .entry(*location)
            .or_default()
            .extend(definitions.iter().copied());
    }
}

fn apply_event_transfer(state: &mut DefinitionState, event: &EventRow) {
    if event.is_kill {
        state.remove(&event.location.id);
    }
    if event.is_definition {
        state
            .entry(event.location.id)
            .or_default()
            .insert(event.event_id);
    }
}

fn event_executes_on_edge(event: &EventRow, edge: &EdgeRow) -> bool {
    !(edge.edge_kind.as_ref() == "Unwind"
        && event.slot_kind.as_ref() == "terminator"
        && event.is_definition
        && matches!(
            event.structured_evidence.as_ref(),
            "TerminatorKind::Call.destination" | "TerminatorKind::InlineAsm.output"
        ))
}

type LiveState = BTreeSet<[u8; 32]>;

fn liveness_fixed_point(
    blocks: &[BlockRow],
    outgoing_edges: &BTreeMap<u64, Vec<&EdgeRow>>,
    events_by_block: &BTreeMap<u64, Vec<&EventRow>>,
    location_count: usize,
) -> Result<(BTreeMap<u64, LiveState>, BTreeMap<u64, LiveState>), RustMirAnalysisError> {
    let mut inputs = blocks
        .iter()
        .map(|block| (block.block_index, LiveState::new()))
        .collect::<BTreeMap<_, _>>();
    let mut outputs = inputs.clone();
    let maximum_iterations = blocks
        .len()
        .saturating_mul(location_count.saturating_add(1))
        .saturating_add(1);
    for _ in 0..maximum_iterations {
        let mut changed = false;
        for block in blocks.iter().rev() {
            let mut output = LiveState::new();
            for edge in &outgoing_edges[&block.block_index] {
                output.extend(inputs[&edge.target_block].iter().copied());
            }
            let mut input = LiveState::new();
            if outgoing_edges[&block.block_index].is_empty() {
                input = reverse_liveness_transfer(
                    LiveState::new(),
                    &events_by_block[&block.block_index],
                    None,
                );
            } else {
                for edge in &outgoing_edges[&block.block_index] {
                    input.extend(reverse_liveness_transfer(
                        inputs[&edge.target_block].clone(),
                        &events_by_block[&block.block_index],
                        Some(edge),
                    ));
                }
            }
            if outputs[&block.block_index] != output {
                outputs.insert(block.block_index, output);
                changed = true;
            }
            if inputs[&block.block_index] != input {
                inputs.insert(block.block_index, input);
                changed = true;
            }
        }
        if !changed {
            return Ok((inputs, outputs));
        }
    }
    Err(RustMirAnalysisError::InvalidStructure(
        "liveness fixed point did not converge within the finite location lattice".to_owned(),
    ))
}

fn reverse_liveness_transfer(
    mut live: LiveState,
    events: &[&EventRow],
    edge: Option<&EdgeRow>,
) -> LiveState {
    for event in events.iter().rev() {
        if edge.is_some_and(|edge| !event_executes_on_edge(event, edge)) {
            continue;
        }
        if event.is_kill || event.is_definition {
            live.remove(&event.location.id);
        }
        if event.is_use {
            live.insert(event.location.id);
        }
    }
    live
}

fn unsupported_remainders(exact_private_complete: bool) -> BTreeSet<UnknownRow> {
    let mut remainders = vec![(
        "alias-points-to",
        "EXACT_ALIAS_CLOSURE_NOT_PROVABLE",
        "reference construction supports conservative may-point-to edges; public MIR does not prove must-alias, no-alias, or complete transitive points-to closure",
    )];
    if !exact_private_complete {
        remainders.push((
            "ownership-state",
            "EXACT_LOAN_REGION_REQUIRES_PRIVATE_ENRICHMENT",
            "public MIR access observations support bounded ownership transitions but do not carry exact borrow-checker loans or region constraints",
        ));
    }
    remainders
        .into_iter()
        .map(|(family, reason_code, detail)| UnknownRow {
            family: Arc::from(family),
            reason_code: Arc::from(reason_code),
            detail: Arc::from(detail),
            bounded: true,
            input_relation: None,
        })
        .collect()
}

fn append_input_unknowns(
    unknowns: &mut BTreeSet<UnknownRow>,
    completeness: &RustMirInputCompleteness,
) {
    for (relation, state, affected) in [
        (
            RustcRelation::MirBlock,
            &completeness.blocks,
            &[
                "cfg",
                "def-use",
                "reaching-definition",
                "liveness",
                "control-dependence-input",
            ][..],
        ),
        (
            RustcRelation::MirLocal,
            &completeness.locals,
            &["ownership-state"][..],
        ),
        (
            RustcRelation::MirPlace,
            &completeness.places,
            &[
                "def-use",
                "reaching-definition",
                "liveness",
                "ownership-state",
                "alias-points-to",
                "drop-resource",
            ][..],
        ),
        (
            RustcRelation::MirOperand,
            &completeness.operands,
            &["control-dependence-input"][..],
        ),
        (
            RustcRelation::MirRvalue,
            &completeness.rvalues,
            &["alias-points-to", "async-lowering", "unsafe-ffi"][..],
        ),
        (
            RustcRelation::MirStatement,
            &completeness.statements,
            &["alias-points-to", "async-lowering", "unsafe-ffi"][..],
        ),
        (
            RustcRelation::MirTerminator,
            &completeness.terminators,
            &["unsafe-ffi", "control-dependence-input"][..],
        ),
        (
            RustcRelation::CfgEdge,
            &completeness.cfg_edges,
            &[
                "cfg",
                "def-use",
                "reaching-definition",
                "liveness",
                "control-dependence-input",
            ][..],
        ),
        (
            RustcRelation::Call,
            &completeness.calls,
            &["unsafe-ffi"][..],
        ),
        (
            RustcRelation::Instance,
            &completeness.instances,
            &["unsafe-ffi"][..],
        ),
        (
            RustcRelation::Access,
            &completeness.accesses,
            &[
                "def-use",
                "reaching-definition",
                "liveness",
                "ownership-state",
                "alias-points-to",
                "drop-resource",
            ][..],
        ),
    ] {
        if let Some(reason) = state.reason() {
            for family in affected {
                unknowns.insert(UnknownRow {
                    family: Arc::from(*family),
                    reason_code: Arc::from("RAW_INPUT_NOT_COMPLETE"),
                    detail: Arc::from(format!(
                        "{} is {}: {reason}",
                        relation.relation_id(),
                        state.status()
                    )),
                    bounded: true,
                    input_relation: Some(Arc::from(relation.relation_id())),
                });
            }
        }
    }
}

fn common_output_fields() -> Vec<Field> {
    vec![
        Field::new("model_epoch_id", DataType::FixedSizeBinary(32), false),
        Field::new("source_snapshot_pin", DataType::FixedSizeBinary(32), false),
        Field::new("analysis_context_pin", DataType::FixedSizeBinary(32), false),
        Field::new("source_generation", DataType::UInt64, false),
        Field::new("provider_run_id", DataType::Utf8, false),
        Field::new("compilation_unit_id", DataType::Utf8, false),
        Field::new("owner_id", DataType::Utf8, false),
        Field::new("source_file_id", DataType::Utf8, false),
        Field::new(
            "source_content_digest",
            DataType::FixedSizeBinary(32),
            false,
        ),
        Field::new("stable_crate_id", DataType::UInt64, true),
        Field::new("def_path_hash", DataType::FixedSizeBinary(16), true),
        Field::new("rustc_release", DataType::Utf8, false),
        Field::new("rustc_commit", DataType::Utf8, false),
        Field::new("rustc_toolchain", DataType::Utf8, false),
        Field::new(
            "toolchain_identity_digest",
            DataType::FixedSizeBinary(32),
            false,
        ),
        Field::new(
            "raw_schema_bundle_digest",
            DataType::FixedSizeBinary(32),
            false,
        ),
        Field::new("algorithm_release", DataType::Utf8, false),
        Field::new("authority", DataType::Utf8, false),
        Field::new("precision_release", DataType::Utf8, false),
        Field::new("analysis_completeness", DataType::Utf8, false),
    ]
}

fn common_output_columns(
    provenance: &RustMirAnalysisProvenance,
    rows: usize,
    completeness: &str,
) -> Vec<ArrayRef> {
    let stable_crate_id = provenance.stable_owner_key.map(|key| key.stable_crate_id);
    let def_path_hash = provenance.stable_owner_key.map(|key| key.def_path_hash);
    vec![
        fixed_repeat(Some(&provenance.model_epoch_id), rows),
        fixed_repeat(Some(&provenance.source_snapshot_pin), rows),
        fixed_repeat(Some(&provenance.analysis_context_pin), rows),
        u64_repeat(Some(provenance.source_generation), rows),
        string_repeat(provenance.provider_run_id.as_ref(), rows),
        string_repeat(provenance.compilation_unit_id.as_ref(), rows),
        string_repeat(provenance.owner_id.as_ref(), rows),
        string_repeat(provenance.source_file_id.as_ref(), rows),
        fixed_repeat(Some(&provenance.source_content_digest), rows),
        u64_repeat(stable_crate_id, rows),
        fixed_repeat(def_path_hash.as_ref(), rows),
        string_repeat(provenance.rustc_release.as_ref(), rows),
        string_repeat(provenance.rustc_commit.as_ref(), rows),
        string_repeat(provenance.rustc_toolchain.as_ref(), rows),
        fixed_repeat(Some(&provenance.toolchain_identity_digest), rows),
        fixed_repeat(Some(&provenance.raw_schema_bundle_digest), rows),
        string_repeat(RUST_MIR_DERIVED_ANALYSIS_RELEASE, rows),
        string_repeat(RUST_MIR_DERIVED_AUTHORITY, rows),
        string_repeat(RUST_MIR_DERIVED_PRECISION_RELEASE, rows),
        string_repeat(completeness, rows),
    ]
}

fn private_enrichment_schema(binding: &RustMirPrivateEnrichmentBinding) -> SchemaRef {
    Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("model_epoch_id", DataType::FixedSizeBinary(32), false),
            Field::new("source_snapshot_pin", DataType::FixedSizeBinary(32), false),
            Field::new("analysis_context_pin", DataType::FixedSizeBinary(32), false),
            Field::new("source_generation", DataType::UInt64, false),
            Field::new("provider_run_id", DataType::Utf8, false),
            Field::new("provider_release", DataType::Utf8, false),
            Field::new("compilation_unit_id", DataType::Utf8, false),
            Field::new("owner_id", DataType::Utf8, false),
            Field::new("source_file_id", DataType::Utf8, false),
            Field::new(
                "source_content_digest",
                DataType::FixedSizeBinary(32),
                false,
            ),
            Field::new("stable_crate_id", DataType::UInt64, false),
            Field::new("def_path_hash", DataType::FixedSizeBinary(16), false),
            Field::new("rustc_release", DataType::Utf8, false),
            Field::new("rustc_commit", DataType::Utf8, false),
            Field::new("rustc_toolchain", DataType::Utf8, false),
            Field::new(
                "toolchain_identity_digest",
                DataType::FixedSizeBinary(32),
                false,
            ),
            Field::new("authority", DataType::Utf8, false),
            Field::new("analysis_completeness", DataType::Utf8, false),
            Field::new("loan_id", DataType::FixedSizeBinary(32), false),
            Field::new("place_id", DataType::FixedSizeBinary(32), false),
            Field::new("region_id", DataType::FixedSizeBinary(32), false),
            Field::new("loan_kind", DataType::Utf8, false),
            Field::new("issued_block", DataType::UInt64, false),
            Field::new("issued_slot_kind", DataType::Utf8, false),
            Field::new("issued_slot_index", DataType::UInt64, false),
            Field::new("killed_block", DataType::UInt64, true),
            Field::new("killed_slot_kind", DataType::Utf8, true),
            Field::new("killed_slot_index", DataType::UInt64, true),
        ],
        [
            (
                "codefabric.relation_id".to_owned(),
                binding.relation_id.as_str().to_owned(),
            ),
            (
                "codefabric.authority".to_owned(),
                binding.authority.label().to_owned(),
            ),
            (
                "codefabric.semantic_encoding".to_owned(),
                "typed-arrow-exact-private-enrichment".to_owned(),
            ),
        ]
        .into_iter()
        .collect(),
    ))
}

#[allow(clippy::too_many_arguments)]
fn private_enrichment_batch(
    provenance: &RustMirAnalysisProvenance,
    rows: &[RustMirExactBorrowckRow],
    binding: &RustMirPrivateEnrichmentBinding,
    provider_run_id: &str,
    provider_release: &str,
    completeness: &str,
) -> Result<RecordBatch, ArrowError> {
    let stable_crate_id = provenance.stable_owner_key.map(|key| key.stable_crate_id);
    let def_path_hash = provenance.stable_owner_key.map(|key| key.def_path_hash);
    let columns = vec![
        fixed_repeat(Some(&provenance.model_epoch_id), rows.len()),
        fixed_repeat(Some(&provenance.source_snapshot_pin), rows.len()),
        fixed_repeat(Some(&provenance.analysis_context_pin), rows.len()),
        u64_repeat(Some(provenance.source_generation), rows.len()),
        string_repeat(provider_run_id, rows.len()),
        string_repeat(provider_release, rows.len()),
        string_repeat(provenance.compilation_unit_id.as_ref(), rows.len()),
        string_repeat(provenance.owner_id.as_ref(), rows.len()),
        string_repeat(provenance.source_file_id.as_ref(), rows.len()),
        fixed_repeat(Some(&provenance.source_content_digest), rows.len()),
        u64_repeat(stable_crate_id, rows.len()),
        fixed_repeat(def_path_hash.as_ref(), rows.len()),
        string_repeat(provenance.rustc_release.as_ref(), rows.len()),
        string_repeat(provenance.rustc_commit.as_ref(), rows.len()),
        string_repeat(provenance.rustc_toolchain.as_ref(), rows.len()),
        fixed_repeat(Some(&provenance.toolchain_identity_digest), rows.len()),
        string_repeat(binding.authority.label(), rows.len()),
        string_repeat(completeness, rows.len()),
        fixed_values(rows.iter().map(|row| Some(&row.loan_id))),
        fixed_values(rows.iter().map(|row| Some(&row.place_id))),
        fixed_values(rows.iter().map(|row| Some(&row.region_id))),
        string_values(rows.iter().map(|row| row.loan_kind.as_ref())),
        u64_values(rows.iter().map(|row| row.issued_block)),
        string_values(rows.iter().map(|row| row.issued_slot_kind.as_ref())),
        u64_values(rows.iter().map(|row| row.issued_slot_index)),
        optional_u64_values(rows.iter().map(|row| row.killed_block)),
        optional_string_values(rows.iter().map(|row| row.killed_slot_kind.as_deref())),
        optional_u64_values(rows.iter().map(|row| row.killed_slot_index)),
    ];
    RecordBatch::try_new(private_enrichment_schema(binding), columns)
}

fn cfg_batch(
    provenance: &RustMirAnalysisProvenance,
    rows: &[EdgeRow],
    bindings: &RustMirAnalysisBindings,
    completeness: &str,
) -> Result<RecordBatch, ArrowError> {
    let mut columns = common_output_columns(provenance, rows.len(), completeness);
    columns.extend([
        fixed_values(rows.iter().map(|row| Some(&row.edge_id))),
        Arc::new(UInt64Array::from_iter_values(
            rows.iter().map(|row| row.source_block),
        )),
        Arc::new(UInt64Array::from_iter_values(
            rows.iter().map(|row| row.target_block),
        )),
        string_values(rows.iter().map(|row| row.edge_kind.as_ref())),
        optional_string_values(rows.iter().map(|row| row.branch_value.as_deref())),
        optional_string_values(rows.iter().map(|row| row.unwind_action.as_deref())),
    ]);
    RecordBatch::try_new(
        RustMirDerivedRelation::CfgEdge.schema_with_bindings(bindings),
        columns,
    )
}

fn def_use_batch(
    provenance: &RustMirAnalysisProvenance,
    rows: &[DefUseRow],
    bindings: &RustMirAnalysisBindings,
    completeness: &str,
) -> Result<RecordBatch, ArrowError> {
    let mut columns = common_output_columns(provenance, rows.len(), completeness);
    columns.extend([
        fixed_values(rows.iter().map(|row| Some(&row.location_id))),
        Arc::new(UInt64Array::from_iter_values(
            rows.iter().map(|row| row.base_local),
        )),
        string_values(rows.iter().map(|row| row.projection_path.as_ref())),
        fixed_values(rows.iter().map(|row| Some(&row.definition.event_id))),
        fixed_values(rows.iter().map(|row| Some(&row.definition.place_id))),
        Arc::new(UInt64Array::from_iter_values(
            rows.iter().map(|row| row.definition.block_index),
        )),
        string_values(rows.iter().map(|row| row.definition.slot_kind.as_ref())),
        Arc::new(UInt64Array::from_iter_values(
            rows.iter().map(|row| row.definition.slot_index),
        )),
        Arc::new(UInt64Array::from_iter_values(
            rows.iter().map(|row| row.definition.access_ordinal),
        )),
        string_values(rows.iter().map(|row| row.definition.access_kind.as_ref())),
        string_values(
            rows.iter()
                .map(|row| row.definition.structured_evidence.as_ref()),
        ),
        Arc::new(BooleanArray::from_iter(
            rows.iter().map(|row| Some(row.definition.runtime_effect)),
        )),
        fixed_values(rows.iter().map(|row| Some(&row.use_event.event_id))),
        fixed_values(rows.iter().map(|row| Some(&row.use_event.place_id))),
        Arc::new(UInt64Array::from_iter_values(
            rows.iter().map(|row| row.use_event.block_index),
        )),
        string_values(rows.iter().map(|row| row.use_event.slot_kind.as_ref())),
        Arc::new(UInt64Array::from_iter_values(
            rows.iter().map(|row| row.use_event.slot_index),
        )),
        Arc::new(UInt64Array::from_iter_values(
            rows.iter().map(|row| row.use_event.access_ordinal),
        )),
        string_values(rows.iter().map(|row| row.use_event.access_kind.as_ref())),
        string_values(
            rows.iter()
                .map(|row| row.use_event.structured_evidence.as_ref()),
        ),
        Arc::new(BooleanArray::from_iter(
            rows.iter().map(|row| Some(row.use_event.runtime_effect)),
        )),
        string_repeat("MAY_REACH", rows.len()),
    ]);
    RecordBatch::try_new(
        RustMirDerivedRelation::DefUse.schema_with_bindings(bindings),
        columns,
    )
}

fn reaching_batch(
    provenance: &RustMirAnalysisProvenance,
    rows: &[ReachingRow],
    bindings: &RustMirAnalysisBindings,
    completeness: &str,
) -> Result<RecordBatch, ArrowError> {
    let mut columns = common_output_columns(provenance, rows.len(), completeness);
    columns.extend([
        Arc::new(UInt64Array::from_iter_values(
            rows.iter().map(|row| row.block_index),
        )) as ArrayRef,
        string_values(rows.iter().map(|row| row.boundary)),
        fixed_values(rows.iter().map(|row| Some(&row.location_id))),
        Arc::new(UInt64Array::from_iter_values(
            rows.iter().map(|row| row.base_local),
        )),
        string_values(rows.iter().map(|row| row.projection_path.as_ref())),
        fixed_values(rows.iter().map(|row| Some(&row.definition.event_id))),
        fixed_values(rows.iter().map(|row| Some(&row.definition.place_id))),
        Arc::new(UInt64Array::from_iter_values(
            rows.iter().map(|row| row.definition.block_index),
        )),
        string_values(rows.iter().map(|row| row.definition.slot_kind.as_ref())),
        Arc::new(UInt64Array::from_iter_values(
            rows.iter().map(|row| row.definition.slot_index),
        )),
        Arc::new(UInt64Array::from_iter_values(
            rows.iter().map(|row| row.definition.access_ordinal),
        )),
        string_values(rows.iter().map(|row| row.definition.access_kind.as_ref())),
        string_values(
            rows.iter()
                .map(|row| row.definition.structured_evidence.as_ref()),
        ),
        Arc::new(BooleanArray::from_iter(
            rows.iter().map(|row| Some(row.definition.runtime_effect)),
        )),
    ]);
    RecordBatch::try_new(
        RustMirDerivedRelation::ReachingDefinition.schema_with_bindings(bindings),
        columns,
    )
}

fn liveness_batch(
    provenance: &RustMirAnalysisProvenance,
    rows: &[LivenessRow],
    bindings: &RustMirAnalysisBindings,
    completeness: &str,
) -> Result<RecordBatch, ArrowError> {
    let mut columns = common_output_columns(provenance, rows.len(), completeness);
    columns.extend([
        Arc::new(UInt64Array::from_iter_values(
            rows.iter().map(|row| row.block_index),
        )) as ArrayRef,
        string_values(rows.iter().map(|row| row.boundary)),
        fixed_values(rows.iter().map(|row| Some(&row.location_id))),
        Arc::new(UInt64Array::from_iter_values(
            rows.iter().map(|row| row.base_local),
        )),
        string_values(rows.iter().map(|row| row.projection_path.as_ref())),
    ]);
    RecordBatch::try_new(
        RustMirDerivedRelation::Liveness.schema_with_bindings(bindings),
        columns,
    )
}

fn ownership_batch(
    provenance: &RustMirAnalysisProvenance,
    rows: &[OwnershipRow],
    bindings: &RustMirAnalysisBindings,
    completeness: &str,
) -> Result<RecordBatch, ArrowError> {
    let mut columns = common_output_columns(provenance, rows.len(), completeness);
    columns.extend([
        fixed_values(rows.iter().map(|row| Some(&row.event.event_id))),
        fixed_values(rows.iter().map(|row| Some(&row.event.place_id))),
        fixed_values(rows.iter().map(|row| Some(&row.location_id))),
        u64_values(rows.iter().map(|row| row.base_local)),
        string_values(rows.iter().map(|row| row.projection_path.as_ref())),
        optional_string_values(rows.iter().map(|row| row.local_role.as_deref())),
        fixed_values(rows.iter().map(|row| row.local_type_key.as_ref())),
        optional_string_values(rows.iter().map(|row| row.local_mutability.as_deref())),
        u64_values(rows.iter().map(|row| row.event.block_index)),
        string_values(rows.iter().map(|row| row.event.slot_kind.as_ref())),
        u64_values(rows.iter().map(|row| row.event.slot_index)),
        u64_values(rows.iter().map(|row| row.event.access_ordinal)),
        string_values(rows.iter().map(|row| row.event.access_kind.as_ref())),
        string_values(rows.iter().map(|row| row.ownership_observation.as_ref())),
        string_values(
            rows.iter()
                .map(|row| row.event.structured_evidence.as_ref()),
        ),
    ]);
    RecordBatch::try_new(
        RustMirDerivedRelation::OwnershipState.schema_with_bindings(bindings),
        columns,
    )
}

fn alias_batch(
    provenance: &RustMirAnalysisProvenance,
    rows: &[AliasRow],
    bindings: &RustMirAnalysisBindings,
    completeness: &str,
) -> Result<RecordBatch, ArrowError> {
    let mut columns = common_output_columns(provenance, rows.len(), completeness);
    columns.extend([
        fixed_values(rows.iter().map(|row| Some(&row.observation_id))),
        fixed_values(rows.iter().map(|row| Some(&row.pointer_place_id))),
        fixed_values(rows.iter().map(|row| Some(&row.pointer_location_id))),
        fixed_values(rows.iter().map(|row| Some(&row.pointee_place_id))),
        fixed_values(rows.iter().map(|row| Some(&row.pointee_location_id))),
        u64_values(rows.iter().map(|row| row.block_index)),
        u64_values(rows.iter().map(|row| row.statement_index)),
        string_values(rows.iter().map(|row| row.rvalue_kind.as_ref())),
        string_values(rows.iter().map(|row| row.normalized_effect.as_ref())),
        u64_values(rows.iter().map(|row| row.source_scope)),
        optional_string_values(rows.iter().map(|row| row.region_kind.as_deref())),
        optional_string_values(rows.iter().map(|row| row.mutability.as_deref())),
        string_repeat("MAY_POINT_TO", rows.len()),
    ]);
    RecordBatch::try_new(
        RustMirDerivedRelation::AliasPointsTo.schema_with_bindings(bindings),
        columns,
    )
}

fn resource_batch(
    provenance: &RustMirAnalysisProvenance,
    rows: &[ResourceRow],
    bindings: &RustMirAnalysisBindings,
    completeness: &str,
) -> Result<RecordBatch, ArrowError> {
    let mut columns = common_output_columns(provenance, rows.len(), completeness);
    columns.extend([
        fixed_values(rows.iter().map(|row| Some(&row.lifecycle_event_id))),
        fixed_values(rows.iter().map(|row| Some(&row.event.place_id))),
        fixed_values(rows.iter().map(|row| Some(&row.location_id))),
        u64_values(rows.iter().map(|row| row.base_local)),
        string_values(rows.iter().map(|row| row.projection_path.as_ref())),
        u64_values(rows.iter().map(|row| row.event.block_index)),
        string_values(rows.iter().map(|row| row.event.slot_kind.as_ref())),
        u64_values(rows.iter().map(|row| row.event.slot_index)),
        string_values(rows.iter().map(|row| row.lifecycle_event.as_ref())),
        string_values(
            rows.iter()
                .map(|row| row.event.structured_evidence.as_ref()),
        ),
    ]);
    RecordBatch::try_new(
        RustMirDerivedRelation::ResourceLifecycle.schema_with_bindings(bindings),
        columns,
    )
}

fn async_batch(
    provenance: &RustMirAnalysisProvenance,
    rows: &[AsyncRow],
    bindings: &RustMirAnalysisBindings,
    completeness: &str,
) -> Result<RecordBatch, ArrowError> {
    let mut columns = common_output_columns(provenance, rows.len(), completeness);
    columns.extend([
        fixed_values(rows.iter().map(|row| Some(&row.observation_id))),
        u64_values(rows.iter().map(|row| row.block_index)),
        u64_values(rows.iter().map(|row| row.statement_index)),
        u64_values(rows.iter().map(|row| row.source_scope)),
        string_values(rows.iter().map(|row| row.rvalue_kind.as_ref())),
        string_values(rows.iter().map(|row| row.aggregate_kind.as_ref())),
        fixed_values(rows.iter().map(|row| row.result_type_key.as_ref())),
        string_repeat("COROUTINE_AGGREGATE_LOWERING_EVIDENCE", rows.len()),
    ]);
    RecordBatch::try_new(
        RustMirDerivedRelation::AsyncLowering.schema_with_bindings(bindings),
        columns,
    )
}

fn unsafe_ffi_batch(
    provenance: &RustMirAnalysisProvenance,
    rows: &[UnsafeFfiRow],
    bindings: &RustMirAnalysisBindings,
    completeness: &str,
) -> Result<RecordBatch, ArrowError> {
    let mut columns = common_output_columns(provenance, rows.len(), completeness);
    columns.extend([
        fixed_values(rows.iter().map(|row| Some(&row.observation_id))),
        u64_values(rows.iter().map(|row| row.block_index)),
        string_values(rows.iter().map(|row| row.slot_kind.as_ref())),
        u64_values(rows.iter().map(|row| row.slot_index)),
        u64_values(rows.iter().map(|row| row.source_scope)),
        string_values(rows.iter().map(|row| row.observation_kind.as_ref())),
        string_values(rows.iter().map(|row| row.raw_kind.as_ref())),
        optional_string_values(rows.iter().map(|row| row.declared_target.as_deref())),
        fixed_values(rows.iter().map(|row| row.resolved_instance_key.as_ref())),
        optional_bool_values(rows.iter().map(|row| row.is_foreign_item)),
        string_values(rows.iter().map(|row| row.structured_evidence.as_ref())),
    ]);
    RecordBatch::try_new(
        RustMirDerivedRelation::UnsafeFfi.schema_with_bindings(bindings),
        columns,
    )
}

fn control_input_batch(
    provenance: &RustMirAnalysisProvenance,
    rows: &[ControlInputRow],
    bindings: &RustMirAnalysisBindings,
    completeness: &str,
) -> Result<RecordBatch, ArrowError> {
    let mut columns = common_output_columns(provenance, rows.len(), completeness);
    columns.extend([
        fixed_values(rows.iter().map(|row| Some(&row.control_input_id))),
        u64_values(rows.iter().map(|row| row.controller_block)),
        string_values(rows.iter().map(|row| row.controller_kind.as_ref())),
        fixed_values(rows.iter().map(|row| row.predicate_operand_id.as_ref())),
        optional_string_values(rows.iter().map(|row| row.predicate_role.as_deref())),
        optional_string_values(rows.iter().map(|row| row.predicate_operand_kind.as_deref())),
        u64_values(rows.iter().map(|row| row.source_scope)),
        u64_values(rows.iter().map(|row| row.normal_target_count)),
        optional_string_values(rows.iter().map(|row| row.unwind_action.as_deref())),
        fixed_values(rows.iter().map(|row| Some(&row.edge_id))),
        u64_values(rows.iter().map(|row| row.target_block)),
        string_values(rows.iter().map(|row| row.edge_kind.as_ref())),
        bool_values(rows.iter().map(|row| row.is_unwind)),
    ]);
    RecordBatch::try_new(
        RustMirDerivedRelation::ControlDependenceInput.schema_with_bindings(bindings),
        columns,
    )
}

fn unknown_batch(
    provenance: &RustMirAnalysisProvenance,
    rows: &[UnknownRow],
    bindings: &RustMirAnalysisBindings,
) -> Result<RecordBatch, ArrowError> {
    let mut columns = common_output_columns(provenance, rows.len(), "unknown");
    columns.extend([
        string_values(rows.iter().map(|row| row.family.as_ref())),
        string_values(rows.iter().map(|row| row.reason_code.as_ref())),
        string_values(rows.iter().map(|row| row.detail.as_ref())),
        Arc::new(BooleanArray::from_iter(
            rows.iter().map(|row| Some(row.bounded)),
        )),
        optional_string_values(rows.iter().map(|row| row.input_relation.as_deref())),
    ]);
    RecordBatch::try_new(
        RustMirDerivedRelation::Unknown.schema_with_bindings(bindings),
        columns,
    )
}

fn row_count(batches: &[RecordBatch]) -> usize {
    batches.iter().map(RecordBatch::num_rows).sum()
}

fn bound(family: &'static str, actual: usize, maximum: usize) -> Result<(), RustMirAnalysisError> {
    if actual > maximum {
        return Err(RustMirAnalysisError::ResourceBound {
            family,
            actual,
            maximum,
        });
    }
    Ok(())
}

fn strings<'a>(
    batch: &'a RecordBatch,
    column: &'static str,
) -> Result<&'a StringArray, RustMirAnalysisError> {
    batch
        .column_by_name(column)
        .and_then(|array| array.as_any().downcast_ref::<StringArray>())
        .ok_or(RustMirAnalysisError::InvalidRawColumn { column })
}

fn u64s<'a>(
    batch: &'a RecordBatch,
    column: &'static str,
) -> Result<&'a UInt64Array, RustMirAnalysisError> {
    batch
        .column_by_name(column)
        .and_then(|array| array.as_any().downcast_ref::<UInt64Array>())
        .ok_or(RustMirAnalysisError::InvalidRawColumn { column })
}

fn bools<'a>(
    batch: &'a RecordBatch,
    column: &'static str,
) -> Result<&'a BooleanArray, RustMirAnalysisError> {
    batch
        .column_by_name(column)
        .and_then(|array| array.as_any().downcast_ref::<BooleanArray>())
        .ok_or(RustMirAnalysisError::InvalidRawColumn { column })
}

fn fixed<'a>(
    batch: &'a RecordBatch,
    column: &'static str,
    width: i32,
) -> Result<&'a FixedSizeBinaryArray, RustMirAnalysisError> {
    batch
        .column_by_name(column)
        .and_then(|array| array.as_any().downcast_ref::<FixedSizeBinaryArray>())
        .filter(|array| array.value_length() == width)
        .ok_or(RustMirAnalysisError::InvalidRawColumn { column })
}

fn optional_u64(array: &UInt64Array, row: usize) -> Option<u64> {
    (!array.is_null(row)).then(|| array.value(row))
}

fn optional_bool(array: &BooleanArray, row: usize) -> Option<bool> {
    (!array.is_null(row)).then(|| array.value(row))
}

fn optional_string(array: &StringArray, row: usize) -> Option<Arc<str>> {
    (!array.is_null(row)).then(|| Arc::from(array.value(row)))
}

fn optional_fixed32(array: &FixedSizeBinaryArray, row: usize) -> Option<[u8; 32]> {
    (!array.is_null(row)).then(|| array32(array.value(row)))
}

fn array32(value: &[u8]) -> [u8; 32] {
    value
        .try_into()
        .expect("schema validation proved a fixed-size binary[32]")
}

fn optional_number(value: Option<u64>) -> String {
    value.map_or_else(|| "-".to_owned(), |value| value.to_string())
}

fn slot_kind_rank(value: &str) -> u8 {
    match value {
        "statement" => 0,
        "terminator" => 1,
        _ => u8::MAX,
    }
}

fn hash_parts<'a>(domain: &[u8], parts: impl IntoIterator<Item = &'a [u8]>) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    for part in parts {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    *hasher.finalize().as_bytes()
}

fn hex_prefix(value: &[u8]) -> String {
    value
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn hex_full(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn fixed_repeat<const N: usize>(value: Option<&[u8; N]>, rows: usize) -> ArrayRef {
    let width = i32::try_from(N).expect("fixed identity width fits i32");
    let mut builder = FixedSizeBinaryBuilder::with_capacity(rows, width);
    for _ in 0..rows {
        if let Some(value) = value {
            builder
                .append_value(value)
                .expect("validated fixed-width output identity");
        } else {
            builder.append_null();
        }
    }
    Arc::new(builder.finish())
}

fn fixed_values<'a, const N: usize>(
    values: impl IntoIterator<Item = Option<&'a [u8; N]>>,
) -> ArrayRef {
    let iterator = values.into_iter();
    let (lower, _) = iterator.size_hint();
    let width = i32::try_from(N).expect("fixed identity width fits i32");
    let mut builder = FixedSizeBinaryBuilder::with_capacity(lower, width);
    for value in iterator {
        if let Some(value) = value {
            builder
                .append_value(value)
                .expect("validated fixed-width output identity");
        } else {
            builder.append_null();
        }
    }
    Arc::new(builder.finish())
}

fn u64_repeat(value: Option<u64>, rows: usize) -> ArrayRef {
    Arc::new(UInt64Array::from(
        std::iter::repeat_n(value, rows).collect::<Vec<_>>(),
    ))
}

fn u64_values(values: impl IntoIterator<Item = u64>) -> ArrayRef {
    Arc::new(UInt64Array::from_iter_values(values))
}

fn optional_u64_values(values: impl IntoIterator<Item = Option<u64>>) -> ArrayRef {
    Arc::new(UInt64Array::from(values.into_iter().collect::<Vec<_>>()))
}

fn bool_values(values: impl IntoIterator<Item = bool>) -> ArrayRef {
    Arc::new(BooleanArray::from_iter(values.into_iter().map(Some)))
}

fn optional_bool_values(values: impl IntoIterator<Item = Option<bool>>) -> ArrayRef {
    Arc::new(BooleanArray::from(values.into_iter().collect::<Vec<_>>()))
}

fn string_repeat(value: &str, rows: usize) -> ArrayRef {
    Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
        value, rows,
    )))
}

fn string_values<'a>(values: impl IntoIterator<Item = &'a str>) -> ArrayRef {
    Arc::new(StringArray::from_iter_values(values))
}

fn optional_string_values<'a>(values: impl IntoIterator<Item = Option<&'a str>>) -> ArrayRef {
    Arc::new(StringArray::from(values.into_iter().collect::<Vec<_>>()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    enum Cell {
        Utf8(Arc<str>),
        U64(u64),
        Bool(bool),
        Bytes(Vec<u8>),
    }

    type TestRow = BTreeMap<&'static str, Cell>;

    fn relation(value: &str) -> RelationId {
        RelationId::new(value).expect("valid test relation")
    }

    fn bindings(prefix: &str) -> RustMirAnalysisBindings {
        RustMirAnalysisBindings {
            relations: RustMirAnalysisRelations {
                cfg_edges: relation(&format!("{prefix}.cfg")),
                def_use: relation(&format!("{prefix}.def_use")),
                reaching_definitions: relation(&format!("{prefix}.reaching")),
                liveness: relation(&format!("{prefix}.liveness")),
                ownership_state: relation(&format!("{prefix}.ownership")),
                alias_points_to: relation(&format!("{prefix}.alias")),
                resource_lifecycle: relation(&format!("{prefix}.resource")),
                async_lowering: relation(&format!("{prefix}.async")),
                unsafe_ffi: relation(&format!("{prefix}.unsafe_ffi")),
                control_dependence_inputs: relation(&format!("{prefix}.control_input")),
                unknowns: relation(&format!("{prefix}.unknown")),
            },
            authority_class: ProviderAuthorityClass::RustApplicationDerived,
            private_enrichment: None,
        }
    }

    fn provenance() -> RustMirAnalysisProvenance {
        RustMirAnalysisProvenance {
            model_epoch_id: [1; 32],
            source_snapshot_pin: [2; 32],
            analysis_context_pin: [3; 32],
            source_generation: 7,
            provider_run_id: Arc::from("run-1"),
            compilation_unit_id: Arc::from("crate-1"),
            owner_id: Arc::from("owner-1"),
            source_file_id: Arc::from("src/lib.rs"),
            source_content_digest: [4; 32],
            stable_owner_key: Some(RustStableOwnerKey {
                stable_crate_id: 11,
                def_path_hash: [5; 16],
            }),
            rustc_release: Arc::from(RUSTC_PUBLIC_RELEASE),
            rustc_commit: Arc::from("8fa1c96cf"),
            rustc_toolchain: Arc::from(RUSTC_TOOLCHAIN),
            toolchain_identity_digest: [6; 32],
            raw_schema_bundle_digest: expected_rust_mir_input_schema_bundle_digest(),
        }
    }

    fn text(value: &str) -> Cell {
        Cell::Utf8(Arc::from(value))
    }

    fn bytes<const N: usize>(value: [u8; N]) -> Cell {
        Cell::Bytes(value.to_vec())
    }

    fn block(index: u64, statements: u64, terminator: &str, entry: bool) -> TestRow {
        BTreeMap::from([
            ("block_index", Cell::U64(index)),
            ("statement_count", Cell::U64(statements)),
            ("terminator_kind", text(terminator)),
            ("is_entry", Cell::Bool(entry)),
        ])
    }

    fn edge(source: u64, target: u64, kind: &str, unwind: Option<&str>) -> TestRow {
        let mut row = BTreeMap::from([
            ("source_block", Cell::U64(source)),
            ("target_block", Cell::U64(target)),
            ("edge_kind", text(kind)),
        ]);
        if let Some(unwind) = unwind {
            row.insert("unwind_action", text(unwind));
        }
        row
    }

    fn place(
        place_id: [u8; 32],
        block: u64,
        slot_kind: &str,
        slot_index: u64,
        ordinal: u64,
        base_local: u64,
    ) -> TestRow {
        BTreeMap::from([
            ("place_id", bytes(place_id)),
            ("block_index", Cell::U64(block)),
            ("slot_kind", text(slot_kind)),
            ("slot_index", Cell::U64(slot_index)),
            ("occurrence_role", text("test-place")),
            ("occurrence_ordinal", Cell::U64(ordinal)),
            ("base_local", Cell::U64(base_local)),
            ("projection_kind", text("BaseLocal")),
        ])
    }

    fn access(
        place_id: [u8; 32],
        block: u64,
        slot_kind: &str,
        slot_index: u64,
        ordinal: u64,
        kind: &str,
    ) -> TestRow {
        access_with_evidence(
            place_id,
            block,
            slot_kind,
            slot_index,
            ordinal,
            kind,
            "test-evidence",
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn access_with_evidence(
        place_id: [u8; 32],
        block: u64,
        slot_kind: &str,
        slot_index: u64,
        ordinal: u64,
        kind: &str,
        evidence: &str,
    ) -> TestRow {
        BTreeMap::from([
            ("block_index", Cell::U64(block)),
            ("slot_kind", text(slot_kind)),
            ("slot_index", Cell::U64(slot_index)),
            ("access_ordinal", Cell::U64(ordinal)),
            ("place_id", bytes(place_id)),
            ("access_kind", text(kind)),
            ("structured_evidence", text(evidence)),
            ("runtime_effect", Cell::Bool(true)),
        ])
    }

    fn raw(
        blocks: Vec<TestRow>,
        places: Vec<TestRow>,
        edges: Vec<TestRow>,
        accesses: Vec<TestRow>,
    ) -> RustMirRawRelations {
        RustMirRawRelations {
            blocks: vec![raw_batch(RustcRelation::MirBlock, blocks)],
            locals: vec![RecordBatch::new_empty(RustcRelation::MirLocal.schema())],
            places: vec![raw_batch(RustcRelation::MirPlace, places)],
            operands: vec![RecordBatch::new_empty(RustcRelation::MirOperand.schema())],
            rvalues: vec![RecordBatch::new_empty(RustcRelation::MirRvalue.schema())],
            statements: vec![RecordBatch::new_empty(RustcRelation::MirStatement.schema())],
            terminators: vec![RecordBatch::new_empty(
                RustcRelation::MirTerminator.schema(),
            )],
            cfg_edges: vec![raw_batch(RustcRelation::CfgEdge, edges)],
            calls: vec![RecordBatch::new_empty(RustcRelation::Call.schema())],
            instances: vec![RecordBatch::new_empty(RustcRelation::Instance.schema())],
            accesses: vec![raw_batch(RustcRelation::Access, accesses)],
            completeness: RustMirInputCompleteness::complete(),
            private_enrichment: None,
        }
    }

    fn raw_batch(relation: RustcRelation, rows: Vec<TestRow>) -> RecordBatch {
        if rows.is_empty() {
            return RecordBatch::new_empty(relation.schema());
        }
        let schema = relation.schema();
        let columns = schema
            .fields()
            .iter()
            .map(|field| match field.data_type() {
                DataType::Utf8 => {
                    let values = rows
                        .iter()
                        .map(
                            |row| match test_value(row, field.name(), field.is_nullable()) {
                                Some(Cell::Utf8(value)) => Some(value.to_string()),
                                None => None,
                                value => panic!("unexpected UTF-8 test cell {value:?}"),
                            },
                        )
                        .collect::<Vec<_>>();
                    Arc::new(StringArray::from(values)) as ArrayRef
                }
                DataType::UInt64 => {
                    let values = rows
                        .iter()
                        .map(
                            |row| match test_value(row, field.name(), field.is_nullable()) {
                                Some(Cell::U64(value)) => Some(value),
                                None => None,
                                value => panic!("unexpected UInt64 test cell {value:?}"),
                            },
                        )
                        .collect::<Vec<_>>();
                    Arc::new(UInt64Array::from(values)) as ArrayRef
                }
                DataType::Boolean => {
                    let values = rows
                        .iter()
                        .map(
                            |row| match test_value(row, field.name(), field.is_nullable()) {
                                Some(Cell::Bool(value)) => Some(value),
                                None => None,
                                value => panic!("unexpected Boolean test cell {value:?}"),
                            },
                        )
                        .collect::<Vec<_>>();
                    Arc::new(BooleanArray::from(values)) as ArrayRef
                }
                DataType::FixedSizeBinary(width) => {
                    let mut builder = FixedSizeBinaryBuilder::with_capacity(rows.len(), *width);
                    for row in &rows {
                        match test_value(row, field.name(), field.is_nullable()) {
                            Some(Cell::Bytes(value)) => builder.append_value(value).unwrap(),
                            None => builder.append_null(),
                            value => panic!("unexpected fixed-binary test cell {value:?}"),
                        }
                    }
                    Arc::new(builder.finish()) as ArrayRef
                }
                value => panic!("unsupported raw test type {value:?}"),
            })
            .collect::<Vec<_>>();
        RecordBatch::try_new(schema, columns).expect("test batch matches raw relation schema")
    }

    fn test_value(row: &TestRow, name: &str, nullable: bool) -> Option<Cell> {
        if let Some(value) = row.get(name) {
            return Some(value.clone());
        }
        let common = match name {
            "provider_run_id" => Some(text("run-1")),
            "compilation_unit_id" => Some(text("crate-1")),
            "owner_id" => Some(text("owner-1")),
            "source_generation" => Some(Cell::U64(7)),
            "source_file_id" => Some(text("src/lib.rs")),
            "source_content_digest" => Some(bytes([4; 32])),
            "stable_crate_id" => Some(Cell::U64(11)),
            "def_path_hash" => Some(bytes([5; 16])),
            _ => None,
        };
        common.or_else(|| {
            if nullable {
                None
            } else {
                panic!("test row omitted required raw field {name}")
            }
        })
    }

    #[test]
    fn straight_line_flow_materializes_cfg_def_use_reaching_and_liveness() {
        let definition_place = [10; 32];
        let use_place = [11; 32];
        let input = raw(
            vec![block(0, 1, "Goto", true), block(1, 1, "Return", false)],
            vec![
                place(definition_place, 0, "statement", 0, 0, 1),
                place(use_place, 1, "statement", 0, 0, 1),
            ],
            vec![edge(0, 1, "Normal", None)],
            vec![
                access(definition_place, 0, "statement", 0, 0, "Write"),
                access(use_place, 1, "statement", 0, 0, "Copy"),
            ],
        );
        let output =
            analyze_rust_mir_relations(&provenance(), &input, &bindings("model.rust")).unwrap();

        assert_eq!(
            output.relation(RustMirDerivedRelation::CfgEdge).num_rows(),
            1
        );
        assert_eq!(
            output.relation(RustMirDerivedRelation::DefUse).num_rows(),
            1
        );
        assert_eq!(
            output
                .relation(RustMirDerivedRelation::ReachingDefinition)
                .num_rows(),
            3
        );
        assert_eq!(
            output.relation(RustMirDerivedRelation::Liveness).num_rows(),
            2
        );
        assert!(
            output.relation(RustMirDerivedRelation::Unknown).num_rows() >= 2,
            "bounded precision remainders remain explicit"
        );
        assert!(output.observation().cfg_complete);
        assert!(output.observation().dataflow_complete);

        let liveness = output.relation(RustMirDerivedRelation::Liveness);
        let blocks = u64s(liveness, "block_index").unwrap();
        let boundaries = strings(liveness, "boundary").unwrap();
        let observed = (0..liveness.num_rows())
            .map(|row| (blocks.value(row), boundaries.value(row)))
            .collect::<BTreeSet<_>>();
        assert_eq!(observed, BTreeSet::from([(0, "EXIT"), (1, "ENTRY")]));
    }

    #[test]
    fn branch_merge_preserves_both_may_reaching_definitions() {
        let left = [20; 32];
        let right = [21; 32];
        let use_place = [22; 32];
        let input = raw(
            vec![
                block(0, 0, "SwitchInt", true),
                block(1, 1, "Goto", false),
                block(2, 1, "Goto", false),
                block(3, 1, "Return", false),
            ],
            vec![
                place(left, 1, "statement", 0, 0, 2),
                place(right, 2, "statement", 0, 0, 2),
                place(use_place, 3, "statement", 0, 0, 2),
            ],
            vec![
                edge(0, 1, "Case", None),
                edge(0, 2, "Default", None),
                edge(1, 3, "Normal", None),
                edge(2, 3, "Normal", None),
            ],
            vec![
                access(left, 1, "statement", 0, 0, "Write"),
                access(right, 2, "statement", 0, 0, "Write"),
                access(use_place, 3, "statement", 0, 0, "Copy"),
            ],
        );
        let output =
            analyze_rust_mir_relations(&provenance(), &input, &bindings("model.rust")).unwrap();
        assert_eq!(
            output.relation(RustMirDerivedRelation::DefUse).num_rows(),
            2
        );

        let reaching = output.relation(RustMirDerivedRelation::ReachingDefinition);
        let blocks = u64s(reaching, "block_index").unwrap();
        let boundaries = strings(reaching, "boundary").unwrap();
        let join_entry = (0..reaching.num_rows())
            .filter(|row| blocks.value(*row) == 3 && boundaries.value(*row) == "ENTRY")
            .count();
        assert_eq!(join_entry, 2);
    }

    #[test]
    fn unwind_cleanup_edge_participates_in_flow_fixed_points() {
        let definition_place = [30; 32];
        let cleanup_use = [31; 32];
        let input = raw(
            vec![
                block(0, 1, "Call", true),
                block(1, 0, "Return", false),
                block(2, 1, "Resume", false),
            ],
            vec![
                place(definition_place, 0, "statement", 0, 0, 3),
                place(cleanup_use, 2, "statement", 0, 0, 3),
            ],
            vec![
                edge(0, 1, "CallReturn", None),
                edge(0, 2, "Unwind", Some("Cleanup")),
            ],
            vec![
                access(definition_place, 0, "statement", 0, 0, "Write"),
                access(cleanup_use, 2, "statement", 0, 0, "Drop"),
            ],
        );
        let output =
            analyze_rust_mir_relations(&provenance(), &input, &bindings("model.rust")).unwrap();
        assert_eq!(
            output.relation(RustMirDerivedRelation::DefUse).num_rows(),
            1
        );

        let cfg = output.relation(RustMirDerivedRelation::CfgEdge);
        let kinds = strings(cfg, "edge_kind").unwrap();
        let unwinds = strings(cfg, "unwind_action").unwrap();
        let unwind_row = (0..cfg.num_rows())
            .find(|row| kinds.value(*row) == "Unwind")
            .expect("unwind edge remains explicit");
        assert_eq!(unwinds.value(unwind_row), "Cleanup");
    }

    #[test]
    fn call_destination_definition_is_excluded_from_unwind_successor() {
        let destination = [32; 32];
        let normal_use = [33; 32];
        let cleanup_use = [34; 32];
        let input = raw(
            vec![
                block(0, 0, "Call", true),
                block(1, 1, "Return", false),
                block(2, 1, "Resume", false),
            ],
            vec![
                place(destination, 0, "terminator", 0, 0, 6),
                place(normal_use, 1, "statement", 0, 0, 6),
                place(cleanup_use, 2, "statement", 0, 0, 6),
            ],
            vec![
                edge(0, 1, "CallReturn", None),
                edge(0, 2, "Unwind", Some("Cleanup")),
            ],
            vec![
                access_with_evidence(
                    destination,
                    0,
                    "terminator",
                    0,
                    0,
                    "Write",
                    "TerminatorKind::Call.destination",
                ),
                access(normal_use, 1, "statement", 0, 0, "Copy"),
                access(cleanup_use, 2, "statement", 0, 0, "Copy"),
            ],
        );
        let output =
            analyze_rust_mir_relations(&provenance(), &input, &bindings("model.rust")).unwrap();
        let def_use = output.relation(RustMirDerivedRelation::DefUse);
        assert_eq!(def_use.num_rows(), 1);
        let use_blocks = u64s(def_use, "use_block").unwrap();
        assert_eq!(use_blocks.value(0), 1);

        let unknown = output.relation(RustMirDerivedRelation::Unknown);
        let reasons = strings(unknown, "reason_code").unwrap();
        assert!(
            (0..unknown.num_rows())
                .any(|row| reasons.value(row) == "NO_REACHING_DEFINITION_WITNESS")
        );
        assert_eq!(
            output.observation().relation_completeness[&RustMirDerivedRelation::ReachingDefinition],
            RustMirAnalysisCompleteness::Partial
        );
    }

    #[test]
    fn declared_partial_input_emits_unknowns_instead_of_empty_complete_flow() {
        let mut input = raw(
            vec![block(0, 0, "Return", true)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        input.completeness.accesses = RustMirRelationCompleteness::Partial {
            reason: Arc::from("provider reported one unsupported access variant"),
        };
        let output =
            analyze_rust_mir_relations(&provenance(), &input, &bindings("model.rust")).unwrap();
        assert!(!output.observation().dataflow_complete);
        assert_eq!(
            output.observation().relation_completeness[&RustMirDerivedRelation::DefUse],
            RustMirAnalysisCompleteness::Partial
        );
        assert_eq!(
            output.relation(RustMirDerivedRelation::DefUse).num_rows(),
            0
        );
        let unknown = output.relation(RustMirDerivedRelation::Unknown);
        let reasons = strings(unknown, "reason_code").unwrap();
        assert!((0..unknown.num_rows()).any(|row| reasons.value(row) == "RAW_INPUT_NOT_COMPLETE"));
    }

    #[test]
    fn unclassified_access_downgrades_dataflow_and_preserves_explicit_cause() {
        let place_id = [40; 32];
        let input = raw(
            vec![block(0, 1, "Return", true)],
            vec![place(place_id, 0, "statement", 0, 0, 4)],
            Vec::new(),
            vec![access(
                place_id,
                0,
                "statement",
                0,
                0,
                "FuturePinnedNightlyVariant",
            )],
        );
        let output =
            analyze_rust_mir_relations(&provenance(), &input, &bindings("model.rust")).unwrap();
        assert!(!output.observation().dataflow_complete);
        let unknown = output.relation(RustMirDerivedRelation::Unknown);
        let reasons = strings(unknown, "reason_code").unwrap();
        assert!(
            (0..unknown.num_rows()).any(|row| reasons.value(row) == "UNCLASSIFIED_ACCESS_KIND")
        );
    }

    #[test]
    fn unavailable_input_is_unknown_never_partial_or_complete() {
        let mut input = raw(
            vec![block(0, 0, "Return", true)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        input.completeness.accesses = RustMirRelationCompleteness::Unavailable {
            reason: Arc::from("compiler run failed before the access relation closed"),
        };
        let output =
            analyze_rust_mir_relations(&provenance(), &input, &bindings("model.rust")).unwrap();
        for role in [
            RustMirDerivedRelation::DefUse,
            RustMirDerivedRelation::ReachingDefinition,
            RustMirDerivedRelation::Liveness,
        ] {
            assert_eq!(
                output.observation().relation_completeness[&role],
                RustMirAnalysisCompleteness::Unknown
            );
            assert_eq!(output.relation(role).num_rows(), 0);
        }
        let unknown = output.relation(RustMirDerivedRelation::Unknown);
        let completeness = strings(unknown, "analysis_completeness").unwrap();
        assert!((0..unknown.num_rows()).all(|row| completeness.value(row) == "unknown"));
    }

    #[test]
    fn same_slot_operand_use_precedes_destination_write() {
        let prior_definition = [60; 32];
        let destination_write = [61; 32];
        let operand_use = [62; 32];
        let input = raw(
            vec![block(0, 2, "Return", true)],
            vec![
                place(prior_definition, 0, "statement", 0, 0, 5),
                place(destination_write, 0, "statement", 1, 0, 5),
                place(operand_use, 0, "statement", 1, 0, 5),
            ],
            Vec::new(),
            vec![
                access(prior_definition, 0, "statement", 0, 0, "Write"),
                // Destination is intentionally lexicographically before the use. Raw occurrence
                // identity and role-local ordinal must not determine MIR evaluation order.
                access(destination_write, 0, "statement", 1, 0, "Write"),
                access(operand_use, 0, "statement", 1, 0, "Copy"),
            ],
        );
        let output =
            analyze_rust_mir_relations(&provenance(), &input, &bindings("model.rust")).unwrap();
        let def_use = output.relation(RustMirDerivedRelation::DefUse);
        assert_eq!(def_use.num_rows(), 1);
        let definitions = fixed(def_use, "definition_place_id", 32).unwrap();
        let uses = fixed(def_use, "use_place_id", 32).unwrap();
        assert_eq!(definitions.value(0), prior_definition);
        assert_eq!(uses.value(0), operand_use);
        assert_ne!(definitions.value(0), destination_write);
    }

    #[test]
    fn full_projection_type_keys_participate_in_memory_location_identity() {
        let first_place = [50; 32];
        let second_place = [51; 32];
        let first_type = [7; 32];
        let mut second_type = first_type;
        second_type[31] = 8;
        let projected = |place_id, slot_index, type_key| {
            BTreeMap::from([
                ("place_id", bytes(place_id)),
                ("block_index", Cell::U64(0)),
                ("slot_kind", text("statement")),
                ("slot_index", Cell::U64(slot_index)),
                ("occurrence_role", text("opaque-cast")),
                ("occurrence_ordinal", Cell::U64(0)),
                ("base_local", Cell::U64(9)),
                ("projection_ordinal", Cell::U64(0)),
                ("projection_kind", text("OpaqueCast")),
                ("projection_type_key", bytes(type_key)),
            ])
        };
        let batch = raw_batch(
            RustcRelation::MirPlace,
            vec![
                projected(first_place, 0, first_type),
                projected(second_place, 1, second_type),
            ],
        );
        let locations = parse_places(&[batch], &provenance()).unwrap();
        let first = &locations[&first_place].location;
        let second = &locations[&second_place].location;

        assert_eq!(&first_type[..8], &second_type[..8]);
        assert_ne!(first.id, second.id);
        assert_ne!(first.projection_path, second.projection_path);
        assert!(first.projection_path.contains(&hex_full(&first_type)));
        assert!(second.projection_path.contains(&hex_full(&second_type)));
    }

    #[test]
    fn model_bindings_select_relation_metadata_without_changing_algorithms() {
        let input = raw(
            vec![block(0, 0, "Return", true)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let first_bindings = bindings("model.one");
        let second_bindings = bindings("model.two");
        let first = analyze_rust_mir_relations(&provenance(), &input, &first_bindings).unwrap();
        let second = analyze_rust_mir_relations(&provenance(), &input, &second_bindings).unwrap();

        for role in RustMirDerivedRelation::ALL {
            let first_batch = first.relation(role);
            let second_batch = second.relation(role);
            assert_eq!(first_batch.num_rows(), second_batch.num_rows());
            assert_eq!(
                first_batch
                    .schema()
                    .metadata()
                    .get("codefabric.relation_id"),
                Some(&first_bindings.relation_id(role).as_str().to_owned())
            );
            assert_eq!(
                second_batch
                    .schema()
                    .metadata()
                    .get("codefabric.relation_id"),
                Some(&second_bindings.relation_id(role).as_str().to_owned())
            );
            assert_ne!(
                first_batch
                    .schema()
                    .metadata()
                    .get("codefabric.relation_id"),
                second_batch
                    .schema()
                    .metadata()
                    .get("codefabric.relation_id")
            );
        }
        assert_eq!(
            first.observation().output_rows,
            second.observation().output_rows
        );
    }

    #[test]
    fn application_authority_is_separate_from_compiler_provenance() {
        let input = raw(
            vec![block(0, 0, "Return", true)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let output =
            analyze_rust_mir_relations(&provenance(), &input, &bindings("model.rust")).unwrap();
        assert_eq!(
            output.observation().authority_class,
            ProviderAuthorityClass::RustApplicationDerived
        );
        for relation in RustMirDerivedRelation::ALL {
            let batch = output.relation(relation);
            assert_eq!(
                batch.schema().metadata().get("codefabric.authority"),
                Some(&RUST_MIR_DERIVED_AUTHORITY.to_owned())
            );
            let authorities = strings(batch, "authority").unwrap();
            assert!((0..batch.num_rows()).all(|row| {
                authorities.value(row) == RUST_MIR_DERIVED_AUTHORITY
                    && !authorities.value(row).contains("rustc_public")
                    && !authorities.value(row).contains("rustc_private")
            }));
            let compiler_releases = strings(batch, "rustc_release").unwrap();
            assert!(
                (0..batch.num_rows())
                    .all(|row| compiler_releases.value(row) == RUSTC_PUBLIC_RELEASE)
            );
        }

        let mut forbidden = bindings("model.forbidden");
        forbidden.authority_class = ProviderAuthorityClass::ProviderNative;
        let error = analyze_rust_mir_relations(&provenance(), &input, &forbidden).unwrap_err();
        assert!(matches!(error, RustMirAnalysisError::InvalidBinding(_)));
    }
}
