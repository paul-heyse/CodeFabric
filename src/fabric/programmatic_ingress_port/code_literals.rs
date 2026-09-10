//! Quoted identifiers are literal operands, separate from controlled entity-kind phrases.

use crate::relational_semantic_query::property_predicate::{TextComparison, TextPropertyPredicate};

pub(crate) fn named_subject(
    value: &str,
) -> Option<crate::relational_semantic_query::property_predicate::NamedEntityPredicate> {
    let (meaning, predicate) = parsed_entity_selection(value)?;
    let selector = crate::production_query_recipe::CANONICAL_ENTITY_SELECTORS
        .iter()
        .find(|(phrase, _)| *phrase == meaning)?
        .1;
    Some(
        crate::relational_semantic_query::property_predicate::NamedEntityPredicate {
            selector: selector.into(),
            predicate,
        },
    )
}

pub(super) fn entity_selection(value: &str) -> Option<(String, String)> {
    let (meaning, predicate) = parsed_entity_selection(value)?;
    Some((
        meaning.into(),
        serde_json::to_string(&predicate).expect("literal predicate serializes"),
    ))
}

fn parsed_entity_selection(value: &str) -> Option<(&'static str, TextPropertyPredicate)> {
    let (phrase, quoted) = value.split_once('`')?;
    let name = quoted.strip_suffix('`')?;
    if name.is_empty() || name.contains('`') || name.chars().any(char::is_control) {
        return None;
    }
    let phrase = phrase.trim().strip_prefix("the ").unwrap_or(phrase.trim());
    let phrase = phrase.strip_suffix(" named").unwrap_or(phrase);
    // Dotted declaration paths require lexical qualification that Python does not yet emit.
    // A dotted module name is already an exact checker-provided module identity.
    if name.contains('.')
        && !matches!(
            phrase,
            "Python module" | "Python syntax node" | "Rust syntax node"
        )
    {
        return None;
    }
    let meaning = match phrase {
        "Python function" => "Python function declarations",
        "Python syntax node" => "Python syntax nodes",
        "Rust syntax node" => "Rust syntax nodes",
        "Rust function" => "Rust function declarations",
        "function" => "function declarations",
        "Python class" => "Python class declarations",
        "Python module" => "Python modules",
        "Python parameter" => "Python parameter declarations",
        "Python binding" => "Python binding declarations",
        "Rust constant" => "Rust constant declarations",
        "Rust static" => "Rust static declarations",
        _ => return None,
    };
    let predicate = TextPropertyPredicate {
        property: if phrase.starts_with("Rust ") && name.contains("::") {
            "qualified name"
        } else {
            "name"
        }
        .into(),
        operator: TextComparison::Equal,
        value: name.into(),
    };
    Some((meaning, predicate))
}

pub(super) fn without_literal_values(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(fields) => {
            if let Some(serde_json::Value::String(phrase)) = fields.get_mut("looking_for")
                && let Some((meaning, _)) = entity_selection(phrase)
            {
                *phrase = meaning;
            }
            if let Some(serde_json::Value::Array(conditions)) = fields.get_mut("where") {
                for condition in conditions {
                    let encoded = condition
                        .as_str()
                        .map_or_else(|| condition.to_string(), str::to_owned);
                    if TextPropertyPredicate::parse(&encoded).is_ok() {
                        *condition = serde_json::json!({"literal_property_predicate":true});
                    }
                }
            }
            for key in ["about", "starting_from", "for", "stop_when"] {
                if let Some(subjects) = fields.get_mut(key) {
                    mask_subjects(subjects);
                }
            }
            for child in fields.values_mut() {
                without_literal_values(child);
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                without_literal_values(child);
            }
        }
        _ => {}
    }
}

fn mask_subjects(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(phrase) if named_subject(phrase).is_some() => {
            *phrase = "literal code subject".into();
        }
        serde_json::Value::Array(subjects) => subjects.iter_mut().for_each(mask_subjects),
        serde_json::Value::Object(fields) => {
            if let Some(phrase) = fields.get_mut("semantic_reference") {
                mask_subjects(phrase);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_names_remain_exact_and_separate_from_the_kind_phrase() {
        let (meaning, predicate) = entity_selection("the Python function named `café`").unwrap();
        assert_eq!(meaning, "Python function declarations");
        assert_eq!(
            TextPropertyPredicate::parse(&predicate).unwrap().value,
            "café"
        );
        let (_, predicate) = entity_selection("the Rust function `crate::module::target`").unwrap();
        assert_eq!(
            TextPropertyPredicate::parse(&predicate).unwrap().property,
            "qualified name"
        );
        for invalid in [
            "Python function named target",
            "Python function named ``",
            "Python function `a` or `b`",
            "Python function `a` extra",
            "Python function `package.Class.method`",
            "function `package.Class.method`",
            "safe to refactor `a`",
        ] {
            assert!(entity_selection(invalid).is_none());
        }
        let module = named_subject("Python module `package.module`").unwrap();
        assert_eq!(module.selector, "python:module");
        assert_eq!(module.predicate.value, "package.module");
    }
    #[test]
    fn objective_intent_check_distinguishes_code_literals_from_judgments() {
        let check = |value: serde_json::Value| {
            super::super::contains_evaluative_intent(&serde_json::to_vec(&value).unwrap())
        };
        assert!(!check(
            serde_json::json!({"queries":[{"looking_for":"the Rust function named `safe_to_refactor`","where":[{"property":"name","operator":"equals","value":"high risk"}]}]})
        ));
        assert!(!check(
            serde_json::json!({"queries":[{"where":[r#"{"property":"name","operator":"equals","value":"should_change"}"#]}]})
        ));
        assert!(!check(serde_json::json!({"queries":[
            {"about":[{"semantic_reference":"Python function `safe_to_refactor`"}]},
            {"starting_from":["Rust function `should_change`"]},
            {"stop_when":["Python function `high_risk`"]}
        ]})));
        assert!(check(
            serde_json::json!({"queries":[{"about":["high risk functions"]}]})
        ));
        assert!(check(
            serde_json::json!({"queries":[{"facts":["safe_to_refactor"]}]})
        ));
        assert!(check(
            serde_json::json!({"queries":[{"looking_for":"high risk functions"}]})
        ));
        assert!(check(
            serde_json::json!({"queries":[{"looking_for":"Python function `x` should change"}]})
        ));
    }
}
