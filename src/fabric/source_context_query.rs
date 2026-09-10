//! Bounded source-span projection after subject selection and independent authorization.

use std::sync::Arc;

use arrow_array::{
    Array, ArrayRef, BinaryArray, BooleanArray, FixedSizeBinaryArray, StringArray, StructArray,
    UInt64Array,
};
use arrow_schema::{DataType, Field, Fields};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use datafusion::common::DataFusionError;
use datafusion::logical_expr::{ColumnarValue, ScalarUDF, Volatility, create_udf};

use super::source_context::{
    SourceAccessGrant, SourceContextContent, SourceContextMaterializationInput, SourceSpanIdentity,
    materialize_authorized_source_context,
};
use super::source_disclosure::SourceDisclosureAuthority;
use crate::identity::{IdentityDomain, encode_public_id};
use crate::workspace_registry::SourceDisclosureGrant;

/// Daemon-owned parameters for the exact selected source and live disclosure policy.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SourceContextParameters {
    pub(crate) authority: Arc<SourceDisclosureAuthority>,
    pub(crate) grant: SourceDisclosureGrant,
    pub(crate) workspace_id: String,
    pub(crate) snapshot_id: String,
    pub(crate) authorization_scope: String,
    pub(crate) policy_identity: String,
    pub(crate) maximum_source_bytes: Option<usize>,
    pub(crate) line_window: Option<(usize, usize)>,
}

pub(crate) const INPUT_FIELDS: [&str; 11] = [
    "public_entity_id",
    "context_id",
    "file_id",
    "content_digest",
    "relative_path",
    "source_generation",
    "start_byte",
    "end_byte",
    "source_bytes",
    "context_kind",
    "language",
];

pub(crate) fn output_type() -> DataType {
    DataType::Struct(Fields::from(vec![
        Field::new("source_context_id", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, true),
        Field::new("bytes", DataType::Binary, true),
        Field::new("returned_bytes", DataType::UInt64, false),
        Field::new("omitted_bytes", DataType::UInt64, false),
        Field::new("complete", DataType::Boolean, false),
        Field::new("start_byte", DataType::UInt64, false),
        Field::new("end_byte", DataType::UInt64, false),
        Field::new("start_line", DataType::UInt64, false),
        Field::new("start_byte_column", DataType::UInt64, false),
        Field::new("end_line", DataType::UInt64, false),
        Field::new("end_byte_column", DataType::UInt64, false),
        Field::new("start_utf8_column", DataType::UInt64, true),
        Field::new("start_utf16_column", DataType::UInt64, true),
        Field::new("end_utf8_column", DataType::UInt64, true),
        Field::new("end_utf16_column", DataType::UInt64, true),
        Field::new("anchor_start_byte", DataType::UInt64, false),
        Field::new("anchor_end_byte", DataType::UInt64, false),
        Field::new("requested_start_byte", DataType::UInt64, false),
        Field::new("requested_end_byte", DataType::UInt64, false),
    ]))
}

pub(crate) fn function(parameters: SourceContextParameters) -> Arc<ScalarUDF> {
    let name = format!(
        "codefabric_source_context_{}",
        blake3::hash(format!("{parameters:?}").as_bytes()).to_hex()
    );
    Arc::new(create_udf(
        &name,
        vec![
            DataType::Utf8,
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(32),
            DataType::Binary,
            DataType::UInt64,
            DataType::UInt64,
            DataType::UInt64,
            DataType::Binary,
            DataType::Utf8,
            DataType::Utf8,
        ],
        output_type(),
        Volatility::Volatile,
        Arc::new(move |values| materialize(&parameters, values)),
    ))
}

pub(crate) fn authorization_function(parameters: SourceContextParameters) -> Arc<ScalarUDF> {
    let name = format!(
        "codefabric_source_access_{}",
        blake3::hash(format!("{parameters:?}").as_bytes()).to_hex()
    );
    Arc::new(create_udf(
        &name,
        vec![],
        DataType::Boolean,
        Volatility::Volatile,
        Arc::new(move |_| {
            if parameters.authority.authorize().map_err(error)? != parameters.grant {
                return Err(error(
                    "source disclosure authorization changed during execution",
                ));
            }
            Ok(ColumnarValue::Scalar(
                datafusion::common::ScalarValue::Boolean(Some(true)),
            ))
        }),
    ))
}

