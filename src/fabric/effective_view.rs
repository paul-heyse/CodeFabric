//! Native DataFusion compilation for exact-base plus immutable-segment effective views.
//!
//! The compiler deliberately produces ordinary DataFusion logical operators. A separate native
//! tie-violation plan is part of the compilation result: callers must prove it empty before
//! treating the effective plan as a complete present-state relation.

use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::{DataType, Schema, SchemaRef};
use datafusion::common::{Column, TableReference};
use datafusion::execution::context::SessionContext;
use datafusion::functions_aggregate::count::count_all_window;
use datafusion::functions_window::expr_fn::row_number;
use datafusion::logical_expr::utils::can_hash;
use datafusion::logical_expr::{Expr, ExprFunctionExt, JoinType, LogicalPlan, LogicalPlanBuilder};
use datafusion::prelude::{col, lit};
use thiserror::Error;

const ROW_NUMBER_COLUMN: &str = "__codefabric_effective_row_number";
const TIE_COUNT_COLUMN: &str = "__codefabric_effective_tie_count";

/// One exact, already-bound logical input to an effective view.
#[derive(Clone, Debug)]
pub struct EffectiveViewInput {
    reference: TableReference,
    plan: LogicalPlan,
}

impl EffectiveViewInput {
    #[must_use]
    pub fn new(reference: TableReference, plan: LogicalPlan) -> Self {
        Self { reference, plan }
    }

    #[must_use]
    pub const fn reference(&self) -> &TableReference {
        &self.reference
    }

    #[must_use]
    pub const fn plan(&self) -> &LogicalPlan {
        &self.plan
    }
}

/// Application-owned direction for one deterministic ordering field.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EffectiveViewSortDirection {
    Ascending,
    Descending,
}

impl EffectiveViewSortDirection {
    const fn ascending(self) -> bool {
        matches!(self, Self::Ascending)
    }
}

/// One application-contract deterministic ordering field.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EffectiveViewOrderField {
    field_name: Arc<str>,
    direction: EffectiveViewSortDirection,
}

impl EffectiveViewOrderField {
    #[must_use]
    pub fn new(field_name: impl Into<Arc<str>>, direction: EffectiveViewSortDirection) -> Self {
        Self {
            field_name: field_name.into(),
            direction,
        }
    }

    #[must_use]
    pub fn descending(field_name: impl Into<Arc<str>>) -> Self {
        Self::new(field_name, EffectiveViewSortDirection::Descending)
    }

    #[must_use]
    pub const fn field_name(&self) -> &Arc<str> {
        &self.field_name
    }

    #[must_use]
    pub const fn direction(&self) -> EffectiveViewSortDirection {
        self.direction
    }
}

/// Application-owned physical field bindings and exact schemas for one effective relation.
#[derive(Clone, Debug)]
pub struct EffectiveViewFieldBindings {
    input_schema: SchemaRef,
    output_schema: SchemaRef,
    primary_key: Arc<[Arc<str>]>,
    generation: Arc<str>,
    deterministic_order: Arc<[EffectiveViewOrderField]>,
    tombstone: Arc<str>,
}

impl EffectiveViewFieldBindings {
    /// Validate and retain the complete field contract used by native plan compilation.
    ///
    /// # Errors
    ///
    /// Rejects absent, duplicate, nullable, non-hashable, or overlapping control bindings and
    /// any declared output that is not an exact control-free projection of the input schema.
    pub fn try_new(
        input_schema: SchemaRef,
        output_schema: SchemaRef,
        primary_key: Vec<impl Into<Arc<str>>>,
        generation: impl Into<Arc<str>>,
        deterministic_order: Vec<EffectiveViewOrderField>,
        tombstone: impl Into<Arc<str>>,
    ) -> Result<Self, EffectiveViewError> {
        let primary_key = primary_key.into_iter().map(Into::into).collect::<Vec<_>>();
        let generation = generation.into();
        let tombstone = tombstone.into();

        if primary_key.is_empty() {
            return Err(EffectiveViewError::EmptyPrimaryKey);
        }
        if deterministic_order.is_empty() {
            return Err(EffectiveViewError::EmptyDeterministicOrder);
        }

        validate_unique_schema_fields("input", input_schema.as_ref())?;
        validate_unique_schema_fields("output", output_schema.as_ref())?;
        for reserved in [ROW_NUMBER_COLUMN, TIE_COUNT_COLUMN] {
            if input_schema.field_with_name(reserved).is_ok() {
                return Err(EffectiveViewError::ReservedFieldName(reserved));
            }
        }

        let mut semantic_bindings = HashSet::new();
        for name in &primary_key {
            register_unique_binding(&mut semantic_bindings, "primary key", name)?;
            validate_required_field(&input_schema, name, true, true)?;
        }
        register_unique_binding(&mut semantic_bindings, "generation", &generation)?;
        validate_required_field(&input_schema, &generation, true, true)?;

        for ordering in &deterministic_order {
            register_unique_binding(
                &mut semantic_bindings,
                "deterministic order",
                &ordering.field_name,
            )?;
            validate_required_field(&input_schema, &ordering.field_name, true, true)?;
        }

        register_unique_binding(&mut semantic_bindings, "tombstone", &tombstone)?;
        let tombstone_field = validate_required_field(&input_schema, &tombstone, true, false)?;
        if tombstone_field.data_type() != &DataType::Boolean {
            return Err(EffectiveViewError::InvalidTombstoneType {
                field: tombstone,
                actual: tombstone_field.data_type().clone(),
            });
        }

        for output_field in output_schema.fields() {
            if semantic_bindings.contains(output_field.name().as_str())
                && !primary_key
                    .iter()
                    .any(|primary_key| primary_key.as_ref() == output_field.name())
            {
                return Err(EffectiveViewError::ControlFieldInOutput(
                    output_field.name().clone(),
                ));
            }
            let input_field = input_schema
                .field_with_name(output_field.name())
                .map_err(|_| EffectiveViewError::MissingOutputField(output_field.name().clone()))?;
            if input_field != output_field.as_ref() {
                return Err(EffectiveViewError::OutputFieldMismatch {
                    field: output_field.name().clone(),
                    expected: Arc::new(input_field.clone()),
                    actual: Arc::clone(output_field),
                });
            }
        }

        Ok(Self {
            input_schema,
            output_schema,
            primary_key: primary_key.into(),
            generation,
            deterministic_order: deterministic_order.into(),
            tombstone,
        })
    }

