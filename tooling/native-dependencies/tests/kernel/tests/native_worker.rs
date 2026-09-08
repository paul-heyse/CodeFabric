//! Actual native policies wired to the unchanged first-party hierarchical budget.
#![allow(dead_code)]
#[path = "../../../../../src/resource_budget.rs"]
mod resource_budget;
#[path = "../../../../../src/fabric/native_resource_policy.rs"]
mod native_resource_policy;
use native_resource_policy::*;
use resource_budget::*;
use std::cell::RefCell;
use std::sync::Arc;
use arrow_schema::resource::{ResourceAllocationRequest, RetainedResourceOwner};

fn budget() -> ResourceBudget {
    let policy = ResourceBudgetPolicy {
        limits: ResourceAmounts { memory_bytes: 64 << 20, disk_bytes: 1000, running_jobs: 100, queued_jobs: 100, retained_generations: 100, retained_bytes: 1 << 20, rows: 1000, pages: 1000 },
        control_reserve: ResourceAmounts { memory_bytes: 16 << 20, disk_bytes: 1, running_jobs: 1, queued_jobs: 1, retained_generations: 1, retained_bytes: 1, rows: 1, pages: 1 },
    };
    ResourceBudget::try_process([1;16], policy).unwrap().workspace([2;16], policy).unwrap()
}
fn limits() -> NativeResourceLimits {
    NativeResourceLimits {
        url_reference_bytes: 65536,
        original_owners: 4, original_owner_depth: 8, kernel_allocations: 128,
        kernel_json: buoyant_kernel::resource::JsonResourceLimits { max_bytes: 65536, max_tokens: 4096, max_depth: 32, max_string_bytes: 16384, max_container_items: 1024 },
        json: arrow_json::resource::ReaderResourceLimits { allocations: 128, collection_entries: 4096, string_bytes: 65536, nesting: 32 },
        parquet: parquet::resource::ReaderResourceLimits { allocations: 128, collection_entries: 4096, string_bytes: 65536, footer_bytes: 65536, page_bytes: 65536, page_values: 4096, output_values: 4096, output_bytes: 65536, schema_depth: 32, codec_bytes: 1 << 20 },
    }
}

fn new_owner(budget: ResourceBudget, class: ResourceClass, limits: NativeResourceLimits) -> Result<Arc<NativeResourceOwner>, buoyant_kernel::resource::ResourceExhausted> {
    NativeResourceOwner::try_new(budget, class, limits, &[url::Url::parse("file:///workspace/").unwrap()])
}

#[test]
fn native_worker_acyclic_adoption_retains_actual_input_policy_roots() {
    let budget = budget();
    let baseline = budget.observation().used.memory_bytes;
    let original = new_owner(budget.clone(), ResourceClass::Data, limits()).unwrap();
    original.mark_joined().unwrap();
    let output = new_owner(budget.clone(), ResourceClass::Data, limits()).unwrap();
    output.try_adopt(original.clone()).unwrap();
    output.try_adopt(original.clone()).unwrap();
    let weak = Arc::downgrade(&original);
    assert!(original.try_adopt(output.clone()).is_err());
    drop(original);
    assert!(weak.upgrade().is_some());
    drop(output);
    assert!(weak.upgrade().is_none());
    assert_eq!(budget.observation().used.memory_bytes, baseline);
}

#[test]
fn native_worker_owner_capacity_and_foreign_workspace_fail_closed() {
    let first_budget = budget();
    let first = new_owner(first_budget.clone(), ResourceClass::Data, limits()).unwrap();
    let second = new_owner(first_budget.clone(), ResourceClass::Data, limits()).unwrap();
    first.mark_joined().unwrap(); second.mark_joined().unwrap();
    let mut policy = limits(); policy.original_owners = 1;
    let output = new_owner(first_budget, ResourceClass::Data, policy).unwrap();
    output.try_adopt(first.clone()).unwrap();
    let error = output.try_adopt(second).unwrap_err();
    assert_eq!(error.kind, "native original owner slots");
    assert_eq!(output.check_available().unwrap_err().kind, error.kind);
    let foreign = new_owner(budget(), ResourceClass::Data, limits()).unwrap();
    assert_eq!(foreign.try_adopt(first).unwrap_err().kind, "native foreign workspace owner");
}

