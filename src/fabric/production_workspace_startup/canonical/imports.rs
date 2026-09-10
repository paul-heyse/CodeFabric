//! Import syntax and checker resolution remain separate, joined only by a proven alias anchor.

use super::{
    DataType, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder, NativeSyntaxRelation, SOURCE,
    ScalarValue, TransformationInputs, TransformationPlanError, col, lit, modules, plan,
    semantic_references, source_alias, source_occurrence_id,
};
use datafusion::functions::core::expr_fn::coalesce;
use datafusion::logical_expr::when;

pub(super) const RELATION: &str = "fact.code_import";

pub(super) fn fields() -> Vec<FieldSpec> {
    vec![
        ("import_id", DataType::FixedSizeBinary(16), true),
        ("semantic_reference_id", DataType::FixedSizeBinary(16), true),
        ("language", DataType::Utf8, false),
        ("context_id", DataType::FixedSizeBinary(16), false),
        ("file_id", DataType::FixedSizeBinary(16), true),
        ("content_digest", DataType::FixedSizeBinary(32), true),
        ("source_generation", DataType::UInt64, false),
        ("workspace_id", DataType::FixedSizeBinary(16), false),
        ("start_byte", DataType::UInt64, true),
        ("end_byte", DataType::UInt64, true),
        ("import_kind", DataType::Utf8, false),
        ("source_name", DataType::Utf8, false),
        ("module_name", DataType::Utf8, true),
        ("imported_name", DataType::Utf8, true),
        ("alias_name", DataType::Utf8, true),
        ("relative_level", DataType::UInt16, true),
        ("star_import", DataType::Boolean, false),
        ("target_entity_id", DataType::FixedSizeBinary(16), true),
        ("target_declaration_id", DataType::FixedSizeBinary(16), true),
        ("resolution", DataType::Utf8, false),
        ("unknown_reason", DataType::Utf8, true),
        (
            "syntax_provider_run_id",
            DataType::FixedSizeBinary(16),
            true,
        ),
        (
            "semantic_provider_run_id",
            DataType::FixedSizeBinary(16),
            true,
        ),
        ("semantic_context_id", DataType::FixedSizeBinary(16), true),
        ("syntax_observation_id", DataType::FixedSizeBinary(16), true),
        ("join_method", DataType::Utf8, false),
        ("provider", DataType::Utf8, false),
        ("provider_compilation_unit", DataType::Utf8, true),
        ("provider_owner", DataType::Utf8, true),
        ("provider_import_ordinal", DataType::UInt64, true),
        ("target_namespace", DataType::Utf8, true),
        ("is_public", DataType::Boolean, true),
    ]
}

pub(super) fn dependencies(available: bool, rust: bool) -> Vec<&'static str> {
    let mut result = if available {
        vec![
            SOURCE,
            NativeSyntaxRelation::RuffImport.as_str(),
            semantic_references::RELATION,
            modules::RELATION,
        ]
    } else {
        vec![]
    };
    if rust {
        result.extend([
            SOURCE,
            super::RUN,
            semantic_references::RELATION,
            super::RustcRelation::HirImport.relation_id(),
        ]);
    }
    result.sort_unstable();
    result.dedup();
    result
}

pub(super) fn build(
    workspace: [u8; 16],
    inputs: &TransformationInputs,
    python: bool,
    rust: bool,
) -> Result<LogicalPlan, TransformationPlanError> {
    let mut plans = Vec::new();
    if python {
        plans.push(python_plan(workspace, inputs)?);
    }
    if rust {
        plans.push(super::rust_references::imports(workspace, inputs)?);
    }
    super::canonical_union(RELATION, fields(), plans)
}

