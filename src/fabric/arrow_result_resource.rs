//! Bounded, epoch-pinned Arrow IPC result resources.
//!
//! Semantic rows remain Arrow from execution through delivery. This module packages each
//! model-supplied relation as one immutable Arrow IPC stream and exposes only lease-checked,
//! deterministic byte-range reads. The small public manifest is a control projection over
//! typed artifact metadata; it never transforms or duplicates semantic rows as JSON.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::sync::{Arc, Mutex};

use arrow_array::RecordBatch;
use arrow_ipc::writer::StreamWriter;
use arrow_schema::{ArrowError, SchemaRef};
use serde::Serialize;
use thiserror::Error;

use crate::relational_program::RelationId;

use super::command::{EpochId, LeaseId};

/// Public media type for every semantic relation subresource.
pub const ARROW_STREAM_MEDIA_TYPE: &str = "application/vnd.apache.arrow.stream";
/// Versioned identity of the application-owned result-resource control contract.
pub const ARROW_RESULT_RESOURCE_FORMAT: &str = "codefabric.arrow-result-resource.v1";

/// Exact Arrow IPC implementation release carried as compatibility metadata.
pub const ARROW_RELEASE: &str = "59.2.0";

/// Exact query execution identity paired with one admitted epoch.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct QueryExecutionPin([u8; 32]);

impl QueryExecutionPin {
    /// Construct a query-execution pin from its canonical bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Borrow the canonical query-execution bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Content-addressed identity of a package or one readable subresource.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResultResourceId([u8; 32]);

impl ResultResourceId {
    /// Borrow the canonical resource identity bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    fn framed(value: &[u8]) -> String {
        format!("b3:{}", hex(value))
    }

    /// Public `b3:` representation used by the manifest and control plane.
    #[must_use]
    pub fn public_id(self) -> String {
        Self::framed(&self.0)
    }
}

/// Exact semantic coverage state supplied with a result relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultCompleteness {
    Complete,
    Partial,
    Unknown,
}

impl ResultCompleteness {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Unknown => "unknown",
        }
    }
}

/// Data-carried explanation for a partial or unknown relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResultUnknownCause(Arc<str>);

impl ResultUnknownCause {
    /// Construct a bounded, non-empty cause code.
    pub fn try_new(value: impl Into<Arc<str>>) -> Result<Self, ArrowResultResourceError> {
        let value = value.into();
        if value.is_empty() || value.len() > 240 || !value.is_ascii() {
            return Err(ArrowResultResourceError::InvalidUnknownCause);
        }
        Ok(Self(value))
    }

    /// Borrow the model/application-supplied cause code.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Counted coverage attached to exactly one relation artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResultCoverage {
    state: ResultCompleteness,
    requested_units: u64,
    completed_units: u64,
    remainder_units: u64,
    unknown_cause: Option<ResultUnknownCause>,
}

impl ResultCoverage {
    /// Construct coverage and verify its exact accounting and cause semantics.
    ///
    /// # Errors
    ///
    /// Rejects arithmetic overflow, mismatched counts, ambiguous completion, or a missing /
    /// extraneous cause. Transport truncation is not representable here: resource overflow
    /// rejects package construction instead.
    pub fn try_new(
        state: ResultCompleteness,
        requested_units: u64,
        completed_units: u64,
        remainder_units: u64,
        unknown_cause: Option<ResultUnknownCause>,
    ) -> Result<Self, ArrowResultResourceError> {
        if completed_units.checked_add(remainder_units) != Some(requested_units) {
            return Err(ArrowResultResourceError::InvalidCoverage(
                "completed plus remainder must equal requested",
            ));
        }
        match state {
            ResultCompleteness::Complete
                if remainder_units == 0
                    && completed_units == requested_units
                    && unknown_cause.is_none() => {}
            ResultCompleteness::Partial
                if requested_units > 0
                    && completed_units > 0
                    && remainder_units > 0
                    && unknown_cause.is_some() => {}
            ResultCompleteness::Unknown
                if requested_units > 0 && remainder_units > 0 && unknown_cause.is_some() => {}
            _ => {
                return Err(ArrowResultResourceError::InvalidCoverage(
                    "state, counts, and unknown cause disagree",
                ));
            }
        }
        Ok(Self {
            state,
            requested_units,
            completed_units,
            remainder_units,
            unknown_cause,
        })
    }

    /// Construct exact complete coverage, including the valid empty-scope case.
    #[must_use]
    pub const fn complete(requested_units: u64) -> Self {
        Self {
            state: ResultCompleteness::Complete,
            requested_units,
            completed_units: requested_units,
            remainder_units: 0,
            unknown_cause: None,
        }
    }

    #[must_use]
    pub const fn state(&self) -> ResultCompleteness {
        self.state
    }

    #[must_use]
    pub const fn requested_units(&self) -> u64 {
        self.requested_units
    }

    #[must_use]
    pub const fn completed_units(&self) -> u64 {
        self.completed_units
    }

    #[must_use]
    pub const fn remainder_units(&self) -> u64 {
        self.remainder_units
    }

    #[must_use]
    pub fn unknown_cause(&self) -> Option<&ResultUnknownCause> {
        self.unknown_cause.as_ref()
    }
}

/// Model-supplied schema, rows, and coverage for one result relation.
#[derive(Clone, Debug)]
pub struct ResultRelationInput {
    relation_id: RelationId,
    schema: SchemaRef,
    batches: Vec<RecordBatch>,
    coverage: ResultCoverage,
}

impl ResultRelationInput {
    #[must_use]
    pub fn new(
        relation_id: RelationId,
        schema: SchemaRef,
        batches: Vec<RecordBatch>,
        coverage: ResultCoverage,
    ) -> Self {
        Self {
            relation_id,
            schema,
            batches,
            coverage,
        }
    }
}

/// Explicit construction and read bounds for one result package.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArrowResultResourceLimits {
    max_relations: usize,
    max_batches_per_relation: usize,
    max_rows_per_relation: u64,
    max_total_batches: usize,
    max_total_rows: u64,
    max_schema_bytes_per_relation: usize,
    max_total_schema_bytes: usize,
    max_ipc_bytes_per_relation: usize,
    max_total_ipc_bytes: usize,
    max_manifest_bytes: usize,
    max_chunk_bytes: usize,
}

