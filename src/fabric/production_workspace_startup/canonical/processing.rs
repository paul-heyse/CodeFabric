//! Query processing partitions retain terminal provider state and semantic gaps separately.

use super::{
    DataType, Expr, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder, NativeSyntaxRelation,
    RUN, ScalarValue, TransformationInputs, TransformationPlanError, col, lit, plan,
};
use datafusion::functions::core::expr_fn::coalesce;
use datafusion::functions_aggregate::count::count;

mod call_owners;

pub(super) const INPUT: &str = "system.requested_processing_scope";
pub(super) const OUTPUT: &str = "system.entity_processing_scope";

pub(super) fn fields() -> Vec<FieldSpec> {
    vec![
        ("workspace_id", DataType::FixedSizeBinary(16), false),
        ("source_generation", DataType::UInt64, false),
        ("input_set_id", DataType::FixedSizeBinary(32), false),
        ("language", DataType::Utf8, false),
        ("scope_kind", DataType::Utf8, false),
        ("relative_path", DataType::Binary, false),
        ("target_name", DataType::Utf8, true),
        ("context_id", DataType::FixedSizeBinary(16), true),
        ("target_kind", DataType::Utf8, true),
        ("target_platform", DataType::Utf8, true),
        ("build_profile", DataType::Utf8, true),
        (
            "build_features",
            DataType::List(std::sync::Arc::new(arrow_schema::Field::new(
                "item",
                DataType::Utf8,
                true,
            ))),
            true,
        ),
        ("default_features", DataType::Boolean, true),
        ("file_id", DataType::FixedSizeBinary(16), true),
        ("family", DataType::Utf8, false),
        ("processing_state", DataType::Utf8, false),
        ("reason", DataType::Utf8, false),
    ]
}

pub(super) fn output_fields() -> Vec<FieldSpec> {
    let mut fields = fields();
    fields.push(("owner_entity_id", DataType::FixedSizeBinary(16), true));
    fields
}

pub(super) fn dependencies(pyrefly: bool, rust: super::RustInputs) -> Vec<&'static str> {
    let mut result = vec![
        INPUT,
        super::calls::RELATION,
        super::REFERENCE,
        super::semantic_references::RELATION,
        super::imports::RELATION,
        super::types::OBSERVATION,
        super::types::CALLABLE,
        super::types::MEMBER,
        super::types::GRAPH,
        super::types::RUST_GRAPH,
        super::DECLARATION,
        super::source_context::RELATION,
        super::syntax::RELATION,
    ];
    if rust.bodies {
        result.extend([
            super::SOURCE,
            RUN,
            super::RustcRelation::MirBody.relation_id(),
            super::RustcRelation::PublicItem.relation_id(),
        ]);
    }
    if pyrefly {
        result.extend([
            RUN,
            crate::pyrefly_service::PyreflyRelation::CallTarget.relation_id(),
            NativeSyntaxRelation::RuffCallableSyntax.as_str(),
        ]);
    }
    if rust.types {
        result.extend([RUN, super::RustcRelation::Type.relation_id()]);
    }
    if rust.references {
        result.extend([RUN, super::RustcRelation::HirReference.relation_id()]);
    }
    if rust.imports {
        result.extend([RUN, super::RustcRelation::HirImport.relation_id()]);
    }
    result.sort_unstable();
    result.dedup();
    result
}

fn scope_key() -> Result<Expr, datafusion::common::DataFusionError> {
    datafusion::logical_expr::when(col("language").eq(lit("python")), col("file_id"))
        .otherwise(col("context_id"))
}

fn count_when(condition: Expr) -> Result<Expr, datafusion::common::DataFusionError> {
    Ok(count(
        datafusion::logical_expr::when(condition, lit(1_i64))
            .otherwise(lit(ScalarValue::Int64(None)))?,
    ))
}

