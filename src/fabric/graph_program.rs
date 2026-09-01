//! Native, bounded graph programs compiled into DataFusion logical operators.
//!
//! Reachability is a relational program: the edge relation, endpoint fields, output relation,
//! output fields, implementation release, and resource envelope are supplied as data. The
//! compiler selects DataFusion's bounded recursive-query rung and emits no SQL text, graph-index
//! identity, row-oriented graph kernel, or opaque logical extension.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::num::{NonZeroU16, NonZeroUsize};
use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::{DataType, SchemaRef};
use datafusion::common::{Column, TableReference};
use datafusion::datasource::cte_worktable::CteWorkTable;
use datafusion::datasource::provider_as_source;
use datafusion::execution::context::SessionContext;
use datafusion::functions::core::expr_fn::coalesce;
use datafusion::functions_aggregate::expr_fn::min;
use datafusion::logical_expr::utils::can_hash;
use datafusion::logical_expr::{Expr, JoinType, LogicalPlan, LogicalPlanBuilder};
use datafusion::physical_plan::execute_stream;
use datafusion::prelude::{col, lit};
use futures::StreamExt;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use thiserror::Error;

use crate::cancellation::Cancellation;
use crate::identity::{
    CanonicalPublicIdentity, IdentityDomain, decode_public_id, decode_public_id_any_kind,
    decode_public_id_prefix, derive_public_recipe_identity,
};
use crate::identity_recipes::{self as recipes, RecipeValue};
use crate::relational_program::{FieldId, RelationId};

const EDGE_SEED_ALIAS: &str = "__codefabric_graph_edge_seed";
const EDGE_STEP_ALIAS: &str = "__codefabric_graph_edge_step";
const FRONTIER_ALIAS: &str = "__codefabric_graph_frontier";
const INTERNAL_SOURCE: &str = "__codefabric_graph_source";
const INTERNAL_TARGET: &str = "__codefabric_graph_target";
const INTERNAL_DEPTH: &str = "__codefabric_graph_depth";
const INTERNAL_MINIMUM_DEPTH: &str = "__codefabric_graph_minimum_depth";

/// One canonical relationship edge admitted to a query-local ordered-path projection.
///
/// Canonical entity and fact identities remain the only public identity. `NodeIndex` and
/// `EdgeIndex` exist only inside [`bounded_shortest_path_witness`].
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OrderedPathEdge {
    pub fact_id: Arc<str>,
    pub source_entity_id: Arc<str>,
    pub target_entity_id: Arc<str>,
}

impl OrderedPathEdge {
    /// Construct one bounded, non-empty canonical edge row.
    ///
    /// # Errors
    ///
    /// Rejects empty, whitespace-padded, control-bearing, or overlong identities.
    pub fn try_new(
        fact_id: impl Into<Arc<str>>,
        source_entity_id: impl Into<Arc<str>>,
        target_entity_id: impl Into<Arc<str>>,
    ) -> Result<Self, GraphProgramError> {
        let edge = Self {
            fact_id: fact_id.into(),
            source_entity_id: source_entity_id.into(),
            target_entity_id: target_entity_id.into(),
        };
        for (kind, value) in [
            ("path fact", edge.fact_id.as_ref()),
            ("path source entity", edge.source_entity_id.as_ref()),
            ("path target entity", edge.target_entity_id.as_ref()),
        ] {
            validate_bounded_text(kind, value)?;
        }
        Ok(edge)
    }
}

/// Explicit query-local resource bounds for ordered shortest-path witness construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderedPathBounds {
    max_path_length: NonZeroU16,
    max_input_edges: NonZeroUsize,
    max_frontier_paths: NonZeroUsize,
}

impl OrderedPathBounds {
    /// Construct a non-zero ordered-path resource envelope.
    ///
    /// # Errors
    ///
    /// Rejects any zero bound.
    pub fn try_new(
        max_path_length: u16,
        max_input_edges: usize,
        max_frontier_paths: usize,
    ) -> Result<Self, GraphProgramError> {
        Ok(Self {
            max_path_length: NonZeroU16::new(max_path_length)
                .ok_or(GraphProgramError::ZeroResourceBound("max_path_length"))?,
            max_input_edges: NonZeroUsize::new(max_input_edges)
                .ok_or(GraphProgramError::ZeroResourceBound("max_input_edges"))?,
            max_frontier_paths: NonZeroUsize::new(max_frontier_paths)
                .ok_or(GraphProgramError::ZeroResourceBound("max_frontier_paths"))?,
        })
    }

    #[must_use]
    pub const fn max_path_length(self) -> u16 {
        self.max_path_length.get()
    }

    #[must_use]
    pub const fn max_input_edges(self) -> usize {
        self.max_input_edges.get()
    }

    #[must_use]
    pub const fn max_frontier_paths(self) -> usize {
        self.max_frontier_paths.get()
    }
}

/// One deterministic shortest-path witness expressed only in canonical application identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderedPathWitness {
    ordered_entity_ids: Arc<[Arc<str>]>,
    ordered_fact_ids: Arc<[Arc<str>]>,
}

/// Complete immutable inputs for the released path-result public identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathResultIdentityInput {
    pub workspace_id: Arc<str>,
    pub analysis_context_id: Arc<str>,
    pub fabric_epoch_id: Arc<str>,
    pub policy_identity: Arc<str>,
    pub ordered_entity_ids: Arc<[Arc<str>]>,
    pub ordered_fact_ids: Arc<[Arc<str>]>,
}

/// Compatibility name for callers migrated from slot-based identities.
pub type PathResultSlotIdentityInput = PathResultIdentityInput;

/// Issue one path-result identity bound to its exact ordered entity/fact witness.
///
/// # Errors
///
/// Rejects invalid bounded identities or an unrepresentable canonical preimage.
pub fn issue_path_result_slot_identity(
    input: &PathResultSlotIdentityInput,
) -> Result<CanonicalPublicIdentity, GraphProgramError> {
    issue_path_result_identity(input)
}

