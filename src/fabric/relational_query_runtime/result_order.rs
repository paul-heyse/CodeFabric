//! Scope filters must run before the final native ordering and explicit result limit.

use crate::relational_program::{RelationalExpression as R, SortExpression};

pub(super) fn before_order(root: R, transform: impl FnOnce(R) -> R) -> R {
    match root {
        R::Limit { input, skip, fetch } => R::Limit {
            input: Box::new(before_order(*input, transform)),
            skip,
            fetch,
        },
        R::Sort { input, expressions } => R::Sort {
            input: Box::new(before_order(*input, transform)),
            expressions,
        },
        input => transform(input),
    }
}

pub(super) fn try_before_order<T>(
    root: R,
    transform: impl FnOnce(R) -> Result<R, T>,
) -> Result<R, T> {
    Ok(match root {
        R::Limit { input, skip, fetch } => R::Limit {
            input: Box::new(try_before_order(*input, transform)?),
            skip,
            fetch,
        },
        R::Sort { input, expressions } => R::Sort {
            input: Box::new(try_before_order(*input, transform)?),
            expressions,
        },
        input => return transform(input),
    })
}

pub(super) fn append(root: &mut R, order: &[SortExpression]) {
    match root {
        R::Limit { input, .. } => append(input, order),
        R::Sort { expressions, .. } => expressions.extend_from_slice(order),
        _ => {
            *root = R::Sort {
                input: Box::new(root.clone()),
                expressions: order.to_vec(),
            };
        }
    }
}
