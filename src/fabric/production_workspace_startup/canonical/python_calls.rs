//! Syntax owns call occurrences; checker definition coordinates own semantic targets.

use super::{
    DECLARATION, DataType, JoinType, LogicalPlan, LogicalPlanBuilder, NativeSyntaxRelation, RUN,
    SOURCE, ScalarValue, TransformationInputs, TransformationPlanError, col, empty, file_id_udf,
    lit, plan, source_alias, source_occurrence_id,
};
use crate::pyrefly_service::PyreflyRelation;
use datafusion::functions::core::expr_fn::coalesce;

pub(super) fn dependencies(pyrefly: bool) -> Vec<&'static str> {
    let mut result = vec![
        SOURCE,
        DECLARATION,
        NativeSyntaxRelation::RuffCallSite.as_str(),
        NativeSyntaxRelation::RuffCallable.as_str(),
        NativeSyntaxRelation::RuffCallableSyntax.as_str(),
    ];
    if pyrefly {
        result.extend([RUN, PyreflyRelation::CallTarget.relation_id()]);
    }
    result
}

fn keys(left: &str, right: &str, left_id: &str, right_id: &str) -> (Vec<String>, Vec<String>) {
    let common = [
        "file_id",
        "analysis_context_id",
        "content_digest",
        "source_generation",
        "provider_run_id",
    ];
    (
        common
            .iter()
            .map(|key| format!("{left}.{key}"))
            .chain([format!("{left}.{left_id}")])
            .collect(),
        common
            .iter()
            .map(|key| format!("{right}.{key}"))
            .chain([format!("{right}.{right_id}")])
            .collect(),
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one native plan exposes exact source, caller and target joins"
)]
pub(super) fn build(
    workspace: [u8; 16],
    inputs: &TransformationInputs,
    pyrefly: bool,
) -> Result<LogicalPlan, TransformationPlanError> {
    let syntax = plan(inputs, NativeSyntaxRelation::RuffCallableSyntax.as_str())?;
    let call_syntax = LogicalPlanBuilder::from(syntax.clone())
        .filter(col("role").eq(lit("call-expression")))?
        .alias("x")?
        .build()?;
    let callee_syntax = LogicalPlanBuilder::from(syntax.clone())
        .filter(col("role").eq(lit("callee-expression")))?
        .alias("f")?
        .build()?;
    let args = LogicalPlanBuilder::from(syntax)
        .filter(col("role").eq(lit("argument")))?
        .aggregate(
            [
                "file_id",
                "analysis_context_id",
                "content_digest",
                "source_generation",
                "provider_run_id",
                "owner_id",
            ]
            .into_iter()
            .map(col)
            .collect::<Vec<_>>(),
            [datafusion::functions_aggregate::count::count(lit(1_u64)).alias("argument_count")],
        )?
        .alias("a")?
        .build()?;
    let callable =
        LogicalPlanBuilder::from(plan(inputs, NativeSyntaxRelation::RuffCallable.as_str())?)
            .alias("u")?
            .build()?;
    let declarations = LogicalPlanBuilder::from(plan(inputs, DECLARATION)?)
        .filter(
            col("language")
                .eq(lit("python"))
                .and(col("entity_kind").eq(lit("function"))),
        )?
        .build()?;
    let caller = LogicalPlanBuilder::from(declarations.clone())
        .alias("o")?
        .build()?;
    let target = LogicalPlanBuilder::from(declarations).alias("d")?.build()?;
    let joined =
        LogicalPlanBuilder::from(plan(inputs, NativeSyntaxRelation::RuffCallSite.as_str())?)
            .alias("c")?
            .join(
                source_alias(inputs)?,
                JoinType::Inner,
                (
                    vec!["c.file_id", "c.content_digest", "c.source_generation"],
                    vec!["s.file_id", "s.content_digest", "s.source_generation"],
                ),
                None,
            )?
            .join(
                call_syntax,
                JoinType::Inner,
                keys("c", "x", "syntax_id", "syntax_id"),
                None,
            )?
            .join(
                callee_syntax,
                JoinType::Inner,
                keys("c", "f", "callee_syntax_id", "syntax_id"),
                None,
            )?
            .join(
                callable,
                JoinType::Inner,
                keys("c", "u", "caller_id", "callable_id"),
                None,
            )?
            .join(
                args,
                JoinType::Left,
                keys("c", "a", "call_site_id", "owner_id"),
                None,
            )?
            .join(
                caller,
                JoinType::Left,
                (
                    vec![
                        "u.declared_binding_id",
                        "u.file_id",
                        "u.content_digest",
                        "u.source_generation",
                        "u.analysis_context_id",
                        "u.provider_run_id",
                    ],
                    vec![
                        "o.provider_observation_id",
                        "o.file_id",
                        "o.content_digest",
                        "o.source_generation",
                        "o.context_id",
                        "o.provider_run_id",
                    ],
                ),
                None,
            )?
            .join(
                targets(inputs, pyrefly)?,
                JoinType::Left,
                (
                    vec![
                        "c.file_id",
                        "c.content_digest",
                        "c.source_generation",
                        "c.analysis_context_id",
                        "f.start_byte",
                        "f.end_byte",
                    ],
                    vec![
                        "p.file_id",
                        "p.content_digest",
                        "p.source_generation",
                        "p.context_id",
                        "p.start_byte",
                        "p.end_byte",
                    ],
                ),
                None,
            )?
            .join(
                target,
                JoinType::Left,
                (
                    vec![
                        "p.target_file_id",
                        "p.target_content_digest",
                        "p.target_start_byte",
                        "p.target_end_byte",
                        "p.source_generation",
                        "p.context_id",
                    ],
                    vec![
                        "d.file_id",
                        "d.content_digest",
                        "d.start_byte",
                        "d.end_byte",
                        "d.source_generation",
                        "d.context_id",
                    ],
                ),
                None,
            )?
            .build()?;
    // A module/lambda call still has a source occurrence while its public caller entity is
    // unavailable. Its fallback owner is application-owned file/context identity, not a raw key.
    let owner = coalesce(vec![
        col("o.entity_id"),
        source_owner(workspace).call(vec![col("c.analysis_context_id"), col("c.file_id")]),
    ]);
    let occurrence = source_occurrence_id(workspace, "codefabric_python_call_site_id_v1", 102, 3)
        .call(vec![
            owner,
            col("c.file_id"),
            col("c.content_digest"),
            col("x.start_byte"),
            col("x.end_byte"),
        ]);
    let null = |kind| ScalarValue::try_from(&kind).map(lit);
    Ok(LogicalPlanBuilder::from(joined)
        .project(vec![
            occurrence.alias("call_site_id"),
            col("o.entity_id").alias("caller_entity_id"),
            col("d.entity_id").alias("target_entity_id"),
            col("u.qualified_name").alias("caller_name"),
            coalesce(vec![col("d.qualified_name"), col("p.qualified_target")]).alias("target_name"),
            lit("python").alias("language"),
            col("c.analysis_context_id").alias("context_id"),
            col("s.workspace_id"),
            col("c.source_generation"),
            col("c.file_id"),
            col("c.content_digest"),
            col("x.start_byte"),
            col("x.end_byte"),
            lit("exact_call_expression").alias("source_mapping"),
            coalesce(vec![col("p.callee_kind"), col("c.dispatch_kind")]).alias("dispatch_kind"),
            datafusion::logical_expr::when(
                col("d.entity_id").is_not_null(),
                lit("resolved_declaration"),
            )
            .otherwise(lit("unknown"))?
            .alias("resolution"),
            datafusion::logical_expr::when(
                col("p.qualified_target").is_null(),
                lit("semantic_target_unavailable"),
            )
            .when(
                col("p.target_file_id").is_null(),
                col("p.target_source_mapping"),
            )
            .when(
                col("d.entity_id").is_null(),
                lit("target_declaration_not_captured"),
            )
            .when(
                col("o.entity_id").is_null(),
                lit("caller_entity_unavailable"),
            )
            .otherwise(lit(ScalarValue::Utf8(None)))?
            .alias("unknown_reason"),
            datafusion::logical_expr::expr_fn::cast(
                coalesce(vec![col("a.argument_count"), lit(0_i64)]),
                DataType::UInt64,
            )
            .alias("argument_count"),
            col("c.dispatch_kind").alias("raw_dispatch_kind"),
            col("p.qualified_target").alias("raw_declared_target"),
            coalesce(vec![col("p.resolution_state"), lit("unknown")])
                .alias("raw_resolution_confidence"),
            null(DataType::FixedSizeBinary(32))?.alias("provider_instance_key"),
            col("c.provider_run_id"),
            null(DataType::Utf8)?.alias("provider_compilation_unit"),
            null(DataType::Utf8)?.alias("provider_owner"),
            null(DataType::UInt64)?.alias("provider_block_index"),
            datafusion::logical_expr::when(
                col("p.qualified_target").is_not_null(),
                lit("ruff+pyrefly"),
            )
            .otherwise(lit("ruff"))?
            .alias("provider"),
        ])?
        .build()?)
}

