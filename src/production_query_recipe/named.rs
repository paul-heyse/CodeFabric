//! Literal subjects use the epoch's entity selector before joining the requested fact family.

use super::{
    Arc, BTreeMap, EpochBoundSelectionFold, EpochBoundSelectionTarget, EpochSemanticRelation,
    JoinKind, ProductionOperatorDefinition, ProductionRelationAuthority,
    ProductionRelationDefinition, ProductionSelectionDefinition, ProductionSemanticFormProgram,
    ProgramRelationalOperator, RELEASE_SELECTION_MAXIMUM_VALUES, ReleasedSemanticForm,
    ScalarOperator, SemanticValueKind, UnionKind,
};
use crate::relational_semantic_query::ProgramJoinPredicate;

pub(super) fn install(
    entities: &EpochSemanticRelation,
    programs: &mut [ProductionSemanticFormProgram],
) {
    let entity_field = |name| {
        entities
            .scope_fields
            .iter()
            .find(|(_, label)| *label == name)
            .map(|(field, _)| field.clone())
    };
    let (Some(public_id), Some(context_id)) = (
        entity_field("public-entity-id"),
        entity_field("analysis-context-id"),
    ) else {
        return;
    };
    let mut names = BTreeMap::from([
        (Arc::from("selector"), entities.selector.clone()),
        (Arc::from("name"), entities.entity_name.clone()),
    ]);
    if let Some(qualified) = entity_field("qualified-name") {
        names.insert(Arc::from("qualified name"), qualified);
    }
    let selection = SubjectSelection {
        source: ProductionRelationDefinition {
            relation_id: entities.relation_id.clone(),
            fields: entities.fields.clone(),
            authority: ProductionRelationAuthority::Epoch,
        },
        public_id,
        context_id,
        name: "named",
        selection_id: "selection.named-subject",
        target: EpochBoundSelectionTarget::NamedEntities { fields: names },
    };
    super::stopping::install(&selection, programs);
    install_selection(&selection, programs);
}

pub(super) struct SubjectSelection {
    pub source: ProductionRelationDefinition,
    pub public_id: crate::relational_program::FieldId,
    pub context_id: crate::relational_program::FieldId,
    pub name: &'static str,
    pub selection_id: &'static str,
    pub target: EpochBoundSelectionTarget,
}

#[allow(
    clippy::too_many_lines,
    reason = "subject alternatives share one native union and exact identity/context join contract"
)]
pub(super) fn install_selection(
    selection: &SubjectSelection,
    programs: &mut [ProductionSemanticFormProgram],
) {
    let entities = &selection.source;
    let public_id = &selection.public_id;
    let context_id = &selection.context_id;
    for program in programs {
        if !matches!(
            program.form,
            ReleasedSemanticForm::FindCodeEntities
                | ReleasedSemanticForm::RetrieveFactsAboutCode
                | ReleasedSemanticForm::FollowCodeRelationships
                | ReleasedSemanticForm::RetrieveSourceAndSyntaxContext
        ) {
            continue;
        }
        let node =
            |suffix: &str| Arc::<str>::from(format!("{}.{suffix}", program.program_binding_id));
        let subject_node = |suffix: &str| node(&format!("{}-{suffix}", selection.name));
        let Some(index) = program
            .operators
            .iter()
            .position(|op| op.node_id == node("subjects"))
        else {
            continue;
        };
        let subjects = program.operators[index].clone();
        let field = |suffix| {
            subjects
                .output_fields
                .iter()
                .find(|field| field.as_str().ends_with(suffix))
                .cloned()
        };
        let (Some(subject_id), Some(subject_context)) =
            (field(".public_entity_id"), field(".context_id"))
        else {
            continue;
        };
        let combined = node(&format!("all-{}-subjects", selection.name));
        for op in &mut program.operators {
            for input in &mut op.input_node_ids {
                if *input == subjects.node_id {
                    *input = combined.clone();
                }
            }
        }
        let mut additions = vec![
            ProductionOperatorDefinition {
                node_id: subject_node("input"),
                ordinal: 0,
                input_node_ids: vec![],
                operator: ProgramRelationalOperator::Input {
                    relation_id: entities.relation_id.clone(),
                },
                output_fields: entities.fields.clone(),
            },
            ProductionOperatorDefinition {
                node_id: subject_node("filter"),
                ordinal: 0,
                input_node_ids: vec![subject_node("input")],
                operator: ProgramRelationalOperator::Filter,
                output_fields: entities.fields.clone(),
            },
        ];
        let selected = if subjects.output_fields == entities.fields {
            subject_node("filter")
        } else {
            additions.push(ProductionOperatorDefinition {
                node_id: subject_node("subjects"),
                ordinal: 0,
                input_node_ids: vec![subjects.input_node_ids[0].clone(), subject_node("filter")],
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
            });
            subject_node("subjects")
        };
        additions.push(ProductionOperatorDefinition {
            node_id: combined,
            ordinal: 0,
            input_node_ids: vec![subjects.node_id, selected],
            operator: ProgramRelationalOperator::Union {
                kind: UnionKind::Distinct,
            },
            output_fields: subjects.output_fields,
        });
        let insertion = index + 1;
        program.operators.splice(insertion..insertion, additions);
        for (ordinal, op) in program.operators.iter_mut().enumerate() {
            op.ordinal = u32::try_from(ordinal).expect("bounded program operators");
        }
        if !program
            .relations
            .iter()
            .any(|relation| relation.relation_id == entities.relation_id)
        {
            program.relations.push(ProductionRelationDefinition {
                relation_id: entities.relation_id.clone(),
                fields: entities.fields.clone(),
                authority: ProductionRelationAuthority::Epoch,
            });
        }
        program.selections.push(ProductionSelectionDefinition {
            selection_id: Arc::from(selection.selection_id),
            value_kind: SemanticValueKind::Text,
            minimum_values: 0,
            maximum_values: RELEASE_SELECTION_MAXIMUM_VALUES,
            operator_node_id: subject_node("filter"),
            target: selection.target.clone(),
            fold: EpochBoundSelectionFold::Any,
            resolutions: vec![],
        });
        for input in &mut program.request_inputs {
            input.minimum_rows = 0;
        }
    }
}
