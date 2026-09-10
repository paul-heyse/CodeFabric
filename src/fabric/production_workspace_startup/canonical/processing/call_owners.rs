//! Outgoing Rust call coverage requires a selected owner and an admitted MIR body.

use super::super::{RustcRelation, SOURCE, file_id_udf, rust_entity_id};
use super::{
    Expr, INPUT, JoinType, LogicalPlan, LogicalPlanBuilder, RUN, TransformationInputs,
    TransformationPlanError, coalesce, col, count, count_when, lit, output_fields, plan,
};

pub(super) fn build(
    inputs: &TransformationInputs,
    workspace: [u8; 16],
) -> Result<LogicalPlan, TransformationPlanError> {
    from_plans(
        plan(inputs, INPUT)?,
        plan(inputs, super::super::DECLARATION)?,
        plan(inputs, SOURCE)?,
        admitted_bodies(inputs, workspace)?,
        plan(inputs, super::super::calls::RELATION)?,
    )
}

fn admitted_bodies(
    inputs: &TransformationInputs,
    workspace: [u8; 16],
) -> Result<LogicalPlan, TransformationPlanError> {
    let bodies = LogicalPlanBuilder::from(plan(inputs, RustcRelation::MirBody.relation_id())?)
        .alias("m")?
        .build()?;
    let owners = LogicalPlanBuilder::from(plan(inputs, RustcRelation::PublicItem.relation_id())?)
        .alias("o")?
        .build()?;
    let runs = LogicalPlanBuilder::from(plan(inputs, RUN)?)
        .filter(col("provider").eq(lit("rustc")))?
        .alias("r")?
        .build()?;
    Ok(LogicalPlanBuilder::from(bodies)
        .join(
            owners,
            JoinType::Inner,
            (
                vec![
                    "m.provider_run_id",
                    "m.compilation_unit_id",
                    "m.owner_id",
                    "m.source_generation",
                    "m.source_file_id",
                    "m.source_content_digest",
                ],
                vec![
                    "o.provider_run_id",
                    "o.compilation_unit_id",
                    "o.owner_id",
                    "o.source_generation",
                    "o.source_file_id",
                    "o.source_content_digest",
                ],
            ),
            None,
        )?
        .join(
            runs,
            JoinType::Inner,
            (
                vec!["m.provider_run_id", "m.source_generation"],
                vec!["r.provider_run_identity", "r.source_generation"],
            ),
            None,
        )?
        .project([
            rust_entity_id(workspace)
                .call(vec![
                    col("r.context_id"),
                    col("o.stable_crate_id"),
                    col("o.def_path_hash"),
                    col("o.item_kind"),
                ])
                .alias("entity_id"),
            col("r.context_id"),
            col("m.source_generation"),
            file_id_udf()
                .call(vec![col("m.source_file_id")])
                .alias("file_id"),
            col("m.source_content_digest").alias("content_digest"),
        ])?
        .distinct()?
        .build()?)
}

