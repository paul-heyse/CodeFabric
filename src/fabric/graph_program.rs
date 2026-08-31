//! Native, bounded graph programs compiled into DataFusion logical operators.
//!
//! Reachability is a relational program: the edge relation, endpoint fields, output relation,
//! output fields, implementation release, and resource envelope are supplied as data. The
//! compiler selects DataFusion's bounded recursive-query rung and emits no SQL text, graph-index
//! identity, row-oriented graph kernel, or opaque logical extension.

use std::collections::BTreeSet;
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
use thiserror::Error;

use crate::relational_program::{FieldId, RelationId};

const EDGE_SEED_ALIAS: &str = "__codefabric_graph_edge_seed";
const EDGE_STEP_ALIAS: &str = "__codefabric_graph_edge_step";
const FRONTIER_ALIAS: &str = "__codefabric_graph_frontier";
const INTERNAL_SOURCE: &str = "__codefabric_graph_source";
const INTERNAL_TARGET: &str = "__codefabric_graph_target";
const INTERNAL_DEPTH: &str = "__codefabric_graph_depth";
const INTERNAL_MINIMUM_DEPTH: &str = "__codefabric_graph_minimum_depth";

/// One exact model-bound relation supplied to the graph compiler.
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

/// Complete model-supplied field and relation bindings for bounded reachability.
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
    /// Validate a model-selected reachability contract before any plan is built.
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

/// Model/runtime dependencies observed while compiling the actual operation.
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
        let optimized = context.state().optimize(&self.plan)?;
        let physical = context.state().create_physical_plan(&optimized).await?;
        let mut stream = execute_stream(physical, context.task_ctx())?;
        let bounds = self.observation.bounds;
        let mut batches = Vec::new();
        let mut rows = 0_usize;
        let mut bytes = 0_usize;
        while let Some(batch) = stream.next().await {
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
    #[error("output-row bound cannot reserve one overflow-probe row")]
    ResourceProbeOverflow,
    #[error("edge relation mismatch: expected {expected:?}, actual {actual:?}")]
    InputRelationMismatch { expected: String, actual: String },
    #[error("edge input schema differs from its model binding")]
    InputSchemaMismatch {
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("compiled reachability schema differs from its model binding")]
    CompiledOutputSchemaMismatch {
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("executed reachability schema differs from its model binding")]
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

    async fn input(
        context: &SessionContext,
        table: &str,
        edges: &[(&str, &str)],
    ) -> GraphRelationInput {
        let batch = RecordBatch::try_new(
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
        .expect("edge batch");
        let provider =
            Arc::new(MemTable::try_new(edge_schema(), vec![vec![batch]]).expect("edge mem table"));
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
    async fn zero_edges_preserve_the_model_output_schema() {
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
