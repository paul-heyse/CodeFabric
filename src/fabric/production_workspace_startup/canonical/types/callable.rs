//! Callable evidence uses native definition anchors or MIR owner/slot identity, never file proximity.

use super::{
    COMPONENT, DECLARATION, DataType, Expr, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder,
    OBSERVATION, Relation, ScalarValue, TransformationInputs, TransformationPlanError, col,
    file_id_udf, graph, lit, plan, scope_fields, union, when,
};

pub(super) fn fields() -> Vec<FieldSpec> {
    let mut fields = scope_fields();
    fields.extend([
        ("owner_entity_id", DataType::FixedSizeBinary(16), true),
        ("type_id", DataType::FixedSizeBinary(16), true),
        ("callable_type_id", DataType::FixedSizeBinary(16), true),
        ("type_role", DataType::Utf8, false),
        ("component_ordinal", DataType::UInt64, true),
        ("parameter_kind", DataType::Utf8, true),
        ("parameter_name", DataType::Utf8, true),
        ("parameter_required", DataType::Boolean, true),
        ("evidence_kind", DataType::Utf8, false),
        ("unknown_reason", DataType::Utf8, true),
        ("provider_owner", DataType::Utf8, true),
        ("provider_compilation_unit", DataType::Utf8, true),
        ("provider_type_key", DataType::FixedSizeBinary(32), true),
        ("provider_local_type_index", DataType::UInt64, true),
    ]);
    fields
}

pub(super) fn build(
    inputs: &TransformationInputs,
    python_available: bool,
) -> Result<LogicalPlan, TransformationPlanError> {
    let mut branches = vec![rust(inputs)?];
    if python_available {
        branches.push(python(inputs)?);
    }
    let observed = union(Relation::Callable, branches.into_iter())?;
    let missing = missing(plan(inputs, DECLARATION)?, observed.clone())?;
    union(Relation::Callable, [observed, missing].into_iter())
}

fn scope(alias: &str) -> Vec<Expr> {
    scope_fields()
        .iter()
        .map(|(name, _, _)| col(format!("{alias}.{name}")).alias(*name))
        .collect()
}

fn python(inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
    let components = LogicalPlanBuilder::from(plan(inputs, COMPONENT)?)
        .filter(col("language").eq(lit("python")))?
        .filter(col("component_role").in_list(vec![lit("parameter"), lit("return")], false))?
        .alias("c")?
        .build()?;
    let joined = graph::definition_nodes(inputs)?
        .filter(col("p.type_kind").eq(lit("function")))?
        .filter(col("d.entity_id").is_not_null())?
        .join_on(
            components,
            JoinType::Inner,
            vec![
                col("r.context_id").eq(col("c.context_id")),
                col("s.workspace_id").eq(col("c.workspace_id")),
                col("r.provider_run_id").eq(col("c.provider_run_id")),
                file_id_udf()
                    .call(vec![col("p.file_id")])
                    .eq(col("c.file_id")),
                col("p.content_digest").eq(col("c.content_digest")),
                col("p.source_generation").eq(col("c.source_generation")),
                col("p.local_type_index").eq(col("c.owner_local_type_index")),
            ],
        )?;
    let mut projection = scope("c");
    projection.extend([
        col("d.entity_id").alias("owner_entity_id"),
        col("c.referenced_type_id").alias("type_id"),
        col("c.owner_type_id").alias("callable_type_id"),
        col("c.component_role").alias("type_role"),
        col("c.component_ordinal").alias("component_ordinal"),
        col("c.parameter_kind").alias("parameter_kind"),
        col("c.parameter_name").alias("parameter_name"),
        col("c.parameter_required").alias("parameter_required"),
        lit("checker-function-type").alias("evidence_kind"),
        col("c.unknown_reason").alias("unknown_reason"),
        col("c.provider_owner").alias("provider_owner"),
        col("c.provider_compilation_unit").alias("provider_compilation_unit"),
        col("c.provider_referenced_type_key").alias("provider_type_key"),
        col("c.owner_local_type_index").alias("provider_local_type_index"),
    ]);
    Ok(joined.project(projection)?.distinct()?.build()?)
}

fn rust(inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
    let parameter = col("o.type_role").eq(lit("mir-argument-type"));
    let mut projection = scope("o");
    projection.extend([
        col("o.owner_entity_id").alias("owner_entity_id"),
        col("o.type_id").alias("type_id"),
        lit(ScalarValue::FixedSizeBinary(16, None)).alias("callable_type_id"),
        when(parameter.clone(), lit("parameter"))
            .otherwise(lit("return"))?
            .alias("type_role"),
        when(
            parameter.clone(),
            col("o.provider_occurrence_ordinal") - lit(1_u64),
        )
        .otherwise(lit(0_u64))?
        .alias("component_ordinal"),
        when(parameter.clone(), lit("positional-only"))
            .otherwise(lit(ScalarValue::Utf8(None)))?
            .alias("parameter_kind"),
        lit(ScalarValue::Utf8(None)).alias("parameter_name"),
        when(parameter, lit(true))
            .otherwise(lit(ScalarValue::Boolean(None)))?
            .alias("parameter_required"),
        lit("compiler-mir-slot").alias("evidence_kind"),
        col("o.unknown_reason").alias("unknown_reason"),
        col("o.provider_owner").alias("provider_owner"),
        col("o.provider_compilation_unit").alias("provider_compilation_unit"),
        col("o.provider_type_key").alias("provider_type_key"),
        col("o.provider_local_type_index").alias("provider_local_type_index"),
    ]);
    Ok(LogicalPlanBuilder::from(plan(inputs, OBSERVATION)?)
        .alias("o")?
        .filter(col("o.language").eq(lit("rust")))?
        .filter(col("o.owner_entity_id").is_not_null())?
        .filter(col("o.type_role").in_list(
            vec![lit("mir-argument-type"), lit("mir-return-type")],
            false,
        ))?
        .project(projection)?
        .distinct()?
        .build()?)
}

