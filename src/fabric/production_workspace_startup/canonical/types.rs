//! Canonical structural types and source propositions over admitted native type graphs.

mod callable;
mod members;
mod normalize;
mod rust;
mod udf;

use super::{
    DECLARATION, DataType, Expr, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder, RUN, SOURCE,
    ScalarValue, TransformationInputs, TransformationPlanError, col, empty, file_id_udf, lit, plan,
    source_alias, source_occurrence_id,
};
use crate::pyrefly_service::PyreflyRelation;
use datafusion::functions::core::expr_fn::{get_field, named_struct};
use datafusion::functions_aggregate::{array_agg::array_agg, count::count_distinct};
use datafusion::logical_expr::when;

mod graph;

pub(super) const GRAPH: &str = "system.canonical_python_type_graph";
pub(super) const RUST_GRAPH: &str = rust::GRAPH;
pub(super) const TYPE: &str = "fact.code_type";
pub(super) const OBSERVATION: &str = "fact.code_type_observation";
pub(super) const COMPONENT: &str = "fact.code_type_component";
pub(super) const MEMBER: &str = "fact.code_member_observation";
pub(super) const CALLABLE: &str = "fact.code_callable_type";

#[derive(Clone, Copy)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent native relation availability is not a global provider state machine"
)]
pub(super) struct Inputs {
    python: bool,
    rust: bool,
    rust_observations: bool,
    rust_locals: bool,
}

