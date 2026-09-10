//! Checker-selected modules are semantic entities even when they have no declaration span.

use super::{
    Arc, DataType, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder, RUN, SOURCE, ScalarUDF,
    TransformationInputs, TransformationPlanError, Volatility, col, create_udf, empty,
    entity_fields, file_id_udf, fixed, identity, ids, invalid, kind_code, lit, plan,
    public_entity_id, source_alias, text,
};
use crate::pyrefly_service::PyreflyRelation;

pub(super) const RELATION: &str = "fact.code_module";

pub(super) fn fields() -> Vec<FieldSpec> {
    let mut fields = entity_fields();
    fields.extend([
        ("content_digest", DataType::FixedSizeBinary(32), false),
        ("source_generation", DataType::UInt64, false),
        ("provider_run_id", DataType::FixedSizeBinary(16), false),
        ("provider", DataType::Utf8, false),
        ("provider_module_id", DataType::Utf8, false),
        ("public_entity_id", DataType::Utf8, true),
    ]);
    fields
}

pub(super) fn dependencies(available: bool) -> Vec<&'static str> {
    if available {
        vec![SOURCE, RUN, PyreflyRelation::ModuleContext.relation_id()]
    } else {
        vec![]
    }
}

pub(super) fn build(
    workspace: [u8; 16],
    inputs: &TransformationInputs,
    available: bool,
) -> Result<LogicalPlan, TransformationPlanError> {
    if !available {
        return empty(fields());
    }
    let raw = LogicalPlanBuilder::from(plan(inputs, PyreflyRelation::ModuleContext.relation_id())?)
        .alias("p")?
        .build()?;
    let runs = LogicalPlanBuilder::from(plan(inputs, RUN)?)
        .filter(col("provider").eq(lit("pyrefly")))?
        .alias("r")?
        .build()?;
    let file = file_id_udf().call(vec![col("p.file_id")]);
    let entity = module_id(workspace).call(vec![
        col("r.context_id"),
        file.clone(),
        col("p.module_name"),
    ]);
    Ok(LogicalPlanBuilder::from(raw)
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
            ],
        )?
        .project(vec![
            entity.clone().alias("entity_id"),
            lit("module").alias("entity_kind"),
            col("p.module_name").alias("name"),
            col("p.module_name").alias("qualified_name"),
            lit("python").alias("language"),
            col("r.context_id").alias("context_id"),
            file.alias("file_id"),
            col("s.workspace_id").alias("workspace_id"),
            col("p.content_digest").alias("content_digest"),
            col("p.source_generation").alias("source_generation"),
            col("r.provider_run_id").alias("provider_run_id"),
            lit("pyrefly").alias("provider"),
            col("p.module_id").alias("provider_module_id"),
            public_entity_id()
                .call(vec![entity, lit("module")])
                .alias("public_entity_id"),
        ])?
        .distinct()?
        .build()?)
}

fn module_id(workspace: [u8; 16]) -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_python_module_entity_id_v1",
        vec![
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(16),
            DataType::Utf8,
        ],
        DataType::FixedSizeBinary(16),
        Volatility::Immutable,
        Arc::new(move |values| {
            ids(values, |arrays, row| {
                let context = fixed(&arrays[0], row)?;
                let owner = identity::semantic_owner_identity(
                    workspace,
                    context,
                    "python-module",
                    fixed(&arrays[1], row)?.to_vec(),
                )
                .map_err(|error| invalid(&error.to_string()))?;
                identity::semantic_entity_identity(
                    workspace,
                    context,
                    kind_code("module"),
                    owner.id,
                    text(&arrays[2], row)?.as_bytes().to_vec(),
                )
                .map(|id| id.id)
                .map_err(|error| invalid(&error.to_string()))
            })
        }),
    ))
}
