//! Versioned common graph and interprocedural analyses over accepted application facts.
//!
//! Input and output relation identities, field identities, family identities, authority, and
//! semantic class come from the installed application contract. DataFusion performs typed relational
//! deduplication and deterministic ordering at both sides of the application algorithm seam.
//! Petgraph is used only for graph algorithms that are irreducible to the selected relational
//! rung. Its `NodeIndex` values remain graph-local handles behind external canonical-ID maps and
//! never cross the Arrow boundary.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::num::{NonZeroU16, NonZeroUsize};
use std::sync::Arc;

use arrow_array::builder::FixedSizeBinaryBuilder;
use arrow_array::{
    Array, ArrayRef, BooleanArray, FixedSizeBinaryArray, RecordBatch, StringArray, UInt32Array,
    UInt64Array,
};
use arrow_schema::{ArrowError, DataType, Field, Schema, SchemaRef};
use arrow_select::concat::concat_batches;
use datafusion::datasource::{MemTable, provider_as_source};
use datafusion::execution::context::SessionContext;
use datafusion::logical_expr::LogicalPlanBuilder;
use datafusion::prelude::col;
use petgraph::Directed;
use petgraph::algo::{condensation, dominators::simple_fast, tarjan_scc, toposort};
use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::Reversed;
use thiserror::Error;

use crate::relational_program::{FieldId, RelationId};

pub const COMMON_CFG_ANALYSIS_RELEASE: &str = "codefabric.common-cfg.petgraph-0.8.3.v1";
pub const COMMON_DATA_DEPENDENCE_RELEASE: &str =
    "codefabric.common-data-dependence.datafusion-55.v1";
pub const COMMON_CALL_GRAPH_RELEASE: &str = "codefabric.common-call-graph.petgraph-0.8.3.v1";
pub const COMMON_INTERPROCEDURAL_RELEASE: &str =
    "codefabric.common-interprocedural.monotone-set.v1";
pub const COMMON_CFG_PRECISION: &str = "canonical-node-exact-accepted-cfg.v1";
pub const COMMON_DATA_DEPENDENCE_PRECISION: &str = "accepted-def-use-reaching-evidence.v1";
pub const COMMON_CALL_GRAPH_PRECISION: &str = "exact-target-edges-explicit-dynamic-remainder.v1";
pub const COMMON_INTERPROCEDURAL_PRECISION: &str =
    "context-insensitive-may-effect-resource-union.v1";

const STAGE_KIND: &str = "__cf_common_stage_kind";
const STAGE_SCOPE: &str = "__cf_common_stage_scope";
const STAGE_SOURCE: &str = "__cf_common_stage_source";
const STAGE_TARGET: &str = "__cf_common_stage_target";
const STAGE_VALUE: &str = "__cf_common_stage_value";
const STAGE_CFG: &str = "cfg";
const STAGE_DATA: &str = "data";
const STAGE_CALL: &str = "call";

type CanonicalGraph = Graph<Arc<str>, (), Directed, u32>;
type CanonicalIndex = NodeIndex<u32>;

/// Exact immutable inputs repeated on every emitted row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommonAnalysisProvenance {
    pub fabric_epoch_id: [u8; 32],
    pub source_pin: [u8; 32],
    pub input_set_pin: [u8; 32],
    pub proof_pin: [u8; 32],
    pub source_generation: u64,
}

/// One accepted canonical CFG node.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CommonCfgNode {
    pub owner_id: Arc<str>,
    pub node_id: Arc<str>,
    pub is_entry: bool,
    pub is_exit: bool,
}

/// One accepted canonical CFG edge.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CommonCfgEdge {
    pub owner_id: Arc<str>,
    pub source_node_id: Arc<str>,
    pub target_node_id: Arc<str>,
}

/// One accepted def-use or reaching-definition witness.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CommonDefUse {
    pub owner_id: Arc<str>,
    pub definition_node_id: Arc<str>,
    pub use_node_id: Arc<str>,
    pub location_id: Arc<str>,
}

/// Current call-target resolution supplied by accepted language-local facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommonCallResolution {
    Exact { callee_id: Arc<str> },
    Dynamic { reason: Arc<str> },
    Unknown { reason: Arc<str> },
}

/// One first-class call site. Syntax is not collapsed into a callable edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommonCallSite {
    pub call_site_id: Arc<str>,
    pub caller_id: Arc<str>,
    pub resolution: CommonCallResolution,
}

/// Local, application-accepted effect/resource seeds for one callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommonCallableLocalSemantics {
    pub callable_id: Arc<str>,
    pub module_id: Arc<str>,
    pub owner_id: Arc<str>,
    pub effects: BTreeSet<Arc<str>>,
    pub resources: BTreeSet<Arc<str>>,
}

/// Input relation family whose execution coverage constrains derived completeness.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CommonInputFamily {
    Cfg,
    DefUseReaching,
    CallTargets,
    LocalEffectResource,
}

/// Execution-derived input census. No status string is authoritative.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommonInputCoverage {
    pub requested_units: u64,
    pub completed_units: u64,
    pub remainder_units: u64,
    pub unknown_units: u64,
    pub execution_proof_pin: [u8; 32],
}

impl CommonInputCoverage {
    fn is_complete(&self) -> bool {
        self.requested_units == self.completed_units
            && self.remainder_units == 0
            && self.unknown_units == 0
            && self.execution_proof_pin != [0; 32]
    }
}

/// Scope of a source/model change for invalidation observation.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CommonChangedScope {
    Owner(Arc<str>),
    Module(Arc<str>),
    Callable(Arc<str>),
}

/// Complete immutable input to one common-analysis execution.
#[derive(Clone, Debug)]
pub struct CommonDerivedAnalysisInput {
    pub provenance: CommonAnalysisProvenance,
    pub cfg_nodes: Vec<CommonCfgNode>,
    pub cfg_edges: Vec<CommonCfgEdge>,
    pub def_use_reaching: Vec<CommonDefUse>,
    pub calls: Vec<CommonCallSite>,
    pub local_semantics: Vec<CommonCallableLocalSemantics>,
    pub coverage: BTreeMap<CommonInputFamily, CommonInputCoverage>,
    pub changed_scopes: BTreeSet<CommonChangedScope>,
}

/// Application-owned relation identities. Runtime logic never dispatches on their spellings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommonAnalysisRelations {
    pub cfg_nodes: RelationId,
    pub cfg_edges: RelationId,
    pub def_use_reaching: RelationId,
    pub call_targets: RelationId,
    pub local_semantics: RelationId,
    pub facts: RelationId,
    pub unknowns: RelationId,
    pub completeness: RelationId,
    pub invalidation: RelationId,
}

/// Physical field identities supplied by the admitted schema contracts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommonAnalysisFields {
    pub fabric_epoch_id: FieldId,
    pub source_pin: FieldId,
    pub input_set_pin: FieldId,
    pub proof_pin: FieldId,
    pub source_generation: FieldId,
    pub authority_id: FieldId,
    pub semantic_class_id: FieldId,
    pub algorithm_release: FieldId,
    pub precision_release: FieldId,
    pub family_id: FieldId,
    pub subject_id: FieldId,
    pub object_id: FieldId,
    pub value_id: FieldId,
    pub distance: FieldId,
    pub iteration: FieldId,
    pub complete: FieldId,
    pub reason_id: FieldId,
    pub detail: FieldId,
    pub requested_units: FieldId,
    pub completed_units: FieldId,
    pub remainder_units: FieldId,
    pub unknown_units: FieldId,
    pub execution_receipt: FieldId,
    pub scope_kind: FieldId,
    pub scope_id: FieldId,
    pub cause_id: FieldId,
}

impl CommonAnalysisFields {
    fn all(&self) -> [&FieldId; 26] {
        [
            &self.fabric_epoch_id,
            &self.source_pin,
            &self.input_set_pin,
            &self.proof_pin,
            &self.source_generation,
            &self.authority_id,
            &self.semantic_class_id,
            &self.algorithm_release,
            &self.precision_release,
            &self.family_id,
            &self.subject_id,
            &self.object_id,
            &self.value_id,
            &self.distance,
            &self.iteration,
            &self.complete,
            &self.reason_id,
            &self.detail,
            &self.requested_units,
            &self.completed_units,
            &self.remainder_units,
            &self.unknown_units,
            &self.execution_receipt,
            &self.scope_kind,
            &self.scope_id,
            &self.cause_id,
        ]
    }
}

/// Application family identities for every emitted semantic family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommonAnalysisFamilies {
    pub dominator: Arc<str>,
    pub post_dominator: Arc<str>,
    pub control_dependence: Arc<str>,
    pub data_dependence: Arc<str>,
    pub call_graph: Arc<str>,
    pub scc_membership: Arc<str>,
    pub reachability: Arc<str>,
    pub callable_effect: Arc<str>,
    pub callable_resource: Arc<str>,
    pub callable_summary: Arc<str>,
}

impl CommonAnalysisFamilies {
    fn all(&self) -> [&Arc<str>; 10] {
        [
            &self.dominator,
            &self.post_dominator,
            &self.control_dependence,
            &self.data_dependence,
            &self.call_graph,
            &self.scc_membership,
            &self.reachability,
            &self.callable_effect,
            &self.callable_resource,
            &self.callable_summary,
        ]
    }
}

/// Authority identity is typed input data, while its legal role is an exhaustive Rust contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommonAnalysisAuthority {
    ApplicationOwned(Arc<str>),
    ProviderNative(Arc<str>),
}

/// The system accepts fact semantics and rejects evaluative/judgment semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommonAnalysisSemanticClass {
    Fact(Arc<str>),
    Judgment(Arc<str>),
}

/// Complete typed binding for the common-analysis boundary.
#[derive(Clone, Debug)]
pub struct CommonAnalysisBindings {
    pub relations: CommonAnalysisRelations,
    pub fields: CommonAnalysisFields,
    pub families: CommonAnalysisFamilies,
    pub authority: CommonAnalysisAuthority,
    pub semantic_class: CommonAnalysisSemanticClass,
}

impl CommonAnalysisBindings {
    /// Validate all binding identities and the authority/semantic boundary.
    pub fn validate(&self) -> Result<(), CommonDerivedAnalysisError> {
        let mut relations = BTreeSet::new();
        for relation in [
            &self.relations.cfg_nodes,
            &self.relations.cfg_edges,
            &self.relations.def_use_reaching,
            &self.relations.call_targets,
            &self.relations.local_semantics,
            &self.relations.facts,
            &self.relations.unknowns,
            &self.relations.completeness,
            &self.relations.invalidation,
        ] {
            if !relations.insert(relation.as_str()) {
                return Err(CommonDerivedAnalysisError::DuplicateRelation(
                    relation.as_str().to_owned(),
                ));
            }
        }
        let mut fields = BTreeSet::new();
        for field in self.fields.all() {
            if !fields.insert(field.as_str()) {
                return Err(CommonDerivedAnalysisError::DuplicateField(
                    field.as_str().to_owned(),
                ));
            }
        }
        let mut families = BTreeSet::new();
        for family in self.families.all() {
            validate_text("family", family)?;
            if !families.insert(family.as_ref()) {
                return Err(CommonDerivedAnalysisError::DuplicateFamily(
                    family.to_string(),
                ));
            }
        }
        match &self.authority {
            CommonAnalysisAuthority::ApplicationOwned(identity) => {
                validate_text("application authority", identity)?;
            }
            CommonAnalysisAuthority::ProviderNative(identity) => {
                return Err(CommonDerivedAnalysisError::ProviderNativeAuthority(
                    identity.to_string(),
                ));
            }
        }
        match &self.semantic_class {
            CommonAnalysisSemanticClass::Fact(identity) => {
                validate_text("fact semantic class", identity)?;
            }
            CommonAnalysisSemanticClass::Judgment(identity) => {
                return Err(CommonDerivedAnalysisError::JudgmentSemanticClass(
                    identity.to_string(),
                ));
            }
        }
        Ok(())
    }

    fn authority_id(&self) -> &str {
        match &self.authority {
            CommonAnalysisAuthority::ApplicationOwned(identity) => identity,
            CommonAnalysisAuthority::ProviderNative(_) => unreachable!("validated authority"),
        }
    }

    fn semantic_class_id(&self) -> &str {
        match &self.semantic_class {
            CommonAnalysisSemanticClass::Fact(identity) => identity,
            CommonAnalysisSemanticClass::Judgment(_) => unreachable!("validated class"),
        }
    }
}

/// Explicit resource and convergence bounds for one execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommonAnalysisBounds {
    max_cfg_nodes: NonZeroUsize,
    max_cfg_edges: NonZeroUsize,
    max_callables: NonZeroUsize,
    max_call_sites: NonZeroUsize,
    max_reachability_pairs: NonZeroUsize,
    max_summary_values: NonZeroUsize,
    max_output_rows: NonZeroUsize,
    max_iterations: NonZeroU16,
}

impl CommonAnalysisBounds {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        max_cfg_nodes: usize,
        max_cfg_edges: usize,
        max_callables: usize,
        max_call_sites: usize,
        max_reachability_pairs: usize,
        max_summary_values: usize,
        max_output_rows: usize,
        max_iterations: u16,
    ) -> Result<Self, CommonDerivedAnalysisError> {
        Ok(Self {
            max_cfg_nodes: nonzero(max_cfg_nodes, "max_cfg_nodes")?,
            max_cfg_edges: nonzero(max_cfg_edges, "max_cfg_edges")?,
            max_callables: nonzero(max_callables, "max_callables")?,
            max_call_sites: nonzero(max_call_sites, "max_call_sites")?,
            max_reachability_pairs: nonzero(max_reachability_pairs, "max_reachability_pairs")?,
            max_summary_values: nonzero(max_summary_values, "max_summary_values")?,
            max_output_rows: nonzero(max_output_rows, "max_output_rows")?,
            max_iterations: NonZeroU16::new(max_iterations)
                .ok_or(CommonDerivedAnalysisError::ZeroBound("max_iterations"))?,
        })
    }

    #[must_use]
    pub const fn max_cfg_nodes(self) -> usize {
        self.max_cfg_nodes.get()
    }

    #[must_use]
    pub const fn max_cfg_edges(self) -> usize {
        self.max_cfg_edges.get()
    }

    #[must_use]
    pub const fn max_callables(self) -> usize {
        self.max_callables.get()
    }

    #[must_use]
    pub const fn max_call_sites(self) -> usize {
        self.max_call_sites.get()
    }

    #[must_use]
    pub const fn max_reachability_pairs(self) -> usize {
        self.max_reachability_pairs.get()
    }

    #[must_use]
    pub const fn max_summary_values(self) -> usize {
        self.max_summary_values.get()
    }

    #[must_use]
    pub const fn max_output_rows(self) -> usize {
        self.max_output_rows.get()
    }

    #[must_use]
    pub const fn max_iterations(self) -> u16 {
        self.max_iterations.get()
    }
}