#[allow(
    clippy::too_many_lines,
    reason = "native partition aggregation and projection stay together"
)]
pub(super) fn build(
    inputs: &TransformationInputs,
    pyrefly: bool,
    rust: super::RustInputs,
    workspace: [u8; 16],
) -> Result<LogicalPlan, TransformationPlanError> {
    let calls = plan(inputs, super::calls::RELATION)?;
    let observations = LogicalPlanBuilder::from(calls)
        .aggregate(
            vec![
                col("language"),
                col("context_id"),
                scope_key()?.alias("scope_key"),
                col("source_generation"),
            ],
            vec![
                count_when(col("target_entity_id").is_null())?.alias("targets"),
                count_when(col("call_site_id").is_null())?.alias("locations"),
                count_when(col("caller_entity_id").is_null())?.alias("callers"),
            ],
        )?
        .alias("g")?
        .build()?;
    let base = LogicalPlanBuilder::from(qualify_references(inputs, rust)?)
        .union(function_source_scope(inputs)?)?
        .build()?;
    let fields = fields();
    let mut joined = LogicalPlanBuilder::from(base)
        .project(
            fields
                .iter()
                .map(|(name, _, _)| col(*name))
                .chain([scope_key()?.alias("scope_key")]),
        )?
        .alias("b")?
        .join(
            observations,
            JoinType::Left,
            (
                vec![
                    "b.language",
                    "b.context_id",
                    "b.scope_key",
                    "b.source_generation",
                ],
                vec![
                    "g.language",
                    "g.context_id",
                    "g.scope_key",
                    "g.source_generation",
                ],
            ),
            None,
        )?;
    if pyrefly {
        joined = joined.join(
            unmapped_python(inputs)?,
            JoinType::Left,
            (
                vec![
                    "b.language",
                    "b.context_id",
                    "b.scope_key",
                    "b.source_generation",
                ],
                vec![
                    "m.language",
                    "m.context_id",
                    "m.scope_key",
                    "m.source_generation",
                ],
            ),
            None,
        )?;
    }
    let present = |name: &str| coalesce(vec![col(name), lit(0_i64)]).gt(lit(0_i64));
    let conditions = [
        present("g.targets"),
        present("g.locations"),
        present("g.callers"),
        if pyrefly {
            present("m.unmapped")
        } else {
            lit(false)
        },
    ];
    let incomplete = conditions
        .iter()
        .cloned()
        .reduce(Expr::or)
        .expect("nonempty gap classes");
    let replace = col("b.family")
        .eq(lit("call-targets"))
        .and(col("b.processing_state").eq(lit("complete")))
        .and(incomplete);
    let mut fragments = vec![lit("call_semantics_incomplete")];
    for (condition, reason) in conditions.into_iter().zip([
        ";unresolved_targets",
        ";source_locations",
        ";caller_entities",
        ";implicit_calls_not_normalized",
    ]) {
        fragments.push(datafusion::logical_expr::when(condition, lit(reason)).otherwise(lit(""))?);
    }
    let reason = datafusion::functions::string::expr_fn::concat(fragments);
    let broad = joined
        .project(
            fields
                .iter()
                .map(|(name, _, _)| {
                    Ok(match *name {
                        "processing_state" => {
                            datafusion::logical_expr::when(replace.clone(), lit("partial"))
                                .otherwise(col("b.processing_state"))?
                                .alias(*name)
                        }
                        "reason" => datafusion::logical_expr::when(replace.clone(), reason.clone())
                            .otherwise(col("b.reason"))?
                            .alias(*name),
                        _ => col(format!("b.{name}")),
                    })
                })
                .collect::<Result<Vec<_>, datafusion::common::DataFusionError>>()?,
        )?
        .build()?;
    let broad = LogicalPlanBuilder::from(broad).project(
        fields
            .iter()
            .map(|(name, _, _)| col(*name))
            .chain([lit(ScalarValue::FixedSizeBinary(16, None)).alias("owner_entity_id")]),
    )?;
    Ok(if rust.bodies {
        broad
            .union(call_owners::build(inputs, workspace)?)?
            .build()?
    } else {
        broad.build()?
    })
}

