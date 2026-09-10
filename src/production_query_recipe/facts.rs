//! Subject-bound facts and calls share one native request-owned entity-reference semi join.

use std::sync::Arc;

pub(crate) mod families;
pub(crate) mod relationships;

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
    subject_facts(epoch, false, false)
}

pub(super) fn calls(
    epoch: &ProgrammaticFabricEpoch,
) -> Result<Option<ProductionSemanticFormProgram>, ProductionQueryRecipeError> {
    subject_facts(epoch, true, false)
}

pub(super) fn source_context(
    epoch: &ProgrammaticFabricEpoch,
) -> Result<Option<ProductionSemanticFormProgram>, ProductionQueryRecipeError> {
    subject_facts(epoch, false, true)
}

// One compact plan keeps request identity matching, family selection and projection together.
#[allow(clippy::too_many_lines)]
fn subject_facts(
    epoch: &ProgrammaticFabricEpoch,
    calls: bool,
    source_context: bool,
) -> Result<Option<ProductionSemanticFormProgram>, ProductionQueryRecipeError> {
    let (source_id, output_id, input_id, input_prefix, form) = if source_context {
        (
            "fact.code_source_context",
            "query.result.source-context",
            "query.input.source-subjects",
            "for-inputs",
            ReleasedSemanticForm::RetrieveSourceAndSyntaxContext,
        )
    } else if calls {
        (
            if epoch
                .relation(&ProgrammaticRelationId::new(
                    "fact.code_relationship_selector",
                ))
                .is_some()
            {
                "fact.code_relationship_selector"
            } else {
                "fact.code_call_selector"
            },
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
    let mut projections = schema
        .fields()
        .iter()
        .zip(&fields)
        .filter(|(field, _)| {
            field.name() != "subject_kind" && (source_context || field.name() != "fact_family")
        })
        .map(|(field, id)| {
            Ok(ProgramProjectionField {
                output_name: None,
                output_nullable: None,
                public_entity_kind: None,
                input_field_id: id.clone(),
                output_field_id: output_field(field.name())?,
            })
        })
        .collect::<Result<Vec<_>, ProductionQueryRecipeError>>()?;
    let bytes_relation = if source_context {
        let Some(bytes) = source_bytes_definition(epoch)? else {
            return Ok(None);
        };
        projections.push(ProgramProjectionField {
            output_name: None,
            output_nullable: None,
            public_entity_kind: None,
            input_field_id: release_field_id("source.exact_source_bytes.source_bytes")?,
            output_field_id: output_field("source_bytes")?,
        });
        Some(bytes)
    } else {
        None
    };
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
    let meanings: &[(&str, &str, &[&str], &str)] = if source_context {
        &[
            (
                "selection.context",
                "context_kind",
                if matches!(
                    source
                        .contract
                        .relation_semantic_role(SchemaRole::Logical)
                        .map_err(|error| ProductionQueryRecipeError::InvalidCompiledRelease {
                            detail: error.to_string(),
                        })?,
                    Some("canonical.source-context.line-window" | super::SOURCE_OCCURRENCES_ROLE)
                ) {
                    &[
                        "exact source span",
                        "function definition",
                        "function body",
                        "surrounding lines",
                    ]
                } else if schema.field_with_name("source_mapping").is_ok() {
                    &["exact source span", "function definition", "function body"]
                } else {
                    &["exact source span"]
                },
                "",
            ),
            (
                "selection.text-handling",
                "text_handling",
                &["lossless UTF-8 else bytes"],
                "lossless UTF-8 else bytes",
            ),
        ]
    } else if calls {
        &[
            (
                "selection.relationship",
                "fact_family",
                if source_id == "fact.code_relationship_selector" {
                    &[
                        "calls",
                        "call relationships",
                        "lexical references",
                        "lexical-references",
                    ]
                } else {
                    &["calls", "call relationships"]
                },
                "",
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
                target: super::EpochBoundSelectionTarget::Predicate {
                    input_field_id: source_field(field)?,
                    scalar_operator: ScalarOperator::Equal,
                },
                fold: EpochBoundSelectionFold::Any,
                resolutions: values
                    .iter()
                    .map(|value| EpochBoundSelectionValueResolution {
                        request_value: SemanticClauseValue::Text(Arc::from(*value)),
                        execution_value: SemanticClauseValue::Text(Arc::from(
                            if *id == "selection.relationship" {
                                match *value {
                                    "call relationships" => "calls",
                                    "lexical references" => "lexical-references",
                                    value => value,
                                }
                            } else if canonical.is_empty() {
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
    let ordering: &[&str] = if calls && source_id == "fact.code_relationship_selector" {
        &[
            "public_entity_id",
            "context_id",
            "file_id",
            "start_byte",
            "public_occurrence_id",
            "provider_owner",
            "provider_block_index",
            "provider_instance_key",
        ]
    } else if calls {
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
    let mut program = ProductionSemanticFormProgram {
        form,
        program_binding_id: Arc::from(binding),
        output_role_id: Arc::from(
            if source_context {
                ResultRole::SourceContexts
            } else {
                ResultRole::Facts
            }
            .released_id(),
        ),
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
    };
    if let Some(bytes) = bytes_relation {
        let mut joined_fields = fields.clone();
        joined_fields.extend(bytes.fields.clone());
        for operator in &mut program.operators {
            if operator.ordinal >= 4 {
                operator.ordinal += 2;
            }
            if operator.node_id == node("project") {
                operator.input_node_ids = vec![node("source-pins")];
            }
        }
        program.operators.insert(
            4,
            operator(
                "source-bytes",
                4,
                &[],
                ProgramRelationalOperator::Input {
                    relation_id: bytes.relation_id.clone(),
                },
                bytes.fields.clone(),
            ),
        );
        let predicates = [
            "workspace_id",
            "source_generation",
            "file_id",
            "content_digest",
        ]
        .into_iter()
        .map(|name| {
            Ok(ProgramJoinPredicate {
                left_field_id: source_field(name)?,
                right_field_id: release_field_id(&format!("source.exact_source_bytes.{name}"))?,
                scalar_operator: ScalarOperator::Equal,
            })
        })
        .collect::<Result<_, ProductionQueryRecipeError>>()?;
        program.operators.insert(
            5,
            operator(
                "source-pins",
                5,
                &["families", "source-bytes"],
                ProgramRelationalOperator::Join {
                    kind: JoinKind::Inner,
                    predicates,
                },
                joined_fields,
            ),
        );
        program.relations.push(bytes);
    }
    Ok(Some(program))
}

fn source_bytes_definition(
    epoch: &ProgrammaticFabricEpoch,
) -> Result<Option<ProductionRelationDefinition>, ProductionQueryRecipeError> {
    let id = "source.exact_source_bytes";
    let Some(relation) = epoch.relation(&ProgrammaticRelationId::new(id)) else {
        return Ok(None);
    };
    let fields = (0..relation.contract.logical_schema().fields().len())
        .map(|index| {
            relation
                .contract
                .field_id_at(SchemaRole::Logical, index)
                .map_err(|error| ProductionQueryRecipeError::InvalidCompiledRelease {
                    detail: error.to_string(),
                })
                .and_then(release_field_id)
        })
        .collect::<Result<_, _>>()?;
    Ok(Some(ProductionRelationDefinition {
        relation_id: release_relation_id(id)?,
        fields,
        authority: ProductionRelationAuthority::Epoch,
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
            SemanticQueryClause::RetrieveSourceContext { for_inputs, .. } => for_inputs,
            _ => continue,
        };
        for reference in references {
            if let SemanticReference::PriorResult(prior) = reference
                && prior.select == ResultRole::Entities
            {
                continue;
            }
            let SemanticReference::Entity { entity_id } = reference else {
                return Err("subject-bound facts require canonical entity IDs or typed entity results; phrase and fact subject resolution is unavailable".to_owned());
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
