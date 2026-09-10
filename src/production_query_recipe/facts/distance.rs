//! Finite relationship walks use native semi joins, never enumerate paths in application memory.

use super::{
    Arc, EpochBoundSelectionValueResolution, FieldId, JoinKind, ProductionOperatorDefinition,
    ProductionQueryRecipeError, ProductionSemanticFormProgram, ProgramJoinPredicate,
    ProgramRelationalOperator, ScalarOperator, SemanticClauseValue, release_field_id,
    release_relation_id,
};
use crate::relational_program::UnionKind;
use crate::relational_semantic_query::EpochBoundSelectionTarget;

// Each step adds only a narrow frontier projection and a semi join. Keep the released templates
// inside the ordinary compiler's finite node bound, including named/location/prior subject nodes.
const NUMBERS: [&str; 8] = [
    "one", "two", "three", "four", "five", "six", "seven", "eight",
];

pub(super) fn calls(
    base: &ProductionSemanticFormProgram,
) -> Result<Vec<ProductionSemanticFormProgram>, ProductionQueryRecipeError> {
    let mut programs = Vec::new();
    let source = &base.operators[0].output_fields;
    if [
        "public_caller_entity_id",
        "public_target_entity_id",
        "workspace_id",
        "context_id",
    ]
    .iter()
    .any(|name| {
        !source
            .iter()
            .any(|field| field.as_str().ends_with(&format!(".{name}")))
    }) {
        // A retained epoch can still serve its older one-step contract without admitting walks.
        return Ok(programs);
    }
    for direction in ["incoming", "outgoing"] {
        for steps in 2..=NUMBERS.len() {
            for cumulative in [false, true] {
                programs.push(walk(base, direction, steps, cumulative)?);
            }
        }
    }
    Ok(programs)
}

fn meanings(steps: usize, cumulative: bool) -> Vec<String> {
    let prefix = if cumulative { "up to " } else { "" };
    [NUMBERS[steps - 1].to_owned(), steps.to_string()]
        .into_iter()
        .flat_map(|number| {
            [
                format!("{prefix}{number} relationship steps"),
                format!("{prefix}{number} steps"),
            ]
        })
        .collect()
}

pub(super) fn known_meaning(value: &str) -> bool {
    (2..=NUMBERS.len()).any(|steps| {
        [false, true].into_iter().any(|cumulative| {
            meanings(steps, cumulative)
                .iter()
                .any(|meaning| meaning == value)
        })
    })
}