fn targets(
    inputs: &TransformationInputs,
    enabled: bool,
) -> Result<LogicalPlan, TransformationPlanError> {
    let fields = vec![
        ("file_id", DataType::FixedSizeBinary(16), false),
        ("content_digest", DataType::FixedSizeBinary(32), false),
        ("source_generation", DataType::UInt64, false),
        ("context_id", DataType::FixedSizeBinary(16), false),
        ("start_byte", DataType::UInt64, false),
        ("end_byte", DataType::UInt64, false),
        ("target_file_id", DataType::FixedSizeBinary(16), true),
        ("target_content_digest", DataType::FixedSizeBinary(32), true),
        ("target_start_byte", DataType::UInt64, true),
        ("target_end_byte", DataType::UInt64, true),
        ("qualified_target", DataType::Utf8, false),
        ("callee_kind", DataType::Utf8, false),
        ("target_source_mapping", DataType::Utf8, false),
        ("resolution_state", DataType::Utf8, false),
    ];
    let selected = if enabled {
        let runs = LogicalPlanBuilder::from(plan(inputs, RUN)?)
            .filter(col("provider").eq(lit("pyrefly")))?
            .alias("r")?
            .build()?;
        LogicalPlanBuilder::from(plan(inputs, PyreflyRelation::CallTarget.relation_id())?)
            .alias("t")?
            .join(
                runs,
                JoinType::Inner,
                (
                    vec!["t.provider_run_id", "t.source_generation"],
                    vec!["r.provider_run_identity", "r.source_generation"],
                ),
                None,
            )?
            .project(
                fields
                    .iter()
                    .map(|(name, _, _)| match *name {
                        "context_id" => col("r.context_id").alias(*name),
                        "file_id" | "target_file_id" => file_id_udf()
                            .call(vec![col(format!("t.{name}"))])
                            .alias(*name),
                        _ => col(format!("t.{name}")),
                    })
                    .collect::<Vec<_>>(),
            )?
            .build()?
    } else {
        empty(fields)?
    };
    Ok(LogicalPlanBuilder::from(selected).alias("p")?.build()?)
}

fn source_owner(workspace: [u8; 16]) -> std::sync::Arc<datafusion::logical_expr::ScalarUDF> {
    std::sync::Arc::new(datafusion::logical_expr::create_udf(
        "codefabric_python_source_call_owner_v1",
        vec![DataType::FixedSizeBinary(16); 2],
        DataType::FixedSizeBinary(16),
        datafusion::logical_expr::Volatility::Immutable,
        std::sync::Arc::new(move |values| {
            super::ids(values, |arrays, row| {
                crate::identity::semantic_owner_identity(
                    workspace,
                    super::fixed(&arrays[0], row)?,
                    "python-source-call-owner",
                    super::fixed(&arrays[1], row)?.to_vec(),
                )
                .map(|owner| owner.id)
                .map_err(|error| super::invalid(&error.to_string()))
            })
        }),
    ))
}
