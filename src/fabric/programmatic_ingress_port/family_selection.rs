//! Resolve a family to its catalog-bound program before lowering that program's row predicates.

use super::{
    Arc, EpochBoundProgramBindingRow, EpochBoundSemanticIngressCatalog, IngressProjection,
    ProgrammaticQueryPortError, SemanticAuthorizedChoice, SemanticClauseValue,
    SemanticInputConstraints, SemanticInputKind, SemanticInputRequirement, SemanticInputValue,
    guarded_selection_choice_id, guarded_selection_field_id, hex_bytes, rejected,
    selection_presentation, semantic_input_value, unavailable,
};

// Keep candidate resolution, guarded choice validation and replay binding in one audited path.
#[allow(clippy::too_many_lines)]
pub(super) fn select<'a>(
    catalog: &'a EpochBoundSemanticIngressCatalog,
    candidates: &[&'a EpochBoundProgramBindingRow],
    query: &str,
    facts: &[String],
    projection: &mut IngressProjection,
) -> Result<Option<&'a EpochBoundProgramBindingRow>, ProgrammaticQueryPortError> {
    let supports = |program: &&EpochBoundProgramBindingRow| {
        facts.iter().all(|fact| {
            catalog.selections.iter().any(|binding| {
                binding.program_binding_id == program.program_binding_id
                    && binding.selection_id.as_ref() == "selection.facts"
                    && binding.resolutions.iter().any(|resolution| {
                        resolution.request_value
                            == SemanticClauseValue::Text(Arc::from(fact.as_str()))
                    })
            })
        })
    };
    let matches = candidates
        .iter()
        .copied()
        .filter(supports)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [program] => return Ok(Some(program)),
        [] => {}
        _ => {
            return Err(rejected(
                "admitted catalog has ambiguous programs for the requested fact meanings",
            ));
        }
    }
    let [original] = facts else {
        return Err(unavailable(
            "facts",
            "requested fact meanings require different typed family programs; use separate blocks",
        ));
    };
    let selection: Arc<str> = Arc::from("selection.facts");
    let catalog_binding = format!("facts.catalog.{}", hex_bytes(&catalog.program_catalog_pin));
    let field = guarded_selection_field_id(query, &catalog_binding, &selection, 0);
    let choices = candidates
        .iter()
        .flat_map(|program| {
            catalog
                .selections
                .iter()
                .filter(move |binding| {
                    binding.program_binding_id == program.program_binding_id
                        && binding.selection_id.as_ref() == "selection.facts"
                })
                .flat_map(move |binding| {
                    binding
                        .resolutions
                        .iter()
                        .map(move |resolution| (*program, resolution))
                })
        })
        .map(|(program, resolution)| {
            let choice_id = guarded_selection_choice_id(
                &program.program_binding_id,
                "selection.facts",
                &resolution.request_value,
            );
            Ok((
                program,
                resolution,
                SemanticAuthorizedChoice {
                    presentation_key: selection_presentation(
                        &resolution.request_value,
                        choice_id.clone(),
                    ),
                    choice_id,
                    value: semantic_input_value(&resolution.request_value)?,
                },
            ))
        })
        .collect::<Result<Vec<_>, ProgrammaticQueryPortError>>()?;
    if choices.is_empty() {
        return Err(rejected("fact programs have no admitted family meanings"));
    }
    match projection.answers.get(&field) {
        Some(SemanticInputValue::Choice(id)) => {
            let (program, resolution, _) = choices
                .iter()
                .find(|(_, _, choice)| choice.choice_id == *id)
                .ok_or_else(|| {
                    rejected("guarded fact family choice is outside the installed catalog")
                })?;
            projection.consumed_answers.insert(field);
            // The injected resolution is only for this exact guarded request. The emitted value
            // still must pass the selected program's immutable ingress catalog validation.
            projection.selection_resolutions.insert(
                (
                    Arc::clone(&program.program_binding_id),
                    selection,
                    SemanticClauseValue::Text(Arc::from(original.as_str())),
                ),
                resolution.execution_value.clone(),
            );
            Ok(Some(program))
        }
        Some(_) => Err(rejected(
            "guarded fact family input is not an authorized enum choice",
        )),
        None => {
            projection.requirements.push(SemanticInputRequirement {
                semantic_field_id: field,
                input_kind: SemanticInputKind::Enum,
                presentation_key: "input.selection-resolution".to_owned(),
                description_key: Some("input.selection-resolution.description".to_owned()),
                required: true,
                constraints: Some(SemanticInputConstraints::Enum {
                    minimum_selections: 1,
                    maximum_selections: 1,
                }),
                authorized_choices: choices.into_iter().map(|(_, _, choice)| choice).collect(),
            });
            Ok(None)
        }
    }
}

