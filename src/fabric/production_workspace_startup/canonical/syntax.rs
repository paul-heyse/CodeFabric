//! Complete CST facts remain source-context occurrences beside semantic declarations.

use super::{
    DataType, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder, NativeSyntaxRelation, SOURCE,
    ScalarValue, TransformationInputs, TransformationPlanError, canonical_union, col, empty,
    entity_fields, lit, plan, public_entity_id, source_alias,
};
use crate::provider_native_rust_syntax::RustSyntaxRelation;
use datafusion::functions::core::expr_fn::{get_field, named_struct};
use datafusion::functions_aggregate::array_agg::array_agg;

mod identity;
mod udf;

pub(super) const RELATION: &str = "fact.code_syntax_node";
const ANCHORS: [&str; 8] = [
    "provider_local_node_id",
    "parent_provider_local_node_id",
    "start_byte",
    "end_byte",
    "normalized_kind_code",
    "ordinal",
    "depth",
    "field_name",
];
const SCOPE: [&str; 9] = [
    "workspace_id",
    "file_id",
    "content_digest",
    "source_generation",
    "byte_length",
    "provider_run_id",
    "provider_id",
    "provider_release",
    "analysis_context_id",
];

pub(super) fn fields() -> Vec<FieldSpec> {
    let mut fields = entity_fields();
    fields.extend([
        ("public_entity_id", DataType::Utf8, true),
        ("parent_entity_id", DataType::FixedSizeBinary(16), true),
        ("content_digest", DataType::FixedSizeBinary(32), false),
        ("source_generation", DataType::UInt64, false),
        ("provider_run_id", DataType::FixedSizeBinary(16), false),
        ("provider", DataType::Utf8, false),
        ("provider_release", DataType::Utf8, false),
        ("provider_context_id", DataType::FixedSizeBinary(16), false),
    ]);
    let schema = NativeSyntaxRelation::TreeSitterCstNode.schema();
    for name in [
        "provider_local_node_id",
        "parent_provider_local_node_id",
        "raw_kind_id",
        "raw_kind",
        "normalized_kind_code",
        "field_name",
        "start_byte",
        "end_byte",
        "named",
        "extra",
        "error",
        "missing",
        "ordinal",
        "depth",
        "raw_kind_disposition",
    ] {
        let field = schema.field_with_name(name).expect("pinned CST schema");
        fields.push((name, field.data_type().clone(), field.is_nullable()));
    }
    fields
}

pub(super) fn dependencies(python: bool, rust: bool) -> Vec<&'static str> {
    let mut inputs = Vec::new();
    if python {
        inputs.push(NativeSyntaxRelation::TreeSitterCstNode.as_str());
    }
    if rust {
        inputs.push(RustSyntaxRelation::CstNode.name());
    }
    if !inputs.is_empty() {
        inputs.push(SOURCE);
    }
    inputs
}

pub(super) fn build(
    inputs: &TransformationInputs,
    python: bool,
    rust: bool,
) -> Result<LogicalPlan, TransformationPlanError> {
    let mut branches = Vec::new();
    for (available, language, relation) in [
        (
            python,
            "python",
            NativeSyntaxRelation::TreeSitterCstNode.as_str(),
        ),
        (rust, "rust", RustSyntaxRelation::CstNode.name()),
    ] {
        if available {
            branches.push(language_plan(inputs, language, relation)?);
        }
    }
    if branches.is_empty() {
        return empty(fields());
    }
    canonical_union(RELATION, fields(), branches)
}

