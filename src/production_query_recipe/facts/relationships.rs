//! One-step semantic denotations retain canonical occurrences and exact target-context evidence.

use super::{
    Arc, EpochBoundRequestInputField, EpochBoundSelectionFold, EpochBoundSelectionValueResolution,
    JoinKind, ProductionOperatorDefinition, ProductionQueryRecipeError,
    ProductionRelationAuthority, ProductionRelationDefinition, ProductionRequestInputDefinition,
    ProductionSelectionDefinition, ProductionSemanticFormProgram, ProgramJoinPredicate,
    ProgramProjectionField, ProgramRelationalOperator, ProgramSortField, ProgrammaticFabricEpoch,
    ProgrammaticRelationId, RELEASE_SELECTION_MAXIMUM_VALUES, ReleasedSemanticForm, ResultRole,
    ScalarOperator, SemanticClauseValue, SemanticValueKind, families, release_field_id,
    release_relation_id,
};
use crate::relational_semantic_query::{
    EpochBoundSelectionTarget, ProgramGroupField, ProgramPublicEntityKind,
};

pub(crate) fn known_meaning(value: &str) -> bool {
    matches!(
        value,
        "calls"
            | "call targets"
            | "call-targets"
            | "call relationships"
            | "lexical references"
            | "lexical-references"
            | "semantic references"
            | "semantic-references"
            | "imports"
            | "syntax parents"
            | "syntax-nodes"
            | "incoming"
            | "outgoing"
            | "one relationship step"
            | "one step"
    )
}

pub(crate) fn is_result(relation: &str) -> bool {
    relation.starts_with("query.result.relationship-")
}

pub(in crate::production_query_recipe) fn programs(
    epoch: &ProgrammaticFabricEpoch,
) -> Result<Vec<ProductionSemanticFormProgram>, ProductionQueryRecipeError> {
    let mut programs = Vec::new();
    for (family, relation, occurrence, kind, meaning, target) in [
        (
            "semantic-references",
            "fact.code_semantic_reference",
            "reference_id",
            "reference",
            "semantic references",
            "target_entity_id",
        ),
        (
            "imports",
            "fact.code_import",
            "import_id",
            "import-occurrence",
            "imports",
            "target_entity_id",
        ),
        (
            "call-targets",
            "fact.code_call_site",
            "call_site_id",
            "call",
            "call targets",
            "target_entity_id",
        ),
        (
            "syntax-nodes",
            "fact.code_syntax_node",
            "entity_id",
            "syntax-node",
            "syntax parents",
            "parent_entity_id",
        ),
    ] {
        for direction in ["incoming", "outgoing"] {
            if let Some(program) = program(
                epoch, family, relation, occurrence, kind, meaning, direction, target,
            )? {
                programs.push(program);
            }
        }
    }
    Ok(programs)
}

