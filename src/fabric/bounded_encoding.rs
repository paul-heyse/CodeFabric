//! Allocation guards around Arrow's native, uncompressed streaming encoder.
//!
//! Arrow 59 streams ordinary data buffers, but builds dictionary bodies, rebased offsets,
//! validity bitmaps, and flatbuffer metadata before calling `Write`. Guard those inputs first;
//! a counting writer alone cannot bound those internal allocations.

use std::io::{self, Write};

use arrow::array::ArrayData;
use arrow_array::{
    Array, RecordBatch,
    cast::AsArray,
    types::{Int16Type, Int32Type, Int64Type},
};
use arrow_ipc::writer::StreamWriter;
use arrow_schema::{DataType, Field, SchemaRef};

use arrow_schema::ArrowError;
use thiserror::Error;

/// Bounded native encoding is a data-fabric primitive shared by in-memory and durable results.
#[derive(Debug, Error)]
pub enum BoundedEncodingError {
    #[error("Arrow encoding counter overflow")]
    CounterOverflow,
    #[error("Arrow encoding exceeds nesting bounds")]
    EncodingNestingLimit,
    #[error("Arrow page bytes {observed} exceed limit {limit}")]
    PageByteLimit { observed: usize, limit: usize },
    #[error("Arrow encoding failed: {0}")]
    Arrow(#[source] ArrowError),
}

use BoundedEncodingError as Error;

#[cfg(test)]
thread_local! { static OUTPUT_ALLOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

pub(in crate::fabric) struct CappedWriter {
    limit: usize,
    count: usize,
    bytes: Option<Vec<u8>>,
}

impl CappedWriter {
    pub(in crate::fabric) const fn counting(limit: usize) -> Self {
        Self {
            limit,
            count: 0,
            bytes: None,
        }
    }

    fn output(size: usize) -> Self {
        #[cfg(test)]
        OUTPUT_ALLOCATIONS.with(|count| count.set(count.get() + 1));
        Self {
            limit: size,
            count: 0,
            bytes: Some(Vec::with_capacity(size)),
        }
    }

    pub(in crate::fabric) const fn count(&self) -> usize {
        self.count
    }
}

impl Write for CappedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = self
            .count
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("result encoding size overflow"))?;
        if count > self.limit {
            return Err(io::Error::other("result encoding exceeds its byte bound"));
        }
        if let Some(output) = &mut self.bytes {
            output.extend_from_slice(bytes);
        }
        self.count = count;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn add(total: &mut usize, amount: usize, limit: usize) -> Result<(), Error> {
    *total = total.checked_add(amount).ok_or(Error::CounterOverflow)?;
    if *total > limit {
        return Err(Error::PageByteLimit {
            observed: *total,
            limit,
        });
    }
    Ok(())
}

fn metadata<'a>(
    entries: impl Iterator<Item = (&'a String, &'a String)>,
    size: &mut usize,
    limit: usize,
) -> Result<(), Error> {
    for (key, value) in entries {
        add(size, 64, limit)?;
        add(size, key.len(), limit)?;
        add(size, value.len(), limit)?;
    }
    Ok(())
}

fn field(field: &Field, size: &mut usize, limit: usize, depth: usize) -> Result<(), Error> {
    add(size, 64, limit)?;
    add(size, field.name().len(), limit)?;
    metadata(field.metadata().iter(), size, limit)?;
    data_type(field.data_type(), size, limit, depth)
}