/// Issue one witness-bound CBEF-v1 path-result identity.
///
/// # Errors
///
/// Rejects malformed canonical IDs, an invalid witness shape, or an invalid policy identity.
pub fn issue_path_result_identity(
    input: &PathResultIdentityInput,
) -> Result<CanonicalPublicIdentity, GraphProgramError> {
    validate_bounded_text("path policy identity", &input.policy_identity)?;
    if input.ordered_entity_ids.len() != input.ordered_fact_ids.len().saturating_add(1) {
        return Err(GraphProgramError::CanonicalIdentity);
    }
    let workspace_id = decode_public_id(IdentityDomain::Workspace, None, &input.workspace_id)
        .map_err(|_| GraphProgramError::CanonicalIdentity)?;
    let analysis_context_id = decode_public_id(
        IdentityDomain::AnalysisContext,
        None,
        &input.analysis_context_id,
    )
    .map_err(|_| GraphProgramError::CanonicalIdentity)?;
    let fabric_epoch_id = decode_public_id_prefix("fabric-epoch", &input.fabric_epoch_id)
        .map_err(|_| GraphProgramError::CanonicalIdentity)?;
    let entity_ids = input
        .ordered_entity_ids
        .iter()
        .map(|value| {
            decode_public_id_any_kind(IdentityDomain::Entity, value)
                .map(RecipeValue::Id)
                .map_err(|_| GraphProgramError::CanonicalIdentity)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let fact_ids = input
        .ordered_fact_ids
        .iter()
        .map(|value| {
            decode_public_id_any_kind(IdentityDomain::RelationFact, value)
                .map(RecipeValue::Id)
                .map_err(|_| GraphProgramError::CanonicalIdentity)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let record = recipes::path_result(recipes::PathResultFields {
        workspace_id: RecipeValue::Id(workspace_id),
        analysis_context_id: RecipeValue::Id(analysis_context_id),
        fabric_epoch_id: RecipeValue::Id(fabric_epoch_id),
        policy_identity: RecipeValue::Utf8(input.policy_identity.to_string()),
        ordered_entity_ids: RecipeValue::OrderedList(entity_ids),
        ordered_fact_ids: RecipeValue::OrderedList(fact_ids),
    })
    .map_err(|_| GraphProgramError::CanonicalIdentity)?;
    derive_public_recipe_identity(
        record,
        vec![
            ("workspace_id", serde_json::json!(input.workspace_id)),
            (
                "analysis_context_id",
                serde_json::json!(input.analysis_context_id),
            ),
            ("fabric_epoch_id", serde_json::json!(input.fabric_epoch_id)),
            ("policy_identity", serde_json::json!(input.policy_identity)),
            (
                "ordered_entity_ids",
                serde_json::json!(input.ordered_entity_ids),
            ),
            (
                "ordered_fact_ids",
                serde_json::json!(input.ordered_fact_ids),
            ),
        ],
        &["path length", "witness provenance", "certainty summary"],
    )
    .map_err(|_| GraphProgramError::CanonicalIdentity)
}

/// Validate the currently released bounded ordered-path policy.
///
/// # Errors
///
/// Rejects every unbounded or unknown policy before graph construction.
pub fn validate_bounded_ordered_path_policy(policy: &str) -> Result<(), GraphProgramError> {
    validate_bounded_text("ordered path policy", policy)?;
    if policy == "shortest" {
        Ok(())
    } else {
        Err(GraphProgramError::UnboundedOrderedPathPolicy(
            policy.to_owned(),
        ))
    }
}

impl OrderedPathWitness {
    #[must_use]
    pub fn ordered_entity_ids(&self) -> &[Arc<str>] {
        &self.ordered_entity_ids
    }

    #[must_use]
    pub fn ordered_fact_ids(&self) -> &[Arc<str>] {
        &self.ordered_fact_ids
    }

    #[must_use]
    pub fn length(&self) -> usize {
        self.ordered_fact_ids.len()
    }
}

#[derive(Clone, Debug)]
struct PendingPath {
    nodes: Vec<NodeIndex>,
    facts: Vec<Arc<str>>,
}

/// Construct one immutable query-local `DiGraph` and return the canonical shortest witness.
///
/// Equal-length paths are ordered by their ordered canonical fact-ID sequence. The graph-local
/// indices never escape, input order cannot affect the result, cycles cannot enter a shortest
/// witness, and every queue/input bound fails closed before a partial witness is returned.
///
/// # Errors
///
/// Rejects duplicate fact identities, an excessive edge/frontier count, unknown endpoints, and
/// invalid canonical identities.
pub fn bounded_shortest_path_witness(
    edges: &[OrderedPathEdge],
    source_entity_id: &str,
    target_entity_id: &str,
    bounds: OrderedPathBounds,
) -> Result<Option<OrderedPathWitness>, GraphProgramError> {
    validate_bounded_text("path source entity", source_entity_id)?;
    validate_bounded_text("path target entity", target_entity_id)?;
    if edges.len() > bounds.max_input_edges() {
        return Err(GraphProgramError::OrderedPathInputEdgesExceeded {
            limit: bounds.max_input_edges(),
            observed: edges.len(),
        });
    }
    let mut ordered = edges.to_vec();
    ordered.sort();
    let mut fact_ids = BTreeSet::new();
    for edge in &ordered {
        if !fact_ids.insert(Arc::clone(&edge.fact_id)) {
            return Err(GraphProgramError::DuplicateOrderedPathFact(
                edge.fact_id.to_string(),
            ));
        }
    }

    let mut graph = DiGraph::<Arc<str>, Arc<str>>::new();
    let mut nodes = BTreeMap::<Arc<str>, NodeIndex>::new();
    for identity in ordered.iter().flat_map(|edge| {
        [
            Arc::clone(&edge.source_entity_id),
            Arc::clone(&edge.target_entity_id),
        ]
    }) {
        nodes
            .entry(Arc::clone(&identity))
            .or_insert_with(|| graph.add_node(identity));
    }
    let source = nodes.get(source_entity_id).copied().ok_or_else(|| {
        GraphProgramError::UnknownOrderedPathEndpoint {
            role: "source",
            entity_id: source_entity_id.to_owned(),
        }
    })?;
    let target = nodes.get(target_entity_id).copied().ok_or_else(|| {
        GraphProgramError::UnknownOrderedPathEndpoint {
            role: "target",
            entity_id: target_entity_id.to_owned(),
        }
    })?;
    for edge in ordered {
        graph.add_edge(
            nodes[edge.source_entity_id.as_ref()],
            nodes[edge.target_entity_id.as_ref()],
            edge.fact_id,
        );
    }

    let mut frontier = VecDeque::from([PendingPath {
        nodes: vec![source],
        facts: Vec::new(),
    }]);
    while let Some(path) = frontier.pop_front() {
        let current = *path.nodes.last().expect("path always has one node");
        if current == target {
            return Ok(Some(OrderedPathWitness {
                ordered_entity_ids: path
                    .nodes
                    .iter()
                    .map(|node| Arc::clone(&graph[*node]))
                    .collect::<Vec<_>>()
                    .into(),
                ordered_fact_ids: path.facts.into(),
            }));
        }
        if path.facts.len() >= usize::from(bounds.max_path_length()) {
            continue;
        }
        let mut outgoing = graph
            .edges(current)
            .map(|edge| {
                (
                    Arc::clone(edge.weight()),
                    Arc::clone(&graph[edge.target()]),
                    edge.target(),
                )
            })
            .collect::<Vec<_>>();
        outgoing.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
        for (fact_id, _, next) in outgoing {
            if path.nodes.contains(&next) {
                continue;
            }
            if frontier.len() >= bounds.max_frontier_paths() {
                return Err(GraphProgramError::OrderedPathFrontierExceeded {
                    limit: bounds.max_frontier_paths(),
                });
            }
            let mut next_path = path.clone();
            next_path.nodes.push(next);
            next_path.facts.push(fact_id);
            frontier.push_back(next_path);
        }
    }
    Ok(None)
}

/// One exact application-contract relation supplied to the graph compiler.
#[derive(Clone, Debug)]
pub struct GraphRelationInput {
    relation_id: RelationId,
    plan: LogicalPlan,
}

impl GraphRelationInput {
    #[must_use]
    pub fn new(relation_id: RelationId, plan: LogicalPlan) -> Self {
        Self { relation_id, plan }
    }

    #[must_use]
    pub const fn relation_id(&self) -> &RelationId {
        &self.relation_id
    }

    #[must_use]
    pub const fn plan(&self) -> &LogicalPlan {
        &self.plan
    }
}

/// Complete application-owned field and relation bindings for bounded reachability.
#[derive(Clone, Debug)]
pub struct ReachabilityBindings {
    operation_id: Arc<str>,
    edge_relation: RelationId,
    edge_schema: SchemaRef,
    source_field: FieldId,
    target_field: FieldId,
    output_relation: RelationId,
    output_schema: SchemaRef,
    output_source_field: FieldId,
    output_target_field: FieldId,
    output_depth_field: FieldId,
    implementation_release: Arc<str>,
}

impl ReachabilityBindings {
    /// Validate an application-owned reachability contract before any plan is built.
    ///
    /// # Errors
    ///
    /// Rejects empty release data, duplicate/reserved bindings, nullable or incompatible
    /// endpoints, non-hashable identity types, and an output schema that is not the exact typed
    /// `(source, target, minimum_depth)` relation.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        operation_id: impl Into<Arc<str>>,
        edge_relation: RelationId,
        edge_schema: SchemaRef,
        source_field: FieldId,
        target_field: FieldId,
        output_relation: RelationId,
        output_schema: SchemaRef,
        output_source_field: FieldId,
        output_target_field: FieldId,
        output_depth_field: FieldId,
        implementation_release: impl Into<Arc<str>>,
    ) -> Result<Self, GraphProgramError> {
        let operation_id = operation_id.into();
        let implementation_release = implementation_release.into();
        validate_bounded_text("operation", &operation_id)?;
        validate_bounded_text("implementation release", &implementation_release)?;
        validate_unique_schema_fields("edge", &edge_schema)?;
        validate_unique_schema_fields("output", &output_schema)?;

        if source_field == target_field {
            return Err(GraphProgramError::DuplicateEndpointBinding(
                source_field.as_str().to_owned(),
            ));
        }
        let output_names = [
            output_source_field.as_str(),
            output_target_field.as_str(),
            output_depth_field.as_str(),
        ];
        if output_names[0] == output_names[1]
            || output_names[0] == output_names[2]
            || output_names[1] == output_names[2]
        {
            return Err(GraphProgramError::DuplicateOutputBinding);
        }
        for name in [source_field.as_str(), target_field.as_str()]
            .into_iter()
            .chain(output_names)
        {
            if is_reserved_field(name) {
                return Err(GraphProgramError::ReservedFieldName(name.to_owned()));
            }
        }

        let source = edge_schema
            .field_with_name(source_field.as_str())
            .map_err(|_| GraphProgramError::MissingEndpointField {
                field: source_field.as_str().to_owned(),
            })?;
        let target = edge_schema
            .field_with_name(target_field.as_str())
            .map_err(|_| GraphProgramError::MissingEndpointField {
                field: target_field.as_str().to_owned(),
            })?;
        if source.is_nullable() || target.is_nullable() {
            return Err(GraphProgramError::NullableEndpoint {
                source_nullable: source.is_nullable(),
                target_nullable: target.is_nullable(),
            });
        }
        if source.data_type() != target.data_type() {
            return Err(GraphProgramError::EndpointTypeMismatch {
                source_type: source.data_type().clone(),
                target_type: target.data_type().clone(),
            });
        }
        if !can_hash(source.data_type()) {
            return Err(GraphProgramError::UnhashableEndpointType(
                source.data_type().clone(),
            ));
        }

        validate_output_schema(&output_schema, &output_names, source.data_type())?;

        Ok(Self {
            operation_id,
            edge_relation,
            edge_schema,
            source_field,
            target_field,
            output_relation,
            output_schema,
            output_source_field,
            output_target_field,
            output_depth_field,
            implementation_release,
        })
    }

    #[must_use]
    pub const fn operation_id(&self) -> &Arc<str> {
        &self.operation_id
    }

    #[must_use]
    pub const fn edge_relation(&self) -> &RelationId {
        &self.edge_relation
    }

    #[must_use]
    pub const fn output_relation(&self) -> &RelationId {
        &self.output_relation
    }

    #[must_use]
    pub const fn output_schema(&self) -> &SchemaRef {
        &self.output_schema
    }
}

/// Explicit request-local limits for one recursive graph execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphResourceBounds {
    max_depth: NonZeroU16,
    max_output_rows: NonZeroUsize,
    max_output_batches: NonZeroUsize,
    max_output_bytes: NonZeroUsize,
}

impl GraphResourceBounds {
    /// Construct a non-zero resource envelope.
    ///
    /// # Errors
    ///
    /// Rejects zero limits and a row limit that cannot reserve one overflow-probe row.
    pub fn try_new(
        max_depth: u16,
        max_output_rows: usize,
        max_output_batches: usize,
        max_output_bytes: usize,
    ) -> Result<Self, GraphProgramError> {
        let bounds = Self {
            max_depth: NonZeroU16::new(max_depth)
                .ok_or(GraphProgramError::ZeroResourceBound("max_depth"))?,
            max_output_rows: NonZeroUsize::new(max_output_rows)
                .ok_or(GraphProgramError::ZeroResourceBound("max_output_rows"))?,
            max_output_batches: NonZeroUsize::new(max_output_batches)
                .ok_or(GraphProgramError::ZeroResourceBound("max_output_batches"))?,
            max_output_bytes: NonZeroUsize::new(max_output_bytes)
                .ok_or(GraphProgramError::ZeroResourceBound("max_output_bytes"))?,
        };
        bounds.probe_rows()?;
        Ok(bounds)
    }

    #[must_use]
    pub const fn max_depth(self) -> u16 {
        self.max_depth.get()
    }

    #[must_use]
    pub const fn max_output_rows(self) -> usize {
        self.max_output_rows.get()
    }

    #[must_use]
    pub const fn max_output_batches(self) -> usize {
        self.max_output_batches.get()
    }

    #[must_use]
    pub const fn max_output_bytes(self) -> usize {
        self.max_output_bytes.get()
    }

    fn probe_rows(self) -> Result<usize, GraphProgramError> {
        self.max_output_rows
            .get()
            .checked_add(1)
            .ok_or(GraphProgramError::ResourceProbeOverflow)
    }
}

/// Highest viable DataFusion rung selected by the reachability compiler.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GraphExecutionRung {
    NativeBoundedRecursiveQuery,
}

/// Native logical operators causally required by the selected program.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GraphNativeOperator {
    EndpointProjection,
    DepthFilter,
    InnerJoin,
    RecursiveQueryDistinct,
    MinimumDepthAggregate,
    DeterministicSort,
    OutputOverflowProbeLimit,
}

