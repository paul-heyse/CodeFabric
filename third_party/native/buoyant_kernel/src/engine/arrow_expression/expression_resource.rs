//! Native evaluator descriptor admission. Payload production remains owned by
//! the selected scalar decoder or fallible Arrow compute operation.
use crate::arrow::array::{Array, ArrayData, ArrayRef};
use crate::arrow::datatypes::DataType;
use crate::arrow::error::ArrowError;
use arrow_schema_59::resource::{
    ResourceAllocationRequest, ResourceOwnerError, current_resource_owner,
};
use std::alloc::Layout;

fn error(kind: &'static str, requested: usize, limit: usize) -> ArrowError {
    let error = ResourceOwnerError {
        kind,
        requested,
        limit,
    };
    if let Some(owner) = current_resource_owner() {
        owner.record_failure(error);
    }
    if let Some(scope) = crate::resource::current_resource_scope() {
        scope.record_failure(crate::resource::ResourceExhausted {
            kind,
            requested,
            limit,
        });
    }
    error.into()
}
pub(crate) fn admit(layout: Layout, kind: &'static str) -> Result<(), ArrowError> {
    if layout.size() == 0 {
        return Ok(());
    }
    if let Some(owner) = current_resource_owner() {
        owner
            .try_reserve_allocation(ResourceAllocationRequest {
                bytes: layout.size(),
                alignment: layout.align(),
                kind,
            })
            .map_err(|denied| error(denied.kind, denied.requested, denied.limit))
    } else if crate::resource::current_resource_scope().is_some() {
        Err(error("native_evaluator_arrow_owner", 1, 0))
    } else {
        Ok(())
    }
}
pub(crate) fn new_vec<T>(count: usize, kind: &'static str) -> Result<Vec<T>, ArrowError> {
    let layout = Layout::array::<T>(count).map_err(|_| error(kind, count, isize::MAX as usize))?;
    admit(layout, kind)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| error(kind, layout.size(), 0))?;
    Ok(values)
}
pub(crate) fn arc<T>(kind: &'static str) -> Result<(), ArrowError> {
    let layout = Layout::new::<[usize; 2]>()
        .extend(Layout::new::<T>())
        .map_err(|_| error(kind, usize::MAX, isize::MAX as usize))?
        .0
        .pad_to_align();
    admit(layout, kind)
}
pub(crate) fn boxed<T>(kind: &'static str) -> Result<(), ArrowError> {
    admit(Layout::new::<T>(), kind)
}
pub(crate) fn clone_type(kind: &DataType) -> Result<DataType, ArrowError> {
    fn preflight(kind: &DataType) -> Result<(), ArrowError> {
        if let DataType::Dictionary(key, value) = kind {
            boxed::<DataType>("native_expression_dictionary_key_type")?;
            preflight(key)?;
            boxed::<DataType>("native_expression_dictionary_value_type")?;
            preflight(value)?;
        }
        Ok(())
    }
    preflight(kind)?;
    Ok(kind.clone())
}
pub(crate) fn array_data(arrays: &[ArrayRef]) -> Result<Vec<ArrayData>, ArrowError> {
    let mut data = new_vec(arrays.len(), "native_expression_ArrayData_descriptors")?;
    for array in arrays {
        data.push(array.try_to_data()?);
    }
    Ok(data)
}
pub(crate) fn data_refs(data: &[ArrayData]) -> Result<Vec<&ArrayData>, ArrowError> {
    let mut refs = new_vec(data.len(), "native_expression_borrowed_descriptors")?;
    refs.extend(data.iter());
    Ok(refs)
}
pub(crate) fn check_evaluator_owner(
    owners: &crate::engine::arrow_data::NativeDataOwners,
) -> crate::DeltaResult<()> {
    if owners.is_governed()
        && (crate::resource::current_resource_scope().is_none()
            || current_resource_owner().is_none())
    {
        return Err(error("native_evaluator_required_scope", 1, 0).into());
    }
    if let Some(scope) = owners.native_scope() {
        scope.check_available()?;
    }
    Ok(())
}

pub(crate) fn check_arrow_owner(
    original: &arrow_schema_59::resource::ResourceOwnerHandle,
) -> crate::DeltaResult<()> {
    if !original.is_empty() && current_resource_owner().is_none() {
        return Err(error("native_evaluator_arrow_scope", 1, 0).into());
    }
    if let (Some(original), Some(current)) = (original.owner(), current_resource_owner()) {
        if !std::sync::Arc::ptr_eq(original, &current) {
            current
                .try_adopt(original.clone())
                .map_err(|denied| error(denied.kind, denied.requested, denied.limit))?;
        }
    }
    Ok(())
}

