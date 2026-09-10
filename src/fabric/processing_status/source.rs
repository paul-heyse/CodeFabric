//! Source dependencies follow resolved subjects; raw IDs alone do not reveal their language.

use super::{BTreeSet, EntityQueryScope};
use crate::relational_semantic_query::{EpochBoundSelectionRow, SemanticClauseValue};
use crate::semantic_query_contract::{SemanticQueryClause, SemanticReference};

impl EntityQueryScope {
    /// Find/source scopes need the addressed files; incoming relationships and context-wide facts
    /// retain their broader dependency scope until their own families prove a narrower selection.
    pub(crate) fn narrow_location_files(
        &mut self,
        clause: &SemanticQueryClause,
    ) -> Result<(), String> {
        let subjects = match clause {
            SemanticQueryClause::FindEntities { within, .. } => within,
            SemanticQueryClause::RetrieveSourceContext { for_inputs, .. } => for_inputs,
            _ => return Ok(()),
        };
        if subjects.is_empty()
            || subjects
                .iter()
                .any(|subject| !matches!(subject, SemanticReference::SourceLocation { .. }))
        {
            return Ok(());
        }
        let mut selected = Vec::new();
        for subject in subjects {
            let SemanticReference::SourceLocation { source_location } = subject else {
                unreachable!("checked location subjects")
            };
            source_location.validate()?;
            if self
                .boundaries
                .selects(source_location.source_file.as_bytes())
            {
                selected.push(
                    serde_json::json!({"kind":"path","root":source_location.source_file})
                        .to_string(),
                );
            }
        }
        if selected.is_empty() {
            self.languages.clear();
        } else {
            self.boundaries = super::SourceBoundaries::authorize(&selected)?;
        }
        Ok(())
    }

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

