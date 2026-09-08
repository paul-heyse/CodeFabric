// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

use bytes::Bytes;
use futures::future::BoxFuture;
use std::sync::Arc;
use std::alloc::Layout;
use std::fmt;
use crate::resource::{ReaderResourcePolicy, ResourceExhausted};

use crate::arrow::async_writer::AsyncFileWriter;
use crate::errors::{ParquetError, Result};
use object_store::ObjectStore;
use object_store::buffered::BufWriter;
use object_store::path::Path;
use tokio::io::AsyncWriteExt;

/// [`ParquetObjectWriter`] for writing to parquet to [`ObjectStore`]
///
/// This type is deprecated: [`BufWriter`] implements [`AsyncWrite`] and can
/// therefore be passed to [`AsyncArrowWriter`] directly via the blanket
/// [`AsyncFileWriter`] implementation for [`AsyncWrite`] types:
///
/// ```
/// # use arrow_array::{ArrayRef, Int64Array, RecordBatch};
/// # use object_store::buffered::BufWriter;
/// # use object_store::memory::InMemory;
/// # use object_store::path::Path;
/// # use object_store::{ObjectStore, ObjectStoreExt};
/// # use std::sync::Arc;
///
/// # use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
/// # use parquet::arrow::AsyncArrowWriter;
///
/// # #[tokio::main(flavor="current_thread")]
/// # async fn main() {
///     let store = Arc::new(InMemory::new());
///
///     let col = Arc::new(Int64Array::from_iter_values([1, 2, 3])) as ArrayRef;
///     let to_write = RecordBatch::try_from_iter([("col", col)]).unwrap();
///
///     let object_store_writer = BufWriter::new(store.clone(), Path::from("test"));
///     let mut writer =
///         AsyncArrowWriter::try_new(object_store_writer, to_write.schema(), None).unwrap();
///     writer.write(&to_write).await.unwrap();
///     writer.close().await.unwrap();
///
///     let buffer = store
///         .get(&Path::from("test"))
///         .await
///         .unwrap()
///         .bytes()
///         .await
///         .unwrap();
///     let mut reader = ParquetRecordBatchReaderBuilder::try_new(buffer)
///         .unwrap()
///         .build()
///         .unwrap();
///     let read = reader.next().unwrap().unwrap();
///
///     assert_eq!(to_write, read);
/// # }
/// ```
///
/// [`AsyncWrite`]: tokio::io::AsyncWrite
/// [`AsyncArrowWriter`]: crate::arrow::async_writer::AsyncArrowWriter
#[deprecated(
    since = "59.2.0",
    note = "Pass an `object_store::buffered::BufWriter` to `AsyncArrowWriter` directly instead; see https://github.com/apache/arrow-rs/issues/10308 and `parquet/examples/object_store.rs`."
)]
#[derive(Debug)]
pub struct ParquetObjectWriter {
    w: BufWriter,
    chunk_size: usize,
    calls: usize,
    finished: bool,
    runtime_id: Option<tokio::runtime::Id>,
    resource_policy: Option<ReaderResourcePolicy>,
}

#[allow(deprecated)]
impl ParquetObjectWriter {
    /// Create a new [`ParquetObjectWriter`] that writes to the specified path in the given store.
    ///
    /// To configure the writer behavior, please build [`BufWriter`] and then use [`Self::from_buf_writer`]
    pub fn new(store: Arc<dyn ObjectStore>, path: Path) -> Self {
        Self::try_new(store, path).expect("object writer admission")
    }

    /// Construct a new ParquetObjectWriter via a existing BufWriter.
    pub fn from_buf_writer(w: BufWriter) -> Self {
        Self { w, chunk_size: 0, calls: 0, finished: false, runtime_id: None, resource_policy: None }
    }

    /// Consume the writer and return the underlying BufWriter.
    pub fn into_inner(self) -> BufWriter {
        self.try_into_inner().expect("owned object writer cannot release its policy")
    }

