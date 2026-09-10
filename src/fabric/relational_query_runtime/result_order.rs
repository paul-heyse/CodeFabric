//! Scope filters must run before the final native ordering and explicit result limit.

use crate::relational_program::RelationalExpression as R;

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
