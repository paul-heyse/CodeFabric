//! Owned callable and call-occurrence observations from the existing Ruff traversal.

use super::{
    Arc, ArrayRef, ArrowError, DataType, Field, NativeSyntaxRelation, PythonFrontendBatch,
    RecordBatch, RelationPin, UInt64Array, batch, fixed16, typed_field, utf8,
};
use crate::ruff_adapter::{PythonCallableSyntaxRole, PythonDispatchKind};

pub(super) fn fields(relation: NativeSyntaxRelation) -> Vec<Field> {
    let specs = match relation {
        NativeSyntaxRelation::RuffCallable => vec![
            ("callable_id", DataType::FixedSizeBinary(16), false),
            ("owner_scope_id", DataType::FixedSizeBinary(16), false),
            ("declared_binding_id", DataType::FixedSizeBinary(16), true),
            ("class_id", DataType::FixedSizeBinary(16), true),
            ("name", DataType::Utf8, false),
            ("qualified_name", DataType::Utf8, false),
            ("parameter_count", DataType::Int32, false),
            ("generic_parameter_count", DataType::Int32, false),
            ("flags", DataType::Int64, false),
            ("start_byte", DataType::UInt64, false),
            ("end_byte", DataType::UInt64, false),
        ],
        NativeSyntaxRelation::RuffCallSite => vec![
            ("call_site_id", DataType::FixedSizeBinary(16), false),
            ("caller_id", DataType::FixedSizeBinary(16), false),
            ("syntax_id", DataType::FixedSizeBinary(16), false),
            ("callee_syntax_id", DataType::FixedSizeBinary(16), false),
            ("receiver_syntax_id", DataType::FixedSizeBinary(16), true),
            ("declared_target_id", DataType::FixedSizeBinary(16), true),
            ("dispatch_kind", DataType::Utf8, false),
            ("flags", DataType::Int64, false),
        ],
        NativeSyntaxRelation::RuffCallableSyntax => vec![
            ("syntax_id", DataType::FixedSizeBinary(16), false),
            ("owner_id", DataType::FixedSizeBinary(16), false),
            ("role", DataType::Utf8, false),
            ("ordinal", DataType::Int32, true),
            ("text", DataType::Utf8, false),
            ("start_byte", DataType::UInt64, false),
            ("end_byte", DataType::UInt64, false),
        ],
        _ => unreachable!("only callable relations reach this projection"),
    };
    specs
        .into_iter()
        .map(|(name, kind, nullable)| {
            typed_field(name, kind, nullable, "ruff-callable-observation")
        })
        .collect()
}

pub(super) fn project(
    pin: RelationPin<'_>,
    relation: NativeSyntaxRelation,
    semantics: Option<&PythonFrontendBatch>,
) -> Result<RecordBatch, ArrowError> {
    use arrow_array::{Int32Array, Int64Array};
    let (count, columns): (_, Vec<ArrayRef>) = match relation {
        NativeSyntaxRelation::RuffCallable => {
            let rows = semantics.map_or(&[][..], |batch| batch.callables.as_slice());
            (
                rows.len(),
                vec![
                    fixed16(rows, |row| Some(&row.callable_id)),
                    fixed16(rows, |row| Some(&row.owner_scope_id)),
                    fixed16(rows, |row| row.declared_binding_id.as_ref()),
                    fixed16(rows, |row| row.class_id.as_ref()),
                    utf8(rows, |row| Some(&row.name)),
                    utf8(rows, |row| Some(&row.qualified_name)),
                    Arc::new(Int32Array::from_iter_values(
                        rows.iter().map(|row| row.parameter_count),
                    )),
                    Arc::new(Int32Array::from_iter_values(
                        rows.iter().map(|row| row.generic_parameter_count),
                    )),
                    Arc::new(Int64Array::from_iter_values(
                        rows.iter().map(|row| row.flags),
                    )),
                    Arc::new(UInt64Array::from_iter_values(
                        rows.iter().map(|row| row.start_byte),
                    )),
                    Arc::new(UInt64Array::from_iter_values(
                        rows.iter().map(|row| row.end_byte),
                    )),
                ],
            )
        }
        NativeSyntaxRelation::RuffCallSite => {
            let rows = semantics.map_or(&[][..], |batch| batch.call_sites.as_slice());
            (
                rows.len(),
                vec![
                    fixed16(rows, |row| Some(&row.call_site_id)),
                    fixed16(rows, |row| Some(&row.caller_id)),
                    fixed16(rows, |row| Some(&row.syntax_id)),
                    fixed16(rows, |row| Some(&row.callee_syntax_id)),
                    fixed16(rows, |row| row.receiver_syntax_id.as_ref()),
                    fixed16(rows, |row| row.declared_target_id.as_ref()),
                    utf8(rows, |row| {
                        Some(match row.dispatch_kind {
                            PythonDispatchKind::DirectName => "direct-name",
                            PythonDispatchKind::Attribute => "attribute",
                            PythonDispatchKind::Unknown => "unknown",
                        })
                    }),
                    Arc::new(Int64Array::from_iter_values(
                        rows.iter().map(|row| row.flags),
                    )),
                ],
            )
        }
        NativeSyntaxRelation::RuffCallableSyntax => {
            let rows = semantics.map_or(&[][..], |batch| batch.callable_syntax.as_slice());
            (
                rows.len(),
                vec![
                    fixed16(rows, |row| Some(&row.syntax_id)),
                    fixed16(rows, |row| Some(&row.owner_id)),
                    utf8(rows, |row| Some(role(row.role))),
                    Arc::new(Int32Array::from_iter(rows.iter().map(|row| row.ordinal))),
                    utf8(rows, |row| Some(&row.text)),
                    Arc::new(UInt64Array::from_iter_values(
                        rows.iter().map(|row| row.start_byte),
                    )),
                    Arc::new(UInt64Array::from_iter_values(
                        rows.iter().map(|row| row.end_byte),
                    )),
                ],
            )
        }
        _ => unreachable!("only callable relations reach this projection"),
    };
    batch(pin, relation, count, columns)
}

const fn role(role: PythonCallableSyntaxRole) -> &'static str {
    match role {
        PythonCallableSyntaxRole::CallExpression => "call-expression",
        PythonCallableSyntaxRole::CalleeExpression => "callee-expression",
        PythonCallableSyntaxRole::Receiver => "receiver",
        PythonCallableSyntaxRole::Argument => "argument",
        PythonCallableSyntaxRole::Decorator => "decorator",
        PythonCallableSyntaxRole::ReturnAnnotation => "return-annotation",
        PythonCallableSyntaxRole::ParameterAnnotation => "parameter-annotation",
        PythonCallableSyntaxRole::ParameterDefault => "parameter-default",
        PythonCallableSyntaxRole::TypeParameter => "type-parameter",
    }
}