    /// Construct the selected native object writer with the upstream 10 MiB capacity.
    pub fn try_new(store: Arc<dyn ObjectStore>, path: Path) -> Result<Self> {
        Self::try_new_with_capacity(store, path, 10 * 1024 * 1024)
    }

    /// Fallible governed constructor. Capacity is explicit because an already
    /// constructed BufWriter does not expose its configuration or buffered state.
    /// Native multipart/store work requires the closed Tokio admission profile;
    /// task admission and terminal joining are owned by that original runtime.
    pub fn try_new_with_capacity(store: Arc<dyn ObjectStore>, path: Path, chunk_size: usize) -> Result<Self> {
        if chunk_size == 0 { return Err(ResourceExhausted { kind: "object writer chunk size", requested: 1, limit: 0 }.into()); }
        let resource_policy = crate::resource::current();
        if let Some(policy) = &resource_policy {
            check_runtime()?;
            ReaderResourcePolicy::check(chunk_size, policy.limits().output_bytes, "object writer chunk size")?;
            policy.reserve(flush_layout()?, "object writer native flush future")?;
        }
        // BufWriter::with_capacity initializes empty Vecs; it performs no
        // backing allocation. Original path/store ownership is supplied by caller.
        let runtime_id = resource_policy.as_ref().map(|_| tokio::runtime::Handle::current().id());
        let mut value = Self { w: BufWriter::with_capacity(store, path, chunk_size), chunk_size, calls: 0, finished: false, runtime_id, resource_policy };
        if let Some(policy) = value.resource_policy.clone() {
            let write = { let future = value.write_inner(Bytes::new()); std::mem::size_of_val(&future) };
            let complete = { let future = value.complete_inner(); std::mem::size_of_val(&future) };
            // &mut self permits one returned future at a time. Its allocation is
            // admitted before any try_write/try_complete can return a BoxFuture.
            policy.reserve(write.max(complete), "object writer exclusive future")?;
        }
        Ok(value)
    }

    /// Extract ungoverned native state. Governed state has an original policy
    /// that must outlive every native buffer and may not escape as a raw BufWriter.
    pub fn try_into_inner(self) -> Result<BufWriter> {
        if self.resource_policy.is_some() || crate::resource::current().is_some() {
            return Err(ResourceExhausted { kind: "owned object writer extraction", requested: 1, limit: 0 }.into());
        }
        Ok(self.w)
    }

    fn validate(&self) -> Result<()> {
        crate::resource::validate_thread_owner(self.resource_policy.as_ref())?;
        if self.resource_policy.is_some() {
            check_runtime()?;
            if self.runtime_id != Some(tokio::runtime::Handle::current().id()) { return Err(ResourceExhausted { kind: "object writer foreign runtime", requested: 1, limit: 0 }.into()); }
        }
        if self.finished { return Err(ResourceExhausted { kind: "object writer already finished", requested: 1, limit: 0 }.into()); }
        Ok(())
    }

    fn admit(&mut self, bytes: usize, completing: bool) -> Result<()> {
        self.validate()?;
        let calls = checked_add(self.calls, 1)?;
        if let Some(policy) = &self.resource_policy {
            let parts = if completing { 1 } else { checked_add(bytes / self.chunk_size, 2)? };
            let fragments = checked_add(checked_add(parts, calls)?, 1)?;
            // BufWriter::put uses original Bytes/slices, never extend_from_slice.
            // One fragment per boundary and the previous <=calls buffered chunks
            // can be replayed into multipart once. Each native Vec<Bytes> doubles
            // from capacity4: all complete replacement allocations sum to <4n+16
            // slots; freezing the final Vec to Arc<[Bytes]> adds one slot per
            // fragment plus its two-word Arc header. Receipts retain all peaks.
            let slots = checked_add(checked_mul(fragments, 5)?, checked_mul(parts, 16)?)?;
            let arrays = checked_add(checked_mul(slots, std::mem::size_of::<Bytes>())?, checked_mul(checked_add(parts, 1)?, 2 * std::mem::size_of::<usize>())?)?;
            // bytes1.12.1 Shared owner: Vec-backed caller Bytes may allocate this
            // four-word original descriptor on first split. Payload itself must
            // already be owned by the caller; this reserves only new descriptors.
            let shared = checked_mul(fragments, 4 * std::mem::size_of::<usize>())?;
            policy.reserve(checked_add(arrays, shared)?, "object writer payload descriptors")?;
            // At most one terminal error is exposed by a call. Preserve its
            // original native payload while retaining the resource policy.
            policy.reserve(std::mem::size_of::<OwnedWriteError<object_store::Error>>().max(std::mem::size_of::<OwnedWriteError<std::io::Error>>()), "object writer error owner")?;
            // BufWriter converts object_store::Error to io::Error on shutdown.
            policy.reserve(std::mem::size_of::<object_store::Error>(), "object writer I/O error payload")?;
        }
        self.calls = calls;
        Ok(())
    }

