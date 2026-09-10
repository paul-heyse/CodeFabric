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
    if mapping.is_empty() {
        return;
    }
    match expression {
        ScalarExpression::Field(field) => rename(field, mapping),
        ScalarExpression::Literal(_) => {}
        ScalarExpression::Call { arguments, .. } => {
            for argument in arguments {
                scalar(argument, mapping);
            }
        }
        #[cfg(feature = "daemon")]
        ScalarExpression::SourceContext { arguments, .. }
        | ScalarExpression::PublicEntityId { arguments } => {
            for argument in arguments {
                scalar(argument, mapping);
            }
        }
    }
}

// Track only renamed values actually produced by each subtree. A query-local input may
// carry the same semantic field IDs as the final result without defining this block's outputs.
fn relational(
    expression: &mut RelationalExpression,
    mapping: &BTreeMap<FieldId, FieldId>,
) -> BTreeMap<FieldId, FieldId> {
    match expression {
        RelationalExpression::Input(_) => BTreeMap::new(),
        RelationalExpression::Projection { input, expressions } => {
            let upstream = relational(input, mapping);
            let mut output = BTreeMap::new();
            for named in expressions {
                scalar(&mut named.expression, &upstream);
                bind(&mut named.field_id, mapping, &mut output);
            }
            output
        }
        RelationalExpression::Filter { input, predicate } => {
            let visible = relational(input, mapping);
            scalar(predicate, &visible);
            visible
        }
        RelationalExpression::Join {
            left,
            right,
            predicates,
            kind,
        } => {
            let left = relational(left, mapping);
            let right = relational(right, mapping);
            let mut visible = left.clone();
            visible.extend(right.clone());
            for predicate in predicates {
                scalar(predicate, &visible);
            }
            let mut output = if kind.retains_left() {
                left
            } else {
                BTreeMap::new()
            };
            if kind.retains_right() {
                output.extend(right);
            }
            output
        }
        RelationalExpression::Union { inputs, .. } => {
            let mut branches = inputs.iter_mut().map(|input| relational(input, mapping));
            let mut visible = branches.next().unwrap_or_default();
            for branch in branches {
                // Native union validation still requires identical field contracts on every
                // branch; a value is visible above the union only with one consistent binding.
                visible.retain(|field, renamed| branch.get(field) == Some(renamed));
            }
            visible
        }
        RelationalExpression::Aggregate {
            input,
            group_by,
            aggregates,
        } => {
            let upstream = relational(input, mapping);
            let mut output = BTreeMap::new();
            for named in group_by {
                scalar(&mut named.expression, &upstream);
                bind(&mut named.field_id, mapping, &mut output);
            }
            for named in aggregates {
                scalar(&mut named.expression.argument, &upstream);
                bind(&mut named.field_id, mapping, &mut output);
            }
            output
        }
        RelationalExpression::Sort { input, expressions } => {
            let visible = relational(input, mapping);
            for sort in expressions {
                scalar(&mut sort.expression, &visible);
            }
            visible
        }
        RelationalExpression::Limit { input, .. } => relational(input, mapping),
    }
}

fn bind(
    field: &mut FieldId,
    mapping: &BTreeMap<FieldId, FieldId>,
    output: &mut BTreeMap<FieldId, FieldId>,
) {
    if let Some(renamed) = mapping.get(field) {
        output.insert(field.clone(), renamed.clone());
        *field = renamed.clone();
    }
}
