//! Source dependencies follow resolved subjects; raw IDs alone do not reveal their language.

use super::{BTreeSet, EntityQueryScope};
use crate::relational_semantic_query::{EpochBoundSelectionRow, SemanticClauseValue};
use crate::semantic_query_contract::{SemanticQueryClause, SemanticReference};

impl EntityQueryScope {
    pub(crate) fn source_predicate_for(
        &self,
        relation: &str,
    ) -> Result<crate::relational_program::ScalarExpression, String> {
        use crate::relational_program::{FieldId, ScalarExpression as E, ScalarOperator as O};
        use datafusion::common::ScalarValue;
        let family_field =
            FieldId::new(format!("{relation}.fact_family")).map_err(|e| e.to_string())?;
        let family = self
            .families
            .iter()
            .map(|family| E::Call {
                operator: O::Equal,
                arguments: vec![
                    E::Field(family_field.clone()),
                    E::Literal(ScalarValue::Utf8(Some((*family).to_owned()))),
                ],
            })
            .reduce(|a, b| E::Call {
                operator: O::Or,
                arguments: vec![a, b],
            })
            .ok_or("source dependency family selection is empty")?;
        Ok(E::Call {
            operator: O::And,
            arguments: vec![
                self.predicate_for(relation, "language", "context_id")?,
                family,
            ],
        })
    }

    pub(crate) fn for_source_subjects(
        self,
        clause: &SemanticQueryClause,
        selections: &[EpochBoundSelectionRow],
        has_occurrence_sources: bool,
    ) -> Result<Self, String> {
        let SemanticQueryClause::RetrieveSourceContext { for_inputs, .. } = clause else {
            return Err("source processing scope requires a source clause".into());
        };
        let resolved = |query: &str, selection: &str| {
            selections.iter().find_map(|row| {
                if row.query_id.as_ref() != query || row.selection_id.as_ref() != selection {
                    return None;
                }
                if let SemanticClauseValue::Text(value) = &row.value {
                    Some(value.as_ref())
                } else {
                    None
                }
            })
        };
        if matches!(
            resolved(clause.query_id(), "selection.context"),
            Some("function definition" | "function body")
        ) {
            return self.with_families(BTreeSet::from(["function-source-context"]));
        }
        let mut families = BTreeSet::new();
        let mut languages = BTreeSet::new();
        let mut all_languages_known = true;
        for subject in for_inputs {
            match subject {
                SemanticReference::Entity { entity_id } => {
                    all_languages_known = false;
                    let kind = entity_id
                        .split(':')
                        .nth(1)
                        .ok_or("invalid source entity ID")?;
                    match kind {
                        "call" => {
                            families.insert("call-targets");
                        }
                        "reference" => {
                            families.extend(["lexical-references", "semantic-references"]);
                        }
                        "import-occurrence" => {
                            families.insert("imports");
                        }
                        "module" => {
                            families.insert("modules");
                        }
                        _ => {
                            families.insert("function-declarations");
                        }
                    }
                }
                SemanticReference::PriorResult(prior) => {
                    let selector = resolved(&prior.results_of, "selection.looking-for")
                        .ok_or("source input has no resolved entity selector")?;
                    families.insert(
                        crate::production_query_recipe::canonical_occurrence_family(selector)
                            .unwrap_or(if selector == "python:module" {
                                "modules"
                            } else {
                                "function-declarations"
                            }),
                    );
                    if let Some((language, _)) = selector.split_once(':') {
                        languages.insert(language.to_owned());
                    } else {
                        all_languages_known = false;
                    }
                }
                SemanticReference::Phrase(value) => {
                    let named =
                        super::super::programmatic_ingress_port::code_literals::named_subject(
                            value,
                        )
                        .ok_or("source input has no supported literal entity meaning")?;
                    families.insert(if named.selector == "python:module" {
                        "modules"
                    } else {
                        "function-declarations"
                    });
                    if let Some((language, _)) = named.selector.split_once(':') {
                        languages.insert(language.to_owned());
                    } else {
                        all_languages_known = false;
                    }
                }
                _ => {
                    return Err(
                        "source processing needs canonical entities or typed entity results".into(),
                    );
                }
            }
        }
        if !has_occurrence_sources
            && families
                .iter()
                .any(|family| *family != "function-declarations")
        {
            return Err(
                "selected snapshot does not contain occurrence/module source mappings".into(),
            );
        }
        let mut scope = self.with_families(families)?;
        if all_languages_known {
            scope
                .languages
                .retain(|language| languages.contains(language));
        }
        Ok(scope)
    }
}

