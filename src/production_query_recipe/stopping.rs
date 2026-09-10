//! A named traversal boundary retains the arriving witness but excludes further expansion.

use super::{
    Arc, EpochBoundSelectionFold, JoinKind, ProductionOperatorDefinition,
    ProductionSelectionDefinition, ProductionSemanticFormProgram, ProgramRelationalOperator,
    RELEASE_SELECTION_MAXIMUM_VALUES, ScalarOperator, SemanticValueKind,
};
use crate::relational_semantic_query::ProgramJoinPredicate;

pub(super) fn install(
    selection: &super::named::SubjectSelection,
    programs: &mut [ProductionSemanticFormProgram],
) {
    for program in programs {
        if program.output_relation_id.as_str() != "query.result.call-facts"
            && !program
                .output_relation_id
                .as_str()
                .starts_with("query.result.call-walk-")
        {
            continue;
        }
        install_program(selection, program);
    }
}

fn install_program(
    selection: &super::named::SubjectSelection,
    program: &mut ProductionSemanticFormProgram,
) {
    let node = |suffix: &str| Arc::<str>::from(format!("{}.{suffix}", program.program_binding_id));
    let Some(source_index) = program
        .operators
        .iter()
        .position(|op| op.node_id == node("source"))
    else {
        return;
    };
    let source = program.operators[source_index].clone();
    let mut predicates = Vec::new();
    for suffix in [".public_entity_id", ".context_id", ".workspace_id"] {
        let field = |fields: &[crate::relational_program::FieldId]| {
            fields
                .iter()
                .find(|field| field.as_str().ends_with(suffix))
                .cloned()
        };
        let (Some(left_field_id), Some(right_field_id)) = (
            field(&source.output_fields),
            field(&selection.source.fields),
        ) else {
            // Retained epochs need actual canonical identity/context contracts to admit stops.
            return;
        };
        predicates.push(ProgramJoinPredicate {
            left_field_id,
            right_field_id,
            scalar_operator: ScalarOperator::Equal,
        });
    }
    let eligible = node("stop-eligible");
    for operator in &mut program.operators {
        for input in &mut operator.input_node_ids {
            if *input == source.node_id {
                *input = eligible.clone();
            }
        }
    }
    let additions = [
        ProductionOperatorDefinition {
            node_id: node("stop-input"),
            ordinal: 0,
            input_node_ids: vec![],
            operator: ProgramRelationalOperator::Input {
                relation_id: selection.source.relation_id.clone(),
            },
            output_fields: selection.source.fields.clone(),
        },
        ProductionOperatorDefinition {
            node_id: node("stop-filter"),
            ordinal: 0,
            input_node_ids: vec![node("stop-input")],
            operator: ProgramRelationalOperator::Filter,
            output_fields: selection.source.fields.clone(),
        },
        ProductionOperatorDefinition {
            node_id: eligible,
            ordinal: 0,
            input_node_ids: vec![source.node_id, node("stop-filter")],
            operator: ProgramRelationalOperator::Join {
                kind: JoinKind::LeftAnti,
                predicates,
            },
            output_fields: source.output_fields,
        },
    ];
    let insertion = source_index + 1;
    program.operators.splice(insertion..insertion, additions);
    for (ordinal, op) in program.operators.iter_mut().enumerate() {
        op.ordinal = u32::try_from(ordinal).expect("bounded program operators");
    }
    if !program
        .relations
        .iter()
        .any(|relation| relation.relation_id == selection.source.relation_id)
    {
        program.relations.push(selection.source.clone());
    }
    program.selections.push(ProductionSelectionDefinition {
        selection_id: Arc::from("selection.stop-when"),
        value_kind: SemanticValueKind::Text,
        minimum_values: 0,
        maximum_values: RELEASE_SELECTION_MAXIMUM_VALUES,
        operator_node_id: node("stop-filter"),
        target: selection.target.clone(),
        fold: EpochBoundSelectionFold::Any,
        resolutions: vec![],
    });
}
