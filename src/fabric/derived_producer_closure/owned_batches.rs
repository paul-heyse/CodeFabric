//! Explicit native ownership boundary for retained DataFusion output.
//!
//! A stream may return projections of provider-owned buffers. Arrow's claim API replaces the
//! existing owner and cannot inspect it, so retained closure output is deliberately copied once
//! into fresh private native buffers. Null bit offsets, child arrays and dictionary buffers are
//! preserved. This boundary trades a bounded copy for provable raw ArrayData/slice lifetime.

use std::sync::Arc;

use arrow::array::ArrayData;
use arrow_array::{RecordBatch, RecordBatchOptions, make_array};
use arrow_buffer::{BooleanBuffer, Buffer, NullBuffer};

use crate::provider_contracts::allocation::ProviderAllocation;
use crate::resource_budget::ResourceBudget;

use super::DerivedProducerClosureError as Error;

pub(super) fn copy_owned_batch(
    batch: &RecordBatch,
    budget: &ResourceBudget,
    maximum_bytes: usize,
) -> Result<RecordBatch, Error> {
    let source_bytes = batch.get_array_memory_size();
    if source_bytes > maximum_bytes {
        return Err(Error::OutputBytesExceeded {
            limit: maximum_bytes,
            observed: source_bytes,
        });
    }
    super::super::bounded_encoding::preflight_schema(batch.schema_ref(), maximum_bytes)
        .map_err(|_| Error::OutputAllocationProfile)?;
    let envelope = source_bytes
        .checked_mul(4)
        .and_then(|n| n.checked_add(4096))
        .ok_or(Error::ResourceCounterOverflow("retained batch envelope"))?;
    let mut allocation = ProviderAllocation::try_new(budget, envelope as u64)?;
    let maximum_arrays = (maximum_bytes / 32).max(1);
    let mut visited = 0;
    let columns = batch
        .columns()
        .iter()
        .map(|array| copy_data(&array.to_data(), 0, &mut visited, maximum_arrays).map(make_array))
        .collect::<Result<Vec<_>, _>>()?;
    let copied = RecordBatch::try_new_with_options(
        Arc::clone(batch.schema_ref()),
        columns,
        &RecordBatchOptions::new().with_row_count(Some(batch.num_rows())),
    )?;
    allocation.claim_new_batches([&copied], maximum_arrays)?;
    Ok(copied)
}

fn copy_data(
    data: &ArrayData,
    depth: usize,
    visited: &mut usize,
    maximum_arrays: usize,
) -> Result<ArrayData, Error> {
    *visited = visited
        .checked_add(1)
        .ok_or(Error::ResourceCounterOverflow("array nodes"))?;
    if depth > 64
        || *visited > maximum_arrays
        || data.child_data().len() > maximum_arrays.saturating_sub(*visited)
        || data.buffers().len() > maximum_arrays.saturating_mul(4)
    {
        return Err(Error::OutputAllocationProfile);
    }
    let buffers = data
        .buffers()
        .iter()
        .map(|buffer| Buffer::from(buffer.as_slice()))
        .collect();
    let children = data
        .child_data()
        .iter()
        .map(|child| copy_data(child, depth + 1, visited, maximum_arrays))
        .collect::<Result<Vec<_>, _>>()?;
    let nulls = data.nulls().map(|nulls| {
        NullBuffer::new(BooleanBuffer::new(
            Buffer::from(nulls.buffer().as_slice()),
            nulls.offset(),
            nulls.len(),
        ))
    });
    Ok(ArrayData::builder(data.data_type().clone())
        .len(data.len())
        .offset(data.offset())
        .buffers(buffers)
        .child_data(children)
        .nulls(nulls)
        .build()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_budget::test_resource_budget;
    use arrow_array::{
        Array, BooleanArray, ListArray,
        builder::StringDictionaryBuilder,
        types::{Int8Type, Int64Type},
    };
    use arrow_schema::{Field, Schema};

    #[test]
    fn wp79_derived_owned_copy_preserves_nested_dictionary_and_null_offsets_without_reclaim() {
        let source_budget = test_resource_budget();
        let destination_budget = test_resource_budget();
        let mut dictionary = StringDictionaryBuilder::<Int8Type>::new();
        dictionary.append("first").unwrap();
        dictionary.append_null();
        dictionary.append("third").unwrap();
        let columns: Vec<arrow_array::ArrayRef> = vec![
            Arc::new(dictionary.finish()),
            Arc::new(ListArray::from_iter_primitive::<Int64Type, _, _>([
                Some(vec![Some(1), None]),
                None,
                Some(vec![Some(3)]),
            ])),
            Arc::new(BooleanArray::from(vec![Some(true), None, Some(false)])),
        ];
        let schema = Arc::new(Schema::new(
            columns
                .iter()
                .enumerate()
                .map(|(i, a)| Field::new(format!("column_{i}"), a.data_type().clone(), true))
                .collect::<Vec<_>>(),
        ));
        let original = RecordBatch::try_new(schema, columns).unwrap();
        let mut source_allocation = ProviderAllocation::try_new(&source_budget, 65536).unwrap();
        source_allocation
            .claim_new_batches([&original], 64)
            .unwrap();
        drop(source_allocation);
        let original_bytes = source_budget.observation().used.memory_bytes;
        let selected = original.slice(1, 2);
        let copied = copy_owned_batch(&selected, &destination_budget, 65536).unwrap();
        for (left, right) in selected.columns().iter().zip(copied.columns()) {
            assert_eq!(left.to_data(), right.to_data());
        }
        assert_eq!(
            source_budget.observation().used.memory_bytes,
            original_bytes
        );
        let raw = copied.column(0).to_data();
        let slice = copied.slice(1, 1);
        drop(copied);
        drop(slice);
        assert!(destination_budget.observation().used.memory_bytes > 0);
        drop(raw);
        assert_eq!(destination_budget.observation().used.memory_bytes, 0);
        assert_eq!(
            source_budget.observation().used.memory_bytes,
            original_bytes
        );
        drop(original);
        drop(selected);
        assert_eq!(source_budget.observation().used.memory_bytes, 0);
    }

    #[test]
    fn wp79_derived_owned_copy_denies_before_new_output_and_preserves_source() {
        let original = RecordBatch::try_new(
            Arc::new(Schema::new(vec![Field::new(
                "value",
                arrow_schema::DataType::Boolean,
                false,
            )])),
            vec![Arc::new(BooleanArray::from(vec![true, false]))],
        )
        .unwrap();
        let parent = test_resource_budget();
        let mut policy = parent.policy();
        policy.limits.memory_bytes = 1024;
        policy.control_reserve.memory_bytes = 128;
        let small = parent.workspace([8; 16], policy).unwrap();
        assert!(matches!(
            copy_owned_batch(&original, &small, 65536),
            Err(Error::Allocation(_))
        ));
        assert_eq!(small.observation().used.memory_bytes, 0);
        assert_eq!(small.observation().peak.memory_bytes, 0);
        assert_eq!(original.num_rows(), 2);
        assert_eq!(original.column(0).null_count(), 0);
    }
}
