//! Associated member coverage follows the selected class census and its own type gaps.

use super::{
    INPUT, JoinType, LogicalPlan, LogicalPlanBuilder, TransformationInputs,
    TransformationPlanError, col, count_when, lit, output_fields, plan,
};
use datafusion::logical_expr::when;

pub(super) fn build(inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
    from_plans(
        plan(inputs, INPUT)?,
        plan(inputs, super::super::types::MEMBER)?,
    )
}

fn from_plans(
    requested: LogicalPlan,
    observations: LogicalPlan,
) -> Result<LogicalPlan, TransformationPlanError> {
    let scope = ["workspace_id", "context_id", "file_id", "source_generation"];
    let owners = LogicalPlanBuilder::from(observations)
        .filter(col("language").eq(lit("python")))?
        .filter(col("owner_entity_id").is_not_null())?
        .aggregate(
            scope
                .into_iter()
                .chain(["owner_entity_id"])
                .map(col)
                .collect::<Vec<_>>(),
            vec![
                count_when(col("record_kind").eq(lit("class")))?.alias("census_rows"),
                count_when(
                    col("unknown_reason").is_not_null().or(col("record_kind")
                        .eq(lit("class"))
                        .and(col("class_census_complete").eq(lit(false)))),
                )?
                .alias("gaps"),
            ],
        )?
        .alias("m")?
        .build()?;
    let selected = col("b.processing_state").in_list(vec![lit("complete"), lit("partial")], false);
    let incomplete = col("m.census_rows")
        .not_eq(lit(1_i64))
        .or(col("m.gaps").gt(lit(0_i64)));
    let state = when(selected.clone().and(incomplete.clone()), lit("partial"))
        .when(selected.clone(), lit("complete"))
        .otherwise(col("b.processing_state"))?;
    let reason = when(
        selected.clone().and(incomplete),
        lit("associated_member_scope_incomplete"),
    )
    .when(selected, lit(""))
    .otherwise(col("b.reason"))?;
    Ok(LogicalPlanBuilder::from(requested)
        .filter(
            col("family")
                .eq(lit("members"))
                .and(col("language").eq(lit("python"))),
        )?
        .alias("b")?
        .join(
            owners,
            JoinType::Inner,
            (
                scope.map(|name| format!("b.{name}")).to_vec(),
                scope.map(|name| format!("m.{name}")).to_vec(),
            ),
            None,
        )?
        .project(output_fields().into_iter().map(|(name, _, _)| {
            match name {
                "scope_kind" => lit("member_owner"),
                "owner_entity_id" => col("m.owner_entity_id"),
                "processing_state" => state.clone(),
                "reason" => reason.clone(),
                _ => col(format!("b.{name}")),
            }
            .alias(name)
        }))?
        .distinct()?
        .build()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{ArrayRef, BooleanArray, RecordBatch, StringArray};
    use datafusion::prelude::SessionContext;
    use std::{collections::BTreeMap, sync::Arc};

    #[tokio::test]
    #[allow(
        clippy::too_many_lines,
        reason = "explicit independent native rows exercise census absence, local gaps, pending and three pin mismatches"
    )]
    async fn member_owner_scope_isolates_gaps_pins_and_pending_work() {
        let context = SessionContext::new();
        let requested = super::super::fields().into_iter().map(|(name, _, _)| {
            let values = match name {
                "workspace_id" => vec!["w"; 3],
                "context_id" => vec!["c"; 3],
                "file_id" => vec!["f", "p", "absent"],
                "source_generation" => vec!["1"; 3],
                "language" => vec!["python"; 3],
                "family" => vec!["members"; 3],
                "processing_state" => vec!["partial", "pending", "unknown"],
                "reason" => vec!["unrelated_type_gap", "semantic_work_pending", "missing"],
                _ => vec![""; 3],
            };
            (name, Arc::new(StringArray::from(values)) as ArrayRef)
        });
        context
            .register_batch("requested", RecordBatch::try_from_iter(requested).unwrap())
            .unwrap();
        let strings: &[(&str, &[&str])] = &[
            ("workspace_id", &["w", "w", "w", "wrong", "w", "w", "w"]),
            ("context_id", &["c", "c", "c", "c", "wrong", "c", "c"]),
            ("file_id", &["f", "f", "p", "f", "f", "f", "f"]),
            ("source_generation", &["1", "1", "1", "1", "1", "2", "1"]),
            (
                "owner_entity_id",
                &[
                    "good",
                    "bad",
                    "pending",
                    "good",
                    "good",
                    "good",
                    "no-census",
                ],
            ),
            ("language", &["python"; 7]),
            (
                "record_kind",
                &[
                    "class", "class", "class", "class", "class", "class", "member",
                ],
            ),
        ];
        let mut columns = strings
            .iter()
            .map(|(name, values)| {
                (
                    *name,
                    Arc::new(StringArray::from(values.to_vec())) as ArrayRef,
                )
            })
            .collect::<Vec<_>>();
        columns.extend([
            (
                "unknown_reason",
                Arc::new(StringArray::from(vec![
                    None,
                    Some("unknown_type"),
                    None,
                    Some("wrong_workspace"),
                    Some("wrong_context"),
                    Some("stale_generation"),
                    None,
                ])) as ArrayRef,
            ),
            (
                "class_census_complete",
                Arc::new(BooleanArray::from(vec![
                    true, true, false, false, false, false, true,
                ])) as ArrayRef,
            ),
        ]);
        context
            .register_batch("observations", RecordBatch::try_from_iter(columns).unwrap())
            .unwrap();
        let result = from_plans(
            context
                .table("requested")
                .await
                .unwrap()
                .into_optimized_plan()
                .unwrap(),
            context
                .table("observations")
                .await
                .unwrap()
                .into_optimized_plan()
                .unwrap(),
        )
        .unwrap();
        let batches = context
            .execute_logical_plan(result)
            .await
            .unwrap()
            .collect()
            .await
            .unwrap();
        let mut actual = BTreeMap::new();
        for batch in batches {
            let values = |name| {
                batch
                    .column_by_name(name)
                    .unwrap()
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap()
            };
            for row in 0..batch.num_rows() {
                assert_eq!(values("scope_kind").value(row), "member_owner");
                actual.insert(
                    values("owner_entity_id").value(row).to_owned(),
                    (
                        values("processing_state").value(row).to_owned(),
                        values("reason").value(row).to_owned(),
                    ),
                );
            }
        }
        let pair = |state: &str, reason: &str| (state.to_owned(), reason.to_owned());
        assert_eq!(
            actual,
            BTreeMap::from([
                ("good".to_owned(), pair("complete", "")),
                (
                    "bad".to_owned(),
                    pair("partial", "associated_member_scope_incomplete")
                ),
                (
                    "pending".to_owned(),
                    pair("pending", "semantic_work_pending")
                ),
                (
                    "no-census".to_owned(),
                    pair("partial", "associated_member_scope_incomplete")
                ),
            ])
        );
    }
}