fn qualify_syntax(inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
    let roots = LogicalPlanBuilder::from(plan(inputs, super::syntax::RELATION)?)
        .filter(col("parent_entity_id").is_null())?
        .project([
            col("workspace_id"),
            col("file_id"),
            col("source_generation"),
        ])?
        .distinct()?
        .alias("s")?
        .build()?;
    let missing = col("b.family")
        .eq(lit("syntax-nodes"))
        .and(col("b.processing_state").eq(lit("complete")))
        .and(col("s.file_id").is_null());
    Ok(LogicalPlanBuilder::from(plan(inputs, INPUT)?)
        .alias("b")?
        .join(
            roots,
            JoinType::Left,
            (
                vec!["b.workspace_id", "b.file_id", "b.source_generation"],
                vec!["s.workspace_id", "s.file_id", "s.source_generation"],
            ),
            None,
        )?
        .project(
            fields()
                .iter()
                .map(|(name, _, _)| {
                    Ok(match *name {
                        "processing_state" => {
                            datafusion::logical_expr::when(missing.clone(), lit("partial"))
                                .otherwise(col("b.processing_state"))?
                                .alias(*name)
                        }
                        "reason" => datafusion::logical_expr::when(
                            missing.clone(),
                            lit("canonical_syntax_tree_unavailable"),
                        )
                        .otherwise(col("b.reason"))?
                        .alias(*name),
                        _ => col(format!("b.{name}")),
                    })
                })
                .collect::<Result<Vec<_>, datafusion::common::DataFusionError>>()?,
        )?
        .build()?)
}

fn qualify_references(
    inputs: &TransformationInputs,
    rust: super::RustInputs,
) -> Result<LogicalPlan, TransformationPlanError> {
    let mut base = qualify_syntax(inputs)?;
    for (relation, family, reason) in [
        (
            super::REFERENCE,
            "lexical-references",
            "lexical_reference_targets_unknown",
        ),
        (
            super::semantic_references::RELATION,
            "semantic-references",
            "canonical_semantic_targets_unknown",
        ),
        (
            super::imports::RELATION,
            "imports",
            "canonical_import_targets_unknown",
        ),
    ] {
        base = qualify_observation_gaps(
            base,
            plan(inputs, relation)?,
            family,
            reason,
            col("resolution")
                .not_eq(lit("resolved"))
                .or(col("target_entity_id").is_null()),
        )?;
    }
    for (relation, language) in [
        (super::types::GRAPH, Some("python")),
        (super::types::RUST_GRAPH, Some("rust")),
        (super::types::OBSERVATION, None),
        (super::types::CALLABLE, None),
    ] {
        let observations = plan(inputs, relation)?;
        let observations = if let Some(language) = language {
            let columns = observations
                .schema()
                .fields()
                .iter()
                .map(|field| col(field.name()))
                .collect::<Vec<_>>();
            LogicalPlanBuilder::from(observations)
                .project(columns.into_iter().chain([lit(language).alias("language")]))?
                .build()?
        } else {
            observations
        };
        base = qualify_observation_gaps(
            base,
            observations,
            "types",
            "canonical_types_unknown",
            col("type_id")
                .is_null()
                .or(col("unknown_reason").is_not_null()),
        )?;
    }
    base = qualify_observation_gaps(
        base,
        plan(inputs, super::types::MEMBER)?,
        "members",
        "canonical_members_unknown",
        col("unknown_reason")
            .is_not_null()
            .or(col("owner_entity_id").is_null()),
    )?;
    if rust.types {
        base = qualify_observation_gaps(
            base,
            missing_rust_type_graphs(inputs)?,
            "types",
            "native_type_source_unavailable",
            col("graph_type_key").is_null(),
        )?;
    }
    for (available, native, family) in [
        (
            rust.references,
            super::RustcRelation::HirReference,
            "semantic-references",
        ),
        (rust.imports, super::RustcRelation::HirImport, "imports"),
    ] {
        if available {
            base = qualify_observation_gaps(
                base,
                super::rust_references::missing_bindings(inputs, native)?,
                family,
                "compiler_observation_source_unavailable",
                lit(true),
            )?;
        }
    }
    Ok(base)
}

