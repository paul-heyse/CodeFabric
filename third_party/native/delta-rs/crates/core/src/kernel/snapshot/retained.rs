//! Borrowed access to the original allocations retained by Delta snapshots.
//!
//! These views perform no replay, projection, serialization or I/O. They expose ownership
//! roots for resource integrations; they do not estimate complete heap usage or reserve
//! capacity before decoding. The opt-in cache admission below owns only the cache identity and batch descriptors.
//! Kernel schema/CRC/log-path allocations and independently escaping Arrow owners
//! still require native admission and lifetime-bound receipts.

use std::fmt::Debug;
use std::ops::Deref;
use std::sync::Arc;

use super::stream::SharedNativeRecordBatch;
use arrow_array::RecordBatch;
use delta_kernel::PredicateRef;
use delta_kernel::engine::arrow_data::ArrowEngineData;
use delta_kernel::snapshot::Snapshot as KernelSnapshot;
use url::Url;

use super::{EagerSnapshot, MaterializedFiles, Metadata, Protocol, Snapshot};
use crate::logstore::LogStore;
use crate::{DeltaResult, DeltaTableConfig, DeltaTableError};
pub use delta_kernel::resource::ResourceExhausted;

/// A borrowed view of the state actually retained by a snapshot.
///
/// File-cache absence is distinct from a materialized empty cache. The configured
/// `require_files` preference is available separately through [`Self::configuration`].
/// Accessors do not clone native state or initialize a lazy file cache.
#[derive(Clone, Copy)]
pub struct SnapshotRetainedState<'a> {
    snapshot: &'a Snapshot,
}

impl<'a> SnapshotRetainedState<'a> {
    /// The original kernel snapshot owner, containing configuration, log paths and CRC.
    ///
    /// This is a borrowed Arc, not a copied kernel snapshot. A caller may retain a shared
    /// owner, but doing so does not attach or transfer a resource receipt. Existing kernel
    /// schema/CRC accessors can expose additional owners with independent lifetimes.
    pub fn kernel_snapshot(&self) -> &'a Arc<KernelSnapshot> {
        &self.snapshot.inner
    }

    /// The original Delta configuration, including any configured I/O runtime handle.
    ///
    /// The runtime's internal allocations are not described by this borrowed value.
    pub fn configuration(&self) -> &'a DeltaTableConfig {
        &self.snapshot.config
    }

    /// The actual retained materialized cache, or `None` when files are not materialized.
    ///
    /// Unlike semantic cache selection, this includes a retained cache even if its identity
    /// or policy no longer matches the snapshot. Resource accounting must not ignore held
    /// allocations merely because they cannot serve a read. Check
    /// [`MaterializedFilesRetainedState::matches_snapshot`] before treating it as a cache
    /// for the current snapshot; this API itself does not select Delta file authority.
    pub fn materialized_files(&self) -> Option<MaterializedFilesRetainedState<'a>> {
        self.snapshot
            .materialized_files
            .as_ref()
            .map(|files| MaterializedFilesRetainedState {
                files,
                matches_snapshot: files.is_cache_for(self.snapshot),
            })
    }
}

/// Borrowed original materialized file backing and its independently owned identity state.
///
/// Metadata, protocol and table URL here belong to the materialized cache identity. They
/// may be separate allocations from the equal values in the kernel snapshot configuration.
/// No accessor returns a serialized copy or a projected/replayed batch.
#[derive(Clone, Copy)]
pub struct MaterializedFilesRetainedState<'a> {
    files: &'a Arc<MaterializedFiles>,
    matches_snapshot: bool,
}

