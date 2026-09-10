//! Decode one accepted owner graph from native Arrow records and return application identities.

use arrow_array::{
    Array, ArrayRef, BooleanArray, FixedSizeBinaryArray, ListArray, StringArray, StructArray,
    UInt64Array,
};
use arrow_schema::{DataType, Field, Fields};
use datafusion::common::DataFusionError;
use datafusion::logical_expr::{ColumnarValue, ScalarUDF, Volatility, create_udf};
use std::collections::BTreeMap;
use std::sync::Arc;

use super::super::super::{fixed, invalid};
use super::super::{
    normalize,
    udf::{column, encode},
};
use super::{Component, Node};

pub(super) fn output_fields() -> Fields {
    let mut fields = super::super::udf::output_fields()
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    fields[0] = Arc::new(Field::new("type_key", DataType::FixedSizeBinary(32), false));
    fields.into()
}

pub(super) fn normalizer(input: DataType) -> Arc<ScalarUDF> {
    let item = Arc::new(Field::new("item", DataType::Struct(output_fields()), false));
    Arc::new(create_udf(
        "codefabric_canonical_rust_type_graph_v1",
        vec![
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(16),
            input,
        ],
        DataType::List(Arc::clone(&item)),
        Volatility::Immutable,
        Arc::new(move |values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let graphs = arrays[2]
                .as_any()
                .downcast_ref::<ListArray>()
                .ok_or_else(|| invalid("invalid Rust graph array"))?;
            let mut lengths = Vec::with_capacity(graphs.len());
            let mut keys = Vec::new();
            let mut identities = Vec::new();
            for row in 0..graphs.len() {
                if graphs.is_null(row) {
                    return Err(invalid("null Rust type graph"));
                }
                let values = graphs.value(row);
                let rows = values
                    .as_any()
                    .downcast_ref::<StructArray>()
                    .ok_or_else(|| invalid("invalid Rust graph records"))?;
                let (native_keys, nodes) = decode(rows)?;
                let normalized =
                    normalize::normalize(fixed(&arrays[0], row)?, fixed(&arrays[1], row)?, &nodes)
                        .map_err(|error| invalid(&error))?;
                lengths.push(normalized.len());
                keys.extend(native_keys);
                identities.extend(normalized);
            }
            let encoded = encode(&identities)?;
            let encoded = encoded
                .as_any()
                .downcast_ref::<StructArray>()
                .ok_or_else(|| invalid("invalid normalized identities"))?;
            let mut columns = vec![crate::fabric::hash32_array(keys.iter().map(Some))];
            columns.extend(encoded.columns()[1..].iter().cloned());
            let records =
                Arc::new(StructArray::try_new(output_fields(), columns, None)?) as ArrayRef;
            Ok(ColumnarValue::Array(Arc::new(ListArray::new(
                Arc::clone(&item),
                arrow::buffer::OffsetBuffer::from_lengths(lengths),
                records,
                None,
            ))))
        }),
    ))
}

fn key(values: &FixedSizeBinaryArray, row: usize) -> Result<[u8; 32], DataFusionError> {
    if values.is_null(row) {
        return Err(invalid("null native Rust type key"));
    }
    values
        .value(row)
        .try_into()
        .map_err(|_| invalid("invalid native Rust type key width"))
}

#[allow(
    clippy::too_many_lines,
    reason = "one typed decoder validates shared node fields and native component references"
)]
fn decode(rows: &StructArray) -> Result<(Vec<[u8; 32]>, Vec<Node<'_>>), DataFusionError> {
    if rows.len() > 1_000_000 {
        return Err(invalid("native Rust type graph exceeds its row bound"));
    }
    let keys = column::<FixedSizeBinaryArray>(rows, "type_key")?;
    let roles = column::<StringArray>(rows, "component_role")?;
    let ordinal = column::<UInt64Array>(rows, "component_ordinal")?;
    let kind = column::<StringArray>(rows, "type_kind")?;
    let primitive = column::<StringArray>(rows, "primitive_kind")?;
    let definition = column::<FixedSizeBinaryArray>(rows, "definition_entity_id")?;
    let generic_count = column::<UInt64Array>(rows, "generic_argument_count")?;
    let lengths = column::<UInt64Array>(rows, "array_length")?;
    let mutability = column::<StringArray>(rows, "mutability")?;
    let region = column::<StringArray>(rows, "region_kind")?;
    let binders = column::<UInt64Array>(rows, "bound_variable_count")?;
    let abi = column::<StringArray>(rows, "function_abi")?;
    let unwind = column::<BooleanArray>(rows, "function_abi_unwind")?;
    let safety = column::<BooleanArray>(rows, "function_unsafe")?;
    let variadic = column::<BooleanArray>(rows, "function_variadic")?;
    let targets = column::<FixedSizeBinaryArray>(rows, "component_type_key")?;
    let mut native_keys = Vec::new();
    let mut nodes = Vec::new();
    let mut indices = BTreeMap::new();
    for row in 0..rows.len() {
        if rows.is_null(row)
            || roles.is_null(row)
            || ordinal.is_null(row)
            || kind.is_null(row)
            || generic_count.is_null(row)
            || binders.is_null(row)
        {
            return Err(invalid("null required Rust type field"));
        }
        if roles.value(row) != "self" {
            continue;
        }
        if ordinal.value(row) != 0 || !targets.is_null(row) {
            return Err(invalid("invalid Rust type root row"));
        }
        let native_key = key(keys, row)?;
        let index = nodes.len() as u64;
        if indices.insert(native_key, index).is_some() {
            return Err(invalid("duplicate Rust type root row"));
        }
        native_keys.push(native_key);
        nodes.push(Node {
            index,
            kind: kind.value(row),
            primitive: (!primitive.is_null(row)).then(|| primitive.value(row)),
            definition: (!definition.is_null(row))
                .then(|| {
                    definition
                        .value(row)
                        .try_into()
                        .map_err(|_| invalid("invalid Rust nominal identity"))
                })
                .transpose()?,
            generic_argument_count: generic_count.value(row),
            array_length: (!lengths.is_null(row)).then(|| lengths.value(row)),
            mutability: (!mutability.is_null(row)).then(|| mutability.value(row)),
            region: (!region.is_null(row)).then(|| region.value(row)),
            bound_variable_count: binders.value(row),
            function_abi: (!abi.is_null(row)).then(|| abi.value(row)),
            function_abi_unwind: (!unwind.is_null(row)).then(|| unwind.value(row)),
            function_unsafe: (!safety.is_null(row)).then(|| safety.value(row)),
            function_variadic: (!variadic.is_null(row)).then(|| variadic.value(row)),
            components: vec![],
        });
    }
    for row in 0..rows.len() {
        if roles.value(row) == "self" {
            continue;
        }
        let owner = indices
            .get(&key(keys, row)?)
            .ok_or_else(|| invalid("Rust type component owner is absent"))?;
        let target = if targets.is_null(row) {
            None
        } else {
            indices.get(&key(targets, row)?).copied()
        };
        let owner = usize::try_from(*owner)
            .map_err(|_| invalid("Rust graph index exceeds the platform bound"))?;
        nodes[owner].components.push(Component {
            role: roles.value(row),
            ordinal: ordinal.value(row),
            target,
        });
    }
    Ok((native_keys, nodes))
}
