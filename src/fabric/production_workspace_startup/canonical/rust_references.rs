//! Compiler denotations and import bindings retain exact owner and independent location validity.

use super::{
    DECLARATION, DataType, Expr, JoinType, LogicalPlan, LogicalPlanBuilder, RUN, RustcRelation,
    ScalarValue, TransformationInputs, TransformationPlanError, col, file_id_udf, lit, plan,
    rust_entity_id, semantic_references, source_alias, source_occurrence_id,
};
use datafusion::functions::core::expr_fn::coalesce;
use datafusion::functions_aggregate::count::count_distinct;
use datafusion::logical_expr::when;

// A compilation owner can emit HIR from many files. Common provider source fields authorize
// the compilation; location fields independently authorize each observation's source bytes.
fn compiler_rows(
    inputs: &TransformationInputs,
    relation: RustcRelation,
) -> Result<LogicalPlanBuilder, TransformationPlanError> {
    let runs = LogicalPlanBuilder::from(plan(inputs, RUN)?)
        .filter(col("provider").eq(lit("rustc")))?
        .alias("r")?
        .build()?;
    let location = LogicalPlanBuilder::from(source_alias(inputs)?)
        .alias("l")?
        .build()?;
    Ok(
        LogicalPlanBuilder::from(plan(inputs, relation.relation_id())?)
            .alias("p")?
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
                location,
                JoinType::Left,
                vec![
                    file_id_udf()
                        .call(vec![col("p.location_file_id")])
                        .eq(col("l.file_id")),
                    col("p.location_content_digest").eq(col("l.content_digest")),
                    col("p.source_generation").eq(col("l.source_generation")),
                    col("p.location_state").eq(lit("captured-source")),
                    col("p.expansion_kind").eq(lit("source-authored")),
                    col("p.span_start_byte").lt(col("p.span_end_byte")),
                    col("p.span_end_byte").lt_eq(col("l.byte_length")),
                ],
            )?,
    )
}

fn located(value: Expr, datatype: &DataType) -> Result<Expr, TransformationPlanError> {
    Ok(when(col("l.file_id").is_not_null(), value)
        .otherwise(lit(ScalarValue::try_from(datatype)?))?)
}

// Accepted native rows that canonical source fencing removes still downgrade requested coverage.
// Unmapped rows that remain in the canonical relation are qualified by their own unknown reason.
pub(super) fn missing_bindings(
    inputs: &TransformationInputs,
    native: RustcRelation,
) -> Result<LogicalPlan, TransformationPlanError> {
    let relation = match native {
        RustcRelation::HirReference => semantic_references::RELATION,
        RustcRelation::HirImport => super::imports::RELATION,
        _ => return Err(super::invalid("invalid compiler reference coverage family").into()),
    };
    missing_binding_rows(
        plan(inputs, native.relation_id())?,
        plan(inputs, RUN)?,
        plan(inputs, relation)?,
        native,
    )
}

pub(super) fn missing_binding_rows(
    raw: LogicalPlan,
    runs: LogicalPlan,
    canonical: LogicalPlan,
    native: RustcRelation,
) -> Result<LogicalPlan, TransformationPlanError> {
    let (ordinal, canonical_ordinal, run_field) = match native {
        RustcRelation::HirReference => (
            "reference_ordinal",
            "provider_occurrence_ordinal",
            "provider_run_id",
        ),
        RustcRelation::HirImport => (
            "import_ordinal",
            "provider_import_ordinal",
            "semantic_provider_run_id",
        ),
        _ => return Err(super::invalid("invalid compiler reference coverage family").into()),
    };
    let runs = LogicalPlanBuilder::from(runs)
        .filter(col("provider").eq(lit("rustc")))?
        .alias("r")?
        .build()?;
    let canonical = LogicalPlanBuilder::from(canonical)
        .filter(col("language").eq(lit("rust")))?
        .alias("q")?
        .build()?;
    let mut conditions = vec![
        col(format!("p.{ordinal}")).eq(col(format!("q.{canonical_ordinal}"))),
        col("r.provider_run_id").eq(col(format!("q.{run_field}"))),
        col("r.context_id").eq(col("q.context_id")),
        col("p.source_generation").eq(col("q.source_generation")),
        col("p.compilation_unit_id").eq(col("q.provider_compilation_unit")),
        col("p.owner_id").eq(col("q.provider_owner")),
    ];
    if native == RustcRelation::HirReference {
        conditions.push(
            col("p.target_ordinal")
                .eq(col("q.provider_target_ordinal"))
                .or(col("p.target_ordinal")
                    .is_null()
                    .and(col("q.provider_target_ordinal").is_null())),
        );
    }
    Ok(LogicalPlanBuilder::from(raw)
        .alias("p")?
        .join(
            runs,
            JoinType::Inner,
            (
                vec!["p.provider_run_id", "p.source_generation"],
                vec!["r.provider_run_identity", "r.source_generation"],
            ),
            None,
        )?
        .join_on(canonical, JoinType::LeftAnti, conditions)?
        .project(vec![
            lit("rust").alias("language"),
            col("r.context_id").alias("context_id"),
            file_id_udf()
                .call(vec![col("p.source_file_id")])
                .alias("file_id"),
            col("p.source_generation").alias("source_generation"),
        ])?
        .build()?)
}

