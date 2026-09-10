//! Exact source descriptors for canonical subjects without fabricated declarations.

use super::super::{
    Expr, JoinType, LogicalPlan, LogicalPlanBuilder, ScalarValue, TransformationInputs,
    TransformationPlanError, calls, col, imports, lit, modules, plan, public_entity_id,
    semantic_references,
};
use datafusion::functions::core::expr_fn::coalesce;

pub(super) fn descriptors(
    inputs: &TransformationInputs,
) -> Result<Vec<LogicalPlan>, TransformationPlanError> {
    [
        (
            super::super::syntax::RELATION,
            "entity_id",
            "syntax-node",
            col("d.name"),
            col("d.raw_kind"),
            "syntax-nodes",
        ),
        (
            calls::RELATION,
            "call_site_id",
            "call",
            lit("call"),
            col("d.raw_dispatch_kind"),
            "call-targets",
        ),
        (
            semantic_references::RELATION,
            "reference_id",
            "reference",
            col("d.name"),
            col("d.reference_kind"),
            "semantic-references",
        ),
        (
            imports::RELATION,
            "import_id",
            "import-occurrence",
            col("d.source_name"),
            col("d.import_kind"),
            "imports",
        ),
        (
            super::super::REFERENCE,
            "reference_id",
            "reference",
            col("d.name"),
            col("d.reference_kind"),
            "lexical-references",
        ),
        (
            modules::RELATION,
            "entity_id",
            "module",
            col("d.name"),
            lit("module"),
            "modules",
        ),
    ]
    .into_iter()
    .map(|(relation, id, kind, name, raw_kind, family)| {
        descriptor(inputs, relation, id, kind, name, raw_kind, family)
    })
    .collect()
}

#[allow(
    clippy::too_many_lines,
    reason = "the native projection keeps exact source validity and subject provenance together"
)]
fn descriptor(
    inputs: &TransformationInputs,
    relation: &str,
    id: &str,
    kind: &str,
    name: Expr,
    raw_kind: Expr,
    family: &str,
) -> Result<LogicalPlan, TransformationPlanError> {
    let subject = LogicalPlanBuilder::from(plan(inputs, relation)?)
        .filter(col(id).is_not_null())?
        .alias("d")?
        .build()?;
    let source = LogicalPlanBuilder::from(plan(inputs, super::super::SOURCE)?)
        .alias("s")?
        .build()?;
    let null_id = || lit(ScalarValue::FixedSizeBinary(16, None));
    let run = if relation == imports::RELATION {
        coalesce(vec![
            col("d.syntax_provider_run_id"),
            col("d.semantic_provider_run_id"),
        ])
    } else {
        col("d.provider_run_id")
    };
    let observation = match relation {
        imports::RELATION => col("d.syntax_observation_id"),
        super::super::REFERENCE => col("d.provider_observation_id"),
        _ => null_id(),
    };
    let (start, end) = if relation == modules::RELATION {
        (lit(0_u64), col("s.byte_length"))
    } else {
        (col("d.start_byte"), col("d.end_byte"))
    };
    let selected = LogicalPlanBuilder::from(subject)
        .join(
            source,
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
        .filter(
            run.clone()
                .is_not_null()
                .and(start.clone().is_not_null())
                .and(end.clone().is_not_null())
                .and(start.clone().lt_eq(end.clone()))
                .and(end.clone().lt_eq(col("s.byte_length"))),
        )?
        .build()?;
    // The filter proves presence before these non-null expressions refine Arrow nullability.
    // No default value can become an emitted descriptor or an application identity.
    let present_run = coalesce(vec![
        run,
        lit(ScalarValue::FixedSizeBinary(16, Some(vec![0; 16]))),
    ]);
    let present = |value| coalesce(vec![value, lit(0_u64)]);
    let descriptor = LogicalPlanBuilder::from(selected)
        .project([
            col(format!("d.{id}")).alias("entity_id"),
            lit(kind).alias("entity_kind"),
            name.alias("name"),
            lit(ScalarValue::Utf8(None)).alias("qualified_name"),
            col("d.language"),
            col("d.context_id"),
            col("d.file_id"),
            col("s.content_digest"),
            col("s.source_generation"),
            present(start).alias("start_byte"),
            present(end).alias("end_byte"),
            present_run.alias("provider_run_id"),
            col("d.provider"),
            raw_kind.alias("raw_kind"),
            observation.alias("provider_observation_id"),
            col("s.workspace_id"),
            null_id().alias("declaration_id"),
            lit("canonical").alias("identity_state"),
            public_entity_id()
                .call(vec![col(format!("d.{id}")), lit(kind)])
                .alias("public_entity_id"),
            lit("entity").alias("subject_kind"),
            lit(family).alias("fact_family"),
            col("s.relative_path"),
            lit(if relation == modules::RELATION {
                "exact-module-file"
            } else {
                "exact-occurrence-span"
            })
            .alias("source_mapping"),
        ])?
        .distinct()?
        .build()?;
    let branch = |context: &str| -> Result<LogicalPlan, TransformationPlanError> {
        Ok(LogicalPlanBuilder::from(descriptor.clone())
            .project(super::fields().into_iter().map(|(name, _, _)| match name {
                "context_kind" => lit(context).alias(name),
                "text_handling" => lit("lossless UTF-8 else bytes").alias(name),
                _ => col(name),
            }))?
            .build()?)
    };
    Ok(LogicalPlanBuilder::from(branch("exact source span")?)
        .union(branch("surrounding lines")?)?
        .union(branch("syntax outline")?)?
        .build()?)
}