#[test]
fn native_worker_allocation_callback_uses_shared_budget_and_preserves_control_headroom() {
    let budget = budget();
    let owner = new_owner(budget.clone(), ResourceClass::Data, limits()).unwrap();
    owner.try_reserve_allocation(ResourceAllocationRequest { bytes: 40 << 20, alignment: 64, kind: "original native buffer" }).unwrap();
    assert!(budget.observation().data_used.memory_bytes >= 40 << 20);
    let error = owner.try_reserve_allocation(ResourceAllocationRequest { bytes: 12 << 20, alignment: 64, kind: "native replacement buffer" }).unwrap_err();
    assert_eq!(error.kind, "native replacement buffer");
    assert_eq!(owner.check_available().unwrap_err().kind, error.kind);
    let control = budget.try_reserve(ResourceClass::Control, ResourceAmounts { memory_bytes: 16 << 20, ..ResourceAmounts::default() }).unwrap();
    drop(control);
    let before = budget.observation().used.memory_bytes;
    assert!(owner.try_reserve_allocation(ResourceAllocationRequest { bytes: 1, alignment: 8, kind: "later allocation" }).is_err());
    assert_eq!(budget.observation().used.memory_bytes, before);
}

#[test]
fn native_worker_rejects_unjoined_input_and_bounds_final_owner_chain_depth() {
    let budget = budget();
    let first = new_owner(budget.clone(), ResourceClass::Data, limits()).unwrap();
    let rejected = new_owner(budget.clone(), ResourceClass::Data, limits()).unwrap();
    assert_eq!(rejected.try_adopt(first.clone()).unwrap_err().kind, "native input owner has not joined");
    first.mark_joined().unwrap();
    assert!(first.scope().reserve(buoyant_kernel::resource::AllocationRequest { kind: "late native allocation", bytes: 1 }).unwrap_err().is_resource_exhausted());
    let second = new_owner(budget.clone(), ResourceClass::Data, limits()).unwrap();
    second.try_adopt(first).unwrap();
    second.mark_joined().unwrap();
    let mut policy = limits(); policy.original_owner_depth = 1;
    let third = new_owner(budget, ResourceClass::Data, policy).unwrap();
    assert_eq!(third.try_adopt(second).unwrap_err().kind, "native original owner depth");
}

thread_local! { static WORKER: RefCell<Option<NativeResourceThreadGuard>> = const { RefCell::new(None) }; }
fn verify_current(expected: &Arc<NativeResourceOwner>) {
    assert!(Arc::ptr_eq(&buoyant_kernel::resource::current_resource_scope().unwrap(), expected.scope()));
    assert!(arrow_json::resource::ReaderResourcePolicy::current().is_some());
    assert!(parquet::resource::ReaderResourcePolicy::current().is_some());
    let expected: Arc<dyn RetainedResourceOwner> = expected.clone();
    assert!(Arc::ptr_eq(&arrow_schema::resource::current_resource_owner().unwrap(), &expected));
}

