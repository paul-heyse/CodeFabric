//! One bounded identity fold per exact tree; native Arrow owns grouped input and output buffers.

use std::sync::Arc;

use arrow_array::{
    Array, ArrayRef, FixedSizeBinaryArray, ListArray, StructArray, UInt16Array, UInt32Array,
    UInt64Array,
};
use arrow_schema::{DataType, Field, Fields};
use datafusion::common::DataFusionError;
use datafusion::logical_expr::{ColumnarValue, ScalarUDF, Volatility, create_udf};

use super::super::{fixed, invalid, number, text};
use super::identity::{self, Anchor};

fn fields() -> Fields {
    vec![
        Field::new("provider_local_node_id", DataType::UInt64, false),
        Field::new("entity_id", DataType::FixedSizeBinary(16), false),
        Field::new("parent_entity_id", DataType::FixedSizeBinary(16), true),
    ]
    .into()
}

pub(super) fn normalizer(nodes_type: DataType) -> Arc<ScalarUDF> {
    let item = Arc::new(Field::new("item", DataType::Struct(fields()), false));
    Arc::new(create_udf(
        "codefabric_canonical_syntax_identity_v1",
        vec![
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(32),
            DataType::UInt64,
            DataType::Utf8,
            nodes_type,
        ],
        DataType::List(item.clone()),
        Volatility::Immutable,
        Arc::new(move |values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let nodes = arrays[5]
                .as_any()
                .downcast_ref::<ListArray>()
                .ok_or_else(|| invalid("invalid grouped syntax nodes"))?;
            let digests = arrays[2]
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .ok_or_else(|| invalid("invalid syntax source digest"))?;
            let mut lengths = Vec::with_capacity(nodes.len());
            let mut output = Vec::new();
            for row in 0..nodes.len() {
                if arrays.iter().any(|array| array.is_null(row)) {
                    return Err(invalid("null syntax normalization input"));
                }
                let array = nodes.value(row);
                let records = array
                    .as_any()
                    .downcast_ref::<StructArray>()
                    .ok_or_else(|| invalid("invalid syntax anchor records"))?;
                let normalized = identity::normalize(
                    fixed(&arrays[0], row)?,
                    fixed(&arrays[1], row)?,
                    digests
                        .value(row)
                        .try_into()
                        .map_err(|_| invalid("invalid syntax digest width"))?,
                    number(&arrays[3], row)?,
                    text(&arrays[4], row)?,
                    &decode(records)?,
                )
                .map_err(|error| invalid(&error))?;
                if output
                    .len()
                    .checked_add(normalized.len())
                    .is_none_or(|len| i32::try_from(len).is_err())
                {
                    return Err(invalid(
                        "syntax identity output exceeds Arrow list offset capacity",
                    ));
                }
                lengths.push(normalized.len());
                output.extend(normalized);
            }
            let columns: Vec<ArrayRef> = vec![
                Arc::new(UInt64Array::from_iter_values(
                    output.iter().map(|node| node.local_id),
                )),
                crate::fabric::id16_array(output.iter().map(|node| Some(&node.entity_id))),
                crate::fabric::id16_array(output.iter().map(|node| node.parent_id.as_ref())),
            ];
            let records = Arc::new(StructArray::new(fields(), columns, None));
            Ok(ColumnarValue::Array(Arc::new(ListArray::new(
                item.clone(),
                arrow::buffer::OffsetBuffer::from_lengths(lengths),
                records,
                None,
            ))))
        }),
    ))
}

fn column<'a, T: Array + 'static>(
    records: &'a StructArray,
    name: &str,
) -> Result<&'a T, DataFusionError> {
    records
        .column_by_name(name)
        .and_then(|array| array.as_any().downcast_ref::<T>())
        .ok_or_else(|| invalid("invalid syntax anchor type"))
}

fn decode(records: &StructArray) -> Result<Vec<Anchor>, DataFusionError> {
    if records.len() as u64 > super::super::super::INPROCESS_MAX_VISITED_NODES {
        return Err(invalid("syntax tree exceeds the admitted node bound"));
    }
    let ids = column::<UInt64Array>(records, "provider_local_node_id")?;
    let parents = column::<UInt64Array>(records, "parent_provider_local_node_id")?;
    let starts = column::<UInt64Array>(records, "start_byte")?;
    let ends = column::<UInt64Array>(records, "end_byte")?;
    let kinds = column::<UInt16Array>(records, "normalized_kind_code")?;
    let ordinals = column::<UInt32Array>(records, "ordinal")?;
    let depths = column::<UInt16Array>(records, "depth")?;
    if records.null_count() != 0
        || [ids as &dyn Array, starts, ends, kinds, ordinals, depths]
            .iter()
            .any(|array| array.null_count() != 0)
    {
        return Err(invalid("null syntax occurrence coordinate"));
    }
    Ok((0..records.len())
        .map(|row| Anchor {
            local_id: ids.value(row),
            parent: (!parents.is_null(row)).then(|| parents.value(row)),
            start: starts.value(row),
            end: ends.value(row),
            normalized_kind: kinds.value(row),
            ordinal: ordinals.value(row),
            depth: depths.value(row),
        })
        .collect())
}