/// Library/API operations causally used by this execution.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CommonAnalysisNativeOperation {
    DataFusionProjection,
    DataFusionDistinct,
    DataFusionDeterministicSort,
    PetgraphGraphExternalIdMap,
    PetgraphSimpleFastDominators,
    PetgraphReversedPostDominators,
    PetgraphTarjanScc,
    PetgraphCondensationForCycles,
    PetgraphTopologicalSchedule,
    BoundedReachability,
    MonotoneSetUnionFixedPoint,
}

/// Input relation dependency observed by compilation/execution.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CommonAnalysisDependency {
    InputRelation(RelationId),
    OutputRelation(RelationId),
    Field(FieldId),
    Family(Arc<str>),
    FabricEpoch([u8; 32]),
    SourcePin([u8; 32]),
    InputSetPin([u8; 32]),
    ProofPin([u8; 32]),
    InputExecutionProof(CommonInputFamily, [u8; 32]),
    Authority(Arc<str>),
}

/// Causal execution observation, not a hand-maintained capability declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommonAnalysisObservation {
    pub operations: BTreeSet<CommonAnalysisNativeOperation>,
    pub dependencies: BTreeSet<CommonAnalysisDependency>,
    pub bounds: CommonAnalysisBounds,
    pub iterations: u16,
    pub used_condensation: bool,
    pub invalidated_callables: BTreeSet<Arc<str>>,
}

/// Four typed Arrow relations emitted by one bounded execution.
#[derive(Clone, Debug)]
pub struct CommonDerivedAnalysisOutput {
    pub facts: RecordBatch,
    pub unknowns: RecordBatch,
    pub completeness: RecordBatch,
    pub invalidation: RecordBatch,
    pub observation: CommonAnalysisObservation,
}

/// Exact clean-vs-incremental equivalence result over semantic output relations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommonAnalysisEquivalence {
    pub equivalent: bool,
    pub clean_digest: [u8; 32],
    pub incremental_digest: [u8; 32],
}

/// Compare clean and incremental semantic results. Invalidation observations are control data and
/// are intentionally excluded from semantic equivalence.
pub fn compare_clean_incremental(
    clean: &CommonDerivedAnalysisOutput,
    incremental: &CommonDerivedAnalysisOutput,
) -> Result<CommonAnalysisEquivalence, CommonDerivedAnalysisError> {
    let clean_digest = semantic_output_digest(clean)?;
    let incremental_digest = semantic_output_digest(incremental)?;
    Ok(CommonAnalysisEquivalence {
        equivalent: clean_digest == incremental_digest,
        clean_digest,
        incremental_digest,
    })
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FactRow {
    family: Arc<str>,
    subject: Arc<str>,
    object: Option<Arc<str>>,
    value: Option<Arc<str>>,
    distance: Option<u32>,
    algorithm: &'static str,
    precision: &'static str,
    iteration: u32,
    complete: bool,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct UnknownRow {
    family: Arc<str>,
    subject: Option<Arc<str>>,
    reason: Arc<str>,
    detail: Option<Arc<str>>,
    iteration: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Progress {
    requested: u64,
    completed: u64,
    remainder: u64,
    unknown: u64,
}

impl Progress {
    const fn complete(self) -> bool {
        self.requested == self.completed && self.remainder == 0 && self.unknown == 0
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct InvalidationRow {
    family: Arc<str>,
    scope_kind: &'static str,
    scope_id: Arc<str>,
    cause: &'static str,
}

#[derive(Clone, Debug)]
struct NormalizedStageRow {
    kind: Arc<str>,
    scope: Arc<str>,
    source: Arc<str>,
    target: Arc<str>,
    value: Option<Arc<str>>,
}

#[derive(Default)]
struct AnalysisState {
    facts: BTreeSet<FactRow>,
    unknowns: BTreeSet<UnknownRow>,
    progress: BTreeMap<Arc<str>, Progress>,
    invalidation: BTreeSet<InvalidationRow>,
    operations: BTreeSet<CommonAnalysisNativeOperation>,
    iterations: u16,
    used_condensation: bool,
    output_exhausted_families: BTreeSet<Arc<str>>,
}

impl AnalysisState {
    fn insert_fact(&mut self, row: FactRow, bounds: CommonAnalysisBounds) {
        if self.facts.len() < bounds.max_output_rows.get() {
            self.facts.insert(row);
        } else {
            self.output_exhausted_families
                .insert(Arc::clone(&row.family));
        }
    }

    fn unknown(
        &mut self,
        family: &Arc<str>,
        subject: Option<Arc<str>>,
        reason: impl Into<Arc<str>>,
        detail: Option<Arc<str>>,
        iteration: u32,
    ) {
        self.unknowns.insert(UnknownRow {
            family: Arc::clone(family),
            subject,
            reason: reason.into(),
            detail,
            iteration,
        });
    }
}

/// Closed errors at the common-analysis boundary.
#[derive(Debug, Error)]
pub enum CommonDerivedAnalysisError {
    #[error("duplicate relation identity {0:?}")]
    DuplicateRelation(String),
    #[error("duplicate field identity {0:?}")]
    DuplicateField(String),
    #[error("duplicate family identity {0:?}")]
    DuplicateFamily(String),
    #[error("invalid {kind} identity {value:?}")]
    InvalidIdentity { kind: &'static str, value: String },
    #[error("provider-native authority cannot own common derived analyses: {0}")]
    ProviderNativeAuthority(String),
    #[error("judgment/evaluative semantics are excluded from the fact substrate: {0}")]
    JudgmentSemanticClass(String),
    #[error("resource bound {0} must be non-zero")]
    ZeroBound(&'static str),
    #[error("invalid provenance pin {0}")]
    InvalidProvenance(&'static str),
    #[error("missing execution coverage for {0:?}")]
    MissingCoverage(CommonInputFamily),
    #[error("contradictory execution coverage for {0:?}")]
    InvalidCoverage(CommonInputFamily),
    #[error("invalid common-analysis input: {0}")]
    InvalidInput(String),
    #[error("DataFusion staging row uses unknown kind {0:?}")]
    UnknownStageKind(String),
    #[error("Arrow schema or array mismatch: {0}")]
    Schema(String),
    #[error(transparent)]
    Arrow(#[from] ArrowError),
    #[error(transparent)]
    DataFusion(#[from] datafusion::error::DataFusionError),
}

fn nonzero(value: usize, name: &'static str) -> Result<NonZeroUsize, CommonDerivedAnalysisError> {
    NonZeroUsize::new(value).ok_or(CommonDerivedAnalysisError::ZeroBound(name))
}

fn validate_text(kind: &'static str, value: &str) -> Result<(), CommonDerivedAnalysisError> {
    if value.is_empty()
        || value.len() > 1_024
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(CommonDerivedAnalysisError::InvalidIdentity {
            kind,
            value: value.to_owned(),
        });
    }
    Ok(())
}

/// Execute the bounded common analysis in one caller-owned DataFusion session.
pub async fn analyze_common_derived(
    context: &SessionContext,
    input: &CommonDerivedAnalysisInput,
    bindings: &CommonAnalysisBindings,
    bounds: CommonAnalysisBounds,
) -> Result<CommonDerivedAnalysisOutput, CommonDerivedAnalysisError> {
    bindings.validate()?;
    validate_provenance(&input.provenance)?;
    validate_input(input)?;
    validate_coverage(&input.coverage)?;

    let mut state = AnalysisState::default();
    state.operations.extend([
        CommonAnalysisNativeOperation::DataFusionProjection,
        CommonAnalysisNativeOperation::DataFusionDistinct,
        CommonAnalysisNativeOperation::DataFusionDeterministicSort,
        CommonAnalysisNativeOperation::PetgraphGraphExternalIdMap,
    ]);
    for family in bindings.families.all() {
        state
            .progress
            .insert(Arc::clone(family), Progress::default());
    }

    let staged = normalize_stage(context, input).await?;
    let (cfg_edges, data_rows, exact_calls) = split_stage(staged)?;
    derive_cfg_families(input, &cfg_edges, bindings, bounds, &mut state)?;
    derive_data_dependence(input, &data_rows, bindings, bounds, &mut state);
    let call_result = derive_call_families(input, &exact_calls, bindings, bounds, &mut state)?;
    derive_invalidation(input, &call_result.reverse_callers, bindings, &mut state);
    apply_upstream_coverage(input, bindings, &mut state)?;

    for family in state
        .output_exhausted_families
        .iter()
        .cloned()
        .collect::<Vec<_>>()
    {
        state.unknown(
            &family,
            None,
            "OUTPUT_ROW_BOUND_EXHAUSTED",
            Some(Arc::from(
                "semantic fact output exceeded the configured row bound",
            )),
            u32::from(state.iterations),
        );
        let progress = state.progress.entry(family).or_default();
        progress.requested = progress.requested.saturating_add(1);
        progress.unknown = progress.unknown.saturating_add(1);
    }

    let facts = native_sort_output(
        context,
        fact_batch(&input.provenance, bindings, &state.facts)?,
        &[
            bindings.fields.family_id.as_str(),
            bindings.fields.subject_id.as_str(),
            bindings.fields.object_id.as_str(),
            bindings.fields.value_id.as_str(),
            bindings.fields.distance.as_str(),
        ],
        "__codefabric_common_facts",
    )
    .await?;
    let unknowns = native_sort_output(
        context,
        unknown_batch(&input.provenance, bindings, &state.unknowns)?,
        &[
            bindings.fields.family_id.as_str(),
            bindings.fields.subject_id.as_str(),
            bindings.fields.reason_id.as_str(),
            bindings.fields.detail.as_str(),
        ],
        "__codefabric_common_unknowns",
    )
    .await?;
    let completeness = native_sort_output(
        context,
        completeness_batch(
            &input.provenance,
            bindings,
            &input.coverage,
            &state.progress,
        )?,
        &[bindings.fields.family_id.as_str()],
        "__codefabric_common_completeness",
    )
    .await?;
    let invalidation = native_sort_output(
        context,
        invalidation_batch(&input.provenance, bindings, &state.invalidation)?,
        &[
            bindings.fields.family_id.as_str(),
            bindings.fields.scope_kind.as_str(),
            bindings.fields.scope_id.as_str(),
        ],
        "__codefabric_common_invalidation",
    )
    .await?;

    let invalidated_callables = state
        .invalidation
        .iter()
        .filter(|row| row.scope_kind == "callable")
        .map(|row| Arc::clone(&row.scope_id))
        .collect();
    Ok(CommonDerivedAnalysisOutput {
        facts,
        unknowns,
        completeness,
        invalidation,
        observation: CommonAnalysisObservation {
            operations: state.operations,
            dependencies: observe_dependencies(input, bindings),
            bounds,
            iterations: state.iterations,
            used_condensation: state.used_condensation,
            invalidated_callables,
        },
    })
}

fn validate_provenance(
    provenance: &CommonAnalysisProvenance,
) -> Result<(), CommonDerivedAnalysisError> {
    for (name, value) in [
        ("fabric_epoch_id", provenance.fabric_epoch_id),
        ("source_pin", provenance.source_pin),
        ("input_set_pin", provenance.input_set_pin),
        ("proof_pin", provenance.proof_pin),
    ] {
        if value == [0; 32] {
            return Err(CommonDerivedAnalysisError::InvalidProvenance(name));
        }
    }
    Ok(())
}

fn validate_input(input: &CommonDerivedAnalysisInput) -> Result<(), CommonDerivedAnalysisError> {
    let mut nodes = BTreeSet::new();
    for node in &input.cfg_nodes {
        validate_text("owner", &node.owner_id)?;
        validate_text("CFG node", &node.node_id)?;
        if !nodes.insert((node.owner_id.as_ref(), node.node_id.as_ref())) {
            return Err(CommonDerivedAnalysisError::InvalidInput(format!(
                "duplicate CFG node {}/{}",
                node.owner_id, node.node_id
            )));
        }
    }
    for edge in &input.cfg_edges {
        validate_text("owner", &edge.owner_id)?;
        validate_text("CFG source", &edge.source_node_id)?;
        validate_text("CFG target", &edge.target_node_id)?;
        if !nodes.contains(&(edge.owner_id.as_ref(), edge.source_node_id.as_ref()))
            || !nodes.contains(&(edge.owner_id.as_ref(), edge.target_node_id.as_ref()))
        {
            return Err(CommonDerivedAnalysisError::InvalidInput(format!(
                "CFG edge {} -> {} escapes owner {}",
                edge.source_node_id, edge.target_node_id, edge.owner_id
            )));
        }
    }
    for row in &input.def_use_reaching {
        for (kind, value) in [
            ("owner", &row.owner_id),
            ("definition node", &row.definition_node_id),
            ("use node", &row.use_node_id),
            ("location", &row.location_id),
        ] {
            validate_text(kind, value)?;
        }
    }
    let mut caller_by_call_site = BTreeMap::new();
    let mut call_targets = BTreeSet::new();
    for call in &input.calls {
        validate_text("call site", &call.call_site_id)?;
        validate_text("caller", &call.caller_id)?;
        if let Some(caller) =
            caller_by_call_site.insert(call.call_site_id.as_ref(), call.caller_id.as_ref())
            && caller != call.caller_id.as_ref()
        {
            return Err(CommonDerivedAnalysisError::InvalidInput(format!(
                "call site {} has conflicting callers",
                call.call_site_id
            )));
        }
        let (resolution_kind, resolution_value) = match &call.resolution {
            CommonCallResolution::Exact { callee_id } => {
                validate_text("callee", callee_id)?;
                ("exact", callee_id.as_ref())
            }
            CommonCallResolution::Dynamic { reason } | CommonCallResolution::Unknown { reason } => {
                validate_text("call gap", reason)?;
                (
                    if matches!(&call.resolution, CommonCallResolution::Dynamic { .. }) {
                        "dynamic"
                    } else {
                        "unknown"
                    },
                    reason.as_ref(),
                )
            }
        };
        if !call_targets.insert((
            call.call_site_id.as_ref(),
            resolution_kind,
            resolution_value,
        )) {
            return Err(CommonDerivedAnalysisError::InvalidInput(format!(
                "duplicate call target observation for {}",
                call.call_site_id
            )));
        }
    }
    let mut callables = BTreeSet::new();
    for local in &input.local_semantics {
        for (kind, value) in [
            ("callable", &local.callable_id),
            ("module", &local.module_id),
            ("owner", &local.owner_id),
        ] {
            validate_text(kind, value)?;
        }
        if !callables.insert(local.callable_id.as_ref()) {
            return Err(CommonDerivedAnalysisError::InvalidInput(format!(
                "duplicate callable local semantics {}",
                local.callable_id
            )));
        }
        for effect in &local.effects {
            validate_text("effect", effect)?;
        }
        for resource in &local.resources {
            validate_text("resource", resource)?;
        }
    }
    Ok(())
}

fn validate_coverage(
    coverage: &BTreeMap<CommonInputFamily, CommonInputCoverage>,
) -> Result<(), CommonDerivedAnalysisError> {
    for family in [
        CommonInputFamily::Cfg,
        CommonInputFamily::DefUseReaching,
        CommonInputFamily::CallTargets,
        CommonInputFamily::LocalEffectResource,
    ] {
        let row = coverage
            .get(&family)
            .ok_or(CommonDerivedAnalysisError::MissingCoverage(family))?;
        let classified = row
            .completed_units
            .checked_add(row.remainder_units)
            .and_then(|value| value.checked_add(row.unknown_units))
            .ok_or(CommonDerivedAnalysisError::InvalidCoverage(family))?;
        if classified != row.requested_units || row.execution_proof_pin == [0; 32] {
            return Err(CommonDerivedAnalysisError::InvalidCoverage(family));
        }
    }
    Ok(())
}

fn staging_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new(STAGE_KIND, DataType::Utf8, false),
        Field::new(STAGE_SCOPE, DataType::Utf8, false),
        Field::new(STAGE_SOURCE, DataType::Utf8, false),
        Field::new(STAGE_TARGET, DataType::Utf8, false),
        Field::new(STAGE_VALUE, DataType::Utf8, true),
    ]))
}

async fn normalize_stage(
    context: &SessionContext,
    input: &CommonDerivedAnalysisInput,
) -> Result<Vec<NormalizedStageRow>, CommonDerivedAnalysisError> {
    let mut rows = Vec::new();
    rows.extend(input.cfg_edges.iter().map(|edge| NormalizedStageRow {
        kind: Arc::from(STAGE_CFG),
        scope: Arc::clone(&edge.owner_id),
        source: Arc::clone(&edge.source_node_id),
        target: Arc::clone(&edge.target_node_id),
        value: None,
    }));
    rows.extend(input.def_use_reaching.iter().map(|row| NormalizedStageRow {
        kind: Arc::from(STAGE_DATA),
        scope: Arc::clone(&row.owner_id),
        source: Arc::clone(&row.definition_node_id),
        target: Arc::clone(&row.use_node_id),
        value: Some(Arc::clone(&row.location_id)),
    }));
    rows.extend(input.calls.iter().filter_map(|call| {
        let CommonCallResolution::Exact { callee_id } = &call.resolution else {
            return None;
        };
        Some(NormalizedStageRow {
            kind: Arc::from(STAGE_CALL),
            scope: Arc::from("call-graph"),
            source: Arc::clone(&call.caller_id),
            target: Arc::clone(callee_id),
            value: Some(Arc::clone(&call.call_site_id)),
        })
    }));
    let batch = staging_batch(&rows)?;
    let schema = staging_schema();
    let provider = Arc::new(MemTable::try_new(Arc::clone(&schema), vec![vec![batch]])?);
    let plan = LogicalPlanBuilder::scan(
        "__codefabric_common_stage",
        provider_as_source(provider),
        None,
    )?
    .project([
        col(STAGE_KIND),
        col(STAGE_SCOPE),
        col(STAGE_SOURCE),
        col(STAGE_TARGET),
        col(STAGE_VALUE),
    ])?
    .distinct()?
    .sort([
        col(STAGE_KIND).sort(true, false),
        col(STAGE_SCOPE).sort(true, false),
        col(STAGE_SOURCE).sort(true, false),
        col(STAGE_TARGET).sort(true, false),
        col(STAGE_VALUE).sort(true, true),
    ])?
    .build()?;
    let batches = context.execute_logical_plan(plan).await?.collect().await?;
    let batch = if batches.is_empty() {
        RecordBatch::new_empty(Arc::clone(&schema))
    } else {
        concat_batches(&schema, &batches)?
    };
    parse_staging_batch(&batch)
}

fn staging_batch(rows: &[NormalizedStageRow]) -> Result<RecordBatch, ArrowError> {
    RecordBatch::try_new(
        staging_schema(),
        vec![
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.kind.as_ref()),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.scope.as_ref()),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.source.as_ref()),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.target.as_ref()),
            )),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| row.value.as_deref())
                    .collect::<Vec<_>>(),
            )),
        ],
    )
}