#[test]
fn native_worker_all_policies_cover_coordinator_runtime_and_blocking_threads() {
    let budget = budget();
    let baseline = budget.observation().used.memory_bytes;
    let owner = new_owner(budget.clone(), ResourceClass::Data, limits()).unwrap();
    let weak = Arc::downgrade(&owner);
    let start_owner = owner.clone();
    let guard = owner.enter_thread().unwrap();
    verify_current(&owner);
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).max_blocking_threads(3)
        .on_thread_start(move || {
            let guard = start_owner.enter_thread().unwrap();
            WORKER.with(|slot| *slot.borrow_mut() = Some(guard));
        })
        .on_thread_stop(|| WORKER.with(|slot| { slot.borrow_mut().take(); })).build().unwrap();
    let output = runtime.block_on(async {
        let async_owner = owner.clone();
        let asynchronous = tokio::spawn(async move {
            verify_current(&async_owner);
            // The fixture itself also admits its fixed native construction:
            // Schema Arc, Field value/Arc, Vec<Field>/Vec<FieldRef>/Fields slice
            // layouts, their three Arc headers and the original field name.
            let bytes = std::mem::size_of::<arrow_schema::Schema>()
                + 2 * std::mem::size_of::<arrow_schema::Field>()
                + 3 * std::mem::size_of::<Arc<arrow_schema::Field>>()
                + 6 * std::mem::size_of::<usize>() + "original field".len();
            async_owner.try_reserve_allocation(ResourceAllocationRequest { bytes, alignment: std::mem::align_of::<arrow_schema::Field>(), kind: "probe native schema" }).unwrap();
            Arc::new(arrow_schema::Schema::new(vec![arrow_schema::Field::new("original field", arrow_schema::DataType::Int64, true)]))
        });
        let blocking_owner = owner.clone();
        tokio::task::spawn_blocking(move || verify_current(&blocking_owner)).await.unwrap();
        asynchronous.await.unwrap()
    });
    drop(runtime); drop(guard); drop(owner);
    assert!(buoyant_kernel::resource::current_resource_scope().is_none());
    assert!(weak.upgrade().is_some());
    drop(output);
    assert!(weak.upgrade().is_none());
    assert_eq!(budget.observation().used.memory_bytes, baseline);
}

#[test]
fn native_worker_starting_after_failure_still_installs_every_required_policy() {
    let owner = new_owner(budget(), ResourceClass::Data, limits()).unwrap();
    owner.record_failure(arrow_schema::resource::ResourceOwnerError { kind: "prior worker exhausted", requested: 9, limit: 8 });
    std::thread::spawn(move || {
        let _guard = owner.enter_thread().unwrap();
        verify_current(&owner);
        let error = owner.scope().reserve(buoyant_kernel::resource::AllocationRequest { kind: "later worker", bytes: 1 }).unwrap_err();
        assert!(error.is_resource_exhausted());
        assert_eq!(owner.check_available().unwrap_err().kind, "prior worker exhausted");
    }).join().unwrap();
}

#[test]
fn native_worker_url_policy_covers_all_worker_kinds_and_rejects_foreign_origins() {
    let owner = new_owner(budget(), ResourceClass::Data, limits()).unwrap();
    let base = url::Url::parse("file:///workspace/_delta_log/").unwrap();
    let start = owner.clone();
    let _guard = owner.enter_thread().unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(1).max_blocking_threads(1)
        .on_thread_start(move || { WORKER.with(|slot| *slot.borrow_mut() = Some(start.enter_thread().unwrap())); })
        .on_thread_stop(|| WORKER.with(|slot| { slot.borrow_mut().take(); })).build().unwrap();
    let first = buoyant_kernel::OwnedFileMeta::try_from_reference(&base, "original.json", 0, 0).unwrap();
    let asynchronous = runtime.block_on(async {
        let base_for_blocking = base.clone();
        let asynchronous = tokio::spawn(async move { buoyant_kernel::OwnedFileMeta::try_from_reference(&base, "nested.json", 0, 0).unwrap() });
        tokio::task::spawn_blocking(move || {
            assert!(buoyant_kernel::OwnedFileMeta::try_from_reference(&base_for_blocking, "https://foreign.example/private", 0, 0).is_err());
        }).await.unwrap();
        asynchronous.await.unwrap()
    });
    drop(runtime);
    assert!(Arc::ptr_eq(first.resource_scope().unwrap(), owner.scope()));
    assert!(Arc::ptr_eq(asynchronous.resource_scope().unwrap(), owner.scope()));
}