impl ArrowResultResourceLimits {
    /// Construct a fully bounded resource envelope.
    ///
    /// # Errors
    ///
    /// Every bound must be nonzero. Empty relations remain valid because their observed row and
    /// batch counts are zero, not because the limits are unbounded.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        max_relations: usize,
        max_batches_per_relation: usize,
        max_rows_per_relation: u64,
        max_total_batches: usize,
        max_total_rows: u64,
        max_schema_bytes_per_relation: usize,
        max_total_schema_bytes: usize,
        max_ipc_bytes_per_relation: usize,
        max_total_ipc_bytes: usize,
        max_manifest_bytes: usize,
        max_chunk_bytes: usize,
    ) -> Result<Self, ArrowResultResourceError> {
        if max_relations == 0
            || max_batches_per_relation == 0
            || max_rows_per_relation == 0
            || max_total_batches == 0
            || max_total_rows == 0
            || max_schema_bytes_per_relation == 0
            || max_total_schema_bytes == 0
            || max_ipc_bytes_per_relation == 0
            || max_total_ipc_bytes == 0
            || max_manifest_bytes == 0
            || max_chunk_bytes == 0
        {
            return Err(ArrowResultResourceError::InvalidLimits);
        }
        Ok(Self {
            max_relations,
            max_batches_per_relation,
            max_rows_per_relation,
            max_total_batches,
            max_total_rows,
            max_schema_bytes_per_relation,
            max_total_schema_bytes,
            max_ipc_bytes_per_relation,
            max_total_ipc_bytes,
            max_manifest_bytes,
            max_chunk_bytes,
        })
    }
}

/// Exact lease identity and caller-supplied wall-clock validity interval.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResultResourceLease {
    lease_id: LeaseId,
    issued_at_unix_ms: i64,
    expires_at_unix_ms: i64,
}

impl ResultResourceLease {
    /// Construct a non-empty lease with a strictly increasing validity interval.
    pub fn try_new(
        lease_id: LeaseId,
        issued_at_unix_ms: i64,
        expires_at_unix_ms: i64,
    ) -> Result<Self, ArrowResultResourceError> {
        if lease_id.as_bytes().iter().all(|byte| *byte == 0)
            || expires_at_unix_ms <= issued_at_unix_ms
        {
            return Err(ArrowResultResourceError::InvalidLease);
        }
        Ok(Self {
            lease_id,
            issued_at_unix_ms,
            expires_at_unix_ms,
        })
    }

    #[must_use]
    pub const fn lease_id(self) -> LeaseId {
        self.lease_id
    }

    #[must_use]
    pub const fn issued_at_unix_ms(self) -> i64 {
        self.issued_at_unix_ms
    }

    #[must_use]
    pub const fn expires_at_unix_ms(self) -> i64 {
        self.expires_at_unix_ms
    }
}

/// Typed immutable metadata for one relation-scoped Arrow IPC artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationArtifactMetadata {
    relation_id: RelationId,
    resource_id: ResultResourceId,
    schema_checksum: [u8; 32],
    schema_byte_length: u64,
    content_checksum: [u8; 32],
    row_count: u64,
    batch_count: u64,
    byte_length: u64,
    coverage: ResultCoverage,
}

impl RelationArtifactMetadata {
    #[must_use]
    pub const fn relation_id(&self) -> &RelationId {
        &self.relation_id
    }

    #[must_use]
    pub const fn resource_id(&self) -> ResultResourceId {
        self.resource_id
    }

    #[must_use]
    pub const fn schema_checksum(&self) -> &[u8; 32] {
        &self.schema_checksum
    }

    #[must_use]
    pub const fn schema_byte_length(&self) -> u64 {
        self.schema_byte_length
    }

    #[must_use]
    pub const fn content_checksum(&self) -> &[u8; 32] {
        &self.content_checksum
    }

    #[must_use]
    pub const fn row_count(&self) -> u64 {
        self.row_count
    }

    #[must_use]
    pub const fn batch_count(&self) -> u64 {
        self.batch_count
    }

    #[must_use]
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    #[must_use]
    pub const fn coverage(&self) -> &ResultCoverage {
        &self.coverage
    }
}

/// Package-level pins and immutable artifact identities returned through the control plane.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArrowResultPackageMetadata {
    package_id: ResultResourceId,
    epoch_id: EpochId,
    query_execution: QueryExecutionPin,
    manifest_resource_id: ResultResourceId,
    manifest_checksum: [u8; 32],
    manifest_byte_length: u64,
    total_rows: u64,
    total_batches: u64,
    total_schema_bytes: u64,
    total_ipc_bytes: u64,
    completion: ResultCompleteness,
    relations: Arc<[RelationArtifactMetadata]>,
}

