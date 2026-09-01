//! Execution-proved closure from accepted fact families to derived-analysis producers.
//!
//! All relation, field, authority, semantic-class, and release identities come from the installed
//! application contract. The compiler binds those identities and execution-count evidence to exact Arrow
//! schemas and constructs ordinary DataFusion logical operators. It owns no fact-family registry
//! and no SQL text. A family is closed only by exactly one execution-proved complete,
//! application-owned runtime producer or exactly one application-owned unsupported remainder.
//! Query requirements traverse the same closure and preserve unsupported, unknown, invalid, and
//! missing states.

use std::collections::BTreeSet;
use std::num::{NonZeroU16, NonZeroUsize};
use std::ops::Not;
use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::{DataType, SchemaRef};
use datafusion::common::{Column, ScalarValue, TableReference};
use datafusion::datasource::cte_worktable::CteWorkTable;
use datafusion::datasource::provider_as_source;
use datafusion::execution::context::SessionContext;
use datafusion::functions::core::expr_fn::coalesce;
use datafusion::functions_aggregate::expr_fn::{count, count_distinct, min};
use datafusion::logical_expr::{Expr, JoinType, LogicalPlan, LogicalPlanBuilder};
use datafusion::physical_plan::execute_stream;
use datafusion::prelude::{col, lit};
use futures::StreamExt;
use thiserror::Error;

use crate::relational_program::{FieldId, RelationId};

const ACCEPTED_ALIAS: &str = "__codefabric_accepted_family";
const PRODUCER_ALIAS: &str = "__codefabric_runtime_producer";
const REMAINDER_ALIAS: &str = "__codefabric_unsupported_remainder";
const FAMILY_ALIAS: &str = "__codefabric_family_closure";
const QUERY_EDGE_ALIAS: &str = "__codefabric_query_edge";
const QUERY_FRONTIER_ALIAS: &str = "__codefabric_query_frontier";
const QUERY_REACH_ALIAS: &str = "__codefabric_query_reach";
const QUERY_SOURCE_ALIAS: &str = "__codefabric_query_source";
const QUERY_RECURSIVE_NAME: &str = "__codefabric_query_requirement_recursive";

const FAMILY: &str = "__cf_family";
const SEMANTIC_CLASS: &str = "__cf_semantic_class";
const SEMANTIC_CLASS_MIN: &str = "__cf_semantic_class_min";
const ACCEPTED_COUNT: &str = "__cf_accepted_count";
const SEMANTIC_CLASS_COUNT: &str = "__cf_semantic_class_count";
const PRODUCER_COUNT: &str = "__cf_producer_count";
const PRODUCER: &str = "__cf_producer";
const PRODUCER_AUTHORITY: &str = "__cf_producer_authority";
const ALGORITHM_RELEASE: &str = "__cf_algorithm_release";
const PRECISION: &str = "__cf_precision";
const INPUT_PIN: &str = "__cf_input_pin";
const INVALIDATION_PIN: &str = "__cf_invalidation_pin";
const MATERIALIZATION_PIN: &str = "__cf_materialization_pin";
const REQUESTED_UNITS: &str = "__cf_requested_units";
const COMPLETED_UNITS: &str = "__cf_completed_units";
const REMAINDER_UNITS: &str = "__cf_remainder_units";
const UNKNOWN_UNITS: &str = "__cf_unknown_units";
const COMPLETENESS_PROOF_PIN: &str = "__cf_completeness_proof_pin";
const PRODUCER_PROOF_PIN: &str = "__cf_producer_proof_pin";
const REMAINDER_COUNT: &str = "__cf_remainder_count";
const REMAINDER: &str = "__cf_remainder";
const REMAINDER_AUTHORITY: &str = "__cf_remainder_authority";
const REMAINDER_REASON: &str = "__cf_remainder_reason";
const REMAINDER_PROOF_PIN: &str = "__cf_remainder_proof_pin";
const CLOSURE_STATE: &str = "__cf_closure_state";
const QUERY_ROOT: &str = "__cf_query_root";
const QUERY_REQUIRED: &str = "__cf_query_required";
const QUERY_DEPTH: &str = "__cf_query_depth";
const QUERY_SOURCE_MARKER: &str = "__cf_query_source_marker";
const QUERY_STATE: &str = "__cf_query_state";
const QUERY_UNKNOWN_CAUSE: &str = "__cf_query_unknown_cause";

const STATE_SUPPORTED: &str = "supported";
const STATE_UNSUPPORTED: &str = "unsupported";
const STATE_UNKNOWN: &str = "unknown";
const STATE_INVALID: &str = "invalid";
const STATE_MISSING: &str = "missing";
const STATE_SATISFIED: &str = "satisfied";

/// One exact contract-bound relation supplied to the closure compiler.
#[derive(Clone, Debug)]
pub struct ProducerClosureRelationInput {
    relation_id: RelationId,
    plan: LogicalPlan,
}

impl ProducerClosureRelationInput {
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

/// The four typed relation inputs required by producer closure.
#[derive(Clone, Debug)]
pub struct DerivedProducerClosureInputs {
    pub accepted_fact_family: ProducerClosureRelationInput,
    pub runtime_producer: ProducerClosureRelationInput,
    pub query_family_requirement: ProducerClosureRelationInput,
    pub unsupported_remainder: ProducerClosureRelationInput,
}

/// An application relation plus its exact Arrow contract and role-to-field bindings.
#[derive(Clone, Debug)]
pub struct ProducerClosureRelationContract<F> {
    relation_id: RelationId,
    schema: SchemaRef,
    fields: F,
}

impl<F> ProducerClosureRelationContract<F> {
    #[must_use]
    pub fn new(relation_id: RelationId, schema: SchemaRef, fields: F) -> Self {
        Self {
            relation_id,
            schema,
            fields,
        }
    }

    #[must_use]
    pub const fn relation_id(&self) -> &RelationId {
        &self.relation_id
    }

    #[must_use]
    pub const fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    #[must_use]
    pub const fn fields(&self) -> &F {
        &self.fields
    }
}

/// Field roles in the `accepted_fact_family` runtime relation.
#[derive(Clone, Debug)]
pub struct AcceptedFactFamilyFields {
    pub family_id: FieldId,
    pub semantic_class_id: FieldId,
}

/// Field roles in the `runtime_producer` runtime relation.
#[derive(Clone, Debug)]
pub struct RuntimeProducerFields {
    pub family_id: FieldId,
    pub producer_id: FieldId,
    pub authority_id: FieldId,
    pub algorithm_release: FieldId,
    pub precision_id: FieldId,
    pub input_pin: FieldId,
    pub invalidation_pin: FieldId,
    pub materialization_pin: FieldId,
    pub requested_unit_count: FieldId,
    pub completed_unit_count: FieldId,
    pub remainder_unit_count: FieldId,
    pub unknown_unit_count: FieldId,
    pub completeness_proof_pin: FieldId,
    pub proof_pin: FieldId,
}

/// Field roles in the `query_family_requirement` runtime relation.
#[derive(Clone, Debug)]
pub struct QueryFamilyRequirementFields {
    pub query_family_id: FieldId,
    pub required_family_id: FieldId,
}

/// Field roles in the `unsupported_remainder` runtime relation.
#[derive(Clone, Debug)]
pub struct UnsupportedRemainderFields {
    pub family_id: FieldId,
    pub remainder_id: FieldId,
    pub authority_id: FieldId,
    pub reason_id: FieldId,
    pub proof_pin: FieldId,
}

/// Field roles in the emitted accepted-family closure relation.
#[derive(Clone, Debug)]
pub struct FamilyClosureFields {
    pub family_id: FieldId,
    pub semantic_class_id: FieldId,
    pub closure_state: FieldId,
    pub producer_id: FieldId,
    pub authority_id: FieldId,
    pub algorithm_release: FieldId,
    pub precision_id: FieldId,
    pub input_pin: FieldId,
    pub invalidation_pin: FieldId,
    pub materialization_pin: FieldId,
    pub requested_unit_count: FieldId,
    pub completed_unit_count: FieldId,
    pub remainder_unit_count: FieldId,
    pub unknown_unit_count: FieldId,
    pub completeness_proof_pin: FieldId,
    pub producer_proof_pin: FieldId,
    pub unsupported_remainder_id: FieldId,
    pub unsupported_reason_id: FieldId,
    pub unsupported_proof_pin: FieldId,
}

/// Field roles in the emitted transitive query-requirement closure relation.
#[derive(Clone, Debug)]
pub struct QueryRequirementClosureFields {
    pub query_family_id: FieldId,
    pub required_family_id: FieldId,
    pub minimum_depth: FieldId,
    pub requirement_state: FieldId,
    pub unknown_cause: FieldId,
}

/// Field roles in the emitted conformance-violation relation.
#[derive(Clone, Debug)]
pub struct ProducerClosureViolationFields {
    pub subject_kind: FieldId,
    pub subject_id: FieldId,
    pub violation_code: FieldId,
    pub related_id: FieldId,
}

/// Application-contract identities whose values execution must read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProducerClosureSemanticIdentities {
    application_owned_authority_id: Arc<str>,
    factual_semantic_class_id: Arc<str>,
}

impl ProducerClosureSemanticIdentities {
    /// Construct the semantic identities installed in the active fabric epoch.
    ///
    /// # Errors
    ///
    /// Rejects empty or unreasonably large identities.
    pub fn try_new(
        application_owned_authority_id: impl Into<Arc<str>>,
        factual_semantic_class_id: impl Into<Arc<str>>,
    ) -> Result<Self, DerivedProducerClosureError> {
        let identities = Self {
            application_owned_authority_id: application_owned_authority_id.into(),
            factual_semantic_class_id: factual_semantic_class_id.into(),
        };
        validate_text(
            "application-owned authority",
            &identities.application_owned_authority_id,
        )?;
        validate_text(
            "factual semantic class",
            &identities.factual_semantic_class_id,
        )?;
        Ok(identities)
    }

    #[must_use]
    pub const fn application_owned_authority_id(&self) -> &Arc<str> {
        &self.application_owned_authority_id
    }

    #[must_use]
    pub const fn factual_semantic_class_id(&self) -> &Arc<str> {
        &self.factual_semantic_class_id
    }
}

/// Complete application binding for input, output, and semantic identities.
#[derive(Clone, Debug)]
pub struct DerivedProducerClosureBindings {
    operation_id: Arc<str>,
    implementation_release: Arc<str>,
    semantic_identities: ProducerClosureSemanticIdentities,
    accepted_fact_family: ProducerClosureRelationContract<AcceptedFactFamilyFields>,
    runtime_producer: ProducerClosureRelationContract<RuntimeProducerFields>,
    query_family_requirement: ProducerClosureRelationContract<QueryFamilyRequirementFields>,
    unsupported_remainder: ProducerClosureRelationContract<UnsupportedRemainderFields>,
    family_closure: ProducerClosureRelationContract<FamilyClosureFields>,
    query_requirement_closure: ProducerClosureRelationContract<QueryRequirementClosureFields>,
    violation: ProducerClosureRelationContract<ProducerClosureViolationFields>,
}

impl DerivedProducerClosureBindings {
    /// Validate the complete application binding before any executable plan is built.
    ///
    /// # Errors
    ///
    /// Rejects duplicate relations/fields, invalid text identities, and any field type,
    /// nullability, order, or exact-schema mismatch.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        operation_id: impl Into<Arc<str>>,
        implementation_release: impl Into<Arc<str>>,
        semantic_identities: ProducerClosureSemanticIdentities,
        accepted_fact_family: ProducerClosureRelationContract<AcceptedFactFamilyFields>,
        runtime_producer: ProducerClosureRelationContract<RuntimeProducerFields>,
        query_family_requirement: ProducerClosureRelationContract<QueryFamilyRequirementFields>,
        unsupported_remainder: ProducerClosureRelationContract<UnsupportedRemainderFields>,
        family_closure: ProducerClosureRelationContract<FamilyClosureFields>,
        query_requirement_closure: ProducerClosureRelationContract<QueryRequirementClosureFields>,
        violation: ProducerClosureRelationContract<ProducerClosureViolationFields>,
    ) -> Result<Self, DerivedProducerClosureError> {
        let operation_id = operation_id.into();
        let implementation_release = implementation_release.into();
        validate_text("operation", &operation_id)?;
        validate_text("implementation release", &implementation_release)?;

        validate_relation_contracts(
            &accepted_fact_family,
            &runtime_producer,
            &query_family_requirement,
            &unsupported_remainder,
            &family_closure,
            &query_requirement_closure,
            &violation,
        )?;

        Ok(Self {
            operation_id,
            implementation_release,
            semantic_identities,
            accepted_fact_family,
            runtime_producer,
            query_family_requirement,
            unsupported_remainder,
            family_closure,
            query_requirement_closure,
            violation,
        })
    }

    #[must_use]
    pub const fn operation_id(&self) -> &Arc<str> {
        &self.operation_id
    }

    #[must_use]
    pub const fn implementation_release(&self) -> &Arc<str> {
        &self.implementation_release
    }

    #[must_use]
    pub const fn semantic_identities(&self) -> &ProducerClosureSemanticIdentities {
        &self.semantic_identities
    }
}