impl<'a> MaterializedFilesRetainedState<'a> {
    /// The original shared batch container, including the original Arrow array backing.
    ///
    /// `Some(cache)` with an empty container means materialized empty state, not lazy state.
    /// Cloning this Arc shares the container. Cloned batches, file views or Arrow arrays can
    /// outlive that container, so Arrow accounting must follow underlying allocation owners.
    /// The container receipt stays with this wrapper; no raw shared container escapes it.
    pub fn batches(&self) -> &'a Arc<MaterializedFileBatches> {
        &self.files.batches
    }

    /// Whether this retained cache currently satisfies the owning snapshot's identity/policy.
    pub fn matches_snapshot(&self) -> bool {
        self.matches_snapshot
    }

    /// Original owned metadata in the cache identity, without cloning its strings or maps.
    pub fn identity_metadata(&self) -> &'a Metadata {
        &self.files.identity.metadata
    }

    /// Original owned protocol in the cache identity.
    pub fn identity_protocol(&self) -> &'a Protocol {
        &self.files.identity.protocol
    }

    /// Original owned table URL in the cache identity.
    pub fn identity_table_root(&self) -> &'a Url {
        &self.files.identity.table_root
    }

    /// Original shared predicate retained alongside the batch cache, when present.
    pub fn existing_predicate(&self) -> Option<&'a PredicateRef> {
        self.files.existing_predicate.as_ref()
    }
}

impl Snapshot {
    /// Inspect the original retained state without loading or copying it.
    pub fn retained_state(&self) -> SnapshotRetainedState<'_> {
        SnapshotRetainedState { snapshot: self }
    }
}

impl EagerSnapshot {
    /// Borrow the original shared Delta snapshot owner.
    ///
    /// Clones of an eager snapshot refer to this same Arc until a native snapshot update
    /// replaces it. This accessor does not expose mutable state or an engine handle.
    pub fn retained_snapshot_owner(&self) -> &Arc<Snapshot> {
        &self.snapshot
    }

    /// Inspect original retained state, preserving lazy versus materialized-empty distinction.
    pub fn retained_state(&self) -> SnapshotRetainedState<'_> {
        self.snapshot.retained_state()
    }
}

/// Finite collection bounds, enforced independently of Arrow decoder allocation limits.
#[derive(Debug, Clone, Copy)]
pub struct MaterializedFilesLimits {
    /// Maximum number of batch descriptors retained in the cache.
    pub max_batches: usize,
    /// Maximum total file rows retained in the cache.
    pub max_rows: usize,
}

/// Original identity inputs which native code will deep-clone only after admission.
#[derive(Debug, Clone, Copy)]
pub struct MaterializedFilesIdentityRequest<'a> {
    /// Actual original metadata, including its owned strings and collections.
    pub metadata: &'a Metadata,
    /// Actual original protocol and feature collections.
    pub protocol: &'a Protocol,
    /// Actual original table URL.
    pub table_root: &'a Url,
    /// Inline cache owner layout. Referenced identity heap is additional.
    pub owner_bytes: usize,
}

/// Checked geometry for the original batch descriptor container.
#[derive(Debug, Clone, Copy)]
pub struct MaterializedFilesBatchRequest {
    /// Enforced collection limits, not a limit on native Arrow buffer allocation.
    pub limits: MaterializedFilesLimits,
    /// Checked maximum batch count multiplied by `size_of::<RecordBatch>()`.
    pub descriptor_bytes: usize,
    /// Inline descriptor owner layout, additional to `descriptor_bytes`.
    pub owner_bytes: usize,
}

/// A pre-admitted resource held by one original cache allocation owner.
///
/// This wrapper deliberately cannot be cloned. Applications supply an already acquired
/// reservation whose Drop releases capacity; native code moves it to the owner, after
/// admission and before allocation. It is neither an estimator nor evidence that the
/// caller reserved enough capacity. Allocator bookkeeping and all referenced Arrow
/// backing require their own native allocation contract.
#[derive(Debug)]
pub struct MaterializedFilesReceipt {
    _resource: Box<dyn Debug + Send + Sync>,
}

impl MaterializedFilesReceipt {
    /// Move an application reservation into its native lifetime owner.
    pub fn new(resource: impl Debug + Send + Sync + 'static) -> Self {
        Self {
            _resource: Box::new(resource),
        }
    }
}

/// Fallible policy for newly allocated materialized cache identity and descriptors.
///
/// Both reservations happen before the first stream is constructed or polled. Identity
/// inputs are borrowed from the actual snapshot; descriptor geometry is derived natively
/// from the policy's enforced maximum. Implementations must pre-admit their requested
/// allocation envelope. These hooks do not account for kernel snapshots, replay workers,
/// Arrow arrays, predicate backing, schema owners, or allocations made by file-view clones.
///
/// This policy is carried through native snapshot updates. Existing unadmitted caches and
/// serde reconstruction are not silently retrofitted with a receipt.
pub trait MaterializedFilesAdmission: Debug + Send + Sync {
    /// Return finite collection limits selected before collection starts.
    fn limits(&self) -> MaterializedFilesLimits;