fn declarations(inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
    let source = LogicalPlanBuilder::from(source_alias(inputs)?)
        .alias("t")?
        .build()?;
    let fields = [
        "entity_id",
        "declaration_id",
        "context_id",
        "file_id",
        "source_generation",
    ];
    let declarations = LogicalPlanBuilder::from(plan(inputs, DECLARATION)?)
        .filter(col("language").eq(lit("rust")))?
        .alias("d")?
        .join(
            source,
            JoinType::Inner,
            (
                vec!["d.file_id", "d.content_digest", "d.source_generation"],
                vec!["t.file_id", "t.content_digest", "t.source_generation"],
            ),
            None,
        )?
        .project(fields.map(|name| col(format!("d.{name}"))))?
        .distinct()?
        .build()?;
    let candidates = LogicalPlanBuilder::from(declarations.clone())
        .aggregate(
            ["entity_id", "context_id", "source_generation"]
                .map(col)
                .to_vec(),
            vec![count_distinct(col("declaration_id")).alias("candidates")],
        )?
        .alias("c")?
        .build()?;
    Ok(LogicalPlanBuilder::from(declarations)
        .alias("d")?
        .join(
            candidates,
            JoinType::Inner,
            (
                vec!["d.entity_id", "d.context_id", "d.source_generation"],
                vec!["c.entity_id", "c.context_id", "c.source_generation"],
            ),
            None,
        )?
        .project(
            fields
                .map(|name| col(format!("d.{name}")))
                .into_iter()
                .chain([col("c.candidates")]),
        )?
        .alias("d")?
        .build()?)
}

#[allow(
    clippy::too_many_lines,
    reason = "one projection keeps compiler resolution, source validity and canonical candidate semantics together"
)]
pub(super) fn references(
    workspace: [u8; 16],
    inputs: &TransformationInputs,
) -> Result<LogicalPlan, TransformationPlanError> {
    let joined = compiler_rows(inputs, RustcRelation::HirReference)?.join_on(
        declarations(inputs)?,
        JoinType::Left,
        vec![
            rust_entity_id(workspace)
                .call(vec![
                    col("r.context_id"),
                    col("p.target_stable_crate_id"),
                    col("p.target_def_path_hash"),
                    col("p.target_definition_kind"),
                ])
                .eq(col("d.entity_id")),
            col("r.context_id").eq(col("d.context_id")),
            col("p.source_generation").eq(col("d.source_generation")),
        ],
    )?;
    let occurrence = source_occurrence_id(
        workspace,
        "codefabric_semantic_reference_occurrence_id_v1",
        101,
        2,
    )
    .call(vec![
        col("l.file_id"),
        col("l.file_id"),
        col("l.content_digest"),
        col("p.span_start_byte"),
        col("p.span_end_byte"),
    ]);
    let raw_resolved = col("p.resolution_kind").eq(lit("definition"));
    let resolution = when(
        col("l.file_id").is_null().or(col("d.entity_id").is_null()),
        lit("unknown"),
    )
    .when(col("d.candidates").gt(lit(1_i64)), lit("candidates"))
    .when(raw_resolved.clone(), lit("resolved"))
    .otherwise(lit("unknown"))?;
    Ok(joined
        .project(vec![
            occurrence.alias("reference_id"),
            col("d.entity_id").alias("target_entity_id"),
            col("d.declaration_id").alias("target_declaration_id"),
            col("p.name").alias("name"),
            lit("rust").alias("language"),
            col("p.reference_kind").alias("reference_kind"),
            col("p.resolution_kind").alias("raw_resolution"),
            resolution.alias("resolution"),
            when(
                col("l.file_id").is_null(),
                lit("compiler_reference_location_unavailable"),
            )
            .when(!raw_resolved, lit("compiler_reference_kind_not_normalized"))
            .when(
                col("d.entity_id").is_null(),
                lit("canonical_definition_unavailable"),
            )
            .when(
                col("d.candidates").gt(lit(1_i64)),
                lit("multiple_canonical_definitions"),
            )
            .otherwise(lit(ScalarValue::Utf8(None)))?
            .alias("unknown_reason"),
            col("r.context_id").alias("context_id"),
            col("l.file_id").alias("file_id"),
            col("l.content_digest").alias("content_digest"),
            col("p.source_generation").alias("source_generation"),
            located(col("p.span_start_byte"), &DataType::UInt64)?.alias("start_byte"),
            located(col("p.span_end_byte"), &DataType::UInt64)?.alias("end_byte"),
            col("r.provider_run_id").alias("provider_run_id"),
            col("p.reference_ordinal").alias("provider_occurrence_ordinal"),
            col("p.target_ordinal").alias("provider_target_ordinal"),
            lit("rustc").alias("provider"),
            col("s.workspace_id").alias("workspace_id"),
            lit("compiler-stable-definition-key").alias("target_mapping"),
            col("d.file_id").alias("target_file_id"),
            col("p.compilation_unit_id").alias("provider_compilation_unit"),
            col("p.owner_id").alias("provider_owner"),
            col("p.target_namespace").alias("target_namespace"),
            col("p.target_definition_kind").alias("target_definition_kind"),
            col("p.target_native_definition_kind").alias("target_native_definition_kind"),
        ])?
        .distinct()?
        .build()?)
}

