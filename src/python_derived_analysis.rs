//! Application-owned Python CFG, dataflow, memory, effect, and suspension analyses.
//!
//! Provider adapters contribute accepted structure and semantic evidence. This module owns the
//! derived meaning. Relational staging, joins, windows, unions, and deterministic sorts remain
//! visible as DataFusion plans; owner-local fixed points use bounded Rust only where recursion is
//! irreducible. Dynamic or degraded evidence remains queryable as typed unknowns and conservative
//! candidates, never as an empty result that implies absence.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use arrow_array::builder::{FixedSizeBinaryBuilder, StringBuilder};
use arrow_array::{ArrayRef, BooleanArray, RecordBatch, StringArray, UInt32Array, UInt64Array};
use arrow_schema::{ArrowError, DataType, Field, Schema, SchemaRef};
use datafusion::common::{Column, TableReference};
use datafusion::datasource::{MemTable, provider_as_source};
use datafusion::execution::context::SessionContext;
use datafusion::functions_window::expr_fn::row_number;
use datafusion::logical_expr::{Expr, ExprFunctionExt, JoinType, LogicalPlan, LogicalPlanBuilder};
use datafusion::prelude::{col, lit};
use thiserror::Error;

use crate::provider_admission::ProviderAuthorityClass;
use crate::relational_program::RelationId;

/// Exact release of this intentionally bounded first application analysis slice.
pub const PYTHON_OWNER_FLOW_ALGORITHM_RELEASE: &str = "codefabric.python-owner-flow.datafusion.v2";
/// Precision contract for this public provider-evidence/application-analysis boundary.
pub const PYTHON_OWNER_FLOW_PRECISION_RELEASE: &str =
    "python-explicit-cfg-owner-may-flow-conservative-memory-effects.v2";
pub const PYTHON_DERIVED_AUTHORITY: &str = "application.python-derived-analysis";

const NODE_SOURCE_ALIAS: &str = "__codefabric_python_node_source";
const NODE_TARGET_ALIAS: &str = "__codefabric_python_node_target";
const DEFINITIONS_ALIAS: &str = "__codefabric_python_definitions";
const USES_ALIAS: &str = "__codefabric_python_uses";
const CANDIDATES_ALIAS: &str = "__codefabric_python_reaching_candidates";
const REACHING_RANK: &str = "__codefabric_python_reaching_rank";
const MAX_NODES: usize = 65_536;
const MAX_EDGES: usize = 262_144;
const MAX_EVENTS: usize = 262_144;
const MAX_MEMORY_LOCATIONS: usize = 262_144;
const MAX_EFFECTS: usize = 262_144;
const MAX_SUSPENSIONS: usize = 65_536;
const MAX_INVALIDATED_OWNERS: usize = 262_144;

/// Exact immutable inputs repeated on every derived output row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonFlowProvenance {
    pub model_epoch_id: [u8; 32],
    pub source_pin: [u8; 32],
    pub analysis_context_id: [u8; 32],
    pub source_generation: u64,
    pub owner_id: [u8; 16],
    pub ruff_provider_run_id: Option<[u8; 16]>,
    pub ruff_provider_release: Arc<str>,
    pub pyrefly_provider_run_id: Option<Arc<str>>,
    pub pyrefly_provider_release: Option<Arc<str>>,
}

/// One application-owned CFG node seed after accepted Ruff structure has been normalized.
///
/// `ordinal` is an owner-local evaluation-order coordinate, never a provider identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonCfgNodeSeed {
    pub ordinal: u32,
    pub kind: Arc<str>,
    pub start_byte: Option<u64>,
    pub end_byte: Option<u64>,
}

/// One application-owned non-sequential CFG successor established by the bounded Python walker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonCfgEdgeSeed {
    pub source_ordinal: u32,
    pub target_ordinal: u32,
    pub kind: Arc<str>,
    /// Branch/return/loop routing replaces the implicit next edge; exception summaries may coexist.
    pub suppresses_sequential_edge: bool,
}

/// Closed role used by the first owner-local reaching-definition slice.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonFlowEventRole {
    Definition,
    Use,
}

/// One canonical definition/use event derived from accepted Ruff binding/reference facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonFlowEventSeed {
    pub node_ordinal: u32,
    pub event_ordinal: u32,
    pub location_id: [u8; 16],
    pub role: PythonFlowEventRole,
}

/// Completeness of one accepted evidence family for this owner and source pin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PythonInputCompleteness {
    Complete,
    Partial { reason: Arc<str> },
    Unavailable { reason: Arc<str> },
}

impl PythonInputCompleteness {
    fn reason(&self) -> Option<&str> {
        match self {
            Self::Complete => None,
            Self::Partial { reason } | Self::Unavailable { reason } => Some(reason),
        }
    }
}

/// Causal accepted-input completeness, separate for each derived family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonOwnerInputCompleteness {
    pub ruff_structure: PythonInputCompleteness,
    pub bindings_and_references: PythonInputCompleteness,
    pub memory_evidence: PythonInputCompleteness,
    pub effect_evidence: PythonInputCompleteness,
    pub async_evidence: PythonInputCompleteness,
    pub reverse_importers: PythonInputCompleteness,
}

impl PythonOwnerInputCompleteness {
    #[must_use]
    pub const fn complete() -> Self {
        Self {
            ruff_structure: PythonInputCompleteness::Complete,
            bindings_and_references: PythonInputCompleteness::Complete,
            memory_evidence: PythonInputCompleteness::Complete,
            effect_evidence: PythonInputCompleteness::Complete,
            async_evidence: PythonInputCompleteness::Complete,
            reverse_importers: PythonInputCompleteness::Complete,
        }
    }
}

/// Application memory-location role. IDs are application-owned and source-pinned.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonMemoryLocationKind {
    Local,
    HeapObject,
    Attribute,
    Subscript,
}

impl PythonMemoryLocationKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Local => "LOCAL",
            Self::HeapObject => "HEAP_OBJECT",
            Self::Attribute => "ATTRIBUTE",
            Self::Subscript => "SUBSCRIPT",
        }
    }
}

/// Conservative memory abstraction derived from accepted Ruff syntax plus Pyrefly evidence.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PythonMemoryLocationSeed {
    pub location_id: [u8; 16],
    pub kind: PythonMemoryLocationKind,
    pub base_location_id: Option<[u8; 16]>,
    pub selector: Option<Arc<str>>,
    pub selector_dynamic: bool,
    pub allocation_node_ordinal: Option<u32>,
}

/// Mechanically observed effect; it is evidence, never a risk or refactoring judgment.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonEffectKind {
    Read,
    Write,
    Call,
    Raise,
    Return,
    Acquire,
    Release,
    Escape,
}

impl PythonEffectKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Read => "READ",
            Self::Write => "WRITE",
            Self::Call => "CALL",
            Self::Raise => "RAISE",
            Self::Return => "RETURN",
            Self::Acquire => "ACQUIRE",
            Self::Release => "RELEASE",
            Self::Escape => "ESCAPE",
        }
    }

    const fn is_resource_lifecycle(self) -> bool {
        matches!(self, Self::Acquire | Self::Release | Self::Escape)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PythonEffectSeed {
    pub node_ordinal: u32,
    pub effect_ordinal: u32,
    pub kind: PythonEffectKind,
    pub subject_location_id: Option<[u8; 16]>,
    pub resource_kind: Option<Arc<str>>,
    pub evidence: Arc<str>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonSuspensionKind {
    Await,
    Yield,
    YieldFrom,
    AsyncFor,
    AsyncWith,
}

impl PythonSuspensionKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Await => "AWAIT",
            Self::Yield => "YIELD",
            Self::YieldFrom => "YIELD_FROM",
            Self::AsyncFor => "ASYNC_FOR",
            Self::AsyncWith => "ASYNC_WITH",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PythonSuspensionSeed {
    pub node_ordinal: u32,
    pub suspension_ordinal: u32,
    pub kind: PythonSuspensionKind,
    pub resume_node_ordinal: Option<u32>,
    pub exceptional_resume_node_ordinal: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonInvalidationSeed {
    pub changed_owners: Vec<[u8; 16]>,
    pub pyrefly_affected_owners: Vec<[u8; 16]>,
    pub reverse_importer_owners: Vec<[u8; 16]>,
}

/// Accepted semantic-input state. Availability identifies the exact current Pyrefly run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PyreflySemanticEvidence {
    Available { provider_run_id: Arc<str> },
    Unknown { reason: Arc<str> },
}

/// Complete input to the bounded owner-local slice.
#[derive(Clone, Debug)]
pub struct PythonOwnerFlowInput {
    pub provenance: PythonFlowProvenance,
    pub nodes: Vec<PythonCfgNodeSeed>,
    pub explicit_edges: Vec<PythonCfgEdgeSeed>,
    pub events: Vec<PythonFlowEventSeed>,
    pub memory_locations: Vec<PythonMemoryLocationSeed>,
    pub effects: Vec<PythonEffectSeed>,
    pub suspensions: Vec<PythonSuspensionSeed>,
    pub invalidation: PythonInvalidationSeed,
    pub completeness: PythonOwnerInputCompleteness,
    pub pyrefly: PyreflySemanticEvidence,
}

/// Model-selected relation identities. Runtime code never dispatches on these strings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonFlowRelations {
    pub cfg_nodes: RelationId,
    pub cfg_edges: RelationId,
    pub evaluation_order: RelationId,
    pub def_use: RelationId,
    pub reaching_definitions: RelationId,
    pub liveness: RelationId,
    pub value_flow: RelationId,
    pub memory_locations: RelationId,
    pub alias_points_to: RelationId,
    pub effects: RelationId,
    pub resource_lifecycle: RelationId,
    pub async_suspension: RelationId,
    pub invalidations: RelationId,
    pub unknowns: RelationId,
}

/// Static output roles. Model bindings own every semantic relation identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonDerivedRelation {
    CfgNode,
    CfgEdge,
    EvaluationOrder,
    DefUse,
    ReachingDefinition,
    Liveness,
    ValueFlow,
    MemoryLocation,
    AliasPointsTo,
    Effect,
    ResourceLifecycle,
    AsyncSuspension,
    Invalidation,
    Unknown,
}

impl PythonDerivedRelation {
    pub const ALL: [Self; 14] = [
        Self::CfgNode,
        Self::CfgEdge,
        Self::EvaluationOrder,
        Self::DefUse,
        Self::ReachingDefinition,
        Self::Liveness,
        Self::ValueFlow,
        Self::MemoryLocation,
        Self::AliasPointsTo,
        Self::Effect,
        Self::ResourceLifecycle,
        Self::AsyncSuspension,
        Self::Invalidation,
        Self::Unknown,
    ];
}

/// Model-selected physical names for every semantic field consumed or emitted by this slice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonFlowFields {
    pub model_epoch_id: Arc<str>,
    pub source_pin: Arc<str>,
    pub analysis_context_id: Arc<str>,
    pub source_generation: Arc<str>,
    pub owner_id: Arc<str>,
    pub ruff_provider_run_id: Arc<str>,
    pub ruff_provider_release: Arc<str>,
    pub pyrefly_provider_run_id: Arc<str>,
    pub pyrefly_provider_release: Arc<str>,
    pub algorithm_release: Arc<str>,
    pub precision_release: Arc<str>,
    pub authority: Arc<str>,
    pub analysis_completeness: Arc<str>,
    pub node_id: Arc<str>,
    pub node_ordinal: Arc<str>,
    pub node_kind: Arc<str>,
    pub start_byte: Arc<str>,
    pub end_byte: Arc<str>,
    pub next_node_id: Arc<str>,
    pub next_edge_id: Arc<str>,
    pub next_enabled: Arc<str>,
    pub edge_id: Arc<str>,
    pub source_node_id: Arc<str>,
    pub target_node_id: Arc<str>,
    pub edge_kind: Arc<str>,
    pub event_id: Arc<str>,
    pub event_ordinal: Arc<str>,
    pub event_role: Arc<str>,
    pub location_id: Arc<str>,
    pub definition_event_id: Arc<str>,
    pub use_event_id: Arc<str>,
    pub relation_kind: Arc<str>,
    pub boundary: Arc<str>,
    pub predecessor_id: Arc<str>,
    pub successor_id: Arc<str>,
    pub memory_kind: Arc<str>,
    pub base_location_id: Arc<str>,
    pub selector: Arc<str>,
    pub selector_dynamic: Arc<str>,
    pub allocation_node_id: Arc<str>,
    pub alias_source_location_id: Arc<str>,
    pub alias_target_location_id: Arc<str>,
    pub evidence: Arc<str>,
    pub effect_id: Arc<str>,
    pub effect_ordinal: Arc<str>,
    pub effect_kind: Arc<str>,
    pub subject_location_id: Arc<str>,
    pub resource_kind: Arc<str>,
    pub suspension_id: Arc<str>,
    pub suspension_ordinal: Arc<str>,
    pub suspension_kind: Arc<str>,
    pub resume_node_id: Arc<str>,
    pub exceptional_resume_node_id: Arc<str>,
    pub invalidated_owner_id: Arc<str>,
    pub invalidation_reason: Arc<str>,
    pub bounded: Arc<str>,
    pub unknown_family: Arc<str>,
    pub unknown_reason: Arc<str>,
    pub unknown_detail: Arc<str>,
}

impl PythonFlowFields {
    /// Validate bounded unique names before they participate in schema or expression binding.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversized, control-containing, or duplicate physical field names.
    pub fn validate(&self) -> Result<(), PythonDerivedAnalysisError> {
        let names = self.all_names();
        let mut unique = BTreeSet::new();
        for name in names {
            if name.is_empty()
                || name.len() > 240
                || name.trim() != name.as_ref()
                || name.chars().any(char::is_control)
            {
                return Err(PythonDerivedAnalysisError::InvalidBinding(
                    name.as_ref().to_owned(),
                ));
            }
            if !unique.insert(name.as_ref()) {
                return Err(PythonDerivedAnalysisError::DuplicateBinding(
                    name.as_ref().to_owned(),
                ));
            }
        }
        Ok(())
    }

    fn all_names(&self) -> [&Arc<str>; 59] {
        [
            &self.model_epoch_id,
            &self.source_pin,
            &self.analysis_context_id,
            &self.source_generation,
            &self.owner_id,
            &self.ruff_provider_run_id,
            &self.ruff_provider_release,
            &self.pyrefly_provider_run_id,
            &self.pyrefly_provider_release,
            &self.algorithm_release,
            &self.precision_release,
            &self.authority,
            &self.analysis_completeness,
            &self.node_id,
            &self.node_ordinal,
            &self.node_kind,
            &self.start_byte,
            &self.end_byte,
            &self.next_node_id,
            &self.next_edge_id,
            &self.next_enabled,
            &self.edge_id,
            &self.source_node_id,
            &self.target_node_id,
            &self.edge_kind,
            &self.event_id,
            &self.event_ordinal,
            &self.event_role,
            &self.location_id,
            &self.definition_event_id,
            &self.use_event_id,
            &self.relation_kind,
            &self.boundary,
            &self.predecessor_id,
            &self.successor_id,
            &self.memory_kind,
            &self.base_location_id,
            &self.selector,
            &self.selector_dynamic,
            &self.allocation_node_id,
            &self.alias_source_location_id,
            &self.alias_target_location_id,
            &self.evidence,
            &self.effect_id,
            &self.effect_ordinal,
            &self.effect_kind,
            &self.subject_location_id,
            &self.resource_kind,
            &self.suspension_id,
            &self.suspension_ordinal,
            &self.suspension_kind,
            &self.resume_node_id,
            &self.exceptional_resume_node_id,
            &self.invalidated_owner_id,
            &self.invalidation_reason,
            &self.bounded,
            &self.unknown_family,
            &self.unknown_reason,
            &self.unknown_detail,
        ]
    }
}

