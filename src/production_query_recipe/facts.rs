//! Subject-bound facts and calls share one native request-owned entity-reference semi join.

use std::sync::Arc;

use super::{
    EpochBoundRequestInputField, EpochBoundSelectionFold, EpochBoundSelectionValueResolution,
    FieldId, JoinKind, ProductionOperatorDefinition, ProductionQueryRecipeError,
    ProductionRelationAuthority, ProductionRelationDefinition, ProductionRequestInputDefinition,
    ProductionSelectionDefinition, ProductionSemanticFormProgram, ProgramProjectionField,
    ProgramRelationalOperator, ProgrammaticFabricEpoch, ProgrammaticRelationId,
    RELEASE_SELECTION_MAXIMUM_VALUES, ReleasedSemanticForm, ResultRole, ScalarOperator, SchemaRole,
    SemanticClauseValue, SemanticValueKind, release_field_id, release_relation_id,
    released_program_binding_id,
};
use crate::relational_semantic_query::{ProgramJoinPredicate, ProgramSortField};
use crate::semantic_query_contract::{
    SemanticQueryClause, SemanticQueryRequest, SemanticReference,
};

pub(super) fn declarations(
    epoch: &ProgrammaticFabricEpoch,
) -> Result<Option<ProductionSemanticFormProgram>, ProductionQueryRecipeError> {
    subject_facts(epoch, false)
}

pub(super) fn calls(
    epoch: &ProgrammaticFabricEpoch,
) -> Result<Option<ProductionSemanticFormProgram>, ProductionQueryRecipeError> {
    subject_facts(epoch, true)
}

