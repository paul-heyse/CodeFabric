//! Abstracts out the APIs necessary to `Runtime` for integrating the blocking
//! pool. When the `blocking` feature flag is **not** enabled, these APIs are
//! shells. This isolates the complexity of dealing with conditional
//! compilation.

mod pool;
pub(crate) use pool::{spawn_blocking, BlockingPool, Spawner};

cfg_fs! {
    pub(crate) use pool::spawn_mandatory_blocking;
}

cfg_trace! {
    pub(crate) use pool::Mandatory;
}

mod schedule;
mod shutdown;
mod task;
pub(crate) use task::BlockingTask;

use crate::runtime::Builder;

pub(crate) fn create_blocking_pool(builder: &Builder, thread_cap: usize) -> BlockingPool {
    BlockingPool::new(builder, thread_cap)
}

#[cfg(all(feature = "rt-multi-thread", feature = "time"))]
pub(crate) use pool::resource_heap_bytes;

#[cfg(all(feature = "rt-multi-thread", feature = "time"))]
pub(crate) fn resource_task_bytes<F, R>() -> Result<usize, crate::runtime::resource::ResourceLayoutError>
where F: FnOnce() -> R + Send + 'static, R: Send + 'static {
    if std::mem::size_of::<F>() > crate::runtime::BOX_FUTURE_THRESHOLD {
        crate::runtime::resource::add(std::mem::size_of::<F>(), crate::runtime::task::resource_task_bytes::<BlockingTask<Box<F>>, schedule::BlockingSchedule>())
    } else {
        Ok(crate::runtime::task::resource_task_bytes::<BlockingTask<F>, schedule::BlockingSchedule>())
    }
}
