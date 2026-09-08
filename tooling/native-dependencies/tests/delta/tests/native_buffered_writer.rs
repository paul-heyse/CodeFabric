//! Original BufWriter integration; physical-store budget closure is tested separately.
extern crate delta_kernel as buoyant_kernel;
#[path = "../../support/native_runtime.rs"]
mod native_runtime;
use deltalake_core::DeltaResult;
#[path = "../../../../../third_party/native/delta-rs/crates/core/src/operations/write/native_writer.rs"] mod native_writer;
use std::{sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}}};
use delta_kernel::resource::*;
use native_writer::AdmittedBufWriter;
use object_store::{ObjectStoreExt, buffered::BufWriter, path::Path};
use parquet::arrow::async_writer::AsyncFileWriter;
use bytes::Bytes;
#[derive(Debug, Default)] struct Accounting { live: AtomicUsize, denied: Mutex<Option<&'static str>>, requests: Mutex<Vec<AllocationRequest>> }
#[derive(Debug)] struct Admission(Arc<Accounting>);
#[derive(Debug)] struct Receipt(usize, Arc<Accounting>);
impl AllocationReceipt for Receipt { fn bytes(&self) -> usize { self.0 } }
impl Drop for Receipt { fn drop(&mut self) { self.1.live.fetch_sub(self.0, Ordering::SeqCst); } }
impl AllocationAdmission for Admission {
 fn try_reserve(&self, request: AllocationRequest) -> Result<Arc<dyn AllocationReceipt>, ResourceExhausted> {
  self.0.requests.lock().unwrap().push(request);
  if *self.0.denied.lock().unwrap() == Some(request.kind) { return Err(ResourceExhausted { kind: request.kind, requested: request.bytes, limit: 0 }); }
  self.0.live.fetch_add(request.bytes, Ordering::SeqCst);
  Ok(Arc::new(Receipt(request.bytes, self.0.clone())))
 }
}
fn scope() -> (Arc<NativeResourceScope>, Arc<Accounting>) {
 let accounting = Arc::new(Accounting::default());
 (NativeResourceScope::try_new(Arc::new(Admission(accounting.clone())), 128).unwrap(), accounting)
}
fn policy(scope: Arc<NativeResourceScope>) -> NativeResourceThreadPolicy {
 NativeResourceThreadPolicy::try_new(scope, JsonResourceLimits { max_bytes: 4096, max_tokens: 256, max_depth: 16, max_string_bytes: 2048, max_container_items: 256 }).unwrap()
}
fn runtime(scope: Arc<NativeResourceScope>) -> tokio::runtime::Runtime {
 native_runtime::kernel_runtime(scope, JsonResourceLimits { max_bytes: 4096, max_tokens: 256, max_depth: 16, max_string_bytes: 2048, max_container_items: 256 })
}