    #[must_use]
    pub const fn input_schema(&self) -> &SchemaRef {
        &self.input_schema
    }

    #[must_use]
    pub const fn output_schema(&self) -> &SchemaRef {
        &self.output_schema
    }

    #[must_use]
    pub fn primary_key(&self) -> &[Arc<str>] {
        &self.primary_key
    }

    #[must_use]
    pub const fn generation(&self) -> &Arc<str> {
        &self.generation
    }

    #[must_use]
    pub fn deterministic_order(&self) -> &[EffectiveViewOrderField] {
        &self.deterministic_order
    }

    #[must_use]
    pub const fn tombstone(&self) -> &Arc<str> {
        &self.tombstone
    }
}

/// The input role observed by successful compilation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EffectiveViewInputRole {
    ExactBase,
    ImmutableOverlaySegment,
}

/// One exact input dependency observed by successful compilation.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EffectiveViewInputObservation {
    role: EffectiveViewInputRole,
    reference: TableReference,
}

impl EffectiveViewInputObservation {
    #[must_use]
    pub const fn role(&self) -> EffectiveViewInputRole {
        self.role
    }

    #[must_use]
    pub const fn reference(&self) -> &TableReference {
        &self.reference
    }
}

/// Semantic role of one model field observed by effective-view compilation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectiveViewFieldRole {
    PrimaryKey {
        ordinal: usize,
    },
    Generation,
    DeterministicOrder {
        ordinal: usize,
        direction: EffectiveViewSortDirection,
    },
    Tombstone,
    Output {
        ordinal: usize,
    },
}

/// Exact type and nullability of one field that causally shaped the compiled plans.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectiveViewFieldObservation {
    role: EffectiveViewFieldRole,
    field_name: Arc<str>,
    data_type: DataType,
    nullable: bool,
}

impl EffectiveViewFieldObservation {
    #[must_use]
    pub const fn role(&self) -> EffectiveViewFieldRole {
        self.role
    }

    #[must_use]
    pub const fn field_name(&self) -> &Arc<str> {
        &self.field_name
    }

    #[must_use]
    pub const fn data_type(&self) -> &DataType {
        &self.data_type
    }

    #[must_use]
    pub const fn nullable(&self) -> bool {
        self.nullable
    }
}

/// Highest viable DataFusion extension rung selected for the effective view.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EffectiveViewExtensionRung {
    NativeLogicalPlan,
}

/// Native operators causally selected by effective-view compilation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EffectiveViewNativeOperator {
    UnionAll,
    ReplacementAntiJoin,
    TombstoneAntiJoin,
    TieCountWindow,
    RowNumberWindow,
    TieViolationFilter,
    LatestRowFilter,
    TombstoneFilter,
    OutputProjection,
}

/// Typed causal evidence emitted by the compiler that actually constructed the plans.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectiveViewCompilationObservation {
    rung: EffectiveViewExtensionRung,
    inputs: Arc<[EffectiveViewInputObservation]>,
    field_bindings: Arc<[EffectiveViewFieldObservation]>,
    operators: BTreeSet<EffectiveViewNativeOperator>,
}

impl EffectiveViewCompilationObservation {
    #[must_use]
    pub const fn rung(&self) -> EffectiveViewExtensionRung {
        self.rung
    }

    #[must_use]
    pub fn inputs(&self) -> &[EffectiveViewInputObservation] {
        &self.inputs
    }

    #[must_use]
    pub fn field_bindings(&self) -> &[EffectiveViewFieldObservation] {
        &self.field_bindings
    }

    #[must_use]
    pub const fn operators(&self) -> &BTreeSet<EffectiveViewNativeOperator> {
        &self.operators
    }
}

/// Two native plans whose shared immutable inputs make the effective view execution-proved.
#[derive(Clone, Debug)]
pub struct CompiledEffectiveView {
    effective_plan: LogicalPlan,
    tie_violation_plan: LogicalPlan,
    output_schema: SchemaRef,
    observation: EffectiveViewCompilationObservation,
}