/// Application/runtime dependencies observed while compiling the actual operation.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GraphCompilationDependency {
    InputRelation(RelationId),
    InputField(FieldId),
    OutputRelation(RelationId),
    OutputField(FieldId),
    ImplementationRelease(Arc<str>),
    SessionMemoryPool,
    /// DataFusion 55's `execute_stream` contract: dropping the stream aborts execution and
    /// releases query resources. This is not a claim that an external cancellation token is
    /// bound to the plan.
    DataFusionExecuteStreamDropAbort,
}

/// Typed causal evidence for rung, operators, dependencies, and limits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphCompilationObservation {
    operation_id: Arc<str>,
    rung: GraphExecutionRung,
    operators: BTreeSet<GraphNativeOperator>,
    dependencies: BTreeSet<GraphCompilationDependency>,
    bounds: GraphResourceBounds,
}

impl GraphCompilationObservation {
    #[must_use]
    pub const fn operation_id(&self) -> &Arc<str> {
        &self.operation_id
    }

    #[must_use]
    pub const fn rung(&self) -> GraphExecutionRung {
        self.rung
    }

    #[must_use]
    pub const fn operators(&self) -> &BTreeSet<GraphNativeOperator> {
        &self.operators
    }

    #[must_use]
    pub const fn dependencies(&self) -> &BTreeSet<GraphCompilationDependency> {
        &self.dependencies
    }