/// Request-local execution limits observed by compilation and enforced during streaming.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProducerClosureResourceBounds {
    max_query_depth: NonZeroU16,
    max_rows_per_relation: NonZeroUsize,
    max_total_batches: NonZeroUsize,
    max_total_bytes: NonZeroUsize,
}

impl ProducerClosureResourceBounds {
    /// Construct a non-zero closure resource envelope.
    ///
    /// # Errors
    ///
    /// Rejects zero limits and a row limit that cannot reserve one overflow-probe row.
    pub fn try_new(
        max_query_depth: u16,
        max_rows_per_relation: usize,
        max_total_batches: usize,
        max_total_bytes: usize,
    ) -> Result<Self, DerivedProducerClosureError> {
        let bounds = Self {
            max_query_depth: NonZeroU16::new(max_query_depth).ok_or(
                DerivedProducerClosureError::ZeroResourceBound("max_query_depth"),
            )?,
            max_rows_per_relation: NonZeroUsize::new(max_rows_per_relation).ok_or(
                DerivedProducerClosureError::ZeroResourceBound("max_rows_per_relation"),
            )?,
            max_total_batches: NonZeroUsize::new(max_total_batches).ok_or(
                DerivedProducerClosureError::ZeroResourceBound("max_total_batches"),
            )?,
            max_total_bytes: NonZeroUsize::new(max_total_bytes).ok_or(
                DerivedProducerClosureError::ZeroResourceBound("max_total_bytes"),
            )?,
        };
        bounds.probe_rows()?;
        Ok(bounds)
    }

    #[must_use]
    pub const fn max_query_depth(self) -> u16 {
        self.max_query_depth.get()
    }

    #[must_use]
    pub const fn max_rows_per_relation(self) -> usize {
        self.max_rows_per_relation.get()
    }

    #[must_use]
    pub const fn max_total_batches(self) -> usize {
        self.max_total_batches.get()
    }

    #[must_use]
    pub const fn max_total_bytes(self) -> usize {
        self.max_total_bytes.get()
    }

    fn probe_rows(self) -> Result<usize, DerivedProducerClosureError> {
        self.max_rows_per_relation
            .get()
            .checked_add(1)
            .ok_or(DerivedProducerClosureError::ResourceProbeOverflow)
    }
}

/// Highest viable extension rung used by this compiler.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProducerClosureExecutionRung {
    NativeLogicalPlans,
}

/// Native operators causally selected by successful closure compilation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProducerClosureNativeOperator {
    Projection,
    Aggregate,
    LeftJoin,
    LeftAntiJoin,
    Filter,
    RecursiveQueryDistinct,
    UnionAll,
    DeterministicSort,
    OutputOverflowProbeLimit,
}

/// Exact application/runtime dependency observed by the compiler that constructed the plans.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProducerClosureCompilationDependency {
    InputRelation(RelationId),
    InputField(FieldId),
    OutputRelation(RelationId),
    OutputField(FieldId),
    ApplicationOwnedAuthority(Arc<str>),
    FactualSemanticClass(Arc<str>),
    ImplementationRelease(Arc<str>),
    SessionMemoryPool,
    DataFusionExecuteStreamDropAbort,
}

/// Causal evidence for native operator, dependency, and resource selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProducerClosureCompilationObservation {
    operation_id: Arc<str>,
    rung: ProducerClosureExecutionRung,
    operators: BTreeSet<ProducerClosureNativeOperator>,
    dependencies: BTreeSet<ProducerClosureCompilationDependency>,
    bounds: ProducerClosureResourceBounds,
}

impl ProducerClosureCompilationObservation {
    #[must_use]
    pub const fn operation_id(&self) -> &Arc<str> {
        &self.operation_id
    }

    #[must_use]
    pub const fn rung(&self) -> ProducerClosureExecutionRung {
        self.rung
    }

    #[must_use]
    pub const fn operators(&self) -> &BTreeSet<ProducerClosureNativeOperator> {
        &self.operators
    }

    #[must_use]
    pub const fn dependencies(&self) -> &BTreeSet<ProducerClosureCompilationDependency> {
        &self.dependencies
    }

    #[must_use]
    pub const fn bounds(&self) -> ProducerClosureResourceBounds {
        self.bounds
    }
}

/// Three optimizer-visible native plans and their exact output contracts.
#[derive(Clone, Debug)]
pub struct CompiledDerivedProducerClosure {
    family_closure_plan: LogicalPlan,
    query_requirement_closure_plan: LogicalPlan,
    violation_plan: LogicalPlan,
    family_closure_schema: SchemaRef,
    query_requirement_closure_schema: SchemaRef,
    violation_schema: SchemaRef,
    observation: ProducerClosureCompilationObservation,
}

impl CompiledDerivedProducerClosure {
    #[must_use]
    pub const fn family_closure_plan(&self) -> &LogicalPlan {
        &self.family_closure_plan
    }

    #[must_use]
    pub const fn query_requirement_closure_plan(&self) -> &LogicalPlan {
        &self.query_requirement_closure_plan
    }

    #[must_use]
    pub const fn violation_plan(&self) -> &LogicalPlan {
        &self.violation_plan
    }

    #[must_use]
    pub const fn observation(&self) -> &ProducerClosureCompilationObservation {
        &self.observation
    }

    /// Execute all closure plans under one DataFusion session and one shared output budget.
    ///
    /// Empty results retain one zero-row batch with the declared Arrow schema. A non-empty
    /// violation relation is returned as typed evidence and makes `is_conformant` false; it is
    /// never collapsed into a persisted Boolean declaration.
    ///
    /// # Errors
    ///
    /// Returns a typed optimizer, planner, execution, schema, or resource-limit failure.
    pub async fn execute(
        &self,
        context: &SessionContext,
    ) -> Result<DerivedProducerClosureExecution, DerivedProducerClosureError> {
        let mut budget = ExecutionBudget::default();
        let family_closure = execute_bounded(
            context,
            &self.family_closure_plan,
            &self.family_closure_schema,
            self.observation.bounds,
            "family_closure",
            &mut budget,
        )
        .await?;
        let query_requirement_closure = execute_bounded(
            context,
            &self.query_requirement_closure_plan,
            &self.query_requirement_closure_schema,
            self.observation.bounds,
            "query_requirement_closure",
            &mut budget,
        )
        .await?;
        let violations = execute_bounded(
            context,
            &self.violation_plan,
            &self.violation_schema,
            self.observation.bounds,
            "violations",
            &mut budget,
        )
        .await?;

        Ok(DerivedProducerClosureExecution {
            family_closure_schema: Arc::clone(&self.family_closure_schema),
            family_closure,
            query_requirement_closure_schema: Arc::clone(&self.query_requirement_closure_schema),
            query_requirement_closure,
            violation_schema: Arc::clone(&self.violation_schema),
            violations,
            observation: self.observation.clone(),
        })
    }
}

/// Exact-schema closure output emitted after bounded DataFusion execution.
#[derive(Clone, Debug)]
pub struct DerivedProducerClosureExecution {
    family_closure_schema: SchemaRef,
    family_closure: Vec<RecordBatch>,
    query_requirement_closure_schema: SchemaRef,
    query_requirement_closure: Vec<RecordBatch>,
    violation_schema: SchemaRef,
    violations: Vec<RecordBatch>,
    observation: ProducerClosureCompilationObservation,
}

impl DerivedProducerClosureExecution {
    #[must_use]
    pub const fn family_closure_schema(&self) -> &SchemaRef {
        &self.family_closure_schema
    }

    #[must_use]
    pub fn family_closure(&self) -> &[RecordBatch] {
        &self.family_closure
    }

    #[must_use]
    pub const fn query_requirement_closure_schema(&self) -> &SchemaRef {
        &self.query_requirement_closure_schema
    }

    #[must_use]
    pub fn query_requirement_closure(&self) -> &[RecordBatch] {
        &self.query_requirement_closure
    }

    #[must_use]
    pub const fn violation_schema(&self) -> &SchemaRef {
        &self.violation_schema
    }

    #[must_use]
    pub fn violations(&self) -> &[RecordBatch] {
        &self.violations
    }

    #[must_use]
    pub const fn observation(&self) -> &ProducerClosureCompilationObservation {
        &self.observation
    }

    #[must_use]
    pub fn is_conformant(&self) -> bool {
        self.violations.iter().all(|batch| batch.num_rows() == 0)
    }
}

/// Compile producer, unsupported-remainder, and transitive query closure as native plans.
///
/// # Errors
///
/// Rejects relation/schema drift and any DataFusion logical-plan construction failure.
pub fn compile_derived_producer_closure(
    inputs: DerivedProducerClosureInputs,
    bindings: &DerivedProducerClosureBindings,
    bounds: ProducerClosureResourceBounds,
) -> Result<CompiledDerivedProducerClosure, DerivedProducerClosureError> {
    validate_input(
        &inputs.accepted_fact_family,
        &bindings.accepted_fact_family,
        "accepted_fact_family",
    )?;
    validate_input(
        &inputs.runtime_producer,
        &bindings.runtime_producer,
        "runtime_producer",
    )?;
    validate_input(
        &inputs.query_family_requirement,
        &bindings.query_family_requirement,
        "query_family_requirement",
    )?;
    validate_input(
        &inputs.unsupported_remainder,
        &bindings.unsupported_remainder,
        "unsupported_remainder",
    )?;

    let accepted = compile_accepted_aggregate(inputs.accepted_fact_family.plan, bindings)?;
    let producers = compile_producer_aggregate(inputs.runtime_producer.plan, bindings)?;
    let remainders = compile_remainder_aggregate(inputs.unsupported_remainder.plan, bindings)?;
    let enriched =
        compile_family_enriched(accepted.clone(), producers.clone(), remainders.clone())?;
    let family_closure_internal = compile_family_closure_internal(enriched.clone(), bindings)?;
    let family_closure_plan =
        compile_family_closure_output(family_closure_internal.clone(), bindings, bounds)?;

    let query_program = compile_query_closure_internal(
        inputs.query_family_requirement.plan,
        family_closure_internal.clone(),
        bindings,
        bounds,
    )?;
    let query_requirement_closure_plan =
        compile_query_closure_output(query_program.closure.clone(), bindings, bounds)?;
    let violation_plan = compile_violations(
        enriched,
        producers,
        remainders,
        accepted,
        query_program.closure,
        query_program.depth_exhaustion,
        bindings,
        bounds,
    )?;

    validate_compiled_schema(
        "family_closure",
        &family_closure_plan,
        &bindings.family_closure.schema,
    )?;
    validate_compiled_schema(
        "query_requirement_closure",
        &query_requirement_closure_plan,
        &bindings.query_requirement_closure.schema,
    )?;
    validate_compiled_schema("violation", &violation_plan, &bindings.violation.schema)?;

    Ok(CompiledDerivedProducerClosure {
        family_closure_plan,
        query_requirement_closure_plan,
        violation_plan,
        family_closure_schema: Arc::clone(&bindings.family_closure.schema),
        query_requirement_closure_schema: Arc::clone(&bindings.query_requirement_closure.schema),
        violation_schema: Arc::clone(&bindings.violation.schema),
        observation: ProducerClosureCompilationObservation {
            operation_id: Arc::clone(&bindings.operation_id),
            rung: ProducerClosureExecutionRung::NativeLogicalPlans,
            operators: BTreeSet::from([
                ProducerClosureNativeOperator::Projection,
                ProducerClosureNativeOperator::Aggregate,
                ProducerClosureNativeOperator::LeftJoin,
                ProducerClosureNativeOperator::LeftAntiJoin,
                ProducerClosureNativeOperator::Filter,
                ProducerClosureNativeOperator::RecursiveQueryDistinct,
                ProducerClosureNativeOperator::UnionAll,
                ProducerClosureNativeOperator::DeterministicSort,
                ProducerClosureNativeOperator::OutputOverflowProbeLimit,
            ]),
            dependencies: observe_dependencies(bindings),
            bounds,
        },
    })
}