#[allow(
    clippy::too_many_lines,
    reason = "one native plan keeps exact source grouping, normalization and raw-node join-back coherent"
)]
fn language_plan(
    inputs: &TransformationInputs,
    language: &str,
    relation: &str,
) -> Result<LogicalPlan, TransformationPlanError> {
    let raw = LogicalPlanBuilder::from(plan(inputs, relation)?)
        .alias("p")?
        .join(
            source_alias(inputs)?,
            JoinType::Inner,
            (
                vec!["p.file_id", "p.content_digest", "p.source_generation"],
                vec!["s.file_id", "s.content_digest", "s.source_generation"],
            ),
            None,
        )?
        .project(
            SCOPE
                .iter()
                .map(|name| {
                    col(format!(
                        "{}.{}",
                        if matches!(*name, "workspace_id" | "byte_length") {
                            "s"
                        } else {
                            "p"
                        },
                        name
                    ))
                    .alias(*name)
                })
                .chain(
                    fields()
                        .into_iter()
                        .filter(|(name, _, _)| {
                            matches!(
                                *name,
                                "provider_local_node_id"
                                    | "parent_provider_local_node_id"
                                    | "raw_kind_id"
                                    | "raw_kind"
                                    | "normalized_kind_code"
                                    | "field_name"
                                    | "start_byte"
                                    | "end_byte"
                                    | "named"
                                    | "extra"
                                    | "error"
                                    | "missing"
                                    | "ordinal"
                                    | "depth"
                                    | "raw_kind_disposition"
                            )
                        })
                        .map(|(name, _, _)| col(format!("p.{name}")).alias(name)),
                ),
        )?
        .build()?;
    let grouped = LogicalPlanBuilder::from(raw.clone())
        .aggregate(
            SCOPE.map(col).to_vec(),
            vec![
                array_agg(named_struct(
                    ANCHORS
                        .iter()
                        .flat_map(|name| [lit(*name), col(*name)])
                        .collect(),
                ))
                .alias("nodes"),
            ],
        )?
        .build()?;
    let node_type = grouped
        .schema()
        .field_with_unqualified_name("nodes")?
        .data_type()
        .clone();
    let normalized = udf::normalizer(node_type)
        .call(vec![
            col("workspace_id"),
            col("file_id"),
            col("content_digest"),
            col("byte_length"),
            lit(language),
            col("nodes"),
        ])
        .alias("normalized");
    let identities = LogicalPlanBuilder::from(grouped)
        .project(SCOPE.map(col).into_iter().chain([normalized]))?
        .unnest_column("normalized")?
        .project(
            SCOPE.map(col).into_iter().chain(
                ["provider_local_node_id", "entity_id", "parent_entity_id"]
                    .map(|name| get_field(col("normalized"), name).alias(name)),
            ),
        )?
        .alias("n")?
        .build()?;
    let joined = LogicalPlanBuilder::from(raw).alias("p")?.join(
        identities,
        JoinType::Inner,
        (
            SCOPE
                .map(|name| format!("p.{name}"))
                .into_iter()
                .chain(["p.provider_local_node_id".into()])
                .collect::<Vec<_>>(),
            SCOPE
                .map(|name| format!("n.{name}"))
                .into_iter()
                .chain(["n.provider_local_node_id".into()])
                .collect::<Vec<_>>(),
        ),
        None,
    )?;
    Ok(joined
        .project(fields().iter().map(|(name, _, _)| {
            match *name {
                "entity_id" | "parent_entity_id" => col(format!("n.{name}")).alias(*name),
                "entity_kind" => lit("syntax-node").alias(*name),
                "name" => col("p.raw_kind").alias(*name),
                "qualified_name" => lit(ScalarValue::Utf8(None)).alias(*name),
                "language" => lit(language).alias(*name),
                "context_id" => {
                    super::fixed_literal(&crate::identity::SOURCE_CONTEXT_ID).alias(*name)
                }
                "public_entity_id" => public_entity_id()
                    .call(vec![col("n.entity_id"), lit("syntax-node")])
                    .alias(*name),
                "provider" => col("p.provider_id").alias(*name),
                "provider_context_id" => col("p.analysis_context_id").alias(*name),
                _ => col(format!("p.{name}")).alias(*name),
            }
        }))?
        .build()?)
}
