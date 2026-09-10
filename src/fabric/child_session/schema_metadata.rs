//! Preserve the compiled result envelope after native branch elimination.

use std::sync::Arc;

use arrow_schema::Schema;
use datafusion::common::{DFSchema, DataFusionError, Result};
use datafusion::logical_expr::{Expr, LogicalPlan, Projection};

pub(super) fn preserve_result_metadata(
    plan: LogicalPlan,
    expected: &Schema,
) -> Result<(LogicalPlan, bool)> {
    if plan.schema().fields() != expected.fields() {
        return Err(DataFusionError::Plan(
            "optimized output fields differ from the compiled result contract".into(),
        ));
    }
    if plan.schema().metadata() == expected.metadata() {
        return Ok((plan, false));
    }
    // UNION intersects metadata. Pruning an empty branch may expose additional input metadata.
    // Keep the exact fields/qualifiers and native dependencies, restoring only the compiled
    // schema-level envelope. Native physical projection still derives types and nullability.
    let schema = DFSchema::new_with_metadata(
        plan.schema()
            .iter()
            .map(|(qualifier, field)| (qualifier.cloned(), field.clone()))
            .collect(),
        expected.metadata().clone(),
    )?
    .with_functional_dependencies(plan.schema().functional_dependencies().clone())?;
    let expressions = plan
        .schema()
        .columns()
        .into_iter()
        .map(Expr::Column)
        .collect();
    Ok((
        LogicalPlan::Projection(Projection::try_new_with_schema(
            expressions,
            Arc::new(plan),
            Arc::new(schema),
        )?),
        true,
    ))
}

pub(super) fn preserve_physical_metadata(
    plan: Arc<dyn datafusion::physical_plan::ExecutionPlan>,
    expected: &Schema,
) -> Result<Arc<dyn datafusion::physical_plan::ExecutionPlan>> {
    use datafusion::physical_expr::PhysicalExpr;
    use datafusion::physical_expr::expressions::Column;
    use datafusion::physical_plan::projection::ProjectionExec;
    if plan.schema().fields() != expected.fields() {
        return Err(DataFusionError::Plan(
            "physical output fields differ from the compiled result contract".into(),
        ));
    }
    let expressions = expected
        .fields()
        .iter()
        .enumerate()
        .map(|(index, field)| {
            (
                Arc::new(Column::new(field.name(), index)) as Arc<dyn PhysicalExpr>,
                field.name().clone(),
            )
        })
        .collect::<Vec<_>>();
    Ok(Arc::new(ProjectionExec::try_new_with_schema_metadata(
        expressions,
        plan,
        expected,
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{Int64Array, RecordBatch};
    use arrow_schema::{DataType, Field};
    use datafusion::logical_expr::LogicalPlanBuilder;
    use datafusion::prelude::{SessionContext, lit};
    use std::collections::HashMap;

    #[tokio::test]
    async fn empty_union_branch_preserves_metadata_without_changing_native_fields_or_rows() {
        let context = SessionContext::new();
        let batch = |origin: &str, value| {
            RecordBatch::try_new(
                Arc::new(Schema::new_with_metadata(
                    vec![Field::new("value", DataType::Int64, false)],
                    HashMap::from([("origin".into(), origin.into())]),
                )),
                vec![Arc::new(Int64Array::from(vec![value]))],
            )
            .unwrap()
        };
        let left = context
            .read_batch(batch("left", 7))
            .unwrap()
            .into_unoptimized_plan();
        let right = context
            .read_batch(batch("right", 99))
            .unwrap()
            .filter(lit(false))
            .unwrap()
            .into_unoptimized_plan();
        let compiled = LogicalPlanBuilder::from(left)
            .union_distinct(right)
            .unwrap()
            .build()
            .unwrap();
        let expected = compiled.schema().as_arrow().clone();
        let optimized = context.state().optimize(&compiled).unwrap();
        assert_ne!(
            optimized.schema().metadata(),
            expected.metadata(),
            "exercise actual native branch elimination"
        );
        let (optimized, deferred) = preserve_result_metadata(optimized, &expected).unwrap();
        assert!(deferred);
        assert_eq!(optimized.schema().as_arrow(), &expected);
        let state = context.state();
        let LogicalPlan::Projection(projection) = &optimized else {
            panic!("metadata projection");
        };
        let physical = state
            .query_planner()
            .create_physical_plan(&projection.input, &state)
            .await
            .unwrap();
        let physical = preserve_physical_metadata(physical, &expected).unwrap();
        assert_eq!(physical.schema().as_ref(), &expected);
        let batches = datafusion::physical_plan::collect(physical, context.task_ctx())
            .await
            .unwrap();
        assert_eq!(batches.iter().map(RecordBatch::num_rows).sum::<usize>(), 1);
        assert_eq!(
            batches[0]
                .column(0)
                .as_any()
                .downcast_ref::<Int64Array>()
                .unwrap()
                .value(0),
            7
        );
        let changed = Schema::new(vec![Field::new("value", DataType::Int64, true)]);
        assert!(
            preserve_result_metadata(optimized, &changed).is_err(),
            "nullability cannot be relabeled"
        );
    }
}
