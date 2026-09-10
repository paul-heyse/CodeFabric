//! Resolve a family to its catalog-bound program before lowering that program's row predicates.

use super::{
    Arc, EpochBoundProgramBindingRow, EpochBoundSemanticIngressCatalog, IngressProjection,
    ProgrammaticQueryPortError, SemanticAuthorizedChoice, SemanticClauseValue,
    SemanticInputConstraints, SemanticInputKind, SemanticInputRequirement, SemanticInputValue,
    guarded_selection_choice_id, guarded_selection_field_id, hex_bytes, rejected,
    selection_presentation, semantic_input_value,
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
        return Err(rejected(
            "requested fact meanings require different typed family programs; use separate requests",
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