fn data_type(value: &DataType, size: &mut usize, limit: usize, depth: usize) -> Result<(), Error> {
    if depth > 64 {
        return Err(Error::EncodingNestingLimit);
    }
    add(size, 32, limit)?;
    match value {
        DataType::List(child)
        | DataType::LargeList(child)
        | DataType::ListView(child)
        | DataType::LargeListView(child)
        | DataType::FixedSizeList(child, _)
        | DataType::Map(child, _) => field(child, size, limit, depth + 1)?,
        DataType::Struct(children) => {
            for child in children {
                field(child, size, limit, depth + 1)?;
            }
        }
        DataType::Union(children, _) => {
            for (_, child) in children.iter() {
                field(child, size, limit, depth + 1)?;
            }
        }
        DataType::Dictionary(key, value) => {
            data_type(key, size, limit, depth + 1)?;
            data_type(value, size, limit, depth + 1)?;
        }
        DataType::RunEndEncoded(runs, values) => {
            field(runs, size, limit, depth + 1)?;
            field(values, size, limit, depth + 1)?;
        }
        DataType::Timestamp(_, Some(timezone)) => add(size, timezone.len(), limit)?,
        _ => {}
    }
    Ok(())
}

pub(in crate::fabric) fn preflight_schema(schema: &SchemaRef, limit: usize) -> Result<(), Error> {
    let mut size = 0;
    metadata(schema.metadata().iter(), &mut size, limit)?;
    for child in schema.fields() {
        field(child, &mut size, limit, 0)?;
    }
    Ok(())
}

fn preflight_data(
    data: &ArrayData,
    limit: usize,
    metadata_size: &mut usize,
    scratch_size: &mut usize,
) -> Result<(), Error> {
    // Bounds metadata vectors and scratch derived from nested row counts, including zero-width
    // Null children. Slices may retain larger source buffers; only dictionaries and view buffers
    // below are necessarily serialized in full by Arrow 59.
    add(metadata_size, 64, limit)?;
    add(
        metadata_size,
        data.buffers()
            .len()
            .checked_mul(64)
            .ok_or(Error::CounterOverflow)?,
        limit,
    )?;
    if data.len() > limit {
        return Err(Error::PageByteLimit {
            observed: data.len(),
            limit,
        });
    }
    if matches!(data.data_type(), DataType::Dictionary(_, _)) {
        let mut dictionary_bytes = 0;
        for child in data.child_data() {
            add(&mut dictionary_bytes, child.get_array_memory_size(), limit)?;
        }
        add(
            scratch_size,
            dictionary_bytes,
            limit.checked_mul(4).ok_or(Error::CounterOverflow)?,
        )?;
    }
    // Bound aggregate offset/null scratch across ALL nodes, not just each individual column.
    add(
        scratch_size,
        data.len().checked_mul(16).ok_or(Error::CounterOverflow)?,
        limit.checked_mul(4).ok_or(Error::CounterOverflow)?,
    )?;
    if matches!(data.data_type(), DataType::BinaryView | DataType::Utf8View) {
        let mut view_bytes = 0;
        for buffer in data.buffers().iter().skip(1) {
            add(&mut view_bytes, buffer.len(), limit)?;
        }
    }
    for child in data.child_data() {
        let selected = match data.data_type() {
            DataType::List(_) | DataType::Map(_, _) => {
                let offsets = data.buffers()[0].typed_data::<i32>();
                let start = usize::try_from(offsets[data.offset()])
                    .map_err(|_| Error::EncodingNestingLimit)?;
                let end = usize::try_from(offsets[data.offset() + data.len()])
                    .map_err(|_| Error::EncodingNestingLimit)?;
                Some(child.slice(start, end - start))
            }
            DataType::LargeList(_) => {
                let offsets = data.buffers()[0].typed_data::<i64>();
                let start = usize::try_from(offsets[data.offset()])
                    .map_err(|_| Error::EncodingNestingLimit)?;
                let end = usize::try_from(offsets[data.offset() + data.len()])
                    .map_err(|_| Error::EncodingNestingLimit)?;
                Some(child.slice(start, end - start))
            }
            DataType::FixedSizeList(_, width) => {
                let width = usize::try_from(*width).map_err(|_| Error::EncodingNestingLimit)?;
                Some(
                    child.slice(
                        data.offset()
                            .checked_mul(width)
                            .ok_or(Error::CounterOverflow)?,
                        data.len()
                            .checked_mul(width)
                            .ok_or(Error::CounterOverflow)?,
                    ),
                )
            }
            _ => None,
        };
        preflight_data(
            selected.as_ref().unwrap_or(child),
            limit,
            metadata_size,
            scratch_size,
        )?;
    }
    Ok(())
}

