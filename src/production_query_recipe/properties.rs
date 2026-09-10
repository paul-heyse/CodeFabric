//! Exact text predicates share the existing authorized filter nodes of the first four forms.

use super::{
    Arc, BTreeMap, EpochBoundSelectionFold, EpochBoundSelectionTarget,
    ProductionSelectionDefinition, ProductionSemanticFormProgram, ProgramRelationalOperator,
    RELEASE_SELECTION_MAXIMUM_VALUES, ReleasedSemanticForm, SemanticValueKind,
};

pub(super) fn install(
    programs: &mut BTreeMap<(ReleasedSemanticForm, Arc<str>), ProductionSemanticFormProgram>,
) {
    for program in programs.values_mut() {
        let Some(filter) = program.operators.iter().find(|node| {
            matches!(node.operator, ProgramRelationalOperator::Filter)
                && !node.node_id.ends_with(".named-filter")
                && !node.node_id.ends_with(".location-filter")
        }) else {
            continue;
        };
        let properties = filter
            .output_fields
            .iter()
            .filter_map(|field| {
                let property = match field.as_str().rsplit('.').next()? {
                    "name" | "entity-name" => "name",
                    "qualified_name" => "qualified name",
                    "entity_kind" | "entity-kind" => "kind",
                    "language" | "entity-language" => "language",
                    "provider" => "provider",
                    "raw_kind" => "raw kind",
                    "resolution" => "resolution",
                    "dispatch_kind" => "dispatch kind",
                    "caller_name" => "caller name",
                    "target_name" => "target name",
                    "source_name" => "source name",
                    _ => return None,
                };
                Some((Arc::from(property), field.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        if properties.is_empty() {
            continue;
        }
        program.selections.push(ProductionSelectionDefinition {
            selection_id: Arc::from("selection.where"),
            value_kind: SemanticValueKind::Text,
            minimum_values: 0,
            maximum_values: RELEASE_SELECTION_MAXIMUM_VALUES,
            operator_node_id: filter.node_id.clone(),
            target: EpochBoundSelectionTarget::TextProperties { fields: properties },
            fold: EpochBoundSelectionFold::All,
            resolutions: vec![],
        });
    }
}

/// Nullable qualified-name columns do not establish support for every language/subject.
/// Until the Python declaration producer supplies lexical qualification, reject that meaning
/// rather than turning its null fields into a complete empty answer.
pub(crate) fn validate_inputs(
    request: &crate::semantic_query_contract::SemanticQueryRequest,
    query_id: &str,
    selections: &[crate::relational_semantic_query::EpochBoundSelectionRow],
) -> Result<(), String> {
    use crate::relational_semantic_query::{
        SemanticClauseValue, property_predicate::TextPropertyPredicate,
    };
    use crate::semantic_query_contract::{SemanticQueryClause as C, SemanticReference as R};
    if !selections.iter().any(|row| row.query_id.as_ref() == query_id
        && row.selection_id.as_ref() == "selection.where"
        && matches!(&row.value, SemanticClauseValue::Text(value) if TextPropertyPredicate::parse(value).is_ok_and(|predicate| predicate.property == "qualified name"))) {
        return Ok(());
    }
    let selector = |query: &str| {
        selections.iter().find_map(|row| {
            if row.query_id.as_ref() == query
                && row.selection_id.as_ref() == "selection.looking-for"
                && let SemanticClauseValue::Text(value) = &row.value
            {
                Some(value.as_ref())
            } else {
                None
            }
        })
    };
    let rust_only = !request.languages.is_empty()
        && request
            .languages
            .iter()
            .all(|language| language.eq_ignore_ascii_case("rust"));
    let supported = |selected: &str, source: bool| {
        ((selected.starts_with("rust:") || (selected == "function" && rust_only))
            && super::canonical_occurrence_family(selected).is_none())
            || (selected == "python:module" && !source)
    };
    let clause = request
        .queries
        .iter()
        .find(|clause| clause.query_id() == query_id)
        .ok_or("query block is absent")?;
    let valid = match clause {
        C::FindEntities { .. } => selector(query_id).is_some_and(|value| supported(value, false)),
        C::RetrieveFacts { about, .. }
        | C::RetrieveSourceContext {
            for_inputs: about, ..
        } => {
            let source = matches!(clause, C::RetrieveSourceContext { .. });
            about.iter().all(|subject| match subject {
                R::PriorResult(prior) => {
                    selector(&prior.results_of).is_some_and(|value| supported(value, source))
                }
                R::Entity { entity_id } => {
                    rust_only
                        && matches!(
                            entity_id.split(':').nth(1),
                            Some(
                                "function"
                                    | "constant"
                                    | "static"
                                    | "constructor-function"
                                    | "constructor-constant"
                            )
                        )
                }
                R::Phrase(value) => {
                    crate::fabric::programmatic_ingress_port::code_literals::named_subject(value)
                        .is_some_and(|named| supported(&named.selector, source))
                }
                _ => false,
            })
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err("qualified names are unavailable for part of the selected subject scope; select supported Rust declarations or Python module metadata".into())
    }
}