pub(crate) fn output_schema(
    kind: DataType,
) -> Result<std::sync::Arc<crate::arrow::datatypes::Schema>, ArrowError> {
    use crate::arrow::datatypes::{Field, FieldRef, Schema};
    let name = "output";
    admit(
        Layout::array::<u8>(name.len()).expect("static field name"),
        "native_evaluator_output_name",
    )?;
    arc::<Field>("native_evaluator_output_field")?;
    let mut fields = new_vec::<FieldRef>(1, "native_evaluator_output_fields")?;
    let slice = Layout::new::<[usize; 2]>()
        .extend(Layout::array::<FieldRef>(1).expect("one field"))
        .expect("bounded field slice")
        .0
        .pad_to_align();
    admit(slice, "native_evaluator_output_Fields")?;
    arc::<Schema>("native_evaluator_output_schema")?;
    fields.push(std::sync::Arc::new(Field::new(name, kind, true)));
    Ok(std::sync::Arc::new(Schema::new(fields)))
}

pub(crate) fn field(
    name: &str,
    kind: &DataType,
    nullable: bool,
    metadata: Option<std::collections::HashMap<String, String>>,
) -> Result<std::sync::Arc<crate::arrow::datatypes::Field>, ArrowError> {
    use crate::arrow::datatypes::Field;
    let layout = Layout::array::<u8>(name.len()).map_err(|_| {
        error(
            "native_transformed_field_name",
            name.len(),
            isize::MAX as usize,
        )
    })?;
    admit(layout, "native_transformed_field_name")?;
    let kind = clone_type(kind)?;
    arc::<Field>("native_transformed_field_owner")?;
    let mut field = Field::new(name, kind, nullable);
    if let Some(metadata) = metadata {
        field.set_metadata(metadata);
    }
    Ok(std::sync::Arc::new(field))
}
pub(crate) fn fields(
    values: Vec<crate::arrow::datatypes::FieldRef>,
) -> Result<crate::arrow::datatypes::Fields, ArrowError> {
    let tail = Layout::array::<crate::arrow::datatypes::FieldRef>(values.len()).map_err(|_| {
        error(
            "native_transformed_Fields",
            values.len(),
            isize::MAX as usize,
        )
    })?;
    let layout = Layout::new::<[usize; 2]>()
        .extend(tail)
        .map_err(|_| {
            error(
                "native_transformed_Fields",
                tail.size(),
                isize::MAX as usize,
            )
        })?
        .0
        .pad_to_align();
    admit(layout, "native_transformed_Fields")?;
    Ok(values.into())
}
pub(crate) fn path(parent: &str, child: &str) -> Result<String, ArrowError> {
    let len = parent
        .len()
        .checked_add(1)
        .and_then(|n| n.checked_add(child.len()))
        .ok_or_else(|| error("native_nested_field_path", usize::MAX, isize::MAX as usize))?;
    let bytes = new_vec::<u8>(len, "native_nested_field_path")?;
    let mut output = String::from_utf8(bytes).expect("empty UTF-8 vector");
    output.push_str(parent);
    output.push('.');
    output.push_str(child);
    Ok(output)
}

/// Count only native fmt implementations used by the selected diagnostic sites, then
/// allocate the complete string once. The arguments are borrowed in both passes.
pub(crate) fn format(arguments: std::fmt::Arguments<'_>) -> Result<String, ArrowError> {
    use std::fmt::Write;
    struct Count(usize);
    impl std::fmt::Write for Count {
        fn write_str(&mut self, value: &str) -> std::fmt::Result {
            self.0 = self.0.checked_add(value.len()).ok_or(std::fmt::Error)?;
            Ok(())
        }
    }
    let mut count = Count(0);
    std::fmt::write(&mut count, arguments).map_err(|_| {
        error(
            "native_expression_diagnostic",
            usize::MAX,
            isize::MAX as usize,
        )
    })?;
    let bytes = new_vec::<u8>(count.0, "native_expression_diagnostic")?;
    let mut output = String::from_utf8(bytes).expect("empty UTF-8 vector");
    output
        .write_fmt(arguments)
        .map_err(|_| error("native_expression_diagnostic", count.0, 0))?;
    Ok(output)
}