impl ArrowResultPackageMetadata {
    #[must_use]
    pub const fn package_id(&self) -> ResultResourceId {
        self.package_id
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
    pub const fn manifest_resource_id(&self) -> ResultResourceId {
        self.manifest_resource_id
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
    pub const fn total_rows(&self) -> u64 {
        self.total_rows
    }

    #[must_use]
    pub const fn total_batches(&self) -> u64 {
        self.total_batches
    }

    #[must_use]
    pub const fn total_schema_bytes(&self) -> u64 {
        self.total_schema_bytes
    }

    #[must_use]
    pub const fn total_ipc_bytes(&self) -> u64 {
        self.total_ipc_bytes
    }

    #[must_use]
    pub const fn completion(&self) -> ResultCompleteness {
        self.completion
    }

    #[must_use]
    pub fn relations(&self) -> &[RelationArtifactMetadata] {
        &self.relations
    }
}

/// One deterministic range read from an immutable result resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResultResourceChunk {
    pub resource_id: ResultResourceId,
    pub offset: u64,
    pub next_offset: u64,
    pub total_length: u64,
    pub content_checksum: [u8; 32],
    pub bytes: Arc<[u8]>,
    pub complete: bool,
}

/// Immutable in-memory package with a narrow storage-adapter seam: metadata plus keyed bytes.
///
/// A future persistence adapter can store the manifest and relation byte slices under their
/// `ResultResourceId` values without gaining schema, query, or row-transformation authority.
#[derive(Debug)]
pub struct ArrowResultResourcePackage {
    metadata: ArrowResultPackageMetadata,
    limits: ArrowResultResourceLimits,
    lease: ResultResourceLease,
    released: Mutex<bool>,
    resources: BTreeMap<ResultResourceId, StoredResource>,
    relation_resources: BTreeMap<RelationId, ResultResourceId>,
}

#[derive(Debug)]
struct StoredResource {
    bytes: Arc<[u8]>,
    checksum: [u8; 32],
}

impl ArrowResultResourcePackage {
    /// Build a bounded package from exact epoch/query pins and model-supplied relations.
    ///
    /// # Errors
    ///
    /// Rejects zero pins, duplicate relations, schema drift, row/batch/byte overflow, IPC
    /// encoding failure, or an oversized canonical manifest. No successful return contains
    /// silently truncated semantic bytes.
    pub fn try_new(
        epoch_id: EpochId,
        query_execution: QueryExecutionPin,
        mut relations: Vec<ResultRelationInput>,
        lease: ResultResourceLease,
        limits: ArrowResultResourceLimits,
    ) -> Result<Self, ArrowResultResourceError> {
        if epoch_id.as_bytes().iter().all(|byte| *byte == 0)
            || query_execution.as_bytes().iter().all(|byte| *byte == 0)
        {
            return Err(ArrowResultResourceError::InvalidPins);
        }
        if relations.is_empty() {
            return Err(ArrowResultResourceError::NoRelations);
        }
        if relations.len() > limits.max_relations {
            return Err(ArrowResultResourceError::RelationLimitExceeded {
                observed: relations.len(),
                limit: limits.max_relations,
            });
        }
        relations.sort_by(|left, right| left.relation_id.cmp(&right.relation_id));

        let mut seen = BTreeSet::new();
        let mut artifacts = Vec::with_capacity(relations.len());
        let mut resources = BTreeMap::new();
        let mut relation_resources = BTreeMap::new();
        let mut total_rows = 0_u64;
        let mut total_batches = 0_usize;
        let mut total_schema_bytes = 0_usize;
        let mut total_ipc_bytes = 0_usize;

        for relation in relations {
            let relation_name = relation.relation_id.as_str().to_owned();
            if !seen.insert(relation.relation_id.clone()) {
                return Err(ArrowResultResourceError::DuplicateRelation(relation_name));
            }
            let remaining_schema_bytes = limits
                .max_total_schema_bytes
                .checked_sub(total_schema_bytes)
                .ok_or(ArrowResultResourceError::CounterOverflow)?;
            let effective_schema_limit = limits
                .max_schema_bytes_per_relation
                .min(remaining_schema_bytes);
            let (schema_bytes, schema_limit_exceeded) =
                encode_canonical_schema(&relation.schema, effective_schema_limit);
            let schema_bytes = match schema_bytes {
                Ok(bytes) => bytes,
                Err(_source)
                    if schema_limit_exceeded
                        && remaining_schema_bytes < limits.max_schema_bytes_per_relation =>
                {
                    return Err(ArrowResultResourceError::TotalSchemaByteLimitExceeded {
                        limit: limits.max_total_schema_bytes,
                    });
                }
                Err(_) if schema_limit_exceeded => {
                    return Err(ArrowResultResourceError::SchemaByteLimitExceeded {
                        relation: relation.relation_id.as_str().to_owned(),
                        limit: limits.max_schema_bytes_per_relation,
                    });
                }
                Err(source) => return Err(ArrowResultResourceError::CanonicalManifest(source)),
            };
            total_schema_bytes = total_schema_bytes
                .checked_add(schema_bytes.len())
                .ok_or(ArrowResultResourceError::CounterOverflow)?;
            if relation.batches.len() > limits.max_batches_per_relation {
                return Err(ArrowResultResourceError::BatchLimitExceeded {
                    relation: relation_name,
                    observed: relation.batches.len(),
                    limit: limits.max_batches_per_relation,
                });
            }
            total_batches = total_batches
                .checked_add(relation.batches.len())
                .ok_or(ArrowResultResourceError::CounterOverflow)?;
            if total_batches > limits.max_total_batches {
                return Err(ArrowResultResourceError::TotalBatchLimitExceeded {
                    observed: total_batches,
                    limit: limits.max_total_batches,
                });
            }

            let mut relation_rows = 0_u64;
            for (batch_index, batch) in relation.batches.iter().enumerate() {
                if batch.schema().as_ref() != relation.schema.as_ref() {
                    return Err(ArrowResultResourceError::SchemaDrift {
                        relation: relation.relation_id.as_str().to_owned(),
                        batch_index,
                    });
                }
                relation_rows = relation_rows
                    .checked_add(
                        u64::try_from(batch.num_rows())
                            .map_err(|_| ArrowResultResourceError::CounterOverflow)?,
                    )
                    .ok_or(ArrowResultResourceError::CounterOverflow)?;
            }
            if relation_rows > limits.max_rows_per_relation {
                return Err(ArrowResultResourceError::RowLimitExceeded {
                    relation: relation.relation_id.as_str().to_owned(),
                    observed: relation_rows,
                    limit: limits.max_rows_per_relation,
                });
            }
            total_rows = total_rows
                .checked_add(relation_rows)
                .ok_or(ArrowResultResourceError::CounterOverflow)?;
            if total_rows > limits.max_total_rows {
                return Err(ArrowResultResourceError::TotalRowLimitExceeded {
                    observed: total_rows,
                    limit: limits.max_total_rows,
                });
            }

            let remaining_total = limits
                .max_total_ipc_bytes
                .checked_sub(total_ipc_bytes)
                .ok_or(ArrowResultResourceError::CounterOverflow)?;
            let effective_limit = limits.max_ipc_bytes_per_relation.min(remaining_total);
            let (ipc_bytes, limit_exceeded) =
                encode_ipc(&relation.schema, &relation.batches, effective_limit);
            let ipc_bytes = match ipc_bytes {
                Ok(bytes) => bytes,
                Err(_source)
                    if limit_exceeded && remaining_total < limits.max_ipc_bytes_per_relation =>
                {
                    return Err(ArrowResultResourceError::TotalIpcByteLimitExceeded {
                        limit: limits.max_total_ipc_bytes,
                    });
                }
                Err(_) if limit_exceeded => {
                    return Err(ArrowResultResourceError::IpcByteLimitExceeded {
                        relation: relation.relation_id.as_str().to_owned(),
                        limit: limits.max_ipc_bytes_per_relation,
                    });
                }
                Err(source) => {
                    return Err(ArrowResultResourceError::IpcEncoding {
                        relation: relation.relation_id.as_str().to_owned(),
                        source,
                    });
                }
            };
            total_ipc_bytes = total_ipc_bytes
                .checked_add(ipc_bytes.len())
                .ok_or(ArrowResultResourceError::CounterOverflow)?;

            let schema_checksum = digest_framed([b"schema.v1".as_slice(), &schema_bytes]);
            let content_checksum = digest_framed([b"arrow-ipc-stream.v1".as_slice(), &ipc_bytes]);
            let resource_id = ResultResourceId(digest_framed([
                b"relation-resource.v1".as_slice(),
                epoch_id.as_bytes(),
                query_execution.as_bytes(),
                relation.relation_id.as_str().as_bytes(),
                &schema_checksum,
                &content_checksum,
            ]));
            let byte_length = u64::try_from(ipc_bytes.len())
                .map_err(|_| ArrowResultResourceError::CounterOverflow)?;
            let schema_byte_length = u64::try_from(schema_bytes.len())
                .map_err(|_| ArrowResultResourceError::CounterOverflow)?;
            let batch_count = u64::try_from(relation.batches.len())
                .map_err(|_| ArrowResultResourceError::CounterOverflow)?;
            let artifact = RelationArtifactMetadata {
                relation_id: relation.relation_id.clone(),
                resource_id,
                schema_checksum,
                schema_byte_length,
                content_checksum,
                row_count: relation_rows,
                batch_count,
                byte_length,
                coverage: relation.coverage,
            };
            resources.insert(
                resource_id,
                StoredResource {
                    bytes: Arc::from(ipc_bytes),
                    checksum: content_checksum,
                },
            );
            relation_resources.insert(relation.relation_id, resource_id);
            artifacts.push(artifact);
        }

        let completion = aggregate_completion(&artifacts);
        let total_batches_u64 =
            u64::try_from(total_batches).map_err(|_| ArrowResultResourceError::CounterOverflow)?;
        let total_schema_bytes_u64 = u64::try_from(total_schema_bytes)
            .map_err(|_| ArrowResultResourceError::CounterOverflow)?;
        let total_ipc_bytes_u64 = u64::try_from(total_ipc_bytes)
            .map_err(|_| ArrowResultResourceError::CounterOverflow)?;
        let package_id = ResultResourceId(package_identity(
            epoch_id,
            query_execution,
            &artifacts,
            total_rows,
            total_batches_u64,
            total_schema_bytes_u64,
            total_ipc_bytes_u64,
            completion,
        ));
        let manifest = PublicResultManifest::new(
            package_id,
            epoch_id,
            query_execution,
            total_rows,
            total_batches_u64,
            total_schema_bytes_u64,
            total_ipc_bytes_u64,
            completion,
            &artifacts,
        )?;
        let manifest_bytes = serde_json_canonicalizer::to_vec(&manifest)
            .map_err(ArrowResultResourceError::CanonicalManifest)?;
        if manifest_bytes.len() > limits.max_manifest_bytes {
            return Err(ArrowResultResourceError::ManifestByteLimitExceeded {
                observed: manifest_bytes.len(),
                limit: limits.max_manifest_bytes,
            });
        }
        let manifest_checksum = digest_framed([b"result-manifest.v1".as_slice(), &manifest_bytes]);
        let manifest_resource_id = ResultResourceId(digest_framed([
            b"manifest-resource.v1".as_slice(),
            package_id.as_bytes(),
            &manifest_checksum,
        ]));
        let manifest_byte_length = u64::try_from(manifest_bytes.len())
            .map_err(|_| ArrowResultResourceError::CounterOverflow)?;
        resources.insert(
            manifest_resource_id,
            StoredResource {
                bytes: Arc::from(manifest_bytes),
                checksum: manifest_checksum,
            },
        );
        let metadata = ArrowResultPackageMetadata {
            package_id,
            epoch_id,
            query_execution,
            manifest_resource_id,
            manifest_checksum,
            manifest_byte_length,
            total_rows,
            total_batches: total_batches_u64,
            total_schema_bytes: total_schema_bytes_u64,
            total_ipc_bytes: total_ipc_bytes_u64,
            completion,
            relations: Arc::from(artifacts),
        };
        Ok(Self {
            metadata,
            limits,
            lease,
            released: Mutex::new(false),
            resources,
            relation_resources,
        })
    }

