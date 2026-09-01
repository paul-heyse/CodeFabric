//! Manifest-last, object-store-backed Arrow result packages.
//!
//! DataFusion output is consumed as a stream. Each page is a fresh Arrow IPC stream that can be
//! decoded independently, is published with create-only object-store semantics, and is bounded
//! before the next page is accepted. The canonical manifest is the final object published; its
//! presence is therefore the only sealed-package signal. No complete semantic result is retained
//! as process-local bytes.

use std::collections::BTreeSet;
use std::fmt;
use std::io::Cursor;
use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::Arc;
use std::time::Instant;

use arrow_array::RecordBatch;
use arrow_ipc::reader::StreamReader;
use arrow_ipc::writer::StreamWriter;
use arrow_schema::{ArrowError, SchemaRef};
use async_trait::async_trait;
use datafusion::physical_plan::SendableRecordBatchStream;
use futures::StreamExt as _;
use object_store::path::Path as ObjectPath;
use object_store::{ObjectStore, ObjectStoreExt as _, PutMode, PutOptions};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::cancellation::Cancellation;
use crate::relational_program::RelationId;

use super::arrow_result_resource::{
    QueryExecutionPin, ResultCompleteness, ResultCoverage, ResultResourceLease,
};
use super::command::EpochId;

/// Current immutable package contract.
pub const STREAMED_RESULT_PACKAGE_FORMAT: &str = "codefabric.streamed-result-package.v1";

