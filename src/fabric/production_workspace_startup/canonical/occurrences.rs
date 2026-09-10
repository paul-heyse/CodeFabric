//! Source occurrences are entities independently of their possible semantic targets.

use super::{
    DECLARATION, ENTITY, LogicalPlan, LogicalPlanBuilder, ScalarValue, TransformationInputs,
    TransformationPlanError, calls, canonical_union, col, entity_fields, imports, lit, modules,
    plan, semantic_references,
};

pub(super) fn dependencies() -> Vec<&'static str> {
    vec![
        DECLARATION,
        super::syntax::RELATION,
        modules::RELATION,
        calls::RELATION,
        semantic_references::RELATION,
        imports::RELATION,
    ]
}

pub(super) fn entities(
    inputs: &TransformationInputs,
) -> Result<LogicalPlan, TransformationPlanError> {
    let mut branches = [DECLARATION, modules::RELATION, super::syntax::RELATION]
        .into_iter()
        .map(|relation| {
            Ok(LogicalPlanBuilder::from(plan(inputs, relation)?)
                .filter(col("entity_id").is_not_null())?
                .project(super::entity_projection())?
                .build()?)
        })
        .collect::<Result<Vec<_>, TransformationPlanError>>()?;
    for (relation, id, kind, name) in [
        (calls::RELATION, "call_site_id", "call", lit("call")),
        (
            semantic_references::RELATION,
            "reference_id",
            "reference",
            col("name"),
        ),
        (
            imports::RELATION,
            "import_id",
            "import-occurrence",
            col("source_name"),
        ),
    ] {
        // Candidate/namespace multiplicity belongs to the fact relations, not the entity census.
        branches.push(
            LogicalPlanBuilder::from(plan(inputs, relation)?)
                .filter(col(id).is_not_null())?
                .project([
                    col(id).alias("entity_id"),
                    lit(kind).alias("entity_kind"),
                    name.alias("name"),
                    lit(ScalarValue::Utf8(None)).alias("qualified_name"),
                    col("language"),
                    col("context_id"),
                    col("file_id"),
                    col("workspace_id"),
                ])?
                .build()?,
        );
    }
    Ok(
        LogicalPlanBuilder::from(canonical_union(ENTITY, entity_fields(), branches)?)
            .distinct()?
            .build()?,
    )
}
