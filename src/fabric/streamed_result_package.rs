//! Manifest-last, object-store-backed Arrow result packages.
//!
//! DataFusion output is consumed as a stream. Each page is a fresh Arrow IPC stream that can be
//! decoded independently and bounded before the next page is accepted. Each page extends the
//! durable recovery intent before its create-only write; encoded pages never accumulate. The
//! canonical manifest is the final object published; its
//! presence is therefore the only sealed-package signal. No decoded semantic result is retained
//! as process-local rows.

use std::collections::BTreeSet;
use std::fmt;
use std::io::Cursor;
use std::num::{NonZeroU64, NonZeroUsize};
use std::ops::Range;
use std::sync::Arc;
use std::time::Instant;

use arrow_array::RecordBatch;
use arrow_ipc::reader::StreamReader;
use arrow_schema::{ArrowError, SchemaRef};
use async_trait::async_trait;
use datafusion::physical_plan::SendableRecordBatchStream;
use futures::StreamExt as _;
use object_store::path::Path as ObjectPath;
use object_store::{GetOptions, ObjectStore, ObjectStoreExt as _, PutMode, PutOptions};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::cancellation::Cancellation;
use crate::relational_program::RelationId;
use crate::resource_budget::{
    ChargedSlice, ChargedValue, ResourceAmounts, ResourceBudget, ResourceClass, ResourceReservation,
};

mod storage_ledger;
use super::bounded_encoding::{
    self, BoundedEncodingError, encode_page, page_size, preflight_schema,
};
use storage_ledger::ResultStorageLedger;

use super::arrow_result_resource::{
    QueryExecutionPin, ResultCompleteness, ResultCoverage, ResultResourceLease,
};
use super::command::EpochId;

/// Current immutable package contract.
pub const STREAMED_RESULT_PACKAGE_FORMAT: &str = "codefabric.streamed-result-package.v1";

/// Monotonically growing manifest-last object set recorded before each newly included write.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PendingResultObjectSet {
    pub manifest_object_path: String,
    pub page_object_paths: Vec<String>,
    pub epoch_id: String,
    pub query_execution: String,
}

/// Durable coordinator port used to close every pre-ResultReady crash window.
#[async_trait]
pub trait ResultPublicationIntentRecorder: fmt::Debug + Send + Sync {
    async fn record_publication_intent(
        &self,
        intent: PendingResultObjectSet,
    ) -> Result<(), ResultPublicationIntentError>;
}

#[derive(Clone, Copy, Debug, Error)]
#[error("durable result-publication intent was rejected")]
pub struct ResultPublicationIntentError;

/// Bounds that cover every page-local and package-wide allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(clippy::struct_field_names)] // Every dimension is an explicit maximum in this envelope.
pub struct StreamedResultPackageLimits {
    max_relations: NonZeroUsize,
    max_pages: NonZeroUsize,
    max_page_rows: NonZeroUsize,
    max_page_bytes: NonZeroUsize,
    max_total_rows: NonZeroU64,
    max_total_bytes: NonZeroU64,
    max_manifest_bytes: NonZeroUsize,
    max_provenance_entries: NonZeroUsize,
    max_provenance_bytes: NonZeroUsize,
}

impl StreamedResultPackageLimits {
    /// Construct a package envelope with no implicit or unbounded dimension.
    ///
    /// # Errors
    /// Rejects zero limits.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        max_relations: usize,
        max_pages: usize,
        max_page_rows: usize,
        max_page_bytes: usize,
        max_total_rows: u64,
        max_total_bytes: u64,
        max_manifest_bytes: usize,
        max_provenance_entries: usize,
        max_provenance_bytes: usize,
    ) -> Result<Self, StreamedResultPackageError> {
        Ok(Self {
            max_relations: NonZeroUsize::new(max_relations)
                .ok_or(StreamedResultPackageError::InvalidLimit("max_relations"))?,
            max_pages: NonZeroUsize::new(max_pages)
                .ok_or(StreamedResultPackageError::InvalidLimit("max_pages"))?,
            max_page_rows: NonZeroUsize::new(max_page_rows)
                .ok_or(StreamedResultPackageError::InvalidLimit("max_page_rows"))?,
            max_page_bytes: NonZeroUsize::new(max_page_bytes)
                .ok_or(StreamedResultPackageError::InvalidLimit("max_page_bytes"))?,
            max_total_rows: NonZeroU64::new(max_total_rows)
                .ok_or(StreamedResultPackageError::InvalidLimit("max_total_rows"))?,
            max_total_bytes: NonZeroU64::new(max_total_bytes)
                .ok_or(StreamedResultPackageError::InvalidLimit("max_total_bytes"))?,
            max_manifest_bytes: NonZeroUsize::new(max_manifest_bytes).ok_or(
                StreamedResultPackageError::InvalidLimit("max_manifest_bytes"),
            )?,
            max_provenance_entries: NonZeroUsize::new(max_provenance_entries).ok_or(
                StreamedResultPackageError::InvalidLimit("max_provenance_entries"),
            )?,
            max_provenance_bytes: NonZeroUsize::new(max_provenance_bytes).ok_or(
                StreamedResultPackageError::InvalidLimit("max_provenance_bytes"),
            )?,
        })
    }

    #[must_use]
    pub const fn max_page_bytes(self) -> usize {
        self.max_page_bytes.get()
    }
}

/// One bounded causal provenance observation included in the package rather than an event.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResultProvenance {
    pub kind: String,
    pub identity: String,
}

/// One streamed relation and the release-owned coverage/provenance attached to it.
pub struct StreamedRelationInput {
    pub relation_id: RelationId,
    pub schema: SchemaRef,
    pub stream: SendableRecordBatchStream,
    pub max_rows: u64,
    pub coverage: ResultCoverage,
    pub provenance: Vec<ResultProvenance>,
    pub row_selection: Option<StreamedRowSelection>,
}

/// A final result selection, after all authorized filters and deterministic ordering.
#[derive(Clone, Debug)]
pub struct StreamedRowSelection {
    pub query_id: String,
    pub maximum_rows: u64,
    /// The native plan fetched up to `maximum_rows + 1`, within its execution grant.
    pub exhaustion_probe: bool,
}

impl fmt::Debug for StreamedRelationInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StreamedRelationInput")
            .field("relation_id", &self.relation_id)
            .field("schema", &self.schema)
            .field("max_rows", &self.max_rows)
            .field("coverage", &self.coverage)
            .field("provenance", &self.provenance)
            .finish_non_exhaustive()
    }
}

/// Exact page entry in the sealed manifest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResultPageManifestEntry {
    pub relation_id: String,
    pub page_ordinal: u64,
    pub object_path: String,
    pub row_count: u64,
    pub batch_count: u64,
    pub byte_length: u64,
    pub schema_checksum: String,
    pub content_checksum: String,
}

/// Relation-level coverage and provenance over an ordered page subsequence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResultRelationManifestEntry {
    pub relation_id: String,
    pub page_start: u64,
    pub page_count: u64,
    pub row_count: u64,
    pub coverage_state: String,
    pub requested_units: u64,
    pub completed_units: u64,
    pub remainder_units: u64,
    pub unknown_cause: Option<String>,
    pub provenance: Vec<ResultProvenance>,
}

/// Canonical semantic envelope plus the ordered immutable page inventory.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StreamedResultPackageManifest {
    pub format: String,
    pub epoch_id: String,
    pub query_execution: String,
    pub canonical_semantic_response: serde_json::Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub processing: Vec<super::processing_status::QueryProcessing>,
    pub total_rows: u64,
    pub total_pages: u64,
    pub total_bytes: u64,
    pub relations: Vec<ResultRelationManifestEntry>,
    pub pages: Vec<ResultPageManifestEntry>,
}

/// Exact create/read/delete capabilities required from result object storage.
#[async_trait]
pub trait ResultObjectSink: fmt::Debug + Send + Sync + 'static {
    async fn create(&self, path: &ObjectPath, bytes: Vec<u8>) -> Result<(), object_store::Error>;
    async fn size(&self, path: &ObjectPath) -> Result<u64, object_store::Error>;
    /// Return exactly the requested range, never a full-object fallback.
    async fn read_range(
        &self,
        path: &ObjectPath,
        range: Range<u64>,
    ) -> Result<Vec<u8>, object_store::Error>;
    async fn delete(&self, path: &ObjectPath) -> Result<(), object_store::Error>;
}

/// Native `object_store` implementation. `PutMode::Create` is the publication primitive.
#[derive(Clone)]
pub struct ObjectStoreResultSink {
    store: Arc<dyn ObjectStore>,
}

impl ObjectStoreResultSink {
    #[must_use]
    pub fn new(store: Arc<dyn ObjectStore>) -> Self {
        Self { store }
    }
}

impl fmt::Debug for ObjectStoreResultSink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ObjectStoreResultSink")
            .field("store", &self.store.to_string())
            .finish()
    }
}

#[async_trait]
impl ResultObjectSink for ObjectStoreResultSink {
    async fn create(&self, path: &ObjectPath, bytes: Vec<u8>) -> Result<(), object_store::Error> {
        self.store
            .put_opts(
                path,
                bytes.into(),
                PutOptions {
                    mode: PutMode::Create,
                    ..PutOptions::default()
                },
            )
            .await?;
        Ok(())
    }

    async fn size(&self, path: &ObjectPath) -> Result<u64, object_store::Error> {
        Ok(self.store.head(path).await?.size)
    }

    async fn read_range(
        &self,
        path: &ObjectPath,
        range: Range<u64>,
    ) -> Result<Vec<u8>, object_store::Error> {
        if range.start > range.end {
            return Err(range_error("reversed object range"));
        }
        let result = self
            .store
            .get_opts(path, GetOptions::default().with_range(Some(range.clone())))
            .await?;
        if result.range != range {
            return Err(range_error("object store returned an unexpected range"));
        }
        let length =
            usize::try_from(range.end - range.start).map_err(|_| range_error("range overflow"))?;
        let mut bytes = Vec::with_capacity(length);
        let mut stream = result.into_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if chunk.len() > length - bytes.len() {
                return Err(range_error("object store exceeded the requested range"));
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.len() != length {
            return Err(range_error("object store truncated the requested range"));
        }
        Ok(bytes)
    }

    async fn delete(&self, path: &ObjectPath) -> Result<(), object_store::Error> {
        self.store.delete(path).await
    }
}

/// A proved sealed package. It retains metadata and an object capability, never page bytes.
#[derive(Clone)]
pub struct SealedStreamedResultPackage {
    epoch_id: EpochId,
    query_execution: QueryExecutionPin,
    manifest_path: ObjectPath,
    manifest_checksum: [u8; 32],
    manifest_byte_length: u64,
    manifest: ChargedValue<StreamedResultPackageManifest>,
    lease: ResultResourceLease,
    sink: Arc<dyn ResultObjectSink>,
    budget: ResourceBudget,
    storage: Arc<ResultStorageLedger>,
}

impl fmt::Debug for SealedStreamedResultPackage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SealedStreamedResultPackage")
            .field("manifest_path", &self.manifest_path)
            .field("epoch_id", &self.epoch_id)
            .field("manifest_checksum", &hex(&self.manifest_checksum))
            .field("manifest_byte_length", &self.manifest_byte_length)
            .field("manifest", &self.manifest)
            .field("lease", &self.lease)
            .finish_non_exhaustive()
    }
}

impl SealedStreamedResultPackage {
    pub(crate) fn resource_budget(&self) -> &ResourceBudget {
        &self.budget
    }
    #[must_use]
    pub const fn epoch_id(&self) -> EpochId {
        self.epoch_id
    }

    #[must_use]
    pub const fn query_execution(&self) -> QueryExecutionPin {
        self.query_execution
    }

    #[must_use]
    pub fn manifest(&self) -> &StreamedResultPackageManifest {
        &self.manifest
    }

    #[must_use]
    pub const fn manifest_path(&self) -> &ObjectPath {
        &self.manifest_path
    }

    #[must_use]
    pub const fn manifest_checksum(&self) -> &[u8; 32] {
        &self.manifest_checksum
    }

    #[must_use]
    pub const fn manifest_byte_length(&self) -> u64 {
        self.manifest_byte_length
    }

    #[must_use]
    pub fn retained_object_bytes(&self) -> u64 {
        self.manifest
            .total_bytes
            .saturating_add(self.manifest_byte_length)
    }

    #[must_use]
    pub const fn lease(&self) -> ResultResourceLease {
        self.lease
    }

