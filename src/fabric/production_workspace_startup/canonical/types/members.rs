//! Native associated members retain exact class ownership and distinct declared/computed types.

use super::{
    DECLARATION, DataType, Expr, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder,
    PyreflyRelation, Relation, ScalarValue, TransformationInputs, TransformationPlanError, col,
    file_id_udf, graph, graph_alias, graph_join, lit, plan, raw, scope_fields, union, when,
};

pub(super) fn fields() -> Vec<FieldSpec> {
    let mut fields = scope_fields();
    fields.extend([
        ("owner_entity_id", DataType::FixedSizeBinary(16), true),
        ("member_entity_id", DataType::FixedSizeBinary(16), true),
        ("record_kind", DataType::Utf8, false),
        ("class_name", DataType::Utf8, false),
        ("class_start_byte", DataType::UInt64, true),
        ("class_end_byte", DataType::UInt64, true),
        ("class_expected_members", DataType::UInt64, true),
        ("class_census_complete", DataType::Boolean, false),
        ("member_ordinal", DataType::UInt64, true),
        ("member_name", DataType::Utf8, true),
        ("member_start_byte", DataType::UInt64, true),
        ("member_end_byte", DataType::UInt64, true),
        ("definition_kind", DataType::Utf8, true),
        ("native_kind", DataType::Utf8, true),
        ("declared_type_id", DataType::FixedSizeBinary(16), true),
        ("computed_type_id", DataType::FixedSizeBinary(16), true),
        ("annotation_present", DataType::Boolean, true),
        ("is_property", DataType::Boolean, true),
        ("is_class_var", DataType::Boolean, true),
        ("is_final", DataType::Boolean, true),
        ("is_abstract", DataType::Boolean, true),
        ("descriptor_has_set", DataType::Boolean, true),
        ("descriptor_has_delete", DataType::Boolean, true),
        ("member_mapping", DataType::Utf8, true),
        ("unknown_reason", DataType::Utf8, true),
        ("provider_class_ordinal", DataType::UInt64, true),
        ("provider_declared_type_index", DataType::UInt64, true),
        ("provider_computed_type_index", DataType::UInt64, true),
    ]);
    fields
}

pub(super) fn build(
    inputs: &TransformationInputs,
    python_available: bool,
) -> Result<LogicalPlan, TransformationPlanError> {
    let observed = union(
        Relation::Member,
        if python_available {
            Some(python(inputs)?)
        } else {
            None
        }
        .into_iter(),
    )?;
    let missing = missing(plan(inputs, DECLARATION)?, observed.clone())?;
    union(Relation::Member, [observed, missing].into_iter())
}

fn definition_join(alias: &str, prefix: &str) -> Vec<Expr> {
    vec![
        col("s.workspace_id").eq(col(format!("{alias}.workspace_id"))),
        col("r.context_id").eq(col(format!("{alias}.context_id"))),
        col("s.file_id").eq(col(format!("{alias}.file_id"))),
        col("p.content_digest").eq(col(format!("{alias}.content_digest"))),
        col("p.source_generation").eq(col(format!("{alias}.source_generation"))),
        col(format!("p.{prefix}_start_byte")).eq(col(format!("{alias}.start_byte"))),
        col(format!("p.{prefix}_end_byte")).eq(col(format!("{alias}.end_byte"))),
    ]
}

fn python(inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
    let mut joined = graph::accepted(raw(inputs, PyreflyRelation::MemberObservation)?, inputs)?;
    for (alias, prefix) in [("d", "class"), ("m", "member")] {
        let definitions = LogicalPlanBuilder::from(graph::definitions(inputs)?)
            .alias(alias)?
            .build()?;
        joined = joined.join_on(definitions, JoinType::Left, definition_join(alias, prefix))?;
    }
    for (alias, index) in [
        ("dt", "declared_local_type_index"),
        ("ct", "computed_local_type_index"),
    ] {
        let mut predicates = graph_join(alias, index);
        predicates.extend([
            col("s.workspace_id").eq(col(format!("{alias}.workspace_id"))),
            col("r.context_id").eq(col(format!("{alias}.context_id"))),
        ]);
        joined = joined.join_on(graph_alias(inputs, alias)?, JoinType::Left, predicates)?;
    }
    let member = col("p.record_kind").eq(lit("member"));
    let unknown = when(
        col("d.entity_id").is_null(),
        lit("canonical_class_owner_unavailable"),
    )
    .when(
        col("p.class_unknown_reason").is_not_null(),
        col("p.class_unknown_reason"),
    )
    .when(
        col("p.unknown_reason").is_not_null(),
        col("p.unknown_reason"),
    )
    .when(
        member.clone().and(col("ct.type_id").is_null()),
        lit("canonical_computed_member_type_unknown"),
    )
    .when(
        member
            .clone()
            .and(col("p.annotation_present").eq(lit(true)))
            .and(col("dt.type_id").is_null()),
        lit("canonical_declared_member_type_unknown"),
    )
    .otherwise(lit(ScalarValue::Utf8(None)))?;
    let projection = fields()
        .into_iter()
        .map(|(name, _, _)| {
            let value = match name {
                "language" => lit("python"),
                "provider" => lit("pyrefly"),
                "context_id" | "provider_run_id" => col(format!("r.{name}")),
                "workspace_id" => col("s.workspace_id"),
                "file_id" => file_id_udf().call(vec![col("p.file_id")]),
                "owner_entity_id" => col("d.entity_id"),
                "member_entity_id" => col("m.entity_id"),
                "declared_type_id" => col("dt.type_id"),
                "computed_type_id" => col("ct.type_id"),
                "unknown_reason" => unknown.clone(),
                "member_mapping" => when(
                    member.clone().and(col("m.entity_id").is_not_null()),
                    lit("canonical-declaration"),
                )
                .when(member.clone(), lit("native-associated-member"))
                .otherwise(lit(ScalarValue::Utf8(None)))?,
                "provider_class_ordinal" => col("p.class_ordinal"),
                "provider_declared_type_index" => col("p.declared_local_type_index"),
                "provider_computed_type_index" => col("p.computed_local_type_index"),
                _ => col(format!("p.{name}")),
            };
            Ok(value.alias(name))
        })
        .collect::<Result<Vec<_>, datafusion::common::DataFusionError>>()?;
    Ok(joined.project(projection)?.distinct()?.build()?)
}

fn missing(
    declarations: LogicalPlan,
    observed: LogicalPlan,
) -> Result<LogicalPlan, TransformationPlanError> {
    let known = LogicalPlanBuilder::from(observed).alias("k")?.build()?;
    let mut projection = scope_fields()
        .into_iter()
        .map(|(name, _, _)| col(format!("d.{name}")).alias(name))
        .collect::<Vec<_>>();
    projection.extend(fields().into_iter().skip(scope_fields().len()).map(
        |(name, datatype, _)| {
            let value = match name {
                "owner_entity_id" => col("d.entity_id"),
                "record_kind" => lit("class"),
                "class_name" => col("d.name"),
                "class_start_byte" => col("d.start_byte"),
                "class_end_byte" => col("d.end_byte"),
                "class_census_complete" => lit(false),
                "unknown_reason" => lit("associated_member_census_unavailable"),
                _ => lit(ScalarValue::try_from(&datatype).expect("closed member scalar schema")),
            };
            value.alias(name)
        },
    ));
    Ok(LogicalPlanBuilder::from(declarations)
        .alias("d")?
        .filter(col("d.entity_id").is_not_null())?
        .filter(col("d.entity_kind").in_list(
            vec![lit("class"), lit("struct"), lit("enum"), lit("trait")],
            false,
        ))?
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
