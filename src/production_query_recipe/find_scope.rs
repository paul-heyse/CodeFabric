//! Semantic `within` scopes select reference occurrences by their canonical target, not by range.

use super::{
    Arc, EpochBoundRequestInputField, EpochSemanticRelation, FieldId, JoinKind,
    ProductionOperatorDefinition, ProductionQueryRecipeError, ProductionRelationAuthority,
    ProductionRelationDefinition, ProductionRequestInputDefinition, ProductionSemanticFormProgram,
    ProgramRelationalOperator, ProgrammaticFabricEpoch, RELEASE_SELECTION_MAXIMUM_VALUES,
    ScalarOperator, SemanticClauseValue, SemanticValueKind, canonical_occurrence_family,
    compiled_find_entities_program, facts, release_field_id, release_relation_id,
};
use crate::relational_semantic_query::ProgramJoinPredicate;

// Keep the selected entity schema, subject binding and native semi-join chain together.
#[allow(clippy::too_many_lines)]
pub(super) fn references(
    epoch: &ProgrammaticFabricEpoch,
    source: &EpochSemanticRelation,
) -> Result<Option<ProductionSemanticFormProgram>, ProductionQueryRecipeError> {
    const REFERENCES: &str = "fact.code_semantic_reference";
    let Some(references) = facts::families::definition(epoch, REFERENCES)? else {
        return Ok(None);
    };
    if !source.occurrences {
        return Ok(None);
    }
    let scope_field = |name| {
        source
            .scope_fields
            .iter()
            .find(|(_, label)| *label == name)
            .map(|(field, _)| field.clone())
            .ok_or_else(|| ProductionQueryRecipeError::InvalidCompiledRelease {
                detail: format!("canonical reference selection requires {name}"),
            })
    };
    let context = scope_field("analysis-context-id")?;
    let workspace = source
        .fields
        .iter()
        .find(|field| field.as_str().ends_with(".workspace_id"))
        .cloned()
        .ok_or_else(|| ProductionQueryRecipeError::InvalidCompiledRelease {
            detail: "canonical reference selection requires workspace identity".into(),
        })?;
    let public = scope_field("public-entity-id")?;
    let mut program = compiled_find_entities_program(source.clone())?;
    let previous = program.program_binding_id.clone();
    let binding = "program.semantic-query.find-reference-targets.v1";
    let node = |name: &str| Arc::<str>::from(format!("{binding}.{name}"));
    program.program_binding_id = Arc::from(binding);
    program.root_node_id = node("limit");
    for operator in &mut program.operators {
        operator.node_id = Arc::from(operator.node_id.replace(previous.as_ref(), binding));
        for input in &mut operator.input_node_ids {
            *input = Arc::from(input.replace(previous.as_ref(), binding));
        }
        if operator.node_id == node("filter") {
            operator.input_node_ids = vec![node("scoped-entities")];
        }
    }
    for selection in &mut program.selections {
        selection.operator_node_id = node("filter");
        selection.resolutions.retain(|resolution| {
            matches!(&resolution.execution_value, SemanticClauseValue::Text(value)
                if canonical_occurrence_family(value) == Some("semantic-references"))
        });
    }
    let output = release_relation_id("query.result.reference-entities")?;
    for relation in &mut program.relations {
        if relation.authority == ProductionRelationAuthority::ProgramResult {
            relation.relation_id = output.clone();
        }
    }
    program.output_relation_id = output;
    let request = release_relation_id("query.input.reference-targets")?;
    let request_fields = ["within.kind", "within.value", "within.producer-role"]
        .into_iter()
        .map(release_field_id)
        .collect::<Result<Vec<_>, _>>()?;
    let reference_field = |name| release_field_id(&format!("{REFERENCES}.{name}"));
    let semi = |pairs: Vec<(FieldId, FieldId)>| ProgramRelationalOperator::Join {
        kind: JoinKind::LeftSemi,
        predicates: pairs
            .into_iter()
            .map(|(left_field_id, right_field_id)| ProgramJoinPredicate {
                left_field_id,
                right_field_id,
                scalar_operator: ScalarOperator::Equal,
            })
            .collect(),
    };
    let mut additions = Vec::new();
    let mut add = |name: &str, inputs: &[&str], operator, output_fields| {
        additions.push(ProductionOperatorDefinition {
            node_id: node(name),
            ordinal: 0,
            input_node_ids: inputs.iter().map(|name| node(name)).collect(),
            operator,
            output_fields,
        });
    };
    add(
        "request",
        &[],
        ProgramRelationalOperator::Input {
            relation_id: request.clone(),
        },
        request_fields.clone(),
    );
    add(
        "subjects",
        &["input", "request"],
        semi(vec![(public, request_fields[1].clone())]),
        source.fields.clone(),
    );
    add(
        "references",
        &[],
        ProgramRelationalOperator::Input {
            relation_id: references.relation_id.clone(),
        },
        references.fields.clone(),
    );
    add(
        "target-references",
        &["references", "subjects"],
        semi(vec![
            (
                reference_field("target_entity_id")?,
                source.entity_id.clone(),
            ),
            (reference_field("context_id")?, context.clone()),
            (reference_field("workspace_id")?, workspace.clone()),
        ]),
        references.fields.clone(),
    );
    add(
        "scoped-entities",
        &["input", "target-references"],
        semi(vec![
            (source.entity_id.clone(), reference_field("reference_id")?),
            (context, reference_field("context_id")?),
            (workspace, reference_field("workspace_id")?),
        ]),
        source.fields.clone(),
    );
    program.operators.splice(1..1, additions);
    for (ordinal, operator) in program.operators.iter_mut().enumerate() {
        operator.ordinal = u32::try_from(ordinal).expect("finite reference scope template");
    }
    program.relations.extend([
        references,
        ProductionRelationDefinition {
            relation_id: request.clone(),
            fields: request_fields.clone(),
            authority: ProductionRelationAuthority::QueryLocal,
        },
    ]);
    program
        .request_inputs
        .push(ProductionRequestInputDefinition {
            input_id: Arc::from("input.within"),
            relation_id: request,
            fields: request_fields
                .into_iter()
                .enumerate()
                .map(|(index, field_id)| EpochBoundRequestInputField {
                    field_id,
                    value_kind: SemanticValueKind::Text,
                    required: index < 2,
                })
                .collect(),
            minimum_rows: 0,
            maximum_rows: RELEASE_SELECTION_MAXIMUM_VALUES,
        });
    Ok(Some(program))
}