impl Inputs {
    pub fn new(python: bool, rust: super::RustInputs) -> Self {
        Self {
            python,
            rust: rust.types,
            rust_observations: rust.types && rust.declarations,
            rust_locals: rust.bodies,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Relation {
    Graph,
    RustGraph,
    Type,
    Observation,
    Component,
    Callable,
    Member,
}

impl Relation {
    pub fn name(self) -> &'static str {
        match self {
            Self::Graph => GRAPH,
            Self::RustGraph => RUST_GRAPH,
            Self::Type => TYPE,
            Self::Observation => OBSERVATION,
            Self::Component => COMPONENT,
            Self::Callable => CALLABLE,
            Self::Member => MEMBER,
        }
    }

    pub fn fields(self) -> Vec<FieldSpec> {
        match self {
            Self::Callable => callable::fields(),
            Self::Member => members::fields(),
            Self::Graph => graph::fields(),
            Self::RustGraph => rust::fields(),
            Self::Type => vec![
                ("type_id", DataType::FixedSizeBinary(16), true),
                ("type_kind_code", DataType::Int32, true),
                ("canonical_key", DataType::Utf8, true),
                ("language", DataType::Utf8, false),
                ("context_id", DataType::FixedSizeBinary(16), false),
                ("workspace_id", DataType::FixedSizeBinary(16), false),
            ],
            Self::Observation => {
                let mut fields = scope_fields();
                fields.extend([
                    ("type_occurrence_id", DataType::FixedSizeBinary(16), true),
                    ("type_id", DataType::FixedSizeBinary(16), true),
                    ("type_role", DataType::Utf8, false),
                    ("start_byte", DataType::UInt64, true),
                    ("end_byte", DataType::UInt64, true),
                    ("unknown_reason", DataType::Utf8, true),
                    ("provider_occurrence_ordinal", DataType::UInt64, false),
                    ("provider_occurrence_kind", DataType::Utf8, false),
                    ("owner_entity_id", DataType::FixedSizeBinary(16), true),
                    ("provider_owner", DataType::Utf8, true),
                    ("provider_compilation_unit", DataType::Utf8, true),
                    ("provider_type_key", DataType::FixedSizeBinary(32), true),
                    ("provider_local_type_index", DataType::UInt64, true),
                ]);
                fields
            }
            Self::Component => {
                let mut fields = scope_fields();
                fields.extend([
                    ("owner_type_id", DataType::FixedSizeBinary(16), true),
                    ("referenced_type_id", DataType::FixedSizeBinary(16), true),
                    ("component_role", DataType::Utf8, false),
                    ("component_ordinal", DataType::UInt64, false),
                    ("parameter_kind", DataType::Utf8, true),
                    ("parameter_name", DataType::Utf8, true),
                    ("parameter_required", DataType::Boolean, true),
                    ("unknown_reason", DataType::Utf8, true),
                    ("owner_local_type_index", DataType::UInt64, true),
                    ("referenced_local_type_index", DataType::UInt64, true),
                    ("provider_owner", DataType::Utf8, true),
                    ("provider_compilation_unit", DataType::Utf8, true),
                    (
                        "provider_owner_type_key",
                        DataType::FixedSizeBinary(32),
                        true,
                    ),
                    (
                        "provider_referenced_type_key",
                        DataType::FixedSizeBinary(32),
                        true,
                    ),
                    ("provider_component_ordinal", DataType::UInt64, false),
                ]);
                fields
            }
        }
    }

    pub fn dependencies(self, available: Inputs) -> Vec<&'static str> {
        let mut result = match self {
            Self::Member => {
                let mut dependencies = vec![DECLARATION];
                if available.python {
                    dependencies.extend([
                        SOURCE,
                        RUN,
                        GRAPH,
                        PyreflyRelation::MemberObservation.relation_id(),
                    ]);
                }
                dependencies
            }
            Self::Callable => {
                let mut dependencies = vec![DECLARATION, OBSERVATION];
                if available.python {
                    dependencies.extend([
                        SOURCE,
                        RUN,
                        COMPONENT,
                        PyreflyRelation::TypeNode.relation_id(),
                    ]);
                }
                dependencies
            }
            Self::Graph if available.python => vec![
                SOURCE,
                RUN,
                DECLARATION,
                PyreflyRelation::TypeNode.relation_id(),
                PyreflyRelation::TypeEdge.relation_id(),
            ],
            Self::RustGraph if available.rust => vec![
                SOURCE,
                RUN,
                DECLARATION,
                crate::rustc_relation_schema::RustcRelation::Type.relation_id(),
            ],
            Self::Type => vec![GRAPH, RUST_GRAPH],
            Self::Observation if available.python => vec![
                SOURCE,
                RUN,
                GRAPH,
                PyreflyRelation::TypeObservation.relation_id(),
            ],
            Self::Component if available.python => {
                vec![GRAPH, PyreflyRelation::TypeEdge.relation_id()]
            }
            _ => vec![],
        };
        if matches!(self, Self::Observation) && available.rust_observations {
            result.extend([
                SOURCE,
                RUN,
                DECLARATION,
                RUST_GRAPH,
                super::RustcRelation::PublicItem.relation_id(),
            ]);
            if available.rust_locals {
                result.push(super::RustcRelation::MirLocal.relation_id());
            }
        }
        if matches!(self, Self::Component) && available.rust {
            result.extend([RUST_GRAPH, super::RustcRelation::Type.relation_id()]);
        }
        result.sort_unstable();
        result.dedup();
        result
    }

    pub fn build(
        self,
        workspace: [u8; 16],
        inputs: &TransformationInputs,
        available: Inputs,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        match self {
            Self::Callable => callable::build(inputs, available.python),
            Self::Member => members::build(inputs, available.python),
            Self::Graph if available.python => graph::build(inputs),
            Self::Graph => empty(self.fields()),
            Self::RustGraph => rust::build(workspace, inputs, available.rust),
            Self::Type => type_union(inputs),
            Self::Observation | Self::Component => {
                let python = if available.python {
                    Some(if matches!(self, Self::Observation) {
                        observations(workspace, inputs)?
                    } else {
                        components(inputs)?
                    })
                } else {
                    None
                };
                let rust =
                    match self {
                        Self::Observation if available.rust_observations => Some(
                            rust::observations(workspace, inputs, available.rust_locals)?,
                        ),
                        Self::Component if available.rust => Some(rust::components(inputs)?),
                        _ => None,
                    };
                union(self, python.into_iter().chain(rust))
            }
        }
    }
}

fn union(
    relation: Relation,
    plans: impl Iterator<Item = LogicalPlan>,
) -> Result<LogicalPlan, TransformationPlanError> {
    super::canonical_union(relation.name(), relation.fields(), plans)
}

fn type_union(inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
    let branch = |relation, language| -> Result<LogicalPlan, TransformationPlanError> {
        Ok(LogicalPlanBuilder::from(plan(inputs, relation)?)
            .filter(col("type_id").is_not_null())?
            .project(Relation::Type.fields().iter().map(|(name, _, _)| {
                if *name == "language" {
                    lit(language).alias(*name)
                } else {
                    col(*name)
                }
            }))?
            .project(super::canonical_projection(TYPE, &Relation::Type.fields()))?
            .build()?)
    };
    Ok(LogicalPlanBuilder::from(branch(GRAPH, "python")?)
        .union(branch(RUST_GRAPH, "rust")?)?
        .distinct()?
        .build()?)
}

fn scope_fields() -> Vec<FieldSpec> {
    vec![
        ("language", DataType::Utf8, false),
        ("context_id", DataType::FixedSizeBinary(16), false),
        ("file_id", DataType::FixedSizeBinary(16), true),
        ("content_digest", DataType::FixedSizeBinary(32), false),
        ("source_generation", DataType::UInt64, false),
        ("provider_run_id", DataType::FixedSizeBinary(16), false),
        ("provider", DataType::Utf8, false),
        ("workspace_id", DataType::FixedSizeBinary(16), false),
    ]
}

fn scope_projection(alias: &str) -> Vec<Expr> {
    scope_fields()
        .iter()
        .map(|(name, _, _)| match *name {
            "language" => lit("python").alias(*name),
            "provider" => lit("pyrefly").alias(*name),
            _ => col(format!("{alias}.{name}")).alias(*name),
        })
        .collect()
}

fn raw(
    inputs: &TransformationInputs,
    relation: PyreflyRelation,
) -> Result<LogicalPlanBuilder, TransformationPlanError> {
    Ok(LogicalPlanBuilder::from(plan(inputs, relation.relation_id())?).alias("p")?)
}

fn graph_alias(
    inputs: &TransformationInputs,
    alias: &str,
) -> Result<LogicalPlan, TransformationPlanError> {
    Ok(LogicalPlanBuilder::from(plan(inputs, GRAPH)?)
        .alias(alias)?
        .build()?)
}

fn graph_join(alias: &str, index: &str) -> Vec<Expr> {
    vec![
        col("p.provider_run_id").eq(col(format!("{alias}.provider_run_identity"))),
        file_id_udf()
            .call(vec![col("p.file_id")])
            .eq(col(format!("{alias}.file_id"))),
        col("p.content_digest").eq(col(format!("{alias}.content_digest"))),
        col("p.source_generation").eq(col(format!("{alias}.source_generation"))),
        col(format!("p.{index}")).eq(col(format!("{alias}.local_type_index"))),
    ]
}

fn observations(
    workspace: [u8; 16],
    inputs: &TransformationInputs,
) -> Result<LogicalPlan, TransformationPlanError> {
    // An occurrence with no native index remains an accepted unknown observation.
    let joined = graph::accepted(raw(inputs, PyreflyRelation::TypeObservation)?, inputs)?.join_on(
        graph_alias(inputs, "g")?,
        JoinType::Left,
        graph_join("g", "local_type_index"),
    )?;
    let file = file_id_udf().call(vec![col("p.file_id")]);
    let located = col("p.start_byte")
        .lt(col("p.end_byte"))
        .and(col("p.end_byte").lt_eq(col("s.byte_length")));
    let occurrence = source_occurrence_id(workspace, "codefabric_type_occurrence_id_v1", 104, 5)
        .call(vec![
            file.clone(),
            file.clone(),
            col("p.content_digest"),
            col("p.start_byte"),
            col("p.end_byte"),
        ]);
    Ok(joined
        .project(vec![
            lit("python").alias("language"),
            col("r.context_id").alias("context_id"),
            file.alias("file_id"),
            col("p.content_digest").alias("content_digest"),
            col("p.source_generation").alias("source_generation"),
            col("r.provider_run_id").alias("provider_run_id"),
            lit("pyrefly").alias("provider"),
            col("s.workspace_id").alias("workspace_id"),
            when(located.clone(), occurrence)
                .otherwise(lit(ScalarValue::FixedSizeBinary(16, None)))?
                .alias("type_occurrence_id"),
            col("g.type_id").alias("type_id"),
            col("p.type_role").alias("type_role"),
            col("p.start_byte").alias("start_byte"),
            col("p.end_byte").alias("end_byte"),
            when(!located, lit("type_location_unavailable"))
                .when(
                    col("p.local_type_index").is_null(),
                    lit("native_type_graph_limit"),
                )
                .when(
                    col("g.local_type_index").is_null(),
                    lit("native_type_node_unavailable"),
                )
                .otherwise(col("g.unknown_reason"))?
                .alias("unknown_reason"),
            col("p.occurrence_ordinal").alias("provider_occurrence_ordinal"),
            lit("checker-expression").alias("provider_occurrence_kind"),
            lit(ScalarValue::FixedSizeBinary(16, None)).alias("owner_entity_id"),
            lit(ScalarValue::Utf8(None)).alias("provider_owner"),
            lit(ScalarValue::Utf8(None)).alias("provider_compilation_unit"),
            lit(ScalarValue::FixedSizeBinary(32, None)).alias("provider_type_key"),
            col("p.local_type_index").alias("provider_local_type_index"),
        ])?
        .build()?)
}

fn components(inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
    let joined = raw(inputs, PyreflyRelation::TypeEdge)?
        .join_on(
            graph_alias(inputs, "g")?,
            JoinType::Inner,
            graph_join("g", "owner_local_type_index"),
        )?
        .join_on(
            graph_alias(inputs, "t")?,
            JoinType::Left,
            graph_join("t", "referenced_local_type_index"),
        )?;
    let mut projection = scope_projection("g");
    projection.extend([
        col("g.type_id").alias("owner_type_id"),
        col("t.type_id").alias("referenced_type_id"),
    ]);
    projection.extend(
        [
            "component_role",
            "component_ordinal",
            "parameter_kind",
            "parameter_name",
            "parameter_required",
        ]
        .map(|name| col(format!("p.{name}")).alias(name)),
    );
    projection.push(
        when(
            col("g.unknown_reason").is_not_null(),
            col("g.unknown_reason"),
        )
        .when(
            col("t.local_type_index").is_null(),
            lit("native_type_component_unavailable"),
        )
        .otherwise(col("t.unknown_reason"))?
        .alias("unknown_reason"),
    );
    projection.extend(
        ["owner_local_type_index", "referenced_local_type_index"]
            .map(|name| col(format!("p.{name}")).alias(name)),
    );
    projection.extend([
        lit(ScalarValue::Utf8(None)).alias("provider_owner"),
        lit(ScalarValue::Utf8(None)).alias("provider_compilation_unit"),
        lit(ScalarValue::FixedSizeBinary(32, None)).alias("provider_owner_type_key"),
        lit(ScalarValue::FixedSizeBinary(32, None)).alias("provider_referenced_type_key"),
        col("p.component_ordinal").alias("provider_component_ordinal"),
    ]);
    Ok(joined.project(projection)?.distinct()?.build()?)
}