/// Model-selected meanings whose spellings are data rather than Rust dispatch keys.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonFlowSemanticValues {
    pub sequential_edge: Arc<str>,
    pub definition_event: Arc<str>,
    pub use_event: Arc<str>,
    pub reaching_definition: Arc<str>,
    pub def_use: Arc<str>,
    pub value_flow: Arc<str>,
    pub may_alias: Arc<str>,
    pub may_point_to: Arc<str>,
    pub live_entry: Arc<str>,
    pub live_exit: Arc<str>,
}

/// Full model binding and application-owned authority selection for this compilation.
#[derive(Clone, Debug)]
pub struct PythonFlowBindings {
    pub relations: PythonFlowRelations,
    pub fields: PythonFlowFields,
    pub values: PythonFlowSemanticValues,
    pub cfg_authority: ProviderAuthorityClass,
    pub dataflow_authority: ProviderAuthorityClass,
    pub alias_authority: ProviderAuthorityClass,
    pub effect_authority: ProviderAuthorityClass,
    pub summary_authority: ProviderAuthorityClass,
}

impl PythonFlowBindings {
    #[must_use]
    pub fn relation_id(&self, role: PythonDerivedRelation) -> &RelationId {
        match role {
            PythonDerivedRelation::CfgNode => &self.relations.cfg_nodes,
            PythonDerivedRelation::CfgEdge => &self.relations.cfg_edges,
            PythonDerivedRelation::EvaluationOrder => &self.relations.evaluation_order,
            PythonDerivedRelation::DefUse => &self.relations.def_use,
            PythonDerivedRelation::ReachingDefinition => &self.relations.reaching_definitions,
            PythonDerivedRelation::Liveness => &self.relations.liveness,
            PythonDerivedRelation::ValueFlow => &self.relations.value_flow,
            PythonDerivedRelation::MemoryLocation => &self.relations.memory_locations,
            PythonDerivedRelation::AliasPointsTo => &self.relations.alias_points_to,
            PythonDerivedRelation::Effect => &self.relations.effects,
            PythonDerivedRelation::ResourceLifecycle => &self.relations.resource_lifecycle,
            PythonDerivedRelation::AsyncSuspension => &self.relations.async_suspension,
            PythonDerivedRelation::Invalidation => &self.relations.invalidations,
            PythonDerivedRelation::Unknown => &self.relations.unknowns,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PythonAnalysisCompleteness {
    Complete,
    Partial,
    Unknown,
}

impl PythonAnalysisCompleteness {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Unknown => "unknown",
        }
    }
}

/// Native operators visible in the compiled logical plans.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonFlowNativeOperator {
    DeterministicNodeSort,
    SequentialSuccessorJoin,
    ExplicitControlUnion,
    DefinitionUseJoin,
    LatestDefinitionWindow,
    LatestDefinitionFilter,
    BoundedOwnerFixedPoint,
    TypedArrowMaterialization,
    DeterministicOutputSort,
}

/// Exact compilation observation derived from the plans actually constructed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonFlowCompilationObservation {
    pub relations: PythonFlowRelations,
    pub operators: BTreeSet<PythonFlowNativeOperator>,
    pub algorithm_release: Arc<str>,
    pub precision_release: Arc<str>,
    pub reaching_definitions_complete: bool,
    pub relation_completeness: BTreeMap<PythonDerivedRelation, PythonAnalysisCompleteness>,
    pub invalidated_owners: BTreeSet<[u8; 16]>,
}

/// Four typed application-owned relations compiled from one immutable owner input.
#[derive(Clone, Debug)]
pub struct CompiledPythonOwnerFlow {
    cfg_nodes_plan: LogicalPlan,
    cfg_edges_plan: LogicalPlan,
    reaching_definitions_plan: LogicalPlan,
    unknowns: RecordBatch,
    derived_relations: BTreeMap<PythonDerivedRelation, RecordBatch>,
    observation: PythonFlowCompilationObservation,
}

impl CompiledPythonOwnerFlow {
    #[must_use]
    pub const fn cfg_nodes_plan(&self) -> &LogicalPlan {
        &self.cfg_nodes_plan
    }

    #[must_use]
    pub const fn cfg_edges_plan(&self) -> &LogicalPlan {
        &self.cfg_edges_plan
    }

    #[must_use]
    pub const fn reaching_definitions_plan(&self) -> &LogicalPlan {
        &self.reaching_definitions_plan
    }

    #[must_use]
    pub const fn unknowns(&self) -> &RecordBatch {
        &self.unknowns
    }

    #[must_use]
    pub fn derived_relation(&self, role: PythonDerivedRelation) -> Option<&RecordBatch> {
        self.derived_relations.get(&role)
    }

    #[must_use]
    pub const fn observation(&self) -> &PythonFlowCompilationObservation {
        &self.observation
    }

    /// Execute all three native plans in one caller-owned epoch session.
    ///
    /// # Errors
    ///
    /// Returns a DataFusion planning or execution failure.
    pub async fn execute(
        &self,
        context: &SessionContext,
    ) -> Result<PythonOwnerFlowExecution, PythonDerivedAnalysisError> {
        let cfg_nodes = context
            .execute_logical_plan(self.cfg_nodes_plan.clone())
            .await?
            .collect()
            .await?;
        let cfg_edges = context
            .execute_logical_plan(self.cfg_edges_plan.clone())
            .await?
            .collect()
            .await?;
        let reaching_definitions = context
            .execute_logical_plan(self.reaching_definitions_plan.clone())
            .await?
            .collect()
            .await?;
        Ok(PythonOwnerFlowExecution {
            cfg_nodes,
            cfg_edges,
            reaching_definitions,
            unknowns: self.unknowns.clone(),
            derived_relations: self.derived_relations.clone(),
            observation: self.observation.clone(),
        })
    }
}

/// Materialized Arrow outputs and the exact plan-compilation observation that produced them.
#[derive(Clone, Debug)]
pub struct PythonOwnerFlowExecution {
    pub cfg_nodes: Vec<RecordBatch>,
    pub cfg_edges: Vec<RecordBatch>,
    pub reaching_definitions: Vec<RecordBatch>,
    pub unknowns: RecordBatch,
    pub derived_relations: BTreeMap<PythonDerivedRelation, RecordBatch>,
    pub observation: PythonFlowCompilationObservation,
}

