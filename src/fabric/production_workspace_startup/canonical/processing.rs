//! Query processing partitions retain terminal provider state and semantic gaps separately.

use super::{
    DataType, Expr, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder, NativeSyntaxRelation,
    RUN, ScalarValue, TransformationInputs, TransformationPlanError, col, lit, plan,
};
use datafusion::functions::core::expr_fn::coalesce;
use datafusion::functions_aggregate::count::count;

pub(super) const INPUT: &str = "system.requested_processing_scope";
pub(super) const OUTPUT: &str = "system.entity_processing_scope";

pub(super) fn fields() -> Vec<FieldSpec> {
    vec![
        ("workspace_id", DataType::FixedSizeBinary(16), false),
        ("source_generation", DataType::UInt64, false),
        ("input_set_id", DataType::FixedSizeBinary(32), false),
        ("language", DataType::Utf8, false),
        ("scope_kind", DataType::Utf8, false),
        ("relative_path", DataType::Binary, false),
        ("target_name", DataType::Utf8, true),
        ("context_id", DataType::FixedSizeBinary(16), true),
        ("target_kind", DataType::Utf8, true),
        ("file_id", DataType::FixedSizeBinary(16), true),
        ("family", DataType::Utf8, false),
        ("processing_state", DataType::Utf8, false),
        ("reason", DataType::Utf8, false),
    ]
}

pub(super) fn dependencies(pyrefly: bool) -> Vec<&'static str> {
    let mut result = vec![INPUT, super::calls::RELATION, super::REFERENCE];
    if pyrefly {
        result.extend([
            RUN,
            crate::pyrefly_service::PyreflyRelation::CallTarget.relation_id(),
            NativeSyntaxRelation::RuffCallableSyntax.as_str(),
        ]);
    }
    result
}

fn scope_key() -> Result<Expr, datafusion::common::DataFusionError> {
    datafusion::logical_expr::when(col("language").eq(lit("python")), col("file_id"))
        .otherwise(col("context_id"))
}

fn count_when(condition: Expr) -> Result<Expr, datafusion::common::DataFusionError> {
    Ok(count(
        datafusion::logical_expr::when(condition, lit(1_i64))
            .otherwise(lit(ScalarValue::Int64(None)))?,
    ))
}

#[allow(
    clippy::too_many_lines,
    reason = "native partition aggregation and projection stay together"
)]
pub(super) fn build(
    inputs: &TransformationInputs,
    pyrefly: bool,
) -> Result<LogicalPlan, TransformationPlanError> {
    let calls = plan(inputs, super::calls::RELATION)?;
    let observations = LogicalPlanBuilder::from(calls)
        .aggregate(
            vec![
                col("language"),
                col("context_id"),
                scope_key()?.alias("scope_key"),
                col("source_generation"),
            ],
            vec![
                count_when(col("target_entity_id").is_null())?.alias("targets"),
                count_when(col("call_site_id").is_null())?.alias("locations"),
                count_when(col("caller_entity_id").is_null())?.alias("callers"),
            ],
        )?
        .alias("g")?
        .build()?;
    let base = qualify_lexical_references(inputs)?;
    let fields = fields();
    let mut joined = LogicalPlanBuilder::from(base)
        .project(
            fields
                .iter()
                .map(|(name, _, _)| col(*name))
                .chain([scope_key()?.alias("scope_key")]),
        )?
        .alias("b")?
        .join(
            observations,
            JoinType::Left,
            (
                vec![
                    "b.language",
                    "b.context_id",
                    "b.scope_key",
                    "b.source_generation",
                ],
                vec![
                    "g.language",
                    "g.context_id",
                    "g.scope_key",
                    "g.source_generation",
                ],
            ),
            None,
        )?;
    if pyrefly {
        joined = joined.join(
            unmapped_python(inputs)?,
            JoinType::Left,
            (
                vec![
                    "b.language",
                    "b.context_id",
                    "b.scope_key",
                    "b.source_generation",
                ],
                vec![
                    "m.language",
                    "m.context_id",
                    "m.scope_key",
                    "m.source_generation",
                ],
            ),
            None,
        )?;
    }
    let present = |name: &str| coalesce(vec![col(name), lit(0_i64)]).gt(lit(0_i64));
    let conditions = [
        present("g.targets"),
        present("g.locations"),
        present("g.callers"),
        if pyrefly {
            present("m.unmapped")
        } else {
            lit(false)
        },
    ];
    let incomplete = conditions
        .iter()
        .cloned()
        .reduce(Expr::or)
        .expect("nonempty gap classes");
    let replace = col("b.family")
        .eq(lit("call-targets"))
        .and(col("b.processing_state").eq(lit("complete")))
        .and(incomplete);
    let mut fragments = vec![lit("call_semantics_incomplete")];
    for (condition, reason) in conditions.into_iter().zip([
        ";unresolved_targets",
        ";source_locations",
        ";caller_entities",
        ";implicit_calls_not_normalized",
    ]) {
        fragments.push(datafusion::logical_expr::when(condition, lit(reason)).otherwise(lit(""))?);
    }
    let reason = datafusion::functions::string::expr_fn::concat(fragments);
    Ok(joined
        .project(
            fields
                .iter()
                .map(|(name, _, _)| {
                    Ok(match *name {
                        "processing_state" => {
                            datafusion::logical_expr::when(replace.clone(), lit("partial"))
                                .otherwise(col("b.processing_state"))?
                                .alias(*name)
                        }
                        "reason" => datafusion::logical_expr::when(replace.clone(), reason.clone())
                            .otherwise(col("b.reason"))?
                            .alias(*name),
                        _ => col(format!("b.{name}")),
                    })
                })
                .collect::<Result<Vec<_>, datafusion::common::DataFusionError>>()?,
        )?
        .build()?)
}