    #[must_use]
    pub const fn bounds(&self) -> GraphResourceBounds {
        self.bounds
    }
}

/// One optimizer-visible native graph plan plus its exact output contract.
#[derive(Clone, Debug)]
pub struct CompiledGraphProgram {
    plan: LogicalPlan,
    output_schema: SchemaRef,
    observation: GraphCompilationObservation,
}

impl CompiledGraphProgram {
    #[must_use]
    pub const fn logical_plan(&self) -> &LogicalPlan {
        &self.plan
    }

    #[must_use]
    pub const fn output_schema(&self) -> &SchemaRef {
        &self.output_schema
    }

    #[must_use]
    pub const fn observation(&self) -> &GraphCompilationObservation {
        &self.observation
    }

    /// Optimize, physically plan, and stream the bounded program with DataFusion's task context.
    ///
    /// Dropping this future/stream leaves cancellation to DataFusion. Output row, batch, and byte
    /// limits fail closed; the extra logical fetch row proves that an apparently full result was
    /// not silently truncated.
    ///
    /// # Errors
    ///
    /// Returns a typed resource, schema, optimizer, physical-planner, or execution failure.
    pub async fn execute(
        &self,
        context: &SessionContext,
    ) -> Result<GraphProgramExecution, GraphProgramError> {
        self.execute_with_cancellation(context, &Cancellation::default())
            .await
    }

    /// Execute with one request-owned cooperative cancellation capability.
    ///
    /// Cancellation is checked at every material planning boundary and before accepting each
    /// output batch. The physical stream is dropped before the typed cancellation error is
    /// returned, which invokes DataFusion's stream-drop abort and resource-release contract.
    ///
    /// # Errors
    ///
    /// Returns [`GraphProgramError::Cancelled`] when cancellation is observed, or the same typed
    /// planning, execution, schema, and resource errors as [`Self::execute`].
    pub async fn execute_with_cancellation(
        &self,
        context: &SessionContext,
        cancellation: &Cancellation,
    ) -> Result<GraphProgramExecution, GraphProgramError> {
        if cancellation.is_cancelled() {
            return Err(GraphProgramError::Cancelled);
        }
        let optimized = context.state().optimize(&self.plan)?;
        if cancellation.is_cancelled() {
            return Err(GraphProgramError::Cancelled);
        }
        let physical = context.state().create_physical_plan(&optimized).await?;
        if cancellation.is_cancelled() {
            return Err(GraphProgramError::Cancelled);
        }
        let mut stream = execute_stream(physical, context.task_ctx())?;
        let bounds = self.observation.bounds;
        let mut batches = Vec::new();
        let mut rows = 0_usize;
        let mut bytes = 0_usize;
        while let Some(batch) = stream.next().await {
            if cancellation.is_cancelled() {
                drop(stream);
                return Err(GraphProgramError::Cancelled);
            }
            let batch = batch?;
            if batch.schema_ref().as_ref() != self.output_schema.as_ref() {
                return Err(GraphProgramError::ExecutedOutputSchemaMismatch {
                    expected: Arc::clone(&self.output_schema),
                    actual: batch.schema(),
                });
            }
            rows = rows
                .checked_add(batch.num_rows())
                .ok_or(GraphProgramError::ResourceCounterOverflow("rows"))?;
            bytes = bytes
                .checked_add(batch.get_array_memory_size())
                .ok_or(GraphProgramError::ResourceCounterOverflow("bytes"))?;
            let batch_count = batches
                .len()
                .checked_add(1)
                .ok_or(GraphProgramError::ResourceCounterOverflow("batches"))?;
            if rows > bounds.max_output_rows() {
                return Err(GraphProgramError::OutputRowsExceeded {
                    limit: bounds.max_output_rows(),
                    observed: rows,
                });
            }
            if batch_count > bounds.max_output_batches() {
                return Err(GraphProgramError::OutputBatchesExceeded {
                    limit: bounds.max_output_batches(),
                    observed: batch_count,
                });
            }
            if bytes > bounds.max_output_bytes() {
                return Err(GraphProgramError::OutputBytesExceeded {
                    limit: bounds.max_output_bytes(),
                    observed: bytes,
                });
            }
            batches.push(batch);
        }
        drop(stream);
        if cancellation.is_cancelled() {
            return Err(GraphProgramError::Cancelled);
        }

        Ok(GraphProgramExecution {
            schema: Arc::clone(&self.output_schema),
            batches,
            observation: self.observation.clone(),
        })
    }
}