#[test]
fn native_bufwriter_deny_and_unpolled_cancel_precede_storage_mutation() {
 let (scope, accounting) = scope(); let runtime = runtime(scope.clone()); let entered = runtime.enter();
 let policy = policy(scope.clone()); let guard = policy.enter_thread();
 let directory = tempfile::tempdir().unwrap();
 let store = Arc::new(object_store::local::LocalFileSystem::new_with_prefix(directory.path()).unwrap());
 let mut writer = AdmittedBufWriter::try_new(BufWriter::with_capacity(store, Path::from("data"), 4).with_max_concurrency(2), 4).unwrap();
 drop(writer.try_write(Bytes::from_static(b"cancelled-before-poll")).unwrap());
 assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
 *accounting.denied.lock().unwrap() = Some("native_buffered_writer_parts");
 let error = runtime.block_on(writer.try_write(Bytes::from_static(b"too large to buffer")).unwrap()).unwrap_err();
 assert!(matches!(error, parquet::errors::ParquetError::ResourceExhausted(_)));
 assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
 drop((writer, guard, policy)); drop(entered); drop(runtime); drop(scope);
 assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
}
#[test]
fn native_bufwriter_keeps_local_multipart_byte_order_and_cumulative_part_charges() {
 let (scope, accounting) = scope(); let runtime = runtime(scope.clone()); let entered = runtime.enter();
 let policy = policy(scope.clone()); let guard = policy.enter_thread();
 let directory = tempfile::tempdir().unwrap();
 let store = Arc::new(object_store::local::LocalFileSystem::new_with_prefix(directory.path()).unwrap());
 let mut writer = AdmittedBufWriter::try_new(BufWriter::with_capacity(store, Path::from("data"), 4).with_max_concurrency(2), 4).unwrap();
 runtime.block_on(async {
  writer.try_write(Bytes::from_static(b"abc")).unwrap().await.unwrap(); writer.try_write(Bytes::from_static(b"defghijklmnop")).unwrap().await.unwrap();
  writer.try_write(Bytes::from_static(b"qrst")).unwrap().await.unwrap(); writer.try_complete().unwrap().await.unwrap();
 });
 assert_eq!(std::fs::read(directory.path().join("data")).unwrap(), b"abcdefghijklmnopqrst");
 let requests = accounting.requests.lock().unwrap();
 assert_eq!(requests.iter().filter(|r| r.kind == "native_buffered_writer_parts").count(), 4);
 assert!(requests.iter().filter(|r| r.kind == "native_buffered_writer_parts").all(|r| r.bytes > 0)); drop(requests);
 drop((writer, guard, policy)); drop(entered); drop(runtime); drop(scope);
 assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
}
struct OriginalBytes { value: [u8; 8], _scope: Arc<NativeResourceScope> }
impl AsRef<[u8]> for OriginalBytes { fn as_ref(&self) -> &[u8] { &self.value } }
#[test]
fn native_bufwriter_single_put_preserves_original_bytes_backing_and_owner() {
 let (scope, accounting) = scope(); let runtime = runtime(scope.clone()); let entered = runtime.enter();
 let policy = policy(scope.clone()); let guard = policy.enter_thread();
 scope.reserve(AllocationRequest { kind: "probe_original_bytes", bytes: std::mem::size_of::<OriginalBytes>() }).unwrap();
 let bytes = Bytes::from_owner(OriginalBytes { value: *b"original", _scope: scope.clone() }); let pointer = bytes.as_ptr();
 let store = Arc::new(object_store::memory::InMemory::new()); let path = Path::from("data");
 let mut writer = AdmittedBufWriter::try_new(BufWriter::with_capacity(store.clone(), path.clone(), 64), 64).unwrap();
 runtime.block_on(async { writer.try_write(bytes).unwrap().await.unwrap(); writer.try_complete().unwrap().await.unwrap(); });
 let retained = runtime.block_on(async { store.get(&path).await.unwrap().bytes().await.unwrap() }); assert_eq!(retained.as_ptr(), pointer);
 runtime.block_on(store.delete(&path)).unwrap();
 drop((writer, store, guard, policy)); drop(entered); drop(runtime); drop(scope);
 assert!(accounting.live.load(Ordering::SeqCst) > 0); assert_eq!(&retained[..], b"original"); drop(retained);
 assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
}
#[test]
fn native_bufwriter_partial_multipart_drop_is_joined_before_receipt_release() {
 let (scope, accounting) = scope(); let runtime = runtime(scope.clone()); let entered = runtime.enter();
 let policy = policy(scope.clone()); let guard = policy.enter_thread();
 let directory = tempfile::tempdir().unwrap();
 let store = Arc::new(object_store::local::LocalFileSystem::new_with_prefix(directory.path()).unwrap());
 let mut writer = AdmittedBufWriter::try_new(BufWriter::with_capacity(store, Path::from("data"), 4).with_max_concurrency(2), 4).unwrap();
 runtime.block_on(writer.try_write(Bytes::from_static(b"partial-upload")).unwrap()).unwrap();
 drop(writer);
 assert!(accounting.live.load(Ordering::SeqCst) > 0);
 drop(guard); drop(policy); drop(entered); drop(runtime); drop(scope);
 assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
 assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
}
#[test]
fn native_bufwriter_original_error_retains_its_admission_after_writer_and_runtime_drop() {
 let (scope, accounting) = scope(); let runtime = runtime(scope.clone()); let entered = runtime.enter();
 let policy = policy(scope.clone()); let guard = policy.enter_thread();
 let directory = tempfile::tempdir().unwrap(); std::fs::create_dir(directory.path().join("data")).unwrap();
 let store = Arc::new(object_store::local::LocalFileSystem::new_with_prefix(directory.path()).unwrap());
 let mut writer = AdmittedBufWriter::try_new(BufWriter::with_capacity(store, Path::from("data"), 64), 64).unwrap();
 runtime.block_on(writer.try_write(Bytes::from_static(b"native-error")).unwrap()).unwrap();
 let error = runtime.block_on(writer.try_complete().unwrap()).unwrap_err();
 drop((writer, guard, policy)); drop(entered); drop(runtime); drop(scope);
 assert!(accounting.live.load(Ordering::SeqCst) > 0);
 assert!(!error.to_string().is_empty()); drop(error);
 assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
}