// Keep the bounded plan construction with its distance/direction and per-edge family contracts.
#[allow(clippy::too_many_lines)]
fn walk(
    base: &ProductionSemanticFormProgram,
    direction: &str,
    steps: usize,
    cumulative: bool,
) -> Result<ProductionSemanticFormProgram, ProductionQueryRecipeError> {
    let mut program = base.clone();
    let old = base.program_binding_id.as_ref();
    let mode = if cumulative { "through" } else { "exact" };
    let binding = format!("program.semantic-query.calls-{direction}-{mode}-{steps}.v1");
    let output = release_relation_id(&format!(
        "query.result.call-walk-{direction}-{mode}-{steps}"
    ))?;
    let output_fields = program
        .output_fields
        .iter()
        .map(|field| {
            Ok((
                field.clone(),
                release_field_id(&format!(
                    "{}.{}",
                    output.as_str(),
                    field
                        .as_str()
                        .rsplit('.')
                        .next()
                        .expect("call field suffix")
                ))?,
            ))
        })
        .collect::<Result<std::collections::BTreeMap<_, _>, ProductionQueryRecipeError>>()?;
    // Result fields belong to this result relation's authority. Scope predicates and later block
    // namespaces bind these declared identities, while epoch inputs keep their original IDs.
    let rename_field = |field: &mut FieldId| {
        if let Some(renamed) = output_fields.get(field) {
            *field = renamed.clone();
        }
    };
    program.output_fields.iter_mut().for_each(rename_field);
    for relation in &mut program.relations {
        if relation.relation_id == program.output_relation_id {
            relation.relation_id = output.clone();
            relation.fields.iter_mut().for_each(rename_field);
        }
    }
    program.output_relation_id = output;
    let rename = |id: &Arc<str>| Arc::from(id.replacen(old, &binding, 1));
    program.program_binding_id = Arc::from(binding.as_str());
    program.root_node_id = rename(&program.root_node_id);
    for node in &mut program.operators {
        node.node_id = rename(&node.node_id);
        node.input_node_ids = node.input_node_ids.iter().map(rename).collect();
        node.output_fields.iter_mut().for_each(rename_field);
        match &mut node.operator {
            ProgramRelationalOperator::Projection { fields } => {
                for field in fields {
                    rename_field(&mut field.output_field_id);
                }
            }
            ProgramRelationalOperator::Sort { fields } => {
                for field in fields {
                    rename_field(&mut field.input_field_id);
                }
            }
            _ => {}
        }
    }
    for selection in &mut program.selections {
        selection.operator_node_id = rename(&selection.operator_node_id);
        match selection.selection_id.as_ref() {
            "selection.relationship" => selection.resolutions.retain(|resolution| {
                matches!(&resolution.execution_value, SemanticClauseValue::Text(value) if value.as_ref() == "calls")
            }),
            "selection.direction" => selection.resolutions.retain(|resolution| {
                matches!(&resolution.execution_value, SemanticClauseValue::Text(value) if value.as_ref() == direction)
            }),
            "selection.distance" => {
                // Distance selects this walk; the source's distance column describes each atomic
                // relationship witness and must not be compared to the requested walk length.
                selection.target = EpochBoundSelectionTarget::Program;
                let meanings = meanings(steps, cumulative);
                selection.resolutions = meanings.iter().map(|meaning| {
                    EpochBoundSelectionValueResolution {
                        request_value: SemanticClauseValue::Text(Arc::from(meaning.as_str())),
                        execution_value: SemanticClauseValue::Text(Arc::from(meanings[0].as_str())),
                    }
                }).collect();
            }
            _ => {}
        }
    }
    let node_id = |name: &str| Arc::<str>::from(format!("{binding}.{name}"));
    let take = |name: &str| {
        program
            .operators
            .iter()
            .find(|node| node.node_id == node_id(name))
            .cloned()
            .expect("compiled call template node")
    };
    let source = take("source");
    let mut families = take("families");
    let mut subjects = take("subjects");
    let mut project = take("project");
    let mut operators = vec![source.clone(), take("about")];
    // Filter every traversed edge, not only the final witnesses. Named and prior subjects are
    // installed later at the same subjects node and therefore only constrain the starting set.
    families.input_node_ids = vec![source.node_id];
    subjects.input_node_ids[0] = families.node_id.clone();
    operators.extend([families.clone(), subjects.clone()]);
    let field = |name: &str| -> Result<FieldId, ProductionQueryRecipeError> {
        families
            .output_fields
            .iter()
            .find(|id| id.as_str().ends_with(&format!(".{name}")))
            .cloned()
            .ok_or_else(|| ProductionQueryRecipeError::InvalidCompiledRelease {
                detail: format!("call walk requires {name}"),
            })
    };
    let endpoint = field(if direction == "incoming" {
        "public_caller_entity_id"
    } else {
        "public_target_entity_id"
    })?;
    let mut previous = subjects.node_id.clone();
    let mut witnesses = vec![previous.clone()];
    let ProgramRelationalOperator::Projection { fields: projected } = &project.operator else {
        unreachable!("compiled call projection")
    };
    let frontier_projection = [endpoint, field("context_id")?, field("workspace_id")?]
        .into_iter()
        .map(|input| {
            projected
                .iter()
                .find(|projection| projection.input_field_id == input)
                .cloned()
                .expect("call endpoint output lineage")
        })
        .collect::<Vec<_>>();
    let frontier_fields = frontier_projection
        .iter()
        .map(|projection| projection.output_field_id.clone())
        .collect::<Vec<_>>();
    for step in 2..=steps {
        let frontier = node_id(&format!("frontier-{step}"));
        operators.push(ProductionOperatorDefinition {
            node_id: frontier.clone(),
            ordinal: 0,
            input_node_ids: vec![previous],
            operator: ProgramRelationalOperator::Projection {
                fields: frontier_projection.clone(),
            },
            output_fields: frontier_fields.clone(),
        });
        previous = node_id(&format!("step-{step}"));
        operators.push(ProductionOperatorDefinition {
            node_id: previous.clone(),
            ordinal: 0,
            input_node_ids: vec![families.node_id.clone(), frontier],
            operator: ProgramRelationalOperator::Join {
                kind: JoinKind::LeftSemi,
                predicates: [
                    field("public_entity_id")?,
                    field("context_id")?,
                    field("workspace_id")?,
                ]
                .into_iter()
                .zip(frontier_fields.iter().cloned())
                .map(|(left_field_id, right_field_id)| ProgramJoinPredicate {
                    left_field_id,
                    right_field_id,
                    scalar_operator: ScalarOperator::Equal,
                })
                .collect(),
            },
            output_fields: families.output_fields.clone(),
        });
        witnesses.push(previous.clone());
    }
    if cumulative {
        previous = node_id("walk-witnesses");
        operators.push(ProductionOperatorDefinition {
            node_id: previous.clone(),
            ordinal: 0,
            input_node_ids: witnesses,
            operator: ProgramRelationalOperator::Union {
                kind: UnionKind::Distinct,
            },
            output_fields: families.output_fields.clone(),
        });
    }
    project.input_node_ids = vec![previous];
    operators.extend([project, take("sort"), take("limit")]);
    for (ordinal, operator) in operators.iter_mut().enumerate() {
        operator.ordinal = u32::try_from(ordinal).expect("bounded walk operators");
    }
    program.operators = operators;
    Ok(program)
}
