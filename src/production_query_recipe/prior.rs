//! Entity-result subjects join their exact native context before family selection and row limits.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::{
    EpochBoundConsumerComposition, JoinKind, ProductionConsumerSlotDefinition,
    ProductionOperatorDefinition, ProductionQueryRecipeError, ProductionRelationAuthority,
    ProductionRelationDefinition, ProductionSemanticFormProgram, ProgramRelationalOperator,
    ReleasedSemanticForm, ResultRole, ScalarOperator, UnionKind, release_field_id,
};
use crate::relational_semantic_query::ProgramJoinPredicate;

// Keep the three inserted native operators beside their schema and slot contract.
#[allow(clippy::too_many_lines)]
pub(super) fn bind_entity_subjects(
    programs: &mut BTreeMap<(ReleasedSemanticForm, Arc<str>), ProductionSemanticFormProgram>,
    maximum_edges: usize,
) -> Result<(), ProductionQueryRecipeError> {
    let Some(entities) = programs
        .values()
        .find(|program| program.form == ReleasedSemanticForm::FindCodeEntities)
        .cloned()
    else {
        return Ok(());
    };
    let public_id = release_field_id("query.result.semantic-entities.public-entity-id")?;
    let context_id = release_field_id("query.result.semantic-entities.analysis-context-id")?;
    if !entities.output_fields.contains(&public_id) || !entities.output_fields.contains(&context_id)
    {
        return Ok(());
    }
    for program in programs.values_mut() {
        let slot = match program.form {
            ReleasedSemanticForm::RetrieveFactsAboutCode => "slot.about",
            ReleasedSemanticForm::FollowCodeRelationships => "slot.starting-from",
            ReleasedSemanticForm::RetrieveSourceAndSyntaxContext => "slot.for-inputs",
            _ => continue,
        };
        let node =
            |suffix: &str| Arc::<str>::from(format!("{}.{suffix}", program.program_binding_id));
        let Some(index) = program
            .operators
            .iter()
            .position(|operator| operator.node_id == node("subjects"))
        else {
            continue;
        };
        let subjects = program.operators[index].clone();
        let field = |suffix: &str| {
            subjects
                .output_fields
                .iter()
                .find(|id| id.as_str().ends_with(suffix))
                .cloned()
        };
        let (Some(subject_id), Some(subject_context)) =
            (field(".public_entity_id"), field(".context_id"))
        else {
            continue;
        };
        let prior_input = node("prior-input");
        let prior_subjects = node("prior-subjects");
        let all_subjects = node("all-subjects");
        for operator in &mut program.operators {
            for input in &mut operator.input_node_ids {
                if *input == subjects.node_id {
                    *input = Arc::clone(&all_subjects);
                }
            }
        }
        let insertion = index + 1;
        program.operators.splice(
            insertion..insertion,
            [
                ProductionOperatorDefinition {
                    node_id: Arc::clone(&prior_input),
                    ordinal: 0,
                    input_node_ids: vec![],
                    operator: ProgramRelationalOperator::Input {
                        relation_id: entities.output_relation_id.clone(),
                    },
                    output_fields: entities.output_fields.clone(),
                },
                ProductionOperatorDefinition {
                    node_id: Arc::clone(&prior_subjects),
                    ordinal: 0,
                    input_node_ids: vec![Arc::clone(&subjects.input_node_ids[0]), prior_input],
                    operator: ProgramRelationalOperator::Join {
                        kind: JoinKind::LeftSemi,
                        predicates: [
                            (subject_id, public_id.clone()),
                            (subject_context, context_id.clone()),
                        ]
                        .into_iter()
                        .map(|(left_field_id, right_field_id)| ProgramJoinPredicate {
                            left_field_id,
                            right_field_id,
                            scalar_operator: ScalarOperator::Equal,
                        })
                        .collect(),
                    },
                    output_fields: subjects.output_fields.clone(),
                },
                ProductionOperatorDefinition {
                    node_id: all_subjects,
                    ordinal: 0,
                    input_node_ids: vec![subjects.node_id, prior_subjects],
                    operator: ProgramRelationalOperator::Union {
                        kind: UnionKind::Distinct,
                    },
                    output_fields: subjects.output_fields,
                },
            ],
        );
        for (ordinal, operator) in program.operators.iter_mut().enumerate() {
            operator.ordinal = u32::try_from(ordinal).expect("bounded program operators");
        }
        program.relations.push(ProductionRelationDefinition {
            relation_id: entities.output_relation_id.clone(),
            fields: entities.output_fields.clone(),
            authority: ProductionRelationAuthority::QueryLocal,
        });
        program
            .consumer_slots
            .push(ProductionConsumerSlotDefinition {
                consumer_slot_id: Arc::from(slot),
                consumer_role_id: Arc::from(ResultRole::Entities.released_id()),
                input_relation_id: entities.output_relation_id.clone(),
                minimum_edges: 0,
                maximum_edges,
                composition: EpochBoundConsumerComposition::MaterializedUnion,
            });
        // Prior subjects arrive only through their typed dependency. The released request
        // still requires references, while its explicit subject relation may be empty.
        for input in &mut program.request_inputs {
            input.minimum_rows = 0;
        }
    }
    Ok(())
}