impl CompiledEffectiveView {
    #[must_use]
    pub const fn effective_plan(&self) -> &LogicalPlan {
        &self.effective_plan
    }

    #[must_use]
    pub const fn tie_violation_plan(&self) -> &LogicalPlan {
        &self.tie_violation_plan
    }

    #[must_use]
    pub const fn output_schema(&self) -> &SchemaRef {
        &self.output_schema
    }

    #[must_use]
    pub const fn observation(&self) -> &EffectiveViewCompilationObservation {
        &self.observation
    }

    /// Prove the deterministic-order relation and execute the effective plan in one session.
    ///
    /// The inputs are immutable authorities. Any non-empty tie plan rejects the view rather than
    /// allowing `row_number` to select an arbitrary peer.
    ///
    /// # Errors
    ///
    /// Returns a typed tie rejection, a DataFusion planning/execution failure, or exact output
    /// schema drift.
    pub async fn execute_proved(
        &self,
        context: &SessionContext,
    ) -> Result<EffectiveViewExecution, EffectiveViewError> {
        let tie_batches = context
            .execute_logical_plan(self.tie_violation_plan.clone())
            .await?
            .collect()
            .await?;
        let conflicting_rows = tie_batches.iter().map(RecordBatch::num_rows).sum::<usize>();
        if conflicting_rows != 0 {
            return Err(EffectiveViewError::NonDeterministicTie { conflicting_rows });
        }

        let batches = context
            .execute_logical_plan(self.effective_plan.clone())
            .await?
            .collect()
            .await?;
        for batch in &batches {
            if batch.schema_ref().as_ref() != self.output_schema.as_ref() {
                return Err(EffectiveViewError::ExecutedOutputSchemaMismatch {
                    expected: Arc::clone(&self.output_schema),
                    actual: batch.schema(),
                });
            }
        }

        Ok(EffectiveViewExecution {
            schema: Arc::clone(&self.output_schema),
            batches,
            tie_proof: EffectiveViewTieProof { examined_ties: 0 },
            observation: self.observation.clone(),
        })
    }
}

/// Proof that the complete deterministic ordering key had no duplicate rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectiveViewTieProof {
    examined_ties: usize,
}

impl EffectiveViewTieProof {
    #[must_use]
    pub const fn examined_ties(self) -> usize {
        self.examined_ties
    }
}

/// Exact-schema batches emitted only after deterministic-order proof succeeds.
#[derive(Clone, Debug)]
pub struct EffectiveViewExecution {
    schema: SchemaRef,
    batches: Vec<RecordBatch>,
    tie_proof: EffectiveViewTieProof,
    observation: EffectiveViewCompilationObservation,
}

impl EffectiveViewExecution {
    #[must_use]
    pub const fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    #[must_use]
    pub fn batches(&self) -> &[RecordBatch] {
        &self.batches
    }

    #[must_use]
    pub const fn tie_proof(&self) -> EffectiveViewTieProof {
        self.tie_proof
    }

    #[must_use]
    pub const fn observation(&self) -> &EffectiveViewCompilationObservation {
        &self.observation
    }

    #[must_use]
    pub fn into_batches(self) -> Vec<RecordBatch> {
        self.batches
    }
}

/// Compile one exact base and zero or more immutable overlay segments into native plans.
///
/// # Errors
///
/// Rejects duplicate inputs, any exact Arrow or qualifier mismatch, unsupported bindings, a
/// DataFusion logical-plan construction failure, or declared output-schema drift.
pub fn compile_effective_view(
    base: EffectiveViewInput,
    mut overlay_segments: Vec<EffectiveViewInput>,
    bindings: &EffectiveViewFieldBindings,
) -> Result<CompiledEffectiveView, EffectiveViewError> {
    overlay_segments.sort_by(|left, right| left.reference.cmp(&right.reference));

    let mut references = HashSet::new();
    if !references.insert(base.reference.clone()) {
        return Err(EffectiveViewError::DuplicateInput(base.reference));
    }
    validate_input(&base, EffectiveViewInputRole::ExactBase, bindings)?;

    let mut observations = vec![EffectiveViewInputObservation {
        role: EffectiveViewInputRole::ExactBase,
        reference: base.reference.clone(),
    }];
    for segment in &overlay_segments {
        if !references.insert(segment.reference.clone()) {
            return Err(EffectiveViewError::DuplicateInput(
                segment.reference.clone(),
            ));
        }
        validate_input(
            segment,
            EffectiveViewInputRole::ImmutableOverlaySegment,
            bindings,
        )?;
        observations.push(EffectiveViewInputObservation {
            role: EffectiveViewInputRole::ImmutableOverlaySegment,
            reference: segment.reference.clone(),
        });
    }

    let has_overlay = !overlay_segments.is_empty();
    let base_plan = base.plan;
    let overlay_plans = overlay_segments
        .into_iter()
        .map(|segment| segment.plan)
        .collect::<Vec<_>>();

    let tie_violation_plan = build_tie_violation_plan(&base_plan, &overlay_plans, bindings)?;
    let effective_rows = build_effective_rows(base_plan, &overlay_plans, bindings)?;

    let live = LogicalPlanBuilder::from(effective_rows)
        .filter(col(bindings.tombstone.as_ref()).eq(lit(false)))?
        .build()?;
    let output_projection = bindings
        .output_schema
        .fields()
        .iter()
        .map(|field| Expr::Column(Column::new_unqualified(field.name().clone())))
        .collect::<Vec<_>>();
    let effective_plan = LogicalPlanBuilder::from(live)
        .project(output_projection)?
        .build()?;

    if effective_plan.schema().as_arrow() != bindings.output_schema.as_ref() {
        return Err(EffectiveViewError::DeclaredOutputSchemaMismatch {
            expected: Arc::clone(&bindings.output_schema),
            actual: Arc::new(effective_plan.schema().as_arrow().clone()),
        });
    }

    let mut operators = BTreeSet::from([
        EffectiveViewNativeOperator::TieCountWindow,
        EffectiveViewNativeOperator::RowNumberWindow,
        EffectiveViewNativeOperator::TieViolationFilter,
        EffectiveViewNativeOperator::LatestRowFilter,
        EffectiveViewNativeOperator::TombstoneFilter,
        EffectiveViewNativeOperator::OutputProjection,
    ]);
    if has_overlay {
        operators.insert(EffectiveViewNativeOperator::UnionAll);
        operators.insert(EffectiveViewNativeOperator::ReplacementAntiJoin);
        operators.insert(EffectiveViewNativeOperator::TombstoneAntiJoin);
    }
    let field_bindings = observe_field_bindings(bindings)?;

    Ok(CompiledEffectiveView {
        effective_plan,
        tie_violation_plan,
        output_schema: Arc::clone(&bindings.output_schema),
        observation: EffectiveViewCompilationObservation {
            rung: EffectiveViewExtensionRung::NativeLogicalPlan,
            inputs: observations.into(),
            field_bindings: field_bindings.into(),
            operators,
        },
    })
}