fn parse_staging_batch(
    batch: &RecordBatch,
) -> Result<Vec<NormalizedStageRow>, CommonDerivedAnalysisError> {
    let kinds = string_column(batch, STAGE_KIND)?;
    let scopes = string_column(batch, STAGE_SCOPE)?;
    let sources = string_column(batch, STAGE_SOURCE)?;
    let targets = string_column(batch, STAGE_TARGET)?;
    let values = string_column(batch, STAGE_VALUE)?;
    Ok((0..batch.num_rows())
        .map(|row| NormalizedStageRow {
            kind: Arc::from(kinds.value(row)),
            scope: Arc::from(scopes.value(row)),
            source: Arc::from(sources.value(row)),
            target: Arc::from(targets.value(row)),
            value: (!values.is_null(row)).then(|| Arc::from(values.value(row))),
        })
        .collect())
}

fn string_column<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a StringArray, CommonDerivedAnalysisError> {
    batch
        .column_by_name(name)
        .ok_or_else(|| CommonDerivedAnalysisError::Schema(format!("missing column {name:?}")))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| CommonDerivedAnalysisError::Schema(format!("column {name:?} is not Utf8")))
}

type NormalizedTriples = (
    Vec<CommonCfgEdge>,
    Vec<CommonDefUse>,
    Vec<(Arc<str>, Arc<str>, Arc<str>)>,
);

fn split_stage(
    rows: Vec<NormalizedStageRow>,
) -> Result<NormalizedTriples, CommonDerivedAnalysisError> {
    let mut cfg = Vec::new();
    let mut data = Vec::new();
    let mut calls = Vec::new();
    for row in rows {
        match row.kind.as_ref() {
            STAGE_CFG => cfg.push(CommonCfgEdge {
                owner_id: row.scope,
                source_node_id: row.source,
                target_node_id: row.target,
            }),
            STAGE_DATA => data.push(CommonDefUse {
                owner_id: row.scope,
                definition_node_id: row.source,
                use_node_id: row.target,
                location_id: row.value.ok_or_else(|| {
                    CommonDerivedAnalysisError::Schema("data stage row has no location".to_owned())
                })?,
            }),
            STAGE_CALL => calls.push((
                row.source,
                row.target,
                row.value.ok_or_else(|| {
                    CommonDerivedAnalysisError::Schema("call stage row has no site".to_owned())
                })?,
            )),
            other => {
                return Err(CommonDerivedAnalysisError::UnknownStageKind(
                    other.to_owned(),
                ));
            }
        }
    }
    Ok((cfg, data, calls))
}

fn derive_cfg_families(
    input: &CommonDerivedAnalysisInput,
    edges: &[CommonCfgEdge],
    bindings: &CommonAnalysisBindings,
    bounds: CommonAnalysisBounds,
    state: &mut AnalysisState,
) -> Result<(), CommonDerivedAnalysisError> {
    state.operations.extend([
        CommonAnalysisNativeOperation::PetgraphSimpleFastDominators,
        CommonAnalysisNativeOperation::PetgraphReversedPostDominators,
    ]);
    let mut nodes_by_owner: BTreeMap<Arc<str>, Vec<&CommonCfgNode>> = BTreeMap::new();
    for node in &input.cfg_nodes {
        nodes_by_owner
            .entry(Arc::clone(&node.owner_id))
            .or_default()
            .push(node);
    }
    let mut edges_by_owner: BTreeMap<Arc<str>, Vec<&CommonCfgEdge>> = BTreeMap::new();
    for edge in edges {
        edges_by_owner
            .entry(Arc::clone(&edge.owner_id))
            .or_default()
            .push(edge);
    }
    let cfg_coverage_complete = input
        .coverage
        .get(&CommonInputFamily::Cfg)
        .is_some_and(CommonInputCoverage::is_complete);

    let total_nodes = u64::try_from(input.cfg_nodes.len()).unwrap_or(u64::MAX);
    let total_edges = u64::try_from(edges.len()).unwrap_or(u64::MAX);
    for family in [
        &bindings.families.dominator,
        &bindings.families.post_dominator,
    ] {
        state.progress.insert(
            Arc::clone(family),
            Progress {
                requested: total_nodes,
                ..Progress::default()
            },
        );
    }
    state.progress.insert(
        Arc::clone(&bindings.families.control_dependence),
        Progress {
            requested: total_edges,
            ..Progress::default()
        },
    );

    if input.cfg_nodes.len() > bounds.max_cfg_nodes.get()
        || edges.len() > bounds.max_cfg_edges.get()
    {
        for family in [
            &bindings.families.dominator,
            &bindings.families.post_dominator,
            &bindings.families.control_dependence,
        ] {
            state.unknown(
                family,
                None,
                "CFG_RESOURCE_BOUND_EXHAUSTED",
                Some(Arc::from(format!(
                    "nodes={} edges={} node_bound={} edge_bound={}",
                    input.cfg_nodes.len(),
                    edges.len(),
                    bounds.max_cfg_nodes,
                    bounds.max_cfg_edges
                ))),
                0,
            );
            let progress = state
                .progress
                .get_mut(family.as_ref())
                .expect("initialized family");
            progress.unknown = progress.requested;
        }
        return Ok(());
    }

    for (owner, mut owner_nodes) in nodes_by_owner {
        owner_nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id));
        let owner_edges = edges_by_owner.remove(&owner).unwrap_or_default();
        let entries = owner_nodes
            .iter()
            .filter(|node| node.is_entry)
            .copied()
            .collect::<Vec<_>>();
        let exits = owner_nodes
            .iter()
            .filter(|node| node.is_exit)
            .copied()
            .collect::<Vec<_>>();
        if entries.len() != 1 || exits.is_empty() {
            for family in [
                &bindings.families.dominator,
                &bindings.families.post_dominator,
                &bindings.families.control_dependence,
            ] {
                state.unknown(
                    family,
                    Some(Arc::clone(&owner)),
                    "CFG_ROOT_OR_EXIT_UNKNOWN",
                    Some(Arc::from(format!(
                        "entry_count={} exit_count={}",
                        entries.len(),
                        exits.len()
                    ))),
                    0,
                );
            }
            let node_count = u64::try_from(owner_nodes.len()).unwrap_or(u64::MAX);
            let edge_count = u64::try_from(owner_edges.len()).unwrap_or(u64::MAX);
            state
                .progress
                .get_mut(&bindings.families.dominator)
                .expect("family")
                .unknown += node_count;
            state
                .progress
                .get_mut(&bindings.families.post_dominator)
                .expect("family")
                .unknown += node_count;
            state
                .progress
                .get_mut(&bindings.families.control_dependence)
                .expect("family")
                .unknown += edge_count;
            continue;
        }

        let (graph, index_by_id) = build_cfg_graph(&owner_nodes, &owner_edges)?;
        let entry = index_by_id[entries[0].node_id.as_ref()];
        let dominators = simple_fast(&graph, entry);
        for node in &owner_nodes {
            let index = index_by_id[node.node_id.as_ref()];
            let Some(strict) = dominators.strict_dominators(index) else {
                state.unknown(
                    &bindings.families.dominator,
                    Some(Arc::clone(&node.node_id)),
                    "CFG_NODE_UNREACHABLE_FROM_ENTRY",
                    Some(Arc::clone(&owner)),
                    0,
                );
                state
                    .progress
                    .get_mut(&bindings.families.dominator)
                    .expect("family")
                    .unknown += 1;
                continue;
            };
            for dominator in strict {
                state.insert_fact(
                    FactRow {
                        family: Arc::clone(&bindings.families.dominator),
                        subject: Arc::clone(&node.node_id),
                        object: Some(Arc::clone(&graph[dominator])),
                        value: None,
                        distance: None,
                        algorithm: COMMON_CFG_ANALYSIS_RELEASE,
                        precision: COMMON_CFG_PRECISION,
                        iteration: 0,
                        complete: cfg_coverage_complete,
                    },
                    bounds,
                );
            }
            state
                .progress
                .get_mut(&bindings.families.dominator)
                .expect("family")
                .completed += 1;
        }

        let mut post_graph = graph.clone();
        // Empty text is rejected for binding identities, so this graph-local sentinel cannot
        // collide with a canonical node and is never emitted.
        let virtual_exit = post_graph.add_node(Arc::from(""));
        for exit in &exits {
            post_graph.add_edge(index_by_id[exit.node_id.as_ref()], virtual_exit, ());
        }
        let post_dominators = simple_fast(Reversed(&post_graph), virtual_exit);
        let original_indices = owner_nodes
            .iter()
            .map(|node| index_by_id[node.node_id.as_ref()])
            .collect::<BTreeSet<_>>();
        for node in &owner_nodes {
            let index = index_by_id[node.node_id.as_ref()];
            let Some(strict) = post_dominators.strict_dominators(index) else {
                state.unknown(
                    &bindings.families.post_dominator,
                    Some(Arc::clone(&node.node_id)),
                    "CFG_NODE_CANNOT_REACH_EXIT",
                    Some(Arc::clone(&owner)),
                    0,
                );
                state
                    .progress
                    .get_mut(&bindings.families.post_dominator)
                    .expect("family")
                    .unknown += 1;
                continue;
            };
            for post_dominator in strict.filter(|candidate| original_indices.contains(candidate)) {
                state.insert_fact(
                    FactRow {
                        family: Arc::clone(&bindings.families.post_dominator),
                        subject: Arc::clone(&node.node_id),
                        object: Some(Arc::clone(&post_graph[post_dominator])),
                        value: None,
                        distance: None,
                        algorithm: COMMON_CFG_ANALYSIS_RELEASE,
                        precision: COMMON_CFG_PRECISION,
                        iteration: 0,
                        complete: cfg_coverage_complete,
                    },
                    bounds,
                );
            }
            state
                .progress
                .get_mut(&bindings.families.post_dominator)
                .expect("family")
                .completed += 1;
        }

        for edge in &owner_edges {
            let controller = index_by_id[edge.source_node_id.as_ref()];
            let successor = index_by_id[edge.target_node_id.as_ref()];
            let post_dominates_controller = post_dominators
                .dominators(controller)
                .is_some_and(|mut iter| iter.any(|candidate| candidate == successor));
            if !post_dominates_controller {
                let stop = post_dominators.immediate_dominator(controller);
                let mut runner = Some(successor);
                let mut seen = BTreeSet::new();
                while let Some(current) = runner {
                    if Some(current) == stop || current == virtual_exit || !seen.insert(current) {
                        break;
                    }
                    state.insert_fact(
                        FactRow {
                            family: Arc::clone(&bindings.families.control_dependence),
                            subject: Arc::clone(&post_graph[current]),
                            object: Some(Arc::clone(&post_graph[controller])),
                            value: None,
                            distance: None,
                            algorithm: COMMON_CFG_ANALYSIS_RELEASE,
                            precision: COMMON_CFG_PRECISION,
                            iteration: 0,
                            complete: cfg_coverage_complete,
                        },
                        bounds,
                    );
                    runner = post_dominators.immediate_dominator(current);
                }
            }
            state
                .progress
                .get_mut(&bindings.families.control_dependence)
                .expect("family")
                .completed += 1;
        }
    }
    Ok(())
}

