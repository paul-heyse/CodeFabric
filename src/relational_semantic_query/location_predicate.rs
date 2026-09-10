//! Native comparisons select every matching original-source span, including zero-width syntax.

use std::collections::BTreeMap;
use std::sync::Arc;

use datafusion::common::ScalarValue;

use super::{FieldId, ScalarExpression as E, ScalarOperator as O, SemanticClauseValue};
use crate::semantic_query_contract::SourceLocation;

fn call(operator: O, left: E, right: E) -> E {
    E::Call {
        operator,
        arguments: vec![left, right],
    }
}
fn number(value: u64) -> E {
    E::Literal(ScalarValue::UInt64(Some(value)))
}

pub(crate) fn expression(
    value: &SemanticClauseValue,
    fields: &BTreeMap<Arc<str>, FieldId>,
) -> Result<E, String> {
    let SemanticClauseValue::Text(value) = value else {
        return Err("source location operand is not typed text".into());
    };
    let location: SourceLocation =
        serde_json::from_str(value).map_err(|error| error.to_string())?;
    location.validate()?;
    let meaning = location.meaning()?;
    let field = |name: &str| {
        fields
            .get(name)
            .cloned()
            .map(E::Field)
            .ok_or_else(|| format!("source location field {name} is unavailable"))
    };
    let mut predicate = call(
        O::Equal,
        field("relative_path")?,
        E::Literal(ScalarValue::Binary(Some(
            location.source_file.as_bytes().to_vec(),
        ))),
    );
    let mut add = |value| {
        predicate = call(O::And, predicate.clone(), value);
    };
    for (name, value) in [
        ("entity_kind", meaning.kind),
        ("language", meaning.language),
        ("raw_kind", meaning.raw_kind),
    ] {
        if let Some(value) = value {
            add(call(
                O::Equal,
                field(name)?,
                E::Literal(ScalarValue::Utf8(Some(value.into()))),
            ));
        }
    }
    if meaning.begins_on_line {
        add(call(
            O::Equal,
            field("start_line")?,
            number(location.start_line.expect("resolved line meaning")),
        ));
        if let Some(column) = location.start_column {
            add(call(O::Equal, field("start_column")?, number(column)));
        }
        return Ok(predicate);
    }
    let (start, end, lower, upper) = if let Some(start) = location.start_byte {
        (
            vec![field("start_byte")?],
            vec![field("end_byte")?],
            vec![number(start)],
            location.end_byte.map(|end| vec![number(end)]),
        )
    } else {
        (
            vec![field("start_line")?, field("start_column")?],
            vec![field("end_line")?, field("end_column")?],
            vec![
                number(location.start_line.expect("validated line")),
                number(location.start_column.unwrap_or(0)),
            ],
            location
                .end_line
                .map(|end| vec![number(end), number(location.end_column.unwrap_or(0))]),
        )
    };
    let within = if let Some(upper) = upper.filter(|upper| *upper != lower) {
        let overlap = call(O::And, less(&start, &upper), less(&lower, &end));
        let zero = call(
            O::And,
            equal(&start, &end),
            call(
                O::And,
                call(O::Or, less(&lower, &start), equal(&lower, &start)),
                less(&start, &upper),
            ),
        );
        call(O::Or, overlap, zero)
    } else {
        call(
            O::Or,
            call(
                O::And,
                call(O::Or, less(&start, &lower), equal(&start, &lower)),
                less(&lower, &end),
            ),
            call(O::And, equal(&start, &lower), equal(&end, &lower)),
        )
    };
    add(within);
    Ok(predicate)
}

fn equal(a: &[E], b: &[E]) -> E {
    a.iter()
        .zip(b)
        .map(|(a, b)| call(O::Equal, a.clone(), b.clone()))
        .reduce(|a, b| call(O::And, a, b))
        .expect("nonempty coordinate")
}

fn less(a: &[E], b: &[E]) -> E {
    let first = call(O::LessThan, a[0].clone(), b[0].clone());
    if a.len() == 1 {
        first
    } else {
        call(
            O::Or,
            first,
            call(
                O::And,
                call(O::Equal, a[0].clone(), b[0].clone()),
                call(O::LessThan, a[1].clone(), b[1].clone()),
            ),
        )
    }
}