// Resolve dimensions in order: a guarded family answer must preserve every admitted direction.
#[allow(clippy::too_many_lines)]
pub(super) fn select_relationship<'a>(
    catalog: &'a EpochBoundSemanticIngressCatalog,
    candidates: &[&'a EpochBoundProgramBindingRow],
    query: &str,
    dimensions: &[(&str, &str)],
    projection: &mut IngressProjection,
) -> Result<Option<&'a EpochBoundProgramBindingRow>, ProgrammaticQueryPortError> {
    let mut remaining = candidates.to_vec();
    for (selection, original) in dimensions {
        let value = SemanticClauseValue::Text(Arc::from(*original));
        let bindings = catalog
            .selections
            .iter()
            .filter(|binding| {
                binding.selection_id.as_ref() == *selection
                    && remaining
                        .iter()
                        .any(|program| program.program_binding_id == binding.program_binding_id)
            })
            .collect::<Vec<_>>();
        let matches = |value: &SemanticClauseValue| {
            bindings
                .iter()
                .filter(|binding| {
                    binding
                        .resolutions
                        .iter()
                        .any(|resolution| &resolution.request_value == value)
                })
                .map(|binding| binding.program_binding_id.clone())
                .collect::<std::collections::BTreeSet<_>>()
        };
        let mut selected = matches(&value);
        if selected.is_empty() {
            let catalog_binding = format!(
                "relationships.catalog.{}",
                hex_bytes(&catalog.program_catalog_pin)
            );
            let field = guarded_selection_field_id(query, &catalog_binding, selection, 0);
            let mut choices = std::collections::BTreeMap::new();
            for binding in &bindings {
                for resolution in &binding.resolutions {
                    let choice_id = guarded_selection_choice_id(
                        &catalog_binding,
                        selection,
                        &resolution.execution_value,
                    );
                    choices
                        .entry(choice_id.clone())
                        .or_insert(SemanticAuthorizedChoice {
                            presentation_key: selection_presentation(
                                &resolution.execution_value,
                                choice_id.clone(),
                            ),
                            choice_id,
                            value: semantic_input_value(&resolution.execution_value)?,
                        });
                }
            }
            if choices.is_empty() {
                return Err(rejected(
                    "relationship programs have no admitted selection meanings",
                ));
            }
            match projection.answers.get(&field) {
                Some(SemanticInputValue::Choice(id)) => {
                    let choice = choices.get(id).ok_or_else(|| {
                        rejected("guarded relationship choice is outside the installed catalog")
                    })?;
                    let chosen = bindings
                        .iter()
                        .flat_map(|binding| &binding.resolutions)
                        .find(|resolution| {
                            guarded_selection_choice_id(
                                &catalog_binding,
                                selection,
                                &resolution.execution_value,
                            ) == choice.choice_id
                        })
                        .expect("choice came from a catalog resolution");
                    selected = bindings
                        .iter()
                        .filter(|binding| {
                            binding.resolutions.iter().any(|resolution| {
                                resolution.execution_value == chosen.execution_value
                            })
                        })
                        .map(|binding| binding.program_binding_id.clone())
                        .collect();
                    for binding in &bindings {
                        if let Some(resolution) = binding
                            .resolutions
                            .iter()
                            .find(|resolution| resolution.execution_value == chosen.execution_value)
                        {
                            projection.selection_resolutions.insert(
                                (
                                    binding.program_binding_id.clone(),
                                    Arc::from(*selection),
                                    value.clone(),
                                ),
                                resolution.execution_value.clone(),
                            );
                        }
                    }
                    projection.consumed_answers.insert(field);
                }
                Some(_) => {
                    return Err(rejected(
                        "guarded relationship input is not an authorized enum choice",
                    ));
                }
                None => {
                    projection.requirements.push(SemanticInputRequirement {
                        semantic_field_id: field,
                        input_kind: SemanticInputKind::Enum,
                        presentation_key: "input.selection-resolution".into(),
                        description_key: Some("input.selection-resolution.description".into()),
                        required: true,
                        constraints: Some(SemanticInputConstraints::Enum {
                            minimum_selections: 1,
                            maximum_selections: 1,
                        }),
                        authorized_choices: choices.into_values().collect(),
                    });
                    return Ok(None);
                }
            }
        }
        remaining.retain(|program| selected.contains(&program.program_binding_id));
    }
    match remaining.as_slice() {
        [program] => Ok(Some(program)),
        _ => Err(rejected(
            "admitted catalog has ambiguous programs for the requested relationship meanings",
        )),
    }
}