fn preflight_buffer_vectors(
    array: &dyn Array,
    limit: usize,
    size: &mut usize,
) -> Result<(), Error> {
    match array.data_type() {
        DataType::Utf8View => add(
            size,
            array
                .as_string_view()
                .data_buffers()
                .len()
                .checked_mul(64)
                .ok_or(Error::CounterOverflow)?,
            limit,
        )?,
        DataType::BinaryView => add(
            size,
            array
                .as_binary_view()
                .data_buffers()
                .len()
                .checked_mul(64)
                .ok_or(Error::CounterOverflow)?,
            limit,
        )?,
        DataType::List(_) => {
            preflight_buffer_vectors(array.as_list::<i32>().values().as_ref(), limit, size)?;
        }
        DataType::LargeList(_) => {
            preflight_buffer_vectors(array.as_list::<i64>().values().as_ref(), limit, size)?;
        }
        DataType::ListView(_) => {
            preflight_buffer_vectors(array.as_list_view::<i32>().values().as_ref(), limit, size)?;
        }
        DataType::LargeListView(_) => {
            preflight_buffer_vectors(array.as_list_view::<i64>().values().as_ref(), limit, size)?;
        }
        DataType::FixedSizeList(_, _) => {
            preflight_buffer_vectors(array.as_fixed_size_list().values().as_ref(), limit, size)?;
        }
        DataType::Map(_, _) => preflight_buffer_vectors(array.as_map().entries(), limit, size)?,
        DataType::Struct(_) => {
            for child in array.as_struct().columns() {
                preflight_buffer_vectors(child.as_ref(), limit, size)?;
            }
        }
        DataType::Union(fields, _) => {
            for (id, _) in fields.iter() {
                preflight_buffer_vectors(array.as_union().child(id).as_ref(), limit, size)?;
            }
        }
        DataType::Dictionary(_, _) => {
            preflight_buffer_vectors(array.as_any_dictionary().values().as_ref(), limit, size)?;
        }
        DataType::RunEndEncoded(runs, _) => match runs.data_type() {
            DataType::Int16 => preflight_buffer_vectors(
                array.as_run::<Int16Type>().values().as_ref(),
                limit,
                size,
            )?,
            DataType::Int32 => preflight_buffer_vectors(
                array.as_run::<Int32Type>().values().as_ref(),
                limit,
                size,
            )?,
            DataType::Int64 => preflight_buffer_vectors(
                array.as_run::<Int64Type>().values().as_ref(),
                limit,
                size,
            )?,
            _ => return Err(Error::EncodingNestingLimit),
        },
        _ => {}
    }
    Ok(())
}

fn write_page(
    schema: &SchemaRef,
    batches: &[RecordBatch],
    output: &mut CappedWriter,
) -> Result<(), Error> {
    let mut writer = StreamWriter::try_new(output, schema).map_err(Error::Arrow)?;
    for batch in batches {
        writer.write(batch).map_err(Error::Arrow)?;
    }
    writer.finish().map_err(Error::Arrow)
}

pub(in crate::fabric) fn page_size(
    schema: &SchemaRef,
    batches: &[RecordBatch],
    limit: usize,
) -> Result<usize, Error> {
    preflight_schema(schema, limit)?;
    let mut size = 0;
    let mut scratch_size = 0;
    for batch in batches {
        for column in batch.columns() {
            // Bound the vector before `to_data` can clone arbitrarily many zero-length buffers.
            preflight_buffer_vectors(column.as_ref(), limit, &mut size)?;
            preflight_data(&column.to_data(), limit, &mut size, &mut scratch_size)?;
        }
    }
    let mut counter = CappedWriter::counting(limit);
    match write_page(schema, batches, &mut counter) {
        Err(Error::Arrow(arrow_schema::ArrowError::IoError(_, _))) => Err(Error::PageByteLimit {
            observed: limit.saturating_add(1),
            limit,
        }),
        Err(error) => Err(error),
        Ok(()) => Ok(counter.count()),
    }
}