#[allow(
    clippy::too_many_lines,
    reason = "keep the owner, MIR and missing-call scope joins together"
)]
fn from_plans(
    requested: LogicalPlan,
    declarations: LogicalPlan,
    sources: LogicalPlan,
    bodies: LogicalPlan,
    calls: LogicalPlan,
) -> Result<LogicalPlan, TransformationPlanError> {
    let base = LogicalPlanBuilder::from(requested)
        .filter(
            col("family")
                .eq(lit("call-targets"))
                .and(col("language").eq(lit("rust"))),
        )?
        .alias("b")?
        .build()?;
    let declarations = LogicalPlanBuilder::from(declarations)
        .filter(
            col("language")
                .eq(lit("rust"))
                .and(col("entity_kind").eq(lit("function")))
                .and(col("entity_id").is_not_null()),
        )?
        .project([
            col("entity_id"),
            col("context_id"),
            col("source_generation"),
            col("file_id"),
            col("content_digest"),
        ])?
        .distinct()?
        .alias("d")?
        .build()?;
    let bodies = LogicalPlanBuilder::from(bodies).alias("m")?.build()?;
    let sources = LogicalPlanBuilder::from(sources).alias("s")?.build()?;
    let calls = LogicalPlanBuilder::from(calls)
        .filter(col("language").eq(lit("rust")))?
        .build()?;
    let gaps = LogicalPlanBuilder::from(calls.clone())
        .aggregate(
            vec![
                col("context_id"),
                col("source_generation"),
                col("caller_entity_id"),
            ],
            vec![
                count_when(col("target_entity_id").is_null())?.alias("targets"),
                count_when(col("call_site_id").is_null())?.alias("locations"),
            ],
        )?
        .alias("g")?
        .build()?;
    // An unowned call cannot be assigned to a different caller safely.
    let unowned = LogicalPlanBuilder::from(calls)
        .filter(col("caller_entity_id").is_null())?
        .aggregate(
            vec![col("context_id"), col("source_generation")],
            vec![count(lit(1_i64)).alias("count")],
        )?
        .alias("u")?
        .build()?;
    let joined = LogicalPlanBuilder::from(base)
        .join(
            declarations,
            JoinType::Inner,
            (
                vec!["b.context_id", "b.source_generation"],
                vec!["d.context_id", "d.source_generation"],
            ),
            None,
        )?
        .join(
            bodies,
            JoinType::Left,
            (
                vec![
                    "d.entity_id",
                    "d.context_id",
                    "d.source_generation",
                    "d.file_id",
                    "d.content_digest",
                ],
                vec![
                    "m.entity_id",
                    "m.context_id",
                    "m.source_generation",
                    "m.file_id",
                    "m.content_digest",
                ],
            ),
            None,
        )?
        .join(
            sources,
            JoinType::Inner,
            (
                vec!["d.file_id", "d.source_generation", "d.content_digest"],
                vec!["s.file_id", "s.source_generation", "s.content_digest"],
            ),
            None,
        )?
        .join(
            gaps,
            JoinType::Left,
            (
                vec!["d.entity_id", "d.context_id", "d.source_generation"],
                vec!["g.caller_entity_id", "g.context_id", "g.source_generation"],
            ),
            None,
        )?
        .join(
            unowned,
            JoinType::Left,
            (
                vec!["d.context_id", "d.source_generation"],
                vec!["u.context_id", "u.source_generation"],
            ),
            None,
        )?;
    let positive = |name| coalesce(vec![col(name), lit(0_i64)]).gt(lit(0_i64));
    let conditions = [
        col("m.entity_id").is_null(),
        positive("g.targets"),
        positive("g.locations"),
        positive("u.count"),
    ];
    let incomplete = conditions
        .iter()
        .cloned()
        .reduce(Expr::or)
        .expect("nonempty gap classes");
    let mut fragments = vec![lit("call_semantics_incomplete")];
    for (condition, reason) in conditions.into_iter().zip([
        ";caller_mir_unavailable",
        ";unresolved_targets",
        ";source_locations",
        ";caller_entities",
    ]) {
        fragments.push(datafusion::logical_expr::when(condition, lit(reason)).otherwise(lit(""))?);
    }
    let reason = datafusion::functions::string::expr_fn::concat(fragments);
    let complete = col("b.processing_state").eq(lit("complete"));
    Ok(joined
        .project(
            output_fields()
                .iter()
                .map(|(name, _, _)| {
                    Ok(match *name {
                        "scope_kind" => lit("call_owner"),
                        "owner_entity_id" => col("d.entity_id"),
                        "file_id" => col("d.file_id"),
                        "relative_path" => col("s.relative_path"),
                        "processing_state" => datafusion::logical_expr::when(
                            complete.clone().and(incomplete.clone()),
                            lit("partial"),
                        )
                        .otherwise(col("b.processing_state"))?,
                        "reason" => datafusion::logical_expr::when(
                            complete.clone().and(incomplete.clone()),
                            reason.clone(),
                        )
                        .otherwise(col("b.reason"))?,
                        _ => col(format!("b.{name}")),
                    }
                    .alias(*name))
                })
                .collect::<Result<Vec<_>, datafusion::common::DataFusionError>>()?,
        )?
        .distinct()?
        .build()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{RecordBatch, StringArray};
    use datafusion::prelude::SessionContext;
    use futures::FutureExt as _;
    use std::{collections::BTreeMap, sync::Arc};

    fn table(context: &SessionContext, name: &str, columns: &[(&str, &[&str])]) -> LogicalPlan {
        let batch = RecordBatch::try_from_iter(columns.iter().map(|(name, values)| {
            (
                *name,
                Arc::new(StringArray::from_iter(
                    values
                        .iter()
                        .map(|value| (*value != "NULL").then_some(*value)),
                )) as arrow_array::ArrayRef,
            )
        }))
        .unwrap();
        context.register_batch(name, batch).unwrap();
        context
            .read_table(
                context
                    .table_provider(name)
                    .now_or_never()
                    .unwrap()
                    .unwrap(),
            )
            .unwrap()
            .logical_plan()
            .clone()
    }

    async fn observations(unowned: bool) -> BTreeMap<String, (String, String)> {
        let context = SessionContext::new();
        let requested = table(
            &context,
            "requested",
            &[
                ("workspace_id", &["workspace"; 2]),
                ("source_generation", &["1"; 2]),
                ("input_set_id", &["inputs"; 2]),
                ("language", &["rust"; 2]),
                ("scope_kind", &["cargo_target"; 2]),
                ("relative_path", &["Cargo.toml"; 2]),
                ("target_name", &["lib"; 2]),
                ("target_kind", &["library"; 2]),
                ("target_platform", &["test-platform"; 2]),
                ("build_profile", &["dev"; 2]),
                ("build_features", &[""; 2]),
                ("default_features", &["true"; 2]),
                ("context_id", &["A", "B"]),
                ("file_id", &["NULL"; 2]),
                ("family", &["call-targets"; 2]),
                ("processing_state", &["complete", "pending"]),
                ("reason", &["", "semantic_work_pending"]),
            ],
        );
        let declarations = table(
            &context,
            "declarations",
            &[
                ("entity_id", &["a", "b", "c", "d", "e"]),
                ("context_id", &["A", "A", "A", "A", "B"]),
                ("entity_kind", &["function"; 5]),
                ("language", &["rust"; 5]),
                ("source_generation", &["1"; 5]),
                ("file_id", &["lib"; 5]),
                ("content_digest", &["current"; 5]),
            ],
        );
        let sources = table(
            &context,
            "sources",
            &[
                ("file_id", &["lib"]),
                ("source_generation", &["1"]),
                ("content_digest", &["current"]),
                ("relative_path", &["src/lib.rs"]),
            ],
        );
        let bodies = table(
            &context,
            "bodies",
            &[
                ("entity_id", &["a", "b", "c", "d", "e"]),
                ("context_id", &["A", "A", "A", "A", "B"]),
                ("source_generation", &["1"; 5]),
                ("file_id", &["lib"; 5]),
                (
                    "content_digest",
                    &["current", "current", "stale", "current", "current"],
                ),
            ],
        );
        let calls = table(
            &context,
            "calls",
            &[
                ("language", &["rust"; 3]),
                (
                    "context_id",
                    &["A", "A", if unowned { "A" } else { "other" }],
                ),
                ("source_generation", &["1"; 3]),
                ("caller_entity_id", &["a", "b", "NULL"]),
                ("target_entity_id", &["d", "NULL", "NULL"]),
                ("call_site_id", &["a-call", "b-call", "NULL"]),
            ],
        );
        let selected = from_plans(requested, declarations, sources, bodies, calls).unwrap();
        let batches = context
            .execute_logical_plan(selected)
            .await
            .unwrap()
            .collect()
            .await
            .unwrap();
        let mut result = BTreeMap::new();
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
                result.insert(
                    values("owner_entity_id").value(row).to_owned(),
                    (
                        values("processing_state").value(row).to_owned(),
                        values("reason").value(row).to_owned(),
                    ),
                );
            }
        }
        result
    }

    #[tokio::test]
    async fn outgoing_call_owners_require_mir_and_keep_only_relevant_gaps() {
        assert_eq!(
            observations(false).await,
            BTreeMap::from([
                ("a".into(), ("complete".into(), "".into())),
                (
                    "b".into(),
                    (
                        "partial".into(),
                        "call_semantics_incomplete;unresolved_targets".into()
                    )
                ),
                (
                    "c".into(),
                    (
                        "partial".into(),
                        "call_semantics_incomplete;caller_mir_unavailable".into()
                    )
                ),
                ("d".into(), ("complete".into(), "".into())),
                (
                    "e".into(),
                    ("pending".into(), "semantic_work_pending".into())
                ),
            ])
        );
        let unknown = observations(true).await;
        for owner in ["a", "b", "c", "d"] {
            assert_eq!(unknown[owner].0, "partial");
            assert!(unknown[owner].1.contains("caller_entities"));
        }
        assert_eq!(
            unknown["e"],
            ("pending".into(), "semantic_work_pending".into())
        );
    }
}
