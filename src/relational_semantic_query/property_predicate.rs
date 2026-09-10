//! Public semantic property names bind only to fields declared by the selected program.

use super::{FieldId, ScalarExpression, ScalarOperator, SemanticClauseValue};
use datafusion::common::ScalarValue;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TextPropertyPredicate {
    pub property: String,
    pub operator: TextComparison,
    pub value: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub(crate) enum TextComparison {
    #[serde(rename = "equals")]
    Equal,
    #[serde(rename = "does not equal")]
    NotEqual,
}

/// Ingress separates controlled kind selection from a literal name before native lowering.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NamedEntityPredicate {
    pub selector: String,
    pub predicate: TextPropertyPredicate,
}

impl NamedEntityPredicate {
    pub(crate) fn expression(
        value: &SemanticClauseValue,
        fields: &BTreeMap<Arc<str>, FieldId>,
    ) -> Result<ScalarExpression, String> {
        let SemanticClauseValue::Text(value) = value else {
            return Err("named entity predicate is not text".into());
        };
        let named: Self =
            serde_json::from_str(value).map_err(|_| "invalid named entity predicate")?;
        if !matches!(named.predicate.property.as_str(), "name" | "qualified name")
            || !matches!(named.predicate.operator, TextComparison::Equal)
        {
            return Err("named subjects require literal name equality".into());
        }
        let selector = fields
            .get("selector")
            .ok_or("entity selector field is unavailable")?;
        let name = named.predicate.lower(fields)?;
        Ok(ScalarExpression::Call {
            operator: ScalarOperator::And,
            arguments: vec![
                ScalarExpression::Call {
                    operator: ScalarOperator::Equal,
                    arguments: vec![
                        ScalarExpression::Field(selector.clone()),
                        ScalarExpression::Literal(ScalarValue::Utf8(Some(named.selector))),
                    ],
                },
                name,
            ],
        })
    }
}

impl TextPropertyPredicate {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        serde_json::from_str(value).map_err(|_| {
            "expected a text property predicate with property, operator and literal value".into()
        })
    }

    pub(crate) fn expression(
        value: &SemanticClauseValue,
        fields: &BTreeMap<Arc<str>, FieldId>,
    ) -> Result<ScalarExpression, String> {
        let SemanticClauseValue::Text(value) = value else {
            return Err("text property predicate is not text".into());
        };
        Self::parse(value)?.lower(fields)
    }

    fn lower(&self, fields: &BTreeMap<Arc<str>, FieldId>) -> Result<ScalarExpression, String> {
        let predicate = self;
        let field = fields
            .get(predicate.property.as_str())
            .ok_or("property is unavailable in the selected result meaning")?;
        let equal = ScalarExpression::Call {
            operator: ScalarOperator::Equal,
            arguments: vec![
                ScalarExpression::Field(field.clone()),
                ScalarExpression::Literal(ScalarValue::Utf8(Some(predicate.value.clone()))),
            ],
        };
        // Rust's retained declaration name is qualified. Match a bare identifier at a namespace
        // boundary while keeping qualified-name predicates exact and preserving stored evidence.
        let equal = if predicate.property == "name" && !predicate.value.contains("::") {
            ScalarExpression::Call {
                operator: ScalarOperator::Or,
                arguments: vec![
                    equal,
                    ScalarExpression::Call {
                        operator: ScalarOperator::TextEndsWith,
                        arguments: vec![
                            ScalarExpression::Field(field.clone()),
                            ScalarExpression::Literal(ScalarValue::Utf8(Some(format!(
                                "::{}",
                                predicate.value
                            )))),
                        ],
                    },
                ],
            }
        } else {
            equal
        };
        Ok(match predicate.operator {
            TextComparison::Equal => equal,
            TextComparison::NotEqual => ScalarExpression::Call {
                operator: ScalarOperator::Not,
                arguments: vec![equal],
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_values_do_not_select_fields_operators_or_physical_objects() {
        let field = FieldId::new("private.source.name").unwrap();
        let fields = BTreeMap::from([(Arc::from("qualified name"), field.clone())]);
        let literal = "safe_to_refactor; SELECT * FROM private.source";
        let value = |property, operator, value| {
            SemanticClauseValue::Text(
                serde_json::json!({"property":property,"operator":operator,"value":value})
                    .to_string()
                    .into(),
            )
        };
        assert_eq!(
            TextPropertyPredicate::expression(&value("qualified name", "equals", literal), &fields)
                .unwrap(),
            ScalarExpression::Call {
                operator: ScalarOperator::Equal,
                arguments: vec![
                    ScalarExpression::Field(field),
                    ScalarExpression::Literal(ScalarValue::Utf8(Some(literal.into())))
                ],
            }
        );
        assert!(
            TextPropertyPredicate::expression(
                &value("private.source.name", "equals", "x"),
                &fields
            )
            .is_err()
        );
        assert!(TextPropertyPredicate::expression(&value("name", "LIKE", "%"), &fields).is_err());
        assert!(
            TextPropertyPredicate::parse(
                r#"{"property":"name","operator":"equals","value":"x","sql":"x"}"#
            )
            .is_err()
        );
        assert!(
            TextPropertyPredicate::parse(r#"{"property":"name","operator":"equals","value":1}"#)
                .is_err()
        );
    }
}