    /// Borrow immutable package and subresource metadata.
    #[must_use]
    pub const fn metadata(&self) -> &ArrowResultPackageMetadata {
        &self.metadata
    }

    /// Return the exact lease control metadata supplied at package construction.
    #[must_use]
    pub const fn lease(&self) -> ResultResourceLease {
        self.lease
    }

    /// Exact bytes retained by the immutable manifest and Arrow IPC resources.
    ///
    /// This is the physical result-storage quantity reserved by epoch resource
    /// governance; schema/row counts remain separately typed metadata.
    pub fn retained_resource_bytes(&self) -> Result<u64, ArrowResultResourceError> {
        self.resources.values().try_fold(0_u64, |total, resource| {
            let bytes = u64::try_from(resource.bytes.len())
                .map_err(|_| ArrowResultResourceError::CounterOverflow)?;
            total
                .checked_add(bytes)
                .ok_or(ArrowResultResourceError::CounterOverflow)
        })
    }

    /// Resolve one model relation to its immutable resource identity.
    #[must_use]
    pub fn relation_resource_id(&self, relation_id: &RelationId) -> Option<ResultResourceId> {
        self.relation_resources.get(relation_id).copied()
    }

    /// Read one deterministic byte range while the exact lease is live.
    ///
    /// # Errors
    ///
    /// Rejects a wrong, released, or expired lease; an unknown resource; an out-of-range offset;
    /// or a zero/over-envelope chunk bound.
    pub fn read_chunk(
        &self,
        lease_id: LeaseId,
        observed_at_unix_ms: i64,
        resource_id: ResultResourceId,
        offset: u64,
        max_bytes: usize,
    ) -> Result<ResultResourceChunk, ArrowResultResourceError> {
        let released = self
            .released
            .lock()
            .map_err(|_| ArrowResultResourceError::LeaseStateUnavailable)?;
        self.validate_lease(*released, lease_id, observed_at_unix_ms)?;
        if max_bytes == 0 {
            return Err(ArrowResultResourceError::ZeroChunkLimit);
        }
        if max_bytes > self.limits.max_chunk_bytes {
            return Err(ArrowResultResourceError::ChunkLimitExceeded {
                requested: max_bytes,
                limit: self.limits.max_chunk_bytes,
            });
        }
        let resource = self
            .resources
            .get(&resource_id)
            .ok_or(ArrowResultResourceError::UnknownResource(resource_id))?;
        let offset =
            usize::try_from(offset).map_err(|_| ArrowResultResourceError::OffsetOutOfRange)?;
        if offset > resource.bytes.len() {
            return Err(ArrowResultResourceError::OffsetOutOfRange);
        }
        let end = offset.saturating_add(max_bytes).min(resource.bytes.len());
        let next_offset =
            u64::try_from(end).map_err(|_| ArrowResultResourceError::CounterOverflow)?;
        let total_length = u64::try_from(resource.bytes.len())
            .map_err(|_| ArrowResultResourceError::CounterOverflow)?;
        Ok(ResultResourceChunk {
            resource_id,
            offset: u64::try_from(offset).map_err(|_| ArrowResultResourceError::CounterOverflow)?,
            next_offset,
            total_length,
            content_checksum: resource.checksum,
            bytes: Arc::from(&resource.bytes[offset..end]),
            complete: end == resource.bytes.len(),
        })
    }

