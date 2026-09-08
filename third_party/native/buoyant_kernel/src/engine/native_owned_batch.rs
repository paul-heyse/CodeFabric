//! Explicit native lifetime transfer for Arrow batches. These owners preserve
//! previous admission; cloning a handle never admits a new data allocation.
use std::sync::Arc;

use crate::arrow::array::RecordBatch;
use crate::resource::{AllocationRequest, NativeResourceScope, ResourceExhausted};
use crate::DeltaResult;

/// Owners of native backing, including metadata and arrays without data buffers.
/// A transformed batch can inherit a preceding owner's lifetime while adding the
/// current native allocation scope. The inherited node is admitted before allocation.
#[derive(Clone, Default)]
pub struct NativeDataOwners {
    scope: Option<Arc<NativeResourceScope>>,
    #[cfg(feature = "arrow-59")]
    json: Option<crate::arrow::json::resource::ReaderResourcePolicy>,
    #[cfg(feature = "arrow-59")]
    parquet: Option<crate::parquet::resource::ReaderResourcePolicy>,
    inherited: Option<Arc<(NativeDataOwners, NativeDataOwners)>>,
    inherited_depth: usize,
}
impl std::fmt::Debug for NativeDataOwners {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeDataOwners")
            .field("governed", &self.is_governed())
            .field("inherited_depth", &self.inherited_depth)
            .finish_non_exhaustive()
    }
}
impl NativeDataOwners {
    /// Hard structural cap on this native pair tree's recursive drop path.
    /// This is independent of an application's immutable owner-group DAG bound.
    pub const MAX_INHERITED_DEPTH: usize = 64;

    /// Cached pair-tree depth, preserved by Clone without traversing ancestry.
    pub fn inherited_depth(&self) -> usize {
        self.inherited_depth
    }

    /// Capture current native lifetime handles. This does not account for or
    /// authorize allocations already performed outside their admitted producers.
    pub fn current() -> Self {
        Self {
            scope: crate::resource::current_resource_scope(),
            #[cfg(feature = "arrow-59")]
            json: crate::arrow::json::resource::ReaderResourcePolicy::current(),
            #[cfg(feature = "arrow-59")]
            parquet: crate::parquet::resource::ReaderResourcePolicy::current(),
            inherited: None,
            inherited_depth: 0,
        }
    }
    pub fn native_scope(&self) -> Option<&Arc<NativeResourceScope>> {
        self.scope.as_ref()
    }
    #[cfg(feature = "arrow-59")]
    pub fn json_policy(&self) -> Option<&crate::arrow::json::resource::ReaderResourcePolicy> {
        self.json.as_ref()
    }
    #[cfg(feature = "arrow-59")]
    pub fn parquet_policy(&self) -> Option<&crate::parquet::resource::ReaderResourcePolicy> {
        self.parquet.as_ref()
    }
    pub fn is_governed(&self) -> bool {
        #[cfg(feature = "arrow-59")]
        if self.json.is_some() || self.parquet.is_some() {
            return true;
        }
        self.scope.is_some() || self.inherited.is_some()
    }
    fn same_current(&self, other: &Self) -> bool {
        fn same<T>(left: Option<&T>, right: Option<&T>, eq: impl FnOnce(&T, &T) -> bool) -> bool {
            match (left, right) {
                (None, None) => true,
                (Some(left), Some(right)) => eq(left, right),
                _ => false,
            }
        }
        if !same(self.scope.as_ref(), other.scope.as_ref(), Arc::ptr_eq) {
            return false;
        }
        #[cfg(feature = "arrow-59")]
        if !same(self.json.as_ref(), other.json.as_ref(), |a, b| {
            a.same_owner(b)
        }) || !same(self.parquet.as_ref(), other.parquet.as_ref(), |a, b| {
            a.same_owner(b)
        }) {
            return false;
        }
        other.inherited.is_none()
            || same(
                self.inherited.as_ref(),
                other.inherited.as_ref(),
                Arc::ptr_eq,
            )
    }
    /// Keep both an input generation and the current allocation generation.
    pub fn try_with_current(self) -> DeltaResult<Self> {
        self.try_merge(Self::current())
    }
    /// Retain an additional producer's admission owners without dropping this one.
    pub fn try_merge(self, current: Self) -> DeltaResult<Self> {
        self.try_merge_with_depth_limit(current, Self::MAX_INHERITED_DEPTH)
    }
    /// Merge with an explicit stricter cap. Values outside 1..=64 are rejected.
    /// Depth is checked before reserving or allocating an inherited node, even
    /// when each new generation owns a fresh admission scope and receipt bank.
    pub fn try_merge_with_depth_limit(
        self,
        mut current: Self,
        max_depth: usize,
    ) -> DeltaResult<Self> {
        if max_depth == 0 || max_depth > Self::MAX_INHERITED_DEPTH {
            return Err(ResourceExhausted {
                kind: "native_owner_transfer_depth_limit",
                requested: max_depth,
                limit: Self::MAX_INHERITED_DEPTH,
            }
            .into());
        }
        let original_depth = self.inherited_depth.max(current.inherited_depth);
        if original_depth > max_depth {
            return Err(ResourceExhausted {
                kind: "native_owner_transfer_depth",
                requested: original_depth,
                limit: max_depth,
            }
            .into());
        }
        if !current.is_governed() || self.same_current(&current) {
            return Ok(self);
        }
        if !self.is_governed() {
            return Ok(current);
        }
        let inherited_depth = original_depth.checked_add(1).ok_or(ResourceExhausted {
            kind: "native_owner_transfer_depth",
            requested: usize::MAX,
            limit: max_depth,
        })?;
        if inherited_depth > max_depth {
            return Err(ResourceExhausted {
                kind: "native_owner_transfer_depth",
                requested: inherited_depth,
                limit: max_depth,
            }
            .into());
        }
        if current.scope.is_none() {
            current.scope = self.scope.clone();
        }
        let scope = current.scope.as_ref().ok_or(ResourceExhausted {
            kind: "native_owner_transfer_scope",
            requested: 1,
            limit: 0,
        })?;
        scope.reserve(AllocationRequest {
            kind: "native_owner_transfer_node",
            bytes: std::mem::size_of::<(Self, Self)>() + 2 * std::mem::size_of::<usize>(),
        })?;
        let previous = current.clone();
        current.inherited = Some(Arc::new((self, previous)));
        current.inherited_depth = inherited_depth;
        Ok(current)
    }
}

