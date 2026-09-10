//! Rename typed compiler fields while keeping epoch relation capabilities unchanged.

use super::{BTreeMap, FieldId, RelationalExpression, RelationalProgram, ScalarExpression};

pub(super) fn fields(program: &mut RelationalProgram, mapping: &BTreeMap<FieldId, FieldId>) {
    for field in &mut program.output_fields {
        rename(field, mapping);
    }
    relational(&mut program.root, mapping);
}

fn rename(field: &mut FieldId, mapping: &BTreeMap<FieldId, FieldId>) {
    if let Some(replacement) = mapping.get(field) {
        *field = replacement.clone();
    }
}

fn scalar(expression: &mut ScalarExpression, mapping: &BTreeMap<FieldId, FieldId>) {
    match expression {
        ScalarExpression::Field(field) => rename(field, mapping),
        ScalarExpression::Literal(_) => {}
        ScalarExpression::Call { arguments, .. } => {
            for argument in arguments {
                scalar(argument, mapping);
            }
        }
        #[cfg(feature = "daemon")]
        ScalarExpression::SourceContext { arguments, .. } => {
            for argument in arguments {
                scalar(argument, mapping);
            }
        }
    }
}

fn relational(expression: &mut RelationalExpression, mapping: &BTreeMap<FieldId, FieldId>) {
    match expression {
        RelationalExpression::Input(_) => {}
        RelationalExpression::Projection { input, expressions } => {
            relational(input, mapping);
            for named in expressions {
                rename(&mut named.field_id, mapping);
                scalar(&mut named.expression, mapping);
            }
        }
        RelationalExpression::Filter { input, predicate } => {
            relational(input, mapping);
            scalar(predicate, mapping);
        }
        RelationalExpression::Join {
            left,
            right,
            predicates,
            ..
        } => {
            relational(left, mapping);
            relational(right, mapping);
            for predicate in predicates {
                scalar(predicate, mapping);
            }
        }
        RelationalExpression::Union { inputs, .. } => {
            for input in inputs {
                relational(input, mapping);
            }
        }
        RelationalExpression::Aggregate {
            input,
            group_by,
            aggregates,
        } => {
            relational(input, mapping);
            for named in group_by {
                rename(&mut named.field_id, mapping);
                scalar(&mut named.expression, mapping);
            }
            for named in aggregates {
                rename(&mut named.field_id, mapping);
                scalar(&mut named.expression.argument, mapping);
            }
        }
        RelationalExpression::Sort { input, expressions } => {
            relational(input, mapping);
            for sort in expressions {
                scalar(&mut sort.expression, mapping);
            }
        }
        RelationalExpression::Limit { input, .. } => relational(input, mapping),
    }
}