fn missing_rust_type_graphs(
    inputs: &TransformationInputs,
) -> Result<LogicalPlan, TransformationPlanError> {
    let raw = LogicalPlanBuilder::from(plan(inputs, super::RustcRelation::Type.relation_id())?)
        .filter(col("component_role").eq(lit("self")))?
        .alias("p")?
        .build()?;
    let runs = LogicalPlanBuilder::from(plan(inputs, RUN)?)
        .filter(col("provider").eq(lit("rustc")))?
        .alias("r")?
        .build()?;
    let graph = LogicalPlanBuilder::from(plan(inputs, super::types::RUST_GRAPH)?)
        .alias("g")?
        .build()?;
    let file = super::file_id_udf().call(vec![col("p.source_file_id")]);
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
            graph,
            JoinType::Left,
            vec![
                col("p.provider_run_id").eq(col("g.provider_run_identity")),
                col("p.owner_id").eq(col("g.provider_owner")),
                col("p.compilation_unit_id").eq(col("g.provider_compilation_unit")),
                col("p.type_key").eq(col("g.type_key")),
                col("p.source_generation").eq(col("g.source_generation")),
                col("p.source_content_digest").eq(col("g.content_digest")),
                file.clone().eq(col("g.file_id")),
            ],
        )?
        .project(vec![
            lit("rust").alias("language"),
            col("r.context_id").alias("context_id"),
            file.alias("file_id"),
            col("p.source_generation").alias("source_generation"),
            col("g.type_key").alias("graph_type_key"),
        ])?
        .build()?)
}

fn qualify_observation_gaps(
    base: LogicalPlan,
    references: LogicalPlan,
    family: &str,
    reason: &str,
    condition: Expr,
) -> Result<LogicalPlan, TransformationPlanError> {
    let gaps = LogicalPlanBuilder::from(references)
        .filter(condition)?
        .aggregate(
            vec![
                col("language"),
                col("context_id"),
                scope_key()?.alias("scope_key"),
                col("source_generation"),
            ],
            vec![count(lit(1_i64)).alias("gaps")],
        )?
        .alias("r")?
        .build()?;
    let base = LogicalPlanBuilder::from(base)
        .project(
            fields()
                .iter()
                .map(|(name, _, _)| col(*name))
                .chain([scope_key()?.alias("scope_key")]),
        )?
        .alias("b")?
        .join(
            gaps,
            JoinType::Left,
            (
                vec![
                    "b.language",
                    "b.context_id",
                    "b.scope_key",
                    "b.source_generation",
                ],
                vec![
                    "r.language",
                    "r.context_id",
                    "r.scope_key",
                    "r.source_generation",
                ],
            ),
            None,
        )?;
    let incomplete = col("b.family")
        .eq(lit(family))
        .and(col("b.processing_state").eq(lit("complete")))
        .and(coalesce(vec![col("r.gaps"), lit(0_i64)]).gt(lit(0_i64)));
    Ok(base
        .project(
            fields()
                .iter()
                .map(|(name, _, _)| {
                    Ok(match *name {
                        "processing_state" => {
                            datafusion::logical_expr::when(incomplete.clone(), lit("partial"))
                                .otherwise(col("b.processing_state"))?
                        }
                        "reason" => datafusion::logical_expr::when(incomplete.clone(), lit(reason))
                            .otherwise(col("b.reason"))?,
                        _ => col(format!("b.{name}")),
                    }
                    .alias(*name))
                })
                .collect::<Result<Vec<_>, datafusion::common::DataFusionError>>()?,
        )?
        .build()?)
}