/// Distinguish an ordinary census from an explicitly supported semantic association scope.
pub(super) fn select_entity_scope<'a>(
    catalog: &'a EpochBoundSemanticIngressCatalog,
    candidates: &[&'a EpochBoundProgramBindingRow],
    within: &[super::SemanticReference],
    looking_for: &str,
) -> Result<Option<&'a EpochBoundProgramBindingRow>, ProgrammaticQueryPortError> {
    use super::{SemanticReference, code_literals};
    if candidates.iter().any(|program| {
        !catalog.selections.iter().any(|selection| {
            selection.program_binding_id == program.program_binding_id
                && selection.selection_id.as_ref() == "selection.looking-for"
        })
    }) {
        return Err(rejected(
            "admitted catalog has ambiguous programs without entity selection contracts",
        ));
    }
    let semantic_scope = within
        .iter()
        .any(|reference| !matches!(reference, SemanticReference::SourceLocation { .. }));
    if semantic_scope
        && within
            .iter()
            .any(|reference| matches!(reference, SemanticReference::SourceLocation { .. }))
    {
        return Err(unavailable(
            "slot.within",
            "mixed source-location and semantic target scopes are unavailable",
        ));
    }
    let meaning = code_literals::entity_selection(looking_for)
        .map_or_else(|| looking_for.to_owned(), |(meaning, _)| meaning);
    let meaning = SemanticClauseValue::Text(Arc::from(meaning));
    let selected = candidates
        .iter()
        .copied()
        .filter(|program| {
            let scoped = catalog.consumer_slots.iter().any(|slot| {
                slot.program_binding_id == program.program_binding_id
                    && slot.consumer_slot_id.as_ref() == "slot.within"
            });
            scoped == semantic_scope
                && (!scoped
                    || catalog.selections.iter().any(|selection| {
                        selection.program_binding_id == program.program_binding_id
                            && selection.selection_id.as_ref() == "selection.looking-for"
                            && selection
                                .resolutions
                                .iter()
                                .any(|resolution| resolution.request_value == meaning)
                    }))
        })
        .collect::<Vec<_>>();
    match selected.as_slice() {
        [program] => Ok(Some(program)),
        [] => Err(unavailable(
            "slot.within",
            "the requested entity meaning has no admitted semantic target scope",
        )),
        _ => Err(rejected(
            "admitted catalog has ambiguous entity scope programs",
        )),
    }
}