    /// Reserve the new cache's owned identity, including actual deep-cloned heap state.
    fn try_reserve_identity(
        &self,
        request: MaterializedFilesIdentityRequest<'_>,
    ) -> Result<MaterializedFilesReceipt, ResourceExhausted>;

    /// Reserve the original descriptor collection, independently of identity lifetime.
    fn try_reserve_batches(
        &self,
        request: MaterializedFilesBatchRequest,
    ) -> Result<MaterializedFilesReceipt, ResourceExhausted>;
}

/// Original immutable batch container with an optional native descriptor receipt.
///
/// Cloning the enclosing Arc shares its receipt and original vector. Borrowed access does
/// not expose the vector or its capacity mutably. Cloning a batch or extracting its arrays
/// can outlive this owner and is *not* covered by the descriptor receipt.
#[derive(Debug)]
pub struct MaterializedFileBatches {
    batches: Vec<SharedNativeRecordBatch>,
    // Drop after the vector; release only when the last original container owner is gone.
    _receipt: Option<MaterializedFilesReceipt>,
}

impl MaterializedFileBatches {
    pub(super) fn unadmitted(batches: Vec<RecordBatch>) -> DeltaResult<Self> {
        // Reconstruction cannot adopt backing produced outside a required native owner.
        if delta_kernel::engine::arrow_data::NativeDataOwners::current().is_governed() {
            return Err(resource_error("materialized_unadmitted_input", 1, 0));
        }
        Ok(Self {
            batches: batches
                .into_iter()
                .map(|batch| {
                    ArrowEngineData::new(batch)
                        .into_owned_record_batch()
                        .try_into_shared()
                })
                .collect::<Result<_, _>>()?,
            _receipt: None,
        })
    }

    /// Capacity of the original descriptor allocation, without allocating or copying it.
    pub fn descriptor_capacity(&self) -> usize {
        self.batches.capacity()
    }
}

impl Deref for MaterializedFileBatches {
    type Target = [SharedNativeRecordBatch];

    fn deref(&self) -> &Self::Target {
        &self.batches
    }
}

impl AsRef<[SharedNativeRecordBatch]> for MaterializedFileBatches {
    fn as_ref(&self) -> &[SharedNativeRecordBatch] {
        &self.batches
    }
}

impl PartialEq for MaterializedFileBatches {
    fn eq(&self, other: &Self) -> bool {
        self.batches == other.batches
    }
}

// Runtime policy and resource receipts do not alter snapshot/cache semantic equality.
impl PartialEq for Snapshot {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
            && self.config == other.config
            && self.materialized_files == other.materialized_files
    }
}

impl PartialEq for MaterializedFiles {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
            && self.policy == other.policy
            && self.scope == other.scope
            && self.existing_predicate == other.existing_predicate
            && self.batches == other.batches
    }
}

fn resource_error(kind: &'static str, requested: usize, limit: usize) -> DeltaTableError {
    delta_kernel::Error::ResourceExhausted(ResourceExhausted {
        kind,
        requested,
        limit,
    })
    .into()
}

pub(super) struct MaterializedFilesCollection {
    batches: MaterializedFileBatches,
    identity_receipt: Option<MaterializedFilesReceipt>,
    limits: Option<MaterializedFilesLimits>,
    rows: usize,
}

