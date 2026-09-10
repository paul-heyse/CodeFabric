//! Native joins select one exact compiler owner before grouping its type observations.

use super::super::super::{
    DECLARATION, DataType, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder, RUN,
    TransformationInputs, TransformationPlanError, col, empty, file_id_udf, lit, plan,
    rust_entity_id, source_alias,
};
use super::udf;
use crate::rustc_relation_schema::RustcRelation;
use datafusion::functions::core::expr_fn::{get_field, named_struct};
use datafusion::functions_aggregate::array_agg::array_agg;

pub(in super::super) const GRAPH: &str = "system.canonical_rust_type_graph";

const SCOPE: [&str; 9] = [
    "workspace_id",
    "context_id",
    "file_id",
    "content_digest",
    "source_generation",
    "provider_run_id",
    "provider_run_identity",
    "provider_owner",
    "provider_compilation_unit",
];

pub(in super::super) fn fields() -> Vec<FieldSpec> {
    vec![
        ("workspace_id", DataType::FixedSizeBinary(16), false),
        ("context_id", DataType::FixedSizeBinary(16), false),
        ("file_id", DataType::FixedSizeBinary(16), true),
        ("content_digest", DataType::FixedSizeBinary(32), false),
        ("source_generation", DataType::UInt64, false),
        ("provider_run_id", DataType::FixedSizeBinary(16), false),
        ("provider_run_identity", DataType::Utf8, false),
        ("provider_owner", DataType::Utf8, false),
        ("provider_compilation_unit", DataType::Utf8, false),
        ("type_key", DataType::FixedSizeBinary(32), true),
        ("type_id", DataType::FixedSizeBinary(16), true),
        ("type_kind_code", DataType::Int32, true),
        ("canonical_key", DataType::Utf8, true),
        ("unknown_reason", DataType::Utf8, true),
    ]
}

#[allow(
    clippy::too_many_lines,
    reason = "one compiler graph plan keeps exact scope joins, typed aggregation and normalization together"
)]
pub(in super::super) fn build(
    workspace: [u8; 16],
    inputs: &TransformationInputs,
    available: bool,
) -> Result<LogicalPlan, TransformationPlanError> {
    if !available {
        return empty(fields());
    }
    let raw = LogicalPlanBuilder::from(plan(inputs, RustcRelation::Type.relation_id())?)
        .alias("p")?
        .build()?;
    let runs = LogicalPlanBuilder::from(plan(inputs, RUN)?)
        .filter(col("provider").eq(lit("rustc")))?
        .alias("r")?
        .build()?;
    let declarations = LogicalPlanBuilder::from(plan(inputs, DECLARATION)?)
        .filter(col("language").eq(lit("rust")))?
        .project(["entity_id", "context_id", "source_generation"].map(col))?
        .distinct()?
        .alias("d")?
        .build()?;
    let joined = LogicalPlanBuilder::from(raw)
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
                file_id_udf()
                    .call(vec![col("p.source_file_id")])
                    .eq(col("s.file_id")),
                col("p.source_content_digest").eq(col("s.content_digest")),
                col("p.source_generation").eq(col("s.source_generation")),
            ],
        )?
        .join_on(
            declarations,
            JoinType::Left,
            vec![
                rust_entity_id(workspace)
                    .call(vec![
                        col("r.context_id"),
                        col("p.definition_stable_crate_id"),
                        col("p.definition_def_path_hash"),
                        col("p.definition_kind"),
                    ])
                    .eq(col("d.entity_id")),
                col("r.context_id").eq(col("d.context_id")),
                col("p.source_generation").eq(col("d.source_generation")),
            ],
        )?;
    let mut record = [
        "type_key",
        "type_kind",
        "component_role",
        "component_ordinal",
        "component_type_key",
        "primitive_kind",
        "generic_argument_count",
        "array_length",
        "mutability",
        "region_kind",
        "bound_variable_count",
        "function_abi",
        "function_abi_unwind",
        "function_unsafe",
        "function_variadic",
    ]
    .into_iter()
    .flat_map(|name| [lit(name), col(format!("p.{name}"))])
    .collect::<Vec<_>>();
    record.extend([lit("definition_entity_id"), col("d.entity_id")]);
    let grouped = joined
        .aggregate(
            vec![
                col("s.workspace_id").alias("workspace_id"),
                col("r.context_id").alias("context_id"),
                col("s.file_id").alias("file_id"),
                col("p.source_content_digest").alias("content_digest"),
                col("p.source_generation").alias("source_generation"),
                col("r.provider_run_id").alias("provider_run_id"),
                col("r.provider_run_identity").alias("provider_run_identity"),
                col("p.owner_id").alias("provider_owner"),
                col("p.compilation_unit_id").alias("provider_compilation_unit"),
            ],
            vec![array_agg(named_struct(record)).alias("nodes")],
        )?
        .build()?;
    let input_type = grouped
        .schema()
        .field_with_unqualified_name("nodes")?
        .data_type()
        .clone();
    Ok(LogicalPlanBuilder::from(grouped)
        .project(
            SCOPE
                .map(col)
                .into_iter()
                .chain([udf::normalizer(input_type)
                    .call(vec![col("workspace_id"), col("context_id"), col("nodes")])
                    .alias("normalized")]),
        )?
        .unnest_column("normalized")?
        .project(
            SCOPE.map(col).into_iter().chain(
                [
                    "type_key",
                    "type_id",
                    "type_kind_code",
                    "canonical_key",
                    "unknown_reason",
                ]
                .map(|name| get_field(col("normalized"), name).alias(name)),
            ),
        )?
        .build()?)
}