/// Exact-schema result emitted only after resource and schema proof succeeds.
#[derive(Clone, Debug)]
pub struct GraphProgramExecution {
    schema: SchemaRef,
    batches: Vec<RecordBatch>,
    observation: GraphCompilationObservation,
}

impl GraphProgramExecution {
    #[must_use]
    pub const fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    #[must_use]
    pub fn batches(&self) -> &[RecordBatch] {
        &self.batches
    }

    #[must_use]
    pub const fn observation(&self) -> &GraphCompilationObservation {
        &self.observation
    }

    #[must_use]
    pub fn into_batches(self) -> Vec<RecordBatch> {
        self.batches
    }
}

/// Compile bounded transitive reachability at DataFusion's native recursive-query rung.
///
/// The output contains one row per reachable `(source, target)` pair and its minimum positive
/// path depth. `UNION` recursion removes duplicate frontier rows, the aggregate selects minimum
/// depth across cycles/multiple paths, and the final sort plus overflow probe makes output
/// deterministic and fail-closed under the selected row bound.
///
/// # Errors
///
/// Rejects relation/schema drift and any DataFusion logical-plan construction failure.
pub fn compile_bounded_reachability(
    input: GraphRelationInput,
    bindings: &ReachabilityBindings,
    bounds: GraphResourceBounds,
) -> Result<CompiledGraphProgram, GraphProgramError> {
    if input.relation_id != bindings.edge_relation {
        return Err(GraphProgramError::InputRelationMismatch {
            expected: bindings.edge_relation.as_str().to_owned(),
            actual: input.relation_id.as_str().to_owned(),
        });
    }
    let actual_edge_schema = input.plan.schema().as_arrow();
    if actual_edge_schema != bindings.edge_schema.as_ref() {
        return Err(GraphProgramError::InputSchemaMismatch {
            expected: Arc::clone(&bindings.edge_schema),
            actual: Arc::new(actual_edge_schema.clone()),
        });
    }

    let source_name = bindings.source_field.as_str();
    let target_name = bindings.target_field.as_str();
    let seed = LogicalPlanBuilder::from(input.plan.clone())
        .alias(EDGE_SEED_ALIAS)?
        .project([
            qualified_column(EDGE_SEED_ALIAS, source_name).alias(INTERNAL_SOURCE),
            qualified_column(EDGE_SEED_ALIAS, target_name).alias(INTERNAL_TARGET),
            lit(1_u32).alias(INTERNAL_DEPTH),
        ])?
        .build()?;

    let recursive_name = format!("__codefabric_graph_recursive_{}", bindings.operation_id);
    let work_table = Arc::new(CteWorkTable::new(
        &recursive_name,
        Arc::new(seed.schema().as_arrow().clone()),
    ));
    let frontier =
        LogicalPlanBuilder::scan(recursive_name.clone(), provider_as_source(work_table), None)?
            .alias(FRONTIER_ALIAS)?
            .build()?;
    let edge_step = LogicalPlanBuilder::from(input.plan)
        .alias(EDGE_STEP_ALIAS)?
        .build()?;

    let recursive_term = LogicalPlanBuilder::from(frontier)
        .filter(
            qualified_column(FRONTIER_ALIAS, INTERNAL_DEPTH).lt(lit(u32::from(bounds.max_depth()))),
        )?
        .join_on(
            edge_step,
            JoinType::Inner,
            [qualified_column(FRONTIER_ALIAS, INTERNAL_TARGET)
                .eq(qualified_column(EDGE_STEP_ALIAS, source_name))],
        )?
        .project([
            qualified_column(FRONTIER_ALIAS, INTERNAL_SOURCE).alias(INTERNAL_SOURCE),
            qualified_column(EDGE_STEP_ALIAS, target_name).alias(INTERNAL_TARGET),
            (qualified_column(FRONTIER_ALIAS, INTERNAL_DEPTH) + lit(1_u32)).alias(INTERNAL_DEPTH),
        ])?
        .build()?;

    let recursive = LogicalPlanBuilder::from(seed)
        .to_recursive_query(recursive_name, recursive_term, true)?
        .build()?;
    let output_source = bindings.output_source_field.as_str();
    let output_target = bindings.output_target_field.as_str();
    let output_depth = bindings.output_depth_field.as_str();
    let plan = LogicalPlanBuilder::from(recursive)
        .aggregate(
            [col(INTERNAL_SOURCE), col(INTERNAL_TARGET)],
            [min(col(INTERNAL_DEPTH)).alias(INTERNAL_MINIMUM_DEPTH)],
        )?
        .project([
            col(INTERNAL_SOURCE).alias(output_source),
            col(INTERNAL_TARGET).alias(output_target),
            // Every aggregate group contains at least one non-null positive depth. `min` is
            // conservatively nullable in DataFusion's generic aggregate contract; the non-null
            // literal makes this stronger graph invariant visible in the output `DFSchema`.
            coalesce(vec![col(INTERNAL_MINIMUM_DEPTH), lit(0_u32)]).alias(output_depth),
        ])?
        .sort([
            col(output_source).sort(true, false),
            col(output_target).sort(true, false),
            col(output_depth).sort(true, false),
        ])?
        .limit(0, Some(bounds.probe_rows()?))?
        .build()?;

    if plan.schema().as_arrow() != bindings.output_schema.as_ref() {
        return Err(GraphProgramError::CompiledOutputSchemaMismatch {
            expected: Arc::clone(&bindings.output_schema),
            actual: Arc::new(plan.schema().as_arrow().clone()),
        });
    }

    let dependencies = BTreeSet::from([
        GraphCompilationDependency::InputRelation(bindings.edge_relation.clone()),
        GraphCompilationDependency::InputField(bindings.source_field.clone()),
        GraphCompilationDependency::InputField(bindings.target_field.clone()),
        GraphCompilationDependency::OutputRelation(bindings.output_relation.clone()),
        GraphCompilationDependency::OutputField(bindings.output_source_field.clone()),
        GraphCompilationDependency::OutputField(bindings.output_target_field.clone()),
        GraphCompilationDependency::OutputField(bindings.output_depth_field.clone()),
        GraphCompilationDependency::ImplementationRelease(Arc::clone(
            &bindings.implementation_release,
        )),
        GraphCompilationDependency::SessionMemoryPool,
        GraphCompilationDependency::DataFusionExecuteStreamDropAbort,
    ]);
    let operators = BTreeSet::from([
        GraphNativeOperator::EndpointProjection,
        GraphNativeOperator::DepthFilter,
        GraphNativeOperator::InnerJoin,
        GraphNativeOperator::RecursiveQueryDistinct,
        GraphNativeOperator::MinimumDepthAggregate,
        GraphNativeOperator::DeterministicSort,
        GraphNativeOperator::OutputOverflowProbeLimit,
    ]);

    Ok(CompiledGraphProgram {
        plan,
        output_schema: Arc::clone(&bindings.output_schema),
        observation: GraphCompilationObservation {
            operation_id: Arc::clone(&bindings.operation_id),
            rung: GraphExecutionRung::NativeBoundedRecursiveQuery,
            operators,
            dependencies,
            bounds,
        },
    })
}