fn compile_accepted_aggregate(
    plan: LogicalPlan,
    bindings: &DerivedProducerClosureBindings,
) -> Result<LogicalPlan, DerivedProducerClosureError> {
    let fields = &bindings.accepted_fact_family.fields;
    let aggregated = LogicalPlanBuilder::from(plan)
        .project([
            col(fields.family_id.as_str()).alias(FAMILY),
            col(fields.semantic_class_id.as_str()).alias(SEMANTIC_CLASS),
        ])?
        .aggregate(
            [col(FAMILY)],
            [
                count(col(FAMILY)).alias(ACCEPTED_COUNT),
                count_distinct(col(SEMANTIC_CLASS)).alias(SEMANTIC_CLASS_COUNT),
                min(col(SEMANTIC_CLASS)).alias(SEMANTIC_CLASS_MIN),
            ],
        )?
        .build()?;
    Ok(LogicalPlanBuilder::from(aggregated)
        .project([
            col(FAMILY),
            col(ACCEPTED_COUNT),
            col(SEMANTIC_CLASS_COUNT),
            coalesce(vec![col(SEMANTIC_CLASS_MIN), lit("")]).alias(SEMANTIC_CLASS),
        ])?
        .build()?)
}

fn compile_producer_aggregate(
    plan: LogicalPlan,
    bindings: &DerivedProducerClosureBindings,
) -> Result<LogicalPlan, DerivedProducerClosureError> {
    let fields = &bindings.runtime_producer.fields;
    let projected = LogicalPlanBuilder::from(plan)
        .project([
            col(fields.family_id.as_str()).alias(FAMILY),
            col(fields.producer_id.as_str()).alias(PRODUCER),
            col(fields.authority_id.as_str()).alias(PRODUCER_AUTHORITY),
            col(fields.algorithm_release.as_str()).alias(ALGORITHM_RELEASE),
            col(fields.precision_id.as_str()).alias(PRECISION),
            col(fields.input_pin.as_str()).alias(INPUT_PIN),
            col(fields.invalidation_pin.as_str()).alias(INVALIDATION_PIN),
            col(fields.materialization_pin.as_str()).alias(MATERIALIZATION_PIN),
            col(fields.requested_unit_count.as_str()).alias(REQUESTED_UNITS),
            col(fields.completed_unit_count.as_str()).alias(COMPLETED_UNITS),
            col(fields.remainder_unit_count.as_str()).alias(REMAINDER_UNITS),
            col(fields.unknown_unit_count.as_str()).alias(UNKNOWN_UNITS),
            col(fields.completeness_proof_pin.as_str()).alias(COMPLETENESS_PROOF_PIN),
            col(fields.proof_pin.as_str()).alias(PRODUCER_PROOF_PIN),
        ])?
        .build()?;
    Ok(LogicalPlanBuilder::from(projected)
        .aggregate(
            [col(FAMILY)],
            [
                count(col(FAMILY)).alias(PRODUCER_COUNT),
                min(col(PRODUCER)).alias(PRODUCER),
                min(col(PRODUCER_AUTHORITY)).alias(PRODUCER_AUTHORITY),
                min(col(ALGORITHM_RELEASE)).alias(ALGORITHM_RELEASE),
                min(col(PRECISION)).alias(PRECISION),
                min(col(INPUT_PIN)).alias(INPUT_PIN),
                min(col(INVALIDATION_PIN)).alias(INVALIDATION_PIN),
                min(col(MATERIALIZATION_PIN)).alias(MATERIALIZATION_PIN),
                min(col(REQUESTED_UNITS)).alias(REQUESTED_UNITS),
                min(col(COMPLETED_UNITS)).alias(COMPLETED_UNITS),
                min(col(REMAINDER_UNITS)).alias(REMAINDER_UNITS),
                min(col(UNKNOWN_UNITS)).alias(UNKNOWN_UNITS),
                min(col(COMPLETENESS_PROOF_PIN)).alias(COMPLETENESS_PROOF_PIN),
                min(col(PRODUCER_PROOF_PIN)).alias(PRODUCER_PROOF_PIN),
            ],
        )?
        .build()?)
}

fn compile_remainder_aggregate(
    plan: LogicalPlan,
    bindings: &DerivedProducerClosureBindings,
) -> Result<LogicalPlan, DerivedProducerClosureError> {
    let fields = &bindings.unsupported_remainder.fields;
    let projected = LogicalPlanBuilder::from(plan)
        .project([
            col(fields.family_id.as_str()).alias(FAMILY),
            col(fields.remainder_id.as_str()).alias(REMAINDER),
            col(fields.authority_id.as_str()).alias(REMAINDER_AUTHORITY),
            col(fields.reason_id.as_str()).alias(REMAINDER_REASON),
            col(fields.proof_pin.as_str()).alias(REMAINDER_PROOF_PIN),
        ])?
        .build()?;
    Ok(LogicalPlanBuilder::from(projected)
        .aggregate(
            [col(FAMILY)],
            [
                count(col(FAMILY)).alias(REMAINDER_COUNT),
                min(col(REMAINDER)).alias(REMAINDER),
                min(col(REMAINDER_AUTHORITY)).alias(REMAINDER_AUTHORITY),
                min(col(REMAINDER_REASON)).alias(REMAINDER_REASON),
                min(col(REMAINDER_PROOF_PIN)).alias(REMAINDER_PROOF_PIN),
            ],
        )?
        .build()?)
}

fn compile_family_enriched(
    accepted: LogicalPlan,
    producers: LogicalPlan,
    remainders: LogicalPlan,
) -> Result<LogicalPlan, DerivedProducerClosureError> {
    let accepted = LogicalPlanBuilder::from(accepted)
        .alias(ACCEPTED_ALIAS)?
        .build()?;
    let producers = LogicalPlanBuilder::from(producers)
        .alias(PRODUCER_ALIAS)?
        .build()?;
    let accepted_and_producer = LogicalPlanBuilder::from(accepted)
        .join_on(
            producers,
            JoinType::Left,
            [qualified(ACCEPTED_ALIAS, FAMILY).eq(qualified(PRODUCER_ALIAS, FAMILY))],
        )?
        .project([
            qualified(ACCEPTED_ALIAS, FAMILY).alias(FAMILY),
            qualified(ACCEPTED_ALIAS, SEMANTIC_CLASS).alias(SEMANTIC_CLASS),
            qualified(ACCEPTED_ALIAS, ACCEPTED_COUNT).alias(ACCEPTED_COUNT),
            qualified(ACCEPTED_ALIAS, SEMANTIC_CLASS_COUNT).alias(SEMANTIC_CLASS_COUNT),
            coalesce(vec![qualified(PRODUCER_ALIAS, PRODUCER_COUNT), lit(0_i64)])
                .alias(PRODUCER_COUNT),
            qualified(PRODUCER_ALIAS, PRODUCER).alias(PRODUCER),
            qualified(PRODUCER_ALIAS, PRODUCER_AUTHORITY).alias(PRODUCER_AUTHORITY),
            qualified(PRODUCER_ALIAS, ALGORITHM_RELEASE).alias(ALGORITHM_RELEASE),
            qualified(PRODUCER_ALIAS, PRECISION).alias(PRECISION),
            qualified(PRODUCER_ALIAS, INPUT_PIN).alias(INPUT_PIN),
            qualified(PRODUCER_ALIAS, INVALIDATION_PIN).alias(INVALIDATION_PIN),
            qualified(PRODUCER_ALIAS, MATERIALIZATION_PIN).alias(MATERIALIZATION_PIN),
            qualified(PRODUCER_ALIAS, REQUESTED_UNITS).alias(REQUESTED_UNITS),
            qualified(PRODUCER_ALIAS, COMPLETED_UNITS).alias(COMPLETED_UNITS),
            qualified(PRODUCER_ALIAS, REMAINDER_UNITS).alias(REMAINDER_UNITS),
            qualified(PRODUCER_ALIAS, UNKNOWN_UNITS).alias(UNKNOWN_UNITS),
            qualified(PRODUCER_ALIAS, COMPLETENESS_PROOF_PIN).alias(COMPLETENESS_PROOF_PIN),
            qualified(PRODUCER_ALIAS, PRODUCER_PROOF_PIN).alias(PRODUCER_PROOF_PIN),
        ])?
        .alias(FAMILY_ALIAS)?
        .build()?;
    let remainders = LogicalPlanBuilder::from(remainders)
        .alias(REMAINDER_ALIAS)?
        .build()?;

    Ok(LogicalPlanBuilder::from(accepted_and_producer)
        .join_on(
            remainders,
            JoinType::Left,
            [qualified(FAMILY_ALIAS, FAMILY).eq(qualified(REMAINDER_ALIAS, FAMILY))],
        )?
        .project([
            qualified(FAMILY_ALIAS, FAMILY).alias(FAMILY),
            qualified(FAMILY_ALIAS, SEMANTIC_CLASS).alias(SEMANTIC_CLASS),
            qualified(FAMILY_ALIAS, ACCEPTED_COUNT).alias(ACCEPTED_COUNT),
            qualified(FAMILY_ALIAS, SEMANTIC_CLASS_COUNT).alias(SEMANTIC_CLASS_COUNT),
            qualified(FAMILY_ALIAS, PRODUCER_COUNT).alias(PRODUCER_COUNT),
            qualified(FAMILY_ALIAS, PRODUCER).alias(PRODUCER),
            qualified(FAMILY_ALIAS, PRODUCER_AUTHORITY).alias(PRODUCER_AUTHORITY),
            qualified(FAMILY_ALIAS, ALGORITHM_RELEASE).alias(ALGORITHM_RELEASE),
            qualified(FAMILY_ALIAS, PRECISION).alias(PRECISION),
            qualified(FAMILY_ALIAS, INPUT_PIN).alias(INPUT_PIN),
            qualified(FAMILY_ALIAS, INVALIDATION_PIN).alias(INVALIDATION_PIN),
            qualified(FAMILY_ALIAS, MATERIALIZATION_PIN).alias(MATERIALIZATION_PIN),
            qualified(FAMILY_ALIAS, REQUESTED_UNITS).alias(REQUESTED_UNITS),
            qualified(FAMILY_ALIAS, COMPLETED_UNITS).alias(COMPLETED_UNITS),
            qualified(FAMILY_ALIAS, REMAINDER_UNITS).alias(REMAINDER_UNITS),
            qualified(FAMILY_ALIAS, UNKNOWN_UNITS).alias(UNKNOWN_UNITS),
            qualified(FAMILY_ALIAS, COMPLETENESS_PROOF_PIN).alias(COMPLETENESS_PROOF_PIN),
            qualified(FAMILY_ALIAS, PRODUCER_PROOF_PIN).alias(PRODUCER_PROOF_PIN),
            coalesce(vec![
                qualified(REMAINDER_ALIAS, REMAINDER_COUNT),
                lit(0_i64),
            ])
            .alias(REMAINDER_COUNT),
            qualified(REMAINDER_ALIAS, REMAINDER).alias(REMAINDER),
            qualified(REMAINDER_ALIAS, REMAINDER_AUTHORITY).alias(REMAINDER_AUTHORITY),
            qualified(REMAINDER_ALIAS, REMAINDER_REASON).alias(REMAINDER_REASON),
            qualified(REMAINDER_ALIAS, REMAINDER_PROOF_PIN).alias(REMAINDER_PROOF_PIN),
        ])?
        .build()?)
}

fn compile_family_closure_internal(
    enriched: LogicalPlan,
    bindings: &DerivedProducerClosureBindings,
) -> Result<LogicalPlan, DerivedProducerClosureError> {
    let semantic = &bindings.semantic_identities;
    let producer_contract_present = all_non_empty([
        PRODUCER,
        ALGORITHM_RELEASE,
        PRECISION,
        INPUT_PIN,
        INVALIDATION_PIN,
        MATERIALIZATION_PIN,
        COMPLETENESS_PROOF_PIN,
        PRODUCER_PROOF_PIN,
    ]);
    let remainder_contract_present =
        all_non_empty([REMAINDER, REMAINDER_REASON, REMAINDER_PROOF_PIN]);
    let accepted_valid = col(ACCEPTED_COUNT)
        .eq(lit(1_i64))
        .and(col(SEMANTIC_CLASS_COUNT).eq(lit(1_i64)))
        .and(col(SEMANTIC_CLASS).eq(lit(semantic.factual_semantic_class_id.as_ref())));
    let producer_owned =
        col(PRODUCER_AUTHORITY).eq(lit(semantic.application_owned_authority_id.as_ref()));
    let remainder_owned =
        col(REMAINDER_AUTHORITY).eq(lit(semantic.application_owned_authority_id.as_ref()));
    let producer_exclusive = col(PRODUCER_COUNT)
        .eq(lit(1_i64))
        .and(col(REMAINDER_COUNT).eq(lit(0_i64)));
    let remainder_exclusive = col(PRODUCER_COUNT)
        .eq(lit(0_i64))
        .and(col(REMAINDER_COUNT).eq(lit(1_i64)));
    let producer_complete = col(REQUESTED_UNITS)
        .eq(col(COMPLETED_UNITS))
        .and(col(REMAINDER_UNITS).eq(lit(0_u64)))
        .and(col(UNKNOWN_UNITS).eq(lit(0_u64)));

    let state = datafusion::logical_expr::expr_fn::when(
        accepted_valid
            .clone()
            .and(producer_exclusive.clone())
            .and(producer_owned.clone())
            .and(producer_contract_present.clone())
            .and(producer_complete.clone()),
        lit(STATE_SUPPORTED),
    )
    .when(
        accepted_valid
            .clone()
            .and(producer_exclusive)
            .and(producer_owned)
            .and(producer_contract_present)
            .and(producer_complete.not()),
        lit(STATE_UNKNOWN),
    )
    .when(
        accepted_valid
            .and(remainder_exclusive)
            .and(remainder_owned)
            .and(remainder_contract_present),
        lit(STATE_UNSUPPORTED),
    )
    .otherwise(lit(STATE_INVALID))?
    .alias(CLOSURE_STATE);

    Ok(LogicalPlanBuilder::from(enriched)
        .project([
            col(FAMILY),
            col(SEMANTIC_CLASS),
            state,
            col(PRODUCER),
            col(PRODUCER_AUTHORITY),
            col(ALGORITHM_RELEASE),
            col(PRECISION),
            col(INPUT_PIN),
            col(INVALIDATION_PIN),
            col(MATERIALIZATION_PIN),
            col(REQUESTED_UNITS),
            col(COMPLETED_UNITS),
            col(REMAINDER_UNITS),
            col(UNKNOWN_UNITS),
            col(COMPLETENESS_PROOF_PIN),
            col(PRODUCER_PROOF_PIN),
            col(REMAINDER),
            col(REMAINDER_REASON),
            col(REMAINDER_PROOF_PIN),
        ])?
        .build()?)
}