fn build_cfg_graph(
    nodes: &[&CommonCfgNode],
    edges: &[&CommonCfgEdge],
) -> Result<(CanonicalGraph, BTreeMap<Arc<str>, CanonicalIndex>), CommonDerivedAnalysisError> {
    let mut graph = CanonicalGraph::with_capacity(nodes.len(), edges.len());
    let mut index_by_id = BTreeMap::new();
    for node in nodes {
        let index = graph.add_node(Arc::clone(&node.node_id));
        index_by_id.insert(Arc::clone(&node.node_id), index);
    }
    for edge in edges {
        let source = index_by_id
            .get(edge.source_node_id.as_ref())
            .copied()
            .ok_or_else(|| CommonDerivedAnalysisError::InvalidInput("missing CFG source".into()))?;
        let target = index_by_id
            .get(edge.target_node_id.as_ref())
            .copied()
            .ok_or_else(|| CommonDerivedAnalysisError::InvalidInput("missing CFG target".into()))?;
        graph.update_edge(source, target, ());
    }
    Ok((graph, index_by_id))
}

fn derive_data_dependence(
    input: &CommonDerivedAnalysisInput,
    rows: &[CommonDefUse],
    bindings: &CommonAnalysisBindings,
    bounds: CommonAnalysisBounds,
    state: &mut AnalysisState,
) {
    let requested = u64::try_from(rows.len()).unwrap_or(u64::MAX);
    state.progress.insert(
        Arc::clone(&bindings.families.data_dependence),
        Progress {
            requested,
            completed: requested,
            ..Progress::default()
        },
    );
    let coverage_complete = input
        .coverage
        .get(&CommonInputFamily::DefUseReaching)
        .is_some_and(CommonInputCoverage::is_complete);
    for row in rows {
        state.insert_fact(
            FactRow {
                family: Arc::clone(&bindings.families.data_dependence),
                subject: Arc::clone(&row.use_node_id),
                object: Some(Arc::clone(&row.definition_node_id)),
                value: Some(Arc::clone(&row.location_id)),
                distance: None,
                algorithm: COMMON_DATA_DEPENDENCE_RELEASE,
                precision: COMMON_DATA_DEPENDENCE_PRECISION,
                iteration: 0,
                complete: coverage_complete,
            },
            bounds,
        );
    }
}

struct CallDerivationResult {
    reverse_callers: BTreeMap<Arc<str>, BTreeSet<Arc<str>>>,
}

#[allow(clippy::too_many_lines)]
fn derive_call_families(
    input: &CommonDerivedAnalysisInput,
    exact_calls: &[(Arc<str>, Arc<str>, Arc<str>)],
    bindings: &CommonAnalysisBindings,
    bounds: CommonAnalysisBounds,
    state: &mut AnalysisState,
) -> Result<CallDerivationResult, CommonDerivedAnalysisError> {
    state.operations.extend([
        CommonAnalysisNativeOperation::PetgraphTarjanScc,
        CommonAnalysisNativeOperation::PetgraphTopologicalSchedule,
        CommonAnalysisNativeOperation::BoundedReachability,
        CommonAnalysisNativeOperation::MonotoneSetUnionFixedPoint,
    ]);

    let mut callables = BTreeSet::new();
    for local in &input.local_semantics {
        callables.insert(Arc::clone(&local.callable_id));
    }
    for call in &input.calls {
        callables.insert(Arc::clone(&call.caller_id));
        if let CommonCallResolution::Exact { callee_id } = &call.resolution {
            callables.insert(Arc::clone(callee_id));
        }
    }
    let callable_count = u64::try_from(callables.len()).unwrap_or(u64::MAX);
    let call_count = u64::try_from(input.calls.len()).unwrap_or(u64::MAX);
    let unresolved_count = u64::try_from(
        input
            .calls
            .iter()
            .filter(|call| !matches!(call.resolution, CommonCallResolution::Exact { .. }))
            .count(),
    )
    .unwrap_or(u64::MAX);
    let call_coverage_complete = input
        .coverage
        .get(&CommonInputFamily::CallTargets)
        .is_some_and(CommonInputCoverage::is_complete);
    let local_coverage_complete = input
        .coverage
        .get(&CommonInputFamily::LocalEffectResource)
        .is_some_and(CommonInputCoverage::is_complete);
    let topology_complete = unresolved_count == 0 && call_coverage_complete;
    state.progress.insert(
        Arc::clone(&bindings.families.call_graph),
        Progress {
            requested: call_count,
            ..Progress::default()
        },
    );
    for family in [
        &bindings.families.scc_membership,
        &bindings.families.reachability,
    ] {
        state.progress.insert(
            Arc::clone(family),
            Progress {
                requested: callable_count.saturating_add(unresolved_count),
                ..Progress::default()
            },
        );
    }
    for family in [
        &bindings.families.callable_effect,
        &bindings.families.callable_resource,
        &bindings.families.callable_summary,
    ] {
        state.progress.insert(
            Arc::clone(family),
            Progress {
                requested: callable_count,
                ..Progress::default()
            },
        );
    }

    if callables.len() > bounds.max_callables.get()
        || input.calls.len() > bounds.max_call_sites.get()
    {
        for family in [
            &bindings.families.call_graph,
            &bindings.families.scc_membership,
            &bindings.families.reachability,
            &bindings.families.callable_effect,
            &bindings.families.callable_resource,
            &bindings.families.callable_summary,
        ] {
            state.unknown(
                family,
                None,
                "CALL_GRAPH_RESOURCE_BOUND_EXHAUSTED",
                Some(Arc::from(format!(
                    "callables={} call_sites={} callable_bound={} call_site_bound={}",
                    callables.len(),
                    input.calls.len(),
                    bounds.max_callables,
                    bounds.max_call_sites
                ))),
                0,
            );
            let progress = state.progress.get_mut(family.as_ref()).expect("family");
            progress.unknown = progress.requested;
        }
        return Ok(CallDerivationResult {
            reverse_callers: BTreeMap::new(),
        });
    }

    let (graph, index_by_id) = build_call_graph(&callables, exact_calls)?;
    let mut adjacency: BTreeMap<Arc<str>, BTreeSet<Arc<str>>> = callables
        .iter()
        .map(|callable| (Arc::clone(callable), BTreeSet::new()))
        .collect();
    let mut reverse_callers: BTreeMap<Arc<str>, BTreeSet<Arc<str>>> = callables
        .iter()
        .map(|callable| (Arc::clone(callable), BTreeSet::new()))
        .collect();
    for (caller, callee, call_site) in exact_calls {
        adjacency
            .get_mut(caller.as_ref())
            .expect("graph contains caller")
            .insert(Arc::clone(callee));
        reverse_callers
            .get_mut(callee.as_ref())
            .expect("graph contains callee")
            .insert(Arc::clone(caller));
        state.insert_fact(
            FactRow {
                family: Arc::clone(&bindings.families.call_graph),
                subject: Arc::clone(caller),
                object: Some(Arc::clone(callee)),
                value: Some(Arc::clone(call_site)),
                distance: None,
                algorithm: COMMON_CALL_GRAPH_RELEASE,
                precision: COMMON_CALL_GRAPH_PRECISION,
                iteration: 0,
                complete: true,
            },
            bounds,
        );
        state
            .progress
            .get_mut(&bindings.families.call_graph)
            .expect("family")
            .completed += 1;
    }

    let mut incomplete = BTreeSet::new();
    for call in &input.calls {
        let (reason, detail) = match &call.resolution {
            CommonCallResolution::Exact { .. } => continue,
            CommonCallResolution::Dynamic { reason } => (
                Arc::from("DYNAMIC_DISPATCH_TARGET_UNKNOWN"),
                Arc::clone(reason),
            ),
            CommonCallResolution::Unknown { reason } => (Arc::clone(reason), Arc::clone(reason)),
        };
        incomplete.insert(Arc::clone(&call.caller_id));
        state.unknown(
            &bindings.families.call_graph,
            Some(Arc::clone(&call.call_site_id)),
            Arc::clone(&reason),
            Some(detail),
            0,
        );
        state
            .progress
            .get_mut(&bindings.families.call_graph)
            .expect("family")
            .unknown += 1;
        for family in [
            &bindings.families.scc_membership,
            &bindings.families.reachability,
        ] {
            state.unknown(
                family,
                Some(Arc::clone(&call.call_site_id)),
                Arc::clone(&reason),
                Some(Arc::from("call topology is incomplete")),
                0,
            );
            state
                .progress
                .get_mut(family.as_ref())
                .expect("family")
                .unknown += 1;
        }
    }
    if !call_coverage_complete || !local_coverage_complete {
        incomplete.extend(callables.iter().cloned());
    }

    let components = deterministic_sccs(&graph);
    for component in &components {
        let component_id = component_identity(component);
        let is_recursive = component.len() > 1
            || component.iter().any(|callable| {
                adjacency
                    .get(callable.as_ref())
                    .is_some_and(|callees| callees.contains(callable.as_ref()))
            });
        for callable in component {
            state.insert_fact(
                FactRow {
                    family: Arc::clone(&bindings.families.scc_membership),
                    subject: Arc::clone(callable),
                    object: Some(Arc::clone(&component_id)),
                    value: None,
                    distance: None,
                    algorithm: COMMON_CALL_GRAPH_RELEASE,
                    precision: COMMON_CALL_GRAPH_PRECISION,
                    iteration: 0,
                    complete: topology_complete,
                },
                bounds,
            );
            state
                .progress
                .get_mut(&bindings.families.scc_membership)
                .expect("family")
                .completed += 1;
            if is_recursive {
                incomplete.insert(Arc::clone(callable));
                for family in [
                    &bindings.families.callable_effect,
                    &bindings.families.callable_resource,
                    &bindings.families.callable_summary,
                ] {
                    state.unknown(
                        family,
                        Some(Arc::clone(callable)),
                        "RECURSIVE_SCC",
                        Some(Arc::clone(&component_id)),
                        0,
                    );
                }
            }
        }
    }

    derive_reachability(
        &callables,
        &adjacency,
        topology_complete,
        bindings,
        bounds,
        state,
    );

    let locals: BTreeMap<&str, &CommonCallableLocalSemantics> = input
        .local_semantics
        .iter()
        .map(|local| (local.callable_id.as_ref(), local))
        .collect();
    let mut effects = BTreeMap::new();
    let mut resources = BTreeMap::new();
    for callable in &callables {
        if let Some(local) = locals.get(callable.as_ref()) {
            let effect_bound_hit = local.effects.len() > bounds.max_summary_values.get();
            let resource_bound_hit = local.resources.len() > bounds.max_summary_values.get();
            effects.insert(
                Arc::clone(callable),
                local
                    .effects
                    .iter()
                    .take(bounds.max_summary_values.get())
                    .cloned()
                    .collect(),
            );
            resources.insert(
                Arc::clone(callable),
                local
                    .resources
                    .iter()
                    .take(bounds.max_summary_values.get())
                    .cloned()
                    .collect(),
            );
            if effect_bound_hit || resource_bound_hit {
                incomplete.insert(Arc::clone(callable));
                for family in [
                    &bindings.families.callable_effect,
                    &bindings.families.callable_resource,
                    &bindings.families.callable_summary,
                ] {
                    state.unknown(
                        family,
                        Some(Arc::clone(callable)),
                        "SUMMARY_VALUE_BOUND_EXHAUSTED",
                        Some(Arc::from(format!(
                            "local_effect_count={} local_resource_count={} value_bound={}",
                            local.effects.len(),
                            local.resources.len(),
                            bounds.max_summary_values
                        ))),
                        0,
                    );
                }
            }
        } else {
            effects.insert(Arc::clone(callable), BTreeSet::new());
            resources.insert(Arc::clone(callable), BTreeSet::new());
            incomplete.insert(Arc::clone(callable));
            for family in [
                &bindings.families.callable_effect,
                &bindings.families.callable_resource,
                &bindings.families.callable_summary,
            ] {
                state.unknown(
                    family,
                    Some(Arc::clone(callable)),
                    "CALLABLE_LOCAL_SEMANTICS_MISSING",
                    None,
                    0,
                );
            }
        }
    }

    let schedule = fixed_point_schedule(&graph, &components, &index_by_id, state)?;
    let exhausted = propagate_summaries(
        &schedule,
        &adjacency,
        &mut effects,
        &mut resources,
        &mut incomplete,
        bindings,
        bounds,
        state,
    );
    if exhausted {
        for callable in &callables {
            incomplete.insert(Arc::clone(callable));
            for family in [
                &bindings.families.callable_effect,
                &bindings.families.callable_resource,
                &bindings.families.callable_summary,
            ] {
                state.unknown(
                    family,
                    Some(Arc::clone(callable)),
                    "FIXED_POINT_CONVERGENCE_EXHAUSTED",
                    Some(Arc::from(format!(
                        "iteration_bound={}",
                        bounds.max_iterations
                    ))),
                    u32::from(state.iterations),
                );
            }
        }
    }

    propagate_incomplete_callers(&reverse_callers, &mut incomplete);
    emit_callable_summaries(
        &callables,
        &effects,
        &resources,
        &incomplete,
        bindings,
        bounds,
        state,
    );

    Ok(CallDerivationResult { reverse_callers })
}