/// A native batch and the explicit owners of all of its admitted data and metadata.
/// Deliberately has no `Deref`, `Clone`, or conversion to a bare `RecordBatch`.
/// Borrowed metadata must not outlive this wrapper without its owners being retained.
#[derive(Debug)]
pub struct NativeOwnedRecordBatch {
    batch: RecordBatch,
    owners: NativeDataOwners,
}
impl NativeOwnedRecordBatch {
    pub(crate) fn from_parts(batch: RecordBatch, owners: NativeDataOwners) -> Self {
        Self { batch, owners }
    }
    pub fn record_batch(&self) -> &RecordBatch {
        &self.batch
    }
    pub fn owners(&self) -> &NativeDataOwners {
        &self.owners
    }
    pub fn num_rows(&self) -> usize {
        self.batch.num_rows()
    }
    pub fn num_columns(&self) -> usize {
        self.batch.num_columns()
    }
    pub fn schema_ref(&self) -> &crate::arrow::datatypes::SchemaRef {
        self.batch.schema_ref()
    }
    pub fn column(&self, index: usize) -> &crate::arrow::array::ArrayRef {
        self.batch.column(index)
    }
    pub fn column_by_name(&self, name: &str) -> Option<&crate::arrow::array::ArrayRef> {
        self.batch.column_by_name(name)
    }
    pub fn columns(&self) -> &[crate::arrow::array::ArrayRef] {
        self.batch.columns()
    }
    /// Clone only the batch descriptor after admitting its new column-vector backing.
    pub fn try_clone(&self) -> DeltaResult<Self> {
        let owners = self.owners.clone().try_with_current()?;
        if owners.is_governed() {
            let bytes = self
                .batch
                .num_columns()
                .checked_mul(std::mem::size_of::<crate::arrow::array::ArrayRef>())
                .ok_or(ResourceExhausted {
                    kind: "native_batch_descriptor",
                    requested: usize::MAX,
                    limit: isize::MAX as usize,
                })?;
            let scope = owners.native_scope().ok_or(ResourceExhausted {
                kind: "native_batch_descriptor_scope",
                requested: bytes,
                limit: 0,
            })?;
            scope.reserve(AllocationRequest {
                kind: "native_batch_descriptor",
                bytes,
            })?;
        }
        let mut columns = Vec::new();
        columns
            .try_reserve_exact(self.batch.num_columns())
            .map_err(|_| ResourceExhausted {
                kind: "native_batch_descriptor_allocator",
                requested: self.batch.num_columns(),
                limit: 0,
            })?;
        columns.extend(self.batch.columns().iter().cloned());
        let batch = RecordBatch::try_new_with_options(
            self.batch.schema_ref().clone(),
            columns,
            &crate::arrow::array::RecordBatchOptions::new()
                .with_row_count(Some(self.batch.num_rows())),
        )?;
        Ok(Self { batch, owners })
    }
    /// Transfer to a kernel engine-data owner without discarding native lifetimes.
    pub fn into_engine_data(self) -> super::ArrowEngineData {
        super::ArrowEngineData::from_native_owned(self)
    }
    /// Post-process a native batch into engine data while retaining its producers.
    pub fn try_map_engine(
        self,
        map: impl FnOnce(RecordBatch) -> DeltaResult<super::ArrowEngineData>,
    ) -> DeltaResult<super::ArrowEngineData> {
        let owners = self.owners.try_with_current()?;
        map(self.batch)?.try_inherit_owners(owners)
    }
    /// Map a producer-owned batch while retaining both input and current native
    /// allocation owners. The callback must admit its new allocations separately.
    pub fn try_map<E: From<crate::Error>>(
        self,
        map: impl FnOnce(RecordBatch) -> Result<RecordBatch, E>,
    ) -> Result<Self, E> {
        let owners = self.owners.try_with_current()?;
        Ok(Self {
            batch: map(self.batch)?,
            owners,
        })
    }
    /// Share one original batch descriptor and its owners, after admitting the Arc.
    pub fn try_into_shared(self) -> DeltaResult<Arc<Self>> {
        if self.owners.is_governed() {
            let scope = self.owners.native_scope().ok_or(ResourceExhausted {
                kind: "native_batch_shared_owner_scope",
                requested: 1,
                limit: 0,
            })?;
            scope.reserve(AllocationRequest {
                kind: "native_batch_shared_owner",
                bytes: std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>(),
            })?;
        }
        Ok(Arc::new(self))
    }
    /// Legacy bare conversion is available only for an ungoverned producer and caller.
    pub fn try_into_ungoverned_record_batch(self) -> DeltaResult<RecordBatch> {
        self.check_ungoverned()?;
        Ok(self.batch)
    }
    /// Legacy borrowed conversion, retaining the same ungoverned-only boundary.
    pub fn try_to_ungoverned_record_batch(&self) -> DeltaResult<RecordBatch> {
        self.check_ungoverned()?;
        Ok(self.batch.clone())
    }
    fn check_ungoverned(&self) -> DeltaResult<()> {
        if self.owners.is_governed() || NativeDataOwners::current().is_governed() {
            return Err(ResourceExhausted {
                kind: "native_batch_bare_conversion",
                requested: 1,
                limit: 0,
            }
            .into());
        }
        Ok(())
    }
    /// Borrow-transform while preserving this producer and the current producer.
    pub fn try_transform<E: From<crate::Error>>(
        &self,
        map: impl FnOnce(&RecordBatch) -> Result<RecordBatch, E>,
    ) -> Result<Self, E> {
        let owners = self.owners.clone().try_with_current()?;
        Ok(Self {
            batch: map(&self.batch)?,
            owners,
        })
    }
    /// Add a producer whose arrays or metadata were incorporated into this batch.
    pub fn try_retain(mut self, owners: NativeDataOwners) -> DeltaResult<Self> {
        self.owners = self.owners.try_merge(owners)?;
        Ok(self)
    }
    /// Convert to a struct array together with its inseparable producer owners.
    /// Consumers must retain the returned owner alongside the original array.
    pub fn into_struct_array(self) -> (crate::arrow::array::StructArray, NativeDataOwners) {
        (self.batch.into(), self.owners)
    }
    pub(crate) fn into_parts(self) -> (RecordBatch, NativeDataOwners) {
        (self.batch, self.owners)
    }
}

impl PartialEq for NativeOwnedRecordBatch {
    fn eq(&self, other: &Self) -> bool {
        self.batch == other.batch
    }
}

#[cfg(test)]
#[path = "native_owned_batch_tests.rs"]
mod tests;