/// Source bound for Arrow 59.2 BooleanBuffer binary operations: the aligned
/// branch collects u64 words into Vec (minimum nonzero capacity four), the
/// unaligned branch allocates rounded MutableBuffer then may replace it for a
/// trailing partial word, and buffer_bin_* may normalize a nonzero bit offset.
/// Admit the sum of these mutually exclusive branch layouts before dispatch;
/// this is a finite upper bound, not an assertion that every buffer is created.
pub(crate) fn bitmap_binary(len: usize) -> Result<(), ArrowError> {
    use arrow_buffer_59::MutableBuffer;
    let expanded = len
        .checked_add(63)
        .ok_or_else(|| error("native_bitmap_geometry", usize::MAX, isize::MAX as usize))?;
    let words = expanded
        .checked_add(63)
        .ok_or_else(|| error("native_bitmap_geometry", usize::MAX, isize::MAX as usize))?
        / 64;
    let vec_layout = Layout::array::<u64>(words.max(4))
        .map_err(|_| error("native_bitmap_geometry", len, isize::MAX as usize))?;
    admit(vec_layout, "native_bitmap_aligned_words")?;
    let whole_bytes = (len / 64)
        .checked_mul(8)
        .ok_or_else(|| error("native_bitmap_geometry", len, isize::MAX as usize))?;
    let initial = MutableBuffer::try_capacity_layout(whole_bytes)
        .map_err(|_| error("native_bitmap_geometry", len, isize::MAX as usize))?;
    admit(initial, "native_bitmap_unaligned_words")?;
    if len % 64 != 0 {
        let required = len
            .checked_add(7)
            .ok_or_else(|| error("native_bitmap_geometry", len, isize::MAX as usize))?
            / 8;
        if required > initial.size() {
            let replacement =
                MutableBuffer::try_capacity_layout(
                    required.max(initial.size().checked_mul(2).ok_or_else(|| {
                        error("native_bitmap_geometry", len, isize::MAX as usize)
                    })?),
                )
                .map_err(|_| error("native_bitmap_geometry", len, isize::MAX as usize))?;
            admit(replacement, "native_bitmap_unaligned_replacement")?;
        }
    }
    let normalized = MutableBuffer::try_capacity_layout(
        len.checked_add(7)
            .ok_or_else(|| error("native_bitmap_geometry", len, isize::MAX as usize))?
            / 8,
    )
    .map_err(|_| error("native_bitmap_geometry", len, isize::MAX as usize))?;
    admit(normalized, "native_bitmap_normalized_payload")?;
    admit(
        arrow_buffer_59::allocation_owner_layout(),
        "native_bitmap_owner",
    )?;
    admit(
        arrow_buffer_59::allocation_owner_layout(),
        "native_bitmap_normalized_owner",
    )?;
    Ok(())
}

pub(crate) fn null_bitmap(len: usize) -> Result<crate::arrow::buffer::NullBuffer, ArrowError> {
    let bytes = len
        .checked_add(7)
        .ok_or_else(|| error("native_null_bitmap", len, isize::MAX as usize))?
        / 8;
    let layout = arrow_buffer_59::MutableBuffer::try_capacity_layout(bytes)
        .map_err(|_| error("native_null_bitmap", len, isize::MAX as usize))?;
    admit(layout, "native_null_bitmap")?;
    admit(
        arrow_buffer_59::allocation_owner_layout(),
        "native_null_bitmap_owner",
    )?;
    let buffer = crate::arrow::buffer::NullBuffer::new_null(len);
    arrow_data_59::try_retain_fresh_buffer(buffer.inner().inner())?;
    Ok(buffer)
}

pub(crate) fn hash_map<K: std::hash::Hash + Eq, V>(
    count: usize,
    kind: &'static str,
) -> Result<std::collections::HashMap<K, V>, ArrowError> {
    prepare_hash_map(count, kind)?.allocate()
}

/// Admission for a map whose geometry is already known, before other native
/// constructors run. Construction consumes this ticket under the same owner.
pub(crate) struct HashMapAllocation<K, V> {
    count: usize,
    bytes: usize,
    kind: &'static str,
    owner: arrow_schema_59::resource::ResourceOwnerHandle,
    types: std::marker::PhantomData<fn() -> (K, V)>,
}

impl<K: std::hash::Hash + Eq, V> HashMapAllocation<K, V> {
    pub(crate) fn allocate(self) -> Result<std::collections::HashMap<K, V>, ArrowError> {
        if !self
            .owner
            .same_owner(&arrow_schema_59::resource::ResourceOwnerHandle::capture())
        {
            return Err(error("native_hash_map_allocation_owner", 1, 0));
        }
        let mut map = std::collections::HashMap::new();
        map.try_reserve(self.count)
            .map_err(|_| error(self.kind, self.bytes, 0))?;
        Ok(map)
    }
}

pub(crate) fn prepare_hash_map<K: std::hash::Hash + Eq, V>(
    count: usize,
    kind: &'static str,
) -> Result<HashMapAllocation<K, V>, ArrowError> {
    // std/hashbrown's 7/8 table load, next-power-of-two bucket count, control
    // tail and alignment are dominated by eight buckets and 48 tail bytes per
    // requested entry. The zero-entry table owns no allocation.
    let bytes = count
        .checked_mul(8)
        .and_then(|n| n.checked_mul(std::mem::size_of::<(K, V)>() + 1))
        .and_then(|n| count.checked_mul(48).and_then(|tail| n.checked_add(tail)))
        .ok_or_else(|| error(kind, count, isize::MAX as usize))?;
    admit(
        Layout::from_size_align(bytes, std::mem::align_of::<(K, V)>())
            .map_err(|_| error(kind, bytes, isize::MAX as usize))?,
        kind,
    )?;
    Ok(HashMapAllocation {
        count,
        bytes,
        kind,
        owner: arrow_schema_59::resource::ResourceOwnerHandle::capture(),
        types: std::marker::PhantomData,
    })
}

