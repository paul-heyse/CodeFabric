//! Semantic denotations retain all checker candidates and their exact source validity.

use super::{
    DECLARATION, DataType, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder, RUN, SOURCE,
    ScalarValue, TransformationInputs, TransformationPlanError, col, empty, file_id_udf, lit,
    modules, plan, source_alias, source_occurrence_id,
};
use crate::pyrefly_service::PyreflyRelation;
use datafusion::functions::core::expr_fn::coalesce;
use datafusion::functions_aggregate::count::count_distinct;
use datafusion::logical_expr::when;

pub(super) const RELATION: &str = "fact.code_semantic_reference";

pub(super) fn fields() -> Vec<FieldSpec> {
    vec![
        ("reference_id", DataType::FixedSizeBinary(16), true),
        ("target_entity_id", DataType::FixedSizeBinary(16), true),
        ("target_declaration_id", DataType::FixedSizeBinary(16), true),
        ("name", DataType::Utf8, false),
        ("language", DataType::Utf8, false),
        ("reference_kind", DataType::Utf8, false),
        ("raw_resolution", DataType::Utf8, false),
        ("resolution", DataType::Utf8, false),
        ("unknown_reason", DataType::Utf8, true),
        ("context_id", DataType::FixedSizeBinary(16), false),
        ("file_id", DataType::FixedSizeBinary(16), true),
        ("content_digest", DataType::FixedSizeBinary(32), false),
        ("source_generation", DataType::UInt64, false),
        ("start_byte", DataType::UInt64, false),
        ("end_byte", DataType::UInt64, false),
        ("provider_run_id", DataType::FixedSizeBinary(16), false),
        ("provider_occurrence_ordinal", DataType::UInt64, false),
        ("provider_target_ordinal", DataType::UInt64, true),
        ("provider", DataType::Utf8, false),
        ("workspace_id", DataType::FixedSizeBinary(16), false),
        ("target_mapping", DataType::Utf8, false),
        ("target_file_id", DataType::FixedSizeBinary(16), true),
    ]
}

