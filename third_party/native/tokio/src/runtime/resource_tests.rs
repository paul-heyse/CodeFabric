use super::*;
use std::alloc::{GlobalAlloc, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
static ROOT_POINTERS: [AtomicUsize; 3] = [const { AtomicUsize::new(0) }; 3];
static ROOT_FREED: [AtomicBool; 3] = [const { AtomicBool::new(false) }; 3];
static ROOT_CHARGED: [AtomicBool; 3] = [const { AtomicBool::new(false) }; 3];
static TEST_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
static FIXED_LIVE: AtomicBool = AtomicBool::new(false);
struct Allocator;
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        for index in 0..3 {
            if pointer as usize == ROOT_POINTERS[index].load(Ordering::SeqCst) {
                ROOT_FREED[index].store(true, Ordering::SeqCst);
                ROOT_CHARGED[index].store(FIXED_LIVE.load(Ordering::SeqCst), Ordering::SeqCst);
            }
        }
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
#[derive(Debug)]
struct Admission;
#[derive(Debug)]
struct Receipt {
    bytes: usize,
    fixed: bool,
}
impl RuntimeAllocationReceipt for Receipt {
    fn bytes(&self) -> usize {
        self.bytes
    }
}
impl Drop for Receipt {
    fn drop(&mut self) {
        if self.fixed {
            FIXED_LIVE.store(false, Ordering::SeqCst);
        }
    }
}
impl RuntimeAllocationAdmission for Admission {
    fn try_reserve(
        &self,
        request: RuntimeAllocationRequest,
    ) -> Result<Arc<dyn RuntimeAllocationReceipt>, ResourceLayoutError> {
        let fixed = request.kind == "tokio_local_runtime";
        if fixed {
            FIXED_LIVE.store(true, Ordering::SeqCst);
        }
        Ok(Arc::new(Receipt {
            bytes: request.bytes,
            fixed,
        }))
    }
    fn record_failure(&self, error: ResourceLayoutError) {
        panic!("unexpected admission failure: {error}");
    }
}
fn allocation_start<T>(pointer: *const T) -> usize {
    let (_, offset) = Layout::new::<[AtomicUsize; 2]>()
        .extend(Layout::new::<T>())
        .unwrap();
    pointer.cast::<u8>().wrapping_sub(offset) as usize
}
#[test]
fn original_root_arcs_keep_receipt_through_actual_deallocation() {
    let _serial = TEST_SERIAL.lock().unwrap();
    let profile = LocalRuntimeProfile {
        worker_threads: 1,
        blocking_threads: 1,
        blocking_queue: 4,
        async_tasks: 8,
        blocking_tasks: 16,
        thread_stack_bytes: 2 << 20,
    };
    let runtime = profile.build(Arc::new(Admission), || {}, || {}).unwrap();
    let handle = runtime.handle().clone();
    let scheduler = match &handle.inner {
        crate::runtime::scheduler::Handle::MultiThread(handle) => handle,
        _ => unreachable!(),
    };
    ROOT_POINTERS[0].store(allocation_start(scheduler.as_ptr()), Ordering::SeqCst);
    ROOT_POINTERS[1].store(
        scheduler.blocking_spawner.resource_allocation_probe().0 as usize,
        Ordering::SeqCst,
    );
    ROOT_POINTERS[2].store(
        allocation_start(Arc::as_ptr(scheduler.local_resource.as_ref().unwrap())),
        Ordering::SeqCst,
    );
    drop(runtime);
    assert!(FIXED_LIVE.load(Ordering::SeqCst));
    drop(handle);
    for index in 0..3 {
        assert!(
            ROOT_FREED[index].load(Ordering::SeqCst),
            "native allocation {index} was retained"
        );
        assert!(
            ROOT_CHARGED[index].load(Ordering::SeqCst),
            "native allocation {index} outlived its receipt"
        );
        ROOT_POINTERS[index].store(0, Ordering::SeqCst);
    }
    assert!(!FIXED_LIVE.load(Ordering::SeqCst));
}

#[test]
fn original_list_entry_waker_keeps_owner_through_last_native_deallocation() {
    let _serial = TEST_SERIAL.lock().unwrap();
    ROOT_FREED[0].store(false, Ordering::SeqCst);
    ROOT_CHARGED[0].store(false, Ordering::SeqCst);
    let profile = LocalRuntimeProfile {
        worker_threads: 1,
        blocking_threads: 1,
        blocking_queue: 4,
        async_tasks: 8,
        blocking_tasks: 16,
        thread_stack_bytes: 2 << 20,
    };
    let runtime = profile.build(Arc::new(Admission), || {}, || {}).unwrap();
    let waker = {
        let _entered = runtime.enter();
        let mut set = crate::util::IdleNotifiedSet::new();
        let mut entry = set.insert_idle(7usize);
        let waker = entry.with_value_and_context(|value, cx| {
            assert_eq!(*value, 7);
            cx.waker().clone()
        });
        let offset = crate::util::idle_notified_set::resource_entry_data_offset::<usize>();
        ROOT_POINTERS[0].store(
            waker.data().cast::<u8>().wrapping_sub(offset) as usize,
            Ordering::SeqCst,
        );
        drop(entry);
        drop(set);
        waker
    };
    drop(runtime);
    assert!(FIXED_LIVE.load(Ordering::SeqCst));
    assert!(!ROOT_FREED[0].load(Ordering::SeqCst));
    let clone = waker.clone();
    waker.wake();
    assert!(FIXED_LIVE.load(Ordering::SeqCst));
    drop(clone);
    assert!(ROOT_FREED[0].load(Ordering::SeqCst));
    assert!(ROOT_CHARGED[0].load(Ordering::SeqCst));
    ROOT_POINTERS[0].store(0, Ordering::SeqCst);
    assert!(!FIXED_LIVE.load(Ordering::SeqCst));
}
