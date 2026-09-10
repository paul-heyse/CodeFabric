//! Canonical type evidence retains compiler owners/slots separately from source occurrences.

use super::super::super::{
    DECLARATION, DataType, Expr, JoinType, LogicalPlan, LogicalPlanBuilder, RUN, ScalarValue,
    TransformationInputs, TransformationPlanError, col, file_id_udf, lit, plan, rust_entity_id,
    source_alias, source_occurrence_id,
};
use super::super::{Relation, scope_fields, union};
use super::GRAPH;
use crate::rustc_relation_schema::RustcRelation;
use datafusion::logical_expr::when;

fn raw(
    inputs: &TransformationInputs,
    relation: RustcRelation,
) -> Result<LogicalPlanBuilder, TransformationPlanError> {
    Ok(LogicalPlanBuilder::from(plan(inputs, relation.relation_id())?).alias("p")?)
}

fn graph(
    inputs: &TransformationInputs,
    alias: &str,
) -> Result<LogicalPlan, TransformationPlanError> {
    Ok(LogicalPlanBuilder::from(plan(inputs, GRAPH)?)
        .alias(alias)?
        .build()?)
}

fn graph_join(alias: &str, key: &str) -> Vec<Expr> {
    vec![
        col("p.provider_run_id").eq(col(format!("{alias}.provider_run_identity"))),
        col("p.compilation_unit_id").eq(col(format!("{alias}.provider_compilation_unit"))),
        col("p.owner_id").eq(col(format!("{alias}.provider_owner"))),
        col("p.source_generation").eq(col(format!("{alias}.source_generation"))),
        col("p.source_content_digest").eq(col(format!("{alias}.content_digest"))),
        file_id_udf()
            .call(vec![col("p.source_file_id")])
            .eq(col(format!("{alias}.file_id"))),
        col(format!("p.{key}")).eq(col(format!("{alias}.type_key"))),
    ]
}

pub(in super::super) fn components(
    inputs: &TransformationInputs,
) -> Result<LogicalPlan, TransformationPlanError> {
    let joined = raw(inputs, RustcRelation::Type)?
        .filter(col("p.component_role").not_eq(lit("self")))?
        .join_on(
            graph(inputs, "g")?,
            JoinType::Inner,
            graph_join("g", "type_key"),
        )?
        .join_on(
            graph(inputs, "t")?,
            JoinType::Left,
            graph_join("t", "component_type_key"),
        )?;
    let mut projection = scope_fields()
        .iter()
        .map(|(name, _, _)| match *name {
            "language" => lit("rust").alias(*name),
            "provider" => lit("rustc").alias(*name),
            _ => col(format!("g.{name}")).alias(*name),
        })
        .collect::<Vec<_>>();
    let parameter = col("p.component_role").eq(lit("function-input"));
    projection.extend([
        col("g.type_id").alias("owner_type_id"),
        col("t.type_id").alias("referenced_type_id"),
        when(parameter.clone(), lit("parameter"))
            .when(
                col("p.component_role").eq(lit("function-output")),
                lit("return"),
            )
            .when(
                col("p.component_role").eq(lit("tuple-element")),
                lit("element"),
            )
            .when(
                col("p.component_role").eq(lit("generic-type-argument")),
                lit("type-argument"),
            )
            .otherwise(col("p.component_role"))?
            .alias("component_role"),
        when(
            col("p.component_role").eq(lit("function-output")),
            lit(0_u64),
        )
        .otherwise(col("p.component_ordinal") - lit(1_u64))?
        .alias("component_ordinal"),
        when(parameter.clone(), lit("positional-only"))
            .otherwise(lit(ScalarValue::Utf8(None)))?
            .alias("parameter_kind"),
        lit(ScalarValue::Utf8(None)).alias("parameter_name"),
        when(parameter, lit(true))
            .otherwise(lit(ScalarValue::Boolean(None)))?
            .alias("parameter_required"),
        when(
            col("g.unknown_reason").is_not_null(),
            col("g.unknown_reason"),
        )
        .when(
            col("t.type_key").is_null(),
            lit("native_type_component_unavailable"),
        )
        .otherwise(col("t.unknown_reason"))?
        .alias("unknown_reason"),
        lit(ScalarValue::UInt64(None)).alias("owner_local_type_index"),
        lit(ScalarValue::UInt64(None)).alias("referenced_local_type_index"),
        col("g.provider_owner").alias("provider_owner"),
        col("g.provider_compilation_unit").alias("provider_compilation_unit"),
        col("p.type_key").alias("provider_owner_type_key"),
        col("p.component_type_key").alias("provider_referenced_type_key"),
        col("p.component_ordinal").alias("provider_component_ordinal"),
    ]);
    Ok(joined.project(projection)?.distinct()?.build()?)
}

pub(in super::super) fn observations(
    workspace: [u8; 16],
    inputs: &TransformationInputs,
    locals: bool,
) -> Result<LogicalPlan, TransformationPlanError> {
    let mut plans = vec![observation(workspace, inputs, false)?];
    if locals {
        plans.push(observation(workspace, inputs, true)?);
    }
    union(Relation::Observation, plans.into_iter())
}