    /// Read and independently decode exactly one manifest-authorized page.
    ///
    /// # Errors
    /// Rejects expired leases, exhausted control capacity, missing objects, and integrity drift.
    pub async fn read_page(
        &self,
        page_ordinal: u64,
        observed_at_unix_ms: i64,
    ) -> Result<ChargedSlice<u8>, StreamedResultPackageError> {
        if observed_at_unix_ms < self.lease.issued_at_unix_ms()
            || observed_at_unix_ms >= self.lease.expires_at_unix_ms()
        {
            return Err(StreamedResultPackageError::Expired);
        }
        let page = self
            .manifest
            .pages
            .get(
                usize::try_from(page_ordinal)
                    .map_err(|_| StreamedResultPackageError::UnknownPage(page_ordinal))?,
            )
            .ok_or(StreamedResultPackageError::UnknownPage(page_ordinal))?;
        let charge = reserve_memory(&self.budget, ResourceClass::Control, page.byte_length)?;
        let _decode = reserve_memory(
            &self.budget,
            ResourceClass::Control,
            (self.storage.limits.max_page_bytes.get() as u64)
                .checked_mul(16)
                .and_then(|n| n.checked_add(65_536))
                .ok_or(StreamedResultPackageError::CounterOverflow)?,
        )?;
        let bytes = read_exact(
            self.sink.as_ref(),
            &ObjectPath::from(page.object_path.clone()),
            page.byte_length,
        )
        .await?;
        validate_page(page, &bytes, self.storage.limits.max_page_bytes.get())?;
        charge
            .into_charged_vec(bytes)
            .map_err(StreamedResultPackageError::Resource)
    }

    /// Verify the complete immutable page in bounded ranges, retaining only the requested chunk.
    /// No IPC decoder or whole-page allocation is needed after the package has been proved.
    ///
    /// # Errors
    /// Rejects invalid ranges, expired leases, exhausted control capacity, and object corruption.
    pub async fn read_page_range(
        &self,
        page_ordinal: u64,
        offset: u64,
        maximum_bytes: usize,
        observed_at_unix_ms: i64,
    ) -> Result<ChargedSlice<u8>, StreamedResultPackageError> {
        if observed_at_unix_ms < self.lease.issued_at_unix_ms()
            || observed_at_unix_ms >= self.lease.expires_at_unix_ms()
        {
            return Err(StreamedResultPackageError::Expired);
        }
        let page = self
            .manifest
            .pages
            .get(
                usize::try_from(page_ordinal)
                    .map_err(|_| StreamedResultPackageError::UnknownPage(page_ordinal))?,
            )
            .ok_or(StreamedResultPackageError::UnknownPage(page_ordinal))?;
        if maximum_bytes == 0 || offset > page.byte_length {
            return Err(StreamedResultPackageError::InvalidReadRange);
        }
        let end = offset
            .saturating_add(maximum_bytes as u64)
            .min(page.byte_length);
        let output_length = usize::try_from(end - offset)
            .map_err(|_| StreamedResultPackageError::CounterOverflow)?;
        let output_charge =
            reserve_memory(&self.budget, ResourceClass::Control, output_length as u64)?;
        let _scratch = reserve_memory(&self.budget, ResourceClass::Control, maximum_bytes as u64)?;
        let path = ObjectPath::from(page.object_path.clone());
        if self.sink.size(&path).await? != page.byte_length {
            return Err(StreamedResultPackageError::PageIntegrity(page_ordinal));
        }
        let mut output = Vec::with_capacity(output_length);
        let mut hasher = blake3::Hasher::new();
        let mut position = 0;
        while position < page.byte_length {
            let range_end = position
                .saturating_add(maximum_bytes as u64)
                .min(page.byte_length);
            let bytes = self.sink.read_range(&path, position..range_end).await?;
            if bytes.len() as u64 != range_end - position {
                return Err(StreamedResultPackageError::PageIntegrity(page_ordinal));
            }
            hasher.update(&bytes);
            let selected_start = offset.max(position);
            let selected_end = end.min(range_end);
            if selected_start < selected_end {
                let start = usize::try_from(selected_start - position)
                    .map_err(|_| StreamedResultPackageError::CounterOverflow)?;
                let end = usize::try_from(selected_end - position)
                    .map_err(|_| StreamedResultPackageError::CounterOverflow)?;
                output.extend_from_slice(&bytes[start..end]);
            }
            position = range_end;
        }
        if hex(hasher.finalize().as_bytes()) != page.content_checksum
            || self.sink.size(&path).await? != page.byte_length
        {
            return Err(StreamedResultPackageError::PageIntegrity(page_ordinal));
        }
        output_charge
            .into_charged_vec(output)
            .map_err(StreamedResultPackageError::Resource)
    }

    /// Delete a sealed package only after its owning retention policy has made it unreachable.
    ///
    /// # Errors
    /// Propagates storage failures; already absent objects are idempotent success.
    pub async fn delete_objects(self) -> Result<(), StreamedResultPackageError> {
        let pages = self
            .manifest
            .pages
            .iter()
            .map(|page| ObjectPath::from(page.object_path.clone()))
            .collect::<Vec<_>>();
        delete_exact_object_set(
            &self.storage,
            &pages,
            &self.manifest_path,
            self.storage.limits,
        )
        .await
    }
}

async fn delete_exact_object_set(
    storage: &ResultStorageLedger,
    page_paths: &[ObjectPath],
    manifest_path: &ObjectPath,
    limits: StreamedResultPackageLimits,
) -> Result<(), StreamedResultPackageError> {
    if page_paths.len() > limits.max_pages.get() {
        return Err(StreamedResultPackageError::ManifestShape);
    }
    for path in page_paths {
        storage
            .delete(path, limits.max_page_bytes.get() as u64)
            .await?;
    }
    storage
        .delete(manifest_path, limits.max_manifest_bytes.get() as u64)
        .await?;
    Ok(())
}

fn range_error(message: &'static str) -> object_store::Error {
    object_store::Error::Generic {
        store: "bounded-result-range",
        source: Box::new(std::io::Error::other(message)),
    }
}

fn reserve_memory(
    budget: &ResourceBudget,
    class: ResourceClass,
    bytes: u64,
) -> Result<ResourceReservation, StreamedResultPackageError> {
    budget
        .try_reserve(
            class,
            ResourceAmounts {
                memory_bytes: bytes,
                ..ResourceAmounts::default()
            },
        )
        .map_err(StreamedResultPackageError::Resource)
}

async fn read_exact(
    sink: &dyn ResultObjectSink,
    path: &ObjectPath,
    length: u64,
) -> Result<Vec<u8>, StreamedResultPackageError> {
    if sink.size(path).await? != length {
        return Err(StreamedResultPackageError::ObjectLength);
    }
    let bytes = sink.read_range(path, 0..length).await?;
    if bytes.len() as u64 != length {
        return Err(StreamedResultPackageError::ObjectLength);
    }
    Ok(bytes)
}

fn add_manifest_entry_size(
    value: &impl Serialize,
    total: &mut usize,
    limit: usize,
) -> Result<(), StreamedResultPackageError> {
    let mut counter = bounded_encoding::CappedWriter::counting(limit);
    serde_json::to_writer(&mut counter, value).map_err(|_| {
        StreamedResultPackageError::ManifestLimit {
            observed: limit.saturating_add(1),
            limit,
        }
    })?;
    *total = total
        .checked_add(counter.count())
        .and_then(|n| n.checked_add(1))
        .ok_or(StreamedResultPackageError::CounterOverflow)?;
    if *total > limit {
        return Err(StreamedResultPackageError::ManifestLimit {
            observed: *total,
            limit,
        });
    }
    Ok(())
}

fn bounded_manifest_bytes(
    value: &impl Serialize,
    limit: usize,
) -> Result<Vec<u8>, StreamedResultPackageError> {
    // Check scalar/string/container size without constructing the canonicalizer's sorted-object
    // buffers. Their subsequent bounded expansion is included in the metadata reservation.
    let mut size = 0;
    add_manifest_entry_size(value, &mut size, limit)?;
    let bytes = serde_json_canonicalizer::to_vec(value)
        .map_err(StreamedResultPackageError::CanonicalManifest)?;
    if bytes.len() > limit {
        return Err(StreamedResultPackageError::ManifestLimit {
            observed: bytes.len(),
            limit,
        });
    }
    Ok(bytes)
}

/// Manifest-last page sealer with shared exact-object storage ownership.
#[derive(Clone, Debug)]
pub struct StreamedResultPackageBuilder {
    sink: Arc<dyn ResultObjectSink>,
    limits: StreamedResultPackageLimits,
    budget: ResourceBudget,
    storage: Arc<ResultStorageLedger>,
}

impl StreamedResultPackageBuilder {
    #[must_use]
    pub fn new(
        sink: Arc<dyn ResultObjectSink>,
        limits: StreamedResultPackageLimits,
        budget: ResourceBudget,
    ) -> Self {
        Self {
            storage: Arc::new(ResultStorageLedger::new(
                Arc::clone(&sink),
                budget.clone(),
                limits,
            )),
            sink,
            limits,
            budget,
        }
    }

    /// Idempotently delete one already-validated private object set, keeping the manifest last.
    /// This is the crash-recovery counterpart to manifest-last publication: a durable cleanup
    /// locator can finish deletion without reconstructing process-local package state.
    ///
    /// # Errors
    /// Propagates storage failures; already absent objects are idempotent success.
    pub async fn delete_retained_object_set(
        &self,
        page_paths: &[ObjectPath],
        manifest_path: &ObjectPath,
    ) -> Result<(), StreamedResultPackageError> {
        delete_exact_object_set(&self.storage, page_paths, manifest_path, self.limits).await
    }