fn compile_family_closure_output(
    closure: LogicalPlan,
    bindings: &DerivedProducerClosureBindings,
    bounds: ProducerClosureResourceBounds,
) -> Result<LogicalPlan, DerivedProducerClosureError> {
    let fields = &bindings.family_closure.fields;
    Ok(LogicalPlanBuilder::from(closure)
        .project([
            col(FAMILY).alias(fields.family_id.as_str()),
            col(SEMANTIC_CLASS).alias(fields.semantic_class_id.as_str()),
            col(CLOSURE_STATE).alias(fields.closure_state.as_str()),
            col(PRODUCER).alias(fields.producer_id.as_str()),
            col(PRODUCER_AUTHORITY).alias(fields.authority_id.as_str()),
            col(ALGORITHM_RELEASE).alias(fields.algorithm_release.as_str()),
            col(PRECISION).alias(fields.precision_id.as_str()),
            col(INPUT_PIN).alias(fields.input_pin.as_str()),
            col(INVALIDATION_PIN).alias(fields.invalidation_pin.as_str()),
            col(MATERIALIZATION_PIN).alias(fields.materialization_pin.as_str()),
            col(REQUESTED_UNITS).alias(fields.requested_unit_count.as_str()),
            col(COMPLETED_UNITS).alias(fields.completed_unit_count.as_str()),
            col(REMAINDER_UNITS).alias(fields.remainder_unit_count.as_str()),
            col(UNKNOWN_UNITS).alias(fields.unknown_unit_count.as_str()),
            col(COMPLETENESS_PROOF_PIN).alias(fields.completeness_proof_pin.as_str()),
            col(PRODUCER_PROOF_PIN).alias(fields.producer_proof_pin.as_str()),
            col(REMAINDER).alias(fields.unsupported_remainder_id.as_str()),
            col(REMAINDER_REASON).alias(fields.unsupported_reason_id.as_str()),
            col(REMAINDER_PROOF_PIN).alias(fields.unsupported_proof_pin.as_str()),
        ])?
        .sort([col(fields.family_id.as_str()).sort(true, false)])?
        .limit(0, Some(bounds.probe_rows()?))?
        .build()?)
}

struct QueryClosureInternal {
    closure: LogicalPlan,
    depth_exhaustion: LogicalPlan,
}

fn compile_query_closure_internal(
    query_plan: LogicalPlan,
    family_closure: LogicalPlan,
    bindings: &DerivedProducerClosureBindings,
    bounds: ProducerClosureResourceBounds,
) -> Result<QueryClosureInternal, DerivedProducerClosureError> {
    let fields = &bindings.query_family_requirement.fields;
    let edges = LogicalPlanBuilder::from(query_plan)
        .project([
            col(fields.query_family_id.as_str()).alias(QUERY_ROOT),
            col(fields.required_family_id.as_str()).alias(QUERY_REQUIRED),
        ])?
        .distinct()?
        .alias(QUERY_EDGE_ALIAS)?
        .build()?;
    let seed = LogicalPlanBuilder::from(edges.clone())
        .project([
            qualified(QUERY_EDGE_ALIAS, QUERY_ROOT).alias(QUERY_ROOT),
            qualified(QUERY_EDGE_ALIAS, QUERY_REQUIRED).alias(QUERY_REQUIRED),
            lit(1_u32).alias(QUERY_DEPTH),
        ])?
        .build()?;
    let work_table = Arc::new(CteWorkTable::new(
        QUERY_RECURSIVE_NAME,
        Arc::new(seed.schema().as_arrow().clone()),
    ));
    let frontier =
        LogicalPlanBuilder::scan(QUERY_RECURSIVE_NAME, provider_as_source(work_table), None)?
            .alias(QUERY_FRONTIER_ALIAS)?
            .build()?;
    let recursive_term = LogicalPlanBuilder::from(frontier)
        .filter(
            qualified(QUERY_FRONTIER_ALIAS, QUERY_DEPTH)
                .lt(lit(u32::from(bounds.max_query_depth()))),
        )?
        .join_on(
            edges.clone(),
            JoinType::Inner,
            [qualified(QUERY_FRONTIER_ALIAS, QUERY_REQUIRED)
                .eq(qualified(QUERY_EDGE_ALIAS, QUERY_ROOT))],
        )?
        .project([
            qualified(QUERY_FRONTIER_ALIAS, QUERY_ROOT).alias(QUERY_ROOT),
            qualified(QUERY_EDGE_ALIAS, QUERY_REQUIRED).alias(QUERY_REQUIRED),
            (qualified(QUERY_FRONTIER_ALIAS, QUERY_DEPTH) + lit(1_u32)).alias(QUERY_DEPTH),
        ])?
        .build()?;
    let recursive = LogicalPlanBuilder::from(seed)
        .to_recursive_query(QUERY_RECURSIVE_NAME.to_owned(), recursive_term, true)?
        .build()?;

    let reach = LogicalPlanBuilder::from(recursive.clone())
        .aggregate(
            [col(QUERY_ROOT), col(QUERY_REQUIRED)],
            [min(col(QUERY_DEPTH)).alias(QUERY_DEPTH)],
        )?
        .alias(QUERY_REACH_ALIAS)?
        .build()?;
    let query_sources = LogicalPlanBuilder::from(edges.clone())
        .project([qualified(QUERY_EDGE_ALIAS, QUERY_ROOT).alias(QUERY_SOURCE_MARKER)])?
        .distinct()?
        .alias(QUERY_SOURCE_ALIAS)?
        .build()?;
    let family_closure = LogicalPlanBuilder::from(family_closure)
        .alias(FAMILY_ALIAS)?
        .build()?;
    let reach_and_family = LogicalPlanBuilder::from(reach)
        .join_on(
            family_closure,
            JoinType::Left,
            [qualified(QUERY_REACH_ALIAS, QUERY_REQUIRED).eq(qualified(FAMILY_ALIAS, FAMILY))],
        )?
        .build()?;
    let joined = LogicalPlanBuilder::from(reach_and_family)
        .join_on(
            query_sources,
            JoinType::Left,
            [qualified(QUERY_REACH_ALIAS, QUERY_REQUIRED)
                .eq(qualified(QUERY_SOURCE_ALIAS, QUERY_SOURCE_MARKER))],
        )?
        .filter(
            qualified(FAMILY_ALIAS, FAMILY).is_not_null().or(qualified(
                QUERY_SOURCE_ALIAS,
                QUERY_SOURCE_MARKER,
            )
            .is_null()),
        )?
        .build()?;

    let state = datafusion::logical_expr::expr_fn::when(
        qualified(FAMILY_ALIAS, FAMILY).is_null(),
        lit(STATE_MISSING),
    )
    .when(
        qualified(FAMILY_ALIAS, CLOSURE_STATE).eq(lit(STATE_SUPPORTED)),
        lit(STATE_SATISFIED),
    )
    .when(
        qualified(FAMILY_ALIAS, CLOSURE_STATE).eq(lit(STATE_UNSUPPORTED)),
        lit(STATE_UNSUPPORTED),
    )
    .when(
        qualified(FAMILY_ALIAS, CLOSURE_STATE).eq(lit(STATE_UNKNOWN)),
        lit(STATE_UNKNOWN),
    )
    .otherwise(lit(STATE_INVALID))?;
    let cause = datafusion::logical_expr::expr_fn::when(
        qualified(FAMILY_ALIAS, FAMILY).is_null(),
        lit("accepted_family_absent"),
    )
    .when(
        qualified(FAMILY_ALIAS, CLOSURE_STATE).eq(lit(STATE_UNSUPPORTED)),
        qualified(FAMILY_ALIAS, REMAINDER_REASON),
    )
    .when(
        qualified(FAMILY_ALIAS, CLOSURE_STATE).eq(lit(STATE_UNKNOWN)),
        lit("required_family_incomplete"),
    )
    .when(
        qualified(FAMILY_ALIAS, CLOSURE_STATE).eq(lit(STATE_INVALID)),
        lit("required_family_invalid"),
    )
    .otherwise(lit(ScalarValue::Utf8(None)))?;
    let closure = LogicalPlanBuilder::from(joined)
        .project([
            qualified(QUERY_REACH_ALIAS, QUERY_ROOT).alias(QUERY_ROOT),
            qualified(QUERY_REACH_ALIAS, QUERY_REQUIRED).alias(QUERY_REQUIRED),
            coalesce(vec![qualified(QUERY_REACH_ALIAS, QUERY_DEPTH), lit(0_u32)])
                .alias(QUERY_DEPTH),
            state.alias(QUERY_STATE),
            cause.alias(QUERY_UNKNOWN_CAUSE),
        ])?
        .build()?;

    let recursive = LogicalPlanBuilder::from(recursive)
        .alias(QUERY_FRONTIER_ALIAS)?
        .build()?;
    let depth_exhaustion = LogicalPlanBuilder::from(recursive)
        .filter(
            qualified(QUERY_FRONTIER_ALIAS, QUERY_DEPTH)
                .eq(lit(u32::from(bounds.max_query_depth()))),
        )?
        .join_on(
            edges,
            JoinType::Inner,
            [qualified(QUERY_FRONTIER_ALIAS, QUERY_REQUIRED)
                .eq(qualified(QUERY_EDGE_ALIAS, QUERY_ROOT))],
        )?
        .project([
            qualified(QUERY_FRONTIER_ALIAS, QUERY_ROOT).alias(QUERY_ROOT),
            qualified(QUERY_FRONTIER_ALIAS, QUERY_REQUIRED).alias(QUERY_REQUIRED),
        ])?
        .distinct()?
        .build()?;

    Ok(QueryClosureInternal {
        closure,
        depth_exhaustion,
    })
}

fn compile_query_closure_output(
    closure: LogicalPlan,
    bindings: &DerivedProducerClosureBindings,
    bounds: ProducerClosureResourceBounds,
) -> Result<LogicalPlan, DerivedProducerClosureError> {
    let fields = &bindings.query_requirement_closure.fields;
    Ok(LogicalPlanBuilder::from(closure)
        .project([
            col(QUERY_ROOT).alias(fields.query_family_id.as_str()),
            col(QUERY_REQUIRED).alias(fields.required_family_id.as_str()),
            col(QUERY_DEPTH).alias(fields.minimum_depth.as_str()),
            col(QUERY_STATE).alias(fields.requirement_state.as_str()),
            col(QUERY_UNKNOWN_CAUSE).alias(fields.unknown_cause.as_str()),
        ])?
        .sort([
            col(fields.query_family_id.as_str()).sort(true, false),
            col(fields.minimum_depth.as_str()).sort(true, false),
            col(fields.required_family_id.as_str()).sort(true, false),
        ])?
        .limit(0, Some(bounds.probe_rows()?))?
        .build()?)
}