/// Bounds that cover every page-local and package-wide allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    async fn read(&self, path: &ObjectPath) -> Result<Vec<u8>, object_store::Error>;
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

    async fn read(&self, path: &ObjectPath) -> Result<Vec<u8>, object_store::Error> {
        Ok(self.store.get(path).await?.bytes().await?.to_vec())
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
    manifest: Arc<StreamedResultPackageManifest>,
    lease: ResultResourceLease,
    sink: Arc<dyn ResultObjectSink>,
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
    #[must_use]
    pub const fn epoch_id(&self) -> EpochId {
        self.epoch_id
    }

    #[must_use]
    pub const fn query_execution(&self) -> QueryExecutionPin {
        self.query_execution
    }

    #[must_use]
    pub const fn manifest(&self) -> &Arc<StreamedResultPackageManifest> {
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
    pub async fn read_page(
        &self,
        page_ordinal: u64,
        observed_at_unix_ms: i64,
    ) -> Result<Vec<u8>, StreamedResultPackageError> {
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
        let bytes = self
            .sink
            .read(&ObjectPath::from(page.object_path.clone()))
            .await?;
        validate_page(page, &bytes)?;
        Ok(bytes)
    }

    /// Delete a sealed package only after its owning retention policy has made it unreachable.
    pub async fn delete_objects(self) -> Result<(), StreamedResultPackageError> {
        for page in &self.manifest.pages {
            self.sink
                .delete(&ObjectPath::from(page.object_path.clone()))
                .await?;
        }
        self.sink.delete(&self.manifest_path).await?;
        Ok(())
    }
}

/// Stateless manifest-last page sealer.
#[derive(Clone, Debug)]
pub struct StreamedResultPackageBuilder {
    sink: Arc<dyn ResultObjectSink>,
    limits: StreamedResultPackageLimits,
}

impl StreamedResultPackageBuilder {
    #[must_use]
    pub fn new(sink: Arc<dyn ResultObjectSink>, limits: StreamedResultPackageLimits) -> Self {
        Self { sink, limits }
    }

    /// Consume DataFusion streams and publish their manifest last.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub async fn seal(
        &self,
        epoch_id: EpochId,
        query_execution: QueryExecutionPin,
        canonical_semantic_response: &[u8],
        mut relations: Vec<StreamedRelationInput>,
        lease: ResultResourceLease,
        cancellation: &Cancellation,
        deadline: Instant,
    ) -> Result<SealedStreamedResultPackage, StreamedResultPackageError> {
        validate_pins(epoch_id, query_execution)?;
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
                if relation.stream.schema().as_ref() != relation.schema.as_ref() {
                    return Err(StreamedResultPackageError::StreamSchemaDrift(
                        relation.relation_id.as_str().to_owned(),
                    ));
                }
                let page_start = u64::try_from(pages.len())
                    .map_err(|_| StreamedResultPackageError::CounterOverflow)?;
                let mut relation_rows = 0_u64;
                let mut relation_pages = 0_u64;
                let mut saw_batch = false;
                while let Some(batch) = relation.stream.next().await {
                    check_cancel_deadline(cancellation, deadline)?;
                    let batch = batch.map_err(StreamedResultPackageError::DataFusion)?;
                    saw_batch = true;
                    if batch.schema().as_ref() != relation.schema.as_ref() {
                        return Err(StreamedResultPackageError::StreamSchemaDrift(
                            relation.relation_id.as_str().to_owned(),
                        ));
                    }
                    let slices = bounded_batch_slices(
                        &relation.schema,
                        &batch,
                        self.limits.max_page_rows.get(),
                        self.limits.max_page_bytes.get(),
                    )?;
                    for (page_batch, encoded) in slices {
                        check_cancel_deadline(cancellation, deadline)?;
                        let page = self
                            .publish_page(
                                epoch_id,
                                query_execution,
                                &relation.relation_id,
                                &relation.schema,
                                u64::try_from(pages.len())
                                    .map_err(|_| StreamedResultPackageError::CounterOverflow)?,
                                &page_batch,
                                encoded,
                                &mut created,
                            )
                            .await?;
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
                        pages.push(page);
                    }
                }
                if !saw_batch {
                    let empty = RecordBatch::new_empty(Arc::clone(&relation.schema));
                    let encoded = encode_page(&relation.schema, std::slice::from_ref(&empty))?;
                    let page = self
                        .publish_page(
                            epoch_id,
                            query_execution,
                            &relation.relation_id,
                            &relation.schema,
                            u64::try_from(pages.len())
                                .map_err(|_| StreamedResultPackageError::CounterOverflow)?,
                            &empty,
                            encoded,
                            &mut created,
                        )
                        .await?;
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
                    pages.push(page);
                }
                relation_entries.push(ResultRelationManifestEntry {
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
                });
            }

            let manifest = StreamedResultPackageManifest {
                format: STREAMED_RESULT_PACKAGE_FORMAT.to_owned(),
                epoch_id: hex(epoch_id.as_bytes()),
                query_execution: query_hex,
                canonical_semantic_response: response,
                total_rows,
                total_pages: u64::try_from(pages.len())
                    .map_err(|_| StreamedResultPackageError::CounterOverflow)?,
                total_bytes,
                relations: relation_entries,
                pages,
            };
            validate_manifest(&manifest, self.limits)?;
            let manifest_bytes = serde_json_canonicalizer::to_vec(&manifest)
                .map_err(StreamedResultPackageError::CanonicalManifest)?;
            if manifest_bytes.len() > self.limits.max_manifest_bytes.get() {
                return Err(StreamedResultPackageError::ManifestLimit {
                    observed: manifest_bytes.len(),
                    limit: self.limits.max_manifest_bytes.get(),
                });
            }
            check_cancel_deadline(cancellation, deadline)?;
            self.sink
                .create(&manifest_path, manifest_bytes.clone())
                .await?;
            let manifest_checksum = digest(&manifest_bytes);
            let manifest_byte_length = u64::try_from(manifest_bytes.len())
                .map_err(|_| StreamedResultPackageError::CounterOverflow)?;
            Ok(SealedStreamedResultPackage {
                epoch_id,
                query_execution,
                manifest_path,
                manifest_checksum,
                manifest_byte_length,
                manifest: Arc::new(manifest),
                lease,
                sink: Arc::clone(&self.sink),
            })
        }
        .await;

        if sealed.is_err() {
            for path in created.into_iter().rev() {
                let _ = self.sink.delete(&path).await;
            }
        }
        sealed
    }

    #[allow(clippy::too_many_arguments)]
    async fn publish_page(
        &self,
        epoch_id: EpochId,
        query_execution: QueryExecutionPin,
        relation_id: &RelationId,
        schema: &SchemaRef,
        page_ordinal: u64,
        batch: &RecordBatch,
        encoded: Vec<u8>,
        created: &mut Vec<ObjectPath>,
    ) -> Result<ResultPageManifestEntry, StreamedResultPackageError> {
        if encoded.len() > self.limits.max_page_bytes.get() {
            return Err(StreamedResultPackageError::PageByteLimit {
                observed: encoded.len(),
                limit: self.limits.max_page_bytes.get(),
            });
        }
        let schema_bytes = encode_page(schema, &[])?;
        let schema_checksum = digest(&schema_bytes);
        let content_checksum = digest(&encoded);
        let relation_hex = hex(relation_id.as_str().as_bytes());
        let object_path = ObjectPath::from(format!(
            "packages/{}/{}/pages/{relation_hex}/{page_ordinal:020}.arrow",
            hex(epoch_id.as_bytes()),
            hex(query_execution.as_bytes())
        ));
        self.sink.create(&object_path, encoded.clone()).await?;
        created.push(object_path.clone());
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
        validate_page(&entry, &encoded)?;
        Ok(entry)
    }

    /// Reopen one exact manifest and prove every page before returning serving authority.
    pub async fn reopen(
        &self,
        manifest_path: ObjectPath,
        expected_epoch: EpochId,
        expected_query: QueryExecutionPin,
        lease: ResultResourceLease,
    ) -> Result<SealedStreamedResultPackage, StreamedResultPackageError> {
        validate_pins(expected_epoch, expected_query)?;
        let bytes = self.sink.read(&manifest_path).await?;
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
            let page_bytes = self
                .sink
                .read(&ObjectPath::from(page.object_path.clone()))
                .await?;
            validate_page(page, &page_bytes)?;
        }
        Ok(SealedStreamedResultPackage {
            epoch_id: expected_epoch,
            query_execution: expected_query,
            manifest_path,
            manifest_checksum: digest(&bytes),
            manifest_byte_length: u64::try_from(bytes.len())
                .map_err(|_| StreamedResultPackageError::CounterOverflow)?,
            manifest: Arc::new(manifest),
            lease,
            sink: Arc::clone(&self.sink),
        })
    }
}