    /// Consume DataFusion streams and publish their manifest last.
    ///
    /// # Errors
    /// Rejects invalid inputs, exhausted bounds, cancellation, deadlines, journal or storage faults.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub async fn seal(
        &self,
        epoch_id: EpochId,
        query_execution: QueryExecutionPin,
        canonical_semantic_response: &[u8],
        relations: Vec<StreamedRelationInput>,
        lease: ResultResourceLease,
        cancellation: &Cancellation,
        deadline: Instant,
        publication_intent: &dyn ResultPublicationIntentRecorder,
    ) -> Result<SealedStreamedResultPackage, StreamedResultPackageError> {
        self.seal_with_processing(
            epoch_id,
            query_execution,
            canonical_semantic_response,
            Vec::new(),
            relations,
            lease,
            cancellation,
            deadline,
            publication_intent,
        )
        .await
    }

    /// Seal query-scoped processing with the same immutable snapshot and result lease.
    ///
    /// # Errors
    /// Same bounded publication failures as [`Self::seal`].
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(crate) async fn seal_with_processing(
        &self,
        epoch_id: EpochId,
        query_execution: QueryExecutionPin,
        canonical_semantic_response: &[u8],
        mut processing: Vec<super::processing_status::QueryProcessing>,
        mut relations: Vec<StreamedRelationInput>,
        lease: ResultResourceLease,
        cancellation: &Cancellation,
        deadline: Instant,
        publication_intent: &dyn ResultPublicationIntentRecorder,
    ) -> Result<SealedStreamedResultPackage, StreamedResultPackageError> {
        validate_pins(epoch_id, query_execution)?;
        check_cancel_deadline(cancellation, deadline)?;
        if canonical_semantic_response.len() > self.limits.max_manifest_bytes.get() {
            return Err(StreamedResultPackageError::ManifestLimit {
                observed: canonical_semantic_response.len(),
                limit: self.limits.max_manifest_bytes.get(),
            });
        }
        // Explicit conservative working envelopes include Serde/flatbuffer container overhead,
        // duplicated journal metadata, and Arrow dictionary/offset/null scratch. They are charged
        // to the shared owner, not a newly created per-package budget or a claim about process RSS.
        let mut metadata_charge = reserve_memory(
            &self.budget,
            ResourceClass::Data,
            (self.limits.max_manifest_bytes.get() as u64)
                .checked_mul(64)
                .ok_or(StreamedResultPackageError::CounterOverflow)?,
        )?;
        let _encoding_charge = reserve_memory(
            &self.budget,
            ResourceClass::Data,
            (self.limits.max_page_bytes.get() as u64)
                .checked_mul(16)
                .and_then(|n| n.checked_add(65_536))
                .ok_or(StreamedResultPackageError::CounterOverflow)?,
        )?;
        if relations.is_empty() || relations.len() > self.limits.max_relations.get() {
            return Err(StreamedResultPackageError::RelationLimit {
                observed: relations.len(),
                limit: self.limits.max_relations.get(),
            });
        }
        let response: serde_json::Value = serde_json::from_slice(canonical_semantic_response)
            .map_err(StreamedResultPackageError::CanonicalResponse)?;
        let recanonicalized = serde_json_canonicalizer::to_vec(&response)
            .map_err(StreamedResultPackageError::CanonicalResponse)?;
        if recanonicalized != canonical_semantic_response {
            return Err(StreamedResultPackageError::NonCanonicalResponse);
        }
        relations.sort_by(|left, right| left.relation_id.cmp(&right.relation_id));
        for pair in relations.windows(2) {
            if pair[0].relation_id == pair[1].relation_id {
                return Err(StreamedResultPackageError::DuplicateRelation(
                    pair[0].relation_id.as_str().to_owned(),
                ));
            }
        }

        let query_hex = hex(query_execution.as_bytes());
        let epoch_hex = hex(epoch_id.as_bytes());
        let manifest_path =
            ObjectPath::from(format!("packages/{epoch_hex}/{query_hex}/manifest.json"));
        let mut created = Vec::<ObjectPath>::new();
        let sealed = async {
            let mut pages = Vec::new();
            let mut metadata_bytes = canonical_semantic_response.len().saturating_add(512);
            let mut relation_entries = Vec::with_capacity(relations.len());
            let mut total_rows = 0_u64;
            let mut total_bytes = 0_u64;

            for mut relation in relations {
                check_cancel_deadline(cancellation, deadline)?;
                if relation.max_rows == 0 {
                    return Err(StreamedResultPackageError::InvalidRelationRowLimit(
                        relation.relation_id.as_str().to_owned(),
                    ));
                }
                validate_provenance(&relation.provenance, self.limits)?;
                preflight_schema(&relation.schema, self.limits.max_page_bytes.get())?;
                if relation.stream.schema().as_ref() != relation.schema.as_ref() {
                    return Err(StreamedResultPackageError::StreamSchemaDrift(
                        relation.relation_id.as_str().to_owned(),
                    ));
                }
                let page_start = u64::try_from(pages.len())
                    .map_err(|_| StreamedResultPackageError::CounterOverflow)?;
                let mut relation_rows = 0_u64;
                let mut observed_rows = 0_u64;
                let mut relation_pages = 0_u64;
                let mut saw_batch = false;
                while let Some(batch) =
                    next_batch(&mut relation.stream, cancellation, deadline).await?
                {
                    check_cancel_deadline(cancellation, deadline)?;
                    let batch = batch.map_err(StreamedResultPackageError::DataFusion)?;
                    // A conservative additional residency envelope covers this received batch
                    // and its zero-copy page slices. Upstream native Arrow ownership may already
                    // charge the backing; we do not replace an unknown upstream owner or claim
                    // unique-allocation/RSS accounting. Slices within this stage share the charge.
                    let _input_charge = reserve_memory(
                        &self.budget,
                        ResourceClass::Data,
                        batch.get_array_memory_size() as u64,
                    )?;
                    saw_batch = true;
                    if batch.schema().as_ref() != relation.schema.as_ref() {
                        return Err(StreamedResultPackageError::StreamSchemaDrift(
                            relation.relation_id.as_str().to_owned(),
                        ));
                    }
                    observed_rows = observed_rows
                        .checked_add(batch.num_rows() as u64)
                        .ok_or(StreamedResultPackageError::CounterOverflow)?;
                    if observed_rows > relation.max_rows {
                        return Err(StreamedResultPackageError::RelationRowLimit {
                            relation: relation.relation_id.as_str().to_owned(),
                            observed: observed_rows,
                            limit: relation.max_rows,
                        });
                    }
                    let batch = if let Some(selection) = &relation.row_selection {
                        let remaining = selection.maximum_rows.saturating_sub(relation_rows);
                        let selected = batch
                            .num_rows()
                            .min(usize::try_from(remaining).unwrap_or(usize::MAX));
                        if selected == 0 && batch.num_rows() != 0 {
                            continue;
                        }
                        batch.slice(0, selected)
                    } else {
                        batch
                    };
                    let slices = bounded_batch_slices(
                        &relation.schema,
                        &batch,
                        self.limits.max_page_rows.get(),
                        self.limits.max_page_bytes.get(),
                    )?;
                    for slice in slices {
                        let (page_batch, encoded) = slice?;
                        check_cancel_deadline(cancellation, deadline)?;
                        let (page, object_path, encoded) = self.prepare_page(
                            epoch_id,
                            query_execution,
                            &relation.relation_id,
                            &relation.schema,
                            u64::try_from(pages.len())
                                .map_err(|_| StreamedResultPackageError::CounterOverflow)?,
                            &page_batch,
                            encoded,
                        )?;
                        relation_rows = relation_rows
                            .checked_add(page.row_count)
                            .ok_or(StreamedResultPackageError::CounterOverflow)?;
                        if relation_rows > relation.max_rows {
                            return Err(StreamedResultPackageError::RelationRowLimit {
                                relation: relation.relation_id.as_str().to_owned(),
                                observed: relation_rows,
                                limit: relation.max_rows,
                            });
                        }
                        total_rows = total_rows
                            .checked_add(page.row_count)
                            .ok_or(StreamedResultPackageError::CounterOverflow)?;
                        total_bytes = total_bytes
                            .checked_add(page.byte_length)
                            .ok_or(StreamedResultPackageError::CounterOverflow)?;
                        relation_pages = relation_pages
                            .checked_add(1)
                            .ok_or(StreamedResultPackageError::CounterOverflow)?;
                        enforce_package_totals(
                            pages.len().saturating_add(1),
                            total_rows,
                            total_bytes,
                            self.limits,
                        )?;
                        add_manifest_entry_size(
                            &page,
                            &mut metadata_bytes,
                            self.limits.max_manifest_bytes.get(),
                        )?;
                        pages.push(page);
                        self.publish_page(
                            &manifest_path,
                            &pages,
                            epoch_id,
                            query_execution,
                            &object_path,
                            encoded,
                            publication_intent,
                        )
                        .await?;
                        created.push(object_path);
                    }
                }
                if let Some(selection) = &relation.row_selection {
                    let summary = processing
                        .iter_mut()
                        .find(|summary| summary.query_id == selection.query_id)
                        .ok_or(StreamedResultPackageError::ManifestShape)?;
                    if summary.maximum_rows != Some(selection.maximum_rows)
                        || selection.maximum_rows == 0
                    {
                        return Err(StreamedResultPackageError::ManifestShape);
                    }
                    summary.additional_rows = if observed_rows > selection.maximum_rows {
                        Some(true)
                    } else if observed_rows < selection.maximum_rows || selection.exhaustion_probe {
                        Some(false)
                    } else {
                        None
                    };
                }
                if !saw_batch {
                    let empty = RecordBatch::new_empty(Arc::clone(&relation.schema));
                    let encoded = encode_page(
                        &relation.schema,
                        std::slice::from_ref(&empty),
                        self.limits.max_page_bytes.get(),
                    )?;
                    let (page, object_path, encoded) = self.prepare_page(
                        epoch_id,
                        query_execution,
                        &relation.relation_id,
                        &relation.schema,
                        u64::try_from(pages.len())
                            .map_err(|_| StreamedResultPackageError::CounterOverflow)?,
                        &empty,
                        encoded,
                    )?;
                    total_bytes = total_bytes
                        .checked_add(page.byte_length)
                        .ok_or(StreamedResultPackageError::CounterOverflow)?;
                    relation_pages = 1;
                    enforce_package_totals(
                        pages.len().saturating_add(1),
                        total_rows,
                        total_bytes,
                        self.limits,
                    )?;
                    add_manifest_entry_size(
                        &page,
                        &mut metadata_bytes,
                        self.limits.max_manifest_bytes.get(),
                    )?;
                    pages.push(page);
                    self.publish_page(
                        &manifest_path,
                        &pages,
                        epoch_id,
                        query_execution,
                        &object_path,
                        encoded,
                        publication_intent,
                    )
                    .await?;
                    created.push(object_path);
                }
                let relation_entry = ResultRelationManifestEntry {
                    relation_id: relation.relation_id.as_str().to_owned(),
                    page_start,
                    page_count: relation_pages,
                    row_count: relation_rows,
                    coverage_state: completeness_name(relation.coverage.state()).to_owned(),
                    requested_units: relation.coverage.requested_units(),
                    completed_units: relation.coverage.completed_units(),
                    remainder_units: relation.coverage.remainder_units(),
                    unknown_cause: relation
                        .coverage
                        .unknown_cause()
                        .map(|cause| cause.as_str().to_owned()),
                    provenance: relation.provenance,
                };
                add_manifest_entry_size(
                    &relation_entry,
                    &mut metadata_bytes,
                    self.limits.max_manifest_bytes.get(),
                )?;
                relation_entries.push(relation_entry);
            }

            let manifest = StreamedResultPackageManifest {
                format: STREAMED_RESULT_PACKAGE_FORMAT.to_owned(),
                epoch_id: hex(epoch_id.as_bytes()),
                query_execution: query_hex,
                canonical_semantic_response: response,
                processing,
                total_rows,
                total_pages: u64::try_from(pages.len())
                    .map_err(|_| StreamedResultPackageError::CounterOverflow)?,
                total_bytes,
                relations: relation_entries,
                pages,
            };
            validate_manifest(&manifest, self.limits)?;
            let manifest_bytes =
                bounded_manifest_bytes(&manifest, self.limits.max_manifest_bytes.get())?;
            if manifest_bytes.len() > self.limits.max_manifest_bytes.get() {
                return Err(StreamedResultPackageError::ManifestLimit {
                    observed: manifest_bytes.len(),
                    limit: self.limits.max_manifest_bytes.get(),
                });
            }
            let manifest_checksum = digest(&manifest_bytes);
            let manifest_byte_length = u64::try_from(manifest_bytes.len())
                .map_err(|_| StreamedResultPackageError::CounterOverflow)?;
            let retained_metadata_bytes = manifest_byte_length
                .checked_mul(64)
                .ok_or(StreamedResultPackageError::CounterOverflow)?;
            metadata_charge.shrink(ResourceAmounts {
                memory_bytes: metadata_charge
                    .amounts()
                    .memory_bytes
                    .saturating_sub(retained_metadata_bytes),
                ..ResourceAmounts::default()
            })?;
            check_cancel_deadline(cancellation, deadline)?;
            self.storage.create(&manifest_path, manifest_bytes).await?;
            Ok(SealedStreamedResultPackage {
                epoch_id,
                query_execution,
                manifest_path,
                manifest_checksum,
                manifest_byte_length,
                manifest: metadata_charge.into_charged_value(manifest),
                lease,
                sink: Arc::clone(&self.sink),
                budget: self.budget.clone(),
                storage: Arc::clone(&self.storage),
            })
        }
        .await;

        if sealed.is_err() {
            for path in created.into_iter().rev() {
                let _ = self
                    .storage
                    .delete(&path, self.limits.max_page_bytes.get() as u64)
                    .await;
            }
        }
        sealed
    }

    #[allow(clippy::too_many_arguments)]
    async fn publish_page(
        &self,
        manifest_path: &ObjectPath,
        pages: &[ResultPageManifestEntry],
        epoch: EpochId,
        query: QueryExecutionPin,
        path: &ObjectPath,
        bytes: Vec<u8>,
        recorder: &dyn ResultPublicationIntentRecorder,
    ) -> Result<(), StreamedResultPackageError> {
        recorder
            .record_publication_intent(PendingResultObjectSet {
                manifest_object_path: manifest_path.to_string(),
                page_object_paths: pages.iter().map(|page| page.object_path.clone()).collect(),
                epoch_id: hex(epoch.as_bytes()),
                query_execution: hex(query.as_bytes()),
            })
            .await?;
        self.storage.create(path, bytes).await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_page(
        &self,
        epoch_id: EpochId,
        query_execution: QueryExecutionPin,
        relation_id: &RelationId,
        schema: &SchemaRef,
        page_ordinal: u64,
        batch: &RecordBatch,
        encoded: Vec<u8>,
    ) -> Result<(ResultPageManifestEntry, ObjectPath, Vec<u8>), StreamedResultPackageError> {
        if encoded.len() > self.limits.max_page_bytes.get() {
            return Err(StreamedResultPackageError::PageByteLimit {
                observed: encoded.len(),
                limit: self.limits.max_page_bytes.get(),
            });
        }
        let schema_bytes = encode_page(schema, &[], self.limits.max_page_bytes.get())?;
        let schema_checksum = digest(&schema_bytes);
        let content_checksum = digest(&encoded);
        let relation_hex = hex(relation_id.as_str().as_bytes());
        let object_path = ObjectPath::from(format!(
            "packages/{}/{}/pages/{relation_hex}/{page_ordinal:020}.arrow",
            hex(epoch_id.as_bytes()),
            hex(query_execution.as_bytes())
        ));
        let entry = ResultPageManifestEntry {
            relation_id: relation_id.as_str().to_owned(),
            page_ordinal,
            object_path: object_path.to_string(),
            row_count: u64::try_from(batch.num_rows())
                .map_err(|_| StreamedResultPackageError::CounterOverflow)?,
            batch_count: 1,
            byte_length: u64::try_from(encoded.len())
                .map_err(|_| StreamedResultPackageError::CounterOverflow)?,
            schema_checksum: hex(&schema_checksum),
            content_checksum: hex(&content_checksum),
        };
        validate_page(&entry, &encoded, self.limits.max_page_bytes.get())?;
        Ok((entry, object_path, encoded))
    }

    /// Reopen one exact manifest and prove every page before returning serving authority.
    ///
    /// # Errors
    /// Rejects exhausted control capacity, invalid manifests, missing objects, and integrity drift.
    pub async fn reopen(
        &self,
        manifest_path: ObjectPath,
        expected_epoch: EpochId,
        expected_query: QueryExecutionPin,
        lease: ResultResourceLease,
    ) -> Result<SealedStreamedResultPackage, StreamedResultPackageError> {
        validate_pins(expected_epoch, expected_query)?;
        let mut metadata_charge = reserve_memory(
            &self.budget,
            ResourceClass::Control,
            (self.limits.max_manifest_bytes.get() as u64)
                .checked_mul(64)
                .ok_or(StreamedResultPackageError::CounterOverflow)?,
        )?;
        let _page_charge = reserve_memory(
            &self.budget,
            ResourceClass::Control,
            (self.limits.max_page_bytes.get() as u64)
                .checked_mul(16)
                .and_then(|n| n.checked_add(65_536))
                .ok_or(StreamedResultPackageError::CounterOverflow)?,
        )?;
        let manifest_length = self.sink.size(&manifest_path).await?;
        if manifest_length > self.limits.max_manifest_bytes.get() as u64 {
            return Err(StreamedResultPackageError::ManifestLimit {
                observed: usize::try_from(manifest_length).unwrap_or(usize::MAX),
                limit: self.limits.max_manifest_bytes.get(),
            });
        }
        self.storage.adopt(&manifest_path, manifest_length).await?;
        let bytes = read_exact(self.sink.as_ref(), &manifest_path, manifest_length).await?;
        if bytes.len() > self.limits.max_manifest_bytes.get() {
            return Err(StreamedResultPackageError::ManifestLimit {
                observed: bytes.len(),
                limit: self.limits.max_manifest_bytes.get(),
            });
        }
        let manifest: StreamedResultPackageManifest = serde_json::from_slice(&bytes)
            .map_err(StreamedResultPackageError::CanonicalManifest)?;
        if serde_json_canonicalizer::to_vec(&manifest)
            .map_err(StreamedResultPackageError::CanonicalManifest)?
            != bytes
        {
            return Err(StreamedResultPackageError::NonCanonicalManifest);
        }
        if manifest.epoch_id != hex(expected_epoch.as_bytes())
            || manifest.query_execution != hex(expected_query.as_bytes())
        {
            return Err(StreamedResultPackageError::PinMismatch);
        }
        validate_manifest(&manifest, self.limits)?;
        for page in &manifest.pages {
            self.storage
                .adopt(
                    &ObjectPath::from(page.object_path.clone()),
                    page.byte_length,
                )
                .await?;
            let page_bytes = read_exact(
                self.sink.as_ref(),
                &ObjectPath::from(page.object_path.clone()),
                page.byte_length,
            )
            .await?;
            validate_page(page, &page_bytes, self.limits.max_page_bytes.get())?;
        }
        metadata_charge.shrink(ResourceAmounts {
            memory_bytes: metadata_charge
                .amounts()
                .memory_bytes
                .saturating_sub(manifest_length.saturating_mul(64)),
            ..ResourceAmounts::default()
        })?;
        Ok(SealedStreamedResultPackage {
            epoch_id: expected_epoch,
            query_execution: expected_query,
            manifest_path,
            manifest_checksum: digest(&bytes),
            manifest_byte_length: u64::try_from(bytes.len())
                .map_err(|_| StreamedResultPackageError::CounterOverflow)?,
            manifest: metadata_charge.into_charged_value(manifest),
            lease,
            sink: Arc::clone(&self.sink),
            budget: self.budget.clone(),
            storage: Arc::clone(&self.storage),
        })
    }
}