fn qualify_lexical_references(
    inputs: &TransformationInputs,
) -> Result<LogicalPlan, TransformationPlanError> {
    let gaps = LogicalPlanBuilder::from(plan(inputs, super::REFERENCE)?)
        .filter(
            col("resolution")
                .not_eq(lit("resolved"))
                .or(col("target_entity_id").is_null()),
        )?
        .aggregate(
            vec![col("context_id"), col("file_id"), col("source_generation")],
            vec![count(lit(1_i64)).alias("gaps")],
        )?
        .alias("r")?
        .build()?;
    let base = LogicalPlanBuilder::from(plan(inputs, INPUT)?)
        .alias("b")?
        .join(
            gaps,
            JoinType::Left,
            (
                vec!["b.context_id", "b.file_id", "b.source_generation"],
                vec!["r.context_id", "r.file_id", "r.source_generation"],
            ),
            None,
        )?;
    let incomplete = col("b.family")
        .eq(lit("lexical-references"))
        .and(col("b.processing_state").eq(lit("complete")))
        .and(coalesce(vec![col("r.gaps"), lit(0_i64)]).gt(lit(0_i64)));
    Ok(base
        .project(
            fields()
                .iter()
                .map(|(name, _, _)| {
                    Ok(match *name {
                        "processing_state" => {
                            datafusion::logical_expr::when(incomplete.clone(), lit("partial"))
                                .otherwise(col("b.processing_state"))?
                        }
                        "reason" => datafusion::logical_expr::when(
                            incomplete.clone(),
                            lit("lexical_reference_targets_unknown"),
                        )
                        .otherwise(col("b.reason"))?,
                        _ => col(format!("b.{name}")),
                    }
                    .alias(*name))
                })
                .collect::<Result<Vec<_>, datafusion::common::DataFusionError>>()?,
        )?
        .build()?)
}

fn unmapped_python(inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
    let syntax = LogicalPlanBuilder::from(plan(
        inputs,
        NativeSyntaxRelation::RuffCallableSyntax.as_str(),
    )?)
    .filter(col("role").eq(lit("callee-expression")))?
    .alias("x")?
    .build()?;
    Ok(
        LogicalPlanBuilder::from(super::python_calls::targets(inputs, true)?)
            .join(
                syntax,
                JoinType::LeftAnti,
                (
                    vec![
                        "p.file_id",
                        "p.content_digest",
                        "p.source_generation",
                        "p.context_id",
                        "p.start_byte",
                        "p.end_byte",
                    ],
                    vec![
                        "x.file_id",
                        "x.content_digest",
                        "x.source_generation",
                        "x.analysis_context_id",
                        "x.start_byte",
                        "x.end_byte",
                    ],
                ),
                None,
            )?
            .aggregate(
                vec![
                    lit("python").alias("language"),
                    col("p.context_id"),
                    col("p.file_id").alias("scope_key"),
                    col("p.source_generation"),
                ],
                vec![count(lit(1_i64)).alias("unmapped")],
            )?
            .alias("m")?
            .build()?,
    )
}
