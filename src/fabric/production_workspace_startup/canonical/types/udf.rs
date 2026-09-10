//! One invocation per accepted graph. Arrow owns the grouped input and normalized output buffers.

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow_array::{
    Array, ArrayRef, BinaryArray, BooleanArray, FixedSizeBinaryArray, Int32Array, ListArray,
    StringArray, StructArray, UInt64Array,
};
use arrow_schema::{DataType, Field, Fields};
use datafusion::common::DataFusionError;
use datafusion::logical_expr::{ColumnarValue, ScalarUDF, Volatility, create_udf};

use super::super::{fixed, invalid};
use super::normalize::{self, Component, Node, NodeIdentity};
use crate::identity::CbefValue;

pub(super) fn output_fields() -> Fields {
    vec![
        Field::new("local_type_index", DataType::UInt64, false),
        Field::new("type_id", DataType::FixedSizeBinary(16), true),
        Field::new("type_kind_code", DataType::Int32, true),
        Field::new("canonical_key", DataType::Utf8, true),
        Field::new("unknown_reason", DataType::Utf8, true),
    ]
    .into()
}

pub(super) fn normalizer(nodes: DataType, edges: DataType) -> Arc<ScalarUDF> {
    let item = Arc::new(Field::new("item", DataType::Struct(output_fields()), false));
    Arc::new(create_udf(
        "codefabric_canonical_python_type_graph_v1",
        vec![
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(16),
            nodes,
            edges,
        ],
        DataType::List(Arc::clone(&item)),
        Volatility::Immutable,
        Arc::new(move |values| {
            let arrays = ColumnarValue::values_to_arrays(values)?;
            let nodes = arrays[2]
                .as_any()
                .downcast_ref::<ListArray>()
                .ok_or_else(|| invalid("expected native type nodes"))?;
            let edges = arrays[3]
                .as_any()
                .downcast_ref::<ListArray>()
                .ok_or_else(|| invalid("expected native type edges"))?;
            let mut lengths = Vec::with_capacity(nodes.len());
            let mut output = Vec::new();
            for row in 0..nodes.len() {
                if nodes.is_null(row) {
                    return Err(invalid("native type graph is null"));
                }
                let values = nodes.value(row);
                let values = values
                    .as_any()
                    .downcast_ref::<StructArray>()
                    .ok_or_else(|| invalid("expected typed native node records"))?;
                let components = (!edges.is_null(row)).then(|| edges.value(row));
                let components = components
                    .as_ref()
                    .map(|array| {
                        array
                            .as_any()
                            .downcast_ref::<StructArray>()
                            .ok_or_else(|| invalid("expected typed native edge records"))
                    })
                    .transpose()?;
                let decoded = decode(values, components)?;
                let normalized = normalize::normalize(
                    fixed(&arrays[0], row)?,
                    fixed(&arrays[1], row)?,
                    &decoded,
                )
                .map_err(|error| invalid(&error))?;
                lengths.push(normalized.len());
                output.extend(normalized);
            }
            let result = ListArray::new(
                Arc::clone(&item),
                arrow::buffer::OffsetBuffer::from_lengths(lengths),
                encode(&output)?,
                None,
            );
            Ok(ColumnarValue::Array(Arc::new(result)))
        }),
    ))
}

pub(super) fn column<'a, T: Array + 'static>(
    rows: &'a StructArray,
    name: &str,
) -> Result<&'a T, DataFusionError> {
    rows.column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref::<T>())
        .ok_or_else(|| invalid(&format!("invalid native type field {name}")))
}