fn bounded_batch_slices(
    schema: &SchemaRef,
    batch: &RecordBatch,
    max_rows: usize,
    max_bytes: usize,
) -> Result<Vec<(RecordBatch, Vec<u8>)>, StreamedResultPackageError> {
    if batch.num_rows() == 0 {
        let encoded = encode_page(schema, std::slice::from_ref(batch))?;
        if encoded.len() > max_bytes {
            return Err(StreamedResultPackageError::PageByteLimit {
                observed: encoded.len(),
                limit: max_bytes,
            });
        }
        return Ok(vec![(batch.clone(), encoded)]);
    }
    let mut slices = Vec::new();
    let mut offset = 0_usize;
    while offset < batch.num_rows() {
        let candidate_rows = max_rows.min(batch.num_rows() - offset);
        append_largest_fitting_slice(
            schema,
            batch,
            offset,
            candidate_rows,
            max_bytes,
            &mut slices,
        )?;
        let emitted = slices
            .last()
            .map(|(slice, _)| slice.num_rows())
            .ok_or(StreamedResultPackageError::CounterOverflow)?;
        offset = offset
            .checked_add(emitted)
            .ok_or(StreamedResultPackageError::CounterOverflow)?;
    }
    Ok(slices)
}

fn append_largest_fitting_slice(
    schema: &SchemaRef,
    batch: &RecordBatch,
    offset: usize,
    candidate_rows: usize,
    max_bytes: usize,
    output: &mut Vec<(RecordBatch, Vec<u8>)>,
) -> Result<(), StreamedResultPackageError> {
    let mut low = 1_usize;
    let mut high = candidate_rows;
    let mut best = None;
    while low <= high {
        let rows = low + (high - low) / 2;
        let slice = batch.slice(offset, rows);
        let encoded = encode_page(schema, std::slice::from_ref(&slice))?;
        if encoded.len() <= max_bytes {
            best = Some((slice, encoded));
            low = rows.saturating_add(1);
        } else {
            if rows == 1 {
                break;
            }
            high = rows - 1;
        }
    }
    let Some(best) = best else {
        let one = batch.slice(offset, 1);
        let observed = encode_page(schema, std::slice::from_ref(&one))?.len();
        return Err(StreamedResultPackageError::PageByteLimit {
            observed,
            limit: max_bytes,
        });
    };
    output.push(best);
    Ok(())
}

fn encode_page(
    schema: &SchemaRef,
    batches: &[RecordBatch],
) -> Result<Vec<u8>, StreamedResultPackageError> {
    let mut bytes = Vec::new();
    {
        let mut writer =
            StreamWriter::try_new(&mut bytes, schema).map_err(StreamedResultPackageError::Arrow)?;
        for batch in batches {
            writer
                .write(batch)
                .map_err(StreamedResultPackageError::Arrow)?;
        }
        writer.finish().map_err(StreamedResultPackageError::Arrow)?;
    }
    Ok(bytes)
}

