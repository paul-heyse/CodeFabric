//! Import syntax and checker resolution remain separate, joined only by a proven alias anchor.

use super::{
    DataType, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder, NativeSyntaxRelation, SOURCE,
    TransformationInputs, TransformationPlanError, col, empty, lit, modules, plan,
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
        ("file_id", DataType::FixedSizeBinary(16), false),
        ("content_digest", DataType::FixedSizeBinary(32), false),
        ("source_generation", DataType::UInt64, false),
        ("workspace_id", DataType::FixedSizeBinary(16), false),
        ("start_byte", DataType::UInt64, false),
        ("end_byte", DataType::UInt64, false),
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
            false,
        ),
        (
            "semantic_provider_run_id",
            DataType::FixedSizeBinary(16),
            true,
        ),
        ("semantic_context_id", DataType::FixedSizeBinary(16), true),
        (
            "syntax_observation_id",
            DataType::FixedSizeBinary(16),
            false,
        ),
        ("join_method", DataType::Utf8, false),
    ]
}

pub(super) fn dependencies(available: bool) -> Vec<&'static str> {
    if available {
        vec![
            SOURCE,
            NativeSyntaxRelation::RuffImport.as_str(),
            semantic_references::RELATION,
            modules::RELATION,
        ]
    } else {
        vec![]
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one native import plan keeps exact join keys and its typed projection together"
)]
pub(super) fn build(
    workspace: [u8; 16],
    inputs: &TransformationInputs,
    available: bool,
) -> Result<LogicalPlan, TransformationPlanError> {
    if !available {
        return empty(fields());
    }
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
        ])?
        .distinct()?
        .build()?)
}
