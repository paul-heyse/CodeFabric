//! Native executor descriptor/queue admission, separate from runtime construction policy.
#[path = "../../support/native_runtime.rs"]
mod native_runtime;
use std::{sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}}};
use buoyant_kernel::resource::*;
use buoyant_kernel_engine::{DefaultEngineBuilder, executor::{TaskExecutor, tokio::{TokioBackgroundExecutor, TokioMultiThreadExecutor}}};

#[derive(Debug, Default)]
struct Accounting { live: AtomicUsize, denied: Mutex<Option<&'static str>>, requests: Mutex<Vec<AllocationRequest>> }
#[derive(Debug)] struct Admission(Arc<Accounting>);
#[derive(Debug)] struct Receipt(usize, Arc<Accounting>);
impl AllocationReceipt for Receipt { fn bytes(&self) -> usize { self.0 } }
impl Drop for Receipt { fn drop(&mut self) { self.1.live.fetch_sub(self.0, Ordering::SeqCst); } }
impl AllocationAdmission for Admission {
    fn try_reserve(&self, request: AllocationRequest) -> Result<Arc<dyn AllocationReceipt>, ResourceExhausted> {
        self.0.requests.lock().unwrap().push(request);
        if *self.0.denied.lock().unwrap() == Some(request.kind) {
            return Err(ResourceExhausted { kind: request.kind, requested: request.bytes, limit: 0 });
        }
        self.0.live.fetch_add(request.bytes, Ordering::SeqCst);
        Ok(Arc::new(Receipt(request.bytes, self.0.clone())))
    }
}
fn scope(slots: usize) -> (Arc<NativeResourceScope>, Arc<Accounting>) {
    let accounting = Arc::new(Accounting::default());
    (NativeResourceScope::try_new(Arc::new(Admission(accounting.clone())), slots).unwrap(), accounting)
}
fn policy(scope: Arc<NativeResourceScope>) -> NativeResourceThreadPolicy {
    NativeResourceThreadPolicy::try_new(scope, JsonResourceLimits { max_bytes: 4096, max_tokens: 256, max_depth: 16, max_string_bytes: 2048, max_container_items: 256 }).unwrap()
}
fn runtime(scope: Arc<NativeResourceScope>) -> tokio::runtime::Runtime {
    native_runtime::kernel_runtime(scope, JsonResourceLimits { max_bytes: 4096, max_tokens: 256, max_depth: 16, max_string_bytes: 2048, max_container_items: 256 })
}

#[test]
fn executor_denies_every_native_enqueue_phase_before_polling_input() {
    for kind in ["native_executor_channel", "tokio_async_task", "native_executor_relay"] {
        let (scope, accounting) = scope(64);
        let runtime = runtime(scope.clone());
        let entered = runtime.enter();
        let policy = policy(scope.clone());
        let guard = policy.enter_thread();
        let executor = TokioMultiThreadExecutor::try_new_current(runtime.handle().clone()).unwrap();
        *accounting.denied.lock().unwrap() = Some(kind);
        let polls = Arc::new(AtomicUsize::new(0));
        let observed = polls.clone();
        let error = executor.try_block_on(async move { observed.fetch_add(1, Ordering::SeqCst); 7 }).unwrap_err();
        assert!(error.is_resource_exhausted());
        assert_eq!(scope.failure().unwrap().kind, kind);
        assert_eq!(polls.load(Ordering::SeqCst), 0);
        drop((executor, guard, policy)); drop(entered); drop(runtime); drop(scope);
        assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn executor_cumulative_calls_exhaust_finite_slots_including_completed_tasks() {
    let (scope, accounting) = scope(40);
    let runtime = runtime(scope.clone());
    let entered = runtime.enter();
    let policy = policy(scope.clone()); let guard = policy.enter_thread();
    let executor = TokioMultiThreadExecutor::try_new_current(runtime.handle().clone()).unwrap();
    let mut accepted = 0;
    while executor.try_block_on(async { 23 }).is_ok() { accepted += 1; }
    assert!(accepted > 0 && accepted < 40);
    assert!(scope.failure().is_some());
    assert!(accounting.requests.lock().unwrap().iter().filter(|request| request.kind == "native_executor_channel").count() > 1);
    drop((executor, guard, policy)); drop(entered); drop(runtime); drop(scope);
    assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
}

#[test]
fn executor_rejects_foreign_runtime_and_uninstalled_worker_before_work() {
    let (scope, _) = scope(64);
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(1).max_blocking_threads(1).build().unwrap();
    let policy = policy(scope.clone()); let guard = policy.enter_thread();
    let executor = TokioMultiThreadExecutor::new(runtime.handle().clone());
    assert!(executor.try_block_on(async { 0 }).unwrap_err().is_resource_exhausted());
    drop((guard, policy, executor, runtime));
    let (scope, _) = self::scope(64);
    let runtime = native_runtime::runtime(scope.clone(), || {}, || {});
    let entered = runtime.enter(); let policy = self::policy(scope.clone()); let guard = policy.enter_thread();
    let executor = TokioMultiThreadExecutor::try_new_current(runtime.handle().clone()).unwrap();
    let polls = Arc::new(AtomicUsize::new(0)); let observed = polls.clone();
    assert!(executor.try_block_on(async move { observed.fetch_add(1, Ordering::SeqCst); }).unwrap_err().is_resource_exhausted());
    assert_eq!(polls.load(Ordering::SeqCst), 0);
    assert_eq!(scope.failure().unwrap().kind, "native_executor_worker_scope");
    drop((guard, policy, executor)); drop(entered); drop(runtime);
}

#[test]
fn engine_required_background_constructor_is_typed_and_allocates_no_native_runtime() {
    let (scope, accounting) = scope(32);
    let policy = policy(scope.clone()); let guard = policy.enter_thread();
    assert!(TokioBackgroundExecutor::try_new().unwrap_err().is_resource_exhausted());
    assert!(DefaultEngineBuilder::new(Arc::new(object_store::memory::InMemory::new())).try_build().unwrap_err().is_resource_exhausted());
    assert!(!accounting.requests.lock().unwrap().iter().any(|request| request.kind == "native_executor_channel"));
    drop((guard, policy, scope));
    assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
}

#[test]
fn owned_join_set_denies_each_roll_before_spawn_and_keeps_completed_receipts() {
    use buoyant_kernel_engine::resource::OwnedJoinSet;
    for denied in ["native_join_set_entry", "tokio_async_task"] {
        let (scope, accounting) = scope(64);
        let runtime = runtime(scope.clone()); let entered = runtime.enter();
        let policy = policy(scope.clone()); let guard = policy.enter_thread();
        let mut tasks = OwnedJoinSet::<Result<u64, buoyant_kernel::Error>>::try_new().unwrap();
        *accounting.denied.lock().unwrap() = Some(denied);
        let polls = Arc::new(AtomicUsize::new(0)); let observed = polls.clone();
        assert!(tasks.spawn(async move { observed.fetch_add(1, Ordering::SeqCst); Ok(9) }).unwrap_err().is_resource_exhausted());
        assert_eq!(tasks.len(), 0);
        assert_eq!(polls.load(Ordering::SeqCst), 0);
        drop((tasks, guard, policy)); drop(entered); drop(runtime); drop(scope);
        assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
    }
    let (scope, accounting) = scope(64);
    let runtime = runtime(scope.clone()); let entered = runtime.enter();
    let policy = policy(scope.clone()); let guard = policy.enter_thread();
    let mut tasks = OwnedJoinSet::<Result<u64, buoyant_kernel::Error>>::try_new().unwrap();
    for value in 0..8 { tasks.spawn(async move { Ok(value) }).unwrap(); }
    let admitted = accounting.live.load(Ordering::SeqCst);
    runtime.block_on(async { while let Some(value) = tasks.join_next().await { value.unwrap().unwrap(); } });
    assert_eq!(accounting.live.load(Ordering::SeqCst), admitted, "completed tasks consume cumulative scope admission until the operation joins");
    drop((tasks, guard, policy)); drop(entered); drop(runtime); drop(scope);
    assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
}

struct Payload { dropped: Arc<AtomicUsize>, accounting: Arc<Accounting> }
impl Drop for Payload {
    fn drop(&mut self) {
        assert!(self.accounting.live.load(Ordering::SeqCst) > 0, "native block payload drops before its receipt owner");
        self.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn owned_channel_denial_prevents_enqueue_and_original_block_owners_outlive_runtime() {
    use buoyant_kernel_engine::resource::try_channel;
    let (scope, accounting) = scope(64);
    let runtime = runtime(scope.clone()); let entered = runtime.enter();
    let policy = policy(scope.clone()); let guard = policy.enter_thread();
    let (sender, receiver) = try_channel::<Payload>(1).unwrap();
    let dropped = Arc::new(AtomicUsize::new(0));
    *accounting.denied.lock().unwrap() = Some("native_channel_send_blocks");
    assert!(runtime.block_on(sender.send(Payload { dropped: dropped.clone(), accounting: accounting.clone() }, "closed")).unwrap_err().is_resource_exhausted());
    assert_eq!(dropped.load(Ordering::SeqCst), 1, "denied payload was never placed in the native block");
    drop((sender, receiver, guard, policy)); drop(entered); drop(runtime); drop(scope);
    assert_eq!(accounting.live.load(Ordering::SeqCst), 0);

    let (scope, accounting) = self::scope(64);
    let runtime = self::runtime(scope.clone()); let entered = runtime.enter();
    let policy = self::policy(scope.clone()); let guard = policy.enter_thread();
    let (sender, receiver) = try_channel::<Payload>(1).unwrap();
    let dropped = Arc::new(AtomicUsize::new(0));
    runtime.block_on(sender.send(Payload { dropped: dropped.clone(), accounting: accounting.clone() }, "closed")).unwrap();
    drop(sender); drop(guard); drop(policy); drop(entered); drop(runtime); drop(scope);
    assert_eq!(dropped.load(Ordering::SeqCst), 0);
    assert!(accounting.live.load(Ordering::SeqCst) > 0);
    drop(receiver);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
}

#[test]
fn owned_channel_recycled_blocks_still_require_finite_total_send_admission() {
    use buoyant_kernel_engine::resource::try_channel;
    let (scope, accounting) = scope(40);
    let runtime = runtime(scope.clone()); let entered = runtime.enter();
    let policy = policy(scope.clone()); let guard = policy.enter_thread();
    let (sender, mut receiver) = try_channel::<u64>(1).unwrap();
    let accepted = runtime.block_on(async {
        let mut count = 0;
        while sender.send(count, "closed").await.is_ok() {
            assert_eq!(receiver.recv().await.unwrap(), Some(count));
            count += 1;
        }
        count
    });
    assert!(accepted > 0 && accepted < 40);
    assert!(scope.failure().is_some());
    drop((sender, receiver, guard, policy)); drop(entered); drop(runtime); drop(scope);
    assert_eq!(accounting.live.load(Ordering::SeqCst), 0);
}

#[test]
fn compatibility_background_executor_runs_inside_current_thread_runtime() {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let executor = TokioBackgroundExecutor::try_new().unwrap();
    runtime.block_on(async {
        assert_eq!(executor.try_block_on(async { 29 }).unwrap(), 29);
    });
}