pub(crate) fn partition_parse(raw: &str) -> Result<(), ArrowError> {
    // Native String/Binary copies the borrowed bytes once. Decimal concatenates
    // at most two substrings and may format a 40-character i128 diagnostic.
    // Arrow timestamp errors include the input plus fixed text (the longest
    // context is 44 bytes); timezone errors include only a borrowed suffix.
    // Four times (bytes + 128) dominates each native String growth series and
    // the subsequent kernel raw-input error copy, all retained cumulatively.
    let bytes = raw
        .len()
        .checked_add(128)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| error("native_partition_parse", raw.len(), isize::MAX as usize))?;
    admit(
        Layout::array::<u8>(bytes)
            .map_err(|_| error("native_partition_parse", bytes, isize::MAX as usize))?,
        "native_partition_parse",
    )
}

pub(crate) fn empty_column(kind: &DataType) -> crate::DeltaResult<ArrayRef> {
    super::scalar_resource::prepare_constructor(kind, 0)?;
    let column = crate::arrow::array::make_builder(kind, 0).try_finish()?;
    super::scalar_resource::retain_fresh_buffers(column.as_ref())?;
    Ok(column)
}

pub(crate) fn null_builder(
    len: usize,
) -> Result<crate::arrow::array::NullBufferBuilder, ArrowError> {
    let bytes = len
        .checked_add(7)
        .ok_or_else(|| error("native_output_validity", len, isize::MAX as usize))?
        / 8;
    let layout = arrow_buffer_59::MutableBuffer::try_capacity_layout(bytes)
        .map_err(|_| error("native_output_validity", bytes, isize::MAX as usize))?;
    admit(layout, "native_output_validity")?;
    admit(
        arrow_buffer_59::allocation_owner_layout(),
        "native_output_validity_owner",
    )?;
    Ok(crate::arrow::array::NullBufferBuilder::new(len))
}

pub(crate) fn boolean_values(
    values: &[bool],
) -> Result<crate::arrow::array::BooleanArray, ArrowError> {
    let bytes = values
        .len()
        .checked_add(7)
        .ok_or_else(|| error("native_selection_bits", values.len(), isize::MAX as usize))?
        / 8;
    let layout = arrow_buffer_59::MutableBuffer::try_capacity_layout(bytes)
        .map_err(|_| error("native_selection_bits", bytes, isize::MAX as usize))?;
    admit(layout, "native_selection_bits")?;
    admit(
        arrow_buffer_59::allocation_owner_layout(),
        "native_selection_bits_owner",
    )?;
    Ok(crate::arrow::array::BooleanArray::new(values.into(), None))
}

/// Exact trusted_len_unzip output path used by selected safe UTF8 casts.
fn trusted_primitive<T: crate::arrow::array::types::ArrowPrimitiveType>(
    rows: usize,
) -> Result<(), ArrowError> {
    let payload = Layout::array::<T::Native>(rows)
        .map_err(|_| error("native_stats_cast_values", rows, isize::MAX as usize))?;
    for (bytes, kind) in [
        (payload.size(), "native_stats_cast_values"),
        (
            rows.checked_add(7)
                .ok_or_else(|| error("native_stats_cast_nulls", rows, isize::MAX as usize))?
                / 8,
            "native_stats_cast_nulls",
        ),
    ] {
        let layout = arrow_buffer_59::MutableBuffer::try_capacity_layout(bytes)
            .map_err(|_| error(kind, bytes, isize::MAX as usize))?;
        admit(layout, kind)?;
        admit(
            arrow_buffer_59::allocation_owner_layout(),
            "native_stats_cast_buffer_owner",
        )?;
    }
    arc::<crate::arrow::array::PrimitiveArray<T>>("native_stats_cast_array_owner")
}