fn unmapped_python(inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
    let syntax = LogicalPlanBuilder::from(plan(
        inputs,
        NativeSyntaxRelation::RuffCallableSyntax.as_str(),
    )?)
    .filter(col("role").eq(lit("callee-expression")))?
    .alias("x")?
    .build()?;
    Ok(
        LogicalPlanBuilder::from(super::python_calls::targets(inputs, true)?)
            .join(
                syntax,
                JoinType::LeftAnti,
                (
                    vec![
                        "p.file_id",
                        "p.content_digest",
                        "p.source_generation",
                        "p.context_id",
                        "p.start_byte",
                        "p.end_byte",
                    ],
                    vec![
                        "x.file_id",
                        "x.content_digest",
                        "x.source_generation",
                        "x.analysis_context_id",
                        "x.start_byte",
                        "x.end_byte",
                    ],
                ),
                None,
            )?
            .aggregate(
                vec![
                    lit("python").alias("language"),
                    col("p.context_id"),
                    col("p.file_id").alias("scope_key"),
                    col("p.source_generation"),
                ],
                vec![count(lit(1_i64)).alias("unmapped")],
            )?
            .alias("m")?
            .build()?,
    )
}

// Function source retrieval needs both terminal declaration coverage and a mapped syntax owner.
// Missing macro/generated/header mappings remain a remainder, not proof that no body exists.
fn function_source_scope(
    inputs: &TransformationInputs,
) -> Result<LogicalPlan, TransformationPlanError> {
    function_scope_from_plans(
        plan(inputs, INPUT)?,
        plan(inputs, super::DECLARATION)?,
        plan(inputs, super::source_context::RELATION)?,
    )
}

