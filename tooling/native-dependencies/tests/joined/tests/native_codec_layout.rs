//! Private allocation-order falsifier for the selected exact native codecs.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use parquet::basic::{Compression, ZstdLevel};
use parquet::compression::{create_codec, CodecOptions};
use parquet::resource::{ReaderResourcePolicy, ReaderResourceLimits, ResourceAdmission, ResourceReceipt, ResourceRequest, ResourceExhausted};
thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static ALLOCATED: Cell<usize> = const { Cell::new(0) };
    static ADMITTED: Cell<usize> = const { Cell::new(0) };
    static EARLY: Cell<bool> = const { Cell::new(false) };
}
struct Observer;
#[global_allocator] static ALLOCATOR: Observer = Observer;
fn record(size: usize) {
    let _ = TRACK.try_with(|track| if track.get() {
        let _ = ALLOCATED.try_with(|used| {
            let total = used.get().saturating_add(size); used.set(total);
            let _ = ADMITTED.try_with(|admitted| if total > admitted.get() { let _ = EARLY.try_with(|v| v.set(true)); });
        });
    });
}
// SAFETY: System receives the original layouts and pointers unchanged. All
// observation is nonallocating TLS, with no failure interception or injection.
unsafe impl GlobalAlloc for Observer {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 { record(layout.size()); unsafe { System.alloc(layout) } }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 { record(layout.size()); unsafe { System.alloc_zeroed(layout) } }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 { record(size); unsafe { System.realloc(pointer, layout, size) } }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) { unsafe { System.dealloc(pointer, layout) } }
}
struct Tracking;
impl Tracking {
    fn begin() -> Self { ALLOCATED.with(|v| v.set(0)); ADMITTED.with(|v| v.set(0)); EARLY.with(|v| v.set(false)); TRACK.with(|v| v.set(true)); Self }
    fn finish(self) -> (usize, usize, bool) { drop(self); (ALLOCATED.with(Cell::get), ADMITTED.with(Cell::get), EARLY.with(Cell::get)) }
}
impl Drop for Tracking { fn drop(&mut self) { TRACK.with(|v| v.set(false)); } }

#[derive(Debug)]
struct Receipt { bytes: AtomicUsize }
impl ResourceReceipt for Receipt { fn bytes(&self) -> usize { self.bytes.load(Ordering::Relaxed) } }
#[derive(Debug)]
struct Admission { deny: &'static str, next: AtomicUsize, receipts: Vec<Arc<Receipt>> }
impl ResourceAdmission for Admission {
    fn try_reserve(&self, request: ResourceRequest) -> Result<Arc<dyn ResourceReceipt>, ResourceExhausted> {
        if request.kind == self.deny { return Err(ResourceExhausted { kind: request.kind, requested: request.bytes, limit: 0 }); }
        let index = self.next.fetch_add(1, Ordering::Relaxed);
        let receipt = self.receipts.get(index).ok_or(ResourceExhausted { kind: "probe receipt bank", requested: index + 1, limit: self.receipts.len() })?;
        receipt.bytes.store(request.bytes, Ordering::Relaxed);
        ADMITTED.with(|v| v.set(v.get().checked_add(request.bytes).unwrap()));
        Ok(receipt.clone())
    }
}
fn policy(deny: &'static str) -> ReaderResourcePolicy {
    let admission = Arc::new(Admission { deny, next: AtomicUsize::new(0), receipts: (0..512).map(|_| Arc::new(Receipt { bytes: AtomicUsize::new(0) })).collect() });
    ReaderResourcePolicy::try_new(admission, ReaderResourceLimits {
        allocations: 512, collection_entries: 128, string_bytes: 65536, footer_bytes: 65536,
        page_bytes: 1 << 20, page_values: 1 << 20, output_values: 2 << 20,
        output_bytes: 2 << 20, schema_depth: 32, codec_bytes: 16 << 20,
    }).unwrap()
}
#[test]
fn selected_codecs_admit_every_rust_allocation_before_construct_encode_decode() {
    for kind in [Compression::SNAPPY, Compression::ZSTD(ZstdLevel::default())] {
        for size in [0, 1, 64, 4096, 65536, 131072] {
            let input: Vec<u8> = (0..size).map(|i| (i * 71) as u8).collect();
            let policy = policy(""); let guard = policy.enter_thread();
            let tracking = Tracking::begin();
            let result = (|| {
                let mut codec = create_codec(kind, &CodecOptions::default())?.unwrap();
                let mut encoded = Vec::new(); codec.compress(&input, &mut encoded)?;
                let mut output = Vec::new(); codec.decompress(&encoded, &mut output, Some(size))?;
                Ok::<_, parquet::errors::ParquetError>(output)
            })();
            let (used, admitted, early) = tracking.finish();
            assert!(result.is_ok(), "kind={kind:?} size={size} result={result:?}");
            assert!(!early && used <= admitted, "kind={kind:?} size={size} used={used} admitted={admitted} early={early}");
            assert_eq!(result.unwrap(), input); drop(guard);
        }
    }
}
#[test]
fn denied_native_codec_owner_allocates_nothing() {
    for (kind, deny) in [(Compression::SNAPPY,"snappy codec owner"), (Compression::ZSTD(ZstdLevel::default()),"zstd codec owner")] {
        let policy = policy(deny); let _guard = policy.enter_thread();
        let tracking = Tracking::begin(); let result = create_codec(kind, &CodecOptions::default());
        let (used, _, _) = tracking.finish();
        assert!(matches!(result, Err(parquet::errors::ParquetError::ResourceExhausted(e)) if e.kind == deny)); assert_eq!(used,0);
    }
}
