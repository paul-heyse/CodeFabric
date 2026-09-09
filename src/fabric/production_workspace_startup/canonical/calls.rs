//! Compiler call observations joined to independently captured Rust syntax and canonical owners.

use super::{
    DataFusionError, DataType, Expr, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder, RUN,
    RustcRelation, SOURCE, ScalarValue, TransformationInputs, TransformationPlanError, col,
    file_id_udf, lit, plan, rust_entity_id, source_alias, source_occurrence_id,
};
use crate::provider_native_rust_syntax::RustSyntaxRelation;

pub(super) const RELATION: &str = "fact.code_call_site";

pub(super) fn dependencies() -> Vec<&'static str> {
    vec![
        SOURCE,
        RUN,
        RustcRelation::Call.relation_id(),
        RustcRelation::MirTerminator.relation_id(),
        RustcRelation::PublicItem.relation_id(),
        RustSyntaxRelation::CstNode.name(),
    ]
}

pub(super) fn fields() -> Vec<FieldSpec> {
    vec![
        ("call_site_id", DataType::FixedSizeBinary(16), true),
        ("caller_entity_id", DataType::FixedSizeBinary(16), true),
        ("target_entity_id", DataType::FixedSizeBinary(16), true),
        ("caller_name", DataType::Utf8, true),
        ("target_name", DataType::Utf8, true),
        ("language", DataType::Utf8, false),
        ("context_id", DataType::FixedSizeBinary(16), false),
        ("workspace_id", DataType::FixedSizeBinary(16), false),
        ("source_generation", DataType::UInt64, false),
        ("file_id", DataType::FixedSizeBinary(16), true),
        ("content_digest", DataType::FixedSizeBinary(32), true),
        ("start_byte", DataType::UInt64, true),
        ("end_byte", DataType::UInt64, true),
        ("source_mapping", DataType::Utf8, false),
        ("dispatch_kind", DataType::Utf8, false),
        ("resolution", DataType::Utf8, false),
        ("unknown_reason", DataType::Utf8, true),
        ("argument_count", DataType::UInt64, false),
        ("raw_dispatch_kind", DataType::Utf8, false),
        ("raw_declared_target", DataType::Utf8, true),
        ("raw_resolution_confidence", DataType::Utf8, false),
        ("provider_instance_key", DataType::FixedSizeBinary(32), true),
        ("provider_run_id", DataType::FixedSizeBinary(16), false),
        ("provider_compilation_unit", DataType::Utf8, true),
        ("provider_owner", DataType::Utf8, true),
        ("provider_block_index", DataType::UInt64, true),
        ("provider", DataType::Utf8, false),
    ]
}