fn error(detail: impl std::fmt::Display) -> DataFusionError {
    DataFusionError::Execution(detail.to_string())
}

fn typed<T: Array + 'static>(arrays: &[ArrayRef], index: usize) -> Result<&T, DataFusionError> {
    arrays
        .get(index)
        .and_then(|array| array.as_any().downcast_ref::<T>())
        .ok_or_else(|| error("source descriptor column type mismatch"))
}

fn public_fixed(
    arrays: &[ArrayRef],
    index: usize,
    row: usize,
    domain: IdentityDomain,
) -> Result<String, DataFusionError> {
    let bytes = typed::<FixedSizeBinaryArray>(arrays, index)?
        .value(row)
        .try_into()
        .map_err(|_| error("invalid source identity width"))?;
    encode_public_id(domain, None, bytes).map_err(error)
}

fn position(bytes: &[u8], offset: usize) -> (u64, u64) {
    let prefix = &bytes[..offset];
    let (line, line_start) =
        prefix
            .iter()
            .enumerate()
            .fold((1, 0), |(line, start), (index, byte)| {
                if *byte == b'\n' {
                    (line + 1, index + 1)
                } else {
                    (line, start)
                }
            });
    (line, (offset - line_start) as u64)
}

/// Expand complete physical lines around a half-open anchor, preserving line endings.
fn line_span(
    bytes: &[u8],
    start: usize,
    end: usize,
    before: usize,
    after: usize,
) -> Result<(usize, usize), &'static str> {
    if start > end || end > bytes.len() {
        return Err("source line anchor is outside captured bytes");
    }
    let mut first = bytes[..start]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);
    for _ in 0..before {
        if first == 0 {
            break;
        }
        first = bytes[..first - 1]
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |index| index + 1);
    }
    let last_anchor = end - usize::from(end > start);
    let mut last = bytes[last_anchor..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(bytes.len(), |index| last_anchor + index + 1);
    for _ in 0..after {
        if last == bytes.len() {
            break;
        }
        last = bytes[last..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |index| last + index + 1);
    }
    Ok((first, last))
}

/// Zero-based columns in decoded UTF-8 bytes and UTF-16 code units. A BOM has no
/// text column; offsets inside a character are deliberately unmappable. Walk both
/// endpoints together without allocating another source or per-character index.
fn text_columns(bytes: &[u8], python: bool, start: usize, end: usize) -> [Option<(u64, u64)>; 2] {
    let Ok(decoded) = crate::source_encoding::DecodedSource::select(bytes, python) else {
        return [None; 2];
    };
    let mut columns = (0, 0);
    let mut result = [None; 2];
    let positions = [start, end];
    for (original, character) in decoded
        .characters()
        .map(|(offset, ch)| (offset, Some(ch)))
        .chain(std::iter::once((decoded.original_len(), None)))
    {
        if original > end {
            break;
        }
        for (index, position) in positions.iter().enumerate() {
            if *position == original {
                result[index] = Some(columns);
            }
        }
        if let Some(character) = character {
            if character == '\n' {
                columns = (0, 0);
            } else {
                columns.0 += character.len_utf8() as u64;
                columns.1 += character.len_utf16() as u64;
            }
        }
    }
    result
}

