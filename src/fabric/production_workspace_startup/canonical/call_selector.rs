//! Canonical call subjects and public endpoint identities, derived with native joins.

use super::{
    DataType, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder, TransformationInputs,
    TransformationPlanError, col, lit, plan, public_entity_id,
};

pub(super) const RELATION: &str = "fact.code_call_selector";

pub(super) fn fields() -> Vec<FieldSpec> {
    let mut result = super::calls::fields();
    result.extend([
        ("public_caller_entity_id", DataType::Utf8, true),
        ("public_target_entity_id", DataType::Utf8, true),
        ("public_call_site_id", DataType::Utf8, true),
        ("public_entity_id", DataType::Utf8, true),
        ("subject_kind", DataType::Utf8, false),
        ("fact_family", DataType::Utf8, false),
        ("direction", DataType::Utf8, false),
        ("distance", DataType::Utf8, false),
    ]);
    result
}

pub(super) fn build(inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
    // Entity projection is distinct before joining: declaration occurrences and parallel
    // provider observations must not multiply call occurrences.
    let entities = LogicalPlanBuilder::from(plan(inputs, super::ENTITY)?)
        .project([col("entity_id"), col("entity_kind"), col("context_id")])?
        .distinct()?
        .build()?;
    let calls = LogicalPlanBuilder::from(plan(inputs, super::calls::RELATION)?)
        .alias("c")?
        .join(
            LogicalPlanBuilder::from(entities.clone())
                .alias("a")?
                .build()?,
            JoinType::Left,
            (
                vec!["c.caller_entity_id", "c.context_id"],
                vec!["a.entity_id", "a.context_id"],
            ),
            None,
        )?
        .join(
            LogicalPlanBuilder::from(entities).alias("t")?.build()?,
            JoinType::Left,
            (
                vec!["c.target_entity_id", "c.context_id"],
                vec!["t.entity_id", "t.context_id"],
            ),
            None,
        )?
        .project(
            super::calls::fields()
                .iter()
                .map(|(name, _, _)| col(format!("c.{name}")))
                .chain([
                    public_entity_id()
                        .call(vec![col("c.caller_entity_id"), col("a.entity_kind")])
                        .alias("public_caller_entity_id"),
                    public_entity_id()
                        .call(vec![col("c.target_entity_id"), col("t.entity_kind")])
                        .alias("public_target_entity_id"),
                    public_entity_id()
                        .call(vec![col("c.call_site_id"), lit("call-site")])
                        .alias("public_call_site_id"),
                ]),
        )?
        .build()?;
    let direction = |name: &str, subject: &str| -> Result<LogicalPlan, TransformationPlanError> {
        Ok(LogicalPlanBuilder::from(calls.clone())
            .project(
                calls
                    .schema()
                    .fields()
                    .iter()
                    .map(|field| col(field.name()))
                    .chain([
                        col(subject).alias("public_entity_id"),
                        lit("entity").alias("subject_kind"),
                        lit("calls").alias("fact_family"),
                        lit(name).alias("direction"),
                        lit("one relationship step").alias("distance"),
                    ]),
            )?
            .build()?)
    };
    Ok(
        LogicalPlanBuilder::from(direction("outgoing", "public_caller_entity_id")?)
            .union(direction("incoming", "public_target_entity_id")?)?
            .build()?,
    )
}
