use std::sync::{Arc, Mutex, atomic::{AtomicUsize, AtomicBool, Ordering}};
use buoyant_kernel::{OwnedFileMeta, OwnedLogPaths, resource::*};
use url::Url;

static WATCHED_ALLOCATION: AtomicUsize = AtomicUsize::new(0);
static WATCHED_FREED: AtomicBool = AtomicBool::new(false);
struct AllocationObserver;
#[global_allocator]
static ALLOCATOR: AllocationObserver = AllocationObserver;
// SAFETY: delegates every allocator operation unchanged to System. Observation
// uses only atomics and never allocates or dereferences the watched pointer.
unsafe impl std::alloc::GlobalAlloc for AllocationObserver {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        unsafe { std::alloc::GlobalAlloc::alloc(&std::alloc::System, layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: std::alloc::Layout) {
        unsafe { std::alloc::GlobalAlloc::dealloc(&std::alloc::System, pointer, layout) }
        if pointer as usize == WATCHED_ALLOCATION.load(Ordering::SeqCst) { WATCHED_FREED.store(true, Ordering::SeqCst); }
    }
}

#[derive(Debug, Default)]
struct Ledger { check_drop: AtomicBool, live: AtomicUsize, deny: Mutex<Option<&'static str>>, requests: Mutex<Vec<AllocationRequest>> }
#[derive(Debug)]
struct Policy(Arc<Ledger>);
#[derive(Debug)]
struct Receipt { bytes: usize, ledger: Arc<Ledger> }
impl AllocationReceipt for Receipt { fn bytes(&self) -> usize { self.bytes } }
impl Drop for Receipt { fn drop(&mut self) {
    if self.ledger.check_drop.load(Ordering::SeqCst) { assert!(WATCHED_FREED.load(Ordering::SeqCst), "original native payload must be deallocated before its receipt is released"); }
    self.ledger.live.fetch_sub(self.bytes, Ordering::SeqCst);
} }
impl AllocationAdmission for Policy {
    fn try_reserve(&self, request: AllocationRequest) -> Result<Arc<dyn AllocationReceipt>, ResourceExhausted> {
        self.0.requests.lock().unwrap().push(request);
        if *self.0.deny.lock().unwrap() == Some(request.kind) { return Err(ResourceExhausted { kind: request.kind, requested: request.bytes, limit: 0 }); }
        self.0.live.fetch_add(request.bytes, Ordering::SeqCst);
        Ok(Arc::new(Receipt { bytes: request.bytes, ledger: self.0.clone() }))
    }
}
fn scope() -> (Arc<NativeResourceScope>, Arc<Ledger>, NativeResourceThreadPolicy) {
    let ledger = Arc::new(Ledger::default());
    let scope = NativeResourceScope::try_new(Arc::new(Policy(ledger.clone())), 1024).unwrap();
    let limits = JsonResourceLimits { max_bytes: 1<<20, max_tokens: 1<<16, max_depth: 64, max_string_bytes: 1<<20, max_container_items: 1<<16 };
    let policy = NativeResourceThreadPolicy::try_new(scope.clone(), limits).unwrap();
    (scope, ledger, policy)
}
fn file(base: &Url, version: usize) -> OwnedFileMeta {
    OwnedFileMeta::try_from_object_path(base, &format!("table/_delta_log/{version:020}.json"), 0, 1).unwrap()
}
#[test]
fn listing_url_admission_precedes_original_native_url_construction() {
    let (scope, ledger, policy) = scope();
    let thread = policy.enter_thread();
    let base = Url::parse("file:///table/_delta_log/00000000000000000000").unwrap();
    *ledger.deny.lock().unwrap() = Some("native_listing_url");
    let error = OwnedFileMeta::try_from_object_path(&base, "table/_delta_log/00000000000000000001.json", 0, 1).unwrap_err();
    assert!(error.is_resource_exhausted());
    assert_eq!(ledger.requests.lock().unwrap().last().unwrap().kind, "native_listing_url");
    drop(thread); drop(policy); drop(scope);
    assert_eq!(ledger.live.load(Ordering::SeqCst), 0);
}
#[test]
fn listing_original_url_and_path_identity_survive_collection_and_outer_drop() {
    let (scope, ledger, policy) = scope();
    let thread = policy.enter_thread();
    let base = Url::parse("file:///table/_delta_log/00000000000000000000").unwrap();
    let meta = file(&base, 0);
    let original = meta.location.as_str().as_ptr();
    let path = meta.into_parsed().unwrap().unwrap();
    assert_eq!(path.location.location.as_str().as_ptr(), original);
    let shared = path.clone();
    assert_eq!(path.filename.as_ptr(), shared.filename.as_ptr());
    let paths = OwnedLogPaths::try_from_iter([Ok(path)]).unwrap();
    let shared_paths = paths.clone();
    assert_eq!(paths.as_ptr(), shared_paths.as_ptr());
    let mut iter = paths.into_iter();
    let yielded = iter.next().unwrap();
    drop(shared_paths); drop(shared); drop(yielded);
    drop(thread); drop(policy); drop(scope);
    assert!(ledger.live.load(Ordering::SeqCst) > 0, "even exhausted owned iterator holds its original Vec until Drop");
    drop(iter);
    assert_eq!(ledger.live.load(Ordering::SeqCst), 0);
}
#[test]
fn denied_descriptor_copy_preserves_original_shared_collection() {
    let (scope, ledger, policy) = scope();
    let thread = policy.enter_thread();
    let base = Url::parse("file:///table/_delta_log/00000000000000000000").unwrap();
    let first = file(&base, 0).into_parsed().unwrap().unwrap();
    let second = file(&base, 1).into_parsed().unwrap().unwrap();
    let mut paths = OwnedLogPaths::try_from_iter([Ok(first)]).unwrap();
    let shared = paths.clone();
    let original = paths.as_ptr();
    *ledger.deny.lock().unwrap() = Some("native_log_path_descriptors");
    assert!(paths.try_push(second).unwrap_err().is_resource_exhausted());
    assert_eq!(paths.len(), 1);
    assert_eq!(paths.as_ptr(), original);
    assert_eq!(shared.as_ptr(), original);
    drop(paths); drop(shared); drop(thread); drop(policy); drop(scope);
    assert_eq!(ledger.live.load(Ordering::SeqCst), 0);
}
#[test]
fn governed_parser_rejects_legacy_unadmitted_file_meta() {
    let (_scope, _ledger, policy) = scope();
    let _thread = policy.enter_thread();
    let meta = buoyant_kernel::FileMeta::new(Url::parse("file:///table/_delta_log/00000000000000000000.json").unwrap(), 0, 1);
    assert!(OwnedFileMeta::unadmitted(meta).into_parsed().unwrap_err().is_resource_exhausted());
}
#[test]
fn native_listing_retains_captured_scope_across_background_runtime() {
    use buoyant_kernel::Engine;
    let (scope, ledger, policy) = scope();
    let thread = policy.enter_thread();
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("_delta_log");
    std::fs::create_dir(&log).unwrap();
    for version in [2, 0, 1] { std::fs::write(log.join(format!("{version:020}.json")), "{}").unwrap(); }
    let engine = buoyant_kernel_engine::DefaultEngineBuilder::new(Arc::new(object_store::local::LocalFileSystem::new())).build().unwrap();
    let root = Url::from_directory_path(&log).unwrap();
    let mut iter = engine.storage_handler().list_from(&root.join("00000000000000000000").unwrap()).unwrap();
    let first = iter.next().unwrap().unwrap();
    assert!(Arc::ptr_eq(first.resource_scope().unwrap(), &scope));
    assert!(first.location.path().ends_with("00000000000000000000.json"));
    let last = iter.last().unwrap().unwrap();
    assert!(last.location.path().ends_with("00000000000000000002.json"));
    drop(engine); drop(thread); drop(policy); drop(scope);
    assert!(ledger.live.load(Ordering::SeqCst) > 0);
    drop(first); drop(last);
    assert_eq!(ledger.live.load(Ordering::SeqCst), 0);
}

#[test]
fn original_log_root_backing_survives_last_segment_or_view_drop() {
    let (scope, ledger, policy) = scope();
    let thread = policy.enter_thread();
    let root = buoyant_kernel::OwnedLogUrl::try_copy(&Url::parse("file:///table/_delta_log/").unwrap()).unwrap();
    let shared = root.clone();
    assert_eq!(root.as_str().as_ptr(), shared.as_str().as_ptr());
    drop(root); drop(thread); drop(policy); drop(scope);
    assert!(ledger.live.load(Ordering::SeqCst) > 0);
    drop(shared);
    assert_eq!(ledger.live.load(Ordering::SeqCst), 0);
}

#[test]
fn original_native_payload_deallocation_precedes_receipt_release() {
    fn observe<T>(make: impl FnOnce(&Url) -> (T, usize)) {
        let (scope, ledger, policy) = scope();
        let thread = policy.enter_thread();
        let base = Url::parse("file:///table/_delta_log/00000000000000000000").unwrap();
        let (value, pointer) = make(&base);
        WATCHED_ALLOCATION.store(pointer, Ordering::SeqCst);
        WATCHED_FREED.store(false, Ordering::SeqCst);
        ledger.check_drop.store(true, Ordering::SeqCst);
        drop(thread); drop(policy); drop(scope);
        drop(value);
        assert!(WATCHED_FREED.load(Ordering::SeqCst));
        assert_eq!(ledger.live.load(Ordering::SeqCst), 0);
        WATCHED_ALLOCATION.store(0, Ordering::SeqCst);
    }
    observe(|base| { let value = file(base, 0); let pointer = value.location.as_str().as_ptr() as usize; (value, pointer) });
    observe(|base| { let value = file(base, 0).into_parsed().unwrap().unwrap(); let pointer = value.filename.as_ptr() as usize; (value, pointer) });
    observe(|base| { let value = OwnedLogPaths::try_from_iter([file(base, 0).into_parsed().map(Option::unwrap)]).unwrap(); let pointer = value.as_ptr() as usize; (value, pointer) });
    observe(|base| { let value = buoyant_kernel::OwnedLogUrl::try_copy(base).unwrap(); let pointer = value.as_str().as_ptr() as usize; (value, pointer) });
}

#[test]
fn read_selection_keeps_original_copied_url_through_exhausted_iterator() {
    let (scope, ledger, policy) = scope();
    let thread = policy.enter_thread();
    let input = buoyant_kernel::FileMeta::new(Url::parse("file:///table/part.parquet").unwrap(), 0, 1);
    let files = buoyant_kernel::OwnedFileMetas::try_copy(std::slice::from_ref(&input)).unwrap();
    let pointer = files[0].location.as_str().as_ptr();
    assert_ne!(pointer, input.location.as_str().as_ptr());
    let mut iterator = files.into_iter();
    let file = iterator.next().unwrap();
    assert_eq!(pointer, file.location.as_str().as_ptr());
    assert!(iterator.next().is_none());
    drop(input); drop(file); drop(thread); drop(policy); drop(scope);
    assert!(ledger.live.load(Ordering::SeqCst) > 0);
    drop(iterator);
    assert_eq!(ledger.live.load(Ordering::SeqCst), 0);
}

#[test]
fn read_selection_denial_precedes_url_copy() {
    let (_scope, ledger, policy) = scope();
    let _thread = policy.enter_thread();
    let input = buoyant_kernel::FileMeta::new(Url::parse("file:///table/part.parquet").unwrap(), 0, 1);
    *ledger.deny.lock().unwrap() = Some("native_read_file_descriptors");
    assert!(buoyant_kernel::OwnedFileMetas::try_copy(&[input]).unwrap_err().is_resource_exhausted());
}

#[test]
fn buffered_total_node_denial_precedes_native_file_open() {
    use buoyant_kernel::Engine;
    use buoyant_kernel::schema::{StructType, StructField, DataType};
    // Build caller-provided schema/inputs before selecting the native scope.
    let schema = Arc::new(StructType::try_new([StructField::nullable("x", DataType::LONG)]).unwrap());
    let input = buoyant_kernel::FileMeta::new(Url::parse("file:///missing/part.json").unwrap(), 0, 1);
    let (scope, ledger, policy) = scope();
    let thread = policy.enter_thread();
    *ledger.deny.lock().unwrap() = Some("native_buffered_total_nodes");
    let engine = buoyant_kernel_engine::DefaultEngineBuilder::new(Arc::new(object_store::memory::InMemory::new())).build().unwrap();
    let error = match engine.json_handler().read_json_files(&[input], schema, None) {
        Ok(_) => panic!("descriptor denial must precede opening the missing file"),
        Err(error) => error,
    };
    assert!(error.is_resource_exhausted());
    assert!(ledger.requests.lock().unwrap().iter().any(|request| request.kind == "native_buffered_total_nodes"));
    drop(error); drop(engine); drop(thread); drop(policy); drop(scope);
    assert_eq!(ledger.live.load(Ordering::SeqCst), 0);
}

#[test]
fn local_url_policy_preserves_native_relative_absolute_unicode_and_stripping() {
    use buoyant_kernel::path::url_resource::*;
    let (scope, ledger, policy) = scope();
    let thread = policy.enter_thread();
    let root = Url::parse("file:///authorized/").unwrap();
    let base = root.join("_delta_log/_sidecars/").unwrap();
    let admission = LocalUrlJoinAdmission::try_new(scope.clone(), std::slice::from_ref(&root), 4096).unwrap();
    let urls = NativeUrlThreadPolicy::new(scope.clone(), admission);
    let guard = urls.enter_thread().unwrap();
    for reference in ["part-雪.parquet", "./part%20one.parquet", "file:///authorized/a.parquet", " \tFi\nLe://LOCALHOST/authorized/雪.parquet\r ", "file://%6cocalhost/authorized/a.parquet", "\\\\localhost\\authorized\\b.parquet"] {
        let expected = base.join(reference).unwrap();
        let owned = OwnedFileMeta::try_from_reference(&base, reference, 3, 7).unwrap();
        assert_eq!(owned.location, expected);
        assert!(Arc::ptr_eq(owned.resource_scope().unwrap(), &scope));
    }
    drop(guard); drop(urls); drop(thread); drop(policy); drop(scope);
    assert_eq!(ledger.live.load(Ordering::SeqCst), 0);
}

#[test]
fn local_url_policy_denies_foreign_authorities_before_native_join_admission() {
    use buoyant_kernel::path::url_resource::*;
    let (scope, ledger, policy) = scope();
    let thread = policy.enter_thread();
    let root = Url::parse("file:///authorized/").unwrap();
    let admission = LocalUrlJoinAdmission::try_new(scope.clone(), std::slice::from_ref(&root), 4096).unwrap();
    let urls = NativeUrlThreadPolicy::new(scope.clone(), admission);
    let guard = urls.enter_thread().unwrap();
    for reference in ["https://example.invalid/a", "ht\ttp://example.invalid/a", "//例.example/a", "\\\\host\\a", "file://localhost.evil/a", "file://user@localhost/a", "file://%2flocalhost/a"] {
        let count = ledger.requests.lock().unwrap().iter().filter(|r| r.kind == "native_local_url_join").count();
        assert!(OwnedFileMeta::try_from_reference(&root, reference, 0, 1).is_err(), "{reference}");
        assert_eq!(ledger.requests.lock().unwrap().iter().filter(|r| r.kind == "native_local_url_join").count(), count);
    }
    drop(guard); drop(urls); drop(thread); drop(policy); drop(scope);
    assert_eq!(ledger.live.load(Ordering::SeqCst), 0);
}

#[test]
fn local_url_policy_validates_native_normalized_root_before_file_escape() {
    use buoyant_kernel::path::url_resource::*;
    let (scope, ledger, policy) = scope();
    let thread = policy.enter_thread();
    let root = Url::parse("file:///authorized/").unwrap();
    let admission = LocalUrlJoinAdmission::try_new(scope.clone(), std::slice::from_ref(&root), 4096).unwrap();
    let urls = NativeUrlThreadPolicy::new(scope.clone(), admission);
    let guard = urls.enter_thread().unwrap();
    for reference in ["../outside", "%2e%2e/outside", "file:///elsewhere/a", "/authorized-lookalike/a", "a?query", "a#fragment"] {
        assert!(OwnedFileMeta::try_from_reference(&root, reference, 0, 1).is_err(), "{reference}");
    }
    drop(guard); drop(urls); drop(thread); drop(policy); drop(scope);
    assert_eq!(ledger.live.load(Ordering::SeqCst), 0);
}

#[test]
fn local_url_join_denial_precedes_original_native_url_allocation() {
    use buoyant_kernel::path::url_resource::*;
    let (scope, ledger, policy) = scope();
    let _thread = policy.enter_thread();
    let root = Url::parse("file:///authorized/").unwrap();
    let admission = LocalUrlJoinAdmission::try_new(scope.clone(), std::slice::from_ref(&root), 4096).unwrap();
    let urls = NativeUrlThreadPolicy::new(scope, admission);
    let _guard = urls.enter_thread().unwrap();
    *ledger.deny.lock().unwrap() = Some("native_local_url_join");
    assert!(OwnedFileMeta::try_from_reference(&root, "雪.parquet", 0, 1).unwrap_err().is_resource_exhausted());
}