#[cfg(test)]
mod tests {
    use super::super::SourceBoundaries;
    use super::*;
    use crate::semantic_query_contract::{PriorResultReference, ResultRole};

    fn scope() -> EntityQueryScope {
        EntityQueryScope {
            boundaries: SourceBoundaries::default(),
            family: "function-declarations",
            families: BTreeSet::new(),
            languages: BTreeSet::from(["python".into(), "rust".into()]),
            contexts: BTreeSet::from([[1; 16]]),
            owners: None,
        }
    }
    fn clause(subjects: Vec<SemanticReference>) -> SemanticQueryClause {
        SemanticQueryClause::RetrieveSourceContext {
            query_id: "source".into(),
            label: None,
            for_inputs: subjects,
            context: vec!["a human phrase resolved elsewhere".into()],
            text_handling: None,
            where_conditions: vec![],
            return_spec: None,
        }
    }
    fn prior(query: &str) -> SemanticReference {
        SemanticReference::PriorResult(PriorResultReference {
            results_of: query.into(),
            select: ResultRole::Entities,
        })
    }
    fn selection(query: &str, id: &str, value: &str) -> EpochBoundSelectionRow {
        EpochBoundSelectionRow {
            query_id: query.into(),
            selection_id: id.into(),
            ordinal: 0,
            value: SemanticClauseValue::Text(value.into()),
        }
    }

    #[test]
    fn resolved_subject_families_and_languages_preserve_mixed_dependencies() {
        let selections = [
            selection("calls", "selection.looking-for", "python:call"),
            selection("imports", "selection.looking-for", "rust:import-occurrence"),
        ];
        let selected = scope()
            .for_source_subjects(
                &clause(vec![prior("calls"), prior("imports")]),
                &selections,
                true,
            )
            .unwrap();
        assert_eq!(
            selected.families,
            BTreeSet::from(["call-targets", "imports"])
        );
        assert_eq!(selected.languages, scope().languages);
        assert_eq!(selected.contexts, scope().contexts);
        let python = scope()
            .for_source_subjects(&clause(vec![prior("calls")]), &selections, true)
            .unwrap();
        assert_eq!(python.languages, BTreeSet::from(["python".into()]));
        assert!(
            scope()
                .for_source_subjects(&clause(vec![prior("calls")]), &selections, false)
                .is_err()
        );
        assert!(
            scope()
                .for_source_subjects(&clause(vec![prior("unresolved")]), &selections, true)
                .is_err()
        );
    }

    #[test]
    fn direct_reference_ids_keep_both_provider_families_and_unknown_language() {
        let reference = SemanticReference::Entity {
            entity_id: "entity:reference:opaque".into(),
        };
        let selected = scope()
            .for_source_subjects(&clause(vec![reference]), &[], true)
            .unwrap();
        assert_eq!(
            selected.families,
            BTreeSet::from(["lexical-references", "semantic-references"])
        );
        assert_eq!(selected.languages, scope().languages);
        let function = SemanticReference::Entity {
            entity_id: "entity:function:opaque".into(),
        };
        let selected = scope()
            .for_source_subjects(&clause(vec![function]), &[], false)
            .unwrap();
        assert_eq!(selected.families, BTreeSet::from(["function-declarations"]));
    }

    #[test]
    fn literal_source_subjects_preserve_declared_language_and_module_family() {
        let selected = scope()
            .for_source_subjects(
                &clause(vec![SemanticReference::Phrase(
                    "Python module `package`".into(),
                )]),
                &[],
                true,
            )
            .unwrap();
        assert_eq!(selected.languages, BTreeSet::from(["python".into()]));
        assert_eq!(selected.families, BTreeSet::from(["modules"]));
        let mixed = scope()
            .for_source_subjects(
                &clause(vec![
                    SemanticReference::Phrase("Python function `target`".into()),
                    SemanticReference::Phrase("Rust function `crate::target`".into()),
                ]),
                &[],
                true,
            )
            .unwrap();
        assert_eq!(mixed.languages, scope().languages);
        assert_eq!(mixed.families, BTreeSet::from(["function-declarations"]));
    }

    #[test]
    fn function_syntax_meaning_retains_its_separate_processing_family() {
        let selected = scope()
            .for_source_subjects(
                &clause(vec![prior("functions")]),
                &[selection("source", "selection.context", "function body")],
                false,
            )
            .unwrap();
        assert_eq!(
            selected.families,
            BTreeSet::from(["function-source-context"])
        );
    }
}