/// Preflight the existing Arrow safe cast selected by stats schema relaxation.
/// The native parser and rounding semantics remain cast_with_options' authority.
pub(crate) fn stats_cast(
    array: &dyn crate::arrow::array::Array,
    target: &DataType,
) -> Result<(), ArrowError> {
    use crate::arrow::array::{cast::AsArray, types::*};
    if current_resource_owner().is_none() && crate::resource::current_resource_scope().is_none() {
        return Ok(());
    }
    let source = array
        .as_string_opt::<i32>()
        .ok_or_else(|| error("native_stats_cast_source", 1, 0))?;
    match target {
        DataType::Date32 => {
            trusted_primitive::<Date32Type>(array.len())?;
            // Date32's native Parser falls back to string_to_datetime for
            // timestamp-shaped inputs, including its allocated diagnostics.
            for value in source.iter().flatten() {
                partition_parse(value)?;
            }
        }
        DataType::Timestamp(unit, zone) => {
            match unit {
                crate::arrow::datatypes::TimeUnit::Second => {
                    trusted_primitive::<TimestampSecondType>(array.len())?
                }
                crate::arrow::datatypes::TimeUnit::Millisecond => {
                    trusted_primitive::<TimestampMillisecondType>(array.len())?
                }
                crate::arrow::datatypes::TimeUnit::Microsecond => {
                    trusted_primitive::<TimestampMicrosecondType>(array.len())?
                }
                crate::arrow::datatypes::TimeUnit::Nanosecond => {
                    trusted_primitive::<TimestampNanosecondType>(array.len())?
                }
            }
            if let Some(zone) = zone {
                partition_parse(zone)?;
            }
            for value in source.iter().flatten() {
                partition_parse(value)?;
            }
        }
        DataType::Decimal128(_, scale) => {
            trusted_primitive::<Decimal128Type>(array.len())?;
            // cast_string_to_decimal validates precision/scale even for an empty
            // array. Its fixed diagnostics cannot contain arbitrary schema data.
            admit(
                Layout::array::<u8>(512).expect("fixed native diagnostic"),
                "native_stats_decimal_type_diagnostic",
            )?;
            for value in source.iter().flatten() {
                // The native parser collects split('.') before validating. Its
                // Vec grows from at least four &str slots by doubling. The sum
                // of complete replacement capacities is less than four times
                // the larger of four slots and the exact borrowed segment count.
                let segments = value
                    .as_bytes()
                    .iter()
                    .filter(|byte| **byte == b'.')
                    .count()
                    .checked_add(1)
                    .ok_or_else(|| {
                        error(
                            "native_stats_decimal_segments",
                            value.len(),
                            isize::MAX as usize,
                        )
                    })?;
                let slots = segments.max(4).checked_mul(4).ok_or_else(|| {
                    error(
                        "native_stats_decimal_segments",
                        segments,
                        isize::MAX as usize,
                    )
                })?;
                admit(
                    Layout::array::<&str>(slots).map_err(|_| {
                        error("native_stats_decimal_segments", slots, isize::MAX as usize)
                    })?,
                    "native_stats_decimal_segments",
                )?;
                // At most four native String series: padded decimals, combined
                // integer/decimal (including optional sign insertion), i256
                // formatting scratch, and one diagnostic. Debug escaping has
                // at most ten bytes per input byte; native scale is at most i8.
                let maximum = value
                    .len()
                    .checked_mul(10)
                    .and_then(|n| n.checked_add(usize::from(scale.unsigned_abs()) + 128))
                    .ok_or_else(|| {
                        error(
                            "native_stats_decimal_scratch",
                            value.len(),
                            isize::MAX as usize,
                        )
                    })?;
                let bytes = maximum
                    .checked_mul(4)
                    .and_then(|n| n.checked_mul(4))
                    .ok_or_else(|| {
                        error("native_stats_decimal_scratch", maximum, isize::MAX as usize)
                    })?;
                admit(
                    Layout::array::<u8>(bytes).map_err(|_| {
                        error("native_stats_decimal_scratch", bytes, isize::MAX as usize)
                    })?,
                    "native_stats_decimal_scratch",
                )?;
            }
        }
        _ => return Err(error("native_stats_cast_target", 1, 0)),
    }
    Ok(())
}

fn display_size(value: impl std::fmt::Display) -> Result<usize, ArrowError> {
    use std::fmt::Write;
    struct Count(usize);
    impl std::fmt::Write for Count {
        fn write_str(&mut self, value: &str) -> std::fmt::Result {
            self.0 = self.0.checked_add(value.len()).ok_or(std::fmt::Error)?;
            Ok(())
        }
    }
    let mut count = Count(0);
    write!(&mut count, "{value}")
        .map_err(|_| error("native_compute_diagnostic", usize::MAX, isize::MAX as usize))?;
    Ok(count.0)
}

