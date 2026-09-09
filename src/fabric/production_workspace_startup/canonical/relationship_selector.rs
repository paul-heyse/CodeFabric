//! Public relationship witnesses retain each occurrence and its provider-specific facts.

use super::{
    DataType, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder, ScalarValue,
    TransformationInputs, TransformationPlanError, col, lit, plan, public_entity_id,
};

pub(super) const RELATION: &str = "fact.code_relationship_selector";

pub(super) fn fields() -> Vec<FieldSpec> {
    let mut fields = super::call_selector::fields();
    // These call-only properties are inapplicable to a lexical reference.
    for (name, _, nullable) in &mut fields {
        if matches!(
            *name,
            "source_mapping"
                | "dispatch_kind"
                | "argument_count"
                | "raw_dispatch_kind"
                | "raw_resolution_confidence"
        ) {
            *nullable = true;
        }
    }
    fields.extend([
        ("public_source_entity_id", DataType::Utf8, true),
        ("public_occurrence_id", DataType::Utf8, true),
        ("relationship_kind", DataType::Utf8, false),
        ("reference_id", DataType::FixedSizeBinary(16), true),
        ("target_declaration_id", DataType::FixedSizeBinary(16), true),
        ("reference_name", DataType::Utf8, true),
        ("reference_kind", DataType::Utf8, true),
        ("resolution_scope", DataType::Utf8, true),
        ("raw_resolution", DataType::Utf8, true),
        (
            "provider_observation_id",
            DataType::FixedSizeBinary(16),
            true,
        ),
        (
            "provider_target_observation_id",
            DataType::FixedSizeBinary(16),
            true,
        ),
    ]);
    fields
}

fn absent(kind: &DataType) -> Result<super::Expr, TransformationPlanError> {
    Ok(lit(ScalarValue::try_from(kind)?))
}

pub(super) fn build(inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
    let calls = plan(inputs, super::call_selector::RELATION)?;
    let call_fields = calls.schema();
    let call_projection = fields()
        .iter()
        .map(|(name, kind, _)| {
            Ok(match *name {
                "public_source_entity_id" => col("public_caller_entity_id"),
                "public_occurrence_id" => col("public_call_site_id"),
                "relationship_kind" => lit("call"),
                _ if call_fields.field_with_unqualified_name(name).is_ok() => col(*name),
                _ => absent(kind)?,
            }
            .alias(*name))
        })
        .collect::<Result<Vec<_>, TransformationPlanError>>()?;
    let calls = LogicalPlanBuilder::from(calls)
        .project(call_projection)?
        .build()?;
    let targets = LogicalPlanBuilder::from(plan(inputs, super::ENTITY)?)
        .project([col("entity_id"), col("entity_kind"), col("context_id")])?
        .distinct()?
        .alias("t")?
        .build()?;
    let references = LogicalPlanBuilder::from(plan(inputs, super::REFERENCE)?)
        .alias("r")?
        .join(
            targets,
            JoinType::Left,
            (
                vec!["r.target_entity_id", "r.context_id"],
                vec!["t.entity_id", "t.context_id"],
            ),
            None,
        )?
        .build()?;
    let direction = |direction: &str| -> Result<LogicalPlan, TransformationPlanError> {
        let source_id = public_entity_id().call(vec![col("r.reference_id"), lit("reference")]);
        let target_id =
            public_entity_id().call(vec![col("r.target_entity_id"), col("t.entity_kind")]);
        let projection = fields()
            .iter()
            .map(|(name, kind, _)| {
                Ok(match *name {
                    "public_source_entity_id" | "public_occurrence_id" => source_id.clone(),
                    "public_target_entity_id" => target_id.clone(),
                    "public_entity_id" => {
                        if direction == "incoming" {
                            target_id.clone()
                        } else {
                            source_id.clone()
                        }
                    }
                    "subject_kind" => lit("entity"),
                    "fact_family" => lit("lexical-references"),
                    "direction" => lit(direction),
                    "distance" => lit("one relationship step"),
                    "relationship_kind" => lit("lexical-reference"),
                    "reference_name" => col("r.name"),
                    _ if references
                        .schema()
                        .field_with_qualified_name(&"r".into(), name)
                        .is_ok() =>
                    {
                        col(format!("r.{name}"))
                    }
                    _ => absent(kind)?,
                }
                .alias(*name))
            })
            .collect::<Result<Vec<_>, TransformationPlanError>>()?;
        Ok(LogicalPlanBuilder::from(references.clone())
            .project(projection)?
            .build()?)
    };
    Ok(LogicalPlanBuilder::from(calls)
        .union(direction("incoming")?)?
        .union(direction("outgoing")?)?
        .build()?)
}
