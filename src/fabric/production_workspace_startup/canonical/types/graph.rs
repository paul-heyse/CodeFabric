//! DataFusion binds exact runs, source and declaration anchors before graph normalization.

use super::{
    DECLARATION, DataType, Expr, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder,
    PyreflyRelation, RUN, TransformationInputs, TransformationPlanError, array_agg, col,
    count_distinct, file_id_udf, get_field, lit, named_struct, plan, raw, source_alias, udf,
};

const SCOPE: [&str; 7] = [
    "workspace_id",
    "context_id",
    "file_id",
    "content_digest",
    "source_generation",
    "provider_run_id",
    "provider_run_identity",
];

pub(super) fn fields() -> Vec<FieldSpec> {
    let mut fields = vec![
        ("workspace_id", DataType::FixedSizeBinary(16), false),
        ("context_id", DataType::FixedSizeBinary(16), false),
        ("file_id", DataType::FixedSizeBinary(16), true),
        ("content_digest", DataType::FixedSizeBinary(32), false),
        ("source_generation", DataType::UInt64, false),
        ("provider_run_id", DataType::FixedSizeBinary(16), false),
        ("provider_run_identity", DataType::Utf8, false),
    ];
    fields.extend([
        ("local_type_index", DataType::UInt64, true),
        ("type_id", DataType::FixedSizeBinary(16), true),
        ("type_kind_code", DataType::Int32, true),
        ("canonical_key", DataType::Utf8, true),
        ("unknown_reason", DataType::Utf8, true),
    ]);
    fields
}

pub(super) fn accepted(
    raw: LogicalPlanBuilder,
    inputs: &TransformationInputs,
) -> Result<LogicalPlanBuilder, TransformationPlanError> {
    let runs = LogicalPlanBuilder::from(plan(inputs, RUN)?)
        .filter(col("provider").eq(lit("pyrefly")))?
        .alias("r")?
        .build()?;
    Ok(raw
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
                    .call(vec![col("p.file_id")])
                    .eq(col("s.file_id")),
                col("p.content_digest").eq(col("s.content_digest")),
                col("p.source_generation").eq(col("s.source_generation")),
            ],
        )?)
}

fn definitions(inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
    let anchors = [
        "context_id",
        "file_id",
        "content_digest",
        "source_generation",
        "start_byte",
        "end_byte",
    ];
    let declarations = LogicalPlanBuilder::from(plan(inputs, DECLARATION)?)
        .filter(col("language").eq(lit("python")))?
        .build()?;
    let unique = LogicalPlanBuilder::from(declarations.clone())
        .aggregate(
            anchors.map(col).to_vec(),
            vec![count_distinct(col("entity_id")).alias("candidates")],
        )?
        .filter(col("candidates").eq(lit(1_i64)))?
        .alias("a")?
        .build()?;
    // Only an unambiguous canonical definition can contribute nominal type identity.
    Ok(LogicalPlanBuilder::from(declarations)
        .alias("d")?
        .join(
            unique,
            JoinType::Inner,
            (
                anchors.map(|name| format!("d.{name}")).to_vec(),
                anchors.map(|name| format!("a.{name}")).to_vec(),
            ),
            None,
        )?
        .project(
            anchors
                .into_iter()
                .chain(["entity_id"])
                .map(|name| col(format!("d.{name}")).alias(name)),
        )?
        .distinct()?
        .alias("d")?
        .build()?)
}

fn record(alias: &str, fields: &[&str]) -> Expr {
    named_struct(
        fields
            .iter()
            .flat_map(|name| [lit(*name), col(format!("{alias}.{name}"))])
            .collect(),
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one graph plan keeps exact scope joins, typed aggregation and normalization together"
)]
pub(super) fn build(inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
    let nodes = accepted(raw(inputs, PyreflyRelation::TypeNode)?, inputs)?.join_on(
        definitions(inputs)?,
        JoinType::Left,
        vec![
            col("r.context_id").eq(col("d.context_id")),
            file_id_udf()
                .call(vec![col("p.definition_file_id")])
                .eq(col("d.file_id")),
            col("p.definition_content_digest").eq(col("d.content_digest")),
            col("p.source_generation").eq(col("d.source_generation")),
            col("p.definition_start_byte").eq(col("d.start_byte")),
            col("p.definition_end_byte").eq(col("d.end_byte")),
        ],
    )?;
    let fields = [
        "local_type_index",
        "type_kind",
        "intrinsic",
        "style",
        "literal_kind",
        "literal_text",
        "literal_bytes",
        "literal_boolean",
    ];
    let mut record_fields = fields
        .iter()
        .flat_map(|name| [lit(*name), col(format!("p.{name}"))])
        .collect::<Vec<_>>();
    record_fields.extend([lit("definition_entity_id"), col("d.entity_id")]);
    let nodes = nodes
        .aggregate(
            vec![
                col("s.workspace_id").alias("workspace_id"),
                col("r.context_id").alias("context_id"),
                col("s.file_id").alias("file_id"),
                col("p.content_digest").alias("content_digest"),
                col("p.source_generation").alias("source_generation"),
                col("r.provider_run_id").alias("provider_run_id"),
                col("r.provider_run_identity").alias("provider_run_identity"),
            ],
            vec![array_agg(named_struct(record_fields)).alias("nodes")],
        )?
        .alias("n")?
        .build()?;
    let edges = raw(inputs, PyreflyRelation::TypeEdge)?
        .aggregate(
            vec![
                col("p.provider_run_id").alias("raw_run"),
                file_id_udf().call(vec![col("p.file_id")]).alias("file_id"),
                col("p.content_digest").alias("content_digest"),
                col("p.source_generation").alias("source_generation"),
            ],
            vec![
                array_agg(record(
                    "p",
                    &[
                        "owner_local_type_index",
                        "referenced_local_type_index",
                        "component_role",
                        "component_ordinal",
                        "parameter_kind",
                        "parameter_name",
                        "parameter_required",
                    ],
                ))
                .alias("edges"),
            ],
        )?
        .alias("e")?
        .build()?;
    let node_type = nodes
        .schema()
        .field_with_unqualified_name("nodes")?
        .data_type()
        .clone();
    let edge_type = edges
        .schema()
        .field_with_unqualified_name("edges")?
        .data_type()
        .clone();
    let normalized = udf::normalizer(node_type, edge_type)
        .call(vec![
            col("n.workspace_id"),
            col("n.context_id"),
            col("n.nodes"),
            col("e.edges"),
        ])
        .alias("normalized");
    let joined = LogicalPlanBuilder::from(nodes)
        .join(
            edges,
            JoinType::Left,
            (
                vec![
                    "n.provider_run_identity",
                    "n.file_id",
                    "n.content_digest",
                    "n.source_generation",
                ],
                vec![
                    "e.raw_run",
                    "e.file_id",
                    "e.content_digest",
                    "e.source_generation",
                ],
            ),
            None,
        )?
        .project(
            SCOPE
                .map(|name| col(format!("n.{name}")).alias(name))
                .into_iter()
                .chain([normalized]),
        )?
        .unnest_column("normalized")?;
    Ok(joined
        .project(
            SCOPE.map(col).into_iter().chain(
                [
                    "local_type_index",
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