fn bounded_batch_slices<'a>(
    schema: &'a SchemaRef,
    batch: &'a RecordBatch,
    max_rows: usize,
    max_bytes: usize,
) -> Result<
    impl Iterator<Item = Result<(RecordBatch, Vec<u8>), StreamedResultPackageError>> + 'a,
    StreamedResultPackageError,
> {
    preflight_schema(schema, max_bytes)?;
    let mut offset = 0_usize;
    let mut finished = false;
    Ok(std::iter::from_fn(move || {
        if finished {
            return None;
        }
        if batch.num_rows() == 0 {
            finished = true;
            return Some(
                encode_page(schema, std::slice::from_ref(batch), max_bytes)
                    .map(|bytes| (batch.clone(), bytes))
                    .map_err(Into::into),
            );
        }
        let candidate_rows = max_rows.min(batch.num_rows() - offset);
        let result = largest_fitting_slice(schema, batch, offset, candidate_rows, max_bytes);
        match &result {
            Ok((slice, _)) => {
                offset += slice.num_rows();
                finished = offset == batch.num_rows();
            }
            Err(_) => finished = true,
        }
        Some(result)
    }))
}

fn largest_fitting_slice(
    schema: &SchemaRef,
    batch: &RecordBatch,
    offset: usize,
    candidate_rows: usize,
    max_bytes: usize,
) -> Result<(RecordBatch, Vec<u8>), StreamedResultPackageError> {
    let mut low = 1_usize;
    let mut high = candidate_rows;
    let mut best = None;
    while low <= high {
        let rows = low + (high - low) / 2;
        let slice = batch.slice(offset, rows);
        match page_size(schema, std::slice::from_ref(&slice), max_bytes) {
            Ok(_) => {
                best = Some(slice);
                low = rows.saturating_add(1);
            }
            Err(BoundedEncodingError::PageByteLimit { .. }) => {
                if rows == 1 {
                    break;
                }
                high = rows - 1;
            }
            Err(error) => return Err(error.into()),
        }
    }
    let Some(best) = best else {
        return Err(StreamedResultPackageError::PageByteLimit {
            observed: max_bytes.saturating_add(1),
            limit: max_bytes,
        });
    };
    let bytes = encode_page(schema, std::slice::from_ref(&best), max_bytes)?;
    Ok((best, bytes))
}

fn validate_page(
    page: &ResultPageManifestEntry,
    bytes: &[u8],
    allocation_limit: usize,
) -> Result<(), StreamedResultPackageError> {
    if u64::try_from(bytes.len()).map_err(|_| StreamedResultPackageError::CounterOverflow)?
        != page.byte_length
        || hex(&digest(bytes)) != page.content_checksum
    {
        return Err(StreamedResultPackageError::PageIntegrity(page.page_ordinal));
    }
    validate_result_ipc_allocation_profile(bytes, allocation_limit)?;
    let mut reader = StreamReader::try_new(Cursor::new(bytes), None)
        .map_err(StreamedResultPackageError::Arrow)?;
    // Encoding preflight includes in-memory schema metadata, which can exceed the compact
    // bytes of a valid empty IPC page. Use the admitted allocation bound, not wire length.
    let schema_bytes = encode_page(&reader.schema(), &[], allocation_limit)?;
    if hex(&digest(&schema_bytes)) != page.schema_checksum {
        return Err(StreamedResultPackageError::PageSchemaIntegrity(
            page.page_ordinal,
        ));
    }
    let mut rows = 0_u64;
    let mut batches = 0_u64;
    for batch in &mut reader {
        let batch = batch.map_err(StreamedResultPackageError::Arrow)?;
        rows = rows
            .checked_add(
                u64::try_from(batch.num_rows())
                    .map_err(|_| StreamedResultPackageError::CounterOverflow)?,
            )
            .ok_or(StreamedResultPackageError::CounterOverflow)?;
        batches = batches
            .checked_add(1)
            .ok_or(StreamedResultPackageError::CounterOverflow)?;
    }
    if rows != page.row_count || batches != page.batch_count {
        return Err(StreamedResultPackageError::PageDecodeCount(
            page.page_ordinal,
        ));
    }
    Ok(())
}

fn validate_result_ipc_allocation_profile(
    bytes: &[u8],
    allocation_limit: usize,
) -> Result<(), StreamedResultPackageError> {
    crate::relation_ipc::validate_arrow_ipc_profile(bytes)
        .map_err(|_| StreamedResultPackageError::IpcAllocationProfile)?;
    let mut offset = 0;
    let mut nested_values = 0_u64;
    loop {
        let length = u32::from_le_bytes(
            bytes[offset + 4..offset + 8]
                .try_into()
                .expect("profile checked frame"),
        ) as usize;
        offset += 8;
        if length == 0 {
            break;
        }
        let message = arrow_ipc::root_as_message(&bytes[offset..offset + length])
            .map_err(|_| StreamedResultPackageError::IpcAllocationProfile)?;
        let batch = message.header_as_record_batch().or_else(|| {
            message
                .header_as_dictionary_batch()
                .and_then(|dictionary| dictionary.data())
        });
        if let Some(batch) = batch {
            if batch.compression().is_some()
                || usize::try_from(batch.length()).map_or(true, |rows| rows > allocation_limit)
            {
                return Err(StreamedResultPackageError::IpcAllocationProfile);
            }
            if let Some(nodes) = batch.nodes() {
                for node in nodes {
                    let length = u64::try_from(node.length())
                        .map_err(|_| StreamedResultPackageError::IpcAllocationProfile)?;
                    if length > allocation_limit as u64 {
                        return Err(StreamedResultPackageError::IpcAllocationProfile);
                    }
                    nested_values = nested_values
                        .checked_add(length)
                        .ok_or(StreamedResultPackageError::CounterOverflow)?;
                    if nested_values > (allocation_limit as u64).saturating_mul(4) {
                        return Err(StreamedResultPackageError::IpcAllocationProfile);
                    }
                }
            }
        }
        offset += length
            + usize::try_from(message.bodyLength())
                .map_err(|_| StreamedResultPackageError::IpcAllocationProfile)?;
    }
    Ok(())
}

