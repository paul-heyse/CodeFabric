#![allow(dead_code)]
#[path = "../../../support/native_runtime.rs"]
mod native_runtime;
use std::sync::Arc;
use crate::{native_resource_policy::*, resource_budget::*};
pub fn budget() -> ResourceBudget {
    let policy = ResourceBudgetPolicy {
        limits: ResourceAmounts { memory_bytes: 256 << 20, disk_bytes: 1000, running_jobs: 100, queued_jobs: 100, retained_generations: 100, retained_bytes: 1 << 20, rows: 1000, pages: 1000 },
        control_reserve: ResourceAmounts { memory_bytes: 16 << 20, disk_bytes: 1, running_jobs: 1, queued_jobs: 1, retained_generations: 1, retained_bytes: 1, rows: 1, pages: 1 },
    };
    ResourceBudget::try_process([1;16], policy).unwrap().workspace([2;16], policy).unwrap()
}
pub fn limits() -> NativeResourceLimits {
    NativeResourceLimits {
        url_reference_bytes: 65536,
        original_owners: 4, original_owner_depth: 8, kernel_allocations: 4096,
        kernel_json: buoyant_kernel::resource::JsonResourceLimits { max_bytes: 65536, max_tokens: 4096, max_depth: 32, max_string_bytes: 16384, max_container_items: 1024 },
        json: arrow_json::resource::ReaderResourceLimits { allocations: 128, collection_entries: 4096, string_bytes: 65536, nesting: 32 },
        parquet: parquet::resource::ReaderResourceLimits { allocations: 128, collection_entries: 4096, string_bytes: 65536, footer_bytes: 65536, page_bytes: 65536, page_values: 4096, output_values: 4096, output_bytes: 65536, schema_depth: 32, codec_bytes: 1 << 20 },
    }
}

pub fn new_owner(budget: ResourceBudget, class: ResourceClass, limits: NativeResourceLimits) -> Result<Arc<NativeResourceOwner>, buoyant_kernel::resource::ResourceExhausted> {
    NativeResourceOwner::try_new(budget, class, limits, &[url::Url::parse("file:///workspace/").unwrap()])
}

thread_local! { static THREAD_POLICY: std::cell::RefCell<Option<NativeResourceThreadGuard>> = const { std::cell::RefCell::new(None) }; }
pub fn runtime(owner: Arc<NativeResourceOwner>) -> tokio::runtime::Runtime {
    native_runtime::runtime(owner.scope().clone(),
        move || THREAD_POLICY.with(|slot| *slot.borrow_mut() = Some(owner.enter_thread().unwrap())),
        || THREAD_POLICY.with(|slot| { slot.borrow_mut().take(); }))
}