/// Selected array/array arithmetic branches of arrow-arith 59.2 numeric.rs and
/// arity.rs. This bounds the original algorithms, including their empty-array
/// descriptor construction, without changing checked arithmetic or decimals.
pub(crate) fn arithmetic(
    left: &dyn crate::arrow::array::Array,
    right: &dyn crate::arrow::array::Array,
) -> Result<(), ArrowError> {
    use crate::arrow::array::{PrimitiveArray, types::Decimal256Type};
    if current_resource_owner().is_none() && crate::resource::current_resource_scope().is_none() {
        return Ok(());
    }
    let rows = left.len();
    // Every native unsupported-operation diagnostic prints only the two borrowed
    // types. Known primitive checked operations add at most two signed i256s
    // (79 bytes each), plus fixed context shorter than128 bytes. The integer
    // formatting and output each have one geometric String series. Timestamp
    // interval branches eagerly allocate their21-byte ok_or message per row.
    let type_bytes = display_size(left.data_type())?
        .checked_add(display_size(right.data_type())?)
        .ok_or_else(|| {
            error(
                "native_arithmetic_diagnostic",
                usize::MAX,
                isize::MAX as usize,
            )
        })?;
    let diagnostic = type_bytes
        .checked_add(2 * 79 + 128)
        .and_then(|n| n.checked_mul(4 * 4))
        .and_then(|n| rows.checked_mul(21).and_then(|eager| n.checked_add(eager)))
        .ok_or_else(|| error("native_arithmetic_diagnostic", rows, isize::MAX as usize))?;
    admit(
        Layout::array::<u8>(diagnostic).map_err(|_| {
            error(
                "native_arithmetic_diagnostic",
                diagnostic,
                isize::MAX as usize,
            )
        })?,
        "native_arithmetic_diagnostic",
    )?;
    let (Some(left_width), Some(right_width)) = (
        left.data_type().primitive_width(),
        right.data_type().primitive_width(),
    ) else {
        // No native output exists for nonprimitive arithmetic; the native
        // dispatch below returns its original admitted diagnostic.
        return Ok(());
    };
    // Date32 subtraction produces an i64 duration; all other supported native
    // output widths are no greater than one of the two primitive operands.
    let width = left_width.max(right_width).max(8);
    let bytes = rows
        .checked_mul(width)
        .ok_or_else(|| error("native_arithmetic_values", rows, isize::MAX as usize))?;
    let aligned = arrow_buffer_59::MutableBuffer::try_capacity_layout(bytes)
        .map_err(|_| error("native_arithmetic_values", bytes, isize::MAX as usize))?;
    admit(aligned, "native_arithmetic_values")?;
    // Infallible arity::binary uses an exact Vec; try_binary uses MutableBuffer.
    admit(
        Layout::from_size_align(bytes, 32)
            .map_err(|_| error("native_arithmetic_values", bytes, isize::MAX as usize))?,
        "native_arithmetic_vector",
    )?;
    admit(
        arrow_buffer_59::allocation_owner_layout(),
        "native_arithmetic_buffer_owner",
    )?;
    // Native PrimitiveArray<T> stores ScalarBuffer<T>, whose T is phantom; all
    // supported T share this payload layout. i256 additionally has the largest
    // selected scalar width and remains an explicit layout representative.
    arc::<PrimitiveArray<Decimal256Type>>("native_arithmetic_array_owner")?;
    if rows == 0 {
        admit(
            Layout::array::<arrow_buffer_59::Buffer>(1).expect("one descriptor"),
            "native_arithmetic_empty_descriptors",
        )?;
        admit(
            arrow_buffer_59::allocation_owner_layout(),
            "native_arithmetic_empty_null_owner",
        )?;
    } else if left.nulls().is_some() && right.nulls().is_some() {
        bitmap_binary(rows)?;
    }
    Ok(())
}