fn validate_manifest(
    manifest: &StreamedResultPackageManifest,
    limits: StreamedResultPackageLimits,
) -> Result<(), StreamedResultPackageError> {
    if manifest.format != STREAMED_RESULT_PACKAGE_FORMAT
        || manifest.relations.is_empty()
        || manifest.relations.len() > limits.max_relations.get()
        || manifest.pages.is_empty()
        || manifest.pages.len() > limits.max_pages.get()
        || manifest.processing.len() > manifest.relations.len()
        || manifest.processing.iter().any(|value| !value.validate())
    {
        return Err(StreamedResultPackageError::ManifestShape);
    }
    let processing_ids = manifest
        .processing
        .iter()
        .map(|value| &value.query_id)
        .collect::<BTreeSet<_>>();
    if processing_ids.len() != manifest.processing.len() {
        return Err(StreamedResultPackageError::ManifestShape);
    }
    let total_rows = manifest.pages.iter().try_fold(0_u64, |total, page| {
        total
            .checked_add(page.row_count)
            .ok_or(StreamedResultPackageError::CounterOverflow)
    })?;
    let total_bytes = manifest.pages.iter().try_fold(0_u64, |total, page| {
        total
            .checked_add(page.byte_length)
            .ok_or(StreamedResultPackageError::CounterOverflow)
    })?;
    if total_rows != manifest.total_rows
        || total_bytes != manifest.total_bytes
        || usize::try_from(manifest.total_pages).ok() != Some(manifest.pages.len())
    {
        return Err(StreamedResultPackageError::ManifestTotals);
    }
    enforce_package_totals(manifest.pages.len(), total_rows, total_bytes, limits)?;
    let mut relation_names = BTreeSet::new();
    let mut next_page = 0;
    for relation in &manifest.relations {
        if !relation_names.insert(relation.relation_id.as_str()) {
            return Err(StreamedResultPackageError::DuplicateRelation(
                relation.relation_id.clone(),
            ));
        }
        let start = usize::try_from(relation.page_start)
            .map_err(|_| StreamedResultPackageError::ManifestShape)?;
        let count = usize::try_from(relation.page_count)
            .map_err(|_| StreamedResultPackageError::ManifestShape)?;
        let end = start
            .checked_add(count)
            .ok_or(StreamedResultPackageError::CounterOverflow)?;
        if start != next_page {
            return Err(StreamedResultPackageError::ManifestShape);
        }
        next_page = end;
        let selected = manifest
            .pages
            .get(start..end)
            .ok_or(StreamedResultPackageError::ManifestShape)?;
        if selected.is_empty()
            || selected
                .iter()
                .any(|page| page.relation_id != relation.relation_id)
            || selected.iter().map(|page| page.row_count).sum::<u64>() != relation.row_count
        {
            return Err(StreamedResultPackageError::ManifestShape);
        }
        validate_provenance(&relation.provenance, limits)?;
    }
    if next_page != manifest.pages.len() {
        return Err(StreamedResultPackageError::ManifestShape);
    }
    for (ordinal, page) in manifest.pages.iter().enumerate() {
        if usize::try_from(page.page_ordinal).ok() != Some(ordinal)
            || usize::try_from(page.byte_length)
                .map_or(true, |length| length > limits.max_page_bytes.get())
            || page.row_count > limits.max_page_rows.get() as u64
            || page.batch_count != 1
            || page.object_path
                != format!(
                    "packages/{}/{}/pages/{}/{:020}.arrow",
                    manifest.epoch_id,
                    manifest.query_execution,
                    hex(page.relation_id.as_bytes()),
                    page.page_ordinal
                )
            || [&page.schema_checksum, &page.content_checksum]
                .iter()
                .any(|checksum| {
                    checksum.len() != 64
                        || !checksum
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                })
        {
            return Err(StreamedResultPackageError::ManifestShape);
        }
    }
    Ok(())
}

fn validate_provenance(
    provenance: &[ResultProvenance],
    limits: StreamedResultPackageLimits,
) -> Result<(), StreamedResultPackageError> {
    if provenance.len() > limits.max_provenance_entries.get() {
        return Err(StreamedResultPackageError::ProvenanceLimit);
    }
    let mut seen = BTreeSet::new();
    for entry in provenance {
        if entry.kind.is_empty()
            || entry.identity.is_empty()
            || entry.kind.len().saturating_add(entry.identity.len())
                > limits.max_provenance_bytes.get()
            || !seen.insert((entry.kind.as_str(), entry.identity.as_str()))
        {
            return Err(StreamedResultPackageError::InvalidProvenance);
        }
    }
    Ok(())
}

fn enforce_package_totals(
    pages: usize,
    rows: u64,
    bytes: u64,
    limits: StreamedResultPackageLimits,
) -> Result<(), StreamedResultPackageError> {
    if pages > limits.max_pages.get() {
        return Err(StreamedResultPackageError::PageLimit {
            observed: pages,
            limit: limits.max_pages.get(),
        });
    }
    if rows > limits.max_total_rows.get() {
        return Err(StreamedResultPackageError::TotalRowLimit {
            observed: rows,
            limit: limits.max_total_rows.get(),
        });
    }
    if bytes > limits.max_total_bytes.get() {
        return Err(StreamedResultPackageError::TotalByteLimit {
            observed: bytes,
            limit: limits.max_total_bytes.get(),
        });
    }
    Ok(())
}