fn qualified_column(qualifier: &'static str, field: &str) -> Expr {
    Expr::Column(Column::new(
        Some(TableReference::bare(qualifier)),
        field.to_owned(),
    ))
}

fn validate_bounded_text(kind: &'static str, value: &str) -> Result<(), GraphProgramError> {
    if value.is_empty() || value.len() > 240 {
        return Err(GraphProgramError::InvalidText {
            kind,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_unique_schema_fields(
    role: &'static str,
    schema: &SchemaRef,
) -> Result<(), GraphProgramError> {
    let mut names = BTreeSet::new();
    for field in schema.fields() {
        if !names.insert(field.name()) {
            return Err(GraphProgramError::DuplicateSchemaField {
                role,
                field: field.name().clone(),
            });
        }
    }
    Ok(())
}

fn validate_output_schema(
    schema: &SchemaRef,
    output_names: &[&str; 3],
    identity_type: &DataType,
) -> Result<(), GraphProgramError> {
    if schema.fields().len() != 3 {
        return Err(GraphProgramError::InvalidOutputSchema(
            "reachability output must contain exactly three fields".to_owned(),
        ));
    }
    let expected_types = [identity_type, identity_type, &DataType::UInt32];
    for ((field, expected_name), expected_type) in
        schema.fields().iter().zip(output_names).zip(expected_types)
    {
        if field.name() != expected_name
            || field.data_type() != expected_type
            || field.is_nullable()
        {
            return Err(GraphProgramError::InvalidOutputSchema(format!(
                "field {expected_name:?} must be non-null {expected_type:?}; observed {} {:?} nullable={}",
                field.name(),
                field.data_type(),
                field.is_nullable()
            )));
        }
    }
    Ok(())
}

fn is_reserved_field(name: &str) -> bool {
    matches!(
        name,
        INTERNAL_SOURCE | INTERNAL_TARGET | INTERNAL_DEPTH | INTERNAL_MINIMUM_DEPTH
    )
}

/// Fail-closed graph binding, planning, execution, and resource errors.
#[derive(Debug, Error)]
pub enum GraphProgramError {
    #[error("invalid {kind} identifier {value:?}")]
    InvalidText { kind: &'static str, value: String },
    #[error("source and target both bind field {0:?}")]
    DuplicateEndpointBinding(String),
    #[error("reachability output field bindings are not unique")]
    DuplicateOutputBinding,
    #[error("field name {0:?} is reserved by the native graph compiler")]
    ReservedFieldName(String),
    #[error("{role} schema contains duplicate field {field:?}")]
    DuplicateSchemaField { role: &'static str, field: String },
    #[error("edge endpoint field {field:?} is absent")]
    MissingEndpointField { field: String },
    #[error(
        "edge endpoints must be non-null: source nullable={source_nullable}, target nullable={target_nullable}"
    )]
    NullableEndpoint {
        source_nullable: bool,
        target_nullable: bool,
    },
    #[error("edge endpoint types differ: source={source_type:?}, target={target_type:?}")]
    EndpointTypeMismatch {
        source_type: DataType,
        target_type: DataType,
    },
    #[error("edge endpoint type {0:?} is not hash-join capable")]
    UnhashableEndpointType(DataType),
    #[error("invalid reachability output schema: {0}")]
    InvalidOutputSchema(String),
    #[error("resource bound {0} must be non-zero")]
    ZeroResourceBound(&'static str),
    #[error("ordered-path input edges exceeded bound {limit}: observed {observed}")]
    OrderedPathInputEdgesExceeded { limit: usize, observed: usize },
    #[error("ordered-path fact identity {0:?} is duplicated")]
    DuplicateOrderedPathFact(String),
    #[error("ordered-path {role} endpoint {entity_id:?} is absent from the admitted graph")]
    UnknownOrderedPathEndpoint {
        role: &'static str,
        entity_id: String,
    },
    #[error("ordered-path frontier exceeded bound {limit}")]
    OrderedPathFrontierExceeded { limit: usize },
    #[error("ordered path policy {0:?} is not a released finite policy")]
    UnboundedOrderedPathPolicy(String),
    #[error("ordered-path canonical identity derivation failed")]
    CanonicalIdentity,
    #[error("output-row bound cannot reserve one overflow-probe row")]
    ResourceProbeOverflow,
    #[error("edge relation mismatch: expected {expected:?}, actual {actual:?}")]
    InputRelationMismatch { expected: String, actual: String },
    #[error("edge input schema differs from its application binding")]
    InputSchemaMismatch {
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("compiled reachability schema differs from its application binding")]
    CompiledOutputSchemaMismatch {
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("executed reachability schema differs from its application binding")]
    ExecutedOutputSchemaMismatch {
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("output rows exceeded bound {limit}: observed at least {observed}")]
    OutputRowsExceeded { limit: usize, observed: usize },
    #[error("output batches exceeded bound {limit}: observed at least {observed}")]
    OutputBatchesExceeded { limit: usize, observed: usize },
    #[error("output bytes exceeded bound {limit}: observed at least {observed}")]
    OutputBytesExceeded { limit: usize, observed: usize },
    #[error("resource counter overflowed for {0}")]
    ResourceCounterOverflow(&'static str),
    #[error("graph execution was cancelled")]
    Cancelled,
    #[error(transparent)]
    DataFusion(#[from] datafusion::error::DataFusionError),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use arrow_array::{StringArray, UInt32Array};
    use arrow_schema::{Field, Schema};
    use datafusion::datasource::MemTable;
    use datafusion::physical_plan::displayable;

    use super::*;

    #[test]
    fn path_result_identity_is_a_witness_bound_domain_18_known_answer() {
        let input = PathResultIdentityInput {
            workspace_id: Arc::from(format!("workspace:{}", "00".repeat(16))),
            analysis_context_id: Arc::from(format!("context:{}", "11".repeat(16))),
            fabric_epoch_id: Arc::from(format!("fabric-epoch:{}", "22".repeat(16))),
            policy_identity: Arc::from("policy:r1"),
            ordered_entity_ids: vec![
                Arc::from(format!("entity:function:{}", "44".repeat(16))),
                Arc::from(format!("entity:function:{}", "45".repeat(16))),
            ]
            .into(),
            ordered_fact_ids: vec![Arc::from(format!("fact:call:{}", "55".repeat(16)))].into(),
        };
        let identity = issue_path_result_identity(&input).expect("domain-18 path identity KAT");
        assert_eq!(identity.public_id, "path:959e262ba970b5e61f5b3e638a998694");
        assert_eq!(
            identity.recipe_evidence()["digest"]["full_digest_hex"],
            "959e262ba970b5e61f5b3e638a9986941e86ded7a0e00a0cd2b6de90afc03e1d"
        );
        assert_eq!(
            identity.recipe_evidence()["record_domain"],
            serde_json::json!({"code": 18, "name": "PATH_RESULT"})
        );

        let mut reversed = input.clone();
        reversed.ordered_entity_ids = input
            .ordered_entity_ids
            .iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>()
            .into();
        assert_ne!(
            issue_path_result_identity(&reversed).unwrap().public_id,
            identity.public_id
        );

        let mut malformed = input;
        malformed.ordered_fact_ids = Arc::from([]);
        assert!(matches!(
            issue_path_result_identity(&malformed),
            Err(GraphProgramError::CanonicalIdentity)
        ));
    }

    fn edge_schema() -> SchemaRef {
        Arc::new(Schema::new(vec![
            Field::new("from", DataType::Utf8, false),
            Field::new("to", DataType::Utf8, false),
        ]))
    }

    fn output_schema() -> SchemaRef {
        Arc::new(Schema::new(vec![
            Field::new("reachable_from", DataType::Utf8, false),
            Field::new("reachable_to", DataType::Utf8, false),
            Field::new("minimum_depth", DataType::UInt32, false),
        ]))
    }

    fn relation_id(value: &str) -> RelationId {
        RelationId::new(value).expect("relation ID")
    }

    fn field_id(value: &str) -> FieldId {
        FieldId::new(value).expect("field ID")
    }

    fn bindings() -> ReachabilityBindings {
        ReachabilityBindings::try_new(
            "call_reachability",
            relation_id("canonical.call_edge"),
            edge_schema(),
            field_id("from"),
            field_id("to"),
            relation_id("derived.call_reachability"),
            output_schema(),
            field_id("reachable_from"),
            field_id("reachable_to"),
            field_id("minimum_depth"),
            "graph.native-recursive.v1",
        )
        .expect("valid reachability binding")
    }

    fn bounds(depth: u16, rows: usize) -> GraphResourceBounds {
        GraphResourceBounds::try_new(depth, rows, 32, 1_000_000).expect("valid bounds")
    }

    fn edge_batch(edges: &[(&str, &str)]) -> RecordBatch {
        RecordBatch::try_new(
            edge_schema(),
            vec![
                Arc::new(StringArray::from_iter_values(
                    edges.iter().map(|(source, _)| *source),
                )),
                Arc::new(StringArray::from_iter_values(
                    edges.iter().map(|(_, target)| *target),
                )),
            ],
        )
        .expect("edge batch")
    }

    async fn input_partitions(
        context: &SessionContext,
        table: &str,
        partitions: Vec<Vec<RecordBatch>>,
    ) -> GraphRelationInput {
        let provider = Arc::new(
            MemTable::try_new(edge_schema(), partitions).expect("partitioned edge mem table"),
        );
        context
            .register_table(table, provider)
            .expect("register edges");
        let plan = context
            .table(table)
            .await
            .expect("edge frame")
            .into_unoptimized_plan();
        GraphRelationInput::new(relation_id("canonical.call_edge"), plan)
    }

    async fn input(
        context: &SessionContext,
        table: &str,
        edges: &[(&str, &str)],
    ) -> GraphRelationInput {
        input_partitions(context, table, vec![vec![edge_batch(edges)]]).await
    }

    fn rows(execution: &GraphProgramExecution) -> Vec<(String, String, u32)> {
        execution
            .batches()
            .iter()
            .flat_map(|batch| {
                let source = batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .expect("source strings");
                let target = batch
                    .column(1)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .expect("target strings");
                let depth = batch
                    .column(2)
                    .as_any()
                    .downcast_ref::<UInt32Array>()
                    .expect("depth values");
                (0..batch.num_rows())
                    .map(|index| {
                        (
                            source.value(index).to_owned(),
                            target.value(index).to_owned(),
                            depth.value(index),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    #[tokio::test]
    async fn cycles_and_duplicate_paths_produce_unique_minimum_depth_rows() {
        let context = SessionContext::new();
        let graph = input(
            &context,
            "cycle_edges",
            &[("a", "b"), ("a", "b"), ("b", "a")],
        )
        .await;
        let compiled =
            compile_bounded_reachability(graph, &bindings(), bounds(2, 10)).expect("compile cycle");
        let execution = compiled.execute(&context).await.expect("execute cycle");

        assert_eq!(
            rows(&execution),
            vec![
                ("a".into(), "a".into(), 2),
                ("a".into(), "b".into(), 1),
                ("b".into(), "a".into(), 1),
                ("b".into(), "b".into(), 2),
            ]
        );
        assert_eq!(
            rows(&execution).into_iter().collect::<BTreeSet<_>>().len(),
            4
        );
        assert_eq!(
            execution.observation().rung(),
            GraphExecutionRung::NativeBoundedRecursiveQuery
        );
        assert!(
            execution
                .observation()
                .operators()
                .contains(&GraphNativeOperator::RecursiveQueryDistinct)
        );
    }

    #[tokio::test]
    async fn partition_and_batch_layout_preserve_deterministic_graph_rows() {
        let compact_context = SessionContext::new();
        let compact = input(
            &compact_context,
            "compact_edges",
            &[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")],
        )
        .await;
        let compact = compile_bounded_reachability(compact, &bindings(), bounds(3, 32))
            .expect("compile compact graph")
            .execute(&compact_context)
            .await
            .expect("execute compact graph");

        let fragmented_context = SessionContext::new();
        let fragmented = input_partitions(
            &fragmented_context,
            "fragmented_edges",
            vec![
                vec![edge_batch(&[("c", "d")]), edge_batch(&[("a", "b")])],
                vec![edge_batch(&[("b", "d")]), edge_batch(&[("a", "c")])],
            ],
        )
        .await;
        let fragmented = compile_bounded_reachability(fragmented, &bindings(), bounds(3, 32))
            .expect("compile fragmented graph")
            .execute(&fragmented_context)
            .await
            .expect("execute fragmented graph");

        assert_eq!(rows(&compact), rows(&fragmented));
        assert_eq!(compact.schema().as_ref(), fragmented.schema().as_ref());
        assert_eq!(compact.observation(), fragmented.observation());
    }

    #[tokio::test]
    async fn cancellation_is_typed_reusable_and_releases_graph_resources() {
        let context = SessionContext::new();
        let graph = input(&context, "cancelled_edges", &[("a", "b"), ("b", "c")]).await;
        let compiled = compile_bounded_reachability(graph, &bindings(), bounds(3, 32))
            .expect("compile cancellable graph");
        let cancellation = Cancellation::with_check_interval(1);
        cancellation.cancel();
        let memory_before = context.state().runtime_env().memory_pool.reserved();

        assert!(matches!(
            compiled
                .execute_with_cancellation(&context, &cancellation)
                .await,
            Err(GraphProgramError::Cancelled)
        ));
        assert_eq!(
            context.state().runtime_env().memory_pool.reserved(),
            memory_before,
            "cancelled graph execution must not retain a DataFusion memory reservation"
        );
        assert!(
            compiled
                .observation()
                .dependencies()
                .contains(&GraphCompilationDependency::DataFusionExecuteStreamDropAbort)
        );

        let completed = compiled
            .execute(&context)
            .await
            .expect("a cancelled attempt cannot poison the reusable logical plan");
        assert!(!rows(&completed).is_empty());
        assert_eq!(
            context.state().runtime_env().memory_pool.reserved(),
            memory_before,
            "completed graph execution must release its DataFusion memory reservation"
        );
    }

    #[tokio::test]
    async fn zero_edges_preserve_the_contract_output_schema() {
        let context = SessionContext::new();
        let graph = input(&context, "empty_edges", &[]).await;
        let compiled = compile_bounded_reachability(graph, &bindings(), bounds(4, 10))
            .expect("compile empty graph");
        assert_eq!(compiled.output_schema().as_ref(), output_schema().as_ref());

        let execution = compiled
            .execute(&context)
            .await
            .expect("execute empty graph");
        assert_eq!(execution.schema().as_ref(), output_schema().as_ref());
        assert!(rows(&execution).is_empty());
    }

    #[tokio::test]
    async fn depth_and_output_limits_are_enforced_without_partial_success() {
        let context = SessionContext::new();
        let graph = input(&context, "bounded_edges", &[("a", "b"), ("b", "c")]).await;
        let compiled = compile_bounded_reachability(graph, &bindings(), bounds(1, 10))
            .expect("compile depth-one graph");
        assert_eq!(
            rows(&compiled.execute(&context).await.expect("depth-one result")),
            vec![("a".into(), "b".into(), 1), ("b".into(), "c".into(), 1)]
        );

        let graph = input(&context, "row_bounded_edges", &[("a", "b"), ("b", "c")]).await;
        let compiled = compile_bounded_reachability(graph, &bindings(), bounds(2, 1))
            .expect("compile row-bounded graph");
        let error = compiled
            .execute(&context)
            .await
            .expect_err("overflow probe must reject partial output");
        assert!(matches!(
            error,
            GraphProgramError::OutputRowsExceeded {
                limit: 1,
                observed: 2
            }
        ));
    }

    #[tokio::test]
    async fn recursive_plan_remains_optimizer_visible_and_has_no_extension() {
        let context = SessionContext::new();
        let graph = input(&context, "visible_edges", &[("a", "b"), ("b", "c")]).await;
        let compiled = compile_bounded_reachability(graph, &bindings(), bounds(2, 10))
            .expect("compile visible graph");
        let logical = compiled.logical_plan().display_indent().to_string();
        for operator in ["RecursiveQuery", "Join", "Aggregate", "Sort", "Limit"] {
            assert!(logical.contains(operator), "missing {operator}:\n{logical}");
        }
        assert!(!logical.contains("Extension"));

        let optimized = context
            .state()
            .optimize(compiled.logical_plan())
            .expect("optimize recursive graph");
        let physical = context
            .state()
            .create_physical_plan(&optimized)
            .await
            .expect("physical recursive graph");
        let physical = displayable(physical.as_ref()).indent(true).to_string();
        assert!(physical.contains("RecursiveQueryExec"), "{physical}");
        assert!(physical.contains("HashJoinExec"), "{physical}");
        assert!(!physical.contains("Extension"), "{physical}");
    }

    #[test]
    fn invalid_bindings_and_bounds_fail_before_plan_construction() {
        let duplicate_endpoint = ReachabilityBindings::try_new(
            "invalid",
            relation_id("canonical.call_edge"),
            edge_schema(),
            field_id("from"),
            field_id("from"),
            relation_id("derived.call_reachability"),
            output_schema(),
            field_id("reachable_from"),
            field_id("reachable_to"),
            field_id("minimum_depth"),
            "v1",
        )
        .expect_err("duplicate endpoint");
        assert!(matches!(
            duplicate_endpoint,
            GraphProgramError::DuplicateEndpointBinding(_)
        ));

        let nullable_edges = Arc::new(Schema::new(vec![
            Field::new("from", DataType::Utf8, true),
            Field::new("to", DataType::Utf8, false),
        ]));
        let nullable = ReachabilityBindings::try_new(
            "invalid",
            relation_id("canonical.call_edge"),
            nullable_edges,
            field_id("from"),
            field_id("to"),
            relation_id("derived.call_reachability"),
            output_schema(),
            field_id("reachable_from"),
            field_id("reachable_to"),
            field_id("minimum_depth"),
            "v1",
        )
        .expect_err("nullable endpoint");
        assert!(matches!(
            nullable,
            GraphProgramError::NullableEndpoint { .. }
        ));

        assert!(matches!(
            GraphResourceBounds::try_new(0, 1, 1, 1),
            Err(GraphProgramError::ZeroResourceBound("max_depth"))
        ));
    }
}