#[allow(
    clippy::too_many_lines,
    reason = "one native import plan keeps exact join keys and its typed projection together"
)]
fn python_plan(
    workspace: [u8; 16],
    inputs: &TransformationInputs,
) -> Result<LogicalPlan, TransformationPlanError> {
    let raw = LogicalPlanBuilder::from(plan(inputs, NativeSyntaxRelation::RuffImport.as_str())?)
        .alias("p")?
        .build()?;
    let semantic = LogicalPlanBuilder::from(plan(inputs, semantic_references::RELATION)?)
        .filter(
            col("language")
                .eq(lit("python"))
                .and(col("reference_kind").eq(lit("import"))),
        )?
        .alias("r")?
        .build()?;
    let modules = LogicalPlanBuilder::from(plan(inputs, modules::RELATION)?)
        .alias("m")?
        .build()?;
    let occurrence = source_occurrence_id(workspace, "codefabric_import_occurrence_id_v1", 103, 4)
        .call(vec![
            col("p.file_id"),
            col("p.file_id"),
            col("p.content_digest"),
            col("p.start_byte"),
            col("p.end_byte"),
        ]);
    Ok(LogicalPlanBuilder::from(raw)
        .join(
            source_alias(inputs)?,
            JoinType::Inner,
            (
                vec!["p.file_id", "p.content_digest", "p.source_generation"],
                vec!["s.file_id", "s.content_digest", "s.source_generation"],
            ),
            None,
        )?
        .filter(
            col("p.start_byte")
                .lt(col("p.end_byte"))
                .and(col("p.end_byte").lt_eq(col("s.byte_length"))),
        )?
        .join_on(
            modules,
            JoinType::Left,
            vec![
                col("p.analysis_context_id").eq(col("m.context_id")),
                col("p.file_id").eq(col("m.file_id")),
                col("p.content_digest").eq(col("m.content_digest")),
                col("p.source_generation").eq(col("m.source_generation")),
            ],
        )?
        .join_on(
            semantic,
            JoinType::Left,
            vec![
                col("p.file_id").eq(col("r.file_id")),
                col("p.content_digest").eq(col("r.content_digest")),
                col("p.source_generation").eq(col("r.source_generation")),
                col("p.analysis_context_id").eq(col("r.context_id")),
                col("p.start_byte").eq(col("r.start_byte")),
                col("r.end_byte").lt_eq(col("p.end_byte")),
            ],
        )?
        .project(vec![
            occurrence.alias("import_id"),
            col("r.reference_id").alias("semantic_reference_id"),
            lit("python").alias("language"),
            col("p.analysis_context_id").alias("context_id"),
            col("p.file_id").alias("file_id"),
            col("p.content_digest").alias("content_digest"),
            col("p.source_generation").alias("source_generation"),
            col("s.workspace_id").alias("workspace_id"),
            col("p.start_byte").alias("start_byte"),
            col("p.end_byte").alias("end_byte"),
            col("p.import_kind").alias("import_kind"),
            col("p.source_name").alias("source_name"),
            col("p.target_module_name").alias("module_name"),
            col("p.imported_name").alias("imported_name"),
            col("p.alias_name").alias("alias_name"),
            col("p.relative_level").alias("relative_level"),
            col("p.star_import").alias("star_import"),
            col("r.target_entity_id").alias("target_entity_id"),
            col("r.target_declaration_id").alias("target_declaration_id"),
            coalesce(vec![col("r.resolution"), lit("unknown")]).alias("resolution"),
            when(
                col("r.reference_id").is_null(),
                lit("checker_import_resolution_unavailable"),
            )
            .otherwise(col("r.unknown_reason"))?
            .alias("unknown_reason"),
            col("p.provider_run_id").alias("syntax_provider_run_id"),
            col("r.provider_run_id").alias("semantic_provider_run_id"),
            coalesce(vec![col("r.context_id"), col("m.context_id")]).alias("semantic_context_id"),
            col("p.import_id").alias("syntax_observation_id"),
            when(
                col("r.reference_id").is_not_null(),
                lit("checker-name-within-ruff-alias"),
            )
            .otherwise(lit("unmatched-syntax"))?
            .alias("join_method"),
            lit("ruff/pyrefly").alias("provider"),
            lit(ScalarValue::Utf8(None)).alias("provider_compilation_unit"),
            lit(ScalarValue::Utf8(None)).alias("provider_owner"),
            lit(ScalarValue::UInt64(None)).alias("provider_import_ordinal"),
            lit(ScalarValue::Utf8(None)).alias("target_namespace"),
            lit(ScalarValue::Boolean(None)).alias("is_public"),
        ])?
        .distinct()?
        .build()?)
}