#[allow(
    clippy::too_many_lines,
    reason = "keep the exact compiler and syntax join coordinates visible in one plan"
)]
pub(super) fn build(
    workspace: [u8; 16],
    inputs: &TransformationInputs,
) -> Result<LogicalPlan, TransformationPlanError> {
    let native = plan(inputs, RustcRelation::Call.relation_id())?;
    let columns = native
        .schema()
        .fields()
        .iter()
        .map(|field| col(field.name()))
        .collect::<Vec<_>>();
    let native = LogicalPlanBuilder::from(native)
        .project(
            columns.into_iter().chain([file_id_udf()
                .call(vec![col("source_file_id")])
                .alias("file_id")]),
        )?
        .alias("p")?
        .build()?;
    let runs = LogicalPlanBuilder::from(plan(inputs, RUN)?)
        .filter(col("provider").eq(lit("rustc")))?
        .alias("r")?
        .build()?;
    let terminators =
        LogicalPlanBuilder::from(plan(inputs, RustcRelation::MirTerminator.relation_id())?)
            .alias("m")?
            .build()?;
    let owners = LogicalPlanBuilder::from(plan(inputs, RustcRelation::PublicItem.relation_id())?)
        .alias("o")?
        .build()?;
    let syntax = LogicalPlanBuilder::from(plan(inputs, RustSyntaxRelation::CstNode.name())?)
        .filter(
            col("raw_kind")
                .eq(lit("call_expression"))
                .and(col("error").eq(lit(false)))
                .and(col("missing").eq(lit(false))),
        )?
        .project([
            col("file_id"),
            col("content_digest"),
            col("source_generation"),
            col("start_byte"),
            col("end_byte"),
        ])?
        .distinct()?
        .alias("k")?
        .build()?;

    // Provider owner and MIR block keys are only join coordinates within one admitted compiler
    // run. Canonical identities below use application owners and exact source call occurrences.
    let joined = LogicalPlanBuilder::from(native)
        .join(
            runs,
            JoinType::Inner,
            (
                vec!["p.provider_run_id", "p.source_generation"],
                vec!["r.provider_run_identity", "r.source_generation"],
            ),
            None,
        )?
        .join(
            source_alias(inputs)?,
            JoinType::Inner,
            (
                vec![
                    "p.file_id",
                    "p.source_content_digest",
                    "p.source_generation",
                ],
                vec!["s.file_id", "s.content_digest", "s.source_generation"],
            ),
            None,
        )?
        .join(
            terminators,
            JoinType::Left,
            (
                vec![
                    "p.provider_run_id",
                    "p.compilation_unit_id",
                    "p.owner_id",
                    "p.block_index",
                    "p.source_generation",
                ],
                vec![
                    "m.provider_run_id",
                    "m.compilation_unit_id",
                    "m.owner_id",
                    "m.block_index",
                    "m.source_generation",
                ],
            ),
            None,
        )?
        .join(
            owners,
            JoinType::Left,
            (
                vec![
                    "p.provider_run_id",
                    "p.compilation_unit_id",
                    "p.owner_id",
                    "p.source_generation",
                ],
                vec![
                    "o.provider_run_id",
                    "o.compilation_unit_id",
                    "o.owner_id",
                    "o.source_generation",
                ],
            ),
            None,
        )?
        .join(
            syntax,
            JoinType::Left,
            (
                vec![
                    "p.file_id",
                    "p.source_content_digest",
                    "p.source_generation",
                    "m.span_start_byte",
                    "m.span_end_byte",
                ],
                vec![
                    "k.file_id",
                    "k.content_digest",
                    "k.source_generation",
                    "k.start_byte",
                    "k.end_byte",
                ],
            ),
            None,
        )?
        .join(
            targets(inputs)?,
            JoinType::Left,
            (
                vec![
                    "p.provider_run_id",
                    "p.source_generation",
                    "p.declared_stable_crate_id",
                    "p.declared_def_path_hash",
                ],
                vec![
                    "t.provider_run_id",
                    "t.source_generation",
                    "t.stable_crate_id",
                    "t.def_path_hash",
                ],
            ),
            None,
        )?
        .build()?;
    project(workspace, joined)
}

fn targets(inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
    let native = plan(inputs, RustcRelation::PublicItem.relation_id())?;
    let native = LogicalPlanBuilder::from(native)
        .project([
            col("provider_run_id"),
            col("source_generation"),
            col("stable_crate_id"),
            col("def_path_hash"),
            col("item_kind"),
            col("qualified_name"),
            col("source_content_digest"),
            file_id_udf()
                .call(vec![col("source_file_id")])
                .alias("file_id"),
        ])?
        .alias("target")?
        .build()?;
    Ok(LogicalPlanBuilder::from(native)
        .join(
            source_alias(inputs)?,
            JoinType::Inner,
            (
                vec![
                    "target.file_id",
                    "target.source_content_digest",
                    "target.source_generation",
                ],
                vec!["s.file_id", "s.content_digest", "s.source_generation"],
            ),
            None,
        )?
        .project([
            col("target.provider_run_id"),
            col("target.source_generation"),
            col("target.stable_crate_id"),
            col("target.def_path_hash"),
            col("target.item_kind"),
            col("target.qualified_name"),
        ])?
        .distinct()?
        .alias("t")?
        .build()?)
}