/// Closed failures for the Python derived-analysis boundary.
#[derive(Debug, Error)]
pub enum PythonDerivedAnalysisError {
    #[error("Python derived-analysis binding is invalid: {0}")]
    InvalidBinding(String),
    #[error("Python derived-analysis field binding is duplicated: {0}")]
    DuplicateBinding(String),
    #[error("Python derived analysis requires application CFG/dataflow authority")]
    ProviderAuthorityClaim,
    #[error("Python derived-analysis provenance is invalid: {0}")]
    InvalidProvenance(&'static str),
    #[error("Python derived-analysis input exceeds bound for {family}: {observed} > {limit}")]
    ResourceBound {
        family: &'static str,
        observed: usize,
        limit: usize,
    },
    #[error("Python owner input is contradictory: {0}")]
    InvalidInput(String),
    #[error(transparent)]
    Arrow(#[from] ArrowError),
    #[error(transparent)]
    DataFusion(#[from] datafusion::error::DataFusionError),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FlowLinkRow {
    edge_id: [u8; 16],
    definition_event_id: [u8; 16],
    use_event_id: [u8; 16],
    location_id: [u8; 16],
    source_node_id: [u8; 16],
    target_node_id: [u8; 16],
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LiveRow {
    node_id: [u8; 16],
    node_ordinal: u32,
    boundary: &'static str,
    location_id: [u8; 16],
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct EvaluationRow {
    edge_id: [u8; 16],
    predecessor_id: [u8; 16],
    successor_id: [u8; 16],
    source_node_id: [u8; 16],
    target_node_id: [u8; 16],
    relation_kind: &'static str,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AliasRow {
    edge_id: [u8; 16],
    source_location_id: [u8; 16],
    target_location_id: [u8; 16],
    relation_kind: Arc<str>,
    evidence: Arc<str>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct InvalidationRow {
    invalidated_owner_id: [u8; 16],
    reason: &'static str,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct UnknownRow {
    family: Arc<str>,
    reason: Arc<str>,
    detail: Arc<str>,
    bounded: bool,
}

#[derive(Default)]
struct PythonFixedPointResult {
    links: Vec<FlowLinkRow>,
    liveness: Vec<LiveRow>,
    unknowns: BTreeSet<UnknownRow>,
}

/// Compile one owner-local CFG and a bounded reaching-definition slice to native DataFusion.
///
/// # Errors
///
/// Rejects provider-native output authority, invalid model bindings/provenance, resource-bound
/// violations, inconsistent ordinals/edges, or Arrow/DataFusion construction failures.
pub fn compile_python_owner_flow(
    input: PythonOwnerFlowInput,
    bindings: &PythonFlowBindings,
) -> Result<CompiledPythonOwnerFlow, PythonDerivedAnalysisError> {
    validate_bindings(bindings)?;
    validate_provenance(&input.provenance, &input.pyrefly)?;
    validate_input(&input)?;

    let mut nodes = input.nodes;
    nodes.sort_by_key(|node| node.ordinal);
    let mut memory_locations = input.memory_locations.clone();
    memory_locations.sort();
    let mut effects = input.effects.clone();
    effects.sort();
    let mut suspensions = input.suspensions.clone();
    suspensions.sort();
    let node_ids = nodes
        .iter()
        .map(|node| {
            (
                node.ordinal,
                derived_id(
                    b"cfg-node",
                    &input.provenance,
                    &[&node.ordinal.to_be_bytes(), node.kind.as_bytes()],
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let suppressed = input
        .explicit_edges
        .iter()
        .filter(|edge| edge.suppresses_sequential_edge)
        .map(|edge| edge.source_ordinal)
        .collect::<BTreeSet<_>>();

    let cfg_status = input_status(
        [&input.completeness.ruff_structure],
        input.provenance.ruff_provider_run_id.is_some(),
    );
    let mut dataflow_status = input_status(
        [
            &input.completeness.ruff_structure,
            &input.completeness.bindings_and_references,
        ],
        input.provenance.ruff_provider_run_id.is_some(),
    );
    let pyrefly_unknown = match &input.pyrefly {
        PyreflySemanticEvidence::Available { .. } => None,
        PyreflySemanticEvidence::Unknown { reason } => Some(reason.clone()),
    };
    if pyrefly_unknown.is_some() && dataflow_status == PythonAnalysisCompleteness::Complete {
        dataflow_status = PythonAnalysisCompleteness::Partial;
    }

    let node_batch = node_batch(
        &nodes,
        &node_ids,
        &suppressed,
        &input.provenance,
        bindings,
        cfg_status,
    )?;
    let explicit_edge_batch = explicit_edge_batch(
        &input.explicit_edges,
        &node_ids,
        &input.provenance,
        bindings,
        cfg_status,
    )?;
    let event_batch = event_batch(
        &input.events,
        &node_ids,
        &input.provenance,
        bindings,
        dataflow_status,
    )?;

    let node_scan = scan_batch("python_cfg_node_seed", node_batch)?;
    let explicit_edge_scan = scan_batch("python_cfg_explicit_edge_seed", explicit_edge_batch)?;
    let event_scan = scan_batch("python_flow_event_seed", event_batch)?;

    let cfg_nodes_plan = LogicalPlanBuilder::from(node_scan.clone())
        .project(cfg_node_output_columns(&bindings.fields))?
        .sort([col(bindings.fields.node_ordinal.as_ref()).sort(true, false)])?
        .build()?;
    let sequential = sequential_edge_plan(node_scan, bindings)?;
    let cfg_edges_plan = LogicalPlanBuilder::from(sequential)
        .union_distinct(explicit_edge_scan)?
        .sort([
            col(bindings.fields.source_node_id.as_ref()).sort(true, false),
            col(bindings.fields.target_node_id.as_ref()).sort(true, false),
            col(bindings.fields.edge_kind.as_ref()).sort(true, false),
        ])?
        .build()?;

    let nonlinear = input
        .explicit_edges
        .iter()
        .any(|edge| edge.kind.as_ref() != bindings.values.sequential_edge.as_ref());
    let mut fixed_point = if dataflow_status == PythonAnalysisCompleteness::Unknown {
        PythonFixedPointResult::default()
    } else {
        execute_owner_fixed_points(
            &nodes,
            &input.explicit_edges,
            &input.events,
            &node_ids,
            &input.provenance,
            bindings,
        )
    };
    if !fixed_point.unknowns.is_empty() && dataflow_status == PythonAnalysisCompleteness::Complete {
        dataflow_status = PythonAnalysisCompleteness::Partial;
    }
    let reaching_definitions_plan =
        if dataflow_status == PythonAnalysisCompleteness::Complete && !nonlinear {
            reaching_definitions_plan(event_scan, bindings)?
        } else {
            scan_batch(
                "python_fixed_point_reaching_definition",
                flow_link_batch(
                    &fixed_point.links,
                    &input.provenance,
                    bindings,
                    PythonDerivedRelation::ReachingDefinition,
                    bindings.values.reaching_definition.as_ref(),
                    dataflow_status,
                )?,
            )?
        };

    let mut unknown_rows = BTreeSet::new();
    append_input_unknowns(&mut unknown_rows, &input.completeness);
    unknown_rows.append(&mut fixed_point.unknowns);
    if input.provenance.ruff_provider_run_id.is_none() {
        unknown_rows.insert(UnknownRow {
            family: Arc::from("python.cfg"),
            reason: Arc::from("RUFF_STRUCTURE_UNAVAILABLE"),
            detail: Arc::from("accepted Ruff structural evidence is absent"),
            bounded: true,
        });
    }
    if let Some(reason) = pyrefly_unknown.as_deref() {
        unknown_rows.insert(UnknownRow {
            family: Arc::from("python.semantic_enrichment"),
            reason: Arc::from("PYREFLY_SEMANTIC_UNAVAILABLE"),
            detail: Arc::from(reason),
            bounded: true,
        });
    }

    let evaluation_rows = derive_evaluation_rows(
        &nodes,
        &input.events,
        &node_ids,
        &input.provenance,
        bindings,
    );
    let memory_status = input_status(
        [
            &input.completeness.ruff_structure,
            &input.completeness.memory_evidence,
        ],
        input.provenance.ruff_provider_run_id.is_some(),
    );
    let mut alias_status = memory_status;
    if pyrefly_unknown.is_some() && alias_status == PythonAnalysisCompleteness::Complete {
        alias_status = PythonAnalysisCompleteness::Partial;
    }
    let (alias_rows, alias_unknowns) = if alias_status == PythonAnalysisCompleteness::Unknown {
        (Vec::new(), BTreeSet::new())
    } else {
        derive_alias_rows(&memory_locations, &input.provenance, bindings)
    };
    if !alias_unknowns.is_empty() && alias_status == PythonAnalysisCompleteness::Complete {
        alias_status = PythonAnalysisCompleteness::Partial;
    }
    unknown_rows.extend(alias_unknowns);
    let effect_status = input_status(
        [
            &input.completeness.ruff_structure,
            &input.completeness.effect_evidence,
        ],
        input.provenance.ruff_provider_run_id.is_some(),
    );
    let mut resource_status = effect_status;
    let resource_rows = if effect_status == PythonAnalysisCompleteness::Unknown {
        Vec::new()
    } else {
        effects
            .iter()
            .filter(|effect| effect.kind.is_resource_lifecycle())
            .cloned()
            .collect::<Vec<_>>()
    };
    for effect in &resource_rows {
        if effect.subject_location_id.is_none() || effect.resource_kind.is_none() {
            if resource_status == PythonAnalysisCompleteness::Complete {
                resource_status = PythonAnalysisCompleteness::Partial;
            }
            unknown_rows.insert(UnknownRow {
                family: Arc::from("python.resource_lifecycle"),
                reason: Arc::from("RESOURCE_IDENTITY_OR_KIND_UNRESOLVED"),
                detail: Arc::from(format!(
                    "{} effect {} lacks a resource identity or semantic kind",
                    effect.kind.label(),
                    effect.effect_ordinal
                )),
                bounded: true,
            });
        }
    }

    let mut async_status = input_status(
        [
            &input.completeness.ruff_structure,
            &input.completeness.async_evidence,
        ],
        input.provenance.ruff_provider_run_id.is_some(),
    );
    if suspensions.iter().any(|row| {
        row.resume_node_ordinal.is_none() && row.exceptional_resume_node_ordinal.is_none()
    }) {
        if async_status == PythonAnalysisCompleteness::Complete {
            async_status = PythonAnalysisCompleteness::Partial;
        }
        unknown_rows.insert(UnknownRow {
            family: Arc::from("python.async_suspension"),
            reason: Arc::from("RESUME_TARGET_UNRESOLVED"),
            detail: Arc::from(
                "accepted await/yield evidence lacks both normal and exceptional resume targets",
            ),
            bounded: true,
        });
    }

    let invalidated_owners = derive_python_invalidation_closure(&input.invalidation)?;
    let invalidation_status = input_status([&input.completeness.reverse_importers], true);
    let mut invalidation_rows = Vec::new();
    for owner in &input.invalidation.changed_owners {
        invalidation_rows.push(InvalidationRow {
            invalidated_owner_id: *owner,
            reason: "SOURCE_OWNER_CHANGED",
        });
    }
    for owner in &input.invalidation.pyrefly_affected_owners {
        invalidation_rows.push(InvalidationRow {
            invalidated_owner_id: *owner,
            reason: "PYREFLY_AFFECTED_MODULE",
        });
    }
    for owner in &input.invalidation.reverse_importer_owners {
        invalidation_rows.push(InvalidationRow {
            invalidated_owner_id: *owner,
            reason: "REVERSE_IMPORTER",
        });
    }
    invalidation_rows.sort();
    invalidation_rows.dedup();

    let mut derived_relations = BTreeMap::new();
    derived_relations.insert(
        PythonDerivedRelation::EvaluationOrder,
        evaluation_batch(&evaluation_rows, &input.provenance, bindings, cfg_status)?,
    );
    for (role, relation_kind) in [
        (
            PythonDerivedRelation::DefUse,
            bindings.values.def_use.as_ref(),
        ),
        (
            PythonDerivedRelation::ValueFlow,
            bindings.values.value_flow.as_ref(),
        ),
    ] {
        derived_relations.insert(
            role,
            flow_link_batch(
                &fixed_point.links,
                &input.provenance,
                bindings,
                role,
                relation_kind,
                dataflow_status,
            )?,
        );
    }
    derived_relations.insert(
        PythonDerivedRelation::Liveness,
        liveness_batch(
            &fixed_point.liveness,
            &input.provenance,
            bindings,
            dataflow_status,
        )?,
    );
    derived_relations.insert(
        PythonDerivedRelation::MemoryLocation,
        memory_batch(
            &memory_locations,
            &node_ids,
            &input.provenance,
            bindings,
            memory_status,
        )?,
    );
    derived_relations.insert(
        PythonDerivedRelation::AliasPointsTo,
        alias_batch(&alias_rows, &input.provenance, bindings, alias_status)?,
    );
    derived_relations.insert(
        PythonDerivedRelation::Effect,
        effect_batch(
            &effects,
            &node_ids,
            &input.provenance,
            bindings,
            PythonDerivedRelation::Effect,
            effect_status,
        )?,
    );
    derived_relations.insert(
        PythonDerivedRelation::ResourceLifecycle,
        effect_batch(
            &resource_rows,
            &node_ids,
            &input.provenance,
            bindings,
            PythonDerivedRelation::ResourceLifecycle,
            resource_status,
        )?,
    );
    derived_relations.insert(
        PythonDerivedRelation::AsyncSuspension,
        suspension_batch(
            &suspensions,
            &node_ids,
            &input.provenance,
            bindings,
            async_status,
        )?,
    );
    derived_relations.insert(
        PythonDerivedRelation::Invalidation,
        invalidation_batch(
            &invalidation_rows,
            &input.provenance,
            bindings,
            invalidation_status,
        )?,
    );
    let unknown_rows = unknown_rows.into_iter().collect::<Vec<_>>();
    let unknowns = unknown_batch(&unknown_rows, &input.provenance, bindings)?;
    derived_relations.insert(PythonDerivedRelation::Unknown, unknowns.clone());

    let mut operators = BTreeSet::from([
        PythonFlowNativeOperator::DeterministicNodeSort,
        PythonFlowNativeOperator::SequentialSuccessorJoin,
        PythonFlowNativeOperator::ExplicitControlUnion,
        PythonFlowNativeOperator::BoundedOwnerFixedPoint,
        PythonFlowNativeOperator::TypedArrowMaterialization,
        PythonFlowNativeOperator::DeterministicOutputSort,
    ]);
    if dataflow_status == PythonAnalysisCompleteness::Complete && !nonlinear {
        operators.extend([
            PythonFlowNativeOperator::DefinitionUseJoin,
            PythonFlowNativeOperator::LatestDefinitionWindow,
            PythonFlowNativeOperator::LatestDefinitionFilter,
        ]);
    }
    let relation_completeness = BTreeMap::from([
        (PythonDerivedRelation::CfgNode, cfg_status),
        (PythonDerivedRelation::CfgEdge, cfg_status),
        (PythonDerivedRelation::EvaluationOrder, cfg_status),
        (PythonDerivedRelation::DefUse, dataflow_status),
        (PythonDerivedRelation::ReachingDefinition, dataflow_status),
        (PythonDerivedRelation::Liveness, dataflow_status),
        (PythonDerivedRelation::ValueFlow, dataflow_status),
        (PythonDerivedRelation::MemoryLocation, memory_status),
        (PythonDerivedRelation::AliasPointsTo, alias_status),
        (PythonDerivedRelation::Effect, effect_status),
        (PythonDerivedRelation::ResourceLifecycle, resource_status),
        (PythonDerivedRelation::AsyncSuspension, async_status),
        (PythonDerivedRelation::Invalidation, invalidation_status),
        (
            PythonDerivedRelation::Unknown,
            PythonAnalysisCompleteness::Unknown,
        ),
    ]);
    Ok(CompiledPythonOwnerFlow {
        cfg_nodes_plan,
        cfg_edges_plan,
        reaching_definitions_plan,
        unknowns,
        derived_relations,
        observation: PythonFlowCompilationObservation {
            relations: bindings.relations.clone(),
            operators,
            algorithm_release: Arc::from(PYTHON_OWNER_FLOW_ALGORITHM_RELEASE),
            precision_release: Arc::from(PYTHON_OWNER_FLOW_PRECISION_RELEASE),
            reaching_definitions_complete: dataflow_status == PythonAnalysisCompleteness::Complete,
            relation_completeness,
            invalidated_owners,
        },
    })
}

fn validate_bindings(bindings: &PythonFlowBindings) -> Result<(), PythonDerivedAnalysisError> {
    bindings.fields.validate()?;
    if bindings.cfg_authority != ProviderAuthorityClass::PythonCfg
        || bindings.dataflow_authority != ProviderAuthorityClass::PythonDataflow
        || bindings.alias_authority != ProviderAuthorityClass::PythonAlias
        || bindings.effect_authority != ProviderAuthorityClass::PythonEffect
        || bindings.summary_authority != ProviderAuthorityClass::PythonSummary
    {
        return Err(PythonDerivedAnalysisError::ProviderAuthorityClaim);
    }
    let relation_ids = PythonDerivedRelation::ALL
        .into_iter()
        .map(|role| bindings.relation_id(role).as_str())
        .collect::<BTreeSet<_>>();
    if relation_ids.len() != PythonDerivedRelation::ALL.len() {
        return Err(PythonDerivedAnalysisError::InvalidBinding(
            "Python derived output roles require distinct relation identities".to_owned(),
        ));
    }
    for value in [
        &bindings.values.sequential_edge,
        &bindings.values.definition_event,
        &bindings.values.use_event,
        &bindings.values.reaching_definition,
        &bindings.values.def_use,
        &bindings.values.value_flow,
        &bindings.values.may_alias,
        &bindings.values.may_point_to,
        &bindings.values.live_entry,
        &bindings.values.live_exit,
    ] {
        if value.is_empty() || value.len() > 240 || value.chars().any(char::is_control) {
            return Err(PythonDerivedAnalysisError::InvalidBinding(
                value.as_ref().to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_provenance(
    provenance: &PythonFlowProvenance,
    pyrefly: &PyreflySemanticEvidence,
) -> Result<(), PythonDerivedAnalysisError> {
    if provenance.model_epoch_id == [0; 32]
        || provenance.source_pin == [0; 32]
        || provenance.analysis_context_id == [0; 32]
        || provenance.owner_id == [0; 16]
        || provenance.ruff_provider_run_id == Some([0; 16])
    {
        return Err(PythonDerivedAnalysisError::InvalidProvenance(
            "zero identity sentinel",
        ));
    }
    if provenance.ruff_provider_release.is_empty()
        || provenance.ruff_provider_release.len() > 1_024
        || provenance
            .ruff_provider_release
            .chars()
            .any(char::is_control)
        || provenance
            .pyrefly_provider_release
            .as_ref()
            .is_some_and(|release| {
                release.is_empty() || release.len() > 1_024 || release.chars().any(char::is_control)
            })
    {
        return Err(PythonDerivedAnalysisError::InvalidProvenance(
            "provider release pins are absent or unbounded",
        ));
    }
    match pyrefly {
        PyreflySemanticEvidence::Available { provider_run_id } => {
            if provider_run_id.is_empty()
                || provider_run_id.len() > 1_024
                || provider_run_id.chars().any(char::is_control)
                || provenance.pyrefly_provider_run_id.as_deref() != Some(provider_run_id.as_ref())
                || provenance.pyrefly_provider_release.is_none()
            {
                return Err(PythonDerivedAnalysisError::InvalidProvenance(
                    "Pyrefly evidence and provider-run pin differ",
                ));
            }
        }
        PyreflySemanticEvidence::Unknown { reason }
            if reason.is_empty()
                || reason.len() > 2_048
                || reason.chars().any(char::is_control)
                || provenance.pyrefly_provider_run_id.is_some() =>
        {
            return Err(PythonDerivedAnalysisError::InvalidProvenance(
                "Pyrefly unknown evidence retains a run pin or has an invalid reason",
            ));
        }
        PyreflySemanticEvidence::Unknown { .. } => {}
    }
    Ok(())
}

fn validate_input(input: &PythonOwnerFlowInput) -> Result<(), PythonDerivedAnalysisError> {
    for (family, observed, limit) in [
        ("cfg nodes", input.nodes.len(), MAX_NODES),
        ("cfg edges", input.explicit_edges.len(), MAX_EDGES),
        ("flow events", input.events.len(), MAX_EVENTS),
        (
            "memory locations",
            input.memory_locations.len(),
            MAX_MEMORY_LOCATIONS,
        ),
        ("effects", input.effects.len(), MAX_EFFECTS),
        (
            "async suspensions",
            input.suspensions.len(),
            MAX_SUSPENSIONS,
        ),
        (
            "invalidated owners",
            input.invalidation.changed_owners.len()
                + input.invalidation.pyrefly_affected_owners.len()
                + input.invalidation.reverse_importer_owners.len(),
            MAX_INVALIDATED_OWNERS,
        ),
    ] {
        if observed > limit {
            return Err(PythonDerivedAnalysisError::ResourceBound {
                family,
                observed,
                limit,
            });
        }
    }
    match input.provenance.ruff_provider_run_id {
        Some(_) if input.nodes.is_empty() => {
            return Err(PythonDerivedAnalysisError::InvalidInput(
                "available Ruff structure has no CFG node/schema carrier".into(),
            ));
        }
        None if !input.nodes.is_empty()
            || !input.explicit_edges.is_empty()
            || !input.events.is_empty()
            || !input.memory_locations.is_empty()
            || !input.effects.is_empty()
            || !input.suspensions.is_empty() =>
        {
            return Err(PythonDerivedAnalysisError::InvalidInput(
                "un-pinned Ruff structure or flow events cannot be analyzed".into(),
            ));
        }
        _ => {}
    }
    let mut ordinals = BTreeSet::new();
    for node in &input.nodes {
        if !ordinals.insert(node.ordinal) {
            return Err(PythonDerivedAnalysisError::InvalidInput(format!(
                "duplicate CFG node ordinal {}",
                node.ordinal
            )));
        }
        if node.kind.is_empty()
            || node
                .start_byte
                .zip(node.end_byte)
                .is_some_and(|(start, end)| start > end)
        {
            return Err(PythonDerivedAnalysisError::InvalidInput(
                "invalid CFG node kind or source range".into(),
            ));
        }
    }
    let mut edge_keys = BTreeSet::new();
    for edge in &input.explicit_edges {
        if !ordinals.contains(&edge.source_ordinal)
            || !ordinals.contains(&edge.target_ordinal)
            || edge.kind.is_empty()
            || !edge_keys.insert((edge.source_ordinal, edge.target_ordinal, edge.kind.clone()))
        {
            return Err(PythonDerivedAnalysisError::InvalidInput(
                "CFG edge is unbound, empty, or duplicated".into(),
            ));
        }
    }
    let mut event_ordinals = BTreeSet::new();
    for event in &input.events {
        if !ordinals.contains(&event.node_ordinal)
            || event.location_id == [0; 16]
            || !event_ordinals.insert(event.event_ordinal)
        {
            return Err(PythonDerivedAnalysisError::InvalidInput(
                "flow event is unbound, zero, or does not have a unique owner-local ordinal".into(),
            ));
        }
    }
    let mut location_ids = BTreeSet::new();
    for location in &input.memory_locations {
        if location.location_id == [0; 16]
            || !location_ids.insert(location.location_id)
            || location.base_location_id == Some(location.location_id)
            || location.selector.as_ref().is_some_and(|selector| {
                selector.is_empty()
                    || selector.len() > 2_048
                    || selector.chars().any(char::is_control)
            })
            || location
                .allocation_node_ordinal
                .is_some_and(|ordinal| !ordinals.contains(&ordinal))
        {
            return Err(PythonDerivedAnalysisError::InvalidInput(
                "memory location is zero, duplicated, self-based, unbounded, or unbound".into(),
            ));
        }
        match location.kind {
            PythonMemoryLocationKind::Local | PythonMemoryLocationKind::HeapObject
                if location.base_location_id.is_some() || location.selector.is_some() =>
            {
                return Err(PythonDerivedAnalysisError::InvalidInput(
                    "local/heap locations cannot carry attribute or subscript selectors".into(),
                ));
            }
            PythonMemoryLocationKind::Attribute | PythonMemoryLocationKind::Subscript
                if location.base_location_id.is_none() =>
            {
                return Err(PythonDerivedAnalysisError::InvalidInput(
                    "attribute/subscript location requires a base location".into(),
                ));
            }
            _ => {}
        }
    }
    for location in &input.memory_locations {
        if location
            .base_location_id
            .is_some_and(|base| !location_ids.contains(&base))
        {
            return Err(PythonDerivedAnalysisError::InvalidInput(
                "memory projection base is not present in the accepted owner locations".into(),
            ));
        }
    }
    for event in &input.events {
        if !input.memory_locations.is_empty() && !location_ids.contains(&event.location_id) {
            return Err(PythonDerivedAnalysisError::InvalidInput(
                "flow event location is absent from the accepted memory relation".into(),
            ));
        }
    }
    let mut effect_ordinals = BTreeSet::new();
    for effect in &input.effects {
        if !ordinals.contains(&effect.node_ordinal)
            || !effect_ordinals.insert(effect.effect_ordinal)
            || effect.evidence.is_empty()
            || effect.evidence.len() > 2_048
            || effect.evidence.chars().any(char::is_control)
            || effect
                .subject_location_id
                .is_some_and(|location| !location_ids.contains(&location))
            || effect.resource_kind.as_ref().is_some_and(|kind| {
                kind.is_empty() || kind.len() > 1_024 || kind.chars().any(char::is_control)
            })
        {
            return Err(PythonDerivedAnalysisError::InvalidInput(
                "effect is unbound, duplicated, or contains unbounded evidence".into(),
            ));
        }
    }
    let mut suspension_ordinals = BTreeSet::new();
    for suspension in &input.suspensions {
        if !ordinals.contains(&suspension.node_ordinal)
            || !suspension_ordinals.insert(suspension.suspension_ordinal)
            || suspension
                .resume_node_ordinal
                .is_some_and(|ordinal| !ordinals.contains(&ordinal))
            || suspension
                .exceptional_resume_node_ordinal
                .is_some_and(|ordinal| !ordinals.contains(&ordinal))
        {
            return Err(PythonDerivedAnalysisError::InvalidInput(
                "async suspension is unbound or duplicated".into(),
            ));
        }
    }
    for (family, state) in [
        ("ruff_structure", &input.completeness.ruff_structure),
        (
            "bindings_and_references",
            &input.completeness.bindings_and_references,
        ),
        ("memory_evidence", &input.completeness.memory_evidence),
        ("effect_evidence", &input.completeness.effect_evidence),
        ("async_evidence", &input.completeness.async_evidence),
        ("reverse_importers", &input.completeness.reverse_importers),
    ] {
        if let Some(reason) = state.reason()
            && (reason.is_empty() || reason.len() > 2_048 || reason.chars().any(char::is_control))
        {
            return Err(PythonDerivedAnalysisError::InvalidInput(format!(
                "{family} completeness reason is unbounded"
            )));
        }
    }
    for (unavailable, populated, family) in [
        (
            matches!(
                &input.completeness.bindings_and_references,
                PythonInputCompleteness::Unavailable { .. }
            ),
            !input.events.is_empty(),
            "bindings_and_references",
        ),
        (
            matches!(
                &input.completeness.memory_evidence,
                PythonInputCompleteness::Unavailable { .. }
            ),
            !input.memory_locations.is_empty(),
            "memory_evidence",
        ),
        (
            matches!(
                &input.completeness.effect_evidence,
                PythonInputCompleteness::Unavailable { .. }
            ),
            !input.effects.is_empty(),
            "effect_evidence",
        ),
        (
            matches!(
                &input.completeness.async_evidence,
                PythonInputCompleteness::Unavailable { .. }
            ),
            !input.suspensions.is_empty(),
            "async_evidence",
        ),
    ] {
        if unavailable && populated {
            return Err(PythonDerivedAnalysisError::InvalidInput(format!(
                "unavailable {family} cannot carry accepted rows"
            )));
        }
    }
    Ok(())
}

fn input_status<const N: usize>(
    states: [&PythonInputCompleteness; N],
    required_identity_available: bool,
) -> PythonAnalysisCompleteness {
    if !required_identity_available
        || states
            .iter()
            .any(|state| matches!(state, PythonInputCompleteness::Unavailable { .. }))
    {
        PythonAnalysisCompleteness::Unknown
    } else if states
        .iter()
        .any(|state| matches!(state, PythonInputCompleteness::Partial { .. }))
    {
        PythonAnalysisCompleteness::Partial
    } else {
        PythonAnalysisCompleteness::Complete
    }
}

fn flow_event_id(
    event: &PythonFlowEventSeed,
    provenance: &PythonFlowProvenance,
    bindings: &PythonFlowBindings,
) -> [u8; 16] {
    let role = match event.role {
        PythonFlowEventRole::Definition => bindings.values.definition_event.as_bytes(),
        PythonFlowEventRole::Use => bindings.values.use_event.as_bytes(),
    };
    derived_id(
        b"flow-event",
        provenance,
        &[
            &event.location_id,
            &event.node_ordinal.to_be_bytes(),
            &event.event_ordinal.to_be_bytes(),
            role,
        ],
    )
}

fn owner_cfg_pairs(
    nodes: &[PythonCfgNodeSeed],
    edges: &[PythonCfgEdgeSeed],
) -> BTreeSet<(u32, u32)> {
    let suppressed = edges
        .iter()
        .filter(|edge| edge.suppresses_sequential_edge)
        .map(|edge| edge.source_ordinal)
        .collect::<BTreeSet<_>>();
    let mut pairs = edges
        .iter()
        .map(|edge| (edge.source_ordinal, edge.target_ordinal))
        .collect::<BTreeSet<_>>();
    for adjacent in nodes.windows(2) {
        if !suppressed.contains(&adjacent[0].ordinal) {
            pairs.insert((adjacent[0].ordinal, adjacent[1].ordinal));
        }
    }
    pairs
}

fn execute_owner_fixed_points(
    nodes: &[PythonCfgNodeSeed],
    edges: &[PythonCfgEdgeSeed],
    events: &[PythonFlowEventSeed],
    node_ids: &BTreeMap<u32, [u8; 16]>,
    provenance: &PythonFlowProvenance,
    bindings: &PythonFlowBindings,
) -> PythonFixedPointResult {
    type DefinitionState = BTreeMap<[u8; 16], BTreeSet<[u8; 16]>>;
    type LiveState = BTreeSet<[u8; 16]>;

    let pairs = owner_cfg_pairs(nodes, edges);
    let mut predecessors = BTreeMap::<u32, Vec<u32>>::new();
    let mut successors = BTreeMap::<u32, Vec<u32>>::new();
    for (source, target) in pairs {
        predecessors.entry(target).or_default().push(source);
        successors.entry(source).or_default().push(target);
    }
    let mut by_node = BTreeMap::<u32, Vec<&PythonFlowEventSeed>>::new();
    for event in events {
        by_node.entry(event.node_ordinal).or_default().push(event);
    }
    for rows in by_node.values_mut() {
        rows.sort_by_key(|event| event.event_ordinal);
    }

    let mut entry = nodes
        .iter()
        .map(|node| (node.ordinal, DefinitionState::new()))
        .collect::<BTreeMap<_, _>>();
    let mut exit = entry.clone();
    let mut changed = true;
    while changed {
        changed = false;
        for node in nodes {
            let mut next_entry = DefinitionState::new();
            for predecessor in predecessors.get(&node.ordinal).into_iter().flatten() {
                for (location, definitions) in &exit[predecessor] {
                    next_entry.entry(*location).or_default().extend(definitions);
                }
            }
            let mut next_exit = next_entry.clone();
            for event in by_node.get(&node.ordinal).into_iter().flatten() {
                if event.role == PythonFlowEventRole::Definition {
                    next_exit.insert(
                        event.location_id,
                        BTreeSet::from([flow_event_id(event, provenance, bindings)]),
                    );
                }
            }
            if entry[&node.ordinal] != next_entry {
                entry.insert(node.ordinal, next_entry);
                changed = true;
            }
            if exit[&node.ordinal] != next_exit {
                exit.insert(node.ordinal, next_exit);
                changed = true;
            }
        }
    }

    let mut result = PythonFixedPointResult::default();
    for node in nodes {
        let mut state = entry[&node.ordinal].clone();
        for event in by_node.get(&node.ordinal).into_iter().flatten() {
            let event_id = flow_event_id(event, provenance, bindings);
            match event.role {
                PythonFlowEventRole::Use => {
                    let definitions = state.get(&event.location_id);
                    if definitions.is_none_or(BTreeSet::is_empty) {
                        result.unknowns.insert(UnknownRow {
                            family: Arc::from("python.def_use"),
                            reason: Arc::from("NO_REACHING_DEFINITION_WITNESS"),
                            detail: Arc::from(format!(
                                "use event {} has no owner-local reaching definition",
                                event.event_ordinal
                            )),
                            bounded: true,
                        });
                    }
                    for definition in definitions.into_iter().flatten() {
                        let definition_seed = events.iter().find(|candidate| {
                            flow_event_id(candidate, provenance, bindings) == *definition
                        });
                        let source_node_id = definition_seed
                            .map(|seed| node_ids[&seed.node_ordinal])
                            .unwrap_or(node_ids[&node.ordinal]);
                        result.links.push(FlowLinkRow {
                            edge_id: derived_id(
                                b"def-use-edge",
                                provenance,
                                &[definition, &event_id, &event.location_id],
                            ),
                            definition_event_id: *definition,
                            use_event_id: event_id,
                            location_id: event.location_id,
                            source_node_id,
                            target_node_id: node_ids[&node.ordinal],
                        });
                    }
                }
                PythonFlowEventRole::Definition => {
                    state.insert(event.location_id, BTreeSet::from([event_id]));
                }
            }
        }
    }
    result.links.sort();
    result.links.dedup();

    let mut live_entry = nodes
        .iter()
        .map(|node| (node.ordinal, LiveState::new()))
        .collect::<BTreeMap<_, _>>();
    let mut live_exit = live_entry.clone();
    let mut changed = true;
    while changed {
        changed = false;
        for node in nodes.iter().rev() {
            let mut next_exit = LiveState::new();
            for successor in successors.get(&node.ordinal).into_iter().flatten() {
                next_exit.extend(&live_entry[successor]);
            }
            let mut next_entry = next_exit.clone();
            for event in by_node.get(&node.ordinal).into_iter().flatten().rev() {
                match event.role {
                    PythonFlowEventRole::Definition => {
                        next_entry.remove(&event.location_id);
                    }
                    PythonFlowEventRole::Use => {
                        next_entry.insert(event.location_id);
                    }
                }
            }
            if live_exit[&node.ordinal] != next_exit {
                live_exit.insert(node.ordinal, next_exit);
                changed = true;
            }
            if live_entry[&node.ordinal] != next_entry {
                live_entry.insert(node.ordinal, next_entry);
                changed = true;
            }
        }
    }
    for node in nodes {
        for (boundary, live) in [
            ("ENTRY", &live_entry[&node.ordinal]),
            ("EXIT", &live_exit[&node.ordinal]),
        ] {
            for location in live {
                result.liveness.push(LiveRow {
                    node_id: node_ids[&node.ordinal],
                    node_ordinal: node.ordinal,
                    boundary,
                    location_id: *location,
                });
            }
        }
    }
    result.liveness.sort();
    result
}

fn derive_evaluation_rows(
    nodes: &[PythonCfgNodeSeed],
    events: &[PythonFlowEventSeed],
    node_ids: &BTreeMap<u32, [u8; 16]>,
    provenance: &PythonFlowProvenance,
    bindings: &PythonFlowBindings,
) -> Vec<EvaluationRow> {
    let mut rows = Vec::new();
    for adjacent in nodes.windows(2) {
        let source = node_ids[&adjacent[0].ordinal];
        let target = node_ids[&adjacent[1].ordinal];
        rows.push(EvaluationRow {
            edge_id: derived_id(b"node-evaluation-order", provenance, &[&source, &target]),
            predecessor_id: source,
            successor_id: target,
            source_node_id: source,
            target_node_id: target,
            relation_kind: "NODE_EVALUATES_BEFORE",
        });
    }
    let mut ordered_events = events.iter().collect::<Vec<_>>();
    ordered_events.sort_by_key(|event| event.event_ordinal);
    for adjacent in ordered_events.windows(2) {
        let predecessor = flow_event_id(adjacent[0], provenance, bindings);
        let successor = flow_event_id(adjacent[1], provenance, bindings);
        rows.push(EvaluationRow {
            edge_id: derived_id(
                b"event-evaluation-order",
                provenance,
                &[&predecessor, &successor],
            ),
            predecessor_id: predecessor,
            successor_id: successor,
            source_node_id: node_ids[&adjacent[0].node_ordinal],
            target_node_id: node_ids[&adjacent[1].node_ordinal],
            relation_kind: "EVENT_EVALUATES_BEFORE",
        });
    }
    rows.sort();
    rows
}

fn derive_alias_rows(
    locations: &[PythonMemoryLocationSeed],
    provenance: &PythonFlowProvenance,
    bindings: &PythonFlowBindings,
) -> (Vec<AliasRow>, BTreeSet<UnknownRow>) {
    let mut rows = Vec::new();
    let mut unknowns = BTreeSet::new();
    for location in locations {
        if let Some(base) = location.base_location_id {
            rows.push(AliasRow {
                edge_id: derived_id(
                    b"python-may-point-to",
                    provenance,
                    &[&base, &location.location_id],
                ),
                source_location_id: base,
                target_location_id: location.location_id,
                relation_kind: Arc::clone(&bindings.values.may_point_to),
                evidence: Arc::from(format!(
                    "{} projection{}",
                    location.kind.label(),
                    if location.selector_dynamic {
                        ";dynamic-selector"
                    } else {
                        ""
                    }
                )),
            });
        }
        if location.selector_dynamic {
            unknowns.insert(UnknownRow {
                family: Arc::from("python.alias_points_to"),
                reason: Arc::from("DYNAMIC_SELECTOR_MULTI_CANDIDATE"),
                detail: Arc::from(
                    "dynamic attribute/subscript selection prevents exact target identity",
                ),
                bounded: true,
            });
        }
    }
    for (index, left) in locations.iter().enumerate() {
        for right in locations.iter().skip(index + 1) {
            if left.kind != right.kind
                || left.base_location_id.is_none()
                || left.base_location_id != right.base_location_id
            {
                continue;
            }
            let may_alias = left.selector_dynamic
                || right.selector_dynamic
                || left.selector.is_some() && left.selector == right.selector;
            if !may_alias {
                continue;
            }
            rows.push(AliasRow {
                edge_id: derived_id(
                    b"python-may-alias",
                    provenance,
                    &[&left.location_id, &right.location_id],
                ),
                source_location_id: left.location_id,
                target_location_id: right.location_id,
                relation_kind: Arc::clone(&bindings.values.may_alias),
                evidence: Arc::from(if left.selector_dynamic || right.selector_dynamic {
                    "shared base with at least one dynamic selector"
                } else {
                    "shared base and equal normalized selector"
                }),
            });
        }
    }
    rows.sort();
    rows.dedup();
    (rows, unknowns)
}

/// Deterministic invalidation closure from changed owners and accepted Pyrefly/reverse-importer
/// evidence. The same set is used for incremental replacement and clean-recompute comparison.
///
/// # Errors
///
/// Rejects zero owner identities or an unbounded invalidation relation.
pub fn derive_python_invalidation_closure(
    seed: &PythonInvalidationSeed,
) -> Result<BTreeSet<[u8; 16]>, PythonDerivedAnalysisError> {
    let observed = seed.changed_owners.len()
        + seed.pyrefly_affected_owners.len()
        + seed.reverse_importer_owners.len();
    if observed > MAX_INVALIDATED_OWNERS {
        return Err(PythonDerivedAnalysisError::ResourceBound {
            family: "invalidated owners",
            observed,
            limit: MAX_INVALIDATED_OWNERS,
        });
    }
    let owners = seed
        .changed_owners
        .iter()
        .chain(&seed.pyrefly_affected_owners)
        .chain(&seed.reverse_importer_owners)
        .copied()
        .collect::<BTreeSet<_>>();
    if owners.contains(&[0; 16]) {
        return Err(PythonDerivedAnalysisError::InvalidInput(
            "invalidation relation contains the zero owner identity".to_owned(),
        ));
    }
    Ok(owners)
}

fn append_input_unknowns(
    unknowns: &mut BTreeSet<UnknownRow>,
    completeness: &PythonOwnerInputCompleteness,
) {
    for (input, state, families) in [
        (
            "ruff_structure",
            &completeness.ruff_structure,
            &[
                "python.cfg",
                "python.evaluation_order",
                "python.def_use",
                "python.reaching_definition",
                "python.liveness",
                "python.memory",
                "python.effect",
                "python.async_suspension",
            ][..],
        ),
        (
            "bindings_and_references",
            &completeness.bindings_and_references,
            &[
                "python.def_use",
                "python.reaching_definition",
                "python.liveness",
                "python.value_flow",
            ][..],
        ),
        (
            "memory_evidence",
            &completeness.memory_evidence,
            &["python.memory", "python.alias_points_to"][..],
        ),
        (
            "effect_evidence",
            &completeness.effect_evidence,
            &["python.effect", "python.resource_lifecycle"][..],
        ),
        (
            "async_evidence",
            &completeness.async_evidence,
            &["python.async_suspension"][..],
        ),
        (
            "reverse_importers",
            &completeness.reverse_importers,
            &["python.invalidation"][..],
        ),
    ] {
        if let Some(reason) = state.reason() {
            for family in families {
                unknowns.insert(UnknownRow {
                    family: Arc::from(*family),
                    reason: Arc::from("ACCEPTED_INPUT_NOT_COMPLETE"),
                    detail: Arc::from(format!("{input}: {reason}")),
                    bounded: true,
                });
            }
        }
    }
}

fn sequential_edge_plan(
    node_scan: LogicalPlan,
    bindings: &PythonFlowBindings,
) -> Result<LogicalPlan, PythonDerivedAnalysisError> {
    let fields = &bindings.fields;
    let source = LogicalPlanBuilder::from(node_scan.clone())
        .alias(NODE_SOURCE_ALIAS)?
        .filter(qualified(NODE_SOURCE_ALIAS, &fields.next_enabled).eq(lit(true)))?
        .build()?;
    let target = LogicalPlanBuilder::from(node_scan)
        .alias(NODE_TARGET_ALIAS)?
        .build()?;
    let joined = LogicalPlanBuilder::from(source)
        .join_on(
            target,
            JoinType::Inner,
            [
                qualified(NODE_SOURCE_ALIAS, &fields.owner_id)
                    .eq(qualified(NODE_TARGET_ALIAS, &fields.owner_id)),
                qualified(NODE_SOURCE_ALIAS, &fields.next_node_id)
                    .eq(qualified(NODE_TARGET_ALIAS, &fields.node_id)),
            ],
        )?
        .project([
            qualified(NODE_SOURCE_ALIAS, &fields.model_epoch_id),
            qualified(NODE_SOURCE_ALIAS, &fields.source_pin),
            qualified(NODE_SOURCE_ALIAS, &fields.analysis_context_id),
            qualified(NODE_SOURCE_ALIAS, &fields.source_generation),
            qualified(NODE_SOURCE_ALIAS, &fields.owner_id),
            qualified(NODE_SOURCE_ALIAS, &fields.ruff_provider_run_id),
            qualified(NODE_SOURCE_ALIAS, &fields.ruff_provider_release),
            qualified(NODE_SOURCE_ALIAS, &fields.pyrefly_provider_run_id),
            qualified(NODE_SOURCE_ALIAS, &fields.pyrefly_provider_release),
            qualified(NODE_SOURCE_ALIAS, &fields.algorithm_release),
            qualified(NODE_SOURCE_ALIAS, &fields.precision_release),
            qualified(NODE_SOURCE_ALIAS, &fields.authority),
            qualified(NODE_SOURCE_ALIAS, &fields.analysis_completeness),
            qualified(NODE_SOURCE_ALIAS, &fields.next_edge_id).alias(fields.edge_id.as_ref()),
            qualified(NODE_SOURCE_ALIAS, &fields.node_id).alias(fields.source_node_id.as_ref()),
            qualified(NODE_TARGET_ALIAS, &fields.node_id).alias(fields.target_node_id.as_ref()),
            lit(bindings.values.sequential_edge.as_ref()).alias(fields.edge_kind.as_ref()),
        ])?
        .build()?;
    Ok(joined)
}

fn reaching_definitions_plan(
    event_scan: LogicalPlan,
    bindings: &PythonFlowBindings,
) -> Result<LogicalPlan, PythonDerivedAnalysisError> {
    let fields = &bindings.fields;
    let definitions = LogicalPlanBuilder::from(event_scan.clone())
        .filter(col(fields.event_role.as_ref()).eq(lit(bindings.values.definition_event.as_ref())))?
        .alias(DEFINITIONS_ALIAS)?
        .build()?;
    let uses = LogicalPlanBuilder::from(event_scan)
        .filter(col(fields.event_role.as_ref()).eq(lit(bindings.values.use_event.as_ref())))?
        .alias(USES_ALIAS)?
        .build()?;
    let candidates = LogicalPlanBuilder::from(definitions)
        .join_on(
            uses,
            JoinType::Inner,
            [
                qualified(DEFINITIONS_ALIAS, &fields.owner_id)
                    .eq(qualified(USES_ALIAS, &fields.owner_id)),
                qualified(DEFINITIONS_ALIAS, &fields.location_id)
                    .eq(qualified(USES_ALIAS, &fields.location_id)),
                qualified(DEFINITIONS_ALIAS, &fields.event_ordinal)
                    .lt_eq(qualified(USES_ALIAS, &fields.event_ordinal)),
            ],
        )?
        .project([
            qualified(USES_ALIAS, &fields.model_epoch_id),
            qualified(USES_ALIAS, &fields.source_pin),
            qualified(USES_ALIAS, &fields.analysis_context_id),
            qualified(USES_ALIAS, &fields.source_generation),
            qualified(USES_ALIAS, &fields.owner_id),
            qualified(USES_ALIAS, &fields.ruff_provider_run_id),
            qualified(USES_ALIAS, &fields.ruff_provider_release),
            qualified(USES_ALIAS, &fields.pyrefly_provider_run_id),
            qualified(USES_ALIAS, &fields.pyrefly_provider_release),
            qualified(USES_ALIAS, &fields.algorithm_release),
            qualified(USES_ALIAS, &fields.precision_release),
            qualified(USES_ALIAS, &fields.authority),
            qualified(USES_ALIAS, &fields.analysis_completeness),
            qualified(USES_ALIAS, &fields.event_id).alias(fields.use_event_id.as_ref()),
            qualified(DEFINITIONS_ALIAS, &fields.event_id)
                .alias(fields.definition_event_id.as_ref()),
            qualified(USES_ALIAS, &fields.location_id),
            qualified(DEFINITIONS_ALIAS, &fields.node_id).alias(fields.source_node_id.as_ref()),
            qualified(USES_ALIAS, &fields.node_id).alias(fields.target_node_id.as_ref()),
            qualified(DEFINITIONS_ALIAS, &fields.event_ordinal),
        ])?
        .alias(CANDIDATES_ALIAS)?
        .build()?;
    let rank = row_number()
        .partition_by(vec![qualified(CANDIDATES_ALIAS, &fields.use_event_id)])
        .order_by(vec![
            qualified(CANDIDATES_ALIAS, &fields.event_ordinal).sort(false, false),
            qualified(CANDIDATES_ALIAS, &fields.definition_event_id).sort(true, false),
        ])
        .build()?
        .alias(REACHING_RANK);
    let ranked = LogicalPlanBuilder::from(candidates)
        .window([rank])?
        .build()?;
    let selected = LogicalPlanBuilder::from(ranked)
        .filter(col(REACHING_RANK).eq(lit(1_u64)))?
        .project([
            col(fields.model_epoch_id.as_ref()),
            col(fields.source_pin.as_ref()),
            col(fields.analysis_context_id.as_ref()),
            col(fields.source_generation.as_ref()),
            col(fields.owner_id.as_ref()),
            col(fields.ruff_provider_run_id.as_ref()),
            col(fields.ruff_provider_release.as_ref()),
            col(fields.pyrefly_provider_run_id.as_ref()),
            col(fields.pyrefly_provider_release.as_ref()),
            col(fields.algorithm_release.as_ref()),
            col(fields.precision_release.as_ref()),
            col(fields.authority.as_ref()),
            col(fields.analysis_completeness.as_ref()),
            col(fields.use_event_id.as_ref()).alias(fields.edge_id.as_ref()),
            col(fields.definition_event_id.as_ref()),
            col(fields.use_event_id.as_ref()),
            col(fields.location_id.as_ref()),
            col(fields.source_node_id.as_ref()),
            col(fields.target_node_id.as_ref()),
            lit(bindings.values.reaching_definition.as_ref()).alias(fields.relation_kind.as_ref()),
        ])?
        .sort([
            col(fields.use_event_id.as_ref()).sort(true, false),
            col(fields.definition_event_id.as_ref()).sort(true, false),
        ])?
        .build()?;
    Ok(selected)
}

fn scan_batch(name: &str, batch: RecordBatch) -> Result<LogicalPlan, PythonDerivedAnalysisError> {
    let schema = batch.schema();
    let provider = Arc::new(MemTable::try_new(schema, vec![vec![batch]])?);
    Ok(LogicalPlanBuilder::scan(name, provider_as_source(provider), None)?.build()?)
}

fn qualified(alias: &'static str, field: &Arc<str>) -> Expr {
    Expr::Column(Column::new(
        Some(TableReference::bare(alias)),
        field.as_ref().to_owned(),
    ))
}

fn derived_id(domain: &[u8], provenance: &PythonFlowProvenance, parts: &[&[u8]]) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new_derive_key("codefabric.python-derived-analysis.v2");
    hasher.update(domain);
    hasher.update(&provenance.model_epoch_id);
    hasher.update(&provenance.source_pin);
    hasher.update(&provenance.analysis_context_id);
    hasher.update(&provenance.owner_id);
    for part in parts {
        hasher.update(&part.len().to_be_bytes());
        hasher.update(part);
    }
    let mut id = [0_u8; 16];
    id.copy_from_slice(&hasher.finalize().as_bytes()[..16]);
    id
}

fn common_fields(fields: &PythonFlowFields) -> Vec<Field> {
    vec![
        Field::new(
            fields.model_epoch_id.as_ref(),
            DataType::FixedSizeBinary(32),
            false,
        ),
        Field::new(
            fields.source_pin.as_ref(),
            DataType::FixedSizeBinary(32),
            false,
        ),
        Field::new(
            fields.analysis_context_id.as_ref(),
            DataType::FixedSizeBinary(32),
            false,
        ),
        Field::new(fields.source_generation.as_ref(), DataType::UInt64, false),
        Field::new(
            fields.owner_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(
            fields.ruff_provider_run_id.as_ref(),
            DataType::FixedSizeBinary(16),
            true,
        ),
        Field::new(fields.ruff_provider_release.as_ref(), DataType::Utf8, false),
        Field::new(
            fields.pyrefly_provider_run_id.as_ref(),
            DataType::Utf8,
            true,
        ),
        Field::new(
            fields.pyrefly_provider_release.as_ref(),
            DataType::Utf8,
            true,
        ),
        Field::new(fields.algorithm_release.as_ref(), DataType::Utf8, false),
        Field::new(fields.precision_release.as_ref(), DataType::Utf8, false),
        Field::new(fields.authority.as_ref(), DataType::Utf8, false),
        Field::new(fields.analysis_completeness.as_ref(), DataType::Utf8, false),
    ]
}

fn common_columns(
    provenance: &PythonFlowProvenance,
    rows: usize,
    completeness: PythonAnalysisCompleteness,
) -> Vec<ArrayRef> {
    vec![
        fixed_repeat(Some(&provenance.model_epoch_id), rows),
        fixed_repeat(Some(&provenance.source_pin), rows),
        fixed_repeat(Some(&provenance.analysis_context_id), rows),
        Arc::new(UInt64Array::from_iter_values(std::iter::repeat_n(
            provenance.source_generation,
            rows,
        ))),
        fixed_repeat(Some(&provenance.owner_id), rows),
        fixed_repeat(provenance.ruff_provider_run_id.as_ref(), rows),
        string_repeat(provenance.ruff_provider_release.as_ref(), rows),
        Arc::new(StringArray::from(
            std::iter::repeat_n(provenance.pyrefly_provider_run_id.as_deref(), rows)
                .collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from(
            std::iter::repeat_n(provenance.pyrefly_provider_release.as_deref(), rows)
                .collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
            PYTHON_OWNER_FLOW_ALGORITHM_RELEASE,
            rows,
        ))),
        Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
            PYTHON_OWNER_FLOW_PRECISION_RELEASE,
            rows,
        ))),
        string_repeat(PYTHON_DERIVED_AUTHORITY, rows),
        string_repeat(completeness.as_str(), rows),
    ]
}

fn fixed_repeat<const N: usize>(value: Option<&[u8; N]>, rows: usize) -> ArrayRef {
    let width = i32::try_from(N).expect("fixed identity width fits i32");
    let mut builder = FixedSizeBinaryBuilder::with_capacity(rows, width);
    for _ in 0..rows {
        if let Some(value) = value {
            builder
                .append_value(value)
                .expect("validated fixed-width identity");
        } else {
            builder.append_null();
        }
    }
    Arc::new(builder.finish())
}

fn node_batch(
    nodes: &[PythonCfgNodeSeed],
    node_ids: &BTreeMap<u32, [u8; 16]>,
    suppressed: &BTreeSet<u32>,
    provenance: &PythonFlowProvenance,
    bindings: &PythonFlowBindings,
    completeness: PythonAnalysisCompleteness,
) -> Result<RecordBatch, ArrowError> {
    let fields = &bindings.fields;
    let mut schema_fields = common_fields(fields);
    schema_fields.extend([
        Field::new(
            fields.node_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(fields.node_ordinal.as_ref(), DataType::UInt32, false),
        Field::new(fields.node_kind.as_ref(), DataType::Utf8, false),
        Field::new(fields.start_byte.as_ref(), DataType::UInt64, true),
        Field::new(fields.end_byte.as_ref(), DataType::UInt64, true),
        Field::new(
            fields.next_node_id.as_ref(),
            DataType::FixedSizeBinary(16),
            true,
        ),
        Field::new(
            fields.next_edge_id.as_ref(),
            DataType::FixedSizeBinary(16),
            true,
        ),
        Field::new(fields.next_enabled.as_ref(), DataType::Boolean, false),
    ]);
    let mut columns = common_columns(provenance, nodes.len(), completeness);
    let next_node_ids = nodes
        .iter()
        .enumerate()
        .map(|(index, _)| {
            nodes
                .get(index.saturating_add(1))
                .map(|target| node_ids[&target.ordinal])
        })
        .collect::<Vec<_>>();
    let next_edge_ids = nodes
        .iter()
        .zip(&next_node_ids)
        .map(|(node, target)| {
            target.map(|target| {
                derived_id(
                    b"cfg-next-edge",
                    provenance,
                    &[&node_ids[&node.ordinal], &target],
                )
            })
        })
        .collect::<Vec<_>>();
    columns.extend([
        fixed_values(nodes.iter().map(|node| Some(&node_ids[&node.ordinal])), 16),
        Arc::new(UInt32Array::from_iter_values(
            nodes.iter().map(|node| node.ordinal),
        )) as ArrayRef,
        Arc::new(StringArray::from_iter_values(
            nodes.iter().map(|node| node.kind.as_ref()),
        )) as ArrayRef,
        Arc::new(UInt64Array::from(
            nodes.iter().map(|node| node.start_byte).collect::<Vec<_>>(),
        )) as ArrayRef,
        Arc::new(UInt64Array::from(
            nodes.iter().map(|node| node.end_byte).collect::<Vec<_>>(),
        )) as ArrayRef,
        fixed_values(next_node_ids.iter().map(Option::as_ref), 16),
        fixed_values(next_edge_ids.iter().map(Option::as_ref), 16),
        Arc::new(BooleanArray::from_iter(nodes.iter().enumerate().map(
            |(index, node)| {
                Some(next_node_ids[index].is_some() && !suppressed.contains(&node.ordinal))
            },
        ))) as ArrayRef,
    ]);
    RecordBatch::try_new(
        relation_schema(&bindings.relations.cfg_nodes, schema_fields),
        columns,
    )
}

fn explicit_edge_batch(
    edges: &[PythonCfgEdgeSeed],
    node_ids: &BTreeMap<u32, [u8; 16]>,
    provenance: &PythonFlowProvenance,
    bindings: &PythonFlowBindings,
    completeness: PythonAnalysisCompleteness,
) -> Result<RecordBatch, ArrowError> {
    let fields = &bindings.fields;
    let mut schema_fields = common_fields(fields);
    schema_fields.extend([
        Field::new(
            fields.edge_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(
            fields.source_node_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(
            fields.target_node_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(fields.edge_kind.as_ref(), DataType::Utf8, false),
    ]);
    let mut columns = common_columns(provenance, edges.len(), completeness);
    let edge_ids = edges
        .iter()
        .map(|edge| {
            derived_id(
                b"cfg-explicit-edge",
                provenance,
                &[
                    &node_ids[&edge.source_ordinal],
                    &node_ids[&edge.target_ordinal],
                    edge.kind.as_bytes(),
                ],
            )
        })
        .collect::<Vec<_>>();
    columns.extend([
        fixed_values(edge_ids.iter().map(Some), 16),
        fixed_values(
            edges
                .iter()
                .map(|edge| Some(&node_ids[&edge.source_ordinal])),
            16,
        ),
        fixed_values(
            edges
                .iter()
                .map(|edge| Some(&node_ids[&edge.target_ordinal])),
            16,
        ),
        Arc::new(StringArray::from_iter_values(
            edges.iter().map(|edge| edge.kind.as_ref()),
        )) as ArrayRef,
    ]);
    RecordBatch::try_new(
        relation_schema(&bindings.relations.cfg_edges, schema_fields),
        columns,
    )
}

fn event_batch(
    events: &[PythonFlowEventSeed],
    node_ids: &BTreeMap<u32, [u8; 16]>,
    provenance: &PythonFlowProvenance,
    bindings: &PythonFlowBindings,
    completeness: PythonAnalysisCompleteness,
) -> Result<RecordBatch, ArrowError> {
    let fields = &bindings.fields;
    let mut schema_fields = common_fields(fields);
    schema_fields.extend([
        Field::new(
            fields.event_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(
            fields.node_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(fields.event_ordinal.as_ref(), DataType::UInt32, false),
        Field::new(fields.event_role.as_ref(), DataType::Utf8, false),
        Field::new(
            fields.location_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
    ]);
    let mut columns = common_columns(provenance, events.len(), completeness);
    let event_ids = events
        .iter()
        .map(|event| {
            let role = match event.role {
                PythonFlowEventRole::Definition => bindings.values.definition_event.as_bytes(),
                PythonFlowEventRole::Use => bindings.values.use_event.as_bytes(),
            };
            derived_id(
                b"flow-event",
                provenance,
                &[
                    &event.location_id,
                    &event.node_ordinal.to_be_bytes(),
                    &event.event_ordinal.to_be_bytes(),
                    role,
                ],
            )
        })
        .collect::<Vec<_>>();
    columns.extend([
        fixed_values(event_ids.iter().map(Some), 16),
        fixed_values(
            events
                .iter()
                .map(|event| Some(&node_ids[&event.node_ordinal])),
            16,
        ),
        Arc::new(UInt32Array::from_iter_values(
            events.iter().map(|event| event.event_ordinal),
        )) as ArrayRef,
        Arc::new(StringArray::from_iter_values(events.iter().map(
            |event| match event.role {
                PythonFlowEventRole::Definition => bindings.values.definition_event.as_ref(),
                PythonFlowEventRole::Use => bindings.values.use_event.as_ref(),
            },
        ))) as ArrayRef,
        fixed_values(events.iter().map(|event| Some(&event.location_id)), 16),
    ]);
    RecordBatch::try_new(
        relation_schema(&bindings.relations.reaching_definitions, schema_fields),
        columns,
    )
}

fn flow_link_batch(
    rows: &[FlowLinkRow],
    provenance: &PythonFlowProvenance,
    bindings: &PythonFlowBindings,
    role: PythonDerivedRelation,
    relation_kind: &str,
    completeness: PythonAnalysisCompleteness,
) -> Result<RecordBatch, ArrowError> {
    let fields = &bindings.fields;
    let mut schema_fields = common_fields(fields);
    schema_fields.extend([
        Field::new(
            fields.edge_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(
            fields.definition_event_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(
            fields.use_event_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(
            fields.location_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(
            fields.source_node_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(
            fields.target_node_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(fields.relation_kind.as_ref(), DataType::Utf8, false),
    ]);
    let mut columns = common_columns(provenance, rows.len(), completeness);
    columns.extend([
        fixed_values(rows.iter().map(|row| Some(&row.edge_id)), 16),
        fixed_values(rows.iter().map(|row| Some(&row.definition_event_id)), 16),
        fixed_values(rows.iter().map(|row| Some(&row.use_event_id)), 16),
        fixed_values(rows.iter().map(|row| Some(&row.location_id)), 16),
        fixed_values(rows.iter().map(|row| Some(&row.source_node_id)), 16),
        fixed_values(rows.iter().map(|row| Some(&row.target_node_id)), 16),
        string_repeat(relation_kind, rows.len()),
    ]);
    RecordBatch::try_new(
        relation_schema(bindings.relation_id(role), schema_fields),
        columns,
    )
}

fn evaluation_batch(
    rows: &[EvaluationRow],
    provenance: &PythonFlowProvenance,
    bindings: &PythonFlowBindings,
    completeness: PythonAnalysisCompleteness,
) -> Result<RecordBatch, ArrowError> {
    let fields = &bindings.fields;
    let mut schema_fields = common_fields(fields);
    schema_fields.extend([
        Field::new(
            fields.edge_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(
            fields.predecessor_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(
            fields.successor_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(
            fields.source_node_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(
            fields.target_node_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(fields.relation_kind.as_ref(), DataType::Utf8, false),
    ]);
    let mut columns = common_columns(provenance, rows.len(), completeness);
    columns.extend([
        fixed_values(rows.iter().map(|row| Some(&row.edge_id)), 16),
        fixed_values(rows.iter().map(|row| Some(&row.predecessor_id)), 16),
        fixed_values(rows.iter().map(|row| Some(&row.successor_id)), 16),
        fixed_values(rows.iter().map(|row| Some(&row.source_node_id)), 16),
        fixed_values(rows.iter().map(|row| Some(&row.target_node_id)), 16),
        string_values(rows.iter().map(|row| row.relation_kind)),
    ]);
    RecordBatch::try_new(
        relation_schema(&bindings.relations.evaluation_order, schema_fields),
        columns,
    )
}

fn liveness_batch(
    rows: &[LiveRow],
    provenance: &PythonFlowProvenance,
    bindings: &PythonFlowBindings,
    completeness: PythonAnalysisCompleteness,
) -> Result<RecordBatch, ArrowError> {
    let fields = &bindings.fields;
    let mut schema_fields = common_fields(fields);
    schema_fields.extend([
        Field::new(
            fields.node_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(fields.node_ordinal.as_ref(), DataType::UInt32, false),
        Field::new(fields.boundary.as_ref(), DataType::Utf8, false),
        Field::new(
            fields.location_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(fields.relation_kind.as_ref(), DataType::Utf8, false),
    ]);
    let mut columns = common_columns(provenance, rows.len(), completeness);
    columns.extend([
        fixed_values(rows.iter().map(|row| Some(&row.node_id)), 16),
        u32_values(rows.iter().map(|row| row.node_ordinal)),
        string_values(rows.iter().map(|row| row.boundary)),
        fixed_values(rows.iter().map(|row| Some(&row.location_id)), 16),
        string_values(rows.iter().map(|row| {
            if row.boundary == "ENTRY" {
                bindings.values.live_entry.as_ref()
            } else {
                bindings.values.live_exit.as_ref()
            }
        })),
    ]);
    RecordBatch::try_new(
        relation_schema(&bindings.relations.liveness, schema_fields),
        columns,
    )
}

fn memory_batch(
    rows: &[PythonMemoryLocationSeed],
    node_ids: &BTreeMap<u32, [u8; 16]>,
    provenance: &PythonFlowProvenance,
    bindings: &PythonFlowBindings,
    completeness: PythonAnalysisCompleteness,
) -> Result<RecordBatch, ArrowError> {
    let fields = &bindings.fields;
    let mut schema_fields = common_fields(fields);
    schema_fields.extend([
        Field::new(
            fields.location_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(fields.memory_kind.as_ref(), DataType::Utf8, false),
        Field::new(
            fields.base_location_id.as_ref(),
            DataType::FixedSizeBinary(16),
            true,
        ),
        Field::new(fields.selector.as_ref(), DataType::Utf8, true),
        Field::new(fields.selector_dynamic.as_ref(), DataType::Boolean, false),
        Field::new(
            fields.allocation_node_id.as_ref(),
            DataType::FixedSizeBinary(16),
            true,
        ),
    ]);
    let mut columns = common_columns(provenance, rows.len(), completeness);
    columns.extend([
        fixed_values(rows.iter().map(|row| Some(&row.location_id)), 16),
        string_values(rows.iter().map(|row| row.kind.label())),
        fixed_values(rows.iter().map(|row| row.base_location_id.as_ref()), 16),
        optional_string_values(rows.iter().map(|row| row.selector.as_deref())),
        bool_values(rows.iter().map(|row| row.selector_dynamic)),
        fixed_values(
            rows.iter().map(|row| {
                row.allocation_node_ordinal
                    .and_then(|ordinal| node_ids.get(&ordinal))
            }),
            16,
        ),
    ]);
    RecordBatch::try_new(
        relation_schema(&bindings.relations.memory_locations, schema_fields),
        columns,
    )
}

fn alias_batch(
    rows: &[AliasRow],
    provenance: &PythonFlowProvenance,
    bindings: &PythonFlowBindings,
    completeness: PythonAnalysisCompleteness,
) -> Result<RecordBatch, ArrowError> {
    let fields = &bindings.fields;
    let mut schema_fields = common_fields(fields);
    schema_fields.extend([
        Field::new(
            fields.edge_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(
            fields.alias_source_location_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(
            fields.alias_target_location_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(fields.relation_kind.as_ref(), DataType::Utf8, false),
        Field::new(fields.evidence.as_ref(), DataType::Utf8, false),
    ]);
    let mut columns = common_columns(provenance, rows.len(), completeness);
    columns.extend([
        fixed_values(rows.iter().map(|row| Some(&row.edge_id)), 16),
        fixed_values(rows.iter().map(|row| Some(&row.source_location_id)), 16),
        fixed_values(rows.iter().map(|row| Some(&row.target_location_id)), 16),
        string_values(rows.iter().map(|row| row.relation_kind.as_ref())),
        string_values(rows.iter().map(|row| row.evidence.as_ref())),
    ]);
    RecordBatch::try_new(
        relation_schema(&bindings.relations.alias_points_to, schema_fields),
        columns,
    )
}

fn effect_batch(
    rows: &[PythonEffectSeed],
    node_ids: &BTreeMap<u32, [u8; 16]>,
    provenance: &PythonFlowProvenance,
    bindings: &PythonFlowBindings,
    role: PythonDerivedRelation,
    completeness: PythonAnalysisCompleteness,
) -> Result<RecordBatch, ArrowError> {
    let fields = &bindings.fields;
    let mut schema_fields = common_fields(fields);
    schema_fields.extend([
        Field::new(
            fields.effect_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(
            fields.node_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(fields.effect_ordinal.as_ref(), DataType::UInt32, false),
        Field::new(fields.effect_kind.as_ref(), DataType::Utf8, false),
        Field::new(
            fields.subject_location_id.as_ref(),
            DataType::FixedSizeBinary(16),
            true,
        ),
        Field::new(fields.resource_kind.as_ref(), DataType::Utf8, true),
        Field::new(fields.evidence.as_ref(), DataType::Utf8, false),
    ]);
    let mut columns = common_columns(provenance, rows.len(), completeness);
    let effect_ids = rows
        .iter()
        .map(|row| {
            derived_id(
                b"python-effect",
                provenance,
                &[
                    &node_ids[&row.node_ordinal],
                    &row.effect_ordinal.to_be_bytes(),
                    row.kind.label().as_bytes(),
                ],
            )
        })
        .collect::<Vec<_>>();
    columns.extend([
        fixed_values(effect_ids.iter().map(Some), 16),
        fixed_values(
            rows.iter().map(|row| Some(&node_ids[&row.node_ordinal])),
            16,
        ),
        u32_values(rows.iter().map(|row| row.effect_ordinal)),
        string_values(rows.iter().map(|row| row.kind.label())),
        fixed_values(rows.iter().map(|row| row.subject_location_id.as_ref()), 16),
        optional_string_values(rows.iter().map(|row| row.resource_kind.as_deref())),
        string_values(rows.iter().map(|row| row.evidence.as_ref())),
    ]);
    RecordBatch::try_new(
        relation_schema(bindings.relation_id(role), schema_fields),
        columns,
    )
}

fn suspension_batch(
    rows: &[PythonSuspensionSeed],
    node_ids: &BTreeMap<u32, [u8; 16]>,
    provenance: &PythonFlowProvenance,
    bindings: &PythonFlowBindings,
    completeness: PythonAnalysisCompleteness,
) -> Result<RecordBatch, ArrowError> {
    let fields = &bindings.fields;
    let mut schema_fields = common_fields(fields);
    schema_fields.extend([
        Field::new(
            fields.suspension_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(
            fields.node_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(fields.suspension_ordinal.as_ref(), DataType::UInt32, false),
        Field::new(fields.suspension_kind.as_ref(), DataType::Utf8, false),
        Field::new(
            fields.resume_node_id.as_ref(),
            DataType::FixedSizeBinary(16),
            true,
        ),
        Field::new(
            fields.exceptional_resume_node_id.as_ref(),
            DataType::FixedSizeBinary(16),
            true,
        ),
    ]);
    let mut columns = common_columns(provenance, rows.len(), completeness);
    let suspension_ids = rows
        .iter()
        .map(|row| {
            derived_id(
                b"python-suspension",
                provenance,
                &[
                    &node_ids[&row.node_ordinal],
                    &row.suspension_ordinal.to_be_bytes(),
                    row.kind.label().as_bytes(),
                ],
            )
        })
        .collect::<Vec<_>>();
    columns.extend([
        fixed_values(suspension_ids.iter().map(Some), 16),
        fixed_values(
            rows.iter().map(|row| Some(&node_ids[&row.node_ordinal])),
            16,
        ),
        u32_values(rows.iter().map(|row| row.suspension_ordinal)),
        string_values(rows.iter().map(|row| row.kind.label())),
        fixed_values(
            rows.iter()
                .map(|row| row.resume_node_ordinal.and_then(|node| node_ids.get(&node))),
            16,
        ),
        fixed_values(
            rows.iter().map(|row| {
                row.exceptional_resume_node_ordinal
                    .and_then(|node| node_ids.get(&node))
            }),
            16,
        ),
    ]);
    RecordBatch::try_new(
        relation_schema(&bindings.relations.async_suspension, schema_fields),
        columns,
    )
}

fn invalidation_batch(
    rows: &[InvalidationRow],
    provenance: &PythonFlowProvenance,
    bindings: &PythonFlowBindings,
    completeness: PythonAnalysisCompleteness,
) -> Result<RecordBatch, ArrowError> {
    let fields = &bindings.fields;
    let mut schema_fields = common_fields(fields);
    schema_fields.extend([
        Field::new(
            fields.invalidated_owner_id.as_ref(),
            DataType::FixedSizeBinary(16),
            false,
        ),
        Field::new(fields.invalidation_reason.as_ref(), DataType::Utf8, false),
    ]);
    let mut columns = common_columns(provenance, rows.len(), completeness);
    columns.extend([
        fixed_values(rows.iter().map(|row| Some(&row.invalidated_owner_id)), 16),
        string_values(rows.iter().map(|row| row.reason)),
    ]);
    RecordBatch::try_new(
        relation_schema(&bindings.relations.invalidations, schema_fields),
        columns,
    )
}

fn unknown_batch(
    rows: &[UnknownRow],
    provenance: &PythonFlowProvenance,
    bindings: &PythonFlowBindings,
) -> Result<RecordBatch, ArrowError> {
    let fields = &bindings.fields;
    let mut schema_fields = common_fields(fields);
    schema_fields.extend([
        Field::new(fields.unknown_family.as_ref(), DataType::Utf8, false),
        Field::new(fields.unknown_reason.as_ref(), DataType::Utf8, false),
        Field::new(fields.unknown_detail.as_ref(), DataType::Utf8, false),
        Field::new(fields.bounded.as_ref(), DataType::Boolean, false),
    ]);
    let mut columns = common_columns(provenance, rows.len(), PythonAnalysisCompleteness::Unknown);
    columns.extend([
        string_values(rows.iter().map(|row| row.family.as_ref())),
        string_values(rows.iter().map(|row| row.reason.as_ref())),
        string_values(rows.iter().map(|row| row.detail.as_ref())),
        bool_values(rows.iter().map(|row| row.bounded)),
    ]);
    RecordBatch::try_new(
        relation_schema(&bindings.relations.unknowns, schema_fields),
        columns,
    )
}

fn cfg_node_output_columns(fields: &PythonFlowFields) -> Vec<Expr> {
    [
        &fields.model_epoch_id,
        &fields.source_pin,
        &fields.analysis_context_id,
        &fields.source_generation,
        &fields.owner_id,
        &fields.ruff_provider_run_id,
        &fields.ruff_provider_release,
        &fields.pyrefly_provider_run_id,
        &fields.pyrefly_provider_release,
        &fields.algorithm_release,
        &fields.precision_release,
        &fields.authority,
        &fields.analysis_completeness,
        &fields.node_id,
        &fields.node_ordinal,
        &fields.node_kind,
        &fields.start_byte,
        &fields.end_byte,
    ]
    .into_iter()
    .map(|field| col(field.as_ref()))
    .collect()
}

fn relation_schema(relation: &RelationId, fields: Vec<Field>) -> SchemaRef {
    Arc::new(Schema::new_with_metadata(
        fields,
        HashMap::from([
            (
                "codefabric.relation_id".to_owned(),
                relation.as_str().to_owned(),
            ),
            (
                "codefabric.authority".to_owned(),
                PYTHON_DERIVED_AUTHORITY.to_owned(),
            ),
            (
                "codefabric.algorithm_release".to_owned(),
                PYTHON_OWNER_FLOW_ALGORITHM_RELEASE.to_owned(),
            ),
            (
                "codefabric.precision_release".to_owned(),
                PYTHON_OWNER_FLOW_PRECISION_RELEASE.to_owned(),
            ),
        ]),
    ))
}

fn fixed_values<'a, const N: usize>(
    values: impl IntoIterator<Item = Option<&'a [u8; N]>>,
    width: i32,
) -> ArrayRef {
    let iterator = values.into_iter();
    let (lower, _) = iterator.size_hint();
    let mut builder = FixedSizeBinaryBuilder::with_capacity(lower, width);
    for value in iterator {
        if let Some(value) = value {
            builder
                .append_value(value)
                .expect("validated fixed-width identity");
        } else {
            builder.append_null();
        }
    }
    Arc::new(builder.finish())
}

fn string_values<'a>(values: impl IntoIterator<Item = &'a str>) -> ArrayRef {
    let iterator = values.into_iter();
    let (lower, _) = iterator.size_hint();
    let mut builder = StringBuilder::with_capacity(lower, lower.saturating_mul(24));
    for value in iterator {
        builder.append_value(value);
    }
    Arc::new(builder.finish())
}

fn string_repeat(value: &str, rows: usize) -> ArrayRef {
    Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
        value, rows,
    )))
}

fn optional_string_values<'a>(values: impl IntoIterator<Item = Option<&'a str>>) -> ArrayRef {
    Arc::new(StringArray::from(values.into_iter().collect::<Vec<_>>()))
}

fn u32_values(values: impl IntoIterator<Item = u32>) -> ArrayRef {
    Arc::new(UInt32Array::from_iter_values(values))
}

fn bool_values(values: impl IntoIterator<Item = bool>) -> ArrayRef {
    Arc::new(BooleanArray::from_iter(values.into_iter().map(Some)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{Array, FixedSizeBinaryArray};

    fn relation(value: &str) -> RelationId {
        RelationId::new(value).expect("test relation")
    }

    fn bindings() -> PythonFlowBindings {
        PythonFlowBindings {
            relations: PythonFlowRelations {
                cfg_nodes: relation("application.python.cfg_node"),
                cfg_edges: relation("application.python.cfg_edge"),
                evaluation_order: relation("application.python.evaluation_order"),
                def_use: relation("application.python.def_use"),
                reaching_definitions: relation("application.python.reaching_definition"),
                liveness: relation("application.python.liveness"),
                value_flow: relation("application.python.value_flow"),
                memory_locations: relation("application.python.memory_location"),
                alias_points_to: relation("application.python.alias_points_to"),
                effects: relation("application.python.effect"),
                resource_lifecycle: relation("application.python.resource_lifecycle"),
                async_suspension: relation("application.python.async_suspension"),
                invalidations: relation("application.python.invalidation"),
                unknowns: relation("application.python.unknown"),
            },
            fields: PythonFlowFields {
                model_epoch_id: "model_epoch_id".into(),
                source_pin: "source_pin".into(),
                analysis_context_id: "analysis_context_id".into(),
                source_generation: "source_generation".into(),
                owner_id: "owner_id".into(),
                ruff_provider_run_id: "ruff_provider_run_id".into(),
                ruff_provider_release: "ruff_provider_release".into(),
                pyrefly_provider_run_id: "pyrefly_provider_run_id".into(),
                pyrefly_provider_release: "pyrefly_provider_release".into(),
                algorithm_release: "algorithm_release".into(),
                precision_release: "precision_release".into(),
                authority: "authority".into(),
                analysis_completeness: "analysis_completeness".into(),
                node_id: "node_id".into(),
                node_ordinal: "node_ordinal".into(),
                node_kind: "node_kind".into(),
                start_byte: "start_byte".into(),
                end_byte: "end_byte".into(),
                next_node_id: "next_node_id".into(),
                next_edge_id: "next_edge_id".into(),
                next_enabled: "next_enabled".into(),
                edge_id: "edge_id".into(),
                source_node_id: "source_node_id".into(),
                target_node_id: "target_node_id".into(),
                edge_kind: "edge_kind".into(),
                event_id: "event_id".into(),
                event_ordinal: "event_ordinal".into(),
                event_role: "event_role".into(),
                location_id: "location_id".into(),
                definition_event_id: "definition_event_id".into(),
                use_event_id: "use_event_id".into(),
                relation_kind: "relation_kind".into(),
                boundary: "boundary".into(),
                predecessor_id: "predecessor_id".into(),
                successor_id: "successor_id".into(),
                memory_kind: "memory_kind".into(),
                base_location_id: "base_location_id".into(),
                selector: "selector".into(),
                selector_dynamic: "selector_dynamic".into(),
                allocation_node_id: "allocation_node_id".into(),
                alias_source_location_id: "alias_source_location_id".into(),
                alias_target_location_id: "alias_target_location_id".into(),
                evidence: "evidence".into(),
                effect_id: "effect_id".into(),
                effect_ordinal: "effect_ordinal".into(),
                effect_kind: "effect_kind".into(),
                subject_location_id: "subject_location_id".into(),
                resource_kind: "resource_kind".into(),
                suspension_id: "suspension_id".into(),
                suspension_ordinal: "suspension_ordinal".into(),
                suspension_kind: "suspension_kind".into(),
                resume_node_id: "resume_node_id".into(),
                exceptional_resume_node_id: "exceptional_resume_node_id".into(),
                invalidated_owner_id: "invalidated_owner_id".into(),
                invalidation_reason: "invalidation_reason".into(),
                bounded: "bounded".into(),
                unknown_family: "unknown_family".into(),
                unknown_reason: "unknown_reason".into(),
                unknown_detail: "unknown_detail".into(),
            },
            values: PythonFlowSemanticValues {
                sequential_edge: "next".into(),
                definition_event: "definition".into(),
                use_event: "use".into(),
                reaching_definition: "reaching_definition".into(),
                def_use: "def_use".into(),
                value_flow: "value_flow".into(),
                may_alias: "may_alias".into(),
                may_point_to: "may_point_to".into(),
                live_entry: "live_entry".into(),
                live_exit: "live_exit".into(),
            },
            cfg_authority: ProviderAuthorityClass::PythonCfg,
            dataflow_authority: ProviderAuthorityClass::PythonDataflow,
            alias_authority: ProviderAuthorityClass::PythonAlias,
            effect_authority: ProviderAuthorityClass::PythonEffect,
            summary_authority: ProviderAuthorityClass::PythonSummary,
        }
    }

    fn provenance() -> PythonFlowProvenance {
        PythonFlowProvenance {
            model_epoch_id: [1; 32],
            source_pin: [2; 32],
            analysis_context_id: [3; 32],
            source_generation: 7,
            owner_id: [4; 16],
            ruff_provider_run_id: Some([5; 16]),
            ruff_provider_release: Arc::from("ruff-0.16.1-components-0.0.7"),
            pyrefly_provider_run_id: Some(Arc::from("pyrefly-run-7")),
            pyrefly_provider_release: Some(Arc::from("pyrefly-1.2.0")),
        }
    }

    fn node(ordinal: u32, kind: &str) -> PythonCfgNodeSeed {
        PythonCfgNodeSeed {
            ordinal,
            kind: Arc::from(kind),
            start_byte: Some(u64::from(ordinal) * 10),
            end_byte: Some(u64::from(ordinal) * 10 + 5),
        }
    }

    fn linear_input() -> PythonOwnerFlowInput {
        PythonOwnerFlowInput {
            provenance: provenance(),
            nodes: vec![node(0, "entry"), node(1, "statement"), node(2, "exit")],
            explicit_edges: Vec::new(),
            events: vec![
                PythonFlowEventSeed {
                    node_ordinal: 0,
                    event_ordinal: 0,
                    location_id: [9; 16],
                    role: PythonFlowEventRole::Definition,
                },
                PythonFlowEventSeed {
                    node_ordinal: 1,
                    event_ordinal: 1,
                    location_id: [9; 16],
                    role: PythonFlowEventRole::Definition,
                },
                PythonFlowEventSeed {
                    node_ordinal: 2,
                    event_ordinal: 2,
                    location_id: [9; 16],
                    role: PythonFlowEventRole::Use,
                },
            ],
            memory_locations: vec![PythonMemoryLocationSeed {
                location_id: [9; 16],
                kind: PythonMemoryLocationKind::Local,
                base_location_id: None,
                selector: None,
                selector_dynamic: false,
                allocation_node_ordinal: None,
            }],
            effects: Vec::new(),
            suspensions: Vec::new(),
            invalidation: PythonInvalidationSeed {
                changed_owners: vec![[4; 16]],
                pyrefly_affected_owners: Vec::new(),
                reverse_importer_owners: Vec::new(),
            },
            completeness: PythonOwnerInputCompleteness::complete(),
            pyrefly: PyreflySemanticEvidence::Available {
                provider_run_id: Arc::from("pyrefly-run-7"),
            },
        }
    }

    fn row_count(batches: &[RecordBatch]) -> usize {
        batches.iter().map(RecordBatch::num_rows).sum()
    }

    fn plan_contains(plan: &LogicalPlan, predicate: impl Copy + Fn(&LogicalPlan) -> bool) -> bool {
        predicate(plan)
            || plan
                .inputs()
                .into_iter()
                .any(|input| plan_contains(input, predicate))
    }

    #[tokio::test]
    async fn native_cfg_and_linear_reaching_definition_execute() {
        let compiled = compile_python_owner_flow(linear_input(), &bindings()).expect("compile");
        let plan = compiled.reaching_definitions_plan();
        assert!(plan_contains(plan, |node| matches!(
            node,
            LogicalPlan::Join(_)
        )));
        assert!(plan_contains(plan, |node| matches!(
            node,
            LogicalPlan::Window(_)
        )));
        assert!(plan_contains(plan, |node| matches!(
            node,
            LogicalPlan::Filter(_)
        )));
        let output = compiled
            .execute(&SessionContext::new())
            .await
            .expect("execute");
        assert_eq!(row_count(&output.cfg_nodes), 3);
        assert_eq!(row_count(&output.cfg_edges), 2);
        assert_eq!(row_count(&output.reaching_definitions), 1);
        assert_eq!(output.unknowns.num_rows(), 0);
        assert!(output.observation.reaching_definitions_complete);
    }

    #[tokio::test]
    async fn branch_loop_and_exception_cfg_participate_in_bounded_fixed_points() {
        let mut input = linear_input();
        input.nodes = vec![
            node(0, "branch"),
            node(1, "true-block"),
            node(2, "loop-header"),
            node(3, "handler"),
        ];
        input.explicit_edges = vec![
            PythonCfgEdgeSeed {
                source_ordinal: 0,
                target_ordinal: 1,
                kind: "true".into(),
                suppresses_sequential_edge: true,
            },
            PythonCfgEdgeSeed {
                source_ordinal: 0,
                target_ordinal: 2,
                kind: "false".into(),
                suppresses_sequential_edge: true,
            },
            PythonCfgEdgeSeed {
                source_ordinal: 2,
                target_ordinal: 0,
                kind: "loop_back".into(),
                suppresses_sequential_edge: true,
            },
            PythonCfgEdgeSeed {
                source_ordinal: 1,
                target_ordinal: 3,
                kind: "exception".into(),
                suppresses_sequential_edge: false,
            },
        ];
        input.events.clear();
        let compiled = compile_python_owner_flow(input, &bindings()).expect("compile");
        let output = compiled
            .execute(&SessionContext::new())
            .await
            .expect("execute");
        assert_eq!(row_count(&output.cfg_edges), 5);
        assert_eq!(row_count(&output.reaching_definitions), 0);
        assert_eq!(output.unknowns.num_rows(), 0);
        assert!(output.observation.reaching_definitions_complete);
        assert!(
            output
                .observation
                .operators
                .contains(&PythonFlowNativeOperator::BoundedOwnerFixedPoint)
        );
    }

    #[test]
    fn missing_pyrefly_input_is_an_explicit_unknown() {
        let mut input = linear_input();
        input.provenance.pyrefly_provider_run_id = None;
        input.pyrefly = PyreflySemanticEvidence::Unknown {
            reason: "sidecar-timeout".into(),
        };
        let compiled = compile_python_owner_flow(input, &bindings()).expect("compile");
        assert_eq!(compiled.unknowns().num_rows(), 1);
        let reason = compiled
            .unknowns()
            .column_by_name("unknown_reason")
            .expect("reason")
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("utf8");
        assert_eq!(reason.value(0), "PYREFLY_SEMANTIC_UNAVAILABLE");
        assert!(!compiled.observation().reaching_definitions_complete);
    }

    #[tokio::test]
    async fn stable_identity_is_independent_of_input_row_order() {
        let mut reversed = linear_input();
        reversed.nodes.reverse();
        reversed.events.reverse();
        let left = compile_python_owner_flow(linear_input(), &bindings())
            .expect("left")
            .execute(&SessionContext::new())
            .await
            .expect("left execute");
        let right = compile_python_owner_flow(reversed, &bindings())
            .expect("right")
            .execute(&SessionContext::new())
            .await
            .expect("right execute");
        let ids = |output: &PythonOwnerFlowExecution| {
            output.cfg_nodes[0]
                .column_by_name("node_id")
                .expect("node id")
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .expect("fixed")
                .iter()
                .map(|value| value.expect("id").to_vec())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&left), ids(&right));
    }

    #[test]
    fn provider_native_output_authority_is_rejected() {
        let mut invalid = bindings();
        invalid.cfg_authority = ProviderAuthorityClass::ProviderNative;
        assert!(matches!(
            compile_python_owner_flow(linear_input(), &invalid),
            Err(PythonDerivedAnalysisError::ProviderAuthorityClaim)
        ));
    }

    #[test]
    fn missing_ruff_evidence_cannot_complete_or_consume_unpinned_structure() {
        let mut input = linear_input();
        input.provenance.ruff_provider_run_id = None;
        input.nodes.clear();
        input.events.clear();
        input.memory_locations.clear();
        let compiled = compile_python_owner_flow(input, &bindings()).expect("explicit unknown");
        assert!(!compiled.observation().reaching_definitions_complete);
        assert_eq!(compiled.unknowns().num_rows(), 1);

        let mut contradictory = linear_input();
        contradictory.provenance.ruff_provider_run_id = None;
        assert!(matches!(
            compile_python_owner_flow(contradictory, &bindings()),
            Err(PythonDerivedAnalysisError::InvalidInput(_))
        ));
    }

    #[test]
    fn zero_or_stale_provider_evidence_pins_fail_closed() {
        let mut zero_ruff = linear_input();
        zero_ruff.provenance.ruff_provider_run_id = Some([0; 16]);
        assert!(matches!(
            compile_python_owner_flow(zero_ruff, &bindings()),
            Err(PythonDerivedAnalysisError::InvalidProvenance(_))
        ));

        let mut stale_pyrefly = linear_input();
        stale_pyrefly.pyrefly = PyreflySemanticEvidence::Unknown {
            reason: "sidecar-timeout".into(),
        };
        assert!(matches!(
            compile_python_owner_flow(stale_pyrefly, &bindings()),
            Err(PythonDerivedAnalysisError::InvalidProvenance(_))
        ));
    }

    #[tokio::test]
    async fn sparse_evaluation_ordinals_join_by_sorted_adjacency() {
        let mut input = linear_input();
        input.nodes = vec![node(10, "entry"), node(30, "statement"), node(70, "exit")];
        input.events.clear();
        let output = compile_python_owner_flow(input, &bindings())
            .expect("sparse ordinals compile")
            .execute(&SessionContext::new())
            .await
            .expect("sparse ordinals execute");
        assert_eq!(row_count(&output.cfg_edges), 2);
        assert!(output.observation.reaching_definitions_complete);
    }

    #[test]
    fn definition_and_use_must_not_share_one_total_order_ordinal() {
        let mut input = linear_input();
        input.events[1].event_ordinal = input.events[0].event_ordinal;
        assert!(matches!(
            compile_python_owner_flow(input, &bindings()),
            Err(PythonDerivedAnalysisError::InvalidInput(_))
        ));
    }

    #[tokio::test]
    async fn exceptional_paths_feed_def_use_liveness_and_value_flow() {
        let mut input = linear_input();
        input.nodes = vec![node(0, "entry"), node(1, "body"), node(2, "handler")];
        input.explicit_edges = vec![
            PythonCfgEdgeSeed {
                source_ordinal: 0,
                target_ordinal: 1,
                kind: "normal".into(),
                suppresses_sequential_edge: true,
            },
            PythonCfgEdgeSeed {
                source_ordinal: 0,
                target_ordinal: 2,
                kind: "exception".into(),
                suppresses_sequential_edge: false,
            },
        ];
        input.events = vec![
            PythonFlowEventSeed {
                node_ordinal: 0,
                event_ordinal: 0,
                location_id: [9; 16],
                role: PythonFlowEventRole::Definition,
            },
            PythonFlowEventSeed {
                node_ordinal: 1,
                event_ordinal: 1,
                location_id: [9; 16],
                role: PythonFlowEventRole::Use,
            },
            PythonFlowEventSeed {
                node_ordinal: 2,
                event_ordinal: 2,
                location_id: [9; 16],
                role: PythonFlowEventRole::Use,
            },
        ];
        let compiled = compile_python_owner_flow(input, &bindings()).expect("compile");
        assert_eq!(
            compiled
                .derived_relation(PythonDerivedRelation::DefUse)
                .expect("def-use")
                .num_rows(),
            2
        );
        assert_eq!(
            compiled
                .derived_relation(PythonDerivedRelation::ValueFlow)
                .expect("value-flow")
                .num_rows(),
            2
        );
        assert!(
            compiled
                .derived_relation(PythonDerivedRelation::Liveness)
                .expect("liveness")
                .num_rows()
                > 0
        );
        let output = compiled
            .execute(&SessionContext::new())
            .await
            .expect("execute");
        assert_eq!(row_count(&output.reaching_definitions), 2);
    }

    #[test]
    fn dynamic_memory_effect_resource_and_async_evidence_remain_typed() {
        let mut input = linear_input();
        input.memory_locations.extend([
            PythonMemoryLocationSeed {
                location_id: [10; 16],
                kind: PythonMemoryLocationKind::HeapObject,
                base_location_id: None,
                selector: None,
                selector_dynamic: false,
                allocation_node_ordinal: Some(0),
            },
            PythonMemoryLocationSeed {
                location_id: [11; 16],
                kind: PythonMemoryLocationKind::Attribute,
                base_location_id: Some([10; 16]),
                selector: None,
                selector_dynamic: true,
                allocation_node_ordinal: None,
            },
            PythonMemoryLocationSeed {
                location_id: [12; 16],
                kind: PythonMemoryLocationKind::Attribute,
                base_location_id: Some([10; 16]),
                selector: Some("handle".into()),
                selector_dynamic: false,
                allocation_node_ordinal: None,
            },
        ]);
        input.effects = vec![
            PythonEffectSeed {
                node_ordinal: 0,
                effect_ordinal: 0,
                kind: PythonEffectKind::Acquire,
                subject_location_id: Some([10; 16]),
                resource_kind: Some("file-handle".into()),
                evidence: "context-manager-enter".into(),
            },
            PythonEffectSeed {
                node_ordinal: 2,
                effect_ordinal: 1,
                kind: PythonEffectKind::Release,
                subject_location_id: Some([10; 16]),
                resource_kind: Some("file-handle".into()),
                evidence: "context-manager-exit".into(),
            },
        ];
        input.suspensions = vec![PythonSuspensionSeed {
            node_ordinal: 1,
            suspension_ordinal: 0,
            kind: PythonSuspensionKind::Await,
            resume_node_ordinal: Some(2),
            exceptional_resume_node_ordinal: Some(0),
        }];

        let compiled = compile_python_owner_flow(input, &bindings()).expect("compile");
        assert_eq!(
            compiled
                .derived_relation(PythonDerivedRelation::MemoryLocation)
                .expect("memory")
                .num_rows(),
            4
        );
        assert!(
            compiled
                .derived_relation(PythonDerivedRelation::AliasPointsTo)
                .expect("alias")
                .num_rows()
                >= 3
        );
        assert_eq!(
            compiled
                .derived_relation(PythonDerivedRelation::ResourceLifecycle)
                .expect("resource")
                .num_rows(),
            2
        );
        assert_eq!(
            compiled
                .derived_relation(PythonDerivedRelation::AsyncSuspension)
                .expect("async")
                .num_rows(),
            1
        );
        assert_eq!(
            compiled.observation().relation_completeness[&PythonDerivedRelation::AliasPointsTo],
            PythonAnalysisCompleteness::Partial
        );
        for role in [
            PythonDerivedRelation::MemoryLocation,
            PythonDerivedRelation::AliasPointsTo,
            PythonDerivedRelation::Effect,
            PythonDerivedRelation::ResourceLifecycle,
            PythonDerivedRelation::AsyncSuspension,
        ] {
            let batch = compiled.derived_relation(role).expect("materialized role");
            assert_eq!(
                batch.schema().metadata().get("codefabric.authority"),
                Some(&PYTHON_DERIVED_AUTHORITY.to_owned())
            );
            let authorities = batch
                .column_by_name("authority")
                .expect("authority")
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("utf8");
            assert!((0..authorities.len()).all(|row| {
                authorities.value(row) == PYTHON_DERIVED_AUTHORITY
                    && !authorities.value(row).contains("ruff")
                    && !authorities.value(row).contains("pyrefly")
            }));
        }
    }

    #[test]
    fn unavailable_memory_is_unknown_and_never_serialized_as_complete() {
        let mut input = linear_input();
        input.memory_locations.clear();
        input.completeness.memory_evidence = PythonInputCompleteness::Unavailable {
            reason: "Pyrefly heap evidence was not produced".into(),
        };
        let compiled = compile_python_owner_flow(input, &bindings()).expect("compile");
        for role in [
            PythonDerivedRelation::MemoryLocation,
            PythonDerivedRelation::AliasPointsTo,
        ] {
            assert_eq!(
                compiled.observation().relation_completeness[&role],
                PythonAnalysisCompleteness::Unknown
            );
            assert_eq!(compiled.derived_relation(role).expect("role").num_rows(), 0);
        }
        let reasons = compiled
            .unknowns()
            .column_by_name("unknown_reason")
            .expect("reason")
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("utf8");
        assert!((0..reasons.len()).any(|row| reasons.value(row) == "ACCEPTED_INPUT_NOT_COMPLETE"));
    }

    #[test]
    fn reverse_importer_invalidation_is_order_independent_for_incremental_recompute() {
        let seed = PythonInvalidationSeed {
            changed_owners: vec![[1; 16], [2; 16]],
            pyrefly_affected_owners: vec![[3; 16], [2; 16]],
            reverse_importer_owners: vec![[4; 16], [3; 16]],
        };
        let mut reordered = seed.clone();
        reordered.changed_owners.reverse();
        reordered.pyrefly_affected_owners.reverse();
        reordered.reverse_importer_owners.reverse();
        assert_eq!(
            derive_python_invalidation_closure(&seed).expect("clean closure"),
            derive_python_invalidation_closure(&reordered).expect("incremental closure")
        );
        assert_eq!(
            derive_python_invalidation_closure(&seed).expect("closure"),
            BTreeSet::from([[1; 16], [2; 16], [3; 16], [4; 16]])
        );
    }
}
