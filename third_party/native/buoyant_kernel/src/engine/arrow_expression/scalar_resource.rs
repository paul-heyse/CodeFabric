//! Admission for the exact scalar -> native Arrow builder path.
//!
//! Schema conversion and builder allocation are distinct phases. The input is
//! borrowed throughout preflight; no cloned kernel schema or serialized scalar
//! stands in for the allocations subsequently made by Arrow.
use crate::arrow::array::{self, ArrayBuilder};
use crate::arrow::datatypes::{DataType as ArrowType, Field as ArrowField, TimeUnit};
use crate::engine::arrow_conversion::TryIntoArrow;
use crate::expressions::{ArrayData, Scalar};
use crate::resource::{current_json_resource_limits, ResourceExhausted};
use crate::schema::{DataType, MetadataValue, PrimitiveType, StructField};
use crate::{DeltaResult, Error};
use std::mem::size_of;
use std::sync::Arc;

fn failure(kind: &'static str, requested: usize, limit: usize) -> Error {
    ResourceExhausted {
        kind,
        requested,
        limit,
    }
    .into()
}
fn add(a: usize, b: usize) -> DeltaResult<usize> {
    a.checked_add(b)
        .filter(|v| *v <= isize::MAX as usize)
        .ok_or_else(|| failure("native_scalar_layout", usize::MAX, isize::MAX as usize))
}
fn mul(a: usize, b: usize) -> DeltaResult<usize> {
    a.checked_mul(b)
        .filter(|v| *v <= isize::MAX as usize)
        .ok_or_else(|| failure("native_scalar_layout", usize::MAX, isize::MAX as usize))
}
fn sum(values: impl IntoIterator<Item = usize>) -> DeltaResult<usize> {
    values.into_iter().try_fold(0, add)
}
fn depth(depth: usize) -> DeltaResult<()> {
    let limit = current_json_resource_limits().map_or(128, |limits| limits.max_depth);
    if depth > limit {
        return Err(failure("native_scalar_depth", depth, limit));
    }
    Ok(())
}
fn admit(kind: &'static str, bytes: usize) -> DeltaResult<()> {
    use arrow_schema_59::resource::{current_resource_owner, ResourceAllocationRequest};
    if let Some(owner) = current_resource_owner() {
        owner
            .try_reserve_allocation(ResourceAllocationRequest {
                kind,
                bytes,
                alignment: 64,
            })
            .map_err(|error| failure(error.kind, error.requested, error.limit))?;
    } else if crate::resource::current_resource_scope().is_some() {
        return Err(failure("native_scalar_arrow_owner_unavailable", 1, 0));
    }
    Ok(())
}
fn unsupported() -> DeltaResult<usize> {
    let message = "Variant and interval scalars are not supported by the native Arrow builder";
    admit("native_scalar_diagnostic", message.len())?;
    Err(Error::unsupported(message))
}
// RawVec growth sums to less than four times the final/minimum requirement,
// including an old and full replacement layout. Initial constructors are paid
// separately, so the bound also covers a constructor larger than the final rows.
fn growing<T>(items: usize) -> DeltaResult<usize> {
    mul(mul(add(items, 8)?, 4)?, size_of::<T>())
}
fn bitmap(rows: usize) -> DeltaResult<usize> {
    mul(add(add(rows, 7)? / 8, 64)?, 4)
}
fn string_growth(bytes: usize) -> DeltaResult<usize> {
    mul(add(bytes, 128)?, 4)
}
fn arc<T>() -> DeltaResult<usize> {
    std::alloc::Layout::new::<(usize, usize)>()
        .extend(std::alloc::Layout::new::<T>())
        .map(|(layout, _)| layout.pad_to_align().size())
        .map_err(|_| failure("native_scalar_arc_layout", usize::MAX, isize::MAX as usize))
}
fn arc_str(bytes: usize) -> DeltaResult<usize> {
    let tail = std::alloc::Layout::array::<u8>(bytes)
        .map_err(|_| failure("native_scalar_arc_layout", bytes, isize::MAX as usize))?;
    std::alloc::Layout::new::<(usize, usize)>()
        .extend(tail)
        .map(|(layout, _)| layout.pad_to_align().size())
        .map_err(|_| failure("native_scalar_arc_layout", bytes, isize::MAX as usize))
}
fn hash_strings(entries: usize) -> DeltaResult<usize> {
    // hashbrown 7/8 load, power-of-two buckets, control/tail and alignment.
    sum([
        mul(mul(entries, 8)?, size_of::<(String, String)>() + 1)?,
        mul(entries, 48)?,
    ])
}
#[derive(Default)]
struct Count(usize);
impl std::io::Write for Count {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self.0.saturating_add(bytes.len());
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn metadata_json_bytes(value: &MetadataValue) -> DeltaResult<usize> {
    // Native MetadataValue serialization borrows all Value containers and uses
    // stack number formatting. Counting writes allocate no output or scratch.
    fn check(value: &serde_json::Value, level: usize) -> DeltaResult<()> {
        depth(level)?;
        match value {
            serde_json::Value::Array(values) => {
                for value in values {
                    check(value, level + 1)?;
                }
            }
            serde_json::Value::Object(values) => {
                for value in values.values() {
                    check(value, level + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    if let MetadataValue::Other(value) = value {
        check(value, 1)?;
    }
    let mut count = Count::default();
    serde_json::to_writer(&mut count, value).map_err(Error::from)?;
    add(count.0, 0)
}
fn field_metadata_bound(field: &StructField) -> DeltaResult<usize> {
    let mut bytes = hash_strings(field.metadata().len())?;
    for (key, value) in field.metadata() {
        // parquet.field.id changes to the longer PARQUET:field_id spelling.
        bytes = add(bytes, add(key.len(), "PARQUET:field_id".len())?)?;
        let value_bytes = match value {
            MetadataValue::String(s) => s.len(),
            _ => metadata_json_bytes(value)?,
        };
        bytes = add(bytes, string_growth(value_bytes)?)?;
        // A malformed nested-id diagnostic formats the original value and key.
        bytes = add(
            bytes,
            string_growth(sum([value_bytes, key.len(), field.name().len(), 160])?)?,
        )?;
    }
    Ok(bytes)
}
fn field_bound(field: &StructField, level: usize) -> DeltaResult<usize> {
    depth(level)?;
    sum([field.name().len(), field_metadata_bound(field)?, type_bound(field.data_type(), level + 1, field.name().len())?])
}
/// Admit metadata output from the original borrowed field without constructing
/// an unused Arrow DataType or copying the field's native schema.
pub(crate) fn prepare_field_metadata(field: &StructField) -> DeltaResult<()> {
    if governed() { admit("native_field_metadata", field_metadata_bound(field)?)?; }
    Ok(())
}
/// Native synthesized parquet field-id metadata contains at most one pair.
pub(crate) fn prepare_field_id_metadata(id: Option<i64>) -> DeltaResult<()> {
    if governed() && id.is_some() {
        admit("native_field_id_metadata", sum([hash_strings(1)?, "PARQUET:field_id".len(), string_growth(20)?])?)?;
    }
    Ok(())
}
fn fields_bound<'a>(
    fields: impl ExactSizeIterator<Item = &'a StructField>,
    level: usize,
) -> DeltaResult<usize> {
    let count = fields.len();
    // try_collect Vec<Field>, Vec<FieldRef>, Arc<[FieldRef]>, each Field Arc.
    let mut bytes = sum([
        growing::<ArrowField>(count)?,
        growing::<Arc<ArrowField>>(count)?,
        mul(count, arc::<ArrowField>()?)?,
        mul(count, size_of::<Arc<ArrowField>>())?,
        2 * size_of::<usize>(),
    ])?;
    for field in fields {
        bytes = add(bytes, field_bound(field, level)?)?;
    }
    Ok(bytes)
}
fn synthetic_field(name: &str, path: usize) -> DeltaResult<usize> {
    // Nested list/map field IDs: synthesized name/Field Arc, one metadata pair,
    // decimal i64 string and source path formatting (including error formatting).
    sum([
        name.len(),
        arc::<ArrowField>()?,
        hash_strings(1)?,
        "PARQUET:field_id".len(),
        20,
        string_growth(add(path, add(name.len(), 1)?)?)?,
        string_growth(add(path, 160)?)?,
    ])
}
fn type_bound(kind: &DataType, level: usize, path: usize) -> DeltaResult<usize> {
    depth(level)?;
    match kind {
        DataType::Primitive(PrimitiveType::Timestamp) => arc_str(3),
        #[cfg(feature = "nanosecond-timestamps")]
        DataType::Primitive(PrimitiveType::TimestampNanos) => arc_str(3),
        DataType::Primitive(PrimitiveType::IntervalYearMonth | PrimitiveType::IntervalDayTime)
        | DataType::Variant(_) => unsupported(),
        DataType::Primitive(_) => Ok(0),
        DataType::Struct(fields) => fields_bound(fields.fields(), level + 1),
        DataType::Array(array) => sum([
            synthetic_field("element", path)?,
            type_bound(array.element_type(), level + 1, add(path, 8)?)?,
        ]),
        DataType::Map(map) => sum([
            synthetic_field("key_value", path)?,
            synthetic_field("key", path)?,
            synthetic_field("value", path)?,
            growing::<ArrowField>(2)?,
            growing::<Arc<ArrowField>>(2)?,
            2 * size_of::<usize>(),
            type_bound(map.key_type(), level + 1, add(path, 4)?)?,
            type_bound(map.value_type(), level + 1, add(path, 6)?)?,
        ]),
    }
}
fn scalar_type_bound(value: &Scalar) -> DeltaResult<usize> {
    match value {
        Scalar::Struct(data) => fields_bound(data.fields().iter(), 1),
        Scalar::Array(data) => sum([
            synthetic_field("element", 0)?,
            type_bound(data.array_type().element_type(), 1, 0)?,
        ]),
        Scalar::Map(data) => sum([
            synthetic_field("key_value", 0)?,
            synthetic_field("key", 0)?,
            synthetic_field("value", 0)?,
            growing::<ArrowField>(2)?,
            growing::<Arc<ArrowField>>(2)?,
            2 * size_of::<usize>(),
            type_bound(data.map_type().key_type(), 1, 0)?,
            type_bound(data.map_type().value_type(), 1, 0)?,
        ]),
        Scalar::Null(kind) => type_bound(kind, 1, 0),
        _ => type_bound(&value.data_type(), 1, 0), // primitive data_type is allocation-free
    }
}
fn scalar_type(value: &Scalar) -> DeltaResult<ArrowType> {
    Ok(match value {
        Scalar::Struct(data) => ArrowType::Struct(
            data.fields()
                .iter()
                .map(TryIntoArrow::try_into_arrow)
                .collect::<Result<Vec<ArrowField>, _>>()?
                .into(),
        ),
        Scalar::Array(data) => ArrowType::List(Arc::new(data.array_type().try_into_arrow()?)),
        Scalar::Map(data) => ArrowType::Map(Arc::new(data.map_type().try_into_arrow()?), false),
        Scalar::Null(kind) => kind.try_into_arrow()?,
        _ => (&value.data_type()).try_into_arrow()?,
    })
}
fn primitive_constructor<B>(rows: usize, width: usize) -> DeltaResult<usize> {
    sum([size_of::<B>(), mul(rows, width)?, bitmap(rows)?])
}
fn constructor(kind: &ArrowType, rows: usize, level: usize) -> DeltaResult<usize> {
    depth(level)?;
    let bytes = match kind {
        ArrowType::Null => size_of::<array::NullBuilder>(),
        ArrowType::Int8 => primitive_constructor::<array::Int8Builder>(rows, 1)?,
        ArrowType::Int16 => primitive_constructor::<array::Int16Builder>(rows, 2)?,
        ArrowType::Int32 => primitive_constructor::<array::Int32Builder>(rows, 4)?,
        ArrowType::Int64 => primitive_constructor::<array::Int64Builder>(rows, 8)?,
        ArrowType::Float32 => primitive_constructor::<array::Float32Builder>(rows, 4)?,
        ArrowType::Float64 => primitive_constructor::<array::Float64Builder>(rows, 8)?,
        ArrowType::Date32 => primitive_constructor::<array::Date32Builder>(rows, 4)?,
        ArrowType::Timestamp(TimeUnit::Microsecond, _) => {
            primitive_constructor::<array::TimestampMicrosecondBuilder>(rows, 8)?
        }
        ArrowType::Timestamp(TimeUnit::Nanosecond, _) => {
            primitive_constructor::<array::TimestampNanosecondBuilder>(rows, 8)?
        }
        ArrowType::Decimal128(_, _) => primitive_constructor::<array::Decimal128Builder>(rows, 16)?,
        ArrowType::Boolean => sum([
            size_of::<array::BooleanBuilder>(),
            bitmap(rows)?,
            bitmap(rows)?,
        ])?,
        ArrowType::Utf8 => sum([
            size_of::<array::StringBuilder>(),
            mul(add(rows, 1)?, 4)?,
            1024,
            bitmap(rows)?,
        ])?,
        ArrowType::Binary => sum([
            size_of::<array::BinaryBuilder>(),
            mul(add(rows, 1)?, 4)?,
            1024,
            bitmap(rows)?,
        ])?,
        ArrowType::List(field) => sum([
            size_of::<array::ListBuilder<Box<dyn ArrayBuilder>>>(),
            mul(add(rows, 1)?, 4)?,
            bitmap(rows)?,
            constructor(field.data_type(), rows, level + 1)?,
        ])?,
        ArrowType::Struct(fields) => {
            let mut bytes = sum([
                size_of::<array::StructBuilder>(),
                mul(fields.len(), size_of::<Box<dyn ArrayBuilder>>())?,
                bitmap(rows)?,
            ])?;
            for field in fields {
                bytes = add(bytes, constructor(field.data_type(), rows, level + 1)?)?;
            }
            bytes
        }
        ArrowType::Map(field, _) => {
            let ArrowType::Struct(fields) = field.data_type() else {
                return Err(failure("native_scalar_map_shape", 1, 0));
            };
            if fields.len() != 2 {
                return Err(failure("native_scalar_map_shape", fields.len(), 2));
            }
            sum([
                size_of::<array::MapBuilder<Box<dyn ArrayBuilder>, Box<dyn ArrayBuilder>>>(),
                mul(add(rows, 1)?, 4)?,
                bitmap(rows)?,
                field.name().len(),
                fields[0].name().len(),
                fields[1].name().len(),
                constructor(fields[0].data_type(), rows, level + 1)?,
                constructor(fields[1].data_type(), rows, level + 1)?,
            ])?
        }
        _ => return unsupported(),
    };
    Ok(bytes)
}
fn primitive_append(width: usize, rows: usize) -> DeltaResult<usize> {
    sum([mul(mul(add(rows, 8)?, 4)?, width)?, bitmap(rows)?])
}
fn null_append(kind: &DataType, rows: usize, level: usize) -> DeltaResult<usize> {
    depth(level)?;
    match kind {
        DataType::Struct(fields) => {
            let mut bytes = bitmap(rows)?;
            for field in fields.fields() {
                bytes = add(bytes, null_append(field.data_type(), rows, level + 1)?)?;
            }
            Ok(bytes)
        }
        DataType::Array(_) | DataType::Map(_) => {
            sum([growing::<i32>(add(rows, 1)?)?, bitmap(rows)?])
        }
        DataType::Variant(_)
        | DataType::Primitive(PrimitiveType::IntervalYearMonth | PrimitiveType::IntervalDayTime) => {
            unsupported()
        }
        DataType::Primitive(PrimitiveType::Void) => Ok(0),
        DataType::Primitive(PrimitiveType::String | PrimitiveType::Binary) => {
            sum([growing::<i32>(add(rows, 1)?)?, bitmap(rows)?])
        }
        DataType::Primitive(PrimitiveType::Boolean) => add(bitmap(rows)?, bitmap(rows)?),
        DataType::Primitive(PrimitiveType::Byte) => primitive_append(1, rows),
        DataType::Primitive(PrimitiveType::Short) => primitive_append(2, rows),
        DataType::Primitive(
            PrimitiveType::Integer | PrimitiveType::Float | PrimitiveType::Date,
        ) => primitive_append(4, rows),
        DataType::Primitive(PrimitiveType::Decimal(_)) => primitive_append(16, rows),
        DataType::Primitive(_) => primitive_append(8, rows),
    }
}
fn append(value: &Scalar, rows: usize, level: usize) -> DeltaResult<usize> {
    depth(level)?;
    match value {
        Scalar::String(value) => sum([
            growing::<i32>(add(rows, 1)?)?,
            string_growth(mul(value.len(), rows)?)?,
            bitmap(rows)?,
        ]),
        Scalar::Binary(value) => sum([
            growing::<i32>(add(rows, 1)?)?,
            string_growth(mul(value.len(), rows)?)?,
            bitmap(rows)?,
        ]),
        Scalar::Boolean(_) => add(bitmap(rows)?, bitmap(rows)?),
        Scalar::Byte(_) => primitive_append(1, rows),
        Scalar::Short(_) => primitive_append(2, rows),
        Scalar::Integer(_) | Scalar::Float(_) | Scalar::Date(_) => primitive_append(4, rows),
        Scalar::Decimal(_) => primitive_append(16, rows),
        Scalar::Null(kind) => null_append(kind, rows, level + 1),
        Scalar::Struct(data) => {
            let mut bytes = bitmap(rows)?;
            for value in data.values() {
                bytes = add(bytes, append(value, rows, level + 1)?)?;
            }
            Ok(bytes)
        }
        Scalar::Array(data) => {
            let mut bytes = sum([growing::<i32>(add(rows, 1)?)?, bitmap(rows)?])?;
            for value in data.array_elements() {
                bytes = add(bytes, append(value, rows, level + 1)?)?;
            }
            Ok(bytes)
        }
        Scalar::Map(data) => {
            let mut bytes = sum([growing::<i32>(add(rows, 1)?)?, bitmap(rows)?])?;
            for (key, value) in data.pairs() {
                bytes = sum([
                    bytes,
                    append(key, rows, level + 1)?,
                    append(value, rows, level + 1)?,
                ])?;
            }
            Ok(bytes)
        }
        _ => primitive_append(8, rows),
    }
}

fn invalid(message: &'static str) -> DeltaResult<()> {
    admit("native_scalar_diagnostic", message.len())?;
    Err(Error::invalid_expression(message))
}
fn matches_type(value: &Scalar, kind: &DataType) -> bool {
    match (value, kind) {
        (Scalar::Null(actual), expected) => actual == expected,
        (Scalar::Struct(data), DataType::Struct(fields)) => {
            data.fields().len() == fields.num_fields()
                && data
                    .fields()
                    .iter()
                    .zip(fields.fields())
                    .all(|(left, right)| left == right)
        }
        (Scalar::Array(data), DataType::Array(kind)) => data.array_type() == kind.as_ref(),
        (Scalar::Map(data), DataType::Map(kind)) => data.map_type() == kind.as_ref(),
        (Scalar::Struct(_) | Scalar::Array(_) | Scalar::Map(_), _) => false,
        (primitive, expected) => &primitive.data_type() == expected,
    }
}
fn validate(value: &Scalar, level: usize) -> DeltaResult<()> {
    depth(level)?;
    match value {
        Scalar::Struct(data) => {
            if data.fields().len() != data.values().len() {
                return invalid("Native scalar struct has unequal field and value counts");
            }
            for (field, value) in data.fields().iter().zip(data.values()) {
                validate(value, level + 1)?;
                if !matches_type(value, field.data_type())
                    || (!field.is_nullable() && value.is_null())
                {
                    return invalid("Native scalar struct field has an incompatible value");
                }
            }
        }
        Scalar::Array(data) => {
            for value in data.array_elements() {
                validate(value, level + 1)?;
                if !matches_type(value, data.array_type().element_type())
                    || (!data.array_type().contains_null() && value.is_null())
                {
                    return invalid("Native scalar list has an incompatible value");
                }
            }
        }
        Scalar::Map(data) => {
            for (key, value) in data.pairs() {
                validate(key, level + 1)?;
                validate(value, level + 1)?;
                if key.is_null()
                    || !matches_type(key, data.map_type().key_type())
                    || !matches_type(value, data.map_type().value_type())
                    || (!data.map_type().value_contains_null() && value.is_null())
                {
                    return invalid("Native scalar map has an incompatible key or value");
                }
            }
        }
        Scalar::Null(kind) => {
            type_bound(kind, level + 1, 0)?;
        }
        _ => {}
    }
    Ok(())
}
fn null_values(kind: &DataType, rows: usize, level: usize) -> DeltaResult<usize> {
    depth(level)?;
    let mut values = rows;
    if let DataType::Struct(fields) = kind {
        for field in fields.fields() {
            values = add(values, null_values(field.data_type(), rows, level + 1)?)?;
        }
    }
    Ok(values)
}
fn counts(value: &Scalar, rows: usize, level: usize) -> DeltaResult<(usize, usize)> {
    depth(level)?;
    let mut values = rows;
    let mut bytes = 0;
    let mut child = |value, level| -> DeltaResult<()> {
        let (count, size) = counts(value, rows, level)?;
        values = add(values, count)?;
        bytes = add(bytes, size)?;
        Ok(())
    };
    match value {
        Scalar::Struct(data) => {
            for value in data.values() {
                child(value, level + 1)?;
            }
        }
        Scalar::Array(data) => {
            for value in data.array_elements() {
                child(value, level + 1)?;
            }
        }
        Scalar::Map(data) => {
            for (key, value) in data.pairs() {
                child(key, level + 1)?;
                child(value, level + 1)?;
            }
        }
        Scalar::String(value) => bytes = mul(value.len(), rows)?,
        Scalar::Binary(value) => bytes = mul(value.len(), rows)?,
        Scalar::Null(kind) => values = null_values(kind, rows, level + 1)?,
        _ => {}
    }
    Ok((values, bytes))
}
fn check_counts(values: usize, bytes: usize) -> DeltaResult<()> {
    // This selected builder profile uses i32 list/map/string/binary offsets.
    // A finite aggregate cap also bounds every individual descendant offset.
    for (kind, count) in [
        ("native_scalar_values", values),
        ("native_scalar_value_bytes", bytes),
    ] {
        if count > i32::MAX as usize {
            return Err(failure(kind, count, i32::MAX as usize));
        }
    }
    Ok(())
}
fn governed() -> bool {
    crate::resource::current_resource_scope().is_some()
        || arrow_schema_59::resource::current_resource_owner().is_some()
}

/// Convert one borrowed kernel type after admitting its actual Arrow schema tree.
pub(crate) fn prepare_type(kind: &DataType) -> DeltaResult<ArrowType> {
    if governed() { admit("native_scalar_schema", type_bound(kind, 1, 0)?)?; }
    Ok(kind.try_into_arrow()?)
}

/// The null-row consumer borrows the original type instead of creating an
/// unadmitted deep Scalar::Null(DataType::clone()) intermediary.
pub(super) fn prepare_null(kind: &DataType, rows: usize) -> DeltaResult<ArrowType> {
    if !governed() {
        return Ok(kind.try_into_arrow()?);
    }
    let schema = type_bound(kind, 1, 0)?;
    check_counts(null_values(kind, rows, 1)?, 0)?;
    admit("native_scalar_schema", schema)?;
    let arrow = kind.try_into_arrow()?;
    admit(
        "native_scalar_builder",
        add(constructor(&arrow, rows, 1)?, null_append(kind, rows, 1)?)?,
    )?;
    Ok(arrow)
}

/// Produce the actual Arrow schema from a borrowed kernel schema. The charge
/// includes the caller's immediately constructed Schema Arc, not only its fields.
pub(crate) fn prepare_schema(
    schema: &crate::schema::StructType,
) -> DeltaResult<crate::arrow::datatypes::Schema> {
    if governed() {
        admit(
            "native_scalar_schema",
            add(
                fields_bound(schema.fields(), 1)?,
                arc::<crate::arrow::datatypes::Schema>()?,
            )?,
        )?;
    }
    Ok(schema.try_into_arrow()?)
}

/// Admit a selected builder using an already-converted original Arrow type.
pub(super) fn prepare_constructor(kind: &ArrowType, rows: usize) -> DeltaResult<()> {
    if governed() { admit("native_scalar_builder", constructor(kind, rows, 1)?)?; }
    Ok(())
}

/// Validate and admit all native create_many appends from the borrowed rows.
/// One cumulative request avoids a receipt per scalar while still reserving
/// every old/full replacement layout before the first append begins.
pub(super) fn prepare_rows(schema: &crate::schema::StructType, rows: &[&[Scalar]]) -> DeltaResult<()> {
    if !governed() { return Ok(()); }
    let mut values = 0;
    let mut bytes = 0;
    let mut backing = 0;
    for row in rows {
        if row.len() != schema.num_fields() { return invalid("Native scalar row has an incompatible field count"); }
        for (value, field) in row.iter().zip(schema.fields()) {
            validate(value, 1)?;
            if !matches_type(value, field.data_type()) || (!field.is_nullable() && value.is_null()) {
                return invalid("Native scalar row has an incompatible field value");
            }
            let (count, size) = counts(value, 1, 1)?;
            values = add(values, count)?; bytes = add(bytes, size)?;
            backing = add(backing, append(value, 1, 1)?)?;
        }
    }
    check_counts(values, bytes)?;
    admit("native_scalar_append", backing)
}

/// Reserve the next borrowed value before appending it to an admitted builder.
/// The bank retains every prior append receipt. Summed geometric-growth bounds
/// therefore cover every prefix, including full replacements of prior backing.
/// The caller must additionally track cumulative i32 offsets across all appends.
pub(crate) fn prepare_append(value: &Scalar, rows: usize) -> DeltaResult<()> {
    if !governed() { return Ok(()); }
    validate(value, 1)?;
    let (values, bytes) = counts(value, rows, 1)?;
    check_counts(values, bytes)?;
    admit("native_scalar_append", append(value, rows, 1)?)
}

pub(super) fn prepare_scalar(value: &Scalar, rows: usize) -> DeltaResult<ArrowType> {
    if !governed() {
        return scalar_type(value);
    }
    let schema = scalar_type_bound(value)?;
    validate(value, 1)?;
    let (values, bytes) = counts(value, rows, 1)?;
    check_counts(values, bytes)?;
    admit("native_scalar_schema", schema)?;
    let kind = scalar_type(value)?;
    admit(
        "native_scalar_builder",
        add(constructor(&kind, rows, 1)?, append(value, rows, 1)?)?,
    )?;
    Ok(kind)
}
pub(super) fn prepare_array(data: &ArrayData) -> DeltaResult<ArrowType> {
    if !governed() {
        return Ok(data.array_type().element_type().try_into_arrow()?);
    }
    let schema = type_bound(data.array_type().element_type(), 1, 0)?;
    let mut values = 0;
    let mut value_bytes = 0;
    for value in data.array_elements() {
        validate(value, 1)?;
        if !matches_type(value, data.array_type().element_type())
            || (!data.array_type().contains_null() && value.is_null())
        {
            invalid("Native scalar array has an incompatible value")?;
        }
        let (count, bytes) = counts(value, 1, 1)?;
        values = add(values, count)?;
        value_bytes = add(value_bytes, bytes)?;
    }
    check_counts(values, value_bytes)?;
    admit("native_scalar_schema", schema)?;
    let kind = data.array_type().element_type().try_into_arrow()?;
    let mut bytes = constructor(&kind, data.array_elements().len(), 1)?;
    for value in data.array_elements() {
        bytes = add(bytes, append(value, 1, 1)?)?;
    }
    admit("native_scalar_builder", bytes)?;
    Ok(kind)
}

pub(super) fn retain_fresh_buffers(array: &dyn array::Array) -> DeltaResult<()> {
    if arrow_schema_59::resource::current_resource_owner().is_none() {
        return Ok(());
    }
    // Visit the existing native buffer graph; do not materialize ArrayData just
    // to find payloads whose ownership must survive a bare Buffer clone.
    array.try_visit_buffers(&mut |buffer| arrow_data_59::try_retain_fresh_buffer(buffer))?;
    Ok(())
}