    async fn write_inner(&mut self, bs: Bytes) -> Result<()> {
        self.admit(bs.len(), false)?;
        let owner = self.resource_policy.clone();
        self.w.put(bs).await.map_err(|error| ParquetError::External(Box::new(OwnedWriteError { error, _owner: owner })))
    }

    async fn complete_inner(&mut self) -> Result<()> {
        self.admit(0, true)?;
        let owner = self.resource_policy.clone();
        self.w.shutdown().await.map_err(|error| ParquetError::External(Box::new(OwnedWriteError { error, _owner: owner })))?;
        self.finished = true;
        Ok(())
    }
}

#[allow(deprecated)]
impl AsyncFileWriter for ParquetObjectWriter {
    fn write(&mut self, bs: Bytes) -> BoxFuture<'_, Result<()>> { Box::pin(self.write_inner(bs)) }
    fn complete(&mut self) -> BoxFuture<'_, Result<()>> { Box::pin(self.complete_inner()) }
    fn try_write(&mut self, bs: Bytes) -> Result<BoxFuture<'_, Result<()>>> {
        self.validate()?;
        Ok(Box::pin(self.write_inner(bs)))
    }
    fn try_complete(&mut self) -> Result<BoxFuture<'_, Result<()>>> {
        self.validate()?;
        Ok(Box::pin(self.complete_inner()))
    }
}
#[allow(deprecated)]
impl From<BufWriter> for ParquetObjectWriter {
    fn from(w: BufWriter) -> Self {
        Self::from_buf_writer(w)
    }
}

fn check_runtime() -> Result<()> {
    #[cfg(feature = "native-owned-local")]
    { tokio::runtime::resource::check_current_profile().map(|_| ()).map_err(|error| ResourceExhausted { kind: error.kind, requested: error.requested, limit: error.limit }.into()) }
    #[cfg(not(feature = "native-owned-local"))]
    { Err(ResourceExhausted { kind: "object writer native-owned-local feature required", requested: 1, limit: 0 }.into()) }
}
fn overflow() -> ParquetError { ResourceExhausted { kind: "object writer allocation layout", requested: usize::MAX, limit: isize::MAX as usize }.into() }
fn checked_add(a: usize, b: usize) -> Result<usize> { a.checked_add(b).filter(|v| *v <= isize::MAX as usize).ok_or_else(overflow) }
fn checked_mul(a: usize, b: usize) -> Result<usize> { a.checked_mul(b).filter(|v| *v <= isize::MAX as usize).ok_or_else(overflow) }
fn future_layout<A, F>(_: impl FnOnce(A) -> F) -> Layout { Layout::new::<F>() }
fn padded_fields(fields: &[Layout]) -> Result<usize> {
    let alignment = fields.iter().map(Layout::align).max().unwrap_or(1);
    fields.iter().try_fold(0, |sum, field| checked_add(sum, checked_add(field.size(), alignment - 1)? & !(alignment - 1)))
}
fn flush_layout() -> Result<usize> {
    use object_store::{PutPayload, PutPayloadMut, PutOptions, PutResult, WriteMultipart};
    // object_store0.13.2 buffered.rs437: two private native Flush futures.
    // Sum each branch's captured values, await state and result, padding EVERY
    // field to maximum alignment. This bounds repr(Rust) field reordering.
    let put = padded_fields(&[
        Layout::new::<Arc<dyn ObjectStore>>(), Layout::new::<Path>(),
        Layout::new::<PutPayloadMut>(), Layout::new::<PutPayload>(), Layout::new::<PutOptions>(),
        Layout::new::<BoxFuture<'static, object_store::Result<PutResult>>>(),
        Layout::new::<object_store::Result<PutResult>>(), Layout::new::<usize>(),
    ])?;
    // The nested finish future layout is derived from its native async fn TYPE,
    // without constructing or polling a future/upload/JoinSet to measure it.
    let multipart = padded_fields(&[
        Layout::new::<WriteMultipart>(), future_layout(WriteMultipart::finish),
        Layout::new::<object_store::Result<PutResult>>(), Layout::new::<usize>(),
    ])?;
    Ok(put.max(multipart))
}