pub(in crate::fabric) fn encode_page(
    schema: &SchemaRef,
    batches: &[RecordBatch],
    limit: usize,
) -> Result<Vec<u8>, Error> {
    let size = page_size(schema, batches, limit)?;
    // No output allocation exists until the complete native counting pass succeeds.
    let mut output = CappedWriter::output(size);
    write_page(schema, batches, &mut output)?;
    Ok(output.bytes.expect("output writer owns its buffer"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{
        ArrayRef, DictionaryArray, Int8Array, StringArray, StringViewArray, types::Int8Type,
    };
    use arrow_schema::Schema;
    use std::sync::Arc;

    #[test]
    fn wp79_result_huge_values_are_rejected_before_output_allocation() {
        let value = "x".repeat(1024 * 1024);
        let strings = Arc::new(StringArray::from(vec![value.as_str()]));
        let dictionary =
            DictionaryArray::<Int8Type>::try_new(Int8Array::from(vec![0]), strings.clone())
                .unwrap();
        for array in [
            strings as ArrayRef,
            Arc::new(dictionary) as ArrayRef,
            Arc::new(StringViewArray::from(vec![value.as_str()])) as ArrayRef,
        ] {
            let schema = Arc::new(Schema::new(vec![Field::new(
                "value",
                array.data_type().clone(),
                false,
            )]));
            let batch = RecordBatch::try_new(schema.clone(), vec![array]).unwrap();
            OUTPUT_ALLOCATIONS.with(|count| count.set(0));
            assert!(matches!(
                encode_page(&schema, &[batch], 1024),
                Err(Error::PageByteLimit { .. })
            ));
            OUTPUT_ALLOCATIONS.with(|count| assert_eq!(count.get(), 0));
        }
    }

    #[test]
    fn wp79_result_schema_metadata_and_nesting_precede_native_encoding() {
        let schema = Arc::new(Schema::new(vec![Field::new(
            "x".repeat(1024 * 1024),
            DataType::Int64,
            false,
        )]));
        OUTPUT_ALLOCATIONS.with(|count| count.set(0));
        assert!(matches!(
            encode_page(&schema, &[], 1024),
            Err(Error::PageByteLimit { .. })
        ));
        OUTPUT_ALLOCATIONS.with(|count| assert_eq!(count.get(), 0));
        let mut nested = DataType::Int64;
        for _ in 0..66 {
            nested = DataType::List(Arc::new(Field::new("item", nested, false)));
        }
        let schema = Arc::new(Schema::new(vec![Field::new("value", nested, false)]));
        assert!(matches!(
            encode_page(&schema, &[], 65536),
            Err(Error::EncodingNestingLimit)
        ));
        OUTPUT_ALLOCATIONS.with(|count| assert_eq!(count.get(), 0));
    }

    #[test]
    fn wp79_result_nested_single_value_is_bounded_without_materializing_it() {
        let child = Arc::new(arrow_array::NullArray::new(1_048_576)) as ArrayRef;
        let array = arrow_array::ListArray::new(
            Arc::new(Field::new("item", DataType::Null, true)),
            arrow::buffer::OffsetBuffer::new(vec![0_i32, 1_048_576].into()),
            child,
            None,
        );
        let schema = Arc::new(Schema::new(vec![Field::new(
            "value",
            array.data_type().clone(),
            false,
        )]));
        let batch = RecordBatch::try_new(schema.clone(), vec![Arc::new(array)]).unwrap();
        OUTPUT_ALLOCATIONS.with(|count| count.set(0));
        assert!(matches!(
            encode_page(&schema, &[batch], 1024),
            Err(Error::PageByteLimit { .. })
        ));
        OUTPUT_ALLOCATIONS.with(|count| assert_eq!(count.get(), 0));
    }
}