/// Boolean kernels use the same finite native aligned/unaligned bitmap paths.
/// `buffers` counts original operation nodes, not a bytes-per-row estimate.
pub(crate) fn predicate_bitmaps(rows: usize, buffers: usize) -> Result<(), ArrowError> {
    for _ in 0..buffers {
        bitmap_binary(rows)?;
    }
    admit(
        Layout::array::<u8>(128).expect("native bitmap length diagnostic"),
        "native_predicate_length_diagnostic",
    )
}
pub(crate) fn logical_nulls(array: &dyn crate::arrow::array::Array) -> Result<(), ArrowError> {
    if current_resource_owner().is_none() && crate::resource::current_resource_scope().is_none() {
        return Ok(());
    }
    match array.data_type() {
        DataType::Dictionary(_, _) | DataType::RunEndEncoded(_, _) | DataType::Union(_, _) => {
            Err(error("native_predicate_encoded_nulls", 1, 0))
        }
        DataType::Null => predicate_bitmaps(array.len(), 1),
        _ => Ok(()),
    }
}
pub(crate) fn comparison(
    left: &dyn crate::arrow::array::Array,
    right: &dyn crate::arrow::array::Array,
) -> Result<(), ArrowError> {
    logical_nulls(left)?;
    logical_nulls(right)?;
    // Plain array/array comparison computes values once and at most one output
    // validity/distinct bitmap. Both operations can use native padded-word
    // collection, already included in bitmap_binary's alternative branches.
    predicate_bitmaps(left.len(), 2)?;
    let bytes = display_size(left.data_type())?
        .checked_add(display_size(right.data_type())?)
        .and_then(|n| n.checked_add(128))
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| {
            error(
                "native_comparison_diagnostic",
                usize::MAX,
                isize::MAX as usize,
            )
        })?;
    admit(
        Layout::array::<u8>(bytes)
            .map_err(|_| error("native_comparison_diagnostic", bytes, isize::MAX as usize))?,
        "native_comparison_diagnostic",
    )
}
pub(crate) fn in_list(
    left: &dyn crate::arrow::array::Array,
    right: &crate::arrow::array::ListArray,
) -> Result<(), ArrowError> {
    use crate::arrow::array::{PrimitiveArray, StringArray, types::Decimal256Type};
    predicate_bitmaps(left.len(), 2)?;
    // The original comparison loop invokes ListArray::value once per valid row;
    // its selected concrete primitive/String slice allocates only this Arc.
    for row in 0..left.len().min(right.len()) {
        if left.is_valid(row) && right.is_valid(row) {
            match left.data_type() {
                DataType::Utf8 => arc::<StringArray>("native_in_list_slice_owner")?,
                kind if kind.primitive_width().is_some() => {
                    arc::<PrimitiveArray<Decimal256Type>>("native_in_list_slice_owner")?
                }
                _ => return Err(error("native_in_list_slice_type", 1, 0)),
            }
        }
    }
    Ok(())
}
pub(crate) fn repeated_boolean(
    value: bool,
    rows: usize,
) -> Result<crate::arrow::array::BooleanArray, ArrowError> {
    use crate::arrow::array::{BooleanArray, BooleanBufferBuilder};
    let mut builder = BooleanBufferBuilder::new(0);
    predicate_bitmaps(rows, 1)?;
    builder.append_n(rows, value);
    Ok(BooleanArray::new(builder.finish(), None))
}

fn arc_slice<T>(count: usize, kind: &'static str) -> Result<(), ArrowError> {
    let tail = Layout::array::<T>(count).map_err(|_| error(kind, count, isize::MAX as usize))?;
    let layout = Layout::new::<[usize; 2]>()
        .extend(tail)
        .map_err(|_| error(kind, count, isize::MAX as usize))?
        .0
        .pad_to_align();
    admit(layout, kind)
}
/// Native selected string/binary view casts. Includes the original to_data
/// round-trip, view buffers Arc, native builder finalization and offset limits.
pub(crate) fn view_cast(
    array: &dyn crate::arrow::array::Array,
    target: &DataType,
) -> Result<(), ArrowError> {
    use crate::arrow::array::{StringArray, StringViewArray, cast::AsArray};
    if current_resource_owner().is_none() && crate::resource::current_resource_scope().is_none() {
        return Ok(());
    }
    let rows = array.len();
    match (array.data_type(), target) {
        (
            DataType::Utf8 | DataType::LargeUtf8 | DataType::Binary | DataType::LargeBinary,
            DataType::Utf8View | DataType::BinaryView,
        ) => {
            let (last_offset, buffer_len) = match array.data_type() {
                DataType::Utf8 => {
                    let a = array.as_string::<i32>();
                    (
                        *a.value_offsets().last().unwrap() as usize,
                        a.values().len(),
                    )
                }
                DataType::LargeUtf8 => {
                    let a = array.as_string::<i64>();
                    (
                        *a.value_offsets().last().unwrap() as usize,
                        a.values().len(),
                    )
                }
                DataType::Binary => {
                    let a = array.as_binary::<i32>();
                    (
                        *a.value_offsets().last().unwrap() as usize,
                        a.values().len(),
                    )
                }
                DataType::LargeBinary => {
                    let a = array.as_binary::<i64>();
                    (
                        *a.value_offsets().last().unwrap() as usize,
                        a.values().len(),
                    )
                }
                _ => unreachable!(),
            };
            if last_offset >= u32::MAX as usize || buffer_len >= u32::MAX as usize {
                return Err(error(
                    "native_view_reused_block_geometry",
                    buffer_len.max(last_offset),
                    u32::MAX as usize - 1,
                ));
            }
            admit(
                Layout::array::<u128>(rows)
                    .map_err(|_| error("native_view_values", rows, isize::MAX as usize))?,
                "native_view_values",
            )?;
            // append_block on the fresh native builder grows its completed Vec
            // to four Buffer slots; finalization creates one immutable Arc tail.
            admit(
                Layout::array::<arrow_buffer_59::Buffer>(4).expect("four descriptors"),
                "native_view_completed_blocks",
            )?;
            arc_slice::<arrow_buffer_59::Buffer>(1, "native_view_buffer_tail")?;
            admit(
                arrow_buffer_59::allocation_owner_layout(),
                "native_view_values_owner",
            )?;
            predicate_bitmaps(rows, 1)?;
            arc::<StringViewArray>("native_view_array_owner")?;
        }
        (DataType::Utf8View | DataType::BinaryView, DataType::Utf8 | DataType::Binary) => {
            let (views, buffers) = match array.data_type() {
                DataType::Utf8View => {
                    let a = array.as_string_view();
                    (a.views(), a.data_buffers().len())
                }
                DataType::BinaryView => {
                    let a = array.as_binary_view();
                    (a.views(), a.data_buffers().len())
                }
                _ => unreachable!(),
            };
            let bytes = views
                .iter()
                .try_fold(0usize, |total, view| {
                    total.checked_add(*view as u32 as usize)
                })
                .ok_or_else(|| error("native_nonview_values", usize::MAX, i32::MAX as usize))?;
            if bytes > i32::MAX as usize {
                return Err(error("native_nonview_values", bytes, i32::MAX as usize));
            }
            array.try_admit_to_data()?;
            arc_slice::<arrow_buffer_59::Buffer>(buffers, "native_view_roundtrip_buffer_tail")?;
            admit(
                Layout::array::<u8>(bytes)
                    .map_err(|_| error("native_nonview_values", bytes, isize::MAX as usize))?,
                "native_nonview_values",
            )?;
            let offsets = rows
                .checked_add(1)
                .ok_or_else(|| error("native_nonview_offsets", rows, isize::MAX as usize))?;
            admit(
                Layout::array::<i32>(offsets)
                    .map_err(|_| error("native_nonview_offsets", rows, isize::MAX as usize))?,
                "native_nonview_offsets",
            )?;
            predicate_bitmaps(rows, 1)?;
            for _ in 0..2 {
                admit(
                    arrow_buffer_59::allocation_owner_layout(),
                    "native_nonview_buffer_owner",
                )?;
            }
            admit(
                Layout::array::<arrow_buffer_59::Buffer>(2).expect("two descriptors"),
                "native_nonview_final_descriptors",
            )?;
            admit(Layout::new::<i32>(), "native_nonview_builder_reset")?;
            arc::<StringArray>("native_nonview_array_owner")?;
        }
        _ => return Err(error("native_view_cast_geometry", 1, 0)),
    }
    Ok(())
}