#[allow(
    clippy::too_many_lines,
    reason = "one bounded Arrow batch has one source-policy check and columnar output construction"
)]
fn materialize(
    parameters: &SourceContextParameters,
    values: &[ColumnarValue],
) -> Result<ColumnarValue, DataFusionError> {
    if parameters.authority.authorize().map_err(error)? != parameters.grant {
        return Err(error(
            "source disclosure authorization changed during execution",
        ));
    }
    let arrays = ColumnarValue::values_to_arrays(values)?;
    if arrays.len() != INPUT_FIELDS.len() || arrays.iter().any(|array| array.null_count() != 0) {
        return Err(error("source descriptor is missing an exact input"));
    }
    let count = arrays[0].len();
    let mut identities = Vec::with_capacity(count);
    let mut texts = Vec::with_capacity(count);
    let mut bytes = Vec::with_capacity(count);
    let mut returned = Vec::with_capacity(count);
    let mut omitted = Vec::with_capacity(count);
    let mut complete = Vec::with_capacity(count);
    let mut starts = Vec::with_capacity(count);
    let mut ends = Vec::with_capacity(count);
    let mut start_lines = Vec::with_capacity(count);
    let mut start_columns = Vec::with_capacity(count);
    let mut end_lines = Vec::with_capacity(count);
    let mut end_columns = Vec::with_capacity(count);
    let mut start_utf8_columns = Vec::with_capacity(count);
    let mut start_utf16_columns = Vec::with_capacity(count);
    let mut end_utf8_columns = Vec::with_capacity(count);
    let mut end_utf16_columns = Vec::with_capacity(count);
    let mut anchor_starts = Vec::with_capacity(count);
    let mut anchor_ends = Vec::with_capacity(count);
    let mut requested_starts = Vec::with_capacity(count);
    let mut requested_ends = Vec::with_capacity(count);
    for row in 0..count {
        let source = typed::<BinaryArray>(&arrays, 8)?.value(row);
        let start = usize::try_from(typed::<UInt64Array>(&arrays, 6)?.value(row)).map_err(error)?;
        let end = usize::try_from(typed::<UInt64Array>(&arrays, 7)?.value(row)).map_err(error)?;
        anchor_starts.push(start as u64);
        anchor_ends.push(end as u64);
        let context_kind = typed::<StringArray>(&arrays, 9)?.value(row);
        let (start, end) = if context_kind == "surrounding lines" {
            let (before, after) = parameters
                .line_window
                .ok_or_else(|| error("surrounding lines has no explicit window"))?;
            line_span(source, start, end, before, after).map_err(error)?
        } else {
            (start, end)
        };
        requested_starts.push(start as u64);
        requested_ends.push(end as u64);
        let digest = typed::<FixedSizeBinaryArray>(&arrays, 3)?.value(row);
        let digest = blake3::Hash::from_bytes(
            digest
                .try_into()
                .map_err(|_| error("invalid source digest width"))?,
        );
        let materialized =
            materialize_authorized_source_context(SourceContextMaterializationInput {
                span: SourceSpanIdentity {
                    entity_id: Arc::from(typed::<StringArray>(&arrays, 0)?.value(row)),
                    workspace_id: Arc::from(parameters.workspace_id.as_str()),
                    source_file_id: Arc::from(public_fixed(
                        &arrays,
                        2,
                        row,
                        IdentityDomain::SourceFile,
                    )?),
                    content_digest: Arc::from(format!("b3:{}", digest.to_hex())),
                    byte_safe_path: Arc::from(format!(
                        "base64:{}",
                        STANDARD.encode(typed::<BinaryArray>(&arrays, 4)?.value(row))
                    )),
                    start_byte: start,
                    end_byte: end,
                    source_generation: typed::<UInt64Array>(&arrays, 5)?.value(row),
                },
                grant: SourceAccessGrant {
                    source_access: true,
                    workspace_id: Arc::from(parameters.workspace_id.as_str()),
                    authorized_start_byte: 0,
                    authorized_end_byte: source.len(),
                    authorization_scope: Arc::from(parameters.authorization_scope.as_str()),
                },
                analysis_context_id: Arc::from(public_fixed(
                    &arrays,
                    1,
                    row,
                    IdentityDomain::AnalysisContext,
                )?),
                snapshot_id: Arc::from(parameters.snapshot_id.as_str()),
                context_kind: Arc::from(context_kind),
                policy_identity: Arc::from(parameters.policy_identity.as_str()),
                source_bytes: source,
                declared_byte_length: source.len(),
                explicit_source_byte_limit: parameters.maximum_source_bytes.unwrap_or(usize::MAX),
                hard_output_byte_limit: 1024 * 1024,
            })
            .map_err(|error| DataFusionError::External(Box::new(error)))?;
        let delivered_end = start + materialized.returned_bytes;
        let (start_line, start_column) = position(source, start);
        let (end_line, end_column) = position(source, delivered_end);
        let [start_text, end_text] = text_columns(
            source,
            typed::<StringArray>(&arrays, 10)?.value(row) == "python",
            start,
            delivered_end,
        );
        start_utf8_columns.push(start_text.map(|columns| columns.0));
        start_utf16_columns.push(start_text.map(|columns| columns.1));
        end_utf8_columns.push(end_text.map(|columns| columns.0));
        end_utf16_columns.push(end_text.map(|columns| columns.1));
        identities.push(materialized.source_context_id.to_string());
        match materialized.content {
            SourceContextContent::Text(text) => {
                texts.push(Some(text));
                bytes.push(None);
            }
            SourceContextContent::Bytes(value) => {
                texts.push(None);
                bytes.push(Some(value));
            }
        }
        returned.push(materialized.returned_bytes as u64);
        omitted.push(materialized.omitted_bytes as u64);
        complete.push(materialized.complete);
        starts.push(start as u64);
        ends.push(delivered_end as u64);
        start_lines.push(start_line);
        start_columns.push(start_column);
        end_lines.push(end_line);
        end_columns.push(end_column);
    }
    let output: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(identities)),
        Arc::new(StringArray::from_iter(texts.iter().map(Option::as_deref))),
        Arc::new(BinaryArray::from_iter(bytes.iter().map(Option::as_deref))),
        Arc::new(UInt64Array::from(returned)),
        Arc::new(UInt64Array::from(omitted)),
        Arc::new(BooleanArray::from(complete)),
        Arc::new(UInt64Array::from(starts)),
        Arc::new(UInt64Array::from(ends)),
        Arc::new(UInt64Array::from(start_lines)),
        Arc::new(UInt64Array::from(start_columns)),
        Arc::new(UInt64Array::from(end_lines)),
        Arc::new(UInt64Array::from(end_columns)),
        Arc::new(UInt64Array::from(start_utf8_columns)),
        Arc::new(UInt64Array::from(start_utf16_columns)),
        Arc::new(UInt64Array::from(end_utf8_columns)),
        Arc::new(UInt64Array::from(end_utf16_columns)),
        Arc::new(UInt64Array::from(anchor_starts)),
        Arc::new(UInt64Array::from(anchor_ends)),
        Arc::new(UInt64Array::from(requested_starts)),
        Arc::new(UInt64Array::from(requested_ends)),
    ];
    let DataType::Struct(fields) = output_type() else {
        unreachable!("closed source schema");
    };
    Ok(ColumnarValue::Array(Arc::new(StructArray::try_new(
        fields, output, None,
    )?)))
}