fn check_cancel_deadline(
    cancellation: &Cancellation,
    deadline: Instant,
) -> Result<(), StreamedResultPackageError> {
    if cancellation.is_cancelled() {
        return Err(StreamedResultPackageError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(StreamedResultPackageError::DeadlineExceeded);
    }
    Ok(())
}

async fn next_batch(
    stream: &mut SendableRecordBatchStream,
    cancellation: &Cancellation,
    deadline: Instant,
) -> Result<
    Option<Result<RecordBatch, datafusion::error::DataFusionError>>,
    StreamedResultPackageError,
> {
    let next = stream.next();
    tokio::pin!(next);
    loop {
        check_cancel_deadline(cancellation, deadline)?;
        tokio::select! {
            batch = &mut next => return Ok(batch),
            () = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => return Err(StreamedResultPackageError::DeadlineExceeded),
            () = tokio::time::sleep(std::time::Duration::from_millis(10)) => {},
        }
    }
}

fn validate_pins(
    epoch: EpochId,
    query: QueryExecutionPin,
) -> Result<(), StreamedResultPackageError> {
    if epoch.as_bytes().iter().all(|byte| *byte == 0)
        || query.as_bytes().iter().all(|byte| *byte == 0)
    {
        return Err(StreamedResultPackageError::InvalidPins);
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

const fn completeness_name(value: ResultCompleteness) -> &'static str {
    match value {
        ResultCompleteness::Complete => "complete",
        ResultCompleteness::Partial => "partial",
        ResultCompleteness::Unknown => "unknown",
    }
}

/// Stable failures from streaming, page publication, exact reopen, and retention.
impl From<BoundedEncodingError> for StreamedResultPackageError {
    fn from(error: BoundedEncodingError) -> Self {
        match error {
            BoundedEncodingError::CounterOverflow => Self::CounterOverflow,
            BoundedEncodingError::EncodingNestingLimit => Self::EncodingNestingLimit,
            BoundedEncodingError::PageByteLimit { observed, limit } => {
                Self::PageByteLimit { observed, limit }
            }
            BoundedEncodingError::Arrow(source) => Self::Arrow(source),
        }
    }
}

/// Stable failures from streaming, page publication, exact reopen, and retention.
#[derive(Debug, Error)]
pub enum StreamedResultPackageError {
    #[error("result Arrow nesting exceeds its bounded encoding profile")]
    EncodingNestingLimit,
    #[error("result IPC framing, nested counts, or compression violates its allocation profile")]
    IpcAllocationProfile,
    #[error("result object length differs from its proved bound")]
    ObjectLength,
    #[error("result range is outside its proved page")]
    InvalidReadRange,
    #[error(transparent)]
    Resource(#[from] crate::resource_budget::ResourceBudgetError),
    #[error("invalid streamed result limit {0}")]
    InvalidLimit(&'static str),
    #[error("streamed result pins are sentinel values")]
    InvalidPins,
    #[error("streamed result relation count {observed} exceeds {limit}")]
    RelationLimit { observed: usize, limit: usize },
    #[error("duplicate streamed result relation {0}")]
    DuplicateRelation(String),
    #[error("stream schema differs for relation {0}")]
    StreamSchemaDrift(String),
    #[error("streamed result relation {0} has a zero row limit")]
    InvalidRelationRowLimit(String),
    #[error("streamed result relation {relation} rows {observed} exceeds {limit}")]
    RelationRowLimit {
        relation: String,
        observed: u64,
        limit: u64,
    },
    #[error("streamed result page count {observed} exceeds {limit}")]
    PageLimit { observed: usize, limit: usize },
    #[error("streamed result page bytes {observed} exceeds {limit}")]
    PageByteLimit { observed: usize, limit: usize },
    #[error("streamed result rows {observed} exceeds {limit}")]
    TotalRowLimit { observed: u64, limit: u64 },
    #[error("streamed result bytes {observed} exceeds {limit}")]
    TotalByteLimit { observed: u64, limit: u64 },
    #[error("streamed result manifest bytes {observed} exceeds {limit}")]
    ManifestLimit { observed: usize, limit: usize },
    #[error("streamed result provenance count exceeds its bound")]
    ProvenanceLimit,
    #[error("streamed result provenance is empty, duplicated, or oversized")]
    InvalidProvenance,
    #[error("canonical semantic response is not canonical JSON")]
    NonCanonicalResponse,
    #[error("streamed result manifest is not canonical JSON")]
    NonCanonicalManifest,
    #[error("streamed result manifest shape is invalid")]
    ManifestShape,
    #[error("streamed result manifest totals differ")]
    ManifestTotals,
    #[error("streamed result manifest pins differ")]
    PinMismatch,
    #[error("streamed result page {0} is unavailable")]
    UnknownPage(u64),
    #[error("streamed result page {0} failed content integrity")]
    PageIntegrity(u64),
    #[error("streamed result page {0} failed schema integrity")]
    PageSchemaIntegrity(u64),
    #[error("streamed result page {0} decoded counts differ")]
    PageDecodeCount(u64),
    #[error("streamed result lease expired")]
    Expired,
    #[error("streamed result work was cancelled")]
    Cancelled,
    #[error("streamed result deadline elapsed")]
    DeadlineExceeded,
    #[error(transparent)]
    PublicationIntent(#[from] ResultPublicationIntentError),
    #[error("streamed result counter overflow")]
    CounterOverflow,
    #[error("streamed result Arrow failure: {0}")]
    Arrow(#[source] ArrowError),
    #[error("streamed result DataFusion failure: {0}")]
    DataFusion(#[source] datafusion::error::DataFusionError),
    #[error("streamed result object-store failure: {0}")]
    ObjectStore(#[from] object_store::Error),
    #[error("streamed result canonical response failure: {0}")]
    CanonicalResponse(#[source] serde_json::Error),
    #[error("streamed result canonical manifest failure: {0}")]
    CanonicalManifest(#[source] serde_json::Error),
}

#[cfg(test)]
pub(super) use crate::resource_budget::test_resource_budget;

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::time::Duration;

    use arrow_array::{ArrayRef, Int64Array};
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::physical_plan::stream::RecordBatchStreamAdapter;
    use futures::{TryStreamExt as _, stream};
    use object_store::memory::InMemory;

    use super::*;
    use crate::fabric::command::LeaseId;

    #[derive(Debug)]
    struct AcceptPublicationIntent;

    #[async_trait]
    impl ResultPublicationIntentRecorder for AcceptPublicationIntent {
        async fn record_publication_intent(
            &self,
            _intent: PendingResultObjectSet,
        ) -> Result<(), ResultPublicationIntentError> {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct RecordingSink {
        store: Arc<InMemory>,
        created: Mutex<Vec<String>>,
        cancel_after_first_create: Option<Cancellation>,
        largest_read: AtomicU64,
        truncate_range: AtomicBool,
        fail_after_create: AtomicBool,
        fail_delete: AtomicBool,
        pause_after_create: AtomicBool,
        creation_signal: tokio::sync::Notify,
    }

    impl RecordingSink {
        fn new(cancel_after_first_create: Option<Cancellation>) -> Self {
            Self {
                store: Arc::new(InMemory::new()),
                created: Mutex::new(Vec::new()),
                cancel_after_first_create,
                largest_read: AtomicU64::new(0),
                truncate_range: AtomicBool::new(false),
                fail_after_create: AtomicBool::new(false),
                fail_delete: AtomicBool::new(false),
                pause_after_create: AtomicBool::new(false),
                creation_signal: tokio::sync::Notify::new(),
            }
        }

        fn created_paths(&self) -> Vec<String> {
            self.created.lock().expect("recording lock").clone()
        }

        async fn object_count(&self) -> usize {
            self.store
                .list(None)
                .try_collect::<Vec<_>>()
                .await
                .expect("in-memory list")
                .len()
        }
    }

    #[derive(Debug)]
    struct AssertIntentBeforeWrite {
        sink: Arc<RecordingSink>,
        intents: Mutex<Vec<PendingResultObjectSet>>,
    }

    #[async_trait]
    impl ResultPublicationIntentRecorder for AssertIntentBeforeWrite {
        async fn record_publication_intent(
            &self,
            intent: PendingResultObjectSet,
        ) -> Result<(), ResultPublicationIntentError> {
            let created = self.sink.created_paths();
            assert_eq!(intent.page_object_paths.len(), created.len() + 1);
            assert_eq!(&intent.page_object_paths[..created.len()], created);
            self.intents.lock().expect("intent lock").push(intent);
            Ok(())
        }
    }

    #[async_trait]
    impl ResultObjectSink for RecordingSink {
        async fn create(
            &self,
            path: &ObjectPath,
            bytes: Vec<u8>,
        ) -> Result<(), object_store::Error> {
            self.store
                .put_opts(
                    path,
                    bytes.into(),
                    PutOptions {
                        mode: PutMode::Create,
                        ..PutOptions::default()
                    },
                )
                .await?;
            {
                let mut created = self.created.lock().expect("recording lock");
                created.push(path.to_string());
                if created.len() == 1
                    && let Some(cancellation) = &self.cancel_after_first_create
                {
                    cancellation.cancel();
                }
            }
            if self.pause_after_create.load(Ordering::Relaxed) {
                self.creation_signal.notify_one();
                std::future::pending::<()>().await;
            }
            if self.fail_after_create.load(Ordering::Relaxed) {
                return Err(range_error("uncertain put after persisted bytes"));
            }
            Ok(())
        }

        async fn size(&self, path: &ObjectPath) -> Result<u64, object_store::Error> {
            Ok(self.store.head(path).await?.size)
        }

        async fn read_range(
            &self,
            path: &ObjectPath,
            range: Range<u64>,
        ) -> Result<Vec<u8>, object_store::Error> {
            self.largest_read
                .fetch_max(range.end - range.start, Ordering::Relaxed);
            let mut bytes =
                ObjectStoreResultSink::new(Arc::clone(&self.store) as Arc<dyn ObjectStore>)
                    .read_range(path, range)
                    .await?;
            if self.truncate_range.load(Ordering::Relaxed) {
                bytes.pop();
            }
            Ok(bytes)
        }

        async fn delete(&self, path: &ObjectPath) -> Result<(), object_store::Error> {
            if self.fail_delete.load(Ordering::Relaxed) {
                return Err(range_error("injected delete failure"));
            }
            self.store.delete(path).await
        }
    }

    fn limits(max_page_rows: usize) -> StreamedResultPackageLimits {
        StreamedResultPackageLimits::try_new(
            8,
            64,
            max_page_rows,
            64 * 1024,
            1_024,
            4 * 1024 * 1024,
            128 * 1024,
            64,
            1_024,
        )
        .expect("valid limits")
    }

    fn pins() -> (EpochId, QueryExecutionPin, ResultResourceLease) {
        (
            EpochId::from_bytes([0x36; 16]),
            QueryExecutionPin::from_bytes([0x46; 32]),
            ResultResourceLease::try_new(LeaseId::from_bytes([0x56; 16]), 10, 10_000)
                .expect("valid lease"),
        )
    }

    fn relation(values: &[i64], batch_rows: usize) -> StreamedRelationInput {
        let schema = Arc::new(Schema::new(vec![Field::new(
            "value",
            DataType::Int64,
            false,
        )]));
        let batches = values
            .chunks(batch_rows)
            .map(|chunk| {
                RecordBatch::try_new(
                    Arc::clone(&schema),
                    vec![Arc::new(Int64Array::from(chunk.to_vec())) as ArrayRef],
                )
                .expect("valid batch")
            })
            .map(Ok)
            .collect::<Vec<Result<RecordBatch, datafusion::error::DataFusionError>>>();
        let stream = Box::pin(RecordBatchStreamAdapter::new(
            Arc::clone(&schema),
            stream::iter(batches),
        ));
        StreamedRelationInput {
            relation_id: RelationId::new("query.result.v1").expect("relation id"),
            schema,
            stream,
            max_rows: 1_024,
            row_selection: None,
            coverage: ResultCoverage::complete(values.len() as u64),
            provenance: vec![ResultProvenance {
                kind: "transformation_release".to_owned(),
                identity: "release:test".to_owned(),
            }],
        }
    }

    async fn seal_fixture(
        sink: Arc<dyn ResultObjectSink>,
        max_page_rows: usize,
        values: &[i64],
        batch_rows: usize,
        cancellation: &Cancellation,
    ) -> Result<SealedStreamedResultPackage, StreamedResultPackageError> {
        seal_fixture_with_intent(
            sink,
            max_page_rows,
            values,
            batch_rows,
            cancellation,
            &AcceptPublicationIntent,
        )
        .await
    }

    async fn seal_fixture_with_intent(
        sink: Arc<dyn ResultObjectSink>,
        max_page_rows: usize,
        values: &[i64],
        batch_rows: usize,
        cancellation: &Cancellation,
        publication_intent: &dyn ResultPublicationIntentRecorder,
    ) -> Result<SealedStreamedResultPackage, StreamedResultPackageError> {
        let (epoch, query, lease) = pins();
        StreamedResultPackageBuilder::new(sink, limits(max_page_rows), test_resource_budget())
            .seal(
                epoch,
                query,
                br#"{"request":"bounded"}"#,
                vec![relation(values, batch_rows)],
                lease,
                cancellation,
                Instant::now() + Duration::from_secs(5),
                publication_intent,
            )
            .await
    }

    #[tokio::test]
    async fn wp45_int_exact_publication_intent_precedes_every_object_write() {
        let sink = Arc::new(RecordingSink::new(None));
        let recorder = AssertIntentBeforeWrite {
            sink: Arc::clone(&sink),
            intents: Mutex::new(Vec::new()),
        };
        let sealed = seal_fixture_with_intent(
            Arc::clone(&sink) as Arc<dyn ResultObjectSink>,
            2,
            &[1, 2, 3, 4, 5],
            3,
            &Cancellation::default(),
            &recorder,
        )
        .await
        .expect("seal after durable intent");
        let intents = recorder.intents.lock().expect("intent lock");
        assert_eq!(intents.len(), sealed.manifest().pages.len());
        let final_intent = intents.last().expect("final cumulative intent");
        assert_eq!(
            final_intent.manifest_object_path,
            sealed.manifest_path().to_string()
        );
        assert_eq!(
            final_intent.page_object_paths,
            sealed
                .manifest()
                .pages
                .iter()
                .map(|page| page.object_path.clone())
                .collect::<Vec<_>>()
        );
        let created = sink.created_paths();
        assert_eq!(created.last(), Some(&final_intent.manifest_object_path));
    }

    #[tokio::test]
    async fn wp79_result_range_reads_are_bounded_and_check_unselected_corruption() {
        let sink = Arc::new(RecordingSink::new(None));
        let sealed = seal_fixture(
            sink.clone(),
            8,
            &[1, 2, 3, 4, 5, 6, 7, 8],
            8,
            &Cancellation::default(),
        )
        .await
        .unwrap();
        let whole = sealed.read_page(0, 100).await.unwrap();
        sink.largest_read.store(0, Ordering::Relaxed);
        let chunk = sealed.read_page_range(0, 11, 17, 100).await.unwrap();
        assert_eq!(&*chunk, &whole[11..28]);
        assert!(sink.largest_read.load(Ordering::Relaxed) <= 17);
        sink.truncate_range.store(true, Ordering::Relaxed);
        assert!(matches!(
            sealed.read_page_range(0, 0, 17, 100).await,
            Err(StreamedResultPackageError::PageIntegrity(0))
        ));
        sink.truncate_range.store(false, Ordering::Relaxed);
        let path = ObjectPath::from(sealed.manifest().pages[0].object_path.clone());
        let mut corrupt = whole.to_vec();
        let last = corrupt.len() - 1;
        corrupt[last] ^= 1;
        sink.store.put(&path, corrupt.into()).await.unwrap();
        assert!(matches!(
            sealed.read_page_range(0, 0, 17, 100).await,
            Err(StreamedResultPackageError::PageIntegrity(0))
        ));
        sink.store.put(&path, vec![0_u8; 2].into()).await.unwrap();
        assert!(matches!(
            sealed.read_page_range(0, 0, 17, 100).await,
            Err(StreamedResultPackageError::PageIntegrity(0))
        ));
    }

    #[tokio::test]
    async fn wp79_result_pages_publish_before_polling_more_input_and_charges_follow_owners() {
        let sink = Arc::new(RecordingSink::new(None));
        let budget = test_resource_budget();
        let mut input = relation(&[1, 2], 1);
        let observed_sink = sink.clone();
        let mut count = 0;
        input.stream = Box::pin(RecordBatchStreamAdapter::new(
            input.schema.clone(),
            input.stream.inspect(move |_| {
                if count > 0 {
                    assert!(
                        !observed_sink.created_paths().is_empty(),
                        "previous page must not be retained in prepared_pages"
                    );
                }
                count += 1;
            }),
        ));
        let (epoch, query, lease) = pins();
        let builder = StreamedResultPackageBuilder::new(sink, limits(1), budget.clone());
        let sealed = builder
            .seal(
                epoch,
                query,
                b"{}",
                vec![input],
                lease,
                &Cancellation::default(),
                Instant::now() + Duration::from_secs(5),
                &AcceptPublicationIntent,
            )
            .await
            .unwrap();
        let retained = budget.observation().used.memory_bytes;
        assert!(retained > 0);
        assert!(retained < 64 * limits(1).max_manifest_bytes.get() as u128);
        let alias = sealed.clone();
        assert_eq!(budget.observation().used.memory_bytes, retained);
        let manifest_path = sealed.manifest_path().clone();
        let pages = sealed
            .manifest()
            .pages
            .iter()
            .map(|page| ObjectPath::from(page.object_path.clone()))
            .collect::<Vec<_>>();
        let retained_storage = u128::from(sealed.retained_object_bytes());
        assert_eq!(budget.observation().used.disk_bytes, retained_storage);
        assert_eq!(budget.observation().used.retained_bytes, retained_storage);
        assert_eq!(budget.observation().used.pages, pages.len() as u128 + 1);
        let chunk = sealed.read_page_range(0, 0, 17, 100).await.unwrap();
        let chunk_alias = chunk.clone();
        drop(sealed);
        drop(alias);
        let storage_metadata = budget.observation().used.memory_bytes - 17;
        assert!(storage_metadata > 0);
        assert_eq!(budget.observation().used.disk_bytes, retained_storage);
        drop(chunk);
        assert_eq!(
            budget.observation().used.memory_bytes,
            storage_metadata + 17
        );
        drop(chunk_alias);
        assert_eq!(budget.observation().used.memory_bytes, storage_metadata);
        builder
            .delete_retained_object_set(&pages, &manifest_path)
            .await
            .unwrap();
        assert_eq!(budget.observation().used.memory_bytes, 0);
        assert_eq!(budget.observation().used.disk_bytes, 0);
        assert_eq!(budget.observation().used.retained_bytes, 0);
        assert_eq!(budget.observation().used.pages, 0);
    }

    #[tokio::test]
    async fn wp79_result_storage_uncertain_put_and_failed_delete_keep_exact_ownership() {
        let sink = Arc::new(RecordingSink::new(None));
        sink.fail_after_create.store(true, Ordering::Relaxed);
        let budget = test_resource_budget();
        let builder = StreamedResultPackageBuilder::new(sink.clone(), limits(1), budget.clone());
        let (epoch, query, lease) = pins();
        let recorder = AssertIntentBeforeWrite {
            sink: sink.clone(),
            intents: Mutex::new(Vec::new()),
        };
        let result = builder
            .seal(
                epoch,
                query,
                b"{}",
                vec![relation(&[1], 1)],
                lease,
                &Cancellation::default(),
                Instant::now() + Duration::from_secs(5),
                &recorder,
            )
            .await;
        assert!(matches!(
            result,
            Err(StreamedResultPackageError::ObjectStore(_))
        ));
        assert_eq!(sink.object_count().await, 1);
        let intent = recorder.intents.lock().unwrap()[0].clone();
        let paths = intent
            .page_object_paths
            .iter()
            .map(|p| ObjectPath::from(p.clone()))
            .collect::<Vec<_>>();
        let manifest = ObjectPath::from(intent.manifest_object_path);
        let expected_bytes = u128::from(sink.size(&paths[0]).await.unwrap());
        assert_eq!(budget.observation().used.disk_bytes, expected_bytes);
        assert_eq!(budget.observation().used.retained_bytes, expected_bytes);
        assert_eq!(budget.observation().used.pages, 1);
        sink.fail_delete.store(true, Ordering::Relaxed);
        assert!(
            builder
                .delete_retained_object_set(&paths, &manifest)
                .await
                .is_err()
        );
        assert_eq!(budget.observation().used.disk_bytes, expected_bytes);
        sink.fail_delete.store(false, Ordering::Relaxed);
        // A confirmed NotFound releases the same charge too.
        sink.store.delete(&paths[0]).await.unwrap();
        builder
            .clone()
            .delete_retained_object_set(&paths, &manifest)
            .await
            .unwrap();
        assert_eq!(budget.observation().used.disk_bytes, 0);
        assert_eq!(budget.observation().used.memory_bytes, 0);
        assert_eq!(budget.observation().used.pages, 0);
    }

    #[tokio::test]
    async fn wp79_result_storage_cancelled_put_future_keeps_recoverable_charge() {
        let sink = Arc::new(RecordingSink::new(None));
        sink.pause_after_create.store(true, Ordering::Relaxed);
        let budget = test_resource_budget();
        let builder = StreamedResultPackageBuilder::new(sink.clone(), limits(1), budget.clone());
        let (epoch, query, lease) = pins();
        let cancellation = Cancellation::default();
        let recorder = AssertIntentBeforeWrite {
            sink: sink.clone(),
            intents: Mutex::new(Vec::new()),
        };
        let mut seal = Box::pin(builder.seal(
            epoch,
            query,
            b"{}",
            vec![relation(&[1], 1)],
            lease,
            &cancellation,
            Instant::now() + Duration::from_secs(5),
            &recorder,
        ));
        tokio::time::timeout(Duration::from_secs(1), async {
            tokio::select! {
                result = &mut seal => panic!("put should remain pending: {result:?}"),
                () = sink.creation_signal.notified() => {}
            }
        })
        .await
        .unwrap();
        drop(seal);
        assert_eq!(sink.object_count().await, 1);
        assert!(budget.observation().used.disk_bytes > 0);
        let intent = recorder.intents.lock().unwrap()[0].clone();
        let paths = intent
            .page_object_paths
            .into_iter()
            .map(ObjectPath::from)
            .collect::<Vec<_>>();
        let manifest = ObjectPath::from(intent.manifest_object_path);
        // Simulate process-owner retirement; only durable intent and exact stored bytes remain.
        drop(builder);
        assert_eq!(budget.observation().used.disk_bytes, 0);
        let recovered = StreamedResultPackageBuilder::new(sink.clone(), limits(1), budget.clone());
        sink.fail_delete.store(true, Ordering::Relaxed);
        assert!(
            recovered
                .delete_retained_object_set(&paths, &manifest)
                .await
                .is_err()
        );
        assert_eq!(
            budget.observation().used.disk_bytes,
            u128::from(sink.size(&paths[0]).await.unwrap())
        );
        assert_eq!(budget.observation().used.pages, 1);
        sink.fail_delete.store(false, Ordering::Relaxed);
        recovered
            .delete_retained_object_set(&paths, &manifest)
            .await
            .unwrap();
        assert_eq!(budget.observation().used.disk_bytes, 0);
        assert_eq!(budget.observation().used.memory_bytes, 0);
    }

    #[tokio::test]
    async fn wp79_result_storage_reopen_deduplicates_and_restart_reacquires_before_cleanup() {
        let sink = Arc::new(RecordingSink::new(None));
        let budget = test_resource_budget();
        let builder = StreamedResultPackageBuilder::new(sink.clone(), limits(1), budget.clone());
        let (epoch, query, lease) = pins();
        let sealed = builder
            .seal(
                epoch,
                query,
                b"{}",
                vec![relation(&[1, 2], 1)],
                lease,
                &Cancellation::default(),
                Instant::now() + Duration::from_secs(5),
                &AcceptPublicationIntent,
            )
            .await
            .unwrap();
        let path = sealed.manifest_path().clone();
        let pages = sealed
            .manifest()
            .pages
            .iter()
            .map(|p| ObjectPath::from(p.object_path.clone()))
            .collect::<Vec<_>>();
        let expected = budget.observation().used.disk_bytes;
        let reopened = builder
            .clone()
            .reopen(path.clone(), epoch, query, lease)
            .await
            .unwrap();
        assert_eq!(budget.observation().used.disk_bytes, expected);
        assert_eq!(budget.observation().used.pages, 3);
        drop(reopened);
        drop(sealed);
        drop(builder);
        assert_eq!(
            budget.observation().used.disk_bytes,
            0,
            "process owner retired"
        );
        let recovered = StreamedResultPackageBuilder::new(sink.clone(), limits(1), budget.clone());
        let reopened = recovered
            .reopen(path.clone(), epoch, query, lease)
            .await
            .unwrap();
        assert_eq!(budget.observation().used.disk_bytes, expected);
        drop(reopened);
        sink.fail_delete.store(true, Ordering::Relaxed);
        assert!(
            recovered
                .delete_retained_object_set(&pages, &path)
                .await
                .is_err()
        );
        assert_eq!(budget.observation().used.disk_bytes, expected);
        sink.fail_delete.store(false, Ordering::Relaxed);
        recovered
            .delete_retained_object_set(&pages, &path)
            .await
            .unwrap();
        assert_eq!(budget.observation().used.memory_bytes, 0);
        assert_eq!(budget.observation().used.disk_bytes, 0);
        assert_eq!(sink.object_count().await, 0);
    }

    #[tokio::test]
    async fn wp79_result_storage_admits_before_write_and_bounds_recovery_object_size() {
        let sink = Arc::new(RecordingSink::new(None));
        let parent = test_resource_budget();
        let mut policy = parent.policy();
        policy.limits.disk_bytes = 1;
        let budget = parent.workspace([79; 16], policy).unwrap();
        let builder = StreamedResultPackageBuilder::new(sink.clone(), limits(1), budget.clone());
        let (epoch, query, lease) = pins();
        assert!(matches!(
            builder
                .seal(
                    epoch,
                    query,
                    b"{}",
                    vec![relation(&[1], 1)],
                    lease,
                    &Cancellation::default(),
                    Instant::now() + Duration::from_secs(5),
                    &AcceptPublicationIntent
                )
                .await,
            Err(StreamedResultPackageError::Resource(_))
        ));
        assert_eq!(sink.object_count().await, 0);
        assert!(sink.created_paths().is_empty());
        assert_eq!(budget.observation().used.memory_bytes, 0);
        let page = ObjectPath::from("bounded-recovery-page");
        sink.store
            .put(&page, vec![0; limits(1).max_page_bytes.get() + 1].into())
            .await
            .unwrap();
        assert!(matches!(
            builder
                .delete_retained_object_set(&[page], &ObjectPath::from("manifest"))
                .await,
            Err(StreamedResultPackageError::ObjectLength)
        ));
        assert_eq!(sink.object_count().await, 1);
        assert_eq!(budget.observation().used.disk_bytes, 0);
    }

    #[tokio::test]
    async fn empty_metadata_rich_results_use_the_admitted_schema_allocation_bound() {
        let schema = Arc::new(Schema::new(
            (0..20)
                .map(|column| {
                    Field::new(format!("column_{column}"), DataType::Int64, false).with_metadata(
                        (0..16)
                            .map(|key| (format!("key-{key}"), format!("value-{key}")))
                            .collect(),
                    )
                })
                .collect::<Vec<_>>(),
        ));
        let batch = RecordBatch::new_empty(schema.clone());
        let mut envelope = limits(1);
        envelope.max_page_bytes = NonZeroUsize::new(65_536).unwrap();
        let encoded = encode_page(
            &schema,
            std::slice::from_ref(&batch),
            envelope.max_page_bytes.get(),
        )
        .unwrap();
        assert!(
            preflight_schema(&schema, encoded.len()).is_err(),
            "fixture must distinguish wire bytes from schema allocation estimate"
        );
        for emit_empty_batch in [false, true] {
            let sink = Arc::new(RecordingSink::new(None));
            let builder = StreamedResultPackageBuilder::new(sink, envelope, test_resource_budget());
            let mut input = relation(&[], 1);
            input.schema = schema.clone();
            input.stream = Box::pin(RecordBatchStreamAdapter::new(
                schema.clone(),
                stream::iter(emit_empty_batch.then(|| Ok(batch.clone())).into_iter()),
            ));
            let (epoch, query, lease) = pins();
            let sealed = builder
                .seal(
                    epoch,
                    query,
                    b"{}",
                    vec![input],
                    lease.clone(),
                    &Cancellation::default(),
                    Instant::now() + Duration::from_secs(5),
                    &AcceptPublicationIntent,
                )
                .await
                .unwrap();
            assert_eq!(sealed.manifest().total_rows, 0);
            assert_eq!(sealed.manifest().total_pages, 1);
            let reopened = builder
                .reopen(sealed.manifest_path().clone(), epoch, query, lease)
                .await
                .unwrap();
            assert_eq!(reopened.manifest().total_rows, 0);
            assert_eq!(
                reopened.manifest().pages[0].schema_checksum,
                sealed.manifest().pages[0].schema_checksum
            );
        }
    }

    #[tokio::test]
    async fn wp79_result_compact_null_rows_use_explicit_allocation_bound() {
        let sink = Arc::new(RecordingSink::new(None));
        let builder = StreamedResultPackageBuilder::new(sink, limits(1024), test_resource_budget());
        let schema = Arc::new(Schema::new(vec![Field::new("value", DataType::Null, true)]));
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(arrow_array::NullArray::new(1024))],
        )
        .unwrap();
        let mut input = relation(&[], 1);
        input.schema = schema.clone();
        input.stream = Box::pin(RecordBatchStreamAdapter::new(
            schema,
            stream::iter([Ok(batch)]),
        ));
        let (epoch, query, lease) = pins();
        let sealed = builder
            .seal(
                epoch,
                query,
                b"{}",
                vec![input],
                lease,
                &Cancellation::default(),
                Instant::now() + Duration::from_secs(5),
                &AcceptPublicationIntent,
            )
            .await
            .unwrap();
        assert!(sealed.manifest().pages[0].byte_length < 1024);
        assert_eq!(sealed.manifest().pages[0].row_count, 1024);
        sealed.read_page(0, 100).await.unwrap();
        builder
            .reopen(sealed.manifest_path().clone(), epoch, query, lease)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn wp79_result_pending_input_cancellation_cleans_written_pages_and_reservations() {
        let sink = Arc::new(RecordingSink::new(None));
        let budget = test_resource_budget();
        let mut input = relation(&[1], 1);
        input.stream = Box::pin(RecordBatchStreamAdapter::new(
            input.schema.clone(),
            input.stream.chain(stream::pending()),
        ));
        let (epoch, query, lease) = pins();
        let builder = StreamedResultPackageBuilder::new(sink.clone(), limits(1), budget.clone());
        let cancellation = Cancellation::default();
        let seal = builder.seal(
            epoch,
            query,
            b"{}",
            vec![input],
            lease,
            &cancellation,
            Instant::now() + Duration::from_secs(2),
            &AcceptPublicationIntent,
        );
        let cancel = async {
            tokio::time::sleep(Duration::from_millis(30)).await;
            cancellation.cancel();
        };
        let (result, ()) =
            tokio::time::timeout(Duration::from_secs(1), async { tokio::join!(seal, cancel) })
                .await
                .unwrap();
        assert!(matches!(result, Err(StreamedResultPackageError::Cancelled)));
        assert!(!sink.created_paths().is_empty());
        assert_eq!(sink.object_count().await, 0);
        assert_eq!(budget.observation().used.memory_bytes, 0);
    }

    #[test]
    fn wp79_result_ipc_declared_lengths_fail_before_decoder_allocation() {
        let mut declared_huge_metadata = vec![255, 255, 255, 255];
        declared_huge_metadata.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            validate_result_ipc_allocation_profile(&declared_huge_metadata, 65536),
            Err(StreamedResultPackageError::IpcAllocationProfile)
        ));
        let schema = Arc::new(Schema::new(vec![Field::new(
            "value",
            DataType::Int64,
            false,
        )]));
        let batch = RecordBatch::new_empty(schema.clone());
        let bytes = encode_page(&schema, &[batch], 65536).unwrap();
        for length in [0, 7, bytes.len() - 1] {
            assert!(matches!(
                validate_result_ipc_allocation_profile(&bytes[..length], 65536),
                Err(StreamedResultPackageError::IpcAllocationProfile)
            ));
        }
    }

    #[test]
    fn wp79_result_nested_slices_use_selected_child_bounds() {
        use arrow_array::Array;
        let array =
            arrow_array::ListArray::from_iter_primitive::<arrow_array::types::Int64Type, _, _>(
                (0..2048).map(|value| Some(vec![Some(value)])),
            );
        let schema = Arc::new(Schema::new(vec![Field::new(
            "value",
            array.data_type().clone(),
            true,
        )]));
        let batch = RecordBatch::try_new(schema.clone(), vec![Arc::new(array)]).unwrap();
        let mut rows = 0;
        for page in bounded_batch_slices(&schema, &batch, 8, 1024).unwrap() {
            let (slice, bytes) = page.unwrap();
            assert!(bytes.len() <= 1024);
            rows += slice.num_rows();
        }
        assert_eq!(rows, 2048);
    }

    #[tokio::test]
    async fn wp79_result_metadata_overflow_cleans_incremental_pages() {
        let sink = Arc::new(RecordingSink::new(None));
        let budget = test_resource_budget();
        let mut bounded = limits(1);
        bounded.max_manifest_bytes = NonZeroUsize::new(1600).unwrap();
        let builder = StreamedResultPackageBuilder::new(sink.clone(), bounded, budget.clone());
        let (epoch, query, lease) = pins();
        let result = builder
            .seal(
                epoch,
                query,
                b"{}",
                vec![relation(&[1, 2, 3, 4, 5, 6, 7, 8], 8)],
                lease,
                &Cancellation::default(),
                Instant::now() + Duration::from_secs(5),
                &AcceptPublicationIntent,
            )
            .await;
        assert!(matches!(
            result,
            Err(StreamedResultPackageError::ManifestLimit { .. })
        ));
        assert!(!sink.created_paths().is_empty());
        assert!(
            sink.created_paths()
                .iter()
                .all(|path| !path.ends_with("manifest.json"))
        );
        assert_eq!(sink.object_count().await, 0);
        assert_eq!(budget.observation().used.memory_bytes, 0);
    }

    #[tokio::test]
    async fn wp36_int_manifest_last_pages_reopen_with_exact_contract() {
        let sink = Arc::new(RecordingSink::new(None));
        let sealed = seal_fixture(
            Arc::clone(&sink) as Arc<dyn ResultObjectSink>,
            2,
            &[1, 2, 3, 4, 5],
            3,
            &Cancellation::default(),
        )
        .await
        .expect("seal package");

        let paths = sink.created_paths();
        assert_eq!(paths.last(), Some(&sealed.manifest_path().to_string()));
        assert_eq!(sealed.manifest().total_rows, 5);
        assert_eq!(sealed.manifest().total_pages, 3);
        assert!(sealed.retained_object_bytes() > sealed.manifest().total_bytes);

        let (epoch, query, lease) = pins();
        let reopened = StreamedResultPackageBuilder::new(
            Arc::clone(&sink) as Arc<dyn ResultObjectSink>,
            limits(2),
            test_resource_budget(),
        )
        .reopen(sealed.manifest_path().clone(), epoch, query, lease)
        .await
        .expect("exact reopen");
        assert_eq!(reopened.manifest(), sealed.manifest());

        let mut tampered = reopened.manifest_path().clone();
        tampered = ObjectPath::from(format!("{tampered}.tampered"));
        assert!(matches!(
            StreamedResultPackageBuilder::new(
                Arc::clone(&sink) as Arc<dyn ResultObjectSink>,
                limits(2),
                test_resource_budget(),
            )
            .reopen(tampered, epoch, query, lease)
            .await,
            Err(StreamedResultPackageError::ObjectStore(_))
        ));
    }

    #[tokio::test]
    async fn result_selection_observes_exhaustion_without_publishing_probe_rows() {
        use super::super::processing_status::{EntityProcessingSummary, QueryProcessing};
        for (values, batch_rows, probe, expected) in [
            (vec![], 1, true, Some(false)),
            (vec![1], 1, true, Some(false)),
            (vec![1, 2], 1, true, Some(false)),
            (vec![1, 2, 3], 1, true, Some(true)),
            (vec![1, 2, 3], 3, true, Some(true)),
            (vec![1, 2], 2, false, None),
            (vec![1], 1, false, Some(false)),
        ] {
            let sink = Arc::new(RecordingSink::new(None));
            let builder =
                StreamedResultPackageBuilder::new(sink.clone(), limits(1), test_resource_budget());
            let (epoch, query, lease) = pins();
            let mut input = relation(&values, batch_rows);
            input.row_selection = Some(StreamedRowSelection {
                query_id: "q1".to_owned(),
                maximum_rows: 2,
                exhaustion_probe: probe,
            });
            let processing = vec![QueryProcessing {
                selection: None,
                query_id: "q1".to_owned(),
                maximum_rows: Some(2),
                additional_rows: None,
                processing: EntityProcessingSummary {
                    source_generation: 7,
                    requested_partitions: 1,
                    completed_partitions: 1,
                    remaining_partitions: 0,
                    scope: "python_files".to_owned(),
                    family: "function-declarations".to_owned(),
                    languages: vec!["python".to_owned()],
                    next_offset: None,
                    remainder: Vec::new(),
                },
            }];
            let sealed = builder
                .seal_with_processing(
                    epoch,
                    query,
                    b"{}",
                    processing,
                    vec![input],
                    lease,
                    &Cancellation::default(),
                    Instant::now() + Duration::from_secs(5),
                    &AcceptPublicationIntent,
                )
                .await
                .unwrap();
            assert_eq!(sealed.manifest().total_rows, values.len().min(2) as u64);
            assert_eq!(sealed.manifest().processing[0].additional_rows, expected);
            assert_eq!(
                sealed.manifest().processing[0]
                    .processing
                    .remaining_partitions,
                0
            );
            let reopened = builder
                .reopen(sealed.manifest_path().clone(), epoch, query, lease)
                .await
                .unwrap();
            assert_eq!(reopened.manifest(), sealed.manifest());
            let mut actual = Vec::new();
            for page in &sealed.manifest().pages {
                let bytes = sealed
                    .read_page(page.page_ordinal, lease.issued_at_unix_ms())
                    .await
                    .unwrap();
                let reader = arrow_ipc::reader::StreamReader::try_new(
                    std::io::Cursor::new(bytes.as_ref()),
                    None,
                )
                .unwrap();
                for batch in reader {
                    let batch = batch.unwrap();
                    actual.extend(
                        batch
                            .column(0)
                            .as_any()
                            .downcast_ref::<Int64Array>()
                            .unwrap()
                            .values()
                            .iter()
                            .copied(),
                    );
                }
            }
            assert_eq!(actual, values.into_iter().take(2).collect::<Vec<_>>());
        }
    }

    #[tokio::test]
    async fn processing_summary_reopens_with_result_and_rejects_corrupt_counts() {
        use super::super::processing_status::{
            EntityProcessingSummary, ProcessingRemainder, QueryProcessing,
        };
        let sink = Arc::new(RecordingSink::new(None));
        let builder =
            StreamedResultPackageBuilder::new(sink.clone(), limits(2), test_resource_budget());
        let (epoch, query, lease) = pins();
        let processing = vec![QueryProcessing {
            selection: None,
            query_id: "q1".to_owned(),
            maximum_rows: Some(2),
            additional_rows: Some(false),
            processing: EntityProcessingSummary {
                source_generation: 7,
                requested_partitions: 2,
                completed_partitions: 1,
                remaining_partitions: 1,
                scope: "selected_cargo_targets".to_owned(),
                family: "function-declarations".to_owned(),
                languages: vec!["rust".to_owned()],
                next_offset: None,
                remainder: vec![ProcessingRemainder {
                    language: "rust".to_owned(),
                    scope_kind: "cargo_target".to_owned(),
                    path: None,
                    path_bytes: b"raw-\xff/Cargo.toml".to_vec(),
                    target: Some("broken".to_owned()),
                    target_kind: Some("binary".to_owned()),
                    analysis_context_id: None,
                    state: "unavailable".to_owned(),
                    reason: "compiler_target_unavailable".to_owned(),
                }],
            },
        }];
        let sealed = builder
            .seal_with_processing(
                epoch,
                query,
                b"{}",
                processing.clone(),
                vec![relation(&[1, 2], 2)],
                lease,
                &Cancellation::default(),
                Instant::now() + Duration::from_secs(5),
                &AcceptPublicationIntent,
            )
            .await
            .unwrap();
        let reopened = builder
            .reopen(sealed.manifest_path().clone(), epoch, query, lease)
            .await
            .unwrap();
        assert_eq!(reopened.manifest().processing, processing);
        let mut corrupt = reopened.manifest().clone();
        corrupt.processing[0].processing.remaining_partitions = 0;
        assert!(matches!(
            validate_manifest(&corrupt, limits(2)),
            Err(StreamedResultPackageError::ManifestShape)
        ));
        corrupt = reopened.manifest().clone();
        corrupt.processing[0].processing.remainder[0].path = Some("lossy replacement".to_owned());
        assert!(matches!(
            validate_manifest(&corrupt, limits(2)),
            Err(StreamedResultPackageError::ManifestShape)
        ));
    }

    #[tokio::test]
    async fn wp36_beh_streamed_pages_decode_independently_across_batch_and_page_sizes() {
        for (batch_rows, page_rows) in [(1, 1), (2, 3), (7, 2), (7, 7)] {
            let sink = Arc::new(RecordingSink::new(None));
            let sealed = seal_fixture(
                Arc::clone(&sink) as Arc<dyn ResultObjectSink>,
                page_rows,
                &[10, 20, 30, 40, 50, 60, 70],
                batch_rows,
                &Cancellation::default(),
            )
            .await
            .expect("seal package");
            let mut decoded = Vec::new();
            for page in 0..sealed.manifest().total_pages {
                let bytes = sealed.read_page(page, 100).await.expect("bounded read");
                let mut reader =
                    StreamReader::try_new(Cursor::new(bytes), None).expect("independent stream");
                for batch in &mut reader {
                    let batch = batch.expect("page batch");
                    let values = batch
                        .column(0)
                        .as_any()
                        .downcast_ref::<Int64Array>()
                        .expect("int64 column");
                    decoded.extend(values.values().iter().copied());
                }
            }
            assert_eq!(decoded, [10, 20, 30, 40, 50, 60, 70]);
            assert_eq!(sealed.manifest().relations[0].coverage_state, "complete");
            assert_eq!(sealed.manifest().relations[0].provenance.len(), 1);
        }
    }

    #[tokio::test]
    async fn wp36_neg_create_only_collision_and_noncanonical_response_fail_closed() {
        let sink = Arc::new(RecordingSink::new(None));
        let erased = Arc::clone(&sink) as Arc<dyn ResultObjectSink>;
        seal_fixture(Arc::clone(&erased), 2, &[1, 2], 2, &Cancellation::default())
            .await
            .expect("first create");
        assert!(matches!(
            seal_fixture(erased, 2, &[1, 2], 2, &Cancellation::default()).await,
            Err(StreamedResultPackageError::ObjectStore(_))
        ));

        let other = Arc::new(RecordingSink::new(None));
        let (epoch, query, lease) = pins();
        let result = StreamedResultPackageBuilder::new(
            other as Arc<dyn ResultObjectSink>,
            limits(2),
            test_resource_budget(),
        )
        .seal(
            epoch,
            query,
            br#"{ "request": "not canonical" }"#,
            vec![relation(&[1], 1)],
            lease,
            &Cancellation::default(),
            Instant::now() + Duration::from_secs(5),
            &AcceptPublicationIntent,
        )
        .await;
        assert!(matches!(
            result,
            Err(StreamedResultPackageError::NonCanonicalResponse)
        ));
    }

    #[tokio::test]
    async fn wp36_ops_cancellation_cleans_unsealed_objects_and_terminal_reopens() {
        let cancellation = Cancellation::with_check_interval(1);
        let sink = Arc::new(RecordingSink::new(Some(cancellation.clone())));
        let recorder = AssertIntentBeforeWrite {
            sink: Arc::clone(&sink),
            intents: Mutex::new(Vec::new()),
        };
        let result = seal_fixture_with_intent(
            Arc::clone(&sink) as Arc<dyn ResultObjectSink>,
            1,
            &[1, 2, 3],
            3,
            &cancellation,
            &recorder,
        )
        .await;
        assert!(matches!(result, Err(StreamedResultPackageError::Cancelled)));
        assert_eq!(recorder.intents.lock().expect("intent lock").len(), 1);
        assert_eq!(sink.object_count().await, 0);

        let stable_sink = Arc::new(RecordingSink::new(None));
        let sealed = seal_fixture(
            Arc::clone(&stable_sink) as Arc<dyn ResultObjectSink>,
            2,
            &[8, 9],
            2,
            &Cancellation::default(),
        )
        .await
        .expect("sealed before restart");
        let (epoch, query, lease) = pins();
        let reopened = StreamedResultPackageBuilder::new(
            stable_sink as Arc<dyn ResultObjectSink>,
            limits(2),
            test_resource_budget(),
        )
        .reopen(sealed.manifest_path().clone(), epoch, query, lease)
        .await
        .expect("sealed package survives process state");
        assert_eq!(
            reopened.read_page(0, 100).await.expect("page"),
            sealed.read_page(0, 100).await.expect("page")
        );
    }
}
