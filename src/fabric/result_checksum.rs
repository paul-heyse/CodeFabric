//! Versioned, order-independent checksums for delivered Arrow result multisets.

use arrow_array::RecordBatch;
use arrow_row::{RowConverter, SortField};
use arrow_schema::{ArrowError, DataType, Schema};
use thiserror::Error;

/// Exact persisted result-checksum contract version.
pub const RESULT_CHECKSUM_VERSION: &str = "ResultChecksumV1";

/// Result checksum plus the canonical schema and row census it commits to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResultChecksumV1 {
    pub checksum: String,
    pub canonical_schema: Vec<u8>,
    pub row_count: u64,
}

/// Stable failures at the canonical Arrow result-checksum boundary.
#[derive(Debug, Error)]
pub enum ResultChecksumError {
    #[error(transparent)]
    Arrow(#[from] ArrowError),
    #[error("RESULT_CHECKSUM_SCHEMA_DRIFT")]
    SchemaDrift,
    #[error("RESULT_CHECKSUM_UNORDERED_MAP")]
    UnorderedMap,
    #[error("RESULT_CHECKSUM_NULLABLE_MAP_KEY")]
    NullableMapKey,
    #[error("RESULT_CHECKSUM_RESOURCE_LIMIT")]
    ResourceLimit,
    #[error("result checksum canonical schema: {0}")]
    CanonicalSchema(String),
}

fn validate_data_type(data_type: &DataType) -> Result<(), ResultChecksumError> {
    match data_type {
        DataType::Map(entries, keys_sorted) => {
            if !keys_sorted {
                return Err(ResultChecksumError::UnorderedMap);
            }
            let DataType::Struct(fields) = entries.data_type() else {
                return Err(ResultChecksumError::Arrow(
                    ArrowError::InvalidArgumentError("map entries are not a struct".to_owned()),
                ));
            };
            let Some(key) = fields.first() else {
                return Err(ResultChecksumError::Arrow(
                    ArrowError::InvalidArgumentError("map has no key field".to_owned()),
                ));
            };
            if key.is_nullable() {
                return Err(ResultChecksumError::NullableMapKey);
            }
            for field in fields {
                validate_data_type(field.data_type())?;
            }
        }
        DataType::Struct(fields) => {
            for field in fields {
                validate_data_type(field.data_type())?;
            }
        }
        DataType::Union(fields, _) => {
            for (_, field) in fields.iter() {
                validate_data_type(field.data_type())?;
            }
        }
        DataType::List(field)
        | DataType::LargeList(field)
        | DataType::ListView(field)
        | DataType::LargeListView(field)
        | DataType::FixedSizeList(field, _)
        | DataType::RunEndEncoded(_, field) => validate_data_type(field.data_type())?,
        DataType::Dictionary(_, value) => validate_data_type(value)?,
        _ => {}
    }
    Ok(())
}

/// Compute `ResultChecksumV1` over one exact schema and a bounded multiset of logical Arrow rows.
///
/// Arrow's row converter supplies stable total-order encodings, including logical dictionary
/// values, nested values, signed zero, and NaN payload ordering. Map columns must declare sorted,
/// non-null keys so representation order cannot silently change map semantics. Encoded rows are
/// sorted without deduplication, preserving duplicate multiplicity while making batch and row
/// arrival order irrelevant.
///
/// # Errors
///
/// Rejects schema drift, unsupported Arrow row encodings, non-canonical maps, counter overflow,
/// or canonical schema/row bytes beyond `maximum_encoding_bytes`.
pub fn result_checksum_v1(
    schema: &Schema,
    batches: &[RecordBatch],
    maximum_encoding_bytes: usize,
) -> Result<ResultChecksumV1, ResultChecksumError> {
    for field in schema.fields() {
        validate_data_type(field.data_type())?;
    }
    let schema_value = serde_json::to_value(schema)
        .map_err(|error| ResultChecksumError::CanonicalSchema(error.to_string()))?;
    let canonical_schema = crate::contracts::jcs::canonicalize_value(&schema_value)
        .map_err(|error| ResultChecksumError::CanonicalSchema(error.to_string()))?;
    if canonical_schema.len() > maximum_encoding_bytes {
        return Err(ResultChecksumError::ResourceLimit);
    }
    let converter = RowConverter::new(
        schema
            .fields()
            .iter()
            .map(|field| SortField::new(field.data_type().clone()))
            .collect(),
    )?;
    let mut row_count = 0_u64;
    let mut encoding_bytes = canonical_schema.len();
    let mut rows = Vec::<Vec<u8>>::new();
    for batch in batches {
        if batch.schema().as_ref() != schema {
            return Err(ResultChecksumError::SchemaDrift);
        }
        row_count = row_count
            .checked_add(
                u64::try_from(batch.num_rows()).map_err(|_| ResultChecksumError::ResourceLimit)?,
            )
            .ok_or(ResultChecksumError::ResourceLimit)?;
        if schema.fields().is_empty() {
            continue;
        }
        let encoded = converter.convert_columns(batch.columns())?;
        for row in &encoded {
            encoding_bytes = encoding_bytes
                .checked_add(row.data().len())
                .ok_or(ResultChecksumError::ResourceLimit)?;
            if encoding_bytes > maximum_encoding_bytes {
                return Err(ResultChecksumError::ResourceLimit);
            }
            rows.push(row.data().to_vec());
        }
    }
    rows.sort_unstable();
    let mut hasher = crate::integrity::IntegrityHasher::for_domain(
        crate::integrity::IntegrityDomain::QueryResultChecksumV1,
    );
    hasher.update(&(canonical_schema.len() as u64).to_be_bytes());
    hasher.update(&canonical_schema);
    hasher.update(&row_count.to_be_bytes());
    for row in rows {
        hasher.update(&(row.len() as u64).to_be_bytes());
        hasher.update(&row);
    }
    Ok(ResultChecksumV1 {
        checksum: crate::integrity::frame_digest(hasher.finalize()),
        canonical_schema,
        row_count,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use arrow_array::types::Int8Type;
    use arrow_array::{
        Array as _, ArrayRef, DictionaryArray, Float64Array, Int8Array, Int64Array, ListArray,
        MapArray, RecordBatchOptions, StringArray, StructArray,
    };
    use arrow_buffer::OffsetBuffer;
    use arrow_schema::Field;

    use super::*;

    const LIMIT: usize = 1024 * 1024;

    fn int_batch(values: Vec<i64>) -> RecordBatch {
        RecordBatch::try_from_iter([("value", Arc::new(Int64Array::from(values)) as ArrayRef)])
            .unwrap()
    }

    #[test]
    fn wp64_behavioral_acceptance() {
        let first = int_batch(vec![3, 1, 2]);
        let second = int_batch(vec![2, 3, 1]);
        let one_batch = result_checksum_v1(first.schema().as_ref(), &[first], LIMIT).unwrap();
        let repartitioned = result_checksum_v1(
            second.schema().as_ref(),
            &[second.slice(0, 1), second.slice(1, 2)],
            LIMIT,
        )
        .unwrap();
        assert_eq!(one_batch.checksum, repartitioned.checksum);
        assert_eq!(one_batch.row_count, 3);
    }

    #[test]
    fn wp64_structural_acceptance() {
        assert_eq!(RESULT_CHECKSUM_VERSION, "ResultChecksumV1");
        assert_eq!(
            crate::integrity::IntegrityDomain::QueryResultChecksumV1.bytes(),
            b"codefabric.query-result-checksum.v1\0"
        );
        let schema = Schema::new(vec![
            Field::new("value", DataType::Int64, false).with_metadata(HashMap::from([
                ("z".to_owned(), "last".to_owned()),
                (
                    "ARROW:extension:name".to_owned(),
                    "codefabric.test".to_owned(),
                ),
            ])),
        ])
        .with_metadata(HashMap::from([
            ("schema-z".to_owned(), "last".to_owned()),
            ("schema-a".to_owned(), "first".to_owned()),
        ]));
        let batch = RecordBatch::try_new(
            Arc::new(schema.clone()),
            vec![Arc::new(Int64Array::from(vec![1]))],
        )
        .unwrap();
        let checksum = result_checksum_v1(&schema, &[batch], LIMIT).unwrap();
        assert!(
            std::str::from_utf8(&checksum.canonical_schema)
                .unwrap()
                .contains("ARROW:extension:name")
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One contract matrix locks the Arrow edge semantics together.
    fn wp64_negative_zero_state() {
        let single = int_batch(vec![1]);
        let duplicate = int_batch(vec![1, 1]);
        assert_ne!(
            result_checksum_v1(
                single.schema().as_ref(),
                std::slice::from_ref(&single),
                LIMIT
            )
            .unwrap()
            .checksum,
            result_checksum_v1(duplicate.schema().as_ref(), &[duplicate], LIMIT)
                .unwrap()
                .checksum
        );

        let metadata_schema = Schema::new(vec![Field::new("value", DataType::Int64, false)])
            .with_metadata(HashMap::from([("revision".to_owned(), "2".to_owned())]));
        let metadata_batch = RecordBatch::try_new(
            Arc::new(metadata_schema.clone()),
            vec![Arc::new(Int64Array::from(vec![1]))],
        )
        .unwrap();
        assert_ne!(
            result_checksum_v1(single.schema().as_ref(), &[single], LIMIT)
                .unwrap()
                .checksum,
            result_checksum_v1(&metadata_schema, &[metadata_batch], LIMIT)
                .unwrap()
                .checksum
        );

        let item = Arc::new(Field::new("item", DataType::Int64, true));
        let list = ListArray::new(
            Arc::clone(&item),
            OffsetBuffer::from_lengths([2, 1]),
            Arc::new(Int64Array::from(vec![1, 2, 3])),
            None,
        );
        let list_schema = Schema::new(vec![Field::new("nested", DataType::List(item), false)]);
        let list_batch =
            RecordBatch::try_new(Arc::new(list_schema.clone()), vec![Arc::new(list)]).unwrap();
        assert_eq!(
            result_checksum_v1(&list_schema, &[list_batch], LIMIT)
                .unwrap()
                .row_count,
            2
        );

        let first_dictionary = DictionaryArray::<Int8Type>::try_new(
            Int8Array::from(vec![0, 1]),
            Arc::new(StringArray::from(vec!["a", "b"])),
        )
        .unwrap();
        let second_dictionary = DictionaryArray::<Int8Type>::try_new(
            Int8Array::from(vec![1, 0]),
            Arc::new(StringArray::from(vec!["b", "a"])),
        )
        .unwrap();
        let dictionary_schema = Schema::new(vec![Field::new(
            "dictionary",
            first_dictionary.data_type().clone(),
            false,
        )]);
        let dictionary_checksum =
            |array: DictionaryArray<Int8Type>| {
                result_checksum_v1(
                    &dictionary_schema,
                    &[RecordBatch::try_new(
                        Arc::new(dictionary_schema.clone()),
                        vec![Arc::new(array)],
                    )
                    .unwrap()],
                    LIMIT,
                )
                .unwrap()
                .checksum
            };
        assert_eq!(
            dictionary_checksum(first_dictionary),
            dictionary_checksum(second_dictionary)
        );

        let entry_fields = vec![
            Arc::new(Field::new("keys", DataType::Utf8, false)),
            Arc::new(Field::new("values", DataType::Int64, true)),
        ];
        let entries = StructArray::new(
            entry_fields.clone().into(),
            vec![
                Arc::new(StringArray::from(vec!["a", "b"])),
                Arc::new(Int64Array::from(vec![1, 2])),
            ],
            None,
        );
        let entry = Arc::new(Field::new(
            "entries",
            DataType::Struct(entry_fields.into()),
            false,
        ));
        let ordered_map = MapArray::new(
            Arc::clone(&entry),
            OffsetBuffer::from_lengths([2]),
            entries.clone(),
            None,
            true,
        );
        let map_schema = Schema::new(vec![Field::new(
            "map",
            DataType::Map(Arc::clone(&entry), true),
            false,
        )]);
        let map_batch =
            RecordBatch::try_new(Arc::new(map_schema.clone()), vec![Arc::new(ordered_map)])
                .unwrap();
        assert!(result_checksum_v1(&map_schema, &[map_batch], LIMIT).is_ok());
        let unordered_schema =
            Schema::new(vec![Field::new("map", DataType::Map(entry, false), false)]);
        assert!(matches!(
            result_checksum_v1(&unordered_schema, &[], LIMIT),
            Err(ResultChecksumError::UnorderedMap)
        ));

        let floats = |values| {
            let batch = RecordBatch::try_from_iter([(
                "float",
                Arc::new(Float64Array::from(values)) as ArrayRef,
            )])
            .unwrap();
            result_checksum_v1(batch.schema().as_ref(), std::slice::from_ref(&batch), LIMIT)
                .unwrap()
                .checksum
        };
        assert_ne!(floats(vec![-0.0]), floats(vec![0.0]));
        assert_ne!(
            floats(vec![f64::from_bits(0x7ff8_0000_0000_0001)]),
            floats(vec![f64::from_bits(0x7ff8_0000_0000_0002)])
        );

        let empty_schema = Arc::new(Schema::empty());
        let zero_columns = RecordBatch::try_new_with_options(
            Arc::clone(&empty_schema),
            vec![],
            &RecordBatchOptions::new().with_row_count(Some(2)),
        )
        .unwrap();
        assert_eq!(
            result_checksum_v1(empty_schema.as_ref(), &[zero_columns], LIMIT)
                .unwrap()
                .row_count,
            2
        );
    }

    #[test]
    fn wp64_operational_acceptance() {
        let batch = int_batch(vec![9, 4, 7]);
        let recorded =
            result_checksum_v1(batch.schema().as_ref(), std::slice::from_ref(&batch), LIMIT)
                .unwrap();
        let replayed = result_checksum_v1(batch.schema().as_ref(), &[batch], LIMIT).unwrap();
        assert_eq!(recorded, replayed);
        assert!(recorded.checksum.starts_with("b3:"));
    }
}