pub(crate) fn clone_metadata(
    metadata: &std::collections::HashMap<String, String>,
) -> Result<std::collections::HashMap<String, String>, ArrowError> {
    let mut output = hash_map(metadata.len(), "native_view_field_metadata")?;
    for (key, value) in metadata {
        let mut key_bytes = new_vec(key.len(), "native_view_metadata_key")?;
        key_bytes.extend_from_slice(key.as_bytes());
        let mut value_bytes = new_vec(value.len(), "native_view_metadata_value")?;
        value_bytes.extend_from_slice(value.as_bytes());
        output.insert(
            String::from_utf8(key_bytes).expect("original UTF8"),
            String::from_utf8(value_bytes).expect("original UTF8"),
        );
    }
    Ok(output)
}

pub(crate) fn offset_limit(requested: usize, limit: usize) -> ArrowError {
    error("native_list_view_expansion", requested, limit)
}

/// Fallible formatting for closures that must return an error value. A refusal
/// remains the inline resource error, never a formatted/boxed replacement.
pub(crate) fn kernel_diagnostic(
    constructor: fn(String) -> crate::Error,
    arguments: std::fmt::Arguments<'_>,
) -> crate::Error {
    match format(arguments) {
        Ok(message) => constructor(message),
        Err(error) => error.into(),
    }
}
pub(crate) fn arrow_diagnostic(
    constructor: fn(String) -> ArrowError,
    arguments: std::fmt::Arguments<'_>,
) -> ArrowError {
    match format(arguments) {
        Ok(message) => constructor(message),
        Err(error) => error,
    }
}

pub(crate) fn joined_names<'a, I: Iterator<Item = &'a str>>(
    names: impl Fn() -> I,
) -> Result<String, ArrowError> {
    let mut length = 0usize;
    let mut count = 0usize;
    for name in names() {
        length = length
            .checked_add(name.len())
            .and_then(|n| n.checked_add(if count == 0 { 0 } else { 2 }))
            .ok_or_else(|| error("native_validation_names", usize::MAX, isize::MAX as usize))?;
        count += 1;
    }
    let mut bytes = new_vec(length, "native_validation_names")?;
    for (index, name) in names().enumerate() {
        if index != 0 {
            bytes.extend_from_slice(b", ");
        }
        bytes.extend_from_slice(name.as_bytes());
    }
    Ok(String::from_utf8(bytes).expect("original UTF8 names"))
}