fn build_call_graph(
    callables: &BTreeSet<Arc<str>>,
    exact_calls: &[(Arc<str>, Arc<str>, Arc<str>)],
) -> Result<(CanonicalGraph, BTreeMap<Arc<str>, CanonicalIndex>), CommonDerivedAnalysisError> {
    let mut graph = CanonicalGraph::with_capacity(callables.len(), exact_calls.len());
    let mut index_by_id = BTreeMap::new();
    for callable in callables {
        index_by_id.insert(Arc::clone(callable), graph.add_node(Arc::clone(callable)));
    }
    for (caller, callee, _) in exact_calls {
        let source = index_by_id.get(caller.as_ref()).copied().ok_or_else(|| {
            CommonDerivedAnalysisError::InvalidInput("missing call graph caller".into())
        })?;
        let target = index_by_id.get(callee.as_ref()).copied().ok_or_else(|| {
            CommonDerivedAnalysisError::InvalidInput("missing call graph callee".into())
        })?;
        graph.update_edge(source, target, ());
    }
    Ok((graph, index_by_id))
}

fn deterministic_sccs(graph: &CanonicalGraph) -> Vec<Vec<Arc<str>>> {
    let mut components = tarjan_scc(graph)
        .into_iter()
        .map(|component| {
            let mut members = component
                .into_iter()
                .map(|index| Arc::clone(&graph[index]))
                .collect::<Vec<_>>();
            members.sort();
            members
        })
        .collect::<Vec<_>>();
    components.sort();
    components
}

fn component_identity(component: &[Arc<str>]) -> Arc<str> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.common-analysis.scc.v1\0");
    for member in component {
        hasher.update(&(member.len() as u64).to_be_bytes());
        hasher.update(member.as_bytes());
    }
    Arc::from(format!("b3:{}", hasher.finalize().to_hex()))
}

fn derive_reachability(
    callables: &BTreeSet<Arc<str>>,
    adjacency: &BTreeMap<Arc<str>, BTreeSet<Arc<str>>>,
    topology_complete: bool,
    bindings: &CommonAnalysisBindings,
    bounds: CommonAnalysisBounds,
    state: &mut AnalysisState,
) {
    let mut emitted_pairs = 0_usize;
    let mut exhausted = false;
    for callable in callables {
        if exhausted {
            state.unknown(
                &bindings.families.reachability,
                Some(Arc::clone(callable)),
                "REACHABILITY_PAIR_BOUND_EXHAUSTED",
                Some(Arc::from(format!(
                    "pair_bound={}",
                    bounds.max_reachability_pairs
                ))),
                0,
            );
            state
                .progress
                .get_mut(&bindings.families.reachability)
                .expect("family")
                .unknown += 1;
            continue;
        }
        let mut distances: BTreeMap<Arc<str>, u32> = BTreeMap::new();
        let mut queue = VecDeque::new();
        queue.push_back((Arc::clone(callable), 0_u32));
        while let Some((current, distance)) = queue.pop_front() {
            let Some(callees) = adjacency.get(current.as_ref()) else {
                continue;
            };
            for callee in callees {
                let next = distance.saturating_add(1);
                if callee == callable || distances.contains_key(callee.as_ref()) {
                    continue;
                }
                distances.insert(Arc::clone(callee), next);
                queue.push_back((Arc::clone(callee), next));
            }
        }
        if emitted_pairs.saturating_add(distances.len()) > bounds.max_reachability_pairs.get() {
            exhausted = true;
            state.unknown(
                &bindings.families.reachability,
                Some(Arc::clone(callable)),
                "REACHABILITY_PAIR_BOUND_EXHAUSTED",
                Some(Arc::from(format!(
                    "observed_at_least={} pair_bound={}",
                    emitted_pairs.saturating_add(distances.len()),
                    bounds.max_reachability_pairs
                ))),
                0,
            );
            state
                .progress
                .get_mut(&bindings.families.reachability)
                .expect("family")
                .unknown += 1;
            continue;
        }
        emitted_pairs += distances.len();
        for (reachable, distance) in distances {
            state.insert_fact(
                FactRow {
                    family: Arc::clone(&bindings.families.reachability),
                    subject: Arc::clone(callable),
                    object: Some(reachable),
                    value: None,
                    distance: Some(distance),
                    algorithm: COMMON_CALL_GRAPH_RELEASE,
                    precision: COMMON_CALL_GRAPH_PRECISION,
                    iteration: 0,
                    complete: topology_complete,
                },
                bounds,
            );
        }
        state
            .progress
            .get_mut(&bindings.families.reachability)
            .expect("family")
            .completed += 1;
    }
}

fn fixed_point_schedule(
    graph: &CanonicalGraph,
    components: &[Vec<Arc<str>>],
    index_by_id: &BTreeMap<Arc<str>, CanonicalIndex>,
    state: &mut AnalysisState,
) -> Result<Vec<Vec<Arc<str>>>, CommonDerivedAnalysisError> {
    let has_cycle = components.iter().any(|component| {
        component.len() > 1
            || component.iter().any(|callable| {
                let index = index_by_id[callable.as_ref()];
                graph.find_edge(index, index).is_some()
            })
    });
    if has_cycle {
        state.used_condensation = true;
        state
            .operations
            .insert(CommonAnalysisNativeOperation::PetgraphCondensationForCycles);
        let condensed = condensation(graph.clone(), true);
        let mut order = toposort(&condensed, None).map_err(|_| {
            CommonDerivedAnalysisError::InvalidInput(
                "petgraph condensation unexpectedly retained a cycle".into(),
            )
        })?;
        order.reverse();
        Ok(order
            .into_iter()
            .map(|index| {
                let mut group = condensed[index].clone();
                group.sort();
                group
            })
            .collect())
    } else {
        let mut order = toposort(graph, None).map_err(|_| {
            CommonDerivedAnalysisError::InvalidInput(
                "acyclic call graph unexpectedly contains a cycle".into(),
            )
        })?;
        order.reverse();
        Ok(order
            .into_iter()
            .map(|index| vec![Arc::clone(&graph[index])])
            .collect())
    }
}

#[allow(clippy::too_many_arguments)]
fn propagate_summaries(
    schedule: &[Vec<Arc<str>>],
    adjacency: &BTreeMap<Arc<str>, BTreeSet<Arc<str>>>,
    effects: &mut BTreeMap<Arc<str>, BTreeSet<Arc<str>>>,
    resources: &mut BTreeMap<Arc<str>, BTreeSet<Arc<str>>>,
    incomplete: &mut BTreeSet<Arc<str>>,
    bindings: &CommonAnalysisBindings,
    bounds: CommonAnalysisBounds,
    state: &mut AnalysisState,
) -> bool {
    if schedule.is_empty() {
        return false;
    }
    for iteration in 1..=bounds.max_iterations.get() {
        state.iterations = iteration;
        let prior_effects = effects.clone();
        let prior_resources = resources.clone();
        let prior_incomplete = incomplete.clone();
        let mut changed = false;
        for group in schedule {
            for caller in group {
                for callee in adjacency.get(caller.as_ref()).into_iter().flatten() {
                    let callee_effects = prior_effects
                        .get(callee.as_ref())
                        .into_iter()
                        .flatten()
                        .take(bounds.max_summary_values.get())
                        .cloned()
                        .collect::<Vec<_>>();
                    let callee_resources = prior_resources
                        .get(callee.as_ref())
                        .into_iter()
                        .flatten()
                        .take(bounds.max_summary_values.get())
                        .cloned()
                        .collect::<Vec<_>>();
                    let mut value_bound_hit = false;
                    {
                        let caller_effects = effects.get_mut(caller.as_ref()).expect("callable");
                        for effect in callee_effects {
                            if caller_effects.contains(effect.as_ref()) {
                                continue;
                            }
                            if caller_effects.len() == bounds.max_summary_values.get() {
                                value_bound_hit = true;
                            } else if caller_effects.insert(effect) {
                                changed = true;
                            }
                        }
                    }
                    {
                        let caller_resources =
                            resources.get_mut(caller.as_ref()).expect("callable");
                        for resource in callee_resources {
                            if caller_resources.contains(resource.as_ref()) {
                                continue;
                            }
                            if caller_resources.len() == bounds.max_summary_values.get() {
                                value_bound_hit = true;
                            } else if caller_resources.insert(resource) {
                                changed = true;
                            }
                        }
                    }
                    if value_bound_hit {
                        if incomplete.insert(Arc::clone(caller)) {
                            changed = true;
                        }
                        for family in [
                            &bindings.families.callable_effect,
                            &bindings.families.callable_resource,
                            &bindings.families.callable_summary,
                        ] {
                            state.unknown(
                                family,
                                Some(Arc::clone(caller)),
                                "SUMMARY_VALUE_BOUND_EXHAUSTED",
                                Some(Arc::from(format!(
                                    "value_bound={}",
                                    bounds.max_summary_values
                                ))),
                                u32::from(iteration),
                            );
                        }
                    }
                    if prior_incomplete.contains(callee.as_ref())
                        && incomplete.insert(Arc::clone(caller))
                    {
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            return false;
        }
    }
    true
}

fn propagate_incomplete_callers(
    reverse_callers: &BTreeMap<Arc<str>, BTreeSet<Arc<str>>>,
    incomplete: &mut BTreeSet<Arc<str>>,
) {
    let mut queue = incomplete.iter().cloned().collect::<VecDeque<_>>();
    while let Some(callee) = queue.pop_front() {
        for caller in reverse_callers.get(callee.as_ref()).into_iter().flatten() {
            if incomplete.insert(Arc::clone(caller)) {
                queue.push_back(Arc::clone(caller));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_callable_summaries(
    callables: &BTreeSet<Arc<str>>,
    effects: &BTreeMap<Arc<str>, BTreeSet<Arc<str>>>,
    resources: &BTreeMap<Arc<str>, BTreeSet<Arc<str>>>,
    incomplete: &BTreeSet<Arc<str>>,
    bindings: &CommonAnalysisBindings,
    bounds: CommonAnalysisBounds,
    state: &mut AnalysisState,
) {
    for callable in callables {
        let callable_incomplete = incomplete.contains(callable.as_ref());
        let callable_effects = &effects[callable.as_ref()];
        let callable_resources = &resources[callable.as_ref()];
        if callable_incomplete {
            for family in [
                &bindings.families.callable_effect,
                &bindings.families.callable_resource,
                &bindings.families.callable_summary,
            ] {
                state.unknown(
                    family,
                    Some(Arc::clone(callable)),
                    "TRANSITIVE_SUMMARY_INCOMPLETE",
                    None,
                    u32::from(state.iterations),
                );
            }
        }
        for effect in callable_effects {
            state.insert_fact(
                FactRow {
                    family: Arc::clone(&bindings.families.callable_effect),
                    subject: Arc::clone(callable),
                    object: None,
                    value: Some(Arc::clone(effect)),
                    distance: None,
                    algorithm: COMMON_INTERPROCEDURAL_RELEASE,
                    precision: COMMON_INTERPROCEDURAL_PRECISION,
                    iteration: u32::from(state.iterations),
                    complete: !callable_incomplete,
                },
                bounds,
            );
        }
        for resource in callable_resources {
            state.insert_fact(
                FactRow {
                    family: Arc::clone(&bindings.families.callable_resource),
                    subject: Arc::clone(callable),
                    object: None,
                    value: Some(Arc::clone(resource)),
                    distance: None,
                    algorithm: COMMON_INTERPROCEDURAL_RELEASE,
                    precision: COMMON_INTERPROCEDURAL_PRECISION,
                    iteration: u32::from(state.iterations),
                    complete: !callable_incomplete,
                },
                bounds,
            );
        }
        state.insert_fact(
            FactRow {
                family: Arc::clone(&bindings.families.callable_summary),
                subject: Arc::clone(callable),
                object: None,
                value: Some(summary_identity(
                    callable_effects,
                    callable_resources,
                    !callable_incomplete,
                )),
                distance: None,
                algorithm: COMMON_INTERPROCEDURAL_RELEASE,
                precision: COMMON_INTERPROCEDURAL_PRECISION,
                iteration: u32::from(state.iterations),
                complete: !callable_incomplete,
            },
            bounds,
        );
        for family in [
            &bindings.families.callable_effect,
            &bindings.families.callable_resource,
            &bindings.families.callable_summary,
        ] {
            let progress = state.progress.get_mut(family.as_ref()).expect("family");
            if callable_incomplete {
                progress.unknown += 1;
            } else {
                progress.completed += 1;
            }
        }
    }
}

fn summary_identity(
    effects: &BTreeSet<Arc<str>>,
    resources: &BTreeSet<Arc<str>>,
    complete: bool,
) -> Arc<str> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.common-analysis.callable-summary.v1\0");
    hasher.update(&[u8::from(complete)]);
    for (kind, values) in [(b'e', effects), (b'r', resources)] {
        hasher.update(&[kind]);
        for value in values {
            hasher.update(&(value.len() as u64).to_be_bytes());
            hasher.update(value.as_bytes());
        }
    }
    Arc::from(format!("b3:{}", hasher.finalize().to_hex()))
}

fn derive_invalidation(
    input: &CommonDerivedAnalysisInput,
    reverse_callers: &BTreeMap<Arc<str>, BTreeSet<Arc<str>>>,
    bindings: &CommonAnalysisBindings,
    state: &mut AnalysisState,
) {
    for scope in &input.changed_scopes {
        match scope {
            CommonChangedScope::Owner(owner) => {
                for family in [
                    &bindings.families.dominator,
                    &bindings.families.post_dominator,
                    &bindings.families.control_dependence,
                    &bindings.families.data_dependence,
                ] {
                    state.invalidation.insert(InvalidationRow {
                        family: Arc::clone(family),
                        scope_kind: "owner",
                        scope_id: Arc::clone(owner),
                        cause: "OWNER_INPUT_CHANGED",
                    });
                }
            }
            CommonChangedScope::Module(module) => {
                for family in [
                    &bindings.families.call_graph,
                    &bindings.families.scc_membership,
                    &bindings.families.reachability,
                    &bindings.families.callable_effect,
                    &bindings.families.callable_resource,
                    &bindings.families.callable_summary,
                ] {
                    state.invalidation.insert(InvalidationRow {
                        family: Arc::clone(family),
                        scope_kind: "module",
                        scope_id: Arc::clone(module),
                        cause: "MODULE_INPUT_CHANGED",
                    });
                }
            }
            CommonChangedScope::Callable(callable) => {
                let mut affected = BTreeSet::from([Arc::clone(callable)]);
                let mut queue = VecDeque::from([Arc::clone(callable)]);
                while let Some(callee) = queue.pop_front() {
                    for caller in reverse_callers.get(callee.as_ref()).into_iter().flatten() {
                        if affected.insert(Arc::clone(caller)) {
                            queue.push_back(Arc::clone(caller));
                        }
                    }
                }
                for affected_callable in affected {
                    for family in [
                        &bindings.families.callable_effect,
                        &bindings.families.callable_resource,
                        &bindings.families.callable_summary,
                    ] {
                        state.invalidation.insert(InvalidationRow {
                            family: Arc::clone(family),
                            scope_kind: "callable",
                            scope_id: Arc::clone(&affected_callable),
                            cause: "TRANSITIVE_CALLEE_INPUT_CHANGED",
                        });
                    }
                }
            }
        }
    }
}

fn apply_upstream_coverage(
    input: &CommonDerivedAnalysisInput,
    bindings: &CommonAnalysisBindings,
    state: &mut AnalysisState,
) -> Result<(), CommonDerivedAnalysisError> {
    for (input_family, derived_families) in [
        (
            CommonInputFamily::Cfg,
            vec![
                &bindings.families.dominator,
                &bindings.families.post_dominator,
                &bindings.families.control_dependence,
            ],
        ),
        (
            CommonInputFamily::DefUseReaching,
            vec![&bindings.families.data_dependence],
        ),
        (
            CommonInputFamily::CallTargets,
            vec![
                &bindings.families.call_graph,
                &bindings.families.scc_membership,
                &bindings.families.reachability,
                &bindings.families.callable_effect,
                &bindings.families.callable_resource,
                &bindings.families.callable_summary,
            ],
        ),
        (
            CommonInputFamily::LocalEffectResource,
            vec![
                &bindings.families.callable_effect,
                &bindings.families.callable_resource,
                &bindings.families.callable_summary,
            ],
        ),
    ] {
        let coverage = input
            .coverage
            .get(&input_family)
            .ok_or(CommonDerivedAnalysisError::MissingCoverage(input_family))?;
        if coverage.is_complete() {
            continue;
        }
        let gap = coverage
            .remainder_units
            .saturating_add(coverage.unknown_units);
        for family in derived_families {
            state.unknown(
                family,
                None,
                "UPSTREAM_INPUT_INCOMPLETE",
                Some(Arc::from(format!(
                    "input_family={input_family:?} remainder={} unknown={} proof={}",
                    coverage.remainder_units,
                    coverage.unknown_units,
                    hex_full(&coverage.execution_proof_pin)
                ))),
                u32::from(state.iterations),
            );
            let progress = state.progress.get_mut(family.as_ref()).expect("family");
            progress.requested = progress.requested.saturating_add(gap);
            progress.remainder = progress.remainder.saturating_add(coverage.remainder_units);
            progress.unknown = progress.unknown.saturating_add(coverage.unknown_units);
        }
    }
    Ok(())
}

fn common_schema_fields(bindings: &CommonAnalysisBindings) -> Vec<Field> {
    vec![
        Field::new(
            bindings.fields.fabric_epoch_id.as_str(),
            DataType::FixedSizeBinary(32),
            false,
        ),
        Field::new(
            bindings.fields.source_pin.as_str(),
            DataType::FixedSizeBinary(32),
            false,
        ),
        Field::new(
            bindings.fields.input_set_pin.as_str(),
            DataType::FixedSizeBinary(32),
            false,
        ),
        Field::new(
            bindings.fields.proof_pin.as_str(),
            DataType::FixedSizeBinary(32),
            false,
        ),
        Field::new(
            bindings.fields.source_generation.as_str(),
            DataType::UInt64,
            false,
        ),
        Field::new(bindings.fields.authority_id.as_str(), DataType::Utf8, false),
        Field::new(
            bindings.fields.semantic_class_id.as_str(),
            DataType::Utf8,
            false,
        ),
    ]
}

fn common_columns(
    provenance: &CommonAnalysisProvenance,
    bindings: &CommonAnalysisBindings,
    rows: usize,
) -> Vec<ArrayRef> {
    vec![
        fixed_repeat(Some(&provenance.fabric_epoch_id), rows),
        fixed_repeat(Some(&provenance.source_pin), rows),
        fixed_repeat(Some(&provenance.input_set_pin), rows),
        fixed_repeat(Some(&provenance.proof_pin), rows),
        Arc::new(UInt64Array::from_iter_values(std::iter::repeat_n(
            provenance.source_generation,
            rows,
        ))),
        Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
            bindings.authority_id(),
            rows,
        ))),
        Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
            bindings.semantic_class_id(),
            rows,
        ))),
    ]
}

