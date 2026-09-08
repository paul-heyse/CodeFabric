#![allow(dead_code, unused_imports)]
#[path = "../../../../../src/resource_budget.rs"]
mod resource_budget;
#[path = "../../../../../src/cancellation.rs"]
mod cancellation;
mod fabric;
use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};
use cancellation::StructuredCancellationScope;
use resource_budget::*;
use fabric::native_execution_lane::*;
use fabric::native_resource_policy::*;
use fabric::native_lane_resource_policy::*;
use buoyant_kernel::{EvaluationHandler, expressions::Scalar, schema::{DataType, StructType, StructField}};
use buoyant_kernel::engine::{arrow_expression::ArrowEvaluationHandler, arrow_data::EngineDataArrowExt};
use arrow_array::cast::AsArray;
use arrow_array::types::Int64Type;
use arrow_schema::resource::{RetainedResourceOwner, ResourceAllocationRequest};

fn setup() -> (ResourceBudget, StructuredCancellationScope, NativeExecutionLane, NativeResourceLimits) {
    let policy = ResourceBudgetPolicy {
        limits: ResourceAmounts { memory_bytes: 128<<20, disk_bytes: 1<<20, running_jobs: 4, queued_jobs: 32, retained_generations: 8, retained_bytes: 1<<20, rows: 65536, pages: 1024 },
        control_reserve: ResourceAmounts { memory_bytes: 32<<20, running_jobs: 1, queued_jobs: 4, ..ResourceAmounts::default() }
    };
    let budget = ResourceBudget::try_process([10;16], policy).unwrap().workspace([11;16], policy).unwrap();
    let scope = StructuredCancellationScope::try_root_with_control_reserve("native-joined-probe", NonZeroUsize::new(4).unwrap(), NonZeroUsize::new(1).unwrap()).unwrap();
    // This fixture is a finite execution harness, not production calibration or
    // certification of Tokio's entire runtime allocation geometry.
    let lane = NativeExecutionLane::try_new(NativeLaneEnvelope { worker_threads: NonZeroUsize::new(1).unwrap(), blocking_threads: NonZeroUsize::new(4).unwrap(),
        thread_stack_bytes: NonZeroUsize::new(2<<20).unwrap(), runtime_memory_bytes: NonZeroU64::new(2<<20).unwrap(), native_buffer_bytes: 0,
        native_task_slots: NonZeroU64::new(16).unwrap(), task_memory_bytes: NonZeroU64::new(16384).unwrap(), parallel_blocking_roots: NonZeroUsize::new(1).unwrap(), blocking_nesting: NonZeroUsize::new(1).unwrap() }).unwrap();
    let limits = NativeResourceLimits { kernel_allocations: 1024, original_owners: 8, original_owner_depth: 8, url_reference_bytes: 65536,
        kernel_json: buoyant_kernel::resource::JsonResourceLimits { max_bytes: 65536, max_tokens: 4096, max_depth: 32, max_string_bytes: 65536, max_container_items: 1024 },
        json: arrow_json::resource::ReaderResourceLimits { allocations: 1024, collection_entries: 4096, string_bytes: 65536, nesting: 32 },
        parquet: parquet::resource::ReaderResourceLimits { allocations: 1024, collection_entries: 4096, string_bytes: 65536, footer_bytes: 65536, page_bytes: 65536, page_values: 4096, output_values: 4096, output_bytes: 65536, schema_depth: 32, codec_bytes: 1<<20 } };
    (budget,scope,lane,limits)
}
fn owner(budget: &ResourceBudget, limits: NativeResourceLimits) -> Arc<NativeResourceOwner> {
    NativeResourceOwner::try_new(budget.clone(), ResourceClass::Data, limits, &[url::Url::parse("file:///workspace/").unwrap()]).unwrap()
}
fn no_cleanup() -> NativeLaneCleanup<impl FnOnce() -> std::future::Ready<Result<(), &'static str>>, impl FnOnce(NativeLaneJoined) -> Result<(), &'static str>> {
    NativeLaneCleanup { before_join: || std::future::ready(Ok(())), after_join: |_| Ok(()) }
}
pub async fn joined_native_original_batch_survives_worker_and_policy_drop() {
    let (budget, scope, lane, limits) = setup();
    let native = owner(&budget, limits); let weak = Arc::downgrade(&native);
    let bridge = NativeOperationResourcePolicy::try_new(native.clone()).unwrap();
    let schema = Arc::new(StructType::try_new([StructField::nullable("original", DataType::LONG)]).unwrap());
    let started = Arc::new(AtomicBool::new(false)); let joined = Arc::new(AtomicBool::new(false));
    let worker_started = started.clone(); let worker_joined = joined.clone(); let worker_owner = native.clone();
    let output = lane.spawn_with_resource_policy(NativeLaneAdmission { scope: &scope, name: "native-original-batch", budget: &budget, class: ResourceClass::Data, deadline: Instant::now()+Duration::from_secs(10) }, Some(bridge.clone()),
        move |_| async move {
            tokio::task::spawn_blocking(move || {
                assert!(Arc::ptr_eq(&buoyant_kernel::resource::current_resource_scope().unwrap(), worker_owner.scope()));
                worker_started.store(true, Ordering::Release);
                std::thread::sleep(Duration::from_millis(30));
                worker_joined.store(true, Ordering::Release);
            });
            let row = [Scalar::Long(7)];
            ArrowEvaluationHandler.create_many(schema, &[&row]).unwrap().try_into_owned_record_batch().map_err(|_| NativeLaneError::Panicked)
        }, no_cleanup()).await.unwrap().wait().await.unwrap().unwrap();
    assert!(started.load(Ordering::Acquire) && joined.load(Ordering::Acquire));
    assert!(bridge.joined_runtime().is_some()); assert!(native.scope().allocations_sealed());
    let bare_buffer = output.column(0).as_primitive::<Int64Type>().values().inner().clone();
    drop(output); drop(bridge); drop(native);
    scope.cancel_and_join(Duration::from_secs(5)).await.unwrap();
    assert!(weak.upgrade().is_some()); assert!(budget.observation().used.memory_bytes > 0);
    drop(bare_buffer); assert!(weak.upgrade().is_none()); assert_eq!(budget.observation().used.memory_bytes, 0);
}
pub async fn hidden_native_failure_rejects_earlier_success_after_join() {
    let (budget, scope, lane, limits) = setup(); let native = owner(&budget, limits); let worker = native.clone();
    let bridge = NativeOperationResourcePolicy::try_new(native).unwrap();
    let output = lane.spawn_with_resource_policy(NativeLaneAdmission { scope: &scope, name: "native-hidden-denial", budget: &budget, class: ResourceClass::Data, deadline: Instant::now()+Duration::from_secs(10) }, Some(bridge.clone()),
        move |_| async move {
            tokio::task::spawn_blocking(move || {
                std::thread::sleep(Duration::from_millis(30));
                assert!(worker.try_reserve_allocation(ResourceAllocationRequest { kind: "hidden original allocation", bytes: 128<<20, alignment: 64 }).is_err());
            }); Ok(7_u64)
        }, no_cleanup()).await.unwrap().wait().await.unwrap();
    assert!(matches!(output, Err(NativeLaneError::NativeResourceExhausted { kind: "hidden original allocation", .. })));
    assert!(bridge.joined_runtime().is_some()); drop(bridge);
    scope.cancel_and_join(Duration::from_secs(5)).await.unwrap(); assert_eq!(budget.observation().used.memory_bytes, 0);
}
pub async fn resource_owner_cannot_bind_two_native_operations() {
    let (budget, scope, lane, limits) = setup(); let native = owner(&budget, limits);
    let first = NativeOperationResourcePolicy::try_new(native.clone()).unwrap();
    let second = NativeOperationResourcePolicy::try_new(native.clone()).unwrap();
    first.begin_operation().unwrap();
    let invoked = Arc::new(AtomicBool::new(false)); let factory = invoked.clone();
    let denied = lane.spawn_with_resource_policy(NativeLaneAdmission { scope: &scope, name: "duplicate-native-owner", budget: &budget, class: ResourceClass::Data, deadline: Instant::now()+Duration::from_secs(10) }, Some(second.clone()),
        move |_| { factory.store(true, Ordering::Release); async { Ok(()) } }, no_cleanup()).await;
    assert!(matches!(denied, Err(NativeLaneError::NativeResourceExhausted { kind: "native resource owner already bound", .. })));
    assert!(!invoked.load(Ordering::Acquire)); drop(first); drop(second); drop(native);
    scope.cancel_and_join(Duration::from_secs(5)).await.unwrap(); assert_eq!(budget.observation().used.memory_bytes, 0);
}