fn decode<'a>(
    rows: &'a StructArray,
    edges: Option<&'a StructArray>,
) -> Result<Vec<Node<'a>>, DataFusionError> {
    if rows.len() > 1_000_000 || edges.is_some_and(|edges| edges.len() > 1_000_000) {
        return Err(invalid("native type graph exceeds its bound"));
    }
    let index = column::<UInt64Array>(rows, "local_type_index")?;
    let kind = column::<StringArray>(rows, "type_kind")?;
    let intrinsic = column::<StringArray>(rows, "intrinsic")?;
    let style = column::<StringArray>(rows, "style")?;
    let definitions = column::<FixedSizeBinaryArray>(rows, "definition_entity_id")?;
    let mut nodes = (0..rows.len())
        .map(|row| {
            if rows.is_null(row) || index.is_null(row) || kind.is_null(row) {
                return Err(invalid("null required native type field"));
            }
            Ok(Node {
                index: index.value(row),
                kind: kind.value(row),
                intrinsic: (!intrinsic.is_null(row)).then(|| intrinsic.value(row)),
                definition: (!definitions.is_null(row))
                    .then(|| {
                        definitions
                            .value(row)
                            .try_into()
                            .map_err(|_| invalid("invalid nominal definition identity"))
                    })
                    .transpose()?,
                style: (!style.is_null(row)).then(|| style.value(row)),
                literal: literal(rows, row)?,
                components: vec![],
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let indices = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.index, index))
        .collect::<BTreeMap<_, _>>();
    if let Some(edges) = edges {
        let owner = column::<UInt64Array>(edges, "owner_local_type_index")?;
        let target = column::<UInt64Array>(edges, "referenced_local_type_index")?;
        let role = column::<StringArray>(edges, "component_role")?;
        let ordinal = column::<UInt64Array>(edges, "component_ordinal")?;
        let kind = column::<StringArray>(edges, "parameter_kind")?;
        let name = column::<StringArray>(edges, "parameter_name")?;
        let required = column::<BooleanArray>(edges, "parameter_required")?;
        for row in 0..edges.len() {
            if edges.is_null(row) || owner.is_null(row) || role.is_null(row) || ordinal.is_null(row)
            {
                return Err(invalid("null required native type edge"));
            }
            let index = indices
                .get(&owner.value(row))
                .ok_or_else(|| invalid("native type edge owner is absent"))?;
            nodes[*index].components.push(Component {
                role: role.value(row),
                ordinal: ordinal.value(row),
                target: (!target.is_null(row)).then(|| target.value(row)),
                parameter_kind: (!kind.is_null(row)).then(|| kind.value(row)),
                parameter_name: (!name.is_null(row)).then(|| name.value(row)),
                parameter_required: (!required.is_null(row)).then(|| required.value(row)),
            });
        }
    }
    Ok(nodes)
}

fn literal(rows: &StructArray, row: usize) -> Result<Option<CbefValue>, DataFusionError> {
    let kind = column::<StringArray>(rows, "literal_kind")?;
    let text = column::<StringArray>(rows, "literal_text")?;
    let bytes = column::<BinaryArray>(rows, "literal_bytes")?;
    let boolean = column::<BooleanArray>(rows, "literal_boolean")?;
    if kind.is_null(row) {
        return if text.is_null(row) && bytes.is_null(row) && boolean.is_null(row) {
            Ok(None)
        } else {
            Err(invalid("native literal value has no discriminator"))
        };
    }
    let (variant, value) = match kind.value(row) {
        "string" if !text.is_null(row) && bytes.is_null(row) && boolean.is_null(row) => {
            (1, normalize::text(text.value(row)))
        }
        "bytes" if text.is_null(row) && !bytes.is_null(row) && boolean.is_null(row) => {
            (2, CbefValue::Bytes(bytes.value(row).to_vec()))
        }
        "integer" if !text.is_null(row) && bytes.is_null(row) && boolean.is_null(row) => {
            let value = text.value(row);
            let digits = value.strip_prefix('-').unwrap_or(value);
            if digits.is_empty()
                || !digits.bytes().all(|byte| byte.is_ascii_digit())
                || (digits.len() > 1 && digits.starts_with('0'))
                || value == "-0"
            {
                return Err(invalid("noncanonical native integer scalar"));
            }
            (3, normalize::text(value))
        }
        "boolean" if text.is_null(row) && bytes.is_null(row) && !boolean.is_null(row) => {
            (4, CbefValue::Boolean(boolean.value(row)))
        }
        _ => return Err(invalid("invalid native literal value columns")),
    };
    Ok(Some(CbefValue::TaggedUnion {
        variant,
        value: Box::new(value),
    }))
}

pub(super) fn encode(nodes: &[NodeIdentity]) -> Result<ArrayRef, DataFusionError> {
    let types = nodes
        .iter()
        .map(|node| node.identity.as_ref().map(|identity| identity.type_id))
        .collect::<Vec<_>>();
    let values = vec![
        Arc::new(UInt64Array::from_iter_values(
            nodes.iter().map(|node| node.index),
        )) as ArrayRef,
        crate::fabric::id16_array(types.iter().map(Option::as_ref)),
        Arc::new(Int32Array::from_iter(nodes.iter().map(|node| {
            node.identity
                .as_ref()
                .map(|identity| identity.type_kind_code)
        }))),
        Arc::new(StringArray::from_iter(nodes.iter().map(|node| {
            node.identity
                .as_ref()
                .map(|identity| identity.canonical_key.as_str())
        }))),
        Arc::new(StringArray::from_iter(
            nodes.iter().map(|node| node.unknown_reason),
        )),
    ];
    Ok(Arc::new(StructArray::try_new(
        output_fields(),
        values,
        None,
    )?))
}