fn fact_schema(bindings: &CommonAnalysisBindings) -> SchemaRef {
    let mut fields = common_schema_fields(bindings);
    fields.extend([
        Field::new(
            bindings.fields.algorithm_release.as_str(),
            DataType::Utf8,
            false,
        ),
        Field::new(
            bindings.fields.precision_release.as_str(),
            DataType::Utf8,
            false,
        ),
        Field::new(bindings.fields.family_id.as_str(), DataType::Utf8, false),
        Field::new(bindings.fields.subject_id.as_str(), DataType::Utf8, false),
        Field::new(bindings.fields.object_id.as_str(), DataType::Utf8, true),
        Field::new(bindings.fields.value_id.as_str(), DataType::Utf8, true),
        Field::new(bindings.fields.distance.as_str(), DataType::UInt32, true),
        Field::new(bindings.fields.iteration.as_str(), DataType::UInt32, false),
        Field::new(bindings.fields.complete.as_str(), DataType::Boolean, false),
    ]);
    Arc::new(Schema::new(fields))
}

fn fact_batch(
    provenance: &CommonAnalysisProvenance,
    bindings: &CommonAnalysisBindings,
    rows: &BTreeSet<FactRow>,
) -> Result<RecordBatch, ArrowError> {
    let mut columns = common_columns(provenance, bindings, rows.len());
    columns.extend([
        Arc::new(StringArray::from_iter_values(
            rows.iter().map(|row| row.algorithm),
        )) as ArrayRef,
        Arc::new(StringArray::from_iter_values(
            rows.iter().map(|row| row.precision),
        )),
        Arc::new(StringArray::from_iter_values(
            rows.iter().map(|row| row.family.as_ref()),
        )),
        Arc::new(StringArray::from_iter_values(
            rows.iter().map(|row| row.subject.as_ref()),
        )),
        Arc::new(StringArray::from(
            rows.iter()
                .map(|row| row.object.as_deref())
                .collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from(
            rows.iter()
                .map(|row| row.value.as_deref())
                .collect::<Vec<_>>(),
        )),
        Arc::new(UInt32Array::from(
            rows.iter().map(|row| row.distance).collect::<Vec<_>>(),
        )),
        Arc::new(UInt32Array::from_iter_values(
            rows.iter().map(|row| row.iteration),
        )),
        Arc::new(BooleanArray::from_iter(
            rows.iter().map(|row| Some(row.complete)),
        )),
    ]);
    RecordBatch::try_new(fact_schema(bindings), columns)
}

fn unknown_schema(bindings: &CommonAnalysisBindings) -> SchemaRef {
    let mut fields = common_schema_fields(bindings);
    fields.extend([
        Field::new(bindings.fields.family_id.as_str(), DataType::Utf8, false),
        Field::new(bindings.fields.subject_id.as_str(), DataType::Utf8, true),
        Field::new(bindings.fields.reason_id.as_str(), DataType::Utf8, false),
        Field::new(bindings.fields.detail.as_str(), DataType::Utf8, true),
        Field::new(bindings.fields.iteration.as_str(), DataType::UInt32, false),
    ]);
    Arc::new(Schema::new(fields))
}

fn unknown_batch(
    provenance: &CommonAnalysisProvenance,
    bindings: &CommonAnalysisBindings,
    rows: &BTreeSet<UnknownRow>,
) -> Result<RecordBatch, ArrowError> {
    let mut columns = common_columns(provenance, bindings, rows.len());
    columns.extend([
        Arc::new(StringArray::from_iter_values(
            rows.iter().map(|row| row.family.as_ref()),
        )) as ArrayRef,
        Arc::new(StringArray::from(
            rows.iter()
                .map(|row| row.subject.as_deref())
                .collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from_iter_values(
            rows.iter().map(|row| row.reason.as_ref()),
        )),
        Arc::new(StringArray::from(
            rows.iter()
                .map(|row| row.detail.as_deref())
                .collect::<Vec<_>>(),
        )),
        Arc::new(UInt32Array::from_iter_values(
            rows.iter().map(|row| row.iteration),
        )),
    ]);
    RecordBatch::try_new(unknown_schema(bindings), columns)
}

fn completeness_schema(bindings: &CommonAnalysisBindings) -> SchemaRef {
    let mut fields = common_schema_fields(bindings);
    fields.extend([
        Field::new(bindings.fields.family_id.as_str(), DataType::Utf8, false),
        Field::new(
            bindings.fields.requested_units.as_str(),
            DataType::UInt64,
            false,
        ),
        Field::new(
            bindings.fields.completed_units.as_str(),
            DataType::UInt64,
            false,
        ),
        Field::new(
            bindings.fields.remainder_units.as_str(),
            DataType::UInt64,
            false,
        ),
        Field::new(
            bindings.fields.unknown_units.as_str(),
            DataType::UInt64,
            false,
        ),
        Field::new(bindings.fields.complete.as_str(), DataType::Boolean, false),
        Field::new(
            bindings.fields.execution_receipt.as_str(),
            DataType::FixedSizeBinary(32),
            false,
        ),
    ]);
    Arc::new(Schema::new(fields))
}

fn completeness_batch(
    provenance: &CommonAnalysisProvenance,
    bindings: &CommonAnalysisBindings,
    coverage: &BTreeMap<CommonInputFamily, CommonInputCoverage>,
    rows: &BTreeMap<Arc<str>, Progress>,
) -> Result<RecordBatch, ArrowError> {
    let mut columns = common_columns(provenance, bindings, rows.len());
    let receipts = rows
        .iter()
        .map(|(family, progress)| completeness_receipt(provenance, coverage, family, *progress))
        .collect::<Vec<_>>();
    columns.extend([
        Arc::new(StringArray::from_iter_values(rows.keys().map(Arc::as_ref))) as ArrayRef,
        Arc::new(UInt64Array::from_iter_values(
            rows.values().map(|row| row.requested),
        )),
        Arc::new(UInt64Array::from_iter_values(
            rows.values().map(|row| row.completed),
        )),
        Arc::new(UInt64Array::from_iter_values(
            rows.values().map(|row| row.remainder),
        )),
        Arc::new(UInt64Array::from_iter_values(
            rows.values().map(|row| row.unknown),
        )),
        Arc::new(BooleanArray::from_iter(
            rows.values().copied().map(Progress::complete).map(Some),
        )),
        fixed_values(receipts.iter().map(Some)),
    ]);
    RecordBatch::try_new(completeness_schema(bindings), columns)
}

fn completeness_receipt(
    provenance: &CommonAnalysisProvenance,
    coverage: &BTreeMap<CommonInputFamily, CommonInputCoverage>,
    family: &str,
    progress: Progress,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.common-analysis.completeness.v1\0");
    hasher.update(&provenance.fabric_epoch_id);
    hasher.update(&provenance.source_pin);
    hasher.update(&provenance.input_set_pin);
    hasher.update(&provenance.proof_pin);
    hasher.update(&provenance.source_generation.to_be_bytes());
    hasher.update(&(family.len() as u64).to_be_bytes());
    hasher.update(family.as_bytes());
    for value in [
        progress.requested,
        progress.completed,
        progress.remainder,
        progress.unknown,
    ] {
        hasher.update(&value.to_be_bytes());
    }
    for input_family in [
        CommonInputFamily::Cfg,
        CommonInputFamily::DefUseReaching,
        CommonInputFamily::CallTargets,
        CommonInputFamily::LocalEffectResource,
    ] {
        let row = &coverage[&input_family];
        hasher.update(&[input_family_code(input_family)]);
        for value in [
            row.requested_units,
            row.completed_units,
            row.remainder_units,
            row.unknown_units,
        ] {
            hasher.update(&value.to_be_bytes());
        }
        hasher.update(&row.execution_proof_pin);
    }
    *hasher.finalize().as_bytes()
}

const fn input_family_code(family: CommonInputFamily) -> u8 {
    match family {
        CommonInputFamily::Cfg => 0,
        CommonInputFamily::DefUseReaching => 1,
        CommonInputFamily::CallTargets => 2,
        CommonInputFamily::LocalEffectResource => 3,
    }
}

fn invalidation_schema(bindings: &CommonAnalysisBindings) -> SchemaRef {
    let mut fields = common_schema_fields(bindings);
    fields.extend([
        Field::new(bindings.fields.family_id.as_str(), DataType::Utf8, false),
        Field::new(bindings.fields.scope_kind.as_str(), DataType::Utf8, false),
        Field::new(bindings.fields.scope_id.as_str(), DataType::Utf8, false),
        Field::new(bindings.fields.cause_id.as_str(), DataType::Utf8, false),
    ]);
    Arc::new(Schema::new(fields))
}

fn invalidation_batch(
    provenance: &CommonAnalysisProvenance,
    bindings: &CommonAnalysisBindings,
    rows: &BTreeSet<InvalidationRow>,
) -> Result<RecordBatch, ArrowError> {
    let mut columns = common_columns(provenance, bindings, rows.len());
    columns.extend([
        Arc::new(StringArray::from_iter_values(
            rows.iter().map(|row| row.family.as_ref()),
        )) as ArrayRef,
        Arc::new(StringArray::from_iter_values(
            rows.iter().map(|row| row.scope_kind),
        )),
        Arc::new(StringArray::from_iter_values(
            rows.iter().map(|row| row.scope_id.as_ref()),
        )),
        Arc::new(StringArray::from_iter_values(
            rows.iter().map(|row| row.cause),
        )),
    ]);
    RecordBatch::try_new(invalidation_schema(bindings), columns)
}

fn fixed_repeat<const N: usize>(value: Option<&[u8; N]>, rows: usize) -> ArrayRef {
    let width = i32::try_from(N).expect("small fixed width");
    let mut builder = FixedSizeBinaryBuilder::with_capacity(rows, width);
    for _ in 0..rows {
        if let Some(value) = value {
            builder.append_value(value).expect("matching fixed width");
        } else {
            builder.append_null();
        }
    }
    Arc::new(builder.finish())
}

fn fixed_values<'a, const N: usize>(
    values: impl IntoIterator<Item = Option<&'a [u8; N]>>,
) -> ArrayRef {
    let values = values.into_iter();
    let (lower, _) = values.size_hint();
    let width = i32::try_from(N).expect("small fixed width");
    let mut builder = FixedSizeBinaryBuilder::with_capacity(lower, width);
    for value in values {
        if let Some(value) = value {
            builder.append_value(value).expect("matching fixed width");
        } else {
            builder.append_null();
        }
    }
    Arc::new(builder.finish())
}

async fn native_sort_output(
    context: &SessionContext,
    batch: RecordBatch,
    sort_columns: &[&str],
    table_name: &'static str,
) -> Result<RecordBatch, CommonDerivedAnalysisError> {
    let schema = batch.schema();
    let provider = Arc::new(MemTable::try_new(Arc::clone(&schema), vec![vec![batch]])?);
    let projection = schema
        .fields()
        .iter()
        .map(|field| col(field.name()))
        .collect::<Vec<_>>();
    let sort = sort_columns
        .iter()
        .map(|name| col(*name).sort(true, true))
        .collect::<Vec<_>>();
    let plan = LogicalPlanBuilder::scan(table_name, provider_as_source(provider), None)?
        .project(projection)?
        .distinct()?
        .sort(sort)?
        .build()?;
    let batches = context.execute_logical_plan(plan).await?.collect().await?;
    if batches.is_empty() {
        Ok(RecordBatch::new_empty(schema))
    } else {
        Ok(concat_batches(&schema, &batches)?)
    }
}

fn observe_dependencies(
    input: &CommonDerivedAnalysisInput,
    bindings: &CommonAnalysisBindings,
) -> BTreeSet<CommonAnalysisDependency> {
    let mut dependencies = BTreeSet::new();
    for relation in [
        &bindings.relations.cfg_nodes,
        &bindings.relations.cfg_edges,
        &bindings.relations.def_use_reaching,
        &bindings.relations.call_targets,
        &bindings.relations.local_semantics,
    ] {
        dependencies.insert(CommonAnalysisDependency::InputRelation(relation.clone()));
    }
    for relation in [
        &bindings.relations.facts,
        &bindings.relations.unknowns,
        &bindings.relations.completeness,
        &bindings.relations.invalidation,
    ] {
        dependencies.insert(CommonAnalysisDependency::OutputRelation(relation.clone()));
    }
    for field in bindings.fields.all() {
        dependencies.insert(CommonAnalysisDependency::Field(field.clone()));
    }
    for family in bindings.families.all() {
        dependencies.insert(CommonAnalysisDependency::Family(Arc::clone(family)));
    }
    dependencies.extend([
        CommonAnalysisDependency::FabricEpoch(input.provenance.fabric_epoch_id),
        CommonAnalysisDependency::SourcePin(input.provenance.source_pin),
        CommonAnalysisDependency::InputSetPin(input.provenance.input_set_pin),
        CommonAnalysisDependency::ProofPin(input.provenance.proof_pin),
        CommonAnalysisDependency::Authority(Arc::from(bindings.authority_id())),
    ]);
    for (family, coverage) in &input.coverage {
        dependencies.insert(CommonAnalysisDependency::InputExecutionProof(
            *family,
            coverage.execution_proof_pin,
        ));
    }
    dependencies
}

fn semantic_output_digest(
    output: &CommonDerivedAnalysisOutput,
) -> Result<[u8; 32], CommonDerivedAnalysisError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.common-analysis.semantic-output.v1\0");
    for (relation, batch) in [
        (b"facts".as_slice(), &output.facts),
        (b"unknowns".as_slice(), &output.unknowns),
        (b"completeness".as_slice(), &output.completeness),
    ] {
        hasher.update(&(relation.len() as u64).to_be_bytes());
        hasher.update(relation);
        hash_batch(&mut hasher, batch)?;
    }
    Ok(*hasher.finalize().as_bytes())
}