fn build_tie_violation_plan(
    base: &LogicalPlan,
    overlays: &[LogicalPlan],
    bindings: &EffectiveViewFieldBindings,
) -> Result<LogicalPlan, EffectiveViewError> {
    // This proof spans base and overlays. It rejects two rows that claim the same complete
    // application-owned ordering identity, even when the duplicate crosses the storage boundary.
    // Effective replacement semantics remain separate and never compare base generations.
    let mut proof_union = base.clone();
    for segment in overlays {
        proof_union = LogicalPlanBuilder::from(proof_union)
            .union(segment.clone())?
            .build()?;
    }

    let mut full_order_key = bindings
        .primary_key
        .iter()
        .map(|name| col(name.as_ref()))
        .collect::<Vec<_>>();
    full_order_key.push(col(bindings.generation.as_ref()));
    full_order_key.extend(
        bindings
            .deterministic_order
            .iter()
            .map(|ordering| col(ordering.field_name.as_ref())),
    );
    let tie_count = count_all_window()
        .partition_by(full_order_key)
        .build()?
        .alias(TIE_COUNT_COLUMN);
    let tie_counted = LogicalPlanBuilder::from(proof_union)
        .window([tie_count])?
        .build()?;

    let mut projection = bindings
        .primary_key
        .iter()
        .map(|name| col(name.as_ref()))
        .collect::<Vec<_>>();
    projection.push(col(bindings.generation.as_ref()));
    projection.extend(
        bindings
            .deterministic_order
            .iter()
            .map(|ordering| col(ordering.field_name.as_ref())),
    );
    projection.push(col(TIE_COUNT_COLUMN));
    Ok(LogicalPlanBuilder::from(tie_counted)
        .filter(col(TIE_COUNT_COLUMN).gt(lit(1_i64)))?
        .project(projection)?
        .build()?)
}

fn build_effective_rows(
    base: LogicalPlan,
    overlays: &[LogicalPlan],
    bindings: &EffectiveViewFieldBindings,
) -> Result<LogicalPlan, EffectiveViewError> {
    if overlays.is_empty() {
        return Ok(base);
    }

    // Segment presence is the replacement authority. Generation and deterministic order choose
    // only between immutable segment rows within this epoch.
    let latest = rank_latest_overlays(overlays, bindings)?;
    let replacement_keys = overlay_keys(&latest, bindings, false)?;
    let tombstone_keys = overlay_keys(&latest, bindings, true)?;
    let join_keys = bindings
        .primary_key
        .iter()
        .map(|name| Column::new_unqualified(name.as_ref()))
        .collect::<Vec<_>>();
    let untouched_base = LogicalPlanBuilder::from(base)
        .join(
            replacement_keys,
            JoinType::LeftAnti,
            (join_keys.clone(), join_keys.clone()),
            None,
        )?
        .join(
            tombstone_keys,
            JoinType::LeftAnti,
            (join_keys.clone(), join_keys),
            None,
        )?
        .build()?;
    let live_overlays = LogicalPlanBuilder::from(latest)
        .filter(col(bindings.tombstone.as_ref()).eq(lit(false)))?
        .build()?;
    Ok(LogicalPlanBuilder::from(untouched_base)
        .union(live_overlays)?
        .build()?)
}