#[cfg(test)]
mod tests {
    use super::text_columns;

    #[test]
    fn surrounding_line_spans_preserve_crlf_half_open_endpoints_and_file_edges() {
        let source = b"zero\r\none\ntwo\nlast";
        assert_eq!(super::line_span(source, 7, 8, 0, 0), Ok((6, 10)));
        assert_eq!(super::line_span(source, 7, 8, 1, 1), Ok((0, 14)));
        assert_eq!(super::line_span(source, 6, 10, 0, 0), Ok((6, 10)));
        assert_eq!(super::line_span(source, 7, 8, 100, 100), Ok((0, 18)));
        assert_eq!(super::line_span(source, 18, 18, 0, 0), Ok((14, 18)));
        assert_eq!(super::line_span(b"", 0, 0, 4, 4), Ok((0, 0)));
        assert!(super::line_span(source, 4, 3, 0, 0).is_err());
        assert!(super::line_span(source, 0, 19, 0, 0).is_err());
    }

    #[test]
    fn source_text_columns_cover_bom_surrogates_crlf_and_partial_characters() {
        let source = "\u{feff}a😀é\r\nz".as_bytes();
        assert_eq!(
            text_columns(source, true, 3, 10),
            [Some((0, 0)), Some((7, 4))]
        );
        assert_eq!(
            text_columns(source, false, 4, 8),
            [Some((1, 1)), Some((5, 3))]
        );
        assert_eq!(text_columns(source, true, 5, 9), [None, None]);
        assert_eq!(text_columns(source, true, 0, 2), [None, None]);
        assert_eq!(
            text_columns(source, true, 12, 13),
            [Some((0, 0)), Some((1, 1))]
        );
        assert_eq!(text_columns(b"", true, 0, 0), [Some((0, 0)); 2]);
        assert_eq!(
            text_columns(b"# coding: latin-1\n\xe9x", true, 18, 20),
            [Some((0, 0)), Some((3, 2))]
        );
        assert_eq!(
            text_columns(b"# coding: latin-1\n\xe9x", false, 18, 20),
            [None; 2]
        );
    }
}