fn project(
    workspace: [u8; 16],
    joined: LogicalPlan,
) -> Result<LogicalPlan, TransformationPlanError> {
    let caller = rust_entity_id(workspace).call(vec![
        col("r.context_id"),
        col("o.stable_crate_id"),
        col("o.def_path_hash"),
        col("o.item_kind"),
    ]);
    let target = rust_entity_id(workspace).call(vec![
        col("r.context_id"),
        col("t.stable_crate_id"),
        col("t.def_path_hash"),
        col("t.item_kind"),
    ]);
    let mapped = col("k.start_byte")
        .is_not_null()
        .and(col("m.span_file").eq(col("o.span_file")))
        .and(col("m.expansion_kind").eq(lit("source-authored")))
        .and(col("m.span_end_byte").lt_eq(col("s.byte_length")));
    let located = |value: Expr, kind| -> Result<Expr, DataFusionError> {
        datafusion::logical_expr::when(mapped.clone(), value)
            .otherwise(lit(ScalarValue::try_from(&kind)?))
    };
    let occurrence =
        source_occurrence_id(workspace, "codefabric_call_site_id_v1", 102, 3).call(vec![
            caller.clone(),
            col("p.file_id"),
            col("p.source_content_digest"),
            col("m.span_start_byte"),
            col("m.span_end_byte"),
        ]);
    Ok(LogicalPlanBuilder::from(joined)
        .project(vec![
            located(occurrence, DataType::FixedSizeBinary(16))?.alias("call_site_id"),
            caller.clone().alias("caller_entity_id"),
            target.clone().alias("target_entity_id"),
            col("o.qualified_name").alias("caller_name"),
            col("t.qualified_name").alias("target_name"),
            lit("rust").alias("language"),
            col("r.context_id").alias("context_id"),
            col("s.workspace_id").alias("workspace_id"),
            col("p.source_generation").alias("source_generation"),
            located(col("p.file_id"), DataType::FixedSizeBinary(16))?.alias("file_id"),
            located(
                col("p.source_content_digest"),
                DataType::FixedSizeBinary(32),
            )?
            .alias("content_digest"),
            located(col("m.span_start_byte"), DataType::UInt64)?.alias("start_byte"),
            located(col("m.span_end_byte"), DataType::UInt64)?.alias("end_byte"),
            datafusion::logical_expr::when(mapped.clone(), lit("exact_call_expression"))
                .otherwise(lit("unmapped_lowered_call"))?
                .alias("source_mapping"),
            datafusion::logical_expr::when(col("p.dispatch_kind").eq(lit("Direct")), lit("direct"))
                .when(
                    col("p.dispatch_kind").eq(lit("FunctionPointer")),
                    lit("function-pointer"),
                )
                .when(col("p.dispatch_kind").eq(lit("Closure")), lit("closure"))
                .when(
                    col("p.dispatch_kind").eq(lit("DynamicTrait")),
                    lit("dynamic-trait"),
                )
                .otherwise(lit("unknown"))?
                .alias("dispatch_kind"),
            datafusion::logical_expr::when(
                target.clone().is_not_null(),
                lit("resolved_declaration"),
            )
            .otherwise(lit("unknown"))?
            .alias("resolution"),
            datafusion::logical_expr::when(caller.is_null(), lit("caller_identity_unavailable"))
                .when(mapped.is_not_true(), lit("source_call_site_unmapped"))
                .when(
                    col("p.declared_def_path_hash")
                        .is_null()
                        .and(col("p.dispatch_kind").not_eq(lit("Direct"))),
                    lit("indirect_target_unresolved"),
                )
                .when(
                    col("p.declared_def_path_hash").is_null(),
                    lit("declared_target_identity_unavailable"),
                )
                .when(target.is_null(), lit("target_declaration_not_captured"))
                .otherwise(lit(ScalarValue::Utf8(None)))?
                .alias("unknown_reason"),
            col("p.argument_count").alias("argument_count"),
            col("p.dispatch_kind").alias("raw_dispatch_kind"),
            col("p.declared_target").alias("raw_declared_target"),
            col("p.resolution_confidence").alias("raw_resolution_confidence"),
            col("p.resolved_instance_key").alias("provider_instance_key"),
            col("r.provider_run_id").alias("provider_run_id"),
            col("p.compilation_unit_id").alias("provider_compilation_unit"),
            col("p.owner_id").alias("provider_owner"),
            col("p.block_index").alias("provider_block_index"),
            lit("rustc").alias("provider"),
        ])?
        .build()?)
}
