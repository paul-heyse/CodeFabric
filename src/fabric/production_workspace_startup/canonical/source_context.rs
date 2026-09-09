//! Subject source descriptors join through exact workspace, generation, file and content pins.

use super::{
    DataType, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder, TransformationInputs,
    TransformationPlanError, col, lit, plan,
};

pub(super) const RELATION: &str = "fact.code_source_context";

pub(super) fn fields() -> Vec<FieldSpec> {
    let mut fields = super::declaration_fields();
    fields.extend([
        ("relative_path", DataType::Binary, false),
        ("context_kind", DataType::Utf8, false),
        ("text_handling", DataType::Utf8, false),
    ]);
    fields
}

pub(super) fn build(inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
    let declarations = LogicalPlanBuilder::from(plan(inputs, super::DECLARATION)?)
        .filter(col("public_entity_id").is_not_null())?
        .alias("d")?
        .build()?;
    let bytes = LogicalPlanBuilder::from(plan(inputs, super::SOURCE)?)
        .alias("s")?
        .build()?;
    Ok(LogicalPlanBuilder::from(declarations)
        .join(
            bytes,
            JoinType::Inner,
            (
                vec![
                    "d.workspace_id",
                    "d.source_generation",
                    "d.file_id",
                    "d.content_digest",
                ],
                vec![
                    "s.workspace_id",
                    "s.source_generation",
                    "s.file_id",
                    "s.content_digest",
                ],
            ),
            None,
        )?
        .project(
            super::declaration_fields()
                .iter()
                .map(|(name, _, _)| col(format!("d.{name}")))
                .chain([
                    col("s.relative_path"),
                    lit("exact source span").alias("context_kind"),
                    lit("lossless UTF-8 else bytes").alias("text_handling"),
                ]),
        )?
        .build()?)
}