#[allow(clippy::too_many_arguments)]
fn compile_violations(
    enriched: LogicalPlan,
    producers: LogicalPlan,
    remainders: LogicalPlan,
    accepted: LogicalPlan,
    query_closure: LogicalPlan,
    depth_exhaustion: LogicalPlan,
    bindings: &DerivedProducerClosureBindings,
    bounds: ProducerClosureResourceBounds,
) -> Result<LogicalPlan, DerivedProducerClosureError> {
    let semantic = &bindings.semantic_identities;
    let none = || lit(ScalarValue::Utf8(None));
    let mut branches = vec![
        violation_branch(
            enriched.clone(),
            col(ACCEPTED_COUNT).not_eq(lit(1_i64)),
            "accepted_fact_family",
            col(FAMILY),
            "duplicate_accepted_family",
            none(),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(SEMANTIC_CLASS_COUNT)
                .not_eq(lit(1_i64))
                .or(col(SEMANTIC_CLASS).not_eq(lit(semantic.factual_semantic_class_id.as_ref()))),
            "accepted_fact_family",
            col(FAMILY),
            "non_fact_semantic_class",
            col(SEMANTIC_CLASS),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(PRODUCER_COUNT)
                .eq(lit(0_i64))
                .and(col(REMAINDER_COUNT).eq(lit(0_i64))),
            "accepted_fact_family",
            col(FAMILY),
            "missing_producer_or_remainder",
            none(),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(PRODUCER_COUNT).gt(lit(1_i64)),
            "accepted_fact_family",
            col(FAMILY),
            "multiple_runtime_producers",
            col(PRODUCER),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(REMAINDER_COUNT).gt(lit(1_i64)),
            "accepted_fact_family",
            col(FAMILY),
            "multiple_unsupported_remainders",
            col(REMAINDER),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(PRODUCER_COUNT)
                .gt(lit(0_i64))
                .and(col(REMAINDER_COUNT).gt(lit(0_i64))),
            "accepted_fact_family",
            col(FAMILY),
            "producer_and_remainder",
            none(),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(PRODUCER_COUNT).eq(lit(1_i64)).and(
                col(PRODUCER_AUTHORITY)
                    .not_eq(lit(semantic.application_owned_authority_id.as_ref())),
            ),
            "runtime_producer",
            col(FAMILY),
            "wrong_runtime_producer_authority",
            col(PRODUCER_AUTHORITY),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(PRODUCER_COUNT).eq(lit(1_i64)).and(
                all_non_empty([
                    PRODUCER,
                    ALGORITHM_RELEASE,
                    PRECISION,
                    INPUT_PIN,
                    INVALIDATION_PIN,
                    MATERIALIZATION_PIN,
                    COMPLETENESS_PROOF_PIN,
                    PRODUCER_PROOF_PIN,
                ])
                .not(),
            ),
            "runtime_producer",
            col(FAMILY),
            "missing_runtime_producer_contract_pin",
            col(PRODUCER),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(PRODUCER_COUNT).eq(lit(1_i64)).and(
                col(REQUESTED_UNITS)
                    .not_eq(col(COMPLETED_UNITS))
                    .or(col(REMAINDER_UNITS).not_eq(lit(0_u64)))
                    .or(col(UNKNOWN_UNITS).not_eq(lit(0_u64))),
            ),
            "runtime_producer",
            col(FAMILY),
            "incomplete_runtime_producer",
            col(PRODUCER),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(REMAINDER_COUNT).eq(lit(1_i64)).and(
                col(REMAINDER_AUTHORITY)
                    .not_eq(lit(semantic.application_owned_authority_id.as_ref())),
            ),
            "unsupported_remainder",
            col(FAMILY),
            "wrong_unsupported_remainder_authority",
            col(REMAINDER_AUTHORITY),
            bindings,
        )?,
        violation_branch(
            enriched.clone(),
            col(REMAINDER_COUNT)
                .eq(lit(1_i64))
                .and(all_non_empty([REMAINDER, REMAINDER_REASON, REMAINDER_PROOF_PIN]).not()),
            "unsupported_remainder",
            col(FAMILY),
            "incomplete_unsupported_remainder",
            col(REMAINDER),
            bindings,
        )?,
    ];

    let accepted_alias = LogicalPlanBuilder::from(accepted)
        .alias(ACCEPTED_ALIAS)?
        .build()?;
    let orphan_producers = LogicalPlanBuilder::from(producers)
        .alias(PRODUCER_ALIAS)?
        .join_on(
            accepted_alias.clone(),
            JoinType::LeftAnti,
            [qualified(PRODUCER_ALIAS, FAMILY).eq(qualified(ACCEPTED_ALIAS, FAMILY))],
        )?
        .project([
            qualified(PRODUCER_ALIAS, FAMILY).alias(FAMILY),
            qualified(PRODUCER_ALIAS, PRODUCER).alias(PRODUCER),
        ])?
        .build()?;
    branches.push(violation_branch(
        orphan_producers,
        lit(true),
        "runtime_producer",
        col(FAMILY),
        "orphan_runtime_producer",
        col(PRODUCER),
        bindings,
    )?);
    let orphan_remainders = LogicalPlanBuilder::from(remainders)
        .alias(REMAINDER_ALIAS)?
        .join_on(
            accepted_alias,
            JoinType::LeftAnti,
            [qualified(REMAINDER_ALIAS, FAMILY).eq(qualified(ACCEPTED_ALIAS, FAMILY))],
        )?
        .project([
            qualified(REMAINDER_ALIAS, FAMILY).alias(FAMILY),
            qualified(REMAINDER_ALIAS, REMAINDER).alias(REMAINDER),
        ])?
        .build()?;
    branches.push(violation_branch(
        orphan_remainders,
        lit(true),
        "unsupported_remainder",
        col(FAMILY),
        "orphan_unsupported_remainder",
        col(REMAINDER),
        bindings,
    )?);

    for (state, code) in [
        (STATE_MISSING, "query_requirement_missing"),
        (STATE_UNKNOWN, "query_requirement_incomplete"),
        (STATE_INVALID, "query_requirement_invalid"),
    ] {
        branches.push(violation_branch(
            query_closure.clone(),
            col(QUERY_STATE).eq(lit(state)),
            "query_family_requirement",
            col(QUERY_ROOT),
            code,
            col(QUERY_REQUIRED),
            bindings,
        )?);
    }
    branches.push(violation_branch(
        depth_exhaustion,
        lit(true),
        "query_family_requirement",
        col(QUERY_ROOT),
        "query_requirement_depth_exhausted",
        col(QUERY_REQUIRED),
        bindings,
    )?);

    let mut iterator = branches.into_iter();
    let mut union = iterator
        .next()
        .ok_or(DerivedProducerClosureError::InternalNoViolationBranches)?;
    for branch in iterator {
        union = LogicalPlanBuilder::from(union).union(branch)?.build()?;
    }
    let fields = &bindings.violation.fields;
    Ok(LogicalPlanBuilder::from(union)
        .distinct()?
        .sort([
            col(fields.subject_kind.as_str()).sort(true, false),
            col(fields.subject_id.as_str()).sort(true, false),
            col(fields.violation_code.as_str()).sort(true, false),
            col(fields.related_id.as_str()).sort(true, true),
        ])?
        .limit(0, Some(bounds.probe_rows()?))?
        .build()?)
}

fn violation_branch(
    source: LogicalPlan,
    condition: Expr,
    subject_kind: &'static str,
    subject_id: Expr,
    violation_code: &'static str,
    related_id: Expr,
    bindings: &DerivedProducerClosureBindings,
) -> Result<LogicalPlan, DerivedProducerClosureError> {
    let fields = &bindings.violation.fields;
    Ok(LogicalPlanBuilder::from(source)
        .filter(condition)?
        .project([
            lit(subject_kind).alias(fields.subject_kind.as_str()),
            subject_id.alias(fields.subject_id.as_str()),
            lit(violation_code).alias(fields.violation_code.as_str()),
            related_id.alias(fields.related_id.as_str()),
        ])?
        .build()?)
}