fn validate_page(
    page: &ResultPageManifestEntry,
    bytes: &[u8],
) -> Result<(), StreamedResultPackageError> {
    if u64::try_from(bytes.len()).map_err(|_| StreamedResultPackageError::CounterOverflow)?
        != page.byte_length
        || hex(&digest(bytes)) != page.content_checksum
    {
        return Err(StreamedResultPackageError::PageIntegrity(page.page_ordinal));
    }
    let mut reader = StreamReader::try_new(Cursor::new(bytes), None)
        .map_err(StreamedResultPackageError::Arrow)?;
    let schema_bytes = encode_page(&reader.schema(), &[])?;
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

fn validate_manifest(
    manifest: &StreamedResultPackageManifest,
    limits: StreamedResultPackageLimits,
) -> Result<(), StreamedResultPackageError> {
    if manifest.format != STREAMED_RESULT_PACKAGE_FORMAT
        || manifest.relations.is_empty()
        || manifest.relations.len() > limits.max_relations.get()
        || manifest.pages.is_empty()
        || manifest.pages.len() > limits.max_pages.get()
    {
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
    for (ordinal, page) in manifest.pages.iter().enumerate() {
        if usize::try_from(page.page_ordinal).ok() != Some(ordinal)
            || usize::try_from(page.byte_length)
                .map_or(true, |length| length > limits.max_page_bytes.get())
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
#[derive(Debug, Error)]
pub enum StreamedResultPackageError {
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
mod tests {
    use std::sync::Mutex;
    use std::time::Duration;

    use arrow_array::{ArrayRef, Int64Array};
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::physical_plan::stream::RecordBatchStreamAdapter;
    use futures::{TryStreamExt as _, stream};
    use object_store::memory::InMemory;

    use super::*;
    use crate::fabric::command::LeaseId;

    #[derive(Debug)]
    struct RecordingSink {
        store: Arc<InMemory>,
        created: Mutex<Vec<String>>,
        cancel_after_first_create: Option<Cancellation>,
    }

    impl RecordingSink {
        fn new(cancel_after_first_create: Option<Cancellation>) -> Self {
            Self {
                store: Arc::new(InMemory::new()),
                created: Mutex::new(Vec::new()),
                cancel_after_first_create,
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
            let mut created = self.created.lock().expect("recording lock");
            created.push(path.to_string());
            if created.len() == 1 {
                if let Some(cancellation) = &self.cancel_after_first_create {
                    cancellation.cancel();
                }
            }
            Ok(())
        }

        async fn read(&self, path: &ObjectPath) -> Result<Vec<u8>, object_store::Error> {
            Ok(self.store.get(path).await?.bytes().await?.to_vec())
        }

        async fn delete(&self, path: &ObjectPath) -> Result<(), object_store::Error> {
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
        let (epoch, query, lease) = pins();
        StreamedResultPackageBuilder::new(sink, limits(max_page_rows))
            .seal(
                epoch,
                query,
                br#"{"request":"bounded"}"#,
                vec![relation(values, batch_rows)],
                lease,
                cancellation,
                Instant::now() + Duration::from_secs(5),
            )
            .await
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
            )
            .reopen(tampered, epoch, query, lease)
            .await,
            Err(StreamedResultPackageError::ObjectStore(_))
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
        let result =
            StreamedResultPackageBuilder::new(other as Arc<dyn ResultObjectSink>, limits(2))
                .seal(
                    epoch,
                    query,
                    br#"{ "request": "not canonical" }"#,
                    vec![relation(&[1], 1)],
                    lease,
                    &Cancellation::default(),
                    Instant::now() + Duration::from_secs(5),
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
        let result = seal_fixture(
            Arc::clone(&sink) as Arc<dyn ResultObjectSink>,
            1,
            &[1, 2, 3],
            3,
            &cancellation,
        )
        .await;
        assert!(matches!(result, Err(StreamedResultPackageError::Cancelled)));
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
        let reopened =
            StreamedResultPackageBuilder::new(stable_sink as Arc<dyn ResultObjectSink>, limits(2))
                .reopen(sealed.manifest_path().clone(), epoch, query, lease)
                .await
                .expect("sealed package survives process state");
        assert_eq!(
            reopened.read_page(0, 100).await.expect("page"),
            sealed.read_page(0, 100).await.expect("page")
        );
    }
}
