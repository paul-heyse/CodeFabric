//! Entity discovery keeps a deterministic exact source anchor for semantic ordering.

use super::{
    DataType, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder, TransformationInputs,
    TransformationPlanError, col, entity_fields, lit, plan, public_entity_id,
};

const KEYS: [&str; 4] = ["entity_id", "context_id", "file_id", "workspace_id"];
const ANCHOR: [&str; 3] = ["relative_path", "start_byte", "end_byte"];

pub(super) fn fields() -> Vec<FieldSpec> {
    let mut fields = entity_fields();
    fields.extend([
        ("selector", DataType::Utf8, false),
        ("public_entity_id", DataType::Utf8, true),
        ("relative_path", DataType::Binary, true),
        ("start_byte", DataType::UInt64, true),
        ("end_byte", DataType::UInt64, true),
    ]);
    fields
}

pub(super) fn build(inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
    let locations =
        LogicalPlanBuilder::from(first_anchor(plan(inputs, super::locations::RELATION)?)?)
            .alias("l")?
            .build()?;
    let input = LogicalPlanBuilder::from(plan(inputs, super::ENTITY)?)
        .alias("e")?
        .join(
            locations,
            JoinType::Left,
            (
                KEYS.map(|name| format!("e.{name}")).to_vec(),
                KEYS.map(|name| format!("l.{name}")).to_vec(),
            ),
            None,
        )?
        .build()?;
    let project =
        |selector: datafusion::logical_expr::Expr| -> Result<LogicalPlan, TransformationPlanError> {
            Ok(LogicalPlanBuilder::from(input.clone())
                .project(
                    entity_fields()
                        .iter()
                        .map(|(name, _, _)| col(format!("e.{name}")).alias(*name))
                        .chain([
                            selector.alias("selector"),
                            public_entity_id()
                                .call(vec![col("e.entity_id"), col("e.entity_kind")])
                                .alias("public_entity_id"),
                        ])
                        .chain(ANCHOR.map(|name| col(format!("l.{name}")).alias(name))),
                )?
                .build()?)
        };
    Ok(LogicalPlanBuilder::from(project(col("e.entity_kind"))?)
        .union(project(datafusion::functions::string::expr_fn::concat(
            vec![col("e.language"), lit(":"), col("e.entity_kind")],
        ))?)?
        .build()?)
}

fn first_anchor(input: LogicalPlan) -> datafusion::common::Result<LogicalPlan> {
    // Select one whole anchor. Independent MINs can manufacture a range that never existed.
    LogicalPlanBuilder::from(input)
        .distinct_on(
            KEYS.into_iter().map(col).collect(),
            KEYS.into_iter().chain(ANCHOR).map(col).collect(),
            Some(
                KEYS.into_iter()
                    .chain(ANCHOR)
                    .map(|name| col(name).sort(true, false))
                    .collect(),
            ),
        )?
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{ArrayRef, BinaryArray, RecordBatch, UInt64Array};
    use datafusion::prelude::SessionContext;
    use std::sync::Arc;

    #[tokio::test]
    async fn first_source_anchor_preserves_whole_spans_and_contexts() {
        let numbers = |values: Vec<u64>| Arc::new(UInt64Array::from(values)) as ArrayRef;
        let input = RecordBatch::try_from_iter([
            ("entity_id", numbers(vec![1, 1, 1])),
            ("context_id", numbers(vec![1, 2, 1])),
            ("file_id", numbers(vec![1, 1, 1])),
            ("workspace_id", numbers(vec![1, 1, 1])),
            (
                "relative_path",
                Arc::new(BinaryArray::from_vec(vec![b"a.py"; 3])) as ArrayRef,
            ),
            ("start_byte", numbers(vec![30, 20, 10])),
            ("end_byte", numbers(vec![60, 40, 90])),
        ])
        .unwrap();
        let context = SessionContext::new();
        let plan = first_anchor(
            context
                .read_batch(input)
                .unwrap()
                .into_optimized_plan()
                .unwrap(),
        )
        .unwrap();
        let batches = context
            .execute_logical_plan(plan)
            .await
            .unwrap()
            .sort(vec![col("context_id").sort(true, false)])
            .unwrap()
            .collect()
            .await
            .unwrap();
        let rows = batches
            .iter()
            .flat_map(|batch| {
                let value = |name, row| {
                    batch
                        .column_by_name(name)
                        .unwrap()
                        .as_any()
                        .downcast_ref::<UInt64Array>()
                        .unwrap()
                        .value(row)
                };
                (0..batch.num_rows()).map(move |row| {
                    (
                        value("context_id", row),
                        value("start_byte", row),
                        value("end_byte", row),
                    )
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(rows, [(1, 10, 90), (2, 20, 40)]);
    }
}
