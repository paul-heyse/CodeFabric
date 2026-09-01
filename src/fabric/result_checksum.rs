//! Versioned, order-independent checksums for delivered Arrow result multisets.

use arrow_array::{
    Array, FixedSizeListArray, LargeListArray, ListArray, MapArray, RecordBatch, StructArray,
};
use arrow_row::{RowConverter, SortField};
use arrow_schema::{ArrowError, DataType, Schema};
use thiserror::Error;

/// Result checksum plus the canonical schema and row census it commits to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResultChecksumV1 {
    pub checksum: String,
    pub canonical_schema: Vec<u8>,
    pub row_count: u64,
}

/// Extension-aware checksum over an application-owned logical result schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResultChecksumV2 {
    pub checksum: String,
    pub canonical_schema: Vec<u8>,
    pub row_count: u64,
}

/// Gate-only checksum domain. It is never used as a delivered query-result digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateResultChecksumV1 {
    pub checksum: String,
    pub canonical_schema: Vec<u8>,
    pub row_count: u64,
}

/// Exact gate-result checksum contract version.
pub const GATE_RESULT_CHECKSUM_VERSION: &str = "GateResultChecksumV1";

/// Compute the canonical schema-and-row checksum for one exact Arrow batch.
///
/// The application-owned schema digest, row count, and sorted Arrow row encodings are committed under the
/// application-owned Arrow-batch integrity domain, so input row order does not affect the result.
///
/// # Errors
///
/// Returns an Arrow row-encoding error for a type unsupported by the pinned Arrow version.
pub fn batch_checksum(batch: &RecordBatch) -> Result<[u8; 32], super::FabricError> {
    let schema = batch.schema();
    let fields = schema
        .fields()
        .iter()
        .map(|field| SortField::new(field.data_type().clone()))
        .collect();
    let converter = RowConverter::new(fields)?;
    let rows = converter.convert_columns(batch.columns())?;
    let mut ordered = rows.iter().map(|row| row.data()).collect::<Vec<_>>();
    ordered.sort_unstable();
    let mut hasher = crate::integrity::IntegrityHasher::for_domain(
        crate::integrity::IntegrityDomain::ArrowBatch,
    );
    if let Some(digest) = schema.metadata().get("com.codefabric.cpg.schema_digest") {
        hasher.update(digest.as_bytes());
    }
    hasher.update(
        &u64::try_from(batch.num_rows())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    for row in ordered {
        hasher.update(&u64::try_from(row.len()).unwrap_or(u64::MAX).to_be_bytes());
        hasher.update(row);
    }
    Ok(hasher.finalize())
}

fn gate_framed(parts: impl IntoIterator<Item = impl AsRef<[u8]>>) -> String {
    let mut bytes = Vec::new();
    for part in parts {
        let part = part.as_ref();
        bytes.extend_from_slice(&(part.len() as u64).to_be_bytes());
        bytes.extend_from_slice(part);
    }
    crate::integrity::framed_digest(&bytes)
}

/// Version-directed replay result for historical and current delivered artifacts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VersionedResultChecksum {
    V1(ResultChecksumV1),
    V2(ResultChecksumV2),
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

fn validate_array(array: &dyn Array) -> Result<(), ResultChecksumError> {
    match array.data_type() {
        DataType::Map(_, _) => {
            let map = array.as_any().downcast_ref::<MapArray>().ok_or_else(|| {
                ResultChecksumError::Arrow(ArrowError::CastError(
                    "Map datatype did not contain MapArray".into(),
                ))
            })?;
            if map.keys().null_count() != 0 {
                return Err(ResultChecksumError::NullableMapKey);
            }
            validate_array(map.keys().as_ref())?;
            validate_array(map.values().as_ref())?;
            let converter =
                RowConverter::new(vec![SortField::new(map.keys().data_type().clone())])?;
            let encoded = converter.convert_columns(&[map.keys().clone()])?;
            for offsets in map.value_offsets().windows(2) {
                let start =
                    usize::try_from(offsets[0]).map_err(|_| ResultChecksumError::ResourceLimit)?;
                let end =
                    usize::try_from(offsets[1]).map_err(|_| ResultChecksumError::ResourceLimit)?;
                for index in start.saturating_add(1)..end {
                    if encoded.row(index - 1).data() >= encoded.row(index).data() {
                        return Err(ResultChecksumError::UnorderedMap);
                    }
                }
            }
        }
        DataType::Struct(_) => {
            let values = array
                .as_any()
                .downcast_ref::<StructArray>()
                .ok_or_else(|| {
                    ResultChecksumError::Arrow(ArrowError::CastError(
                        "Struct datatype did not contain StructArray".into(),
                    ))
                })?;
            for column in values.columns() {
                validate_array(column.as_ref())?;
            }
        }
        DataType::List(_) => validate_array(
            array
                .as_any()
                .downcast_ref::<ListArray>()
                .ok_or_else(|| {
                    ResultChecksumError::Arrow(ArrowError::CastError(
                        "List datatype did not contain ListArray".into(),
                    ))
                })?
                .values()
                .as_ref(),
        )?,
        DataType::LargeList(_) => validate_array(
            array
                .as_any()
                .downcast_ref::<LargeListArray>()
                .ok_or_else(|| {
                    ResultChecksumError::Arrow(ArrowError::CastError(
                        "LargeList datatype did not contain LargeListArray".into(),
                    ))
                })?
                .values()
                .as_ref(),
        )?,
        DataType::FixedSizeList(_, _) => validate_array(
            array
                .as_any()
                .downcast_ref::<FixedSizeListArray>()
                .ok_or_else(|| {
                    ResultChecksumError::Arrow(ArrowError::CastError(
                        "FixedSizeList datatype did not contain FixedSizeListArray".into(),
                    ))
                })?
                .values()
                .as_ref(),
        )?,
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
fn result_checksum(
    schema: &Schema,
    batches: &[RecordBatch],
    maximum_encoding_bytes: usize,
    domain: crate::integrity::IntegrityDomain,
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
        for column in batch.columns() {
            validate_array(column.as_ref())?;
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
    let mut hasher = crate::integrity::IntegrityHasher::for_domain(domain);
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

/// Verify or reproduce the released V1 checksum contract.
///
/// # Errors
///
/// Returns a schema-drift or resource-limit error when canonical encoding cannot be completed
/// within the supplied bound.
pub fn result_checksum_v1(
    schema: &Schema,
    batches: &[RecordBatch],
    maximum_encoding_bytes: usize,
) -> Result<ResultChecksumV1, ResultChecksumError> {
    result_checksum(
        schema,
        batches,
        maximum_encoding_bytes,
        crate::integrity::IntegrityDomain::QueryResultChecksumV1,
    )
}

/// Compute `ResultChecksumV2` over extension-typed and nested-list result schemas.
///
/// V2 deliberately preserves V1's order-independent row-multiset semantics while using a new
/// frozen integrity domain. V1 remains available for already-released artifacts.
///
/// # Errors
///
/// Returns a schema-drift or resource-limit error when extension-aware canonical encoding cannot
/// be completed within the supplied bound.
pub fn result_checksum_v2(
    schema: &Schema,
    batches: &[RecordBatch],
    maximum_encoding_bytes: usize,
) -> Result<ResultChecksumV2, ResultChecksumError> {
    let result = result_checksum(
        schema,
        batches,
        maximum_encoding_bytes,
        crate::integrity::IntegrityDomain::QueryResultChecksumV2,
    )?;
    Ok(ResultChecksumV2 {
        checksum: result.checksum,
        canonical_schema: result.canonical_schema,
        row_count: result.row_count,
    })
}

/// Compute the separate gate checksum after recursively validating admitted arrays.
///
/// The query-result V2 checksum remains immutable and replayable. Gate identity is derived in a
/// distinct versioned domain so a gate receipt can never be substituted for a delivered result.
///
/// # Errors
///
/// Returns the same schema, canonical-map, Arrow, and resource failures as V2.
pub fn gate_result_checksum_v1(
    schema: &Schema,
    batches: &[RecordBatch],
    maximum_encoding_bytes: usize,
) -> Result<GateResultChecksumV1, ResultChecksumError> {
    let result = result_checksum_v2(schema, batches, maximum_encoding_bytes)?;
    let checksum = gate_framed([
        GATE_RESULT_CHECKSUM_VERSION.as_bytes(),
        result.checksum.as_bytes(),
        result.canonical_schema.as_slice(),
        &result.row_count.to_be_bytes(),
    ]);
    Ok(GateResultChecksumV1 {
        checksum,
        canonical_schema: result.canonical_schema,
        row_count: result.row_count,
    })
}

/// Reproduce the checksum contract declared by a persisted result artifact.
///
/// New artifacts are always emitted as V2; V1 remains a verifier-only branch for artifacts that
/// were accepted before the extension-aware result schemas became active.
///
/// # Errors
///
/// Returns a schema-drift error for an unsupported version or propagates the selected version's
/// canonical-encoding failure.
pub fn result_checksum_for_version(
    version: &str,
    schema: &Schema,
    batches: &[RecordBatch],
    maximum_encoding_bytes: usize,
) -> Result<VersionedResultChecksum, ResultChecksumError> {
    match version {
        "ResultChecksumV1" => result_checksum_v1(schema, batches, maximum_encoding_bytes)
            .map(VersionedResultChecksum::V1),
        "ResultChecksumV2" => result_checksum_v2(schema, batches, maximum_encoding_bytes)
            .map(VersionedResultChecksum::V2),
        _ => Err(ResultChecksumError::SchemaDrift),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use arrow_array::types::Int8Type;
    use arrow_array::{
        ArrayRef, DictionaryArray, Float64Array, Int8Array, Int64Array, ListArray, MapArray,
        RecordBatchOptions, StringArray, StructArray,
    };
    use arrow_buffer::OffsetBuffer;
    use arrow_schema::{Field, UnionFields, UnionMode};

    use super::*;

    const LIMIT: usize = 1024 * 1024;

    fn int_batch(values: Vec<i64>) -> RecordBatch {
        RecordBatch::try_from_iter([("value", Arc::new(Int64Array::from(values)) as ArrayRef)])
            .unwrap()
    }

    #[test]
    fn result_checksum_v2_continuity() {
        let ordered = int_batch(vec![9, 4, 7, 4]);
        let repartitioned = int_batch(vec![4, 9, 4, 7]);
        let v1 = result_checksum_v1(
            ordered.schema().as_ref(),
            std::slice::from_ref(&ordered),
            LIMIT,
        )
        .unwrap();
        let v1_replay = result_checksum_for_version(
            "ResultChecksumV1",
            repartitioned.schema().as_ref(),
            &[repartitioned.slice(0, 1), repartitioned.slice(1, 3)],
            LIMIT,
        )
        .unwrap();
        assert_eq!(v1_replay, VersionedResultChecksum::V1(v1.clone()));

        let v2 = result_checksum_v2(
            ordered.schema().as_ref(),
            std::slice::from_ref(&ordered),
            LIMIT,
        )
        .unwrap();
        let v2_replay = result_checksum_for_version(
            "ResultChecksumV2",
            repartitioned.schema().as_ref(),
            &[repartitioned.slice(0, 2), repartitioned.slice(2, 2)],
            LIMIT,
        )
        .unwrap();
        assert_eq!(v2_replay, VersionedResultChecksum::V2(v2.clone()));
        assert_ne!(v1.checksum, v2.checksum);
        assert_eq!(v1.canonical_schema, v2.canonical_schema);
        assert_eq!(v1.row_count, v2.row_count);
        assert_eq!(
            v1.checksum,
            "b3:51aafc2a031f8581631c49268b8bb117c2bf2f38d4feaba1f986af969a00f5e9"
        );
        assert_eq!(
            v2.checksum,
            "b3:e5febaf4048ed5adc3995d37fd1d23d3955273acea29645240c1aa42deb5e3e9"
        );
        assert!(
            result_checksum_for_version(
                "ResultChecksumV3",
                ordered.schema().as_ref(),
                &[ordered],
                LIMIT,
            )
            .is_err()
        );
    }

    fn unordered_map_type() -> DataType {
        let entries = DataType::Struct(
            vec![
                Arc::new(Field::new("keys", DataType::Utf8, false)),
                Arc::new(Field::new("values", DataType::Int64, true)),
            ]
            .into(),
        );
        DataType::Map(Arc::new(Field::new("entries", entries, false)), false)
    }

    fn assert_rejects_nested_unordered_map(data_type: &DataType) {
        assert!(matches!(
            validate_data_type(data_type),
            Err(ResultChecksumError::UnorderedMap)
        ));
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

        assert_rejects_nested_unordered_map(&DataType::Struct(
            vec![Arc::new(Field::new("nested", unordered_map_type(), true))].into(),
        ));
        assert_rejects_nested_unordered_map(&DataType::Union(
            UnionFields::try_new(
                vec![0],
                vec![Arc::new(Field::new("nested", unordered_map_type(), true))],
            )
            .unwrap(),
            UnionMode::Sparse,
        ));
        assert_rejects_nested_unordered_map(&DataType::List(Arc::new(Field::new(
            "nested",
            unordered_map_type(),
            true,
        ))));
        assert_rejects_nested_unordered_map(&DataType::Dictionary(
            Box::new(DataType::Int8),
            Box::new(unordered_map_type()),
        ));

        let schema_only = result_checksum_v1(empty_schema.as_ref(), &[], LIMIT).unwrap();
        let exact_schema_limit = schema_only.canonical_schema.len();
        assert!(result_checksum_v1(empty_schema.as_ref(), &[], exact_schema_limit).is_ok());
        assert!(matches!(
            result_checksum_v1(empty_schema.as_ref(), &[], exact_schema_limit - 1),
            Err(ResultChecksumError::ResourceLimit)
        ));

        let row_batch = int_batch(vec![1]);
        let row_schema = row_batch.schema();
        let row_schema_only = result_checksum_v1(row_schema.as_ref(), &[], LIMIT).unwrap();
        let converter = RowConverter::new(
            row_schema
                .fields()
                .iter()
                .map(|field| SortField::new(field.data_type().clone()))
                .collect(),
        )
        .unwrap();
        let encoded = converter.convert_columns(row_batch.columns()).unwrap();
        let exact_row_limit = row_schema_only.canonical_schema.len() + encoded.row(0).data().len();
        assert!(
            result_checksum_v1(
                row_schema.as_ref(),
                std::slice::from_ref(&row_batch),
                exact_row_limit
            )
            .is_ok()
        );
        assert!(matches!(
            result_checksum_v1(
                row_schema.as_ref(),
                std::slice::from_ref(&row_batch),
                exact_row_limit - 1,
            ),
            Err(ResultChecksumError::ResourceLimit)
        ));
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

    #[test]
    fn canonical_batch_checksum_is_row_order_independent() {
        let first = int_batch(vec![9, 4, 7]);
        let reordered = int_batch(vec![7, 9, 4]);
        assert_eq!(
            batch_checksum(&first).unwrap(),
            batch_checksum(&reordered).unwrap()
        );
    }
}