fn function_scope_from_plans(
    requested: LogicalPlan,
    declarations: LogicalPlan,
    mapped: LogicalPlan,
) -> Result<LogicalPlan, TransformationPlanError> {
    let declarations = LogicalPlanBuilder::from(declarations)
        .filter(col("entity_kind").eq(lit("function")))?
        .alias("d")?
        .build()?;
    let mapped = LogicalPlanBuilder::from(mapped)
        .filter(col("context_kind").eq(lit("function body")))?
        .alias("s")?
        .build()?;
    let gaps = LogicalPlanBuilder::from(declarations)
        .join(
            mapped,
            JoinType::LeftAnti,
            (
                vec![
                    "d.declaration_id",
                    "d.file_id",
                    "d.content_digest",
                    "d.source_generation",
                    "d.context_id",
                ],
                vec![
                    "s.declaration_id",
                    "s.file_id",
                    "s.content_digest",
                    "s.source_generation",
                    "s.context_id",
                ],
            ),
            None,
        )?
        .aggregate(
            vec![
                col("language"),
                col("context_id"),
                scope_key()?.alias("scope_key"),
                col("source_generation"),
            ],
            vec![count(lit(1_i64)).alias("gaps")],
        )?
        .alias("g")?
        .build()?;
    let base = LogicalPlanBuilder::from(requested)
        .filter(col("family").eq(lit("function-declarations")))?
        .project(
            fields()
                .iter()
                .map(|(name, _, _)| col(*name))
                .chain([scope_key()?.alias("scope_key")]),
        )?
        .alias("b")?
        .join(
            gaps,
            JoinType::Left,
            (
                vec![
                    "b.language",
                    "b.context_id",
                    "b.scope_key",
                    "b.source_generation",
                ],
                vec![
                    "g.language",
                    "g.context_id",
                    "g.scope_key",
                    "g.source_generation",
                ],
            ),
            None,
        )?;
    let incomplete = col("b.processing_state")
        .eq(lit("complete"))
        .and(coalesce(vec![col("g.gaps"), lit(0_i64)]).gt(lit(0_i64)));
    Ok(base
        .project(
            fields()
                .iter()
                .map(|(name, _, _)| {
                    Ok(match *name {
                        "family" => lit("function-source-context").alias(*name),
                        "processing_state" => {
                            datafusion::logical_expr::when(incomplete.clone(), lit("partial"))
                                .otherwise(col("b.processing_state"))?
                                .alias(*name)
                        }
                        "reason" => datafusion::logical_expr::when(
                            incomplete.clone(),
                            lit("source-syntax-owner-unmapped"),
                        )
                        .otherwise(col("b.reason"))?
                        .alias(*name),
                        _ => col(format!("b.{name}")),
                    })
                })
                .collect::<Result<Vec<_>, datafusion::common::DataFusionError>>()?,
        )?
        .build()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{ArrayRef, RecordBatch, StringArray};
    use datafusion::prelude::SessionContext;
    use std::sync::Arc;

    fn fixture_plan(context: &SessionContext, columns: &[(&str, &[&str])]) -> LogicalPlan {
        let batch = RecordBatch::try_from_iter(columns.iter().map(|(name, values)| {
            (
                *name,
                Arc::new(StringArray::from(values.to_vec())) as ArrayRef,
            )
        }))
        .unwrap();
        context.read_batch(batch).unwrap().into_unoptimized_plan()
    }

    #[tokio::test]
    async fn function_source_scope_keeps_missing_owners_and_exact_context_digest_pins() {
        let context = SessionContext::new();
        let requested = fixture_plan(
            &context,
            &[
                ("workspace_id", &["w"; 5]),
                ("source_generation", &["1"; 5]),
                ("input_set_id", &["i"; 5]),
                ("language", &["python", "python", "rust", "rust", "python"]),
                ("scope_kind", &["source"; 5]),
                (
                    "relative_path",
                    &["good.py", "bad.py", "lib.rs", "lib.rs", "pending.py"],
                ),
                ("target_name", &[""; 5]),
                ("context_id", &["py", "py", "rust-a", "rust-b", "py"]),
                ("target_kind", &[""; 5]),
                ("target_platform", &[""; 5]),
                ("build_profile", &[""; 5]),
                ("build_features", &[""; 5]),
                ("default_features", &[""; 5]),
                ("file_id", &["good", "bad", "lib", "lib", "pending"]),
                ("family", &["function-declarations"; 5]),
                (
                    "processing_state",
                    &["complete", "complete", "complete", "complete", "pending"],
                ),
                ("reason", &["", "", "", "", "provider-pending"]),
            ],
        );
        let declarations = fixture_plan(
            &context,
            &[
                ("language", &["python", "python", "rust", "rust", "python"]),
                ("context_id", &["py", "py", "rust-a", "rust-b", "py"]),
                ("file_id", &["good", "bad", "lib", "lib", "pending"]),
                ("source_generation", &["1"; 5]),
                ("content_digest", &["digest"; 5]),
                (
                    "declaration_id",
                    &[
                        "good-decl",
                        "bad-decl",
                        "rust-decl",
                        "rust-decl",
                        "pending-decl",
                    ],
                ),
                ("entity_kind", &["function"; 5]),
            ],
        );
        let mapped = fixture_plan(
            &context,
            &[
                ("context_id", &["py", "py", "rust-a"]),
                ("file_id", &["good", "bad", "lib"]),
                ("source_generation", &["1"; 3]),
                ("content_digest", &["digest", "stale-digest", "digest"]),
                ("declaration_id", &["good-decl", "bad-decl", "rust-decl"]),
                ("context_kind", &["function body"; 3]),
            ],
        );
        let selected = function_scope_from_plans(requested, declarations, mapped).unwrap();
        let batches = context
            .execute_logical_plan(selected)
            .await
            .unwrap()
            .collect()
            .await
            .unwrap();
        let rows = batches
            .iter()
            .flat_map(|batch| {
                let text = |name| {
                    batch
                        .column_by_name(name)
                        .unwrap()
                        .as_any()
                        .downcast_ref::<StringArray>()
                        .unwrap()
                };
                (0..batch.num_rows()).map(move |row| {
                    (
                        (
                            text("context_id").value(row).to_owned(),
                            text("file_id").value(row).to_owned(),
                        ),
                        (
                            text("processing_state").value(row).to_owned(),
                            text("reason").value(row).to_owned(),
                        ),
                    )
                })
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        for (ctx, file, state, reason) in [
            ("py", "good", "complete", ""),
            ("py", "bad", "partial", "source-syntax-owner-unmapped"),
            ("rust-a", "lib", "complete", ""),
            ("rust-b", "lib", "partial", "source-syntax-owner-unmapped"),
            ("py", "pending", "pending", "provider-pending"),
        ] {
            assert_eq!(
                rows[&(ctx.to_owned(), file.to_owned())],
                (state.to_owned(), reason.to_owned())
            );
        }
    }
}