impl MaterializedFilesCollection {
    pub(super) fn try_new(
        snapshot: &Snapshot,
        admission: Option<&dyn MaterializedFilesAdmission>,
    ) -> DeltaResult<Self> {
        let Some(admission) = admission else {
            return Ok(Self {
                batches: MaterializedFileBatches {
                    batches: Vec::new(),
                    _receipt: None,
                },
                identity_receipt: None,
                limits: None,
                rows: 0,
            });
        };
        let limits = admission.limits();
        let descriptor_bytes = limits
            .max_batches
            .checked_mul(std::mem::size_of::<SharedNativeRecordBatch>())
            .ok_or_else(|| {
                resource_error(
                    "materialized_batch_descriptor_count",
                    limits.max_batches,
                    usize::MAX / std::mem::size_of::<SharedNativeRecordBatch>(),
                )
            })?;
        let identity_receipt = admission
            .try_reserve_identity(MaterializedFilesIdentityRequest {
                metadata: snapshot.metadata(),
                protocol: snapshot.protocol(),
                table_root: snapshot.inner.table_root(),
                owner_bytes: std::mem::size_of::<MaterializedFiles>(),
            })
            .map_err(delta_kernel::Error::from)?;
        let batch_receipt = admission
            .try_reserve_batches(MaterializedFilesBatchRequest {
                limits,
                descriptor_bytes,
                owner_bytes: std::mem::size_of::<MaterializedFileBatches>(),
            })
            .map_err(delta_kernel::Error::from)?;
        let mut batches = MaterializedFileBatches {
            batches: Vec::new(),
            _receipt: Some(batch_receipt),
        };
        batches
            .batches
            .try_reserve_exact(limits.max_batches)
            .map_err(|_| {
                resource_error("materialized_descriptor_allocator", descriptor_bytes, 0)
            })?;
        Ok(Self {
            batches,
            identity_receipt: Some(identity_receipt),
            limits: Some(limits),
            rows: 0,
        })
    }

    pub(super) fn push(&mut self, batch: SharedNativeRecordBatch) -> DeltaResult<()> {
        if let Some(limits) = self.limits {
            if self.batches.len() >= limits.max_batches {
                return Err(resource_error(
                    "materialized_batch_count",
                    self.batches.len().saturating_add(1),
                    limits.max_batches,
                ));
            }
            let rows = self.rows.checked_add(batch.num_rows()).ok_or_else(|| {
                resource_error("materialized_row_count", usize::MAX, limits.max_rows)
            })?;
            if rows > limits.max_rows {
                return Err(resource_error(
                    "materialized_row_count",
                    rows,
                    limits.max_rows,
                ));
            }
            self.rows = rows;
        }
        self.batches.batches.push(batch);
        Ok(())
    }

    pub(super) fn finish(self, snapshot: &Snapshot) -> MaterializedFiles {
        MaterializedFiles {
            identity: snapshot.identity(),
            policy: snapshot.materialized_files_policy(),
            scope: super::MaterializedFilesScope::FullTable,
            existing_predicate: None,
            batches: Arc::new(self.batches),
            _identity_receipt: self.identity_receipt,
        }
    }
}

impl Snapshot {
    /// Materialize the original full-table file cache using the supplied native engine.
    ///
    /// Any installed cache policy is admitted before constructing the replay stream. Existing
    /// valid cache owners are shared without another reservation or identity clone.
    pub async fn try_materialize_files_with_engine(
        self: Arc<Self>,
        engine: Arc<dyn delta_kernel::Engine>,
    ) -> DeltaResult<Arc<Self>> {
        self.materialize_files_with_engine(engine, None).await
    }

    /// Install admission before any original materialized cache is created.
    ///
    /// A retained cache is rejected even if semantically invalid. Attaching a receipt after
    /// allocation would not prove pre-admission. The kernel snapshot was already allocated;
    /// this method makes no resource claim for that independent native owner.
    pub fn with_materialized_files_admission(
        mut self,
        admission: Arc<dyn MaterializedFilesAdmission>,
    ) -> DeltaResult<Self> {
        if self.materialized_files.is_some() {
            return Err(DeltaTableError::Generic(
                "materialized cache admission must precede cache construction".into(),
            ));
        }
        self.materialized_files_admission = Some(admission);
        Ok(self)
    }
}

impl EagerSnapshot {
    /// Build a snapshot whose new cache identity and descriptor collection are pre-admitted.
    ///
    /// Kernel snapshot/engine allocation is governed separately. A lazy configuration keeps
    /// the policy for later native materialization and updates without collecting any files.
    pub async fn try_new_with_admission(
        log_store: &dyn LogStore,
        config: DeltaTableConfig,
        version: Option<delta_kernel::Version>,
        admission: Arc<dyn MaterializedFilesAdmission>,
    ) -> DeltaResult<Self> {
        let snapshot = Snapshot::try_new(log_store, config, version)
            .await?
            .with_materialized_files_admission(admission)?;
        Self::try_new_with_snapshot(log_store, Arc::new(snapshot)).await
    }
}