// One compact plan keeps request identity matching, family selection and projection together.
#[allow(clippy::too_many_lines)]
fn subject_facts(
    epoch: &ProgrammaticFabricEpoch,
    calls: bool,
) -> Result<Option<ProductionSemanticFormProgram>, ProductionQueryRecipeError> {
    let (source_id, output_id, input_id, input_prefix, form) = if calls {
        (
            "fact.code_call_selector",
            "query.result.call-facts",
            "query.input.call-subjects",
            "starting-from",
            ReleasedSemanticForm::FollowCodeRelationships,
        )
    } else {
        (
            "fact.code_declaration",
            "query.result.declaration-facts",
            "query.input.declaration-about",
            "about",
            ReleasedSemanticForm::RetrieveFactsAboutCode,
        )
    };
    let Some(source) = epoch.relation(&ProgrammaticRelationId::new(source_id)) else {
        return Ok(None);
    };
    let schema = source.contract.logical_schema();
    if schema.field_with_name("public_entity_id").is_err() {
        return Ok(None);
    }
    let fields = (0..schema.fields().len())
        .map(|index| {
            source
                .contract
                .field_id_at(SchemaRole::Logical, index)
                .map_err(|error| ProductionQueryRecipeError::InvalidCompiledRelease {
                    detail: error.to_string(),
                })
                .and_then(release_field_id)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let source_field = |name: &str| -> Result<FieldId, ProductionQueryRecipeError> {
        let index = schema.index_of(name).map_err(|error| {
            ProductionQueryRecipeError::InvalidCompiledRelease {
                detail: error.to_string(),
            }
        })?;
        Ok(fields[index].clone())
    };
    let output_field = |name: &str| release_field_id(&format!("{output_id}.{name}"));
    let projections = schema
        .fields()
        .iter()
        .zip(&fields)
        .filter(|(field, _)| !matches!(field.name().as_str(), "subject_kind" | "fact_family"))
        .map(|(field, id)| {
            Ok(ProgramProjectionField {
                input_field_id: id.clone(),
                output_field_id: output_field(field.name())?,
            })
        })
        .collect::<Result<Vec<_>, ProductionQueryRecipeError>>()?;
    let output_fields = projections
        .iter()
        .map(|field| field.output_field_id.clone())
        .collect::<Vec<_>>();
    let request_fields = ["kind", "value", "producer-role"]
        .into_iter()
        .map(|suffix| release_field_id(&format!("{input_prefix}.{suffix}")))
        .collect::<Result<Vec<_>, _>>()?;
    let request_relation = release_relation_id(input_id)?;
    let source_relation = release_relation_id(source_id)?;
    let output_relation = release_relation_id(output_id)?;
    let binding = released_program_binding_id(form);
    let node = |name: &str| Arc::<str>::from(format!("{binding}.{name}"));
    let operator = |name: &str, ordinal, input_names: &[&str], operator, output_fields| {
        ProductionOperatorDefinition {
            node_id: node(name),
            ordinal,
            input_node_ids: input_names.iter().map(|name| node(name)).collect(),
            operator,
            output_fields,
        }
    };
    let meanings: &[(&str, &str, &[&str], &str)] = if calls {
        &[
            (
                "selection.relationship",
                "fact_family",
                &["calls", "call relationships"],
                "calls",
            ),
            (
                "selection.direction",
                "direction",
                &["outgoing", "incoming"],
                "",
            ),
            (
                "selection.distance",
                "distance",
                &["one relationship step", "one step"],
                "one relationship step",
            ),
        ]
    } else {
        &[(
            "selection.facts",
            "fact_family",
            &["declarations", "declaration locations and provenance"],
            "declarations",
        )]
    };
    let selections = meanings
        .iter()
        .map(|(id, field, values, canonical)| {
            Ok(ProductionSelectionDefinition {
                selection_id: Arc::from(*id),
                value_kind: SemanticValueKind::Text,
                minimum_values: 1,
                maximum_values: if calls {
                    1
                } else {
                    RELEASE_SELECTION_MAXIMUM_VALUES
                },
                operator_node_id: node("families"),
                input_field_id: source_field(field)?,
                scalar_operator: ScalarOperator::Equal,
                fold: EpochBoundSelectionFold::Any,
                resolutions: values
                    .iter()
                    .map(|value| EpochBoundSelectionValueResolution {
                        request_value: SemanticClauseValue::Text(Arc::from(*value)),
                        execution_value: SemanticClauseValue::Text(Arc::from(
                            if canonical.is_empty() {
                                *value
                            } else {
                                *canonical
                            },
                        )),
                    })
                    .collect(),
            })
        })
        .collect::<Result<_, ProductionQueryRecipeError>>()?;
    let ordering: &[&str] = if calls {
        &[
            "public_entity_id",
            "context_id",
            "file_id",
            "start_byte",
            "call_site_id",
            "provider_owner",
            "provider_block_index",
            "provider_instance_key",
        ]
    } else {
        &[
            "public_entity_id",
            "file_id",
            "start_byte",
            "declaration_id",
        ]
    };
    Ok(Some(ProductionSemanticFormProgram {
        form,
        program_binding_id: Arc::from(binding),
        output_role_id: Arc::from(ResultRole::Facts.released_id()),
        root_node_id: node("limit"),
        output_relation_id: output_relation.clone(),
        output_fields: output_fields.clone(),
        relations: vec![
            ProductionRelationDefinition {
                relation_id: source_relation.clone(),
                fields: fields.clone(),
                authority: ProductionRelationAuthority::Epoch,
            },
            ProductionRelationDefinition {
                relation_id: request_relation.clone(),
                fields: request_fields.clone(),
                authority: ProductionRelationAuthority::QueryLocal,
            },
            ProductionRelationDefinition {
                relation_id: output_relation,
                fields: output_fields.clone(),
                authority: ProductionRelationAuthority::ProgramResult,
            },
        ],
        operators: vec![
            operator(
                "source",
                0,
                &[],
                ProgramRelationalOperator::Input {
                    relation_id: source_relation,
                },
                fields.clone(),
            ),
            operator(
                "about",
                1,
                &[],
                ProgramRelationalOperator::Input {
                    relation_id: request_relation.clone(),
                },
                request_fields.clone(),
            ),
            operator(
                "subjects",
                2,
                &["source", "about"],
                ProgramRelationalOperator::Join {
                    kind: JoinKind::LeftSemi,
                    predicates: vec![
                        ProgramJoinPredicate {
                            left_field_id: source_field("subject_kind")?,
                            right_field_id: request_fields[0].clone(),
                            scalar_operator: ScalarOperator::Equal,
                        },
                        ProgramJoinPredicate {
                            left_field_id: source_field("public_entity_id")?,
                            right_field_id: request_fields[1].clone(),
                            scalar_operator: ScalarOperator::Equal,
                        },
                    ],
                },
                fields.clone(),
            ),
            operator(
                "families",
                3,
                &["subjects"],
                ProgramRelationalOperator::Filter,
                fields.clone(),
            ),
            operator(
                "project",
                4,
                &["families"],
                ProgramRelationalOperator::Projection {
                    fields: projections,
                },
                output_fields.clone(),
            ),
            operator(
                "sort",
                5,
                &["project"],
                ProgramRelationalOperator::Sort {
                    fields: ordering
                        .iter()
                        .map(|name| {
                            Ok(ProgramSortField {
                                input_field_id: output_field(name)?,
                                ascending: true,
                                nulls_first: false,
                            })
                        })
                        .collect::<Result<_, ProductionQueryRecipeError>>()?,
                },
                output_fields.clone(),
            ),
            operator(
                "limit",
                6,
                &["sort"],
                ProgramRelationalOperator::Limit { skip: 0 },
                output_fields,
            ),
        ],
        selections,
        request_inputs: vec![ProductionRequestInputDefinition {
            input_id: Arc::from(format!("input.{input_prefix}")),
            relation_id: request_relation,
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
        returns: Vec::new(),
        consumer_slots: Vec::new(),
        required_fact_families: Vec::new(),
    }))
}

/// Until semantic phrase/prior-result resolution is installed, reject those meanings explicitly.
/// The typed request join also checks the kind slug, so a different entity kind never aliases.
pub(crate) fn validate_canonical_fact_references(
    request: &SemanticQueryRequest,
) -> Result<(), String> {
    for clause in &request.queries {
        let references = match clause {
            SemanticQueryClause::RetrieveFacts { about, .. } => about,
            SemanticQueryClause::FollowRelationships { starting_from, .. } => starting_from,
            _ => continue,
        };
        for reference in references {
            let SemanticReference::Entity { entity_id } = reference else {
                return Err("subject-bound facts currently require explicit canonical entity IDs; phrase, fact and prior-result resolution is unavailable".to_owned());
            };
            let slug = entity_id
                .split(':')
                .nth(1)
                .ok_or("invalid canonical entity ID")?;
            crate::identity::decode_public_id(
                crate::identity::IdentityDomain::Entity,
                Some(slug),
                entity_id,
            )
            .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}