fn rank_latest_overlays(
    overlays: &[LogicalPlan],
    bindings: &EffectiveViewFieldBindings,
) -> Result<LogicalPlan, EffectiveViewError> {
    let (first, rest) = overlays
        .split_first()
        .expect("caller proves at least one immutable overlay");
    let mut union = first.clone();
    for segment in rest {
        union = LogicalPlanBuilder::from(union)
            .union(segment.clone())?
            .build()?;
    }
    let primary_key = bindings
        .primary_key
        .iter()
        .map(|name| col(name.as_ref()))
        .collect::<Vec<_>>();
    let mut latest_order = vec![col(bindings.generation.as_ref()).sort(false, false)];
    latest_order.extend(bindings.deterministic_order.iter().map(|ordering| {
        col(ordering.field_name.as_ref()).sort(ordering.direction.ascending(), false)
    }));
    let row_number = row_number()
        .partition_by(primary_key)
        .order_by(latest_order)
        .build()?
        .alias(ROW_NUMBER_COLUMN);
    let ranked = LogicalPlanBuilder::from(union)
        .window([row_number])?
        .filter(col(ROW_NUMBER_COLUMN).eq(lit(1_u64)))?
        .build()?;
    let input_projection = bindings
        .input_schema
        .fields()
        .iter()
        .map(|field| col(field.name()))
        .collect::<Vec<_>>();
    Ok(LogicalPlanBuilder::from(ranked)
        .project(input_projection)?
        .build()?)
}

fn overlay_keys(
    latest: &LogicalPlan,
    bindings: &EffectiveViewFieldBindings,
    tombstone: bool,
) -> Result<LogicalPlan, EffectiveViewError> {
    let key_projection = bindings
        .primary_key
        .iter()
        .map(|name| col(name.as_ref()))
        .collect::<Vec<_>>();
    Ok(LogicalPlanBuilder::from(latest.clone())
        .filter(col(bindings.tombstone.as_ref()).eq(lit(tombstone)))?
        .project(key_projection)?
        .build()?)
}

fn observe_field_bindings(
    bindings: &EffectiveViewFieldBindings,
) -> Result<Vec<EffectiveViewFieldObservation>, EffectiveViewError> {
    let mut observations = Vec::new();
    for (ordinal, name) in bindings.primary_key.iter().enumerate() {
        observations.push(field_observation(
            bindings,
            name,
            EffectiveViewFieldRole::PrimaryKey { ordinal },
        )?);
    }
    observations.push(field_observation(
        bindings,
        &bindings.generation,
        EffectiveViewFieldRole::Generation,
    )?);
    for (ordinal, ordering) in bindings.deterministic_order.iter().enumerate() {
        observations.push(field_observation(
            bindings,
            &ordering.field_name,
            EffectiveViewFieldRole::DeterministicOrder {
                ordinal,
                direction: ordering.direction,
            },
        )?);
    }
    observations.push(field_observation(
        bindings,
        &bindings.tombstone,
        EffectiveViewFieldRole::Tombstone,
    )?);
    for (ordinal, field) in bindings.output_schema.fields().iter().enumerate() {
        observations.push(EffectiveViewFieldObservation {
            role: EffectiveViewFieldRole::Output { ordinal },
            field_name: Arc::from(field.name().as_str()),
            data_type: field.data_type().clone(),
            nullable: field.is_nullable(),
        });
    }
    Ok(observations)
}

fn field_observation(
    bindings: &EffectiveViewFieldBindings,
    name: &str,
    role: EffectiveViewFieldRole,
) -> Result<EffectiveViewFieldObservation, EffectiveViewError> {
    let field = bindings
        .input_schema
        .field_with_name(name)
        .map_err(|_| EffectiveViewError::MissingBoundField(name.to_owned()))?;
    Ok(EffectiveViewFieldObservation {
        role,
        field_name: Arc::from(name),
        data_type: field.data_type().clone(),
        nullable: field.is_nullable(),
    })
}

fn validate_unique_schema_fields(
    schema_role: &'static str,
    schema: &Schema,
) -> Result<(), EffectiveViewError> {
    let mut names = HashSet::new();
    for field in schema.fields() {
        if !names.insert(field.name()) {
            return Err(EffectiveViewError::DuplicateSchemaField {
                schema_role,
                field: field.name().clone(),
            });
        }
    }
    Ok(())
}

fn register_unique_binding<'a>(
    bindings: &mut HashSet<&'a str>,
    role: &'static str,
    name: &'a str,
) -> Result<(), EffectiveViewError> {
    if !bindings.insert(name) {
        return Err(EffectiveViewError::OverlappingFieldBinding {
            role,
            field: name.to_owned(),
        });
    }
    Ok(())
}

fn validate_required_field<'a>(
    schema: &'a SchemaRef,
    name: &str,
    require_non_null: bool,
    require_hashable: bool,
) -> Result<&'a arrow_schema::Field, EffectiveViewError> {
    let field = schema
        .field_with_name(name)
        .map_err(|_| EffectiveViewError::MissingBoundField(name.to_owned()))?;
    if require_non_null && field.is_nullable() {
        return Err(EffectiveViewError::NullableBoundField(name.to_owned()));
    }
    if require_hashable && !can_hash(field.data_type()) {
        return Err(EffectiveViewError::NonHashableBoundField {
            field: name.to_owned(),
            data_type: field.data_type().clone(),
        });
    }
    Ok(field)
}