fn hash_batch(
    hasher: &mut blake3::Hasher,
    batch: &RecordBatch,
) -> Result<(), CommonDerivedAnalysisError> {
    hasher.update(&(batch.num_rows() as u64).to_be_bytes());
    hasher.update(&(batch.num_columns() as u64).to_be_bytes());
    for (field, array) in batch.schema().fields().iter().zip(batch.columns()) {
        hasher.update(&(field.name().len() as u64).to_be_bytes());
        hasher.update(field.name().as_bytes());
        hasher.update(format!("{:?}", field.data_type()).as_bytes());
        for row in 0..batch.num_rows() {
            if array.is_null(row) {
                hasher.update(&[0]);
                continue;
            }
            hasher.update(&[1]);
            match field.data_type() {
                DataType::Utf8 => hash_value(
                    hasher,
                    array
                        .as_any()
                        .downcast_ref::<StringArray>()
                        .ok_or_else(|| CommonDerivedAnalysisError::Schema(field.name().clone()))?
                        .value(row)
                        .as_bytes(),
                ),
                DataType::FixedSizeBinary(32) => hash_value(
                    hasher,
                    array
                        .as_any()
                        .downcast_ref::<FixedSizeBinaryArray>()
                        .ok_or_else(|| CommonDerivedAnalysisError::Schema(field.name().clone()))?
                        .value(row),
                ),
                DataType::UInt64 => hash_value(
                    hasher,
                    &array
                        .as_any()
                        .downcast_ref::<UInt64Array>()
                        .ok_or_else(|| CommonDerivedAnalysisError::Schema(field.name().clone()))?
                        .value(row)
                        .to_be_bytes(),
                ),
                DataType::UInt32 => hash_value(
                    hasher,
                    &array
                        .as_any()
                        .downcast_ref::<UInt32Array>()
                        .ok_or_else(|| CommonDerivedAnalysisError::Schema(field.name().clone()))?
                        .value(row)
                        .to_be_bytes(),
                ),
                DataType::Boolean => hash_value(
                    hasher,
                    &[u8::from(
                        array
                            .as_any()
                            .downcast_ref::<BooleanArray>()
                            .ok_or_else(|| {
                                CommonDerivedAnalysisError::Schema(field.name().clone())
                            })?
                            .value(row),
                    )],
                ),
                other => {
                    return Err(CommonDerivedAnalysisError::Schema(format!(
                        "unsupported digest type {other:?}"
                    )));
                }
            }
        }
    }
    Ok(())
}