fn missing(
    declarations: LogicalPlan,
    observed: LogicalPlan,
) -> Result<LogicalPlan, TransformationPlanError> {
    let known = LogicalPlanBuilder::from(observed).alias("k")?.build()?;
    let mut projection = scope("d");
    projection.extend(fields().into_iter().skip(scope_fields().len()).map(
        |(name, datatype, _)| {
            let value = match name {
                "owner_entity_id" => col("d.entity_id"),
                "type_role" => lit("unknown"),
                "evidence_kind" => lit("canonical-declaration-without-callable-evidence"),
                "unknown_reason" => lit("callable_type_evidence_unavailable"),
                _ => lit(ScalarValue::try_from(&datatype).expect("closed callable scalar schema")),
            };
            value.alias(name)
        },
    ));
    Ok(LogicalPlanBuilder::from(declarations)
        .alias("d")?
        .filter(col("d.entity_id").is_not_null())?
        .filter(col("d.entity_kind").in_list(vec![lit("function"), lit("method")], false))?
        .join_on(
            known,
            JoinType::LeftAnti,
            vec![
                col("d.entity_id").eq(col("k.owner_entity_id")),
                col("d.context_id").eq(col("k.context_id")),
                col("d.workspace_id").eq(col("k.workspace_id")),
            ],
        )?
        .project(projection)?
        .distinct()?
        .build()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{
        Array, ArrayRef, FixedSizeBinaryArray, RecordBatch, StringArray, UInt64Array,
    };
    use datafusion::prelude::SessionContext;
    use std::sync::Arc;

    #[tokio::test]
    #[allow(
        clippy::too_many_lines,
        reason = "one native fixture verifies independent owner/context/workspace absence and typed unknown rows"
    )]
    async fn absent_callable_evidence_is_unknown_with_exact_owner_context_and_workspace() {
        let ids = |values: &[u8]| -> ArrayRef {
            Arc::new(
                FixedSizeBinaryArray::try_from_iter(values.iter().map(|value| [*value; 16]))
                    .unwrap(),
            )
        };
        let mut columns = scope_fields()
            .into_iter()
            .map(|(name, datatype, _)| {
                let column: ArrayRef = match name {
                    "workspace_id" => ids(&[1, 1, 2, 1, 1]),
                    "context_id" => ids(&[1, 2, 1, 1, 1]),
                    _ => match datatype {
                        DataType::FixedSizeBinary(size) => Arc::new(
                            FixedSizeBinaryArray::try_from_iter(
                                (0..5).map(|_| vec![3; usize::try_from(size).unwrap()]),
                            )
                            .unwrap(),
                        ),
                        DataType::UInt64 => Arc::new(UInt64Array::from(vec![1; 5])),
                        DataType::Utf8 => Arc::new(StringArray::from(vec!["python"; 5])),
                        _ => unreachable!(),
                    },
                };
                (name, column)
            })
            .collect::<Vec<_>>();
        columns.extend([
            ("entity_id", ids(&[10, 10, 10, 11, 12])),
            (
                "entity_kind",
                Arc::new(StringArray::from(vec![
                    "function", "function", "method", "function", "class",
                ])),
            ),
        ]);
        let context = SessionContext::new();
        context
            .register_batch("declarations", RecordBatch::try_from_iter(columns).unwrap())
            .unwrap();
        context
            .register_batch(
                "observed",
                RecordBatch::try_from_iter([
                    ("owner_entity_id", ids(&[10, 10])),
                    ("context_id", ids(&[1, 1])),
                    ("workspace_id", ids(&[1, 1])),
                ])
                .unwrap(),
            )
            .unwrap();
        let output = missing(
            context
                .table("declarations")
                .await
                .unwrap()
                .into_optimized_plan()
                .unwrap(),
            context
                .table("observed")
                .await
                .unwrap()
                .into_optimized_plan()
                .unwrap(),
        )
        .unwrap();
        let batches = context
            .execute_logical_plan(output)
            .await
            .unwrap()
            .collect()
            .await
            .unwrap();
        assert_eq!(batches.iter().map(RecordBatch::num_rows).sum::<usize>(), 3);
        let mut actual = std::collections::BTreeSet::new();
        for batch in batches {
            for row in 0..batch.num_rows() {
                let id = |name| {
                    batch
                        .column_by_name(name)
                        .unwrap()
                        .as_any()
                        .downcast_ref::<FixedSizeBinaryArray>()
                        .unwrap()
                        .value(row)[0]
                };
                actual.insert((id("owner_entity_id"), id("context_id"), id("workspace_id")));
                assert!(batch.column_by_name("type_id").unwrap().is_null(row));
                assert_eq!(
                    batch
                        .column_by_name("unknown_reason")
                        .unwrap()
                        .as_any()
                        .downcast_ref::<StringArray>()
                        .unwrap()
                        .value(row),
                    "callable_type_evidence_unavailable"
                );
            }
        }
        assert_eq!(
            actual,
            std::collections::BTreeSet::from([(10, 2, 1), (10, 1, 2), (11, 1, 1)])
        );
    }
}