fn all_non_empty<const N: usize>(fields: [&'static str; N]) -> Expr {
    fields
        .into_iter()
        .map(|field| col(field).not_eq(lit("")))
        .reduce(Expr::and)
        .unwrap_or_else(|| lit(true))
}

fn qualified(alias: &'static str, field: &str) -> Expr {
    Expr::Column(Column::new(
        Some(TableReference::bare(alias)),
        field.to_owned(),
    ))
}

fn validate_relation_contracts(
    accepted: &ProducerClosureRelationContract<AcceptedFactFamilyFields>,
    producer: &ProducerClosureRelationContract<RuntimeProducerFields>,
    query: &ProducerClosureRelationContract<QueryFamilyRequirementFields>,
    remainder: &ProducerClosureRelationContract<UnsupportedRemainderFields>,
    family_output: &ProducerClosureRelationContract<FamilyClosureFields>,
    query_output: &ProducerClosureRelationContract<QueryRequirementClosureFields>,
    violation: &ProducerClosureRelationContract<ProducerClosureViolationFields>,
) -> Result<(), DerivedProducerClosureError> {
    let relation_ids = [
        accepted.relation_id(),
        producer.relation_id(),
        query.relation_id(),
        remainder.relation_id(),
        family_output.relation_id(),
        query_output.relation_id(),
        violation.relation_id(),
    ];
    let mut unique_relations = BTreeSet::new();
    for relation_id in relation_ids {
        if !unique_relations.insert(relation_id.as_str()) {
            return Err(DerivedProducerClosureError::DuplicateRelationId(
                relation_id.as_str().to_owned(),
            ));
        }
    }

    validate_exact_fields(
        "accepted_fact_family",
        &accepted.schema,
        &[
            (&accepted.fields.family_id, DataType::Utf8, false),
            (&accepted.fields.semantic_class_id, DataType::Utf8, false),
        ],
    )?;
    validate_exact_fields(
        "runtime_producer",
        &producer.schema,
        &[
            (&producer.fields.family_id, DataType::Utf8, false),
            (&producer.fields.producer_id, DataType::Utf8, false),
            (&producer.fields.authority_id, DataType::Utf8, false),
            (&producer.fields.algorithm_release, DataType::Utf8, false),
            (&producer.fields.precision_id, DataType::Utf8, false),
            (&producer.fields.input_pin, DataType::Utf8, false),
            (&producer.fields.invalidation_pin, DataType::Utf8, false),
            (&producer.fields.materialization_pin, DataType::Utf8, false),
            (
                &producer.fields.requested_unit_count,
                DataType::UInt64,
                false,
            ),
            (
                &producer.fields.completed_unit_count,
                DataType::UInt64,
                false,
            ),
            (
                &producer.fields.remainder_unit_count,
                DataType::UInt64,
                false,
            ),
            (&producer.fields.unknown_unit_count, DataType::UInt64, false),
            (
                &producer.fields.completeness_proof_pin,
                DataType::Utf8,
                false,
            ),
            (&producer.fields.proof_pin, DataType::Utf8, false),
        ],
    )?;
    validate_exact_fields(
        "query_family_requirement",
        &query.schema,
        &[
            (&query.fields.query_family_id, DataType::Utf8, false),
            (&query.fields.required_family_id, DataType::Utf8, false),
        ],
    )?;
    validate_exact_fields(
        "unsupported_remainder",
        &remainder.schema,
        &[
            (&remainder.fields.family_id, DataType::Utf8, false),
            (&remainder.fields.remainder_id, DataType::Utf8, false),
            (&remainder.fields.authority_id, DataType::Utf8, false),
            (&remainder.fields.reason_id, DataType::Utf8, false),
            (&remainder.fields.proof_pin, DataType::Utf8, false),
        ],
    )?;
    validate_exact_fields(
        "family_closure",
        &family_output.schema,
        &[
            (&family_output.fields.family_id, DataType::Utf8, false),
            (
                &family_output.fields.semantic_class_id,
                DataType::Utf8,
                false,
            ),
            (&family_output.fields.closure_state, DataType::Utf8, false),
            (&family_output.fields.producer_id, DataType::Utf8, true),
            (&family_output.fields.authority_id, DataType::Utf8, true),
            (
                &family_output.fields.algorithm_release,
                DataType::Utf8,
                true,
            ),
            (&family_output.fields.precision_id, DataType::Utf8, true),
            (&family_output.fields.input_pin, DataType::Utf8, true),
            (&family_output.fields.invalidation_pin, DataType::Utf8, true),
            (
                &family_output.fields.materialization_pin,
                DataType::Utf8,
                true,
            ),
            (
                &family_output.fields.requested_unit_count,
                DataType::UInt64,
                true,
            ),
            (
                &family_output.fields.completed_unit_count,
                DataType::UInt64,
                true,
            ),
            (
                &family_output.fields.remainder_unit_count,
                DataType::UInt64,
                true,
            ),
            (
                &family_output.fields.unknown_unit_count,
                DataType::UInt64,
                true,
            ),
            (
                &family_output.fields.completeness_proof_pin,
                DataType::Utf8,
                true,
            ),
            (
                &family_output.fields.producer_proof_pin,
                DataType::Utf8,
                true,
            ),
            (
                &family_output.fields.unsupported_remainder_id,
                DataType::Utf8,
                true,
            ),
            (
                &family_output.fields.unsupported_reason_id,
                DataType::Utf8,
                true,
            ),
            (
                &family_output.fields.unsupported_proof_pin,
                DataType::Utf8,
                true,
            ),
        ],
    )?;
    validate_exact_fields(
        "query_requirement_closure",
        &query_output.schema,
        &[
            (&query_output.fields.query_family_id, DataType::Utf8, false),
            (
                &query_output.fields.required_family_id,
                DataType::Utf8,
                false,
            ),
            (&query_output.fields.minimum_depth, DataType::UInt32, false),
            (
                &query_output.fields.requirement_state,
                DataType::Utf8,
                false,
            ),
            (&query_output.fields.unknown_cause, DataType::Utf8, true),
        ],
    )?;
    validate_exact_fields(
        "producer_closure_violation",
        &violation.schema,
        &[
            (&violation.fields.subject_kind, DataType::Utf8, false),
            (&violation.fields.subject_id, DataType::Utf8, false),
            (&violation.fields.violation_code, DataType::Utf8, false),
            (&violation.fields.related_id, DataType::Utf8, true),
        ],
    )?;
    Ok(())
}

fn validate_exact_fields(
    role: &'static str,
    schema: &SchemaRef,
    expected: &[(&FieldId, DataType, bool)],
) -> Result<(), DerivedProducerClosureError> {
    if schema.fields().len() != expected.len() {
        return Err(DerivedProducerClosureError::SchemaFieldCount {
            relation: role,
            expected: expected.len(),
            actual: schema.fields().len(),
        });
    }
    let mut identities = BTreeSet::new();
    for (ordinal, ((field_id, data_type, nullable), actual)) in
        expected.iter().zip(schema.fields()).enumerate()
    {
        if !identities.insert(field_id.as_str()) {
            return Err(DerivedProducerClosureError::DuplicateFieldId {
                relation: role,
                field: field_id.as_str().to_owned(),
            });
        }
        if actual.name() != field_id.as_str()
            || actual.data_type() != data_type
            || actual.is_nullable() != *nullable
        {
            return Err(DerivedProducerClosureError::SchemaFieldMismatch {
                relation: role,
                ordinal,
                expected_name: field_id.as_str().to_owned(),
                expected_type: data_type.clone(),
                expected_nullable: *nullable,
                actual_name: actual.name().clone(),
                actual_type: actual.data_type().clone(),
                actual_nullable: actual.is_nullable(),
            });
        }
    }
    Ok(())
}

fn validate_input<F>(
    input: &ProducerClosureRelationInput,
    contract: &ProducerClosureRelationContract<F>,
    role: &'static str,
) -> Result<(), DerivedProducerClosureError> {
    if input.relation_id != contract.relation_id {
        return Err(DerivedProducerClosureError::InputRelationMismatch {
            role,
            expected: contract.relation_id.as_str().to_owned(),
            actual: input.relation_id.as_str().to_owned(),
        });
    }
    let actual = input.plan.schema().as_arrow();
    if actual != contract.schema.as_ref() {
        return Err(DerivedProducerClosureError::InputSchemaMismatch {
            role,
            expected: Arc::clone(&contract.schema),
            actual: Arc::new(actual.clone()),
        });
    }
    Ok(())
}

fn validate_compiled_schema(
    role: &'static str,
    plan: &LogicalPlan,
    expected: &SchemaRef,
) -> Result<(), DerivedProducerClosureError> {
    if plan.schema().as_arrow() != expected.as_ref() {
        return Err(DerivedProducerClosureError::CompiledSchemaMismatch {
            role,
            expected: Arc::clone(expected),
            actual: Arc::new(plan.schema().as_arrow().clone()),
        });
    }
    Ok(())
}

fn validate_text(kind: &'static str, value: &str) -> Result<(), DerivedProducerClosureError> {
    if value.is_empty() || value.len() > 240 {
        return Err(DerivedProducerClosureError::InvalidText {
            kind,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn observe_dependencies(
    bindings: &DerivedProducerClosureBindings,
) -> BTreeSet<ProducerClosureCompilationDependency> {
    let mut dependencies = BTreeSet::from([
        ProducerClosureCompilationDependency::ApplicationOwnedAuthority(Arc::clone(
            &bindings.semantic_identities.application_owned_authority_id,
        )),
        ProducerClosureCompilationDependency::FactualSemanticClass(Arc::clone(
            &bindings.semantic_identities.factual_semantic_class_id,
        )),
        ProducerClosureCompilationDependency::ImplementationRelease(Arc::clone(
            &bindings.implementation_release,
        )),
        ProducerClosureCompilationDependency::SessionMemoryPool,
        ProducerClosureCompilationDependency::DataFusionExecuteStreamDropAbort,
    ]);

    macro_rules! relation_dependencies {
        ($contract:expr, [$($field:expr),+ $(,)?]) => {{
            dependencies.insert(ProducerClosureCompilationDependency::InputRelation(
                $contract.relation_id.clone(),
            ));
            $(dependencies.insert(ProducerClosureCompilationDependency::InputField(
                $field.clone(),
            ));)+
        }};
    }
    relation_dependencies!(
        bindings.accepted_fact_family,
        [
            bindings.accepted_fact_family.fields.family_id,
            bindings.accepted_fact_family.fields.semantic_class_id,
        ]
    );
    relation_dependencies!(
        bindings.runtime_producer,
        [
            bindings.runtime_producer.fields.family_id,
            bindings.runtime_producer.fields.producer_id,
            bindings.runtime_producer.fields.authority_id,
            bindings.runtime_producer.fields.algorithm_release,
            bindings.runtime_producer.fields.precision_id,
            bindings.runtime_producer.fields.input_pin,
            bindings.runtime_producer.fields.invalidation_pin,
            bindings.runtime_producer.fields.materialization_pin,
            bindings.runtime_producer.fields.requested_unit_count,
            bindings.runtime_producer.fields.completed_unit_count,
            bindings.runtime_producer.fields.remainder_unit_count,
            bindings.runtime_producer.fields.unknown_unit_count,
            bindings.runtime_producer.fields.completeness_proof_pin,
            bindings.runtime_producer.fields.proof_pin,
        ]
    );
    relation_dependencies!(
        bindings.query_family_requirement,
        [
            bindings.query_family_requirement.fields.query_family_id,
            bindings.query_family_requirement.fields.required_family_id,
        ]
    );
    relation_dependencies!(
        bindings.unsupported_remainder,
        [
            bindings.unsupported_remainder.fields.family_id,
            bindings.unsupported_remainder.fields.remainder_id,
            bindings.unsupported_remainder.fields.authority_id,
            bindings.unsupported_remainder.fields.reason_id,
            bindings.unsupported_remainder.fields.proof_pin,
        ]
    );

    macro_rules! output_dependencies {
        ($contract:expr, [$($field:expr),+ $(,)?]) => {{
            dependencies.insert(ProducerClosureCompilationDependency::OutputRelation(
                $contract.relation_id.clone(),
            ));
            $(dependencies.insert(ProducerClosureCompilationDependency::OutputField(
                $field.clone(),
            ));)+
        }};
    }
    output_dependencies!(
        bindings.family_closure,
        [
            bindings.family_closure.fields.family_id,
            bindings.family_closure.fields.semantic_class_id,
            bindings.family_closure.fields.closure_state,
            bindings.family_closure.fields.producer_id,
            bindings.family_closure.fields.authority_id,
            bindings.family_closure.fields.algorithm_release,
            bindings.family_closure.fields.precision_id,
            bindings.family_closure.fields.input_pin,
            bindings.family_closure.fields.invalidation_pin,
            bindings.family_closure.fields.materialization_pin,
            bindings.family_closure.fields.requested_unit_count,
            bindings.family_closure.fields.completed_unit_count,
            bindings.family_closure.fields.remainder_unit_count,
            bindings.family_closure.fields.unknown_unit_count,
            bindings.family_closure.fields.completeness_proof_pin,
            bindings.family_closure.fields.producer_proof_pin,
            bindings.family_closure.fields.unsupported_remainder_id,
            bindings.family_closure.fields.unsupported_reason_id,
            bindings.family_closure.fields.unsupported_proof_pin,
        ]
    );
    output_dependencies!(
        bindings.query_requirement_closure,
        [
            bindings.query_requirement_closure.fields.query_family_id,
            bindings.query_requirement_closure.fields.required_family_id,
            bindings.query_requirement_closure.fields.minimum_depth,
            bindings.query_requirement_closure.fields.requirement_state,
            bindings.query_requirement_closure.fields.unknown_cause,
        ]
    );
    output_dependencies!(
        bindings.violation,
        [
            bindings.violation.fields.subject_kind,
            bindings.violation.fields.subject_id,
            bindings.violation.fields.violation_code,
            bindings.violation.fields.related_id,
        ]
    );
    dependencies
}

#[derive(Default)]
struct ExecutionBudget {
    batches: usize,
    bytes: usize,
}

async fn execute_bounded(
    context: &SessionContext,
    plan: &LogicalPlan,
    expected_schema: &SchemaRef,
    bounds: ProducerClosureResourceBounds,
    relation: &'static str,
    budget: &mut ExecutionBudget,
) -> Result<Vec<RecordBatch>, DerivedProducerClosureError> {
    let optimized = context.state().optimize(plan)?;
    let physical = context.state().create_physical_plan(&optimized).await?;
    let mut stream = execute_stream(physical, context.task_ctx())?;
    let mut batches = Vec::new();
    let mut relation_rows = 0_usize;
    while let Some(batch) = stream.next().await {
        let batch = batch?;
        if batch.schema_ref().as_ref() != expected_schema.as_ref() {
            return Err(DerivedProducerClosureError::ExecutedSchemaMismatch {
                relation,
                expected: Arc::clone(expected_schema),
                actual: batch.schema(),
            });
        }
        relation_rows = relation_rows
            .checked_add(batch.num_rows())
            .ok_or(DerivedProducerClosureError::ResourceCounterOverflow("rows"))?;
        budget.batches = budget.batches.checked_add(1).ok_or(
            DerivedProducerClosureError::ResourceCounterOverflow("batches"),
        )?;
        budget.bytes = budget
            .bytes
            .checked_add(batch.get_array_memory_size())
            .ok_or(DerivedProducerClosureError::ResourceCounterOverflow(
                "bytes",
            ))?;
        if relation_rows > bounds.max_rows_per_relation() {
            return Err(DerivedProducerClosureError::OutputRowsExceeded {
                relation,
                limit: bounds.max_rows_per_relation(),
                observed: relation_rows,
            });
        }
        if budget.batches > bounds.max_total_batches() {
            return Err(DerivedProducerClosureError::OutputBatchesExceeded {
                limit: bounds.max_total_batches(),
                observed: budget.batches,
            });
        }
        if budget.bytes > bounds.max_total_bytes() {
            return Err(DerivedProducerClosureError::OutputBytesExceeded {
                limit: bounds.max_total_bytes(),
                observed: budget.bytes,
            });
        }
        batches.push(batch);
    }
    drop(stream);
    if batches.is_empty() {
        batches.push(RecordBatch::new_empty(Arc::clone(expected_schema)));
    }
    Ok(batches)
}

/// Fail-closed binding, planning, execution, and resource errors.
#[derive(Debug, Error)]
pub enum DerivedProducerClosureError {
    #[error("invalid {kind} identity {value:?}")]
    InvalidText { kind: &'static str, value: String },
    #[error("runtime relation identity {0:?} is bound more than once")]
    DuplicateRelationId(String),
    #[error("{relation} binds field identity {field:?} more than once")]
    DuplicateFieldId {
        relation: &'static str,
        field: String,
    },
    #[error("{relation} schema has {actual} fields; expected exactly {expected}")]
    SchemaFieldCount {
        relation: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error(
        "{relation} field {ordinal} mismatch: expected {expected_name:?} {expected_type:?} nullable={expected_nullable}, observed {actual_name:?} {actual_type:?} nullable={actual_nullable}"
    )]
    SchemaFieldMismatch {
        relation: &'static str,
        ordinal: usize,
        expected_name: String,
        expected_type: DataType,
        expected_nullable: bool,
        actual_name: String,
        actual_type: DataType,
        actual_nullable: bool,
    },
    #[error("{role} relation mismatch: expected {expected:?}, observed {actual:?}")]
    InputRelationMismatch {
        role: &'static str,
        expected: String,
        actual: String,
    },
    #[error("{role} input schema differs from the installed application binding")]
    InputSchemaMismatch {
        role: &'static str,
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("compiled {role} schema differs from the installed application binding")]
    CompiledSchemaMismatch {
        role: &'static str,
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("executed {relation} schema differs from the compiled contract")]
    ExecutedSchemaMismatch {
        relation: &'static str,
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("resource bound {0} must be non-zero")]
    ZeroResourceBound(&'static str),
    #[error("output-row bound cannot reserve an overflow-probe row")]
    ResourceProbeOverflow,
    #[error("{relation} output rows exceeded {limit}: observed at least {observed}")]
    OutputRowsExceeded {
        relation: &'static str,
        limit: usize,
        observed: usize,
    },
    #[error("total output batches exceeded {limit}: observed at least {observed}")]
    OutputBatchesExceeded { limit: usize, observed: usize },
    #[error("total output bytes exceeded {limit}: observed at least {observed}")]
    OutputBytesExceeded { limit: usize, observed: usize },
    #[error("resource counter overflowed for {0}")]
    ResourceCounterOverflow(&'static str),
    #[error("internal producer-closure compiler constructed no violation branches")]
    InternalNoViolationBranches,
    #[error(transparent)]
    DataFusion(#[from] datafusion::error::DataFusionError),
}

#[cfg(test)]
mod tests {
    use arrow_array::{Array, StringArray, UInt32Array, UInt64Array};
    use arrow_schema::{Field, Schema};
    use datafusion::datasource::MemTable;

    use super::*;

    const APP_AUTHORITY: &str = "authority.application-derived.v2";
    const PROVIDER_AUTHORITY: &str = "authority.provider-native.v2";
    const FACT_CLASS: &str = "semantic.fact.v2";
    const JUDGMENT_CLASS: &str = "semantic.judgment.v2";

    fn relation_id(value: &str) -> RelationId {
        RelationId::new(value).expect("relation ID")
    }

    fn field_id(value: &str) -> FieldId {
        FieldId::new(value).expect("field ID")
    }

    fn utf8_field(name: &str, nullable: bool) -> Field {
        Field::new(name, DataType::Utf8, nullable)
    }

    fn schema(fields: Vec<Field>) -> SchemaRef {
        Arc::new(Schema::new(fields))
    }

    fn bindings() -> DerivedProducerClosureBindings {
        let accepted_fields = AcceptedFactFamilyFields {
            family_id: field_id("accepted_family_id"),
            semantic_class_id: field_id("accepted_semantic_class_id"),
        };
        let producer_fields = RuntimeProducerFields {
            family_id: field_id("producer_family_id"),
            producer_id: field_id("runtime_producer_id"),
            authority_id: field_id("runtime_authority_id"),
            algorithm_release: field_id("algorithm_release_pin"),
            precision_id: field_id("precision_profile_id"),
            input_pin: field_id("producer_input_pin"),
            invalidation_pin: field_id("invalidation_policy_pin"),
            materialization_pin: field_id("materialization_policy_pin"),
            requested_unit_count: field_id("producer_requested_unit_count"),
            completed_unit_count: field_id("producer_completed_unit_count"),
            remainder_unit_count: field_id("producer_remainder_unit_count"),
            unknown_unit_count: field_id("producer_unknown_unit_count"),
            completeness_proof_pin: field_id("producer_completeness_proof_pin"),
            proof_pin: field_id("producer_execution_proof_pin"),
        };
        let query_fields = QueryFamilyRequirementFields {
            query_family_id: field_id("query_family_id"),
            required_family_id: field_id("query_required_family_id"),
        };
        let remainder_fields = UnsupportedRemainderFields {
            family_id: field_id("remainder_family_id"),
            remainder_id: field_id("unsupported_remainder_id"),
            authority_id: field_id("remainder_authority_id"),
            reason_id: field_id("unsupported_reason_id"),
            proof_pin: field_id("remainder_proof_pin"),
        };
        let family_output_fields = FamilyClosureFields {
            family_id: field_id("closed_family_id"),
            semantic_class_id: field_id("closed_semantic_class_id"),
            closure_state: field_id("family_closure_state"),
            producer_id: field_id("closed_producer_id"),
            authority_id: field_id("closed_authority_id"),
            algorithm_release: field_id("closed_algorithm_release"),
            precision_id: field_id("closed_precision_id"),
            input_pin: field_id("closed_input_pin"),
            invalidation_pin: field_id("closed_invalidation_pin"),
            materialization_pin: field_id("closed_materialization_pin"),
            requested_unit_count: field_id("closed_requested_unit_count"),
            completed_unit_count: field_id("closed_completed_unit_count"),
            remainder_unit_count: field_id("closed_remainder_unit_count"),
            unknown_unit_count: field_id("closed_unknown_unit_count"),
            completeness_proof_pin: field_id("closed_completeness_proof_pin"),
            producer_proof_pin: field_id("closed_producer_proof_pin"),
            unsupported_remainder_id: field_id("closed_unsupported_remainder_id"),
            unsupported_reason_id: field_id("closed_unsupported_reason_id"),
            unsupported_proof_pin: field_id("closed_unsupported_proof_pin"),
        };
        let query_output_fields = QueryRequirementClosureFields {
            query_family_id: field_id("closed_query_family_id"),
            required_family_id: field_id("closed_query_required_family_id"),
            minimum_depth: field_id("query_requirement_minimum_depth"),
            requirement_state: field_id("query_requirement_state"),
            unknown_cause: field_id("query_requirement_unknown_cause"),
        };
        let violation_fields = ProducerClosureViolationFields {
            subject_kind: field_id("violation_subject_kind"),
            subject_id: field_id("violation_subject_id"),
            violation_code: field_id("producer_closure_violation_code"),
            related_id: field_id("violation_related_id"),
        };

        let accepted = ProducerClosureRelationContract::new(
            relation_id("runtime.accepted_fact_family"),
            schema(vec![
                utf8_field(accepted_fields.family_id.as_str(), false),
                utf8_field(accepted_fields.semantic_class_id.as_str(), false),
            ]),
            accepted_fields,
        );
        let producer = ProducerClosureRelationContract::new(
            relation_id("runtime.derived_producer"),
            schema(vec![
                utf8_field(producer_fields.family_id.as_str(), false),
                utf8_field(producer_fields.producer_id.as_str(), false),
                utf8_field(producer_fields.authority_id.as_str(), false),
                utf8_field(producer_fields.algorithm_release.as_str(), false),
                utf8_field(producer_fields.precision_id.as_str(), false),
                utf8_field(producer_fields.input_pin.as_str(), false),
                utf8_field(producer_fields.invalidation_pin.as_str(), false),
                utf8_field(producer_fields.materialization_pin.as_str(), false),
                Field::new(
                    producer_fields.requested_unit_count.as_str(),
                    DataType::UInt64,
                    false,
                ),
                Field::new(
                    producer_fields.completed_unit_count.as_str(),
                    DataType::UInt64,
                    false,
                ),
                Field::new(
                    producer_fields.remainder_unit_count.as_str(),
                    DataType::UInt64,
                    false,
                ),
                Field::new(
                    producer_fields.unknown_unit_count.as_str(),
                    DataType::UInt64,
                    false,
                ),
                utf8_field(producer_fields.completeness_proof_pin.as_str(), false),
                utf8_field(producer_fields.proof_pin.as_str(), false),
            ]),
            producer_fields,
        );
        let query = ProducerClosureRelationContract::new(
            relation_id("runtime.query_family_requirement"),
            schema(vec![
                utf8_field(query_fields.query_family_id.as_str(), false),
                utf8_field(query_fields.required_family_id.as_str(), false),
            ]),
            query_fields,
        );
        let remainder = ProducerClosureRelationContract::new(
            relation_id("runtime.unsupported_remainder"),
            schema(vec![
                utf8_field(remainder_fields.family_id.as_str(), false),
                utf8_field(remainder_fields.remainder_id.as_str(), false),
                utf8_field(remainder_fields.authority_id.as_str(), false),
                utf8_field(remainder_fields.reason_id.as_str(), false),
                utf8_field(remainder_fields.proof_pin.as_str(), false),
            ]),
            remainder_fields,
        );
        let family_output = ProducerClosureRelationContract::new(
            relation_id("derived.accepted_family_producer_closure"),
            schema(vec![
                utf8_field(family_output_fields.family_id.as_str(), false),
                utf8_field(family_output_fields.semantic_class_id.as_str(), false),
                utf8_field(family_output_fields.closure_state.as_str(), false),
                utf8_field(family_output_fields.producer_id.as_str(), true),
                utf8_field(family_output_fields.authority_id.as_str(), true),
                utf8_field(family_output_fields.algorithm_release.as_str(), true),
                utf8_field(family_output_fields.precision_id.as_str(), true),
                utf8_field(family_output_fields.input_pin.as_str(), true),
                utf8_field(family_output_fields.invalidation_pin.as_str(), true),
                utf8_field(family_output_fields.materialization_pin.as_str(), true),
                Field::new(
                    family_output_fields.requested_unit_count.as_str(),
                    DataType::UInt64,
                    true,
                ),
                Field::new(
                    family_output_fields.completed_unit_count.as_str(),
                    DataType::UInt64,
                    true,
                ),
                Field::new(
                    family_output_fields.remainder_unit_count.as_str(),
                    DataType::UInt64,
                    true,
                ),
                Field::new(
                    family_output_fields.unknown_unit_count.as_str(),
                    DataType::UInt64,
                    true,
                ),
                utf8_field(family_output_fields.completeness_proof_pin.as_str(), true),
                utf8_field(family_output_fields.producer_proof_pin.as_str(), true),
                utf8_field(family_output_fields.unsupported_remainder_id.as_str(), true),
                utf8_field(family_output_fields.unsupported_reason_id.as_str(), true),
                utf8_field(family_output_fields.unsupported_proof_pin.as_str(), true),
            ]),
            family_output_fields,
        );
        let query_output = ProducerClosureRelationContract::new(
            relation_id("derived.query_family_requirement_closure"),
            schema(vec![
                utf8_field(query_output_fields.query_family_id.as_str(), false),
                utf8_field(query_output_fields.required_family_id.as_str(), false),
                Field::new(
                    query_output_fields.minimum_depth.as_str(),
                    DataType::UInt32,
                    false,
                ),
                utf8_field(query_output_fields.requirement_state.as_str(), false),
                utf8_field(query_output_fields.unknown_cause.as_str(), true),
            ]),
            query_output_fields,
        );
        let violation = ProducerClosureRelationContract::new(
            relation_id("proof.derived_producer_closure_violation"),
            schema(vec![
                utf8_field(violation_fields.subject_kind.as_str(), false),
                utf8_field(violation_fields.subject_id.as_str(), false),
                utf8_field(violation_fields.violation_code.as_str(), false),
                utf8_field(violation_fields.related_id.as_str(), true),
            ]),
            violation_fields,
        );

        DerivedProducerClosureBindings::try_new(
            "operation.derived-producer-closure.v2",
            "derived-producer-closure@1.0.0",
            ProducerClosureSemanticIdentities::try_new(APP_AUTHORITY, FACT_CLASS)
                .expect("semantic identities"),
            accepted,
            producer,
            query,
            remainder,
            family_output,
            query_output,
            violation,
        )
        .expect("closure bindings")
    }

    fn relation_batch(schema: SchemaRef, rows: &[Vec<&str>]) -> RecordBatch {
        let columns = (0..schema.fields().len())
            .map(|column| match schema.field(column).data_type() {
                DataType::Utf8 => Arc::new(StringArray::from(
                    rows.iter().map(|row| row[column]).collect::<Vec<_>>(),
                )) as arrow_array::ArrayRef,
                DataType::UInt64 => Arc::new(UInt64Array::from(
                    rows.iter()
                        .map(|row| row[column].parse::<u64>().expect("u64 fixture"))
                        .collect::<Vec<_>>(),
                )),
                other => panic!("unsupported fixture type {other:?}"),
            })
            .collect::<Vec<_>>();
        RecordBatch::try_new(schema, columns).expect("string relation")
    }

    fn relation_input<F>(
        contract: &ProducerClosureRelationContract<F>,
        batch: RecordBatch,
    ) -> ProducerClosureRelationInput {
        let provider = Arc::new(
            MemTable::try_new(Arc::clone(&contract.schema), vec![vec![batch]]).expect("MemTable"),
        );
        let plan = LogicalPlanBuilder::scan(
            contract.relation_id.as_str(),
            provider_as_source(provider),
            None,
        )
        .expect("scan")
        .build()
        .expect("input plan");
        ProducerClosureRelationInput::new(contract.relation_id.clone(), plan)
    }

    type ProducerRow<'a> = [&'a str; 14];
    type RemainderRow<'a> = [&'a str; 5];

    fn inputs(
        bindings: &DerivedProducerClosureBindings,
        accepted: &[(&str, &str)],
        producers: &[ProducerRow<'_>],
        queries: &[(&str, &str)],
        remainders: &[RemainderRow<'_>],
    ) -> DerivedProducerClosureInputs {
        let accepted_rows = accepted
            .iter()
            .map(|row| vec![row.0, row.1])
            .collect::<Vec<_>>();
        let producer_rows = producers.iter().map(|row| row.to_vec()).collect::<Vec<_>>();
        let query_rows = queries
            .iter()
            .map(|row| vec![row.0, row.1])
            .collect::<Vec<_>>();
        let remainder_rows = remainders
            .iter()
            .map(|row| row.to_vec())
            .collect::<Vec<_>>();
        DerivedProducerClosureInputs {
            accepted_fact_family: relation_input(
                &bindings.accepted_fact_family,
                relation_batch(
                    Arc::clone(&bindings.accepted_fact_family.schema),
                    &accepted_rows,
                ),
            ),
            runtime_producer: relation_input(
                &bindings.runtime_producer,
                relation_batch(
                    Arc::clone(&bindings.runtime_producer.schema),
                    &producer_rows,
                ),
            ),
            query_family_requirement: relation_input(
                &bindings.query_family_requirement,
                relation_batch(
                    Arc::clone(&bindings.query_family_requirement.schema),
                    &query_rows,
                ),
            ),
            unsupported_remainder: relation_input(
                &bindings.unsupported_remainder,
                relation_batch(
                    Arc::clone(&bindings.unsupported_remainder.schema),
                    &remainder_rows,
                ),
            ),
        }
    }

    fn producer<'a>(family: &'a str, producer: &'a str) -> ProducerRow<'a> {
        [
            family,
            producer,
            APP_AUTHORITY,
            "algorithm@1",
            "precision.sound-bounded",
            "input:b3:11",
            "invalidation:b3:22",
            "materialization:b3:33",
            "1",
            "1",
            "0",
            "0",
            "completeness-proof:b3:40",
            "proof:b3:44",
        ]
    }

    fn remainder(family: &str) -> RemainderRow<'_> {
        [
            family,
            "remainder.dynamic-dispatch",
            APP_AUTHORITY,
            "unknown.dynamic-dispatch",
            "proof:b3:55",
        ]
    }

    fn bounds() -> ProducerClosureResourceBounds {
        ProducerClosureResourceBounds::try_new(16, 4_096, 256, 16 * 1024 * 1024).expect("bounds")
    }

    async fn execute(
        bindings: &DerivedProducerClosureBindings,
        inputs: DerivedProducerClosureInputs,
    ) -> DerivedProducerClosureExecution {
        let compiled =
            compile_derived_producer_closure(inputs, bindings, bounds()).expect("compile closure");
        compiled
            .execute(&SessionContext::new())
            .await
            .expect("execute closure")
    }

    fn string_values(batches: &[RecordBatch], field: &str) -> Vec<Option<String>> {
        let mut values = Vec::new();
        for batch in batches {
            let column = batch
                .column_by_name(field)
                .expect("named string output")
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("string output");
            values.extend(
                (0..column.len())
                    .map(|index| (!column.is_null(index)).then(|| column.value(index).to_owned())),
            );
        }
        values
    }

    fn u32_values(batches: &[RecordBatch], field: &str) -> Vec<u32> {
        let mut values = Vec::new();
        for batch in batches {
            let column = batch
                .column_by_name(field)
                .expect("named u32 output")
                .as_any()
                .downcast_ref::<UInt32Array>()
                .expect("u32 output");
            values.extend(column.values().iter().copied());
        }
        values
    }

    fn violation_codes(
        execution: &DerivedProducerClosureExecution,
        bindings: &DerivedProducerClosureBindings,
    ) -> BTreeSet<String> {
        string_values(
            execution.violations(),
            bindings.violation.fields.violation_code.as_str(),
        )
        .into_iter()
        .flatten()
        .collect()
    }

    fn stable_rows(batches: &[RecordBatch]) -> Vec<Vec<Option<String>>> {
        let mut rows = Vec::new();
        for batch in batches {
            for row in 0..batch.num_rows() {
                let mut rendered = Vec::new();
                for column in batch.columns() {
                    if let Some(strings) = column.as_any().downcast_ref::<StringArray>() {
                        rendered
                            .push((!strings.is_null(row)).then(|| strings.value(row).to_owned()));
                    } else if let Some(values) = column.as_any().downcast_ref::<UInt32Array>() {
                        rendered.push(Some(values.value(row).to_string()));
                    } else if let Some(values) = column.as_any().downcast_ref::<UInt64Array>() {
                        rendered
                            .push((!values.is_null(row)).then(|| values.value(row).to_string()));
                    } else {
                        panic!("unexpected output type")
                    }
                }
                rows.push(rendered);
            }
        }
        rows
    }

    #[tokio::test]
    async fn exact_application_producer_closes_one_family() {
        let bindings = bindings();
        let producer = producer("family.control-dependence", "producer.common-cdg@1");
        let execution = execute(
            &bindings,
            inputs(
                &bindings,
                &[("family.control-dependence", FACT_CLASS)],
                &[producer],
                &[],
                &[],
            ),
        )
        .await;

        assert!(execution.is_conformant());
        assert_eq!(
            string_values(
                execution.family_closure(),
                bindings.family_closure.fields.closure_state.as_str(),
            ),
            vec![Some(STATE_SUPPORTED.to_owned())]
        );
        assert!(
            execution
                .observation()
                .operators()
                .contains(&ProducerClosureNativeOperator::RecursiveQueryDistinct)
        );
        assert!(execution.observation().dependencies().contains(
            &ProducerClosureCompilationDependency::ApplicationOwnedAuthority(Arc::from(
                APP_AUTHORITY,
            ))
        ));
    }

    #[tokio::test]
    async fn explicit_remainder_closes_family_and_downgrades_query() {
        let bindings = bindings();
        let remainder = remainder("family.dynamic-call-target");
        let execution = execute(
            &bindings,
            inputs(
                &bindings,
                &[("family.dynamic-call-target", FACT_CLASS)],
                &[],
                &[("query.callers", "family.dynamic-call-target")],
                &[remainder],
            ),
        )
        .await;

        assert!(execution.is_conformant());
        assert_eq!(
            string_values(
                execution.family_closure(),
                bindings.family_closure.fields.closure_state.as_str(),
            ),
            vec![Some(STATE_UNSUPPORTED.to_owned())]
        );
        assert_eq!(
            string_values(
                execution.query_requirement_closure(),
                bindings
                    .query_requirement_closure
                    .fields
                    .requirement_state
                    .as_str(),
            ),
            vec![Some(STATE_UNSUPPORTED.to_owned())]
        );
        assert_eq!(
            string_values(
                execution.query_requirement_closure(),
                bindings
                    .query_requirement_closure
                    .fields
                    .unknown_cause
                    .as_str(),
            ),
            vec![Some("unknown.dynamic-dispatch".to_owned())]
        );
    }

    #[tokio::test]
    async fn zero_multiple_and_both_are_independent_violations() {
        let bindings = bindings();
        let producer_a = producer("family.multiple", "producer.a@1");
        let producer_b = producer("family.multiple", "producer.b@1");
        let producer_both = producer("family.both", "producer.both@1");
        let remainder_both = remainder("family.both");
        let execution = execute(
            &bindings,
            inputs(
                &bindings,
                &[
                    ("family.zero", FACT_CLASS),
                    ("family.multiple", FACT_CLASS),
                    ("family.both", FACT_CLASS),
                ],
                &[producer_a, producer_b, producer_both],
                &[],
                &[remainder_both],
            ),
        )
        .await;
        let codes = violation_codes(&execution, &bindings);

        assert!(!execution.is_conformant());
        assert!(codes.contains("missing_producer_or_remainder"));
        assert!(codes.contains("multiple_runtime_producers"));
        assert!(codes.contains("producer_and_remainder"));
        assert_eq!(
            string_values(
                execution.family_closure(),
                bindings.family_closure.fields.closure_state.as_str(),
            ),
            vec![
                Some(STATE_INVALID.to_owned()),
                Some(STATE_INVALID.to_owned()),
                Some(STATE_INVALID.to_owned()),
            ]
        );
    }

    #[tokio::test]
    async fn transitive_gap_and_incomplete_family_propagate_to_queries() {
        let bindings = bindings();
        let mut partial = producer("family.partial", "producer.partial@1");
        partial[9] = "0";
        partial[11] = "1";
        let execution = execute(
            &bindings,
            inputs(
                &bindings,
                &[("family.partial", FACT_CLASS)],
                &[partial],
                &[
                    ("query.root", "query.intermediate"),
                    ("query.intermediate", "family.absent"),
                    ("query.partial", "family.partial"),
                ],
                &[],
            ),
        )
        .await;
        let query_fields = &bindings.query_requirement_closure.fields;
        let roots = string_values(
            execution.query_requirement_closure(),
            query_fields.query_family_id.as_str(),
        );
        let required = string_values(
            execution.query_requirement_closure(),
            query_fields.required_family_id.as_str(),
        );
        let states = string_values(
            execution.query_requirement_closure(),
            query_fields.requirement_state.as_str(),
        );
        let depths = u32_values(
            execution.query_requirement_closure(),
            query_fields.minimum_depth.as_str(),
        );
        let rows = roots
            .into_iter()
            .zip(required)
            .zip(states)
            .zip(depths)
            .map(|(((root, required), state), depth)| {
                (root.unwrap(), required.unwrap(), state.unwrap(), depth)
            })
            .collect::<Vec<_>>();

        assert!(rows.contains(&(
            "query.root".to_owned(),
            "family.absent".to_owned(),
            STATE_MISSING.to_owned(),
            2,
        )));
        assert!(rows.contains(&(
            "query.partial".to_owned(),
            "family.partial".to_owned(),
            STATE_UNKNOWN.to_owned(),
            1,
        )));
        let codes = violation_codes(&execution, &bindings);
        assert!(codes.contains("query_requirement_missing"));
        assert!(codes.contains("query_requirement_incomplete"));
        assert!(codes.contains("incomplete_runtime_producer"));
    }

    #[tokio::test]
    async fn provider_authority_and_judgment_semantics_are_rejected() {
        let bindings = bindings();
        let mut provider = producer("family.refactor-risk", "provider.raw-risk@1");
        provider[2] = PROVIDER_AUTHORITY;
        let execution = execute(
            &bindings,
            inputs(
                &bindings,
                &[("family.refactor-risk", JUDGMENT_CLASS)],
                &[provider],
                &[],
                &[],
            ),
        )
        .await;
        let codes = violation_codes(&execution, &bindings);

        assert!(!execution.is_conformant());
        assert!(codes.contains("wrong_runtime_producer_authority"));
        assert!(codes.contains("non_fact_semantic_class"));
        assert_eq!(
            string_values(
                execution.family_closure(),
                bindings.family_closure.fields.closure_state.as_str(),
            ),
            vec![Some(STATE_INVALID.to_owned())]
        );
    }

    #[tokio::test]
    async fn input_permutation_does_not_change_output() {
        let bindings = bindings();
        let producer_a = producer("family.a", "producer.a@1");
        let producer_b = producer("family.b", "producer.b@1");
        let remainder_c = remainder("family.c");
        let left = execute(
            &bindings,
            inputs(
                &bindings,
                &[
                    ("family.c", FACT_CLASS),
                    ("family.a", FACT_CLASS),
                    ("family.b", FACT_CLASS),
                ],
                &[producer_b, producer_a],
                &[("query.all", "family.c"), ("query.all", "family.a")],
                &[remainder_c],
            ),
        )
        .await;
        let producer_a = producer("family.a", "producer.a@1");
        let producer_b = producer("family.b", "producer.b@1");
        let remainder_c = remainder("family.c");
        let right = execute(
            &bindings,
            inputs(
                &bindings,
                &[
                    ("family.b", FACT_CLASS),
                    ("family.a", FACT_CLASS),
                    ("family.c", FACT_CLASS),
                ],
                &[producer_a, producer_b],
                &[("query.all", "family.a"), ("query.all", "family.c")],
                &[remainder_c],
            ),
        )
        .await;

        assert_eq!(
            stable_rows(left.family_closure()),
            stable_rows(right.family_closure())
        );
        assert_eq!(
            stable_rows(left.query_requirement_closure()),
            stable_rows(right.query_requirement_closure())
        );
        assert_eq!(
            stable_rows(left.violations()),
            stable_rows(right.violations())
        );
    }

    #[tokio::test]
    async fn empty_relations_preserve_all_declared_output_schemas() {
        let bindings = bindings();
        let execution = execute(&bindings, inputs(&bindings, &[], &[], &[], &[])).await;

        assert!(execution.is_conformant());
        assert_eq!(execution.family_closure().len(), 1);
        assert_eq!(execution.family_closure()[0].num_rows(), 0);
        assert_eq!(
            execution.family_closure()[0].schema_ref(),
            execution.family_closure_schema()
        );
        assert_eq!(execution.query_requirement_closure().len(), 1);
        assert_eq!(execution.query_requirement_closure()[0].num_rows(), 0);
        assert_eq!(
            execution.query_requirement_closure()[0].schema_ref(),
            execution.query_requirement_closure_schema()
        );
        assert_eq!(execution.violations().len(), 1);
        assert_eq!(execution.violations()[0].num_rows(), 0);
        assert_eq!(
            execution.violations()[0].schema_ref(),
            execution.violation_schema()
        );
    }
}