#[derive(Debug)]
struct OwnedWriteError<E> { error: E, _owner: Option<ReaderResourcePolicy> }
impl<E: fmt::Display> fmt::Display for OwnedWriteError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.error.fmt(f) }
}
impl<E: std::error::Error + 'static> std::error::Error for OwnedWriteError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> { Some(&self.error) }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use arrow_array::{ArrayRef, Int64Array, RecordBatch};
    use object_store::memory::InMemory;
    use std::sync::Arc;

    use super::*;
    use crate::arrow::AsyncArrowWriter;
    use crate::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use object_store::ObjectStoreExt;

    #[tokio::test]
    async fn test_async_writer() {
        let store = Arc::new(InMemory::new());

        let col = Arc::new(Int64Array::from_iter_values([1, 2, 3])) as ArrayRef;
        let to_write = RecordBatch::try_from_iter([("col", col)]).unwrap();

        let object_store_writer = ParquetObjectWriter::new(store.clone(), Path::from("test"));
        let mut writer =
            AsyncArrowWriter::try_new(object_store_writer, to_write.schema(), None).unwrap();
        writer.write(&to_write).await.unwrap();
        writer.close().await.unwrap();

        let buffer = store
            .get(&Path::from("test"))
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        let mut reader = ParquetRecordBatchReaderBuilder::try_new(buffer)
            .unwrap()
            .build()
            .unwrap();
        let read = reader.next().unwrap().unwrap();

        assert_eq!(to_write, read);
    }
    #[cfg(feature = "native-owned-local")]
    mod resource_tests {
        use super::*;
    #[derive(Debug)]
    struct Receipt(usize);
    impl crate::resource::ResourceReceipt for Receipt { fn bytes(&self) -> usize { self.0 } }
    impl tokio::runtime::resource::RuntimeAllocationReceipt for Receipt { fn bytes(&self) -> usize { self.0 } }
    #[derive(Debug)]
    struct Admission { deny: &'static str }
    impl crate::resource::ResourceAdmission for Admission {
        fn try_reserve(&self, request: crate::resource::ResourceRequest) -> std::result::Result<Arc<dyn crate::resource::ResourceReceipt>, ResourceExhausted> {
            if self.deny == request.kind { return Err(ResourceExhausted { kind: request.kind, requested: request.bytes, limit: 0 }); }
            Ok(Arc::new(Receipt(request.bytes)))
        }
    }
    impl tokio::runtime::resource::RuntimeAllocationAdmission for Admission {
        fn try_reserve(&self, request: tokio::runtime::resource::RuntimeAllocationRequest) -> std::result::Result<Arc<dyn tokio::runtime::resource::RuntimeAllocationReceipt>, tokio::runtime::resource::ResourceLayoutError> { Ok(Arc::new(Receipt(request.bytes))) }
        fn record_failure(&self, _: tokio::runtime::resource::ResourceLayoutError) {}
    }
    fn policy(deny: &'static str) -> ReaderResourcePolicy {
        ReaderResourcePolicy::try_new(Arc::new(Admission { deny }), crate::resource::ReaderResourceLimits {
            allocations: 4096, collection_entries: 10000, string_bytes: 10000,
            footer_bytes: 100000, page_bytes: 100000, page_values: 10000,
            output_values: 10000, output_bytes: 1000000, schema_depth: 32, codec_bytes: 100000,
        }).unwrap()
    }
    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::resource::LocalRuntimeProfile {
            worker_threads: 1, blocking_threads: 1, blocking_queue: 16,
            thread_stack_bytes: 2 * 1024 * 1024, async_tasks: 128, blocking_tasks: 32,
        }.build(Arc::new(Admission { deny: "" }), || {}, || {}).unwrap()
    }

    #[test]
    fn resource_writer_object_requires_owned_runtime_before_native_construction() {
        let policy = policy("");
        let _scope = policy.enter_thread();
        let result = ParquetObjectWriter::try_new_with_capacity(Arc::new(InMemory::new()), Path::from("v"), 64);
        assert!(matches!(result, Err(ParquetError::ResourceExhausted(ResourceExhausted { kind: "tokio_local_profile_required", .. }))));
    }

    #[test]
    fn resource_writer_object_prebuilt_and_denied_payload_do_not_mutate_store() {
        let runtime = runtime();
        runtime.block_on(async {
            let store = Arc::new(InMemory::new());
            let path = Path::from("v");
            let mut prebuilt = ParquetObjectWriter::from_buf_writer(BufWriter::with_capacity(store.clone(), path.clone(), 64));
            let policy = policy("object writer payload descriptors");
            let _scope = policy.enter_thread();
            assert!(matches!(prebuilt.try_write(Bytes::from_static(b"x")), Err(ParquetError::ResourceExhausted(_))));
            let mut writer = ParquetObjectWriter::try_new_with_capacity(store.clone(), path.clone(), 64).unwrap();
            let error = writer.try_write(Bytes::from_static(b"x")).unwrap().await.unwrap_err();
            assert!(matches!(error, ParquetError::ResourceExhausted(ResourceExhausted { kind: "object writer payload descriptors", .. })));
            assert!(store.head(&path).await.is_err());
            assert!(matches!(writer.try_into_inner(), Err(ParquetError::ResourceExhausted(_))));
        });
    }

    #[test]
    fn resource_writer_object_native_buffered_and_multipart_complete() {
        let runtime = runtime();
        runtime.block_on(async {
            for chunk_size in [4, 64] {
                let store = Arc::new(InMemory::new());
                let path = Path::from("v");
                let policy = policy("");
                let _scope = policy.enter_thread();
                let mut writer = ParquetObjectWriter::try_new_with_capacity(store.clone(), path.clone(), chunk_size).unwrap();
                writer.try_write(Bytes::from_static(b"abc")).unwrap().await.unwrap();
                writer.try_write(Bytes::from_static(b"defghijk")).unwrap().await.unwrap();
                writer.try_complete().unwrap().await.unwrap();
                assert!(matches!(writer.try_complete(), Err(ParquetError::ResourceExhausted(_))));
                assert_eq!(store.get(&path).await.unwrap().bytes().await.unwrap(), Bytes::from_static(b"abcdefghijk"));
            }
        });
    }

    #[test]
    fn resource_writer_object_layout_derives_native_finish_future_without_constructing_it() {
        assert!(flush_layout().unwrap() >= future_layout(object_store::WriteMultipart::finish).size());
        assert!(checked_mul(usize::MAX, 2).is_err());
    }

    #[test]
    fn resource_writer_object_rejects_original_policy_in_foreign_runtime() {
        let first = runtime();
        let second = runtime();
        let policy = policy("");
        let _scope = policy.enter_thread();
        let mut writer = first.block_on(async { ParquetObjectWriter::try_new_with_capacity(Arc::new(InMemory::new()), Path::from("v"), 64).unwrap() });
        second.block_on(async {
            assert!(matches!(writer.try_write(Bytes::from_static(b"x")), Err(ParquetError::ResourceExhausted(ResourceExhausted { kind: "object writer foreign runtime", .. }))));
        });
    }

    }
}