    #[allow(
        clippy::too_many_lines,
        reason = "keep resolved subject kinds and retained coverage capability checks together"
    )]
    pub(crate) fn for_source_subjects(
        self,
        clause: &SemanticQueryClause,
        selections: &[EpochBoundSelectionRow],
        has_occurrence_sources: bool,
        has_syntax_sources: bool,
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
                SemanticReference::SourceLocation { source_location } => {
                    source_location.validate()?;
                    let meaning = source_location.meaning()?;
                    match meaning.kind {
                        Some("syntax-node") => {
                            families.insert("syntax-nodes");
                        }
                        Some("call") => {
                            families.insert("call-targets");
                        }
                        Some("reference") => {
                            families.extend(["lexical-references", "semantic-references"]);
                        }
                        Some("import-occurrence") => {
                            families.insert("imports");
                        }
                        Some("module") => {
                            families.insert("modules");
                        }
                        Some("function") => {
                            families.insert("function-declarations");
                        }
                        None => {
                            families.extend([
                                "function-declarations",
                                "syntax-nodes",
                                "call-targets",
                                "lexical-references",
                                "semantic-references",
                                "imports",
                                "modules",
                            ]);
                        }
                        Some(_) => {
                            return Err(
                                "source location kind has no source dependency family".into()
                            );
                        }
                    }
                    if let Some(language) = meaning.language {
                        languages.insert(language.to_owned());
                    } else if source_location.source_file.as_bytes().ends_with(b".rs") {
                        languages.insert("rust".into());
                    } else if source_location.source_file.as_bytes().ends_with(b".py")
                        || source_location.source_file.as_bytes().ends_with(b".pyi")
                    {
                        languages.insert("python".into());
                    } else {
                        all_languages_known = false;
                    }
                }
                SemanticReference::Entity { entity_id } => {
                    all_languages_known = false;
                    let kind = entity_id
                        .split(':')
                        .nth(1)
                        .ok_or("invalid source entity ID")?;
                    match kind {
                        "syntax-node" => {
                            families.insert("syntax-nodes");
                        }
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
                    families.insert(
                        crate::production_query_recipe::canonical_occurrence_family(
                            &named.selector,
                        )
                        .unwrap_or(if named.selector == "python:module" {
                            "modules"
                        } else {
                            "function-declarations"
                        }),
                    );
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
        if !has_syntax_sources && families.contains("syntax-nodes") {
            return Err("selected snapshot does not contain syntax source mappings".into());
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
        if resolved(clause.query_id(), "selection.context") == Some("syntax outline") {
            scope.families.insert("syntax-nodes");
            if scope.families.contains("function-declarations") {
                scope.families.insert("function-source-context");
            }
            if !scope.contexts.is_empty() {
                scope.contexts.insert(crate::identity::SOURCE_CONTEXT_ID);
            }
        }
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
            .for_source_subjects(&clause(vec![prior("calls")]), &selections, true, true)
            .unwrap();
        assert_eq!(python.languages, BTreeSet::from(["python".into()]));
        assert!(
            scope()
                .for_source_subjects(&clause(vec![prior("calls")]), &selections, false, false)
                .is_err()
        );
        assert!(
            scope()
                .for_source_subjects(&clause(vec![prior("unresolved")]), &selections, true, true)
                .is_err()
        );
    }

    #[test]
    fn direct_reference_ids_keep_both_provider_families_and_unknown_language() {
        let reference = SemanticReference::Entity {
            entity_id: "entity:reference:opaque".into(),
        };
        let selected = scope()
            .for_source_subjects(&clause(vec![reference]), &[], true, true)
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
            .for_source_subjects(&clause(vec![function]), &[], false, false)
            .unwrap();
        assert_eq!(selected.families, BTreeSet::from(["function-declarations"]));
    }

    #[test]
    fn literal_source_subjects_preserve_declared_language_and_module_family() {
        let syntax = clause(vec![SemanticReference::Phrase(
            "Python syntax node `.`".into(),
        )]);
        let selected = scope()
            .for_source_subjects(&syntax, &[], true, true)
            .unwrap();
        assert_eq!(selected.families, BTreeSet::from(["syntax-nodes"]));
        assert_eq!(selected.languages, BTreeSet::from(["python".into()]));
        assert!(
            scope()
                .for_source_subjects(&syntax, &[], true, false)
                .is_err()
        );
        let selected = scope()
            .for_source_subjects(
                &clause(vec![SemanticReference::Phrase(
                    "Python module `package`".into(),
                )]),
                &[],
                true,
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
                true,
            )
            .unwrap();
        assert_eq!(mixed.languages, scope().languages);
        assert_eq!(mixed.families, BTreeSet::from(["function-declarations"]));
    }

    #[test]
    fn source_locations_narrow_only_addressed_files_inside_authorized_boundaries() {
        let subject = |file| {
            SemanticReference::SourceLocation { source_location: serde_json::from_value(serde_json::json!({"source_file":file,"start_byte":0,"semantic_location":"Python syntax node"})).unwrap() }
        };
        let mut selected = scope();
        selected.boundaries =
            SourceBoundaries::authorize(&[r#"{"kind":"path","root":"src"}"#.into()]).unwrap();
        selected
            .narrow_location_files(&clause(vec![subject("src/a.py"), subject("outside.py")]))
            .unwrap();
        assert!(selected.boundaries.selects(b"src/a.py"));
        assert!(!selected.boundaries.selects(b"src/b.py"));
        assert!(!selected.boundaries.selects(b"outside.py"));
        selected
            .narrow_location_files(&clause(vec![subject("outside.py")]))
            .unwrap();
        assert!(selected.languages.is_empty());
    }

    #[test]
    fn function_outline_preserves_semantic_and_parser_dependencies() {
        let selected = scope()
            .for_source_subjects(
                &clause(vec![SemanticReference::Phrase(
                    "Python function `target`".into(),
                )]),
                &[selection("source", "selection.context", "syntax outline")],
                true,
                true,
            )
            .unwrap();
        assert_eq!(selected.languages, BTreeSet::from(["python".into()]));
        assert_eq!(
            selected.families,
            BTreeSet::from([
                "function-declarations",
                "function-source-context",
                "syntax-nodes"
            ])
        );
        let mut contexts = scope().contexts;
        contexts.insert(crate::identity::SOURCE_CONTEXT_ID);
        assert_eq!(selected.contexts, contexts);
    }

    #[test]
    fn function_syntax_meaning_retains_its_separate_processing_family() {
        let selected = scope()
            .for_source_subjects(
                &clause(vec![prior("functions")]),
                &[selection("source", "selection.context", "function body")],
                false,
                false,
            )
            .unwrap();
        assert_eq!(
            selected.families,
            BTreeSet::from(["function-source-context"])
        );
    }
}