// Keep source lineage, exact target joins and the program's public selection contract together.
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn program(
    epoch: &ProgrammaticFabricEpoch,
    family: &str,
    relation: &str,
    occurrence: &str,
    kind: &str,
    meaning: &str,
    direction: &str,
    target: &str,
) -> Result<Option<ProductionSemanticFormProgram>, ProductionQueryRecipeError> {
    let Some(source) = families::definition(epoch, relation)? else {
        return Ok(None);
    };
    let entity_relation = "fact.code_entity";
    let Some(entities) = families::definition(epoch, entity_relation)? else {
        return Ok(None);
    };
    let schema = epoch
        .relation(&ProgrammaticRelationId::new(relation))
        .expect("bound source")
        .contract
        .logical_schema();
    let output = format!("query.result.relationship-{family}-{direction}");
    let binding = format!("program.semantic-query.relationship-{family}-{direction}.v1");
    let input_id = release_relation_id(&format!("query.input.relationship-{family}-{direction}"))?;
    let field = |relation: &str, name: &str| release_field_id(&format!("{relation}.{name}"));
    let request_fields = [
        "starting-from.kind",
        "starting-from.value",
        "starting-from.producer-role",
    ]
    .into_iter()
    .map(release_field_id)
    .collect::<Result<Vec<_>, _>>()?;
    let target_fields = ["entity_id", "entity_kind", "context_id", "workspace_id"]
        .into_iter()
        .map(|name| field(entity_relation, name))
        .collect::<Result<Vec<_>, _>>()?;
    let mut projections = source
        .fields
        .iter()
        .zip(schema.fields())
        .filter(|(_, f)| f.name() != "public_entity_id")
        .map(|(id, f)| {
            Ok(ProgramProjectionField {
                input_field_id: id.clone(),
                output_field_id: field(&output, f.name())?,
                output_name: None,
                output_nullable: None,
                public_entity_kind: None,
            })
        })
        .collect::<Result<Vec<_>, ProductionQueryRecipeError>>()?;
    projections.push(ProgramProjectionField {
        input_field_id: target_fields[1].clone(),
        output_field_id: field(&output, "target_entity_kind")?,
        output_name: Some(Arc::from("target_entity_kind")),
        output_nullable: Some(family != "syntax-nodes"),
        public_entity_kind: None,
    });
    for (name, is_target) in [
        ("public_source_entity_id", false),
        ("public_occurrence_id", false),
        ("public_target_entity_id", true),
        ("public_entity_id", direction == "incoming"),
    ] {
        projections.push(ProgramProjectionField {
            input_field_id: field(relation, if is_target { target } else { occurrence })?,
            output_field_id: field(&output, name)?,
            output_name: Some(Arc::from(name)),
            output_nullable: Some(true),
            public_entity_kind: Some(if is_target {
                ProgramPublicEntityKind::Field(target_fields[1].clone())
            } else {
                ProgramPublicEntityKind::Literal(Arc::from(kind))
            }),
        });
    }
    let output_fields = projections
        .iter()
        .map(|f| f.output_field_id.clone())
        .collect::<Vec<_>>();
    let node = |name: &str| Arc::<str>::from(format!("{binding}.{name}"));
    let mut operators = Vec::new();
    let mut add = |name: &str, inputs: &[&str], operator, output_fields| {
        operators.push(ProductionOperatorDefinition {
            node_id: node(name),
            ordinal: u32::try_from(operators.len()).expect("finite template"),
            input_node_ids: inputs.iter().map(|name| node(name)).collect(),
            operator,
            output_fields,
        });
    };
    add(
        "source",
        &[],
        ProgramRelationalOperator::Input {
            relation_id: source.relation_id.clone(),
        },
        source.fields.clone(),
    );
    add(
        "entities",
        &[],
        ProgramRelationalOperator::Input {
            relation_id: entities.relation_id.clone(),
        },
        entities.fields.clone(),
    );
    // A target can have several canonical names; denotation evidence must not multiply with them.
    add(
        "target-kinds",
        &["entities"],
        ProgramRelationalOperator::Aggregate {
            group_by: target_fields
                .iter()
                .map(|id| ProgramGroupField {
                    input_field_id: id.clone(),
                    output_field_id: id.clone(),
                })
                .collect(),
            aggregates: vec![],
        },
        target_fields.clone(),
    );
    let mut joined = source.fields.clone();
    joined.extend(target_fields.clone());
    add(
        "targets",
        &["source", "target-kinds"],
        ProgramRelationalOperator::Join {
            kind: if family == "syntax-nodes" {
                JoinKind::Inner
            } else {
                JoinKind::Left
            },
            predicates: [
                (target, "entity_id"),
                ("context_id", "context_id"),
                ("workspace_id", "workspace_id"),
            ]
            .into_iter()
            .map(|(left, right)| {
                Ok(ProgramJoinPredicate {
                    left_field_id: field(relation, left)?,
                    right_field_id: field(entity_relation, right)?,
                    scalar_operator: ScalarOperator::Equal,
                })
            })
            .collect::<Result<_, ProductionQueryRecipeError>>()?,
        },
        joined,
    );
    add(
        "project",
        &["targets"],
        ProgramRelationalOperator::Projection {
            fields: projections,
        },
        output_fields.clone(),
    );
    add(
        "request",
        &[],
        ProgramRelationalOperator::Input {
            relation_id: input_id.clone(),
        },
        request_fields.clone(),
    );
    add(
        "subjects",
        &["project", "request"],
        ProgramRelationalOperator::Join {
            kind: JoinKind::LeftSemi,
            predicates: vec![ProgramJoinPredicate {
                left_field_id: field(&output, "public_entity_id")?,
                right_field_id: request_fields[1].clone(),
                scalar_operator: ScalarOperator::Equal,
            }],
        },
        output_fields.clone(),
    );
    add(
        "families",
        &["subjects"],
        ProgramRelationalOperator::Filter,
        output_fields.clone(),
    );
    add(
        "sort",
        &["families"],
        ProgramRelationalOperator::Sort {
            fields: output_fields
                .iter()
                .map(|id| ProgramSortField {
                    input_field_id: id.clone(),
                    ascending: true,
                    nulls_first: false,
                })
                .collect(),
        },
        output_fields.clone(),
    );
    add(
        "limit",
        &["sort"],
        ProgramRelationalOperator::Limit { skip: 0 },
        output_fields.clone(),
    );
    let output_id = release_relation_id(&output)?;
    Ok(Some(ProductionSemanticFormProgram {
        form: ReleasedSemanticForm::FollowCodeRelationships,
        program_binding_id: Arc::from(binding.clone()),
        output_role_id: Arc::from(ResultRole::Facts.released_id()),
        root_node_id: node("limit"),
        output_relation_id: output_id.clone(),
        output_fields: output_fields.clone(),
        relations: vec![
            source,
            entities,
            ProductionRelationDefinition {
                relation_id: input_id.clone(),
                fields: request_fields.clone(),
                authority: ProductionRelationAuthority::QueryLocal,
            },
            ProductionRelationDefinition {
                relation_id: output_id,
                fields: output_fields,
                authority: ProductionRelationAuthority::ProgramResult,
            },
        ],
        operators,
        selections: [
            ("selection.relationship", vec![meaning, family], family),
            ("selection.direction", vec![direction], direction),
            (
                "selection.distance",
                vec!["one relationship step", "one step"],
                "one relationship step",
            ),
        ]
        .into_iter()
        .map(|(id, meanings, value)| {
            let mut meanings = meanings;
            meanings.sort_unstable();
            meanings.dedup();
            ProductionSelectionDefinition {
                selection_id: Arc::from(id),
                value_kind: SemanticValueKind::Text,
                minimum_values: 1,
                maximum_values: 1,
                operator_node_id: node("families"),
                target: EpochBoundSelectionTarget::Program,
                fold: EpochBoundSelectionFold::Any,
                resolutions: meanings
                    .into_iter()
                    .map(|meaning| EpochBoundSelectionValueResolution {
                        request_value: SemanticClauseValue::Text(Arc::from(meaning)),
                        execution_value: SemanticClauseValue::Text(Arc::from(value)),
                    })
                    .collect(),
            }
        })
        .collect(),
        request_inputs: vec![ProductionRequestInputDefinition {
            input_id: Arc::from("input.starting-from"),
            relation_id: input_id,
            fields: request_fields
                .into_iter()
                .enumerate()
                .map(|(index, field_id)| EpochBoundRequestInputField {
                    field_id,
                    value_kind: SemanticValueKind::Text,
                    required: index < 2,
                })
                .collect(),
            minimum_rows: 1,
            maximum_rows: RELEASE_SELECTION_MAXIMUM_VALUES,
        }],
        returns: vec![],
        consumer_slots: vec![],
        required_fact_families: vec![],
    }))
}
