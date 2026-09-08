//! Uses the production Tokio admission adapter; fixture policies are installed
//! on every native worker and removed only at worker shutdown.
#![allow(dead_code)]
#[path = "../../../../src/fabric/native_tokio_resource.rs"]
mod native_tokio_resource;
use std::sync::Arc;
use buoyant_kernel::resource::{JsonResourceLimits, NativeResourceScope, NativeResourceThreadPolicy, NativeResourceThreadGuard};
use tokio::runtime::{Runtime, resource::LocalRuntimeProfile};

pub fn profile() -> LocalRuntimeProfile {
    LocalRuntimeProfile { worker_threads: 1, blocking_threads: 2, blocking_queue: 16,
        thread_stack_bytes: 2 << 20, async_tasks: 256, blocking_tasks: 256 }
}
pub fn runtime<Start, Stop>(scope: Arc<NativeResourceScope>, start: Start, stop: Stop) -> Runtime
where Start: Fn() + Send + Sync + 'static, Stop: Fn() + Send + Sync + 'static {
    profile().build(native_tokio_resource::NativeTokioAdmission::try_new(scope).unwrap(), start, stop).unwrap()
}
thread_local! { static KERNEL: std::cell::RefCell<Option<NativeResourceThreadGuard>> = const { std::cell::RefCell::new(None) }; }
pub fn kernel_runtime(scope: Arc<NativeResourceScope>, limits: JsonResourceLimits) -> Runtime {
    let policy = NativeResourceThreadPolicy::try_new(scope.clone(), limits).unwrap();
    runtime(scope, move || KERNEL.with(|slot| *slot.borrow_mut() = Some(policy.enter_thread())),
        || KERNEL.with(|slot| { slot.borrow_mut().take(); }))
}