fn validate_input(
    input: &EffectiveViewInput,
    role: EffectiveViewInputRole,
    bindings: &EffectiveViewFieldBindings,
) -> Result<(), EffectiveViewError> {
    if input.plan.schema().as_arrow() != bindings.input_schema.as_ref() {
        return Err(EffectiveViewError::InputSchemaMismatch {
            role,
            reference: input.reference.clone(),
            expected: Arc::clone(&bindings.input_schema),
            actual: Arc::new(input.plan.schema().as_arrow().clone()),
        });
    }
    for (field_index, (qualifier, _)) in input.plan.schema().iter().enumerate() {
        if qualifier != Some(&input.reference) {
            return Err(EffectiveViewError::InputQualifierMismatch {
                role,
                reference: input.reference.clone(),
                field_index,
                actual: qualifier.cloned(),
            });
        }
    }
    Ok(())
}

/// Fail-closed effective-view compilation and execution errors.
#[derive(Debug, Error)]
pub enum EffectiveViewError {
    #[error("effective-view primary key must contain at least one field")]
    EmptyPrimaryKey,
    #[error("effective-view deterministic order must contain at least one tie-break field")]
    EmptyDeterministicOrder,
    #[error("reserved internal effective-view field is present: {0}")]
    ReservedFieldName(&'static str),
    #[error("duplicate {schema_role} schema field {field}")]
    DuplicateSchemaField {
        schema_role: &'static str,
        field: String,
    },
    #[error("{role} binding overlaps an earlier semantic binding at field {field}")]
    OverlappingFieldBinding { role: &'static str, field: String },
    #[error("bound effective-view field is absent: {0}")]
    MissingBoundField(String),
    #[error("bound effective-view field must be non-nullable: {0}")]
    NullableBoundField(String),
    #[error("bound effective-view field {field} with type {data_type:?} is not hashable")]
    NonHashableBoundField { field: String, data_type: DataType },
    #[error("tombstone field {field} must be Boolean, actual {actual:?}")]
    InvalidTombstoneType { field: Arc<str>, actual: DataType },
    #[error("control field cannot survive effective-view output projection: {0}")]
    ControlFieldInOutput(String),
    #[error("declared output field is absent from input: {0}")]
    MissingOutputField(String),
    #[error("declared output field {field} differs: expected {expected:?}, actual {actual:?}")]
    OutputFieldMismatch {
        field: String,
        expected: Arc<arrow_schema::Field>,
        actual: Arc<arrow_schema::Field>,
    },
    #[error("effective-view input is duplicated: {0}")]
    DuplicateInput(TableReference),
    #[error("{role:?} input {reference} schema differs: expected {expected:?}, actual {actual:?}")]
    InputSchemaMismatch {
        role: EffectiveViewInputRole,
        reference: TableReference,
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error(
        "{role:?} input {reference} qualifier differs at field {field_index}: actual {actual:?}"
    )]
    InputQualifierMismatch {
        role: EffectiveViewInputRole,
        reference: TableReference,
        field_index: usize,
        actual: Option<TableReference>,
    },
    #[error(
        "declared effective-view output schema differs: expected {expected:?}, actual {actual:?}"
    )]
    DeclaredOutputSchemaMismatch {
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("effective-view deterministic ordering has {conflicting_rows} tied rows")]
    NonDeterministicTie { conflicting_rows: usize },
    #[error(
        "executed effective-view output schema differs: expected {expected:?}, actual {actual:?}"
    )]
    ExecutedOutputSchemaMismatch {
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error(transparent)]
    DataFusion(#[from] datafusion::error::DataFusionError),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use arrow_array::{Array as _, BooleanArray, Int64Array, RecordBatch, StringArray};
    use arrow_schema::{DataType, Field, Schema, SchemaRef};
    use datafusion::datasource::MemTable;
    use datafusion::execution::context::SessionContext;
    use datafusion::logical_expr::{JoinType, LogicalPlan};

    use super::{
        CompiledEffectiveView, EffectiveViewError, EffectiveViewExtensionRung,
        EffectiveViewFieldBindings, EffectiveViewInput, EffectiveViewNativeOperator,
        EffectiveViewOrderField, compile_effective_view,
    };

    type Row<'a> = (i64, Option<&'a str>, i64, i64, bool);

    fn input_schema() -> SchemaRef {
        Arc::new(Schema::new(vec![
            Arc::new(Field::new("id", DataType::Int64, false)),
            Arc::new(Field::new("value", DataType::Utf8, true)),
            Arc::new(Field::new("generation", DataType::Int64, false)),
            Arc::new(Field::new("operation_order", DataType::Int64, false)),
            Arc::new(Field::new("tombstone", DataType::Boolean, false)),
        ]))
    }

    fn output_schema() -> SchemaRef {
        Arc::new(Schema::new(vec![
            Arc::new(Field::new("id", DataType::Int64, false)),
            Arc::new(Field::new("value", DataType::Utf8, true)),
        ]))
    }

    fn bindings() -> EffectiveViewFieldBindings {
        EffectiveViewFieldBindings::try_new(
            input_schema(),
            output_schema(),
            vec!["id"],
            "generation",
            vec![EffectiveViewOrderField::descending("operation_order")],
            "tombstone",
        )
        .expect("valid effective-view bindings")
    }

    fn batch(rows: &[Row<'_>]) -> RecordBatch {
        let schema = input_schema();
        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(Int64Array::from_iter_values(rows.iter().map(|row| row.0))),
                Arc::new(StringArray::from(
                    rows.iter().map(|row| row.1).collect::<Vec<_>>(),
                )),
                Arc::new(Int64Array::from_iter_values(rows.iter().map(|row| row.2))),
                Arc::new(Int64Array::from_iter_values(rows.iter().map(|row| row.3))),
                Arc::new(BooleanArray::from_iter(rows.iter().map(|row| Some(row.4)))),
            ],
        )
        .expect("test batch")
    }

    async fn registered_input(
        context: &SessionContext,
        name: &str,
        rows: &[Row<'_>],
    ) -> EffectiveViewInput {
        let batch = batch(rows);
        let provider =
            Arc::new(MemTable::try_new(batch.schema(), vec![vec![batch]]).expect("test MemTable"));
        context
            .register_table(name, provider)
            .expect("register exact input");
        let reference = datafusion::common::TableReference::bare(name);
        let plan = context
            .table(reference.clone())
            .await
            .expect("resolve exact input")
            .into_unoptimized_plan();
        EffectiveViewInput::new(reference, plan)
    }

    fn rows(execution: super::EffectiveViewExecution) -> Vec<(i64, Option<String>)> {
        let mut rows = Vec::new();
        for batch in execution.batches() {
            assert_eq!(batch.schema_ref().as_ref(), output_schema().as_ref());
            let ids = batch
                .column(0)
                .as_any()
                .downcast_ref::<Int64Array>()
                .expect("id column");
            let values = batch
                .column(1)
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("value column");
            for row in 0..batch.num_rows() {
                rows.push((
                    ids.value(row),
                    (!values.is_null(row)).then(|| values.value(row).to_owned()),
                ));
            }
        }
        rows.sort();
        rows
    }

    #[tokio::test]
    async fn insert_replace_and_delete_are_one_native_latest_row_semantics() {
        let context = SessionContext::new();
        let base = registered_input(
            &context,
            "base",
            &[
                // Base generations are deliberately larger than the overlay generations.
                // Segment presence, not an accidentally comparable storage-local generation,
                // is the authority for replacement and deletion.
                (1, Some("old"), 100, 0, false),
                (2, Some("delete-me"), 100, 0, false),
                (3, Some("unchanged"), 0, 0, false),
            ],
        )
        .await;
        let segment = registered_input(
            &context,
            "segment_1",
            &[
                (1, Some("replacement"), 1, 10, false),
                (2, None, 1, 11, true),
                (4, Some("inserted"), 1, 12, false),
            ],
        )
        .await;

        let compiled = compile_effective_view(base, vec![segment], &bindings()).expect("compile");
        let execution = compiled.execute_proved(&context).await.expect("execute");

        assert_eq!(execution.tie_proof().examined_ties(), 0);
        assert_eq!(
            rows(execution),
            [
                (1, Some("replacement".to_owned())),
                (3, Some("unchanged".to_owned())),
                (4, Some("inserted".to_owned())),
            ]
        );
    }

    #[tokio::test]
    async fn immutable_segment_argument_order_cannot_change_results() {
        let context = SessionContext::new();
        let base = registered_input(&context, "base", &[(1, Some("base"), 0, 0, false)]).await;
        let earlier = registered_input(
            &context,
            "segment_a",
            &[
                (1, Some("earlier"), 1, 5, false),
                (2, Some("new"), 1, 6, false),
            ],
        )
        .await;
        let later =
            registered_input(&context, "segment_b", &[(1, Some("latest"), 2, 1, false)]).await;

        let forward = compile_effective_view(
            base.clone(),
            vec![earlier.clone(), later.clone()],
            &bindings(),
        )
        .expect("forward compilation");
        let reverse = compile_effective_view(base, vec![later, earlier], &bindings())
            .expect("reverse compilation");

        let forward_rows = rows(forward.execute_proved(&context).await.expect("forward"));
        let reverse_rows = rows(reverse.execute_proved(&context).await.expect("reverse"));
        assert_eq!(forward_rows, reverse_rows);
        assert_eq!(
            forward_rows,
            [(1, Some("latest".to_owned())), (2, Some("new".to_owned())),]
        );
    }

    #[tokio::test]
    async fn duplicate_complete_order_key_is_rejected_not_arbitrarily_ranked() {
        let context = SessionContext::new();
        let base = registered_input(&context, "base", &[]).await;
        let left =
            registered_input(&context, "segment_left", &[(7, Some("left"), 4, 9, false)]).await;
        let right = registered_input(
            &context,
            "segment_right",
            &[(7, Some("right"), 4, 9, false)],
        )
        .await;
        let compiled =
            compile_effective_view(base, vec![left, right], &bindings()).expect("compile");

        let error = compiled
            .execute_proved(&context)
            .await
            .expect_err("tie must reject the effective relation");
        assert!(matches!(
            error,
            EffectiveViewError::NonDeterministicTie {
                conflicting_rows: 2
            }
        ));
    }

    #[tokio::test]
    async fn empty_base_and_segments_preserve_the_declared_empty_schema() {
        let context = SessionContext::new();
        let base = registered_input(&context, "base", &[]).await;
        let empty_segment = registered_input(&context, "empty_segment", &[]).await;
        let with_segment = compile_effective_view(base.clone(), vec![empty_segment], &bindings())
            .expect("compile empty segment")
            .execute_proved(&context)
            .await
            .expect("execute empty segment");
        let without_segments = compile_effective_view(base, vec![], &bindings())
            .expect("compile no segments")
            .execute_proved(&context)
            .await
            .expect("execute no segments");

        for execution in [with_segment, without_segments] {
            assert_eq!(execution.schema().as_ref(), output_schema().as_ref());
            assert!(rows(execution).is_empty());
        }
    }

    #[tokio::test]
    async fn optimized_plan_remains_visible_as_native_datafusion_operators() {
        let context = SessionContext::new();
        let base = registered_input(&context, "base", &[(1, Some("base"), 0, 0, false)]).await;
        let segment = registered_input(&context, "segment", &[(1, Some("new"), 1, 1, false)]).await;
        let compiled = compile_effective_view(base, vec![segment], &bindings()).expect("compile");
        let optimized = context
            .state()
            .optimize(compiled.effective_plan())
            .expect("optimize native view");

        let mut kinds = BTreeSet::new();
        let mut native_shape = OptimizedNativeShape::default();
        observe_plan_kinds(&optimized, &mut kinds, &mut native_shape);
        assert!(kinds.contains("Union"));
        assert!(kinds.contains("Join"));
        assert!(kinds.contains("Window"));
        assert!(kinds.contains("Filter"));
        assert!(kinds.contains("Projection"));
        assert!(!kinds.contains("Extension"));
        assert_eq!(native_shape.union_all, 1);
        assert_eq!(native_shape.left_anti_joins, 2);

        assert_eq!(
            compiled.observation().rung(),
            EffectiveViewExtensionRung::NativeLogicalPlan
        );
        assert_eq!(compiled.observation().inputs().len(), 2);
        assert!(
            compiled
                .observation()
                .operators()
                .contains(&EffectiveViewNativeOperator::UnionAll)
        );
        assert!(
            compiled
                .observation()
                .operators()
                .contains(&EffectiveViewNativeOperator::ReplacementAntiJoin)
        );
        assert!(
            compiled
                .observation()
                .operators()
                .contains(&EffectiveViewNativeOperator::TombstoneAntiJoin)
        );
    }

    #[test]
    fn invalid_bindings_fail_before_plan_construction() {
        let empty_key = EffectiveViewFieldBindings::try_new(
            input_schema(),
            output_schema(),
            Vec::<&str>::new(),
            "generation",
            vec![EffectiveViewOrderField::descending("operation_order")],
            "tombstone",
        )
        .expect_err("empty key");
        assert!(matches!(empty_key, EffectiveViewError::EmptyPrimaryKey));

        let empty_order = EffectiveViewFieldBindings::try_new(
            input_schema(),
            output_schema(),
            vec!["id"],
            "generation",
            vec![],
            "tombstone",
        )
        .expect_err("empty deterministic order");
        assert!(matches!(
            empty_order,
            EffectiveViewError::EmptyDeterministicOrder
        ));

        let nullable_key_schema = Arc::new(Schema::new(vec![
            Arc::new(Field::new("id", DataType::Int64, true)),
            Arc::new(Field::new("value", DataType::Utf8, true)),
            Arc::new(Field::new("generation", DataType::Int64, false)),
            Arc::new(Field::new("operation_order", DataType::Int64, false)),
            Arc::new(Field::new("tombstone", DataType::Boolean, false)),
        ]));
        let nullable_key = EffectiveViewFieldBindings::try_new(
            nullable_key_schema,
            output_schema(),
            vec!["id"],
            "generation",
            vec![EffectiveViewOrderField::descending("operation_order")],
            "tombstone",
        )
        .expect_err("nullable key");
        assert!(matches!(
            nullable_key,
            EffectiveViewError::NullableBoundField(field) if field == "id"
        ));
    }

    #[derive(Default)]
    struct OptimizedNativeShape {
        union_all: usize,
        left_anti_joins: usize,
    }

    fn observe_plan_kinds(
        plan: &LogicalPlan,
        kinds: &mut BTreeSet<&'static str>,
        shape: &mut OptimizedNativeShape,
    ) {
        let kind = match plan {
            LogicalPlan::Projection(_) => "Projection",
            LogicalPlan::Filter(_) => "Filter",
            LogicalPlan::Window(_) => "Window",
            LogicalPlan::Union(_) => {
                shape.union_all += 1;
                "Union"
            }
            LogicalPlan::Join(join) => {
                if join.join_type == JoinType::LeftAnti {
                    shape.left_anti_joins += 1;
                }
                "Join"
            }
            LogicalPlan::Extension(_) => "Extension",
            LogicalPlan::TableScan(_) => "TableScan",
            _ => "Other",
        };
        kinds.insert(kind);
        for input in plan.inputs() {
            observe_plan_kinds(input, kinds, shape);
        }
    }

    #[allow(dead_code)]
    fn _assert_compiled_is_send_sync(_: &CompiledEffectiveView) {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CompiledEffectiveView>();
    }
}
