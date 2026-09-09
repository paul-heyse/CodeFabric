//! Subject source descriptors join through exact workspace, generation, file and content pins.

use super::{
    DataType, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder, TransformationInputs,
    TransformationPlanError, col, lit, plan,
};

use crate::provider_native_rust_syntax::RustSyntaxRelation;

pub(super) const RELATION: &str = "fact.code_source_context";

pub(super) fn fields() -> Vec<FieldSpec> {
    let mut fields = super::declaration_fields();
    fields.extend([
        ("relative_path", DataType::Binary, false),
        ("context_kind", DataType::Utf8, false),
        ("text_handling", DataType::Utf8, false),
        ("source_mapping", DataType::Utf8, false),
    ]);
    fields
}

pub(super) fn dependencies(python: bool, rust: bool) -> Vec<&'static str> {
    let mut inputs = vec![super::DECLARATION, super::SOURCE];
    if python {
        inputs.push(super::NativeSyntaxRelation::TreeSitterCstNode.as_str());
    }
    if rust {
        inputs.push(RustSyntaxRelation::CstNode.name());
    }
    inputs
}

pub(super) fn build(
    inputs: &TransformationInputs,
    python: bool,
    rust: bool,
) -> Result<LogicalPlan, TransformationPlanError> {
    let mut result =
        LogicalPlanBuilder::from(exact(inputs, "exact source span", "declaration-span")?)
            .union(exact(
                inputs,
                "surrounding lines",
                "declaration-line-anchor",
            )?)?
            .build()?;
    if python {
        result = LogicalPlanBuilder::from(result)
            .union(functions(
                inputs,
                "python",
                super::NativeSyntaxRelation::TreeSitterCstNode.as_str(),
                "function_definition",
            )?)?
            .build()?;
    }
    if rust {
        result = LogicalPlanBuilder::from(result)
            .union(functions(
                inputs,
                "rust",
                RustSyntaxRelation::CstNode.name(),
                "function_item",
            )?)?
            .build()?;
    }
    Ok(result)
}

fn exact(
    inputs: &TransformationInputs,
    kind: &str,
    mapping: &str,
) -> Result<LogicalPlan, TransformationPlanError> {
    let declarations = LogicalPlanBuilder::from(plan(inputs, super::DECLARATION)?)
        .filter(col("public_entity_id").is_not_null())?
        .alias("d")?
        .build()?;
    let bytes = LogicalPlanBuilder::from(plan(inputs, super::SOURCE)?)
        .alias("s")?
        .build()?;
    Ok(LogicalPlanBuilder::from(declarations)
        .join(
            bytes,
            JoinType::Inner,
            (
                vec![
                    "d.workspace_id",
                    "d.source_generation",
                    "d.file_id",
                    "d.content_digest",
                ],
                vec![
                    "s.workspace_id",
                    "s.source_generation",
                    "s.file_id",
                    "s.content_digest",
                ],
            ),
            None,
        )?
        .project(
            super::declaration_fields()
                .iter()
                .map(|(name, _, _)| col(format!("d.{name}")))
                .chain([
                    col("s.relative_path"),
                    lit(kind).alias("context_kind"),
                    lit("lossless UTF-8 else bytes").alias("text_handling"),
                    lit(mapping).alias("source_mapping"),
                ]),
        )?
        .build()?)
}

fn source_keys(left: &str, right: &str) -> (Vec<String>, Vec<String>) {
    let fields = ["file_id", "content_digest", "source_generation"];
    (
        fields
            .iter()
            .map(|field| format!("{left}.{field}"))
            .collect(),
        fields
            .iter()
            .map(|field| format!("{right}.{field}"))
            .collect(),
    )
}

fn syntax_keys(left: &str, right: &str) -> (Vec<String>, Vec<String>) {
    let (mut left_keys, mut right_keys) = source_keys(left, right);
    left_keys.push(format!("{left}.provider_run_id"));
    right_keys.push(format!("{right}.provider_run_id"));
    left_keys.push(format!("{left}.provider_local_node_id"));
    right_keys.push(format!("{right}.parent_provider_local_node_id"));
    (left_keys, right_keys)
}

fn functions(
    inputs: &TransformationInputs,
    language: &str,
    relation: &str,
    kind: &str,
) -> Result<LogicalPlan, TransformationPlanError> {
    let nodes = plan(inputs, relation)?;
    let valid = || {
        col("missing")
            .eq(lit(false))
            .and(col("error").eq(lit(false)))
    };
    let functions = LogicalPlanBuilder::from(nodes.clone())
        .filter(col("raw_kind").eq(lit(kind)).and(valid()))?
        .alias("f")?
        .build()?;
    let names = LogicalPlanBuilder::from(nodes.clone())
        .filter(col("field_name").eq(lit("name")).and(valid()))?
        .alias("n")?
        .build()?;
    let bodies = LogicalPlanBuilder::from(nodes)
        .filter(
            col("field_name")
                .eq(lit("body"))
                .and(col("raw_kind").eq(lit("block")))
                .and(valid()),
        )?
        .alias("b")?
        .build()?;
    let declarations = LogicalPlanBuilder::from(plan(inputs, super::DECLARATION)?)
        .filter(
            col("language")
                .eq(lit(language))
                .and(col("entity_kind").eq(lit("function")))
                .and(col("public_entity_id").is_not_null()),
        )?
        .alias("d")?
        .build()?;
    let (mut declaration_keys, mut node_keys) = source_keys("d", "f");
    // Exact keyed matches avoid a per-file all-pairs range join.
    declaration_keys.push("d.start_byte".to_owned());
    if language == "python" {
        declaration_keys.push("d.end_byte".to_owned());
        node_keys.extend(["n.start_byte".to_owned(), "n.end_byte".to_owned()]);
    } else {
        node_keys.push("f.start_byte".to_owned());
    }
    let syntax = LogicalPlanBuilder::from(functions)
        .join(names, JoinType::Inner, syntax_keys("f", "n"), None)?
        .join(bodies, JoinType::Inner, syntax_keys("f", "b"), None)?
        .build()?;
    let selected = LogicalPlanBuilder::from(declarations)
        .join(syntax, JoinType::Inner, (declaration_keys, node_keys), None)?
        .filter(col("d.end_byte").lt_eq(col("f.end_byte")))?
        .join(
            super::source_alias(inputs)?,
            JoinType::Inner,
            source_keys("d", "s"),
            None,
        )?
        .build()?;
    let descriptor = |kind: &str, node: &str| -> Result<LogicalPlan, TransformationPlanError> {
        Ok(LogicalPlanBuilder::from(selected.clone())
            .project(
                super::declaration_fields()
                    .iter()
                    .map(|(name, _, _)| match *name {
                        "start_byte" | "end_byte" => col(format!("{node}.{name}")).alias(*name),
                        _ => col(format!("d.{name}")),
                    })
                    .chain([
                        col("s.relative_path"),
                        lit(kind).alias("context_kind"),
                        lit("lossless UTF-8 else bytes").alias("text_handling"),
                        lit(if language == "python" {
                            "exact-function-name-child"
                        } else {
                            "exact-function-header-start"
                        })
                        .alias("source_mapping"),
                    ]),
            )?
            .build()?)
    };
    Ok(
        LogicalPlanBuilder::from(descriptor("function definition", "f")?)
            .union(descriptor("function body", "b")?)?
            .build()?,
    )
}