pub(super) fn dependencies(available: bool) -> Vec<&'static str> {
    if available {
        vec![
            SOURCE,
            RUN,
            DECLARATION,
            modules::RELATION,
            PyreflyRelation::Reference.relation_id(),
        ]
    } else {
        vec![]
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one native reference plan keeps candidate cardinality, validity and projection together"
)]
pub(super) fn build(
    workspace: [u8; 16],
    inputs: &TransformationInputs,
    available: bool,
) -> Result<LogicalPlan, TransformationPlanError> {
    if !available {
        return empty(fields());
    }
    let raw = LogicalPlanBuilder::from(plan(inputs, PyreflyRelation::Reference.relation_id())?)
        .alias("p")?
        .build()?;
    let runs = LogicalPlanBuilder::from(plan(inputs, RUN)?)
        .filter(col("provider").eq(lit("pyrefly")))?
        .alias("r")?
        .build()?;
    let declarations = LogicalPlanBuilder::from(plan(inputs, DECLARATION)?)
        .filter(col("language").eq(lit("python")))?
        .build()?;
    let anchor = [
        "context_id",
        "file_id",
        "content_digest",
        "source_generation",
        "start_byte",
        "end_byte",
    ];
    let candidates = LogicalPlanBuilder::from(declarations.clone())
        .aggregate(
            anchor.map(col).to_vec(),
            vec![count_distinct(col("entity_id")).alias("candidates")],
        )?
        .alias("c")?
        .build()?;
    let declarations = LogicalPlanBuilder::from(declarations)
        .alias("d")?
        .join(
            candidates,
            JoinType::Inner,
            (
                anchor.map(|name| format!("d.{name}")).to_vec(),
                anchor.map(|name| format!("c.{name}")).to_vec(),
            ),
            None,
        )?
        .project(
            [
                "entity_id",
                "declaration_id",
                "context_id",
                "file_id",
                "content_digest",
                "source_generation",
                "start_byte",
                "end_byte",
            ]
            .map(|name| col(format!("d.{name}")))
            .into_iter()
            .chain([col("c.candidates")]),
        )?
        .alias("d")?
        .build()?;
    let modules = LogicalPlanBuilder::from(plan(inputs, modules::RELATION)?)
        .alias("m")?
        .build()?;
    let file = file_id_udf().call(vec![col("p.file_id")]);
    let target_file = file_id_udf().call(vec![col("p.target_file_id")]);
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
                file.clone().eq(col("s.file_id")),
                col("p.content_digest").eq(col("s.content_digest")),
                col("p.source_generation").eq(col("s.source_generation")),
                col("p.start_byte").lt(col("p.end_byte")),
                col("p.end_byte").lt_eq(col("s.byte_length")),
            ],
        )?
        .join_on(
            declarations,
            JoinType::Left,
            vec![
                col("p.target_is_module").eq(lit(false)),
                col("r.context_id").eq(col("d.context_id")),
                target_file.clone().eq(col("d.file_id")),
                col("p.target_content_digest").eq(col("d.content_digest")),
                col("p.source_generation").eq(col("d.source_generation")),
                col("p.target_start_byte").eq(col("d.start_byte")),
                col("p.target_end_byte").eq(col("d.end_byte")),
            ],
        )?
        .join_on(
            modules,
            JoinType::Left,
            vec![
                col("p.target_is_module").eq(lit(true)),
                target_file.clone().eq(col("m.file_id")),
                col("p.target_content_digest").eq(col("m.content_digest")),
                col("p.source_generation").eq(col("m.source_generation")),
                col("r.context_id").eq(col("m.context_id")),
            ],
        )?;
    let target = coalesce(vec![col("d.entity_id"), col("m.entity_id")]);
    let occurrence = source_occurrence_id(
        workspace,
        "codefabric_semantic_reference_occurrence_id_v1",
        101,
        2,
    )
    .call(vec![
        file.clone(),
        file.clone(),
        col("p.content_digest"),
        col("p.start_byte"),
        col("p.end_byte"),
    ]);
    Ok(joined
        .project(vec![
            occurrence.alias("reference_id"),
            target.clone().alias("target_entity_id"),
            col("d.declaration_id").alias("target_declaration_id"),
            col("p.name").alias("name"),
            lit("python").alias("language"),
            col("p.reference_kind").alias("reference_kind"),
            col("p.resolution_state").alias("raw_resolution"),
            when(target.clone().is_null(), lit("unknown"))
                .when(col("d.candidates").gt(lit(1_i64)), lit("candidates"))
                .otherwise(col("p.resolution_state"))?
                .alias("resolution"),
            when(
                col("p.resolution_state").eq(lit("unresolved")),
                lit("checker_definition_unavailable"),
            )
            .when(target.is_null(), lit("canonical_definition_unavailable"))
            .when(
                col("d.candidates").gt(lit(1_i64)),
                lit("multiple_canonical_definitions"),
            )
            .otherwise(lit(ScalarValue::Utf8(None)))?
            .alias("unknown_reason"),
            col("r.context_id").alias("context_id"),
            file.alias("file_id"),
            col("p.content_digest").alias("content_digest"),
            col("p.source_generation").alias("source_generation"),
            col("p.start_byte").alias("start_byte"),
            col("p.end_byte").alias("end_byte"),
            col("r.provider_run_id").alias("provider_run_id"),
            col("p.occurrence_ordinal").alias("provider_occurrence_ordinal"),
            col("p.target_ordinal").alias("provider_target_ordinal"),
            lit("pyrefly").alias("provider"),
            col("s.workspace_id").alias("workspace_id"),
            col("p.definition_mapping").alias("target_mapping"),
            target_file.alias("target_file_id"),
        ])?
        .distinct()?
        .build()?)
}