fn hash_value(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn hex_full(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::Value;

    use super::*;

    const WP33_EXPECTATIONS: &str =
        include_str!("../contracts/acceptance/relational-fabric-v3/expectations.jsonl");
    const WP33_FIXTURES: &str =
        include_str!("../contracts/acceptance/relational-fabric-v3/negative-fixtures.jsonl");

    fn claim_003() -> Value {
        WP33_EXPECTATIONS
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("valid WP33 expectation row"))
            .find(|row| row["claim_id"] == "RFV3-CLAIM-003")
            .expect("frozen Claim 003 expectation")
    }

    fn claim_003_fixture(kind: &str) -> Value {
        WP33_FIXTURES
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("valid WP33 fixture row"))
            .find(|row| row["claim_id"] == "RFV3-CLAIM-003" && row["kind"] == kind)
            .unwrap_or_else(|| panic!("frozen Claim 003 {kind} fixture"))
    }

    fn artifact_column(relation: &Value, name: &str) -> usize {
        relation["columns"]
            .as_array()
            .expect("artifact relation columns")
            .iter()
            .position(|column| column == name)
            .unwrap_or_else(|| panic!("artifact relation lacks {name}"))
    }

    fn claim_003_calls(provider: &Value, inputs: &Value) -> Vec<CommonCallSite> {
        let occurrences = &inputs["canonical_call_occurrences"];
        let callables = &inputs["canonical_callable_lookup"];
        let provider_ordinal = artifact_column(provider, "call_occurrence_ordinal");
        let provider_start = artifact_column(provider, "start_byte");
        let provider_end = artifact_column(provider, "end_byte");
        let provider_target = artifact_column(provider, "qualified_target");
        let occurrence_ordinal = artifact_column(occurrences, "call_occurrence_ordinal");
        let occurrence_site = artifact_column(occurrences, "call_site_id");
        let occurrence_owner = artifact_column(occurrences, "owner_id");
        let occurrence_file = artifact_column(occurrences, "file_id");
        let occurrence_digest = artifact_column(occurrences, "content_digest");
        let occurrence_start = artifact_column(occurrences, "start_byte");
        let occurrence_end = artifact_column(occurrences, "end_byte");
        let callable_target = artifact_column(callables, "qualified_target");
        let callable_id = artifact_column(callables, "callable_id");
        let source = &provider["source_image"];

        let mut calls = provider["rows"]
            .as_array()
            .expect("Claim 003 provider rows")
            .iter()
            .map(|provider_row| {
                let provider_row = provider_row.as_array().expect("Claim 003 provider row");
                let occurrence = occurrences["rows"]
                    .as_array()
                    .expect("Claim 003 canonical occurrences")
                    .iter()
                    .find(|occurrence| {
                        let occurrence = occurrence.as_array().unwrap();
                        occurrence[occurrence_ordinal] == provider_row[provider_ordinal]
                            && occurrence[occurrence_start] == provider_row[provider_start]
                            && occurrence[occurrence_end] == provider_row[provider_end]
                            && occurrence[occurrence_file] == source["file_id"]
                            && occurrence[occurrence_digest] == source["content_digest"]
                    })
                    .expect("exact Claim 003 provider/canonical occurrence join")
                    .as_array()
                    .unwrap();
                let callable = callables["rows"]
                    .as_array()
                    .expect("Claim 003 callable lookup")
                    .iter()
                    .find(|callable| {
                        callable.as_array().unwrap()[callable_target]
                            == provider_row[provider_target]
                    })
                    .expect("exact Claim 003 target/callable join")
                    .as_array()
                    .unwrap();
                CommonCallSite {
                    call_site_id: Arc::from(
                        occurrence[occurrence_site]
                            .as_str()
                            .expect("Claim 003 call-site identity"),
                    ),
                    caller_id: Arc::from(
                        occurrence[occurrence_owner]
                            .as_str()
                            .expect("Claim 003 caller identity"),
                    ),
                    resolution: CommonCallResolution::Exact {
                        callee_id: Arc::from(
                            callable[callable_id]
                                .as_str()
                                .expect("Claim 003 callable identity"),
                        ),
                    },
                }
            })
            .collect::<Vec<_>>();

        for remainder in provider["coverage_terminal"]["remainders"]
            .as_array()
            .expect("Claim 003 target-set remainders")
        {
            let call_site_id = remainder["call_site_id"]
                .as_str()
                .expect("Claim 003 remainder call-site identity");
            let occurrence = occurrences["rows"]
                .as_array()
                .expect("Claim 003 canonical occurrences")
                .iter()
                .find(|occurrence| occurrence.as_array().unwrap()[occurrence_site] == call_site_id)
                .expect("Claim 003 remainder/canonical occurrence join")
                .as_array()
                .unwrap();
            calls.push(CommonCallSite {
                call_site_id: Arc::from(call_site_id),
                caller_id: Arc::from(
                    occurrence[occurrence_owner]
                        .as_str()
                        .expect("Claim 003 remainder owner"),
                ),
                resolution: CommonCallResolution::Unknown {
                    reason: Arc::from(
                        remainder["reason"]
                            .as_str()
                            .expect("Claim 003 typed remainder reason"),
                    ),
                },
            });
        }
        calls
    }

    fn relation(value: &str) -> RelationId {
        RelationId::new(value).expect("test relation")
    }

    fn field(value: &str) -> FieldId {
        FieldId::new(value).expect("test field")
    }

    pub(crate) fn bindings() -> CommonAnalysisBindings {
        CommonAnalysisBindings {
            relations: CommonAnalysisRelations {
                cfg_nodes: relation("input.cfg_nodes"),
                cfg_edges: relation("input.cfg_edges"),
                def_use_reaching: relation("input.def_use"),
                call_targets: relation("input.call_targets"),
                local_semantics: relation("input.local_semantics"),
                facts: relation("output.common_facts"),
                unknowns: relation("output.common_unknowns"),
                completeness: relation("output.common_completeness"),
                invalidation: relation("output.common_invalidation"),
            },
            fields: CommonAnalysisFields {
                fabric_epoch_id: field("fabric_epoch_id"),
                source_pin: field("source_pin"),
                input_set_pin: field("input_set_pin"),
                proof_pin: field("proof_pin"),
                source_generation: field("source_generation"),
                authority_id: field("authority_id"),
                semantic_class_id: field("semantic_class_id"),
                algorithm_release: field("algorithm_release"),
                precision_release: field("precision_release"),
                family_id: field("family_id"),
                subject_id: field("subject_id"),
                object_id: field("object_id"),
                value_id: field("value_id"),
                distance: field("distance"),
                iteration: field("iteration"),
                complete: field("complete"),
                reason_id: field("reason_id"),
                detail: field("detail"),
                requested_units: field("requested_units"),
                completed_units: field("completed_units"),
                remainder_units: field("remainder_units"),
                unknown_units: field("unknown_units"),
                execution_receipt: field("execution_receipt"),
                scope_kind: field("scope_kind"),
                scope_id: field("scope_id"),
                cause_id: field("cause_id"),
            },
            families: CommonAnalysisFamilies {
                dominator: Arc::from("family.dom"),
                post_dominator: Arc::from("family.postdom"),
                control_dependence: Arc::from("family.control"),
                data_dependence: Arc::from("family.data"),
                call_graph: Arc::from("family.call"),
                scc_membership: Arc::from("family.scc"),
                reachability: Arc::from("family.reach"),
                callable_effect: Arc::from("family.effect"),
                callable_resource: Arc::from("family.resource"),
                callable_summary: Arc::from("family.summary"),
            },
            authority: CommonAnalysisAuthority::ApplicationOwned(Arc::from("authority.app")),
            semantic_class: CommonAnalysisSemanticClass::Fact(Arc::from("semantic.fact")),
        }
    }

    fn complete_coverage() -> BTreeMap<CommonInputFamily, CommonInputCoverage> {
        [
            CommonInputFamily::Cfg,
            CommonInputFamily::DefUseReaching,
            CommonInputFamily::CallTargets,
            CommonInputFamily::LocalEffectResource,
        ]
        .into_iter()
        .map(|family| {
            (
                family,
                CommonInputCoverage {
                    requested_units: 1,
                    completed_units: 1,
                    remainder_units: 0,
                    unknown_units: 0,
                    execution_proof_pin: [9; 32],
                },
            )
        })
        .collect()
    }

    fn input() -> CommonDerivedAnalysisInput {
        CommonDerivedAnalysisInput {
            provenance: CommonAnalysisProvenance {
                fabric_epoch_id: [1; 32],
                source_pin: [2; 32],
                input_set_pin: [3; 32],
                proof_pin: [4; 32],
                source_generation: 7,
            },
            cfg_nodes: Vec::new(),
            cfg_edges: Vec::new(),
            def_use_reaching: Vec::new(),
            calls: Vec::new(),
            local_semantics: Vec::new(),
            coverage: complete_coverage(),
            changed_scopes: BTreeSet::new(),
        }
    }

    fn bounds() -> CommonAnalysisBounds {
        CommonAnalysisBounds::try_new(100, 200, 100, 200, 10_000, 100, 20_000, 100)
            .expect("test bounds")
    }

    fn local(callable: &str, effects: &[&str], resources: &[&str]) -> CommonCallableLocalSemantics {
        CommonCallableLocalSemantics {
            callable_id: Arc::from(callable),
            module_id: Arc::from("module"),
            owner_id: Arc::from("owner"),
            effects: effects.iter().copied().map(Arc::from).collect(),
            resources: resources.iter().copied().map(Arc::from).collect(),
        }
    }

    fn exact(site: &str, caller: &str, callee: &str) -> CommonCallSite {
        CommonCallSite {
            call_site_id: Arc::from(site),
            caller_id: Arc::from(caller),
            resolution: CommonCallResolution::Exact {
                callee_id: Arc::from(callee),
            },
        }
    }

    fn strings<'a>(batch: &'a RecordBatch, name: &str) -> &'a StringArray {
        batch
            .column_by_name(name)
            .expect("column")
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("string column")
    }

    fn contains_fact(
        output: &CommonDerivedAnalysisOutput,
        family: &str,
        subject: &str,
        object: Option<&str>,
        value: Option<&str>,
    ) -> bool {
        let families = strings(&output.facts, "family_id");
        let subjects = strings(&output.facts, "subject_id");
        let objects = strings(&output.facts, "object_id");
        let values = strings(&output.facts, "value_id");
        (0..output.facts.num_rows()).any(|row| {
            families.value(row) == family
                && subjects.value(row) == subject
                && optional_string(objects, row) == object
                && optional_string(values, row) == value
        })
    }

    fn contains_unknown(
        output: &CommonDerivedAnalysisOutput,
        family: &str,
        subject: Option<&str>,
        reason: &str,
    ) -> bool {
        let families = strings(&output.unknowns, "family_id");
        let subjects = strings(&output.unknowns, "subject_id");
        let reasons = strings(&output.unknowns, "reason_id");
        (0..output.unknowns.num_rows()).any(|row| {
            families.value(row) == family
                && optional_string(subjects, row) == subject
                && reasons.value(row) == reason
        })
    }

    fn optional_string(array: &StringArray, row: usize) -> Option<&str> {
        (!array.is_null(row)).then(|| array.value(row))
    }

    fn claim_003_input(provider: &Value, inputs: &Value) -> CommonDerivedAnalysisInput {
        let mut analysis = input();
        analysis.calls = claim_003_calls(provider, inputs);
        let terminal = &provider["coverage_terminal"];
        let coverage = analysis
            .coverage
            .get_mut(&CommonInputFamily::CallTargets)
            .expect("Claim 003 call-target coverage");
        coverage.requested_units = terminal["requested_call_sites"]
            .as_u64()
            .expect("Claim 003 requested call sites");
        coverage.completed_units = terminal["completed_call_sites"]
            .as_u64()
            .expect("Claim 003 completed call sites");
        coverage.remainder_units = u64::try_from(
            terminal["remainders"]
                .as_array()
                .expect("Claim 003 coverage remainders")
                .len(),
        )
        .unwrap();
        analysis
    }

    fn expected_call_facts(decoded: &Value) -> BTreeSet<(String, String, String)> {
        let call_site = artifact_column(decoded, "call_site_id");
        let caller = artifact_column(decoded, "caller_id");
        let callee = artifact_column(decoded, "callee_id");
        decoded["rows"]
            .as_array()
            .expect("Claim 003 expected rows")
            .iter()
            .map(|row| {
                let row = row.as_array().expect("Claim 003 expected row");
                (
                    row[call_site].as_str().unwrap().to_owned(),
                    row[caller].as_str().unwrap().to_owned(),
                    row[callee].as_str().unwrap().to_owned(),
                )
            })
            .collect()
    }

    fn actual_call_facts(
        output: &CommonDerivedAnalysisOutput,
    ) -> BTreeSet<(String, String, String)> {
        let families = strings(&output.facts, "family_id");
        let callers = strings(&output.facts, "subject_id");
        let callees = strings(&output.facts, "object_id");
        let call_sites = strings(&output.facts, "value_id");
        (0..output.facts.num_rows())
            .filter(|row| families.value(*row) == "family.call")
            .map(|row| {
                (
                    call_sites.value(row).to_owned(),
                    callers.value(row).to_owned(),
                    callees.value(row).to_owned(),
                )
            })
            .collect()
    }

    #[tokio::test]
    async fn wp38_claim_003_positive_executes_candidate_preserving_common_call_graph() {
        let claim = claim_003();
        let inputs = &claim["complete_input_universe"]["inputs"];
        let analysis = claim_003_input(&inputs["provider_call_targets"], inputs);
        let output =
            analyze_common_derived(&SessionContext::new(), &analysis, &bindings(), bounds())
                .await
                .expect("execute production common call-graph analysis");

        let actual = actual_call_facts(&output);
        assert_eq!(actual, expected_call_facts(&claim["decoded_expectation"]));
        assert_eq!(
            actual
                .iter()
                .filter(|(site, _, _)| {
                    site == "entity:call-site:308e2487678a1d769a48da4b4e39b713"
                })
                .count(),
            2,
            "both accepted provider candidates remain visible"
        );
    }

    #[tokio::test]
    async fn wp38_claim_003_causal_provider_target_changes_common_call_graph() {
        let claim = claim_003();
        let inputs = &claim["complete_input_universe"]["inputs"];
        let fixture = claim_003_fixture("causal");
        let baseline = claim_003_input(&inputs["provider_call_targets"], inputs);
        let mut changed_provider = inputs["provider_call_targets"].clone();
        *changed_provider
            .pointer_mut(
                fixture["mutation"]["json_pointer"]
                    .as_str()
                    .expect("Claim 003 causal pointer"),
            )
            .expect("Claim 003 causal target") = fixture["mutation"]["after"].clone();
        let changed = claim_003_input(&changed_provider, inputs);

        let baseline_output =
            analyze_common_derived(&SessionContext::new(), &baseline, &bindings(), bounds())
                .await
                .expect("execute baseline common call graph");
        let changed_output =
            analyze_common_derived(&SessionContext::new(), &changed, &bindings(), bounds())
                .await
                .expect("execute causally changed common call graph");

        let baseline_facts = actual_call_facts(&baseline_output);
        let changed_facts = actual_call_facts(&changed_output);
        assert_ne!(baseline_facts, changed_facts);
        assert_eq!(
            changed_facts,
            expected_call_facts(&fixture["expected_decoded"])
        );
    }

    #[tokio::test]
    async fn wp38_claim_003_negative_preserves_known_fact_and_typed_unknown() {
        let claim = claim_003();
        let inputs = &claim["complete_input_universe"]["inputs"];
        let fixture = claim_003_fixture("negative");
        let provider = &fixture["mutation"]["after"];
        let analysis = claim_003_input(provider, inputs);
        let output =
            analyze_common_derived(&SessionContext::new(), &analysis, &bindings(), bounds())
                .await
                .expect("execute partial production common call-graph analysis");

        assert_eq!(
            actual_call_facts(&output),
            expected_call_facts(&fixture["expected_decoded"]["known_facts"])
        );
        let unknown = &fixture["expected_decoded"]["unknown_remainder"]["rows"][0];
        assert!(contains_unknown(
            &output,
            "family.call",
            unknown[0].as_str(),
            unknown[2].as_str().expect("Claim 003 typed unknown reason"),
        ));
        assert_eq!(
            fixture["expected_decoded"]["published_false_edges"],
            serde_json::json!([])
        );
    }

    #[tokio::test]
    async fn branch_postdominator_and_control_dependence_are_exact() {
        let mut input = input();
        input.cfg_nodes = [
            ("entry", true, false),
            ("branch", false, false),
            ("left", false, false),
            ("right", false, false),
            ("merge", false, false),
            ("exit", false, true),
        ]
        .into_iter()
        .map(|(node, is_entry, is_exit)| CommonCfgNode {
            owner_id: Arc::from("owner"),
            node_id: Arc::from(node),
            is_entry,
            is_exit,
        })
        .collect();
        input.cfg_edges = [
            ("entry", "branch"),
            ("branch", "left"),
            ("branch", "right"),
            ("left", "merge"),
            ("right", "merge"),
            ("merge", "exit"),
        ]
        .into_iter()
        .map(|(source, target)| CommonCfgEdge {
            owner_id: Arc::from("owner"),
            source_node_id: Arc::from(source),
            target_node_id: Arc::from(target),
        })
        .collect();
        let output = analyze_common_derived(&SessionContext::new(), &input, &bindings(), bounds())
            .await
            .expect("analysis");
        assert!(contains_fact(
            &output,
            "family.postdom",
            "left",
            Some("merge"),
            None
        ));
        assert!(contains_fact(
            &output,
            "family.control",
            "left",
            Some("branch"),
            None
        ));
        assert!(contains_fact(
            &output,
            "family.dom",
            "left",
            Some("branch"),
            None
        ));
    }

    #[tokio::test]
    async fn recursive_scc_is_deterministic_and_explicitly_incomplete() {
        let mut input = input();
        input.local_semantics = vec![local("a", &["ea"], &[]), local("b", &["eb"], &[])];
        input.calls = vec![exact("ab", "a", "b"), exact("ba", "b", "a")];
        let output = analyze_common_derived(&SessionContext::new(), &input, &bindings(), bounds())
            .await
            .expect("analysis");
        assert!(contains_unknown(
            &output,
            "family.summary",
            Some("a"),
            "RECURSIVE_SCC"
        ));
        assert!(output.observation.used_condensation);
        assert!(
            contains_fact(&output, "family.scc", "a", None, None)
                || strings(&output.facts, "family_id")
                    .iter()
                    .flatten()
                    .any(|value| value == "family.scc")
        );
    }

    #[tokio::test]
    async fn dynamic_unknown_propagates_to_transitive_callers() {
        let mut input = input();
        input.local_semantics = vec![local("upstream", &[], &[]), local("dynamic", &[], &[])];
        input.calls = vec![
            exact("known", "upstream", "dynamic"),
            CommonCallSite {
                call_site_id: Arc::from("dynamic-site"),
                caller_id: Arc::from("dynamic"),
                resolution: CommonCallResolution::Dynamic {
                    reason: Arc::from("open-world dispatch"),
                },
            },
        ];
        let output = analyze_common_derived(&SessionContext::new(), &input, &bindings(), bounds())
            .await
            .expect("analysis");
        assert!(contains_unknown(
            &output,
            "family.call",
            Some("dynamic-site"),
            "DYNAMIC_DISPATCH_TARGET_UNKNOWN"
        ));
        assert!(contains_unknown(
            &output,
            "family.summary",
            Some("upstream"),
            "TRANSITIVE_SUMMARY_INCOMPLETE"
        ));
    }

    #[tokio::test]
    async fn effect_and_resource_sets_reach_a_fixed_point() {
        let mut input = input();
        input.local_semantics = vec![
            local("caller", &["local"], &[]),
            local("callee", &["read"], &["database"]),
        ];
        input.calls = vec![exact("site", "caller", "callee")];
        let output = analyze_common_derived(&SessionContext::new(), &input, &bindings(), bounds())
            .await
            .expect("analysis");
        assert!(contains_fact(
            &output,
            "family.effect",
            "caller",
            None,
            Some("read")
        ));
        assert!(contains_fact(
            &output,
            "family.resource",
            "caller",
            None,
            Some("database")
        ));
    }

    #[tokio::test]
    async fn convergence_bound_exhaustion_is_explicit() {
        let mut input = input();
        input.local_semantics = vec![local("a", &["ea"], &[]), local("b", &["eb"], &[])];
        input.calls = vec![exact("ab", "a", "b"), exact("ba", "b", "a")];
        let tight = CommonAnalysisBounds::try_new(100, 200, 100, 200, 10_000, 100, 20_000, 1)
            .expect("bounds");
        let output = analyze_common_derived(&SessionContext::new(), &input, &bindings(), tight)
            .await
            .expect("analysis");
        assert!(contains_unknown(
            &output,
            "family.summary",
            Some("a"),
            "FIXED_POINT_CONVERGENCE_EXHAUSTED"
        ));
        assert_eq!(output.observation.iterations, 1);
    }

    #[tokio::test]
    async fn input_permutation_and_incremental_invalidation_are_deterministic() {
        let mut clean = input();
        clean.local_semantics = vec![local("a", &[], &[]), local("b", &["read"], &[])];
        clean.calls = vec![exact("site-1", "a", "b"), exact("site-2", "a", "b")];
        let mut incremental = clean.clone();
        incremental.calls.reverse();
        incremental.local_semantics.reverse();
        incremental
            .changed_scopes
            .insert(CommonChangedScope::Callable(Arc::from("b")));
        let clean_output =
            analyze_common_derived(&SessionContext::new(), &clean, &bindings(), bounds())
                .await
                .expect("clean");
        let incremental_output =
            analyze_common_derived(&SessionContext::new(), &incremental, &bindings(), bounds())
                .await
                .expect("incremental");
        assert!(
            compare_clean_incremental(&clean_output, &incremental_output)
                .expect("digest")
                .equivalent
        );
        assert!(
            incremental_output
                .observation
                .invalidated_callables
                .contains("a")
        );
        assert!(
            incremental_output
                .observation
                .invalidated_callables
                .contains("b")
        );
    }

    #[test]
    fn provider_authority_and_judgment_semantics_are_rejected() {
        let mut model = bindings();
        model.authority = CommonAnalysisAuthority::ProviderNative(Arc::from("provider"));
        assert!(matches!(
            model.validate(),
            Err(CommonDerivedAnalysisError::ProviderNativeAuthority(_))
        ));
        let mut model = bindings();
        model.semantic_class = CommonAnalysisSemanticClass::Judgment(Arc::from("risk"));
        assert!(matches!(
            model.validate(),
            Err(CommonDerivedAnalysisError::JudgmentSemanticClass(_))
        ));
    }

    #[tokio::test]
    async fn empty_inputs_preserve_all_typed_output_schemas() {
        let output =
            analyze_common_derived(&SessionContext::new(), &input(), &bindings(), bounds())
                .await
                .expect("empty analysis");
        assert_eq!(output.facts.num_rows(), 0);
        assert_eq!(output.unknowns.num_rows(), 0);
        assert_eq!(output.invalidation.num_rows(), 0);
        assert_eq!(output.completeness.num_rows(), 10);
        assert_eq!(output.facts.schema().fields().len(), 16);
        assert_eq!(output.unknowns.schema().fields().len(), 12);
        assert_eq!(output.completeness.schema().fields().len(), 14);
        assert_eq!(output.invalidation.schema().fields().len(), 11);
    }
}
