//! Compile- and test-only probes for the exact pinned dependency graph.
//!
//! This module is a library-compatibility assurance tier: it proves that selected public APIs
//! compile and that small known-answer behaviors remain available. It is not a CodeFabric runtime
//! contract, does not own production behavior, and must not be called from daemon request paths.
//! Production modules and their acceptance tests remain authoritative for application semantics.

use std::sync::Arc;

use arrow_array::{ArrayRef, Int32Array, RecordBatch};
use arrow_buffer::Buffer;
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use datafusion::datasource::MemTable;
use serde::{Deserialize, Serialize};

/// Small serializable record used to prove the stable utility stack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatibilityRecord {
    /// Stable identifier used by the smoke checks.
    pub id: u32,
}

/// Constructs an Arrow schema through the exact `59.2.0` public type universe.
#[must_use]
pub fn schema() -> SchemaRef {
    Arc::new(Schema::new(vec![Field::new("id", DataType::Int32, false)]))
}

/// Builds a batch and reads it through the sealed provider adapter.
///
/// # Errors
///
/// Returns a DataFusion error if batch construction or provider execution fails.
pub async fn provider_row_count() -> datafusion::error::Result<usize> {
    let schema = schema();
    let values: ArrayRef = Arc::new(Int32Array::from(vec![1_i32, 2_i32]));
    let batch = RecordBatch::try_new(Arc::clone(&schema), vec![values])?;
    let provider = MemTable::try_new(schema, vec![vec![batch]])?;
    let batches =
        crate::governed_session::collect_provider(crate::governed_session::ProviderReadRequest {
            provider: Arc::new(provider),
            filter: None,
            projection: None,
            limit: None,
        })
        .await?;
    Ok(batches.iter().map(RecordBatch::num_rows).sum())
}

/// Exercises direct Arrow-family crates that carry types across the data plane.
///
/// # Errors
///
/// Returns an Arrow error if a fixed compatibility kernel rejects its input.
pub fn arrow_family_probe() -> Result<(), arrow_schema::ArrowError> {
    let values = Int32Array::from(vec![3_i32, 1_i32, 2_i32]);
    let _buffer = Buffer::from_vec(vec![1_u8, 2_u8, 3_u8]);
    let sorted = arrow_ord::sort::sort(&values, None)?;
    let _cast = arrow_cast::cast(sorted.as_ref(), &DataType::Int64)?;
    let _concat = arrow_select::concat::concat(&[&values, &values])?;
    let _rows = arrow_row::RowConverter::new(vec![arrow_row::SortField::new(DataType::Int32)])?;
    let _length =
        arrow_string::length::length(&arrow_array::StringArray::from(vec!["codefabric"]))?;
    Ok(())
}

/// Exercises non-Arrow utilities in the stable dependency contract.
///
/// # Errors
///
/// Returns an error if the fixed compatibility values cannot be encoded or parsed.
pub fn utility_probe() -> Result<(), Box<dyn std::error::Error>> {
    let record = CompatibilityRecord { id: 7 };
    let encoded = serde_json::to_vec(&record)?;
    let decoded: CompatibilityRecord = serde_json::from_slice(&encoded)?;
    if record != decoded {
        return Err(std::io::Error::other("serde round-trip changed the record").into());
    }

    let digest = crate::integrity::digest_bytes(&encoded);
    if digest == [0_u8; 32] {
        return Err(std::io::Error::other("BLAKE3 returned the reserved zero digest").into());
    }
    let parsed = url::Url::parse("file:///tmp/codefabric")?;
    if parsed.scheme() != "file" {
        return Err(std::io::Error::other("URL parser changed the file scheme").into());
    }
    let _ready = futures::future::ready(record);
    let _runtime = tokio::runtime::Handle::try_current()?;
    tracing::trace!(target: "codefabric::compatibility", "stable utility graph ready");
    let _store = object_store::local::LocalFileSystem::new();
    let _writer_properties = parquet::file::properties::WriterProperties::builder().build();
    let _arrow_reexport = arrow::array::Int32Array::from(vec![1_i32]);
    Ok(())
}

/// Returns the number of object-hash algorithms compiled into the read-only Git profile.
#[must_use]
pub const fn git_hash_algorithm_count() -> usize {
    crate::git_state::supported_hash_algorithms().len()
}