pub(super) fn imports(
    workspace: [u8; 16],
    inputs: &TransformationInputs,
) -> Result<LogicalPlan, TransformationPlanError> {
    let references = LogicalPlanBuilder::from(plan(inputs, semantic_references::RELATION)?)
        .filter(
            col("language")
                .eq(lit("rust"))
                .and(col("reference_kind").eq(lit("import"))),
        )?
        .alias("q")?
        .build()?;
    let joined = compiler_rows(inputs, RustcRelation::HirImport)?.join_on(
        references,
        JoinType::Left,
        vec![
            col("p.reference_ordinal").eq(col("q.provider_occurrence_ordinal")),
            col("p.compilation_unit_id").eq(col("q.provider_compilation_unit")),
            col("p.owner_id").eq(col("q.provider_owner")),
            col("r.provider_run_id").eq(col("q.provider_run_id")),
            col("r.context_id").eq(col("q.context_id")),
            col("p.source_generation").eq(col("q.source_generation")),
        ],
    )?;
    let occurrence = source_occurrence_id(workspace, "codefabric_import_occurrence_id_v1", 103, 4)
        .call(vec![
            col("l.file_id"),
            col("l.file_id"),
            col("l.content_digest"),
            col("p.span_start_byte"),
            col("p.span_end_byte"),
        ]);
    Ok(joined
        .project(vec![
            occurrence.alias("import_id"),
            col("q.reference_id").alias("semantic_reference_id"),
            lit("rust").alias("language"),
            col("r.context_id").alias("context_id"),
            col("l.file_id").alias("file_id"),
            col("l.content_digest").alias("content_digest"),
            col("p.source_generation").alias("source_generation"),
            col("s.workspace_id").alias("workspace_id"),
            located(col("p.span_start_byte"), &DataType::UInt64)?.alias("start_byte"),
            located(col("p.span_end_byte"), &DataType::UInt64)?.alias("end_byte"),
            col("p.import_kind").alias("import_kind"),
            col("p.path").alias("source_name"),
            lit(ScalarValue::Utf8(None)).alias("module_name"),
            col("q.name").alias("imported_name"),
            col("p.alias").alias("alias_name"),
            lit(ScalarValue::UInt16(None)).alias("relative_level"),
            col("p.import_kind").eq(lit("glob")).alias("star_import"),
            col("q.target_entity_id").alias("target_entity_id"),
            col("q.target_declaration_id").alias("target_declaration_id"),
            when(col("l.file_id").is_null(), lit("unknown"))
                .otherwise(coalesce(vec![col("q.resolution"), lit("unknown")]))?
                .alias("resolution"),
            when(
                col("l.file_id").is_null(),
                lit("compiler_import_location_unavailable"),
            )
            .when(
                col("q.provider_occurrence_ordinal").is_null(),
                lit("compiler_import_reference_unavailable"),
            )
            .otherwise(col("q.unknown_reason"))?
            .alias("unknown_reason"),
            lit(ScalarValue::FixedSizeBinary(16, None)).alias("syntax_provider_run_id"),
            col("r.provider_run_id").alias("semantic_provider_run_id"),
            col("r.context_id").alias("semantic_context_id"),
            lit(ScalarValue::FixedSizeBinary(16, None)).alias("syntax_observation_id"),
            lit("compiler-reference-ordinal").alias("join_method"),
            lit("rustc").alias("provider"),
            col("p.compilation_unit_id").alias("provider_compilation_unit"),
            col("p.import_ordinal").alias("provider_import_ordinal"),
            col("p.owner_id").alias("provider_owner"),
            col("q.target_namespace").alias("target_namespace"),
            col("p.is_public").alias("is_public"),
        ])?
        .distinct()?
        .build()?)
}