#[allow(
    clippy::too_many_lines,
    reason = "one observation projection keeps source-location validity independent of native owner and type identity"
)]
fn observation(
    workspace: [u8; 16],
    inputs: &TransformationInputs,
    local: bool,
) -> Result<LogicalPlan, TransformationPlanError> {
    let owners = LogicalPlanBuilder::from(plan(inputs, RustcRelation::PublicItem.relation_id())?)
        .alias("o")?
        .build()?;
    let runs = LogicalPlanBuilder::from(plan(inputs, RUN)?)
        .filter(col("provider").eq(lit("rustc")))?
        .alias("r")?
        .build()?;
    let declarations = LogicalPlanBuilder::from(plan(inputs, DECLARATION)?)
        .filter(col("language").eq(lit("rust")))?
        .project(
            [
                "entity_id",
                "context_id",
                "file_id",
                "content_digest",
                "source_generation",
            ]
            .map(col),
        )?
        .distinct()?
        .alias("d")?
        .build()?;
    let owner_id = rust_entity_id(workspace).call(vec![
        col("r.context_id"),
        col("o.stable_crate_id"),
        col("o.def_path_hash"),
        col("o.item_kind"),
    ]);
    let file = file_id_udf().call(vec![col("p.source_file_id")]);
    let joined = raw(
        inputs,
        if local {
            RustcRelation::MirLocal
        } else {
            RustcRelation::PublicItem
        },
    )?
    .join(
        owners,
        JoinType::Inner,
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
        runs,
        JoinType::Inner,
        (
            vec!["p.provider_run_id", "p.source_generation"],
            vec!["r.provider_run_identity", "r.source_generation"],
        ),
        None,
    )?
    .join_on(
        source_alias(inputs)?,
        JoinType::Inner,
        vec![
            file.clone().eq(col("s.file_id")),
            col("p.source_content_digest").eq(col("s.content_digest")),
            col("p.source_generation").eq(col("s.source_generation")),
        ],
    )?
    .join_on(
        graph(inputs, "g")?,
        JoinType::Left,
        graph_join("g", "type_key"),
    )?
    .join_on(
        declarations,
        JoinType::Left,
        vec![
            owner_id.eq(col("d.entity_id")),
            col("r.context_id").eq(col("d.context_id")),
            col("s.file_id").eq(col("d.file_id")),
            col("s.content_digest").eq(col("d.content_digest")),
            col("s.source_generation").eq(col("d.source_generation")),
        ],
    )?;
    let located = col("p.span_file_bytes")
        .is_not_null()
        .and(col("p.span_file_bytes").eq(col("o.span_file_bytes")))
        .and(col("p.expansion_kind").eq(lit("source-authored")))
        .and(col("p.span_start_byte").lt(col("p.span_end_byte")))
        .and(col("p.span_end_byte").lt_eq(col("s.byte_length")));
    let location =
        |value: Expr, datatype: DataType| -> Result<Expr, datafusion::common::DataFusionError> {
            when(located.clone(), value).otherwise(lit(ScalarValue::try_from(&datatype)?))
        };
    let occurrence = if local {
        // MIR slots are executable observations. Their indices do not mint source occurrence IDs.
        lit(ScalarValue::FixedSizeBinary(16, None))
    } else {
        location(
            source_occurrence_id(workspace, "codefabric_rust_type_occurrence_id_v1", 104, 5).call(
                vec![
                    col("d.entity_id"),
                    file.clone(),
                    col("p.source_content_digest"),
                    col("p.span_start_byte"),
                    col("p.span_end_byte"),
                ],
            ),
            DataType::FixedSizeBinary(16),
        )?
    };
    let role = if local {
        when(
            col("p.local_role").eq(lit("argument")),
            lit("mir-argument-type"),
        )
        .when(
            col("p.local_role").eq(lit("return")),
            lit("mir-return-type"),
        )
        .otherwise(lit("mir-local-type"))?
    } else {
        lit("compiler-item-type")
    };
    Ok(joined
        .project(vec![
            lit("rust").alias("language"),
            col("r.context_id").alias("context_id"),
            file.alias("file_id"),
            col("p.source_content_digest").alias("content_digest"),
            col("p.source_generation").alias("source_generation"),
            col("r.provider_run_id").alias("provider_run_id"),
            lit("rustc").alias("provider"),
            col("s.workspace_id").alias("workspace_id"),
            occurrence.alias("type_occurrence_id"),
            col("g.type_id").alias("type_id"),
            role.alias("type_role"),
            location(col("p.span_start_byte"), DataType::UInt64)?.alias("start_byte"),
            location(col("p.span_end_byte"), DataType::UInt64)?.alias("end_byte"),
            when(col("d.entity_id").is_null(), lit("type_owner_unavailable"))
                .when(
                    located.is_not_true(),
                    lit("compiler_type_location_unavailable"),
                )
                .when(
                    col("g.type_key").is_null(),
                    lit("native_type_node_unavailable"),
                )
                .otherwise(col("g.unknown_reason"))?
                .alias("unknown_reason"),
            (if local {
                col("p.local_index")
            } else {
                lit(0_u64)
            })
            .alias("provider_occurrence_ordinal"),
            lit(if local {
                "mir-local"
            } else {
                "item-declaration"
            })
            .alias("provider_occurrence_kind"),
            col("d.entity_id").alias("owner_entity_id"),
            col("p.owner_id").alias("provider_owner"),
            col("p.compilation_unit_id").alias("provider_compilation_unit"),
            col("p.type_key").alias("provider_type_key"),
            lit(ScalarValue::UInt64(None)).alias("provider_local_type_index"),
        ])?
        .build()?)
}