    /// Release this package under the exact live lease.
    ///
    /// # Errors
    ///
    /// Wrong, already-released, and expired leases remain distinguishable failures.
    pub fn release(
        &self,
        lease_id: LeaseId,
        observed_at_unix_ms: i64,
    ) -> Result<(), ArrowResultResourceError> {
        let mut released = self
            .released
            .lock()
            .map_err(|_| ArrowResultResourceError::LeaseStateUnavailable)?;
        self.validate_lease(*released, lease_id, observed_at_unix_ms)?;
        *released = true;
        Ok(())
    }

    fn validate_lease(
        &self,
        released: bool,
        lease_id: LeaseId,
        observed_at_unix_ms: i64,
    ) -> Result<(), ArrowResultResourceError> {
        if lease_id != self.lease.lease_id {
            return Err(ArrowResultResourceError::WrongLease);
        }
        if released {
            return Err(ArrowResultResourceError::Released);
        }
        if observed_at_unix_ms < self.lease.issued_at_unix_ms
            || observed_at_unix_ms >= self.lease.expires_at_unix_ms
        {
            return Err(ArrowResultResourceError::Expired);
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct PublicResultManifest {
    format: &'static str,
    arrow_release: &'static str,
    package_id: String,
    epoch_id: String,
    query_execution: String,
    completion_state: &'static str,
    complete: bool,
    truncated: bool,
    unknown: bool,
    relation_count: u64,
    total_rows: u64,
    total_batches: u64,
    total_schema_bytes: u64,
    total_ipc_bytes: u64,
    subresources: Vec<PublicRelationResource>,
}

impl PublicResultManifest {
    #[allow(clippy::too_many_arguments)]
    fn new(
        package_id: ResultResourceId,
        epoch_id: EpochId,
        query_execution: QueryExecutionPin,
        total_rows: u64,
        total_batches: u64,
        total_schema_bytes: u64,
        total_ipc_bytes: u64,
        completion: ResultCompleteness,
        artifacts: &[RelationArtifactMetadata],
    ) -> Result<Self, ArrowResultResourceError> {
        Ok(Self {
            format: ARROW_RESULT_RESOURCE_FORMAT,
            arrow_release: ARROW_RELEASE,
            package_id: package_id.public_id(),
            epoch_id: hex(epoch_id.as_bytes()),
            query_execution: ResultResourceId::framed(query_execution.as_bytes()),
            completion_state: completion.as_str(),
            complete: completion == ResultCompleteness::Complete,
            truncated: false,
            unknown: completion == ResultCompleteness::Unknown,
            relation_count: u64::try_from(artifacts.len())
                .map_err(|_| ArrowResultResourceError::CounterOverflow)?,
            total_rows,
            total_batches,
            total_schema_bytes,
            total_ipc_bytes,
            subresources: artifacts.iter().map(PublicRelationResource::from).collect(),
        })
    }
}

#[derive(Debug, Serialize)]
struct PublicRelationResource {
    relation_id: String,
    resource_id: String,
    media_type: &'static str,
    schema_checksum: String,
    schema_byte_length: u64,
    content_checksum: String,
    row_count: u64,
    batch_count: u64,
    byte_length: u64,
    completion_state: &'static str,
    requested_units: u64,
    completed_units: u64,
    remainder_units: u64,
    complete: bool,
    truncated: bool,
    unknown: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    unknown_cause: Option<String>,
}

impl From<&RelationArtifactMetadata> for PublicRelationResource {
    fn from(value: &RelationArtifactMetadata) -> Self {
        let state = value.coverage.state;
        Self {
            relation_id: value.relation_id.as_str().to_owned(),
            resource_id: value.resource_id.public_id(),
            media_type: ARROW_STREAM_MEDIA_TYPE,
            schema_checksum: ResultResourceId::framed(&value.schema_checksum),
            schema_byte_length: value.schema_byte_length,
            content_checksum: ResultResourceId::framed(&value.content_checksum),
            row_count: value.row_count,
            batch_count: value.batch_count,
            byte_length: value.byte_length,
            completion_state: state.as_str(),
            requested_units: value.coverage.requested_units,
            completed_units: value.coverage.completed_units,
            remainder_units: value.coverage.remainder_units,
            complete: state == ResultCompleteness::Complete,
            truncated: false,
            unknown: state == ResultCompleteness::Unknown,
            unknown_cause: value
                .coverage
                .unknown_cause
                .as_ref()
                .map(|cause| cause.as_str().to_owned()),
        }
    }
}

fn aggregate_completion(artifacts: &[RelationArtifactMetadata]) -> ResultCompleteness {
    if artifacts
        .iter()
        .any(|artifact| artifact.coverage.state == ResultCompleteness::Unknown)
    {
        ResultCompleteness::Unknown
    } else if artifacts
        .iter()
        .any(|artifact| artifact.coverage.state == ResultCompleteness::Partial)
    {
        ResultCompleteness::Partial
    } else {
        ResultCompleteness::Complete
    }
}

fn package_identity(
    epoch_id: EpochId,
    query_execution: QueryExecutionPin,
    artifacts: &[RelationArtifactMetadata],
    total_rows: u64,
    total_batches: u64,
    total_schema_bytes: u64,
    total_ipc_bytes: u64,
    completion: ResultCompleteness,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    update_framed(&mut hasher, b"result-package.v1");
    update_framed(&mut hasher, epoch_id.as_bytes());
    update_framed(&mut hasher, query_execution.as_bytes());
    update_framed(&mut hasher, completion.as_str().as_bytes());
    update_framed(&mut hasher, &total_rows.to_be_bytes());
    update_framed(&mut hasher, &total_batches.to_be_bytes());
    update_framed(&mut hasher, &total_schema_bytes.to_be_bytes());
    update_framed(&mut hasher, &total_ipc_bytes.to_be_bytes());
    for artifact in artifacts {
        update_framed(&mut hasher, artifact.relation_id.as_str().as_bytes());
        update_framed(&mut hasher, artifact.resource_id.as_bytes());
        update_framed(&mut hasher, &artifact.schema_checksum);
        update_framed(&mut hasher, &artifact.schema_byte_length.to_be_bytes());
        update_framed(&mut hasher, &artifact.content_checksum);
        update_framed(&mut hasher, &artifact.row_count.to_be_bytes());
        update_framed(&mut hasher, &artifact.batch_count.to_be_bytes());
        update_framed(&mut hasher, &artifact.byte_length.to_be_bytes());
        update_framed(&mut hasher, artifact.coverage.state.as_str().as_bytes());
        update_framed(
            &mut hasher,
            &artifact.coverage.requested_units.to_be_bytes(),
        );
        update_framed(
            &mut hasher,
            &artifact.coverage.completed_units.to_be_bytes(),
        );
        update_framed(
            &mut hasher,
            &artifact.coverage.remainder_units.to_be_bytes(),
        );
        update_framed(
            &mut hasher,
            artifact
                .coverage
                .unknown_cause
                .as_ref()
                .map_or(&[][..], |cause| cause.as_str().as_bytes()),
        );
    }
    *hasher.finalize().as_bytes()
}

fn digest_framed<'a>(parts: impl IntoIterator<Item = &'a [u8]>) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    for part in parts {
        update_framed(&mut hasher, part);
    }
    *hasher.finalize().as_bytes()
}

fn update_framed(hasher: &mut blake3::Hasher, part: &[u8]) {
    let length = u64::try_from(part.len()).expect("Rust byte slices fit the u64 digest frame");
    hasher.update(&length.to_be_bytes());
    hasher.update(part);
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn encode_canonical_schema(
    schema: &SchemaRef,
    byte_limit: usize,
) -> (Result<Vec<u8>, serde_json::Error>, bool) {
    let mut output = BoundedVecWriter::new(byte_limit);
    let result = serde_json_canonicalizer::to_writer(schema.as_ref(), &mut output);
    let exceeded = output.exceeded;
    (result.map(|()| output.bytes), exceeded)
}

fn encode_ipc(
    schema: &SchemaRef,
    batches: &[RecordBatch],
    byte_limit: usize,
) -> (Result<Vec<u8>, ArrowError>, bool) {
    let mut output = BoundedVecWriter::new(byte_limit);
    let result = (|| {
        let mut writer = StreamWriter::try_new(&mut output, schema.as_ref())?;
        for batch in batches {
            writer.write(batch)?;
        }
        writer.finish()
    })();
    let exceeded = output.exceeded;
    (result.map(|()| output.bytes), exceeded)
}

struct BoundedVecWriter {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}

impl BoundedVecWriter {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
            exceeded: false,
        }
    }
}

impl io::Write for BoundedVecWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(io::Error::other("Arrow result IPC byte limit exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Stable failures at the immutable Arrow result-resource boundary.
#[derive(Debug, Error)]
pub enum ArrowResultResourceError {
    #[error("INVALID_REQUEST_SCHEMA:RESULT_RESOURCE_LIMITS")]
    InvalidLimits,
    #[error("INVALID_REQUEST_SCHEMA:RESULT_RESOURCE_PINS")]
    InvalidPins,
    #[error("INVALID_REQUEST_SCHEMA:RESULT_RESOURCE_LEASE")]
    InvalidLease,
    #[error("INVALID_REQUEST_SCHEMA:RESULT_UNKNOWN_CAUSE")]
    InvalidUnknownCause,
    #[error("INVALID_REQUEST_SCHEMA:RESULT_COVERAGE:{0}")]
    InvalidCoverage(&'static str),
    #[error("INVALID_REQUEST_SCHEMA:RESULT_RELATIONS_EMPTY")]
    NoRelations,
    #[error("INVALID_REQUEST_SCHEMA:DUPLICATE_RESULT_RELATION:{0}")]
    DuplicateRelation(String),
    #[error("QUERY_HARD_LIMIT_EXCEEDED:RESULT_RELATIONS:{observed}>{limit}")]
    RelationLimitExceeded { observed: usize, limit: usize },
    #[error("QUERY_HARD_LIMIT_EXCEEDED:RESULT_BATCHES:{relation}:{observed}>{limit}")]
    BatchLimitExceeded {
        relation: String,
        observed: usize,
        limit: usize,
    },
    #[error("QUERY_HARD_LIMIT_EXCEEDED:RESULT_TOTAL_BATCHES:{observed}>{limit}")]
    TotalBatchLimitExceeded { observed: usize, limit: usize },
    #[error("QUERY_HARD_LIMIT_EXCEEDED:RESULT_ROWS:{relation}:{observed}>{limit}")]
    RowLimitExceeded {
        relation: String,
        observed: u64,
        limit: u64,
    },
    #[error("QUERY_HARD_LIMIT_EXCEEDED:RESULT_TOTAL_ROWS:{observed}>{limit}")]
    TotalRowLimitExceeded { observed: u64, limit: u64 },
    #[error("QUERY_HARD_LIMIT_EXCEEDED:RESULT_SCHEMA_BYTES:{relation}:{limit}")]
    SchemaByteLimitExceeded { relation: String, limit: usize },
    #[error("QUERY_HARD_LIMIT_EXCEEDED:RESULT_TOTAL_SCHEMA_BYTES:{limit}")]
    TotalSchemaByteLimitExceeded { limit: usize },
    #[error("QUERY_HARD_LIMIT_EXCEEDED:RESULT_IPC_BYTES:{relation}:{limit}")]
    IpcByteLimitExceeded { relation: String, limit: usize },
    #[error("QUERY_HARD_LIMIT_EXCEEDED:RESULT_TOTAL_IPC_BYTES:{limit}")]
    TotalIpcByteLimitExceeded { limit: usize },
    #[error("QUERY_HARD_LIMIT_EXCEEDED:RESULT_MANIFEST_BYTES:{observed}>{limit}")]
    ManifestByteLimitExceeded { observed: usize, limit: usize },
    #[error("INVALID_REQUEST_SCHEMA:RESULT_SCHEMA_DRIFT:{relation}:{batch_index}")]
    SchemaDrift {
        relation: String,
        batch_index: usize,
    },
    #[error("INTERNAL_INVARIANT_VIOLATION:RESULT_COUNTER_OVERFLOW")]
    CounterOverflow,
    #[error("INTERNAL_INVARIANT_VIOLATION:RESULT_IPC_ENCODING:{relation}:{source}")]
    IpcEncoding {
        relation: String,
        #[source]
        source: ArrowError,
    },
    #[error("INTERNAL_INVARIANT_VIOLATION:RESULT_MANIFEST_CANONICALIZATION:{0}")]
    CanonicalManifest(#[source] serde_json::Error),
    #[error("RESULT_RESOURCE_WRONG_LEASE")]
    WrongLease,
    #[error("RESULT_RESOURCE_RELEASED")]
    Released,
    #[error("RESULT_RESOURCE_EXPIRED")]
    Expired,
    #[error("RESULT_RESOURCE_UNKNOWN:{0:?}")]
    UnknownResource(ResultResourceId),
    #[error("INVALID_REQUEST_SCHEMA:RESULT_RESOURCE_OFFSET")]
    OffsetOutOfRange,
    #[error("INVALID_REQUEST_SCHEMA:RESULT_RESOURCE_CHUNK_ZERO")]
    ZeroChunkLimit,
    #[error("QUERY_HARD_LIMIT_EXCEEDED:RESULT_RESOURCE_CHUNK:{requested}>{limit}")]
    ChunkLimitExceeded { requested: usize, limit: usize },
    #[error("INTERNAL_INVARIANT_VIOLATION:RESULT_RESOURCE_LEASE_STATE")]
    LeaseStateUnavailable,
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use arrow_array::{Int64Array, RecordBatch, StringArray};
    use arrow_ipc::reader::StreamReader;
    use arrow_schema::{DataType, Field, Schema};

    use super::*;

    fn limits() -> ArrowResultResourceLimits {
        ArrowResultResourceLimits::try_new(
            8,
            8,
            100,
            16,
            200,
            1 << 20,
            2 << 20,
            1 << 20,
            2 << 20,
            1 << 20,
            37,
        )
        .unwrap()
    }

    fn epoch() -> EpochId {
        EpochId::from_bytes([0x11; 16])
    }

    fn query() -> QueryExecutionPin {
        QueryExecutionPin::from_bytes([0x22; 32])
    }

    fn lease() -> ResultResourceLease {
        ResultResourceLease::try_new(LeaseId::from_bytes([0x33; 16]), 1_000, 2_000).unwrap()
    }

    fn string_schema() -> SchemaRef {
        Arc::new(Schema::new(vec![Field::new(
            "value",
            DataType::Utf8,
            false,
        )]))
    }

    fn int_schema() -> SchemaRef {
        Arc::new(Schema::new(vec![Field::new(
            "number",
            DataType::Int64,
            false,
        )]))
    }

    fn string_relation(id: &str, values: &[&str]) -> ResultRelationInput {
        let schema = string_schema();
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![Arc::new(StringArray::from(values.to_vec()))],
        )
        .unwrap();
        ResultRelationInput::new(
            RelationId::new(id).unwrap(),
            schema,
            vec![batch],
            ResultCoverage::complete(u64::try_from(values.len()).unwrap()),
        )
    }

    fn package(relations: Vec<ResultRelationInput>) -> ArrowResultResourcePackage {
        ArrowResultResourcePackage::try_new(epoch(), query(), relations, lease(), limits()).unwrap()
    }

    fn read_all(
        package: &ArrowResultResourcePackage,
        resource_id: ResultResourceId,
    ) -> (Vec<u8>, [u8; 32]) {
        let mut offset = 0;
        let mut bytes = Vec::new();
        let checksum = loop {
            let chunk = package
                .read_chunk(lease().lease_id(), 1_500, resource_id, offset, 17)
                .unwrap();
            assert_eq!(chunk.offset, offset);
            bytes.extend_from_slice(&chunk.bytes);
            offset = chunk.next_offset;
            if chunk.complete {
                assert_eq!(chunk.total_length, offset);
                break chunk.content_checksum;
            }
        };
        (bytes, checksum)
    }

    #[test]
    fn empty_relation_retains_its_schema_and_explicit_completion() {
        let schema = Arc::new(Schema::new(vec![Field::new(
            "entity_id",
            DataType::Utf8,
            false,
        )]));
        let relation_id = RelationId::new("public.empty_entities").unwrap();
        let package = package(vec![ResultRelationInput::new(
            relation_id.clone(),
            Arc::clone(&schema),
            Vec::new(),
            ResultCoverage::complete(0),
        )]);
        let resource_id = package.relation_resource_id(&relation_id).unwrap();
        let (bytes, _) = read_all(&package, resource_id);
        let mut reader = StreamReader::try_new(Cursor::new(bytes), None).unwrap();
        assert_eq!(reader.schema().as_ref(), schema.as_ref());
        assert!(reader.next().is_none());
        assert!(reader.is_finished());

        let (manifest, _) = read_all(&package, package.metadata().manifest_resource_id());
        let manifest: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
        assert_eq!(manifest["complete"], true);
        assert_eq!(manifest["truncated"], false);
        assert_eq!(manifest["unknown"], false);
        assert_eq!(manifest["subresources"][0]["row_count"], 0);
    }

    #[test]
    fn multiple_model_relations_keep_distinct_arrow_schemas() {
        let first = string_relation("public.names", &["Ada", "Grace"]);
        let int_schema = int_schema();
        let int_batch = RecordBatch::try_new(
            Arc::clone(&int_schema),
            vec![Arc::new(Int64Array::from(vec![7_i64, 9]))],
        )
        .unwrap();
        let second_id = RelationId::new("public.counts").unwrap();
        let second = ResultRelationInput::new(
            second_id.clone(),
            Arc::clone(&int_schema),
            vec![int_batch],
            ResultCoverage::complete(2),
        );
        let package = package(vec![first, second]);
        let (bytes, _) = read_all(&package, package.relation_resource_id(&second_id).unwrap());
        let reader = StreamReader::try_new(Cursor::new(bytes), None).unwrap();
        assert_eq!(reader.schema().as_ref(), int_schema.as_ref());
        assert_eq!(package.metadata().relations().len(), 2);
        assert_ne!(
            package.metadata().relations()[0].schema_checksum(),
            package.metadata().relations()[1].schema_checksum()
        );
    }

    #[test]
    fn schema_drift_is_rejected_before_publication() {
        let declared = string_schema();
        let actual = int_schema();
        let batch =
            RecordBatch::try_new(actual, vec![Arc::new(Int64Array::from(vec![1_i64]))]).unwrap();
        let error = ArrowResultResourcePackage::try_new(
            epoch(),
            query(),
            vec![ResultRelationInput::new(
                RelationId::new("public.drift").unwrap(),
                declared,
                vec![batch],
                ResultCoverage::complete(1),
            )],
            lease(),
            limits(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ArrowResultResourceError::SchemaDrift { .. }
        ));
    }

    #[test]
    fn canonical_schema_bytes_obey_relation_and_package_bounds() {
        let relation_limits = ArrowResultResourceLimits::try_new(
            2,
            2,
            10,
            2,
            10,
            16,
            1 << 20,
            1 << 20,
            1 << 20,
            1 << 20,
            64,
        )
        .unwrap();
        let relation_error = ArrowResultResourcePackage::try_new(
            epoch(),
            query(),
            vec![string_relation("public.schema_bytes", &["row"])],
            lease(),
            relation_limits,
        )
        .unwrap_err();
        assert!(matches!(
            relation_error,
            ArrowResultResourceError::SchemaByteLimitExceeded { .. }
        ));

        let one_schema_bytes = serde_json_canonicalizer::to_vec(string_schema().as_ref())
            .unwrap()
            .len();
        let package_limits = ArrowResultResourceLimits::try_new(
            2,
            2,
            10,
            2,
            10,
            1 << 20,
            one_schema_bytes,
            1 << 20,
            1 << 20,
            1 << 20,
            64,
        )
        .unwrap();
        let package_error = ArrowResultResourcePackage::try_new(
            epoch(),
            query(),
            vec![
                string_relation("public.schema_alpha", &["a"]),
                string_relation("public.schema_beta", &["b"]),
            ],
            lease(),
            package_limits,
        )
        .unwrap_err();
        assert!(matches!(
            package_error,
            ArrowResultResourceError::TotalSchemaByteLimitExceeded { .. }
        ));
    }

    #[test]
    fn row_and_ipc_byte_overflow_fail_without_truncation() {
        let row_limits = ArrowResultResourceLimits::try_new(
            2,
            2,
            1,
            2,
            1,
            1 << 20,
            1 << 20,
            1 << 20,
            1 << 20,
            1 << 20,
            64,
        )
        .unwrap();
        let row_error = ArrowResultResourcePackage::try_new(
            epoch(),
            query(),
            vec![string_relation("public.rows", &["a", "b"])],
            lease(),
            row_limits,
        )
        .unwrap_err();
        assert!(matches!(
            row_error,
            ArrowResultResourceError::RowLimitExceeded { .. }
        ));

        let byte_limits = ArrowResultResourceLimits::try_new(
            2,
            2,
            10,
            2,
            10,
            1 << 20,
            1 << 20,
            16,
            16,
            1 << 20,
            16,
        )
        .unwrap();
        let byte_error = ArrowResultResourcePackage::try_new(
            epoch(),
            query(),
            vec![string_relation("public.bytes", &["semantic bytes"])],
            lease(),
            byte_limits,
        )
        .unwrap_err();
        assert!(matches!(
            byte_error,
            ArrowResultResourceError::IpcByteLimitExceeded { .. }
                | ArrowResultResourceError::TotalIpcByteLimitExceeded { .. }
        ));

        let batch_limits = ArrowResultResourceLimits::try_new(
            2,
            1,
            10,
            1,
            10,
            1 << 20,
            1 << 20,
            1 << 20,
            1 << 20,
            1 << 20,
            64,
        )
        .unwrap();
        let schema = string_schema();
        let batches = ["a", "b"]
            .into_iter()
            .map(|value| {
                RecordBatch::try_new(
                    Arc::clone(&schema),
                    vec![Arc::new(StringArray::from(vec![value]))],
                )
                .unwrap()
            })
            .collect();
        let batch_error = ArrowResultResourcePackage::try_new(
            epoch(),
            query(),
            vec![ResultRelationInput::new(
                RelationId::new("public.batches").unwrap(),
                schema,
                batches,
                ResultCoverage::complete(2),
            )],
            lease(),
            batch_limits,
        )
        .unwrap_err();
        assert!(matches!(
            batch_error,
            ArrowResultResourceError::BatchLimitExceeded { .. }
        ));
    }

    #[test]
    fn unknown_coverage_is_explicit_and_never_transport_truncation() {
        let schema = string_schema();
        let relation_id = RelationId::new("public.unknown_members").unwrap();
        let coverage = ResultCoverage::try_new(
            ResultCompleteness::Unknown,
            1,
            0,
            1,
            Some(ResultUnknownCause::try_new("provider_unavailable").unwrap()),
        )
        .unwrap();
        let package = package(vec![ResultRelationInput::new(
            relation_id,
            schema,
            Vec::new(),
            coverage,
        )]);
        let (manifest, _) = read_all(&package, package.metadata().manifest_resource_id());
        let manifest: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
        assert_eq!(manifest["completion_state"], "unknown");
        assert_eq!(manifest["complete"], false);
        assert_eq!(manifest["truncated"], false);
        assert_eq!(manifest["unknown"], true);
        assert_eq!(
            manifest["subresources"][0]["unknown_cause"],
            "provider_unavailable"
        );
    }

    #[test]
    fn chunks_reassemble_exact_bytes_and_checksum() {
        let relation_id = RelationId::new("public.chunked").unwrap();
        let package = package(vec![string_relation(
            relation_id.as_str(),
            &["one", "two", "three"],
        )]);
        let resource_id = package.relation_resource_id(&relation_id).unwrap();
        let (bytes, declared_checksum) = read_all(&package, resource_id);
        assert_eq!(
            digest_framed([b"arrow-ipc-stream.v1".as_slice(), bytes.as_slice()]),
            declared_checksum
        );
        let reader = StreamReader::try_new(Cursor::new(bytes), None).unwrap();
        let batches = reader.collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(batches.iter().map(RecordBatch::num_rows).sum::<usize>(), 3);
    }

    #[test]
    fn wrong_released_and_expired_leases_are_distinct() {
        let package = package(vec![string_relation("public.leased", &["row"])]);
        let resource = package.metadata().manifest_resource_id();
        assert!(matches!(
            package.read_chunk(LeaseId::from_bytes([0x44; 16]), 1_500, resource, 1, 1),
            Err(ArrowResultResourceError::WrongLease)
        ));
        assert!(matches!(
            package.read_chunk(lease().lease_id(), 2_000, resource, 0, 1),
            Err(ArrowResultResourceError::Expired)
        ));
        package.release(lease().lease_id(), 1_500).unwrap();
        assert!(matches!(
            package.read_chunk(lease().lease_id(), 1_500, resource, 0, 1),
            Err(ArrowResultResourceError::Released)
        ));
        assert!(matches!(
            package.release(lease().lease_id(), 1_500),
            Err(ArrowResultResourceError::Released)
        ));
    }

    #[test]
    fn deterministic_rebuild_ignores_input_relation_order() {
        let left = package(vec![
            string_relation("public.zeta", &["z"]),
            string_relation("public.alpha", &["a"]),
        ]);
        let right = package(vec![
            string_relation("public.alpha", &["a"]),
            string_relation("public.zeta", &["z"]),
        ]);
        assert_eq!(left.metadata(), right.metadata());
        let (left_manifest, _) = read_all(&left, left.metadata().manifest_resource_id());
        let (right_manifest, _) = read_all(&right, right.metadata().manifest_resource_id());
        assert_eq!(left_manifest, right_manifest);
        assert_eq!(
            left_manifest,
            serde_json_canonicalizer::to_vec(
                &serde_json::from_slice::<serde_json::Value>(&left_manifest).unwrap()
            )
            .unwrap()
        );
    }
}
