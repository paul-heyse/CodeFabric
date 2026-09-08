use super::*;

use std::sync::Condvar;

use arrow_array::{Int64Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema};
use datafusion::common::runtime::SpawnedTask;
use datafusion::datasource::memory::MemorySourceConfig;
use datafusion::execution::context::SessionContext;
use datafusion::execution::runtime_env::RuntimeEnv;
use datafusion::physical_plan::repartition::RepartitionExec;
use datafusion::physical_plan::stream::RecordBatchReceiverStreamBuilder;
use datafusion::physical_plan::{Partitioning, collect};
use datafusion::prelude::SessionConfig;
use futures::StreamExt as _;
use tokio::sync::oneshot;

use crate::fabric::child_session::ChildResourceLimits;
use crate::fabric::child_session::resource_governance::{
    EpochResourceCoordinator, EpochResourceError, EpochResourcePolicy, EpochWorkClass,
    EpochWorkRequest, WorkspaceResourceCoordinator, test_lifecycle_work_class_policies,
};
use crate::fabric::command::{EpochId, PrincipalId};
use crate::resource_budget::{ResourceAmounts, ResourceBudget, ResourceBudgetPolicy};

struct Fixture {
    coordinator: EpochResourceCoordinator,
    budget: ResourceBudget,
    runtime: Arc<RuntimeEnv>,
}

fn fixture(concurrent: usize) -> Fixture {
    let envelope = ResourceBudgetPolicy {
        limits: ResourceAmounts {
            memory_bytes: 64 << 20,
            disk_bytes: 64 << 20,
            running_jobs: 16,
            queued_jobs: 32,
            retained_generations: 16,
            retained_bytes: 64 << 20,
            rows: 1_000_000,
            pages: 1024,
        },
        control_reserve: ResourceAmounts {
            memory_bytes: 8 << 20,
            running_jobs: 2,
            queued_jobs: 2,
            ..ResourceAmounts::default()
        },
    };
    let budget = ResourceBudget::try_process([81; 16], envelope)
        .unwrap()
        .workspace([82; 16], envelope)
        .unwrap();
    let child = ChildResourceLimits::try_new(1 << 20, 4 << 20, 4, 4, 128, 2).unwrap();
    let runtime = child.runtime_env().unwrap();
    let policy = EpochResourcePolicy::try_new(
        child,
        test_lifecycle_work_class_policies(),
        concurrent,
        1,
        16,
        10_000,
        1,
        2,
        2,
        1 << 20,
        20_000,
    )
    .unwrap();
    let coordinator = WorkspaceResourceCoordinator::try_new(
        [83; 32],
        policy,
        Arc::clone(&runtime),
        budget.clone(),
    )
    .unwrap()
    .for_epoch(EpochId::from_bytes([84; 16]))
    .unwrap();
    Fixture {
        coordinator,
        budget,
        runtime,
    }
}

fn request(class: EpochWorkClass, cancellation: Cancellation) -> EpochWorkRequest {
    EpochWorkRequest {
        epoch_id: EpochId::from_bytes([84; 16]),
        principal_id: PrincipalId::from_bytes([85; 16]),
        class,
        cancellation,
    }
}

fn schema() -> arrow_schema::SchemaRef {
    Arc::new(Schema::new(vec![Field::new(
        "value",
        DataType::Int64,
        false,
    )]))
}

/// A failing test still opens each gate before runtime shutdown, avoiding an orphaned worker.
struct Gate(Arc<(Mutex<bool>, Condvar)>);
struct GateWaiter(Arc<(Mutex<bool>, Condvar)>);

fn gate() -> (Gate, GateWaiter) {
    let state = Arc::new((Mutex::new(false), Condvar::new()));
    (Gate(Arc::clone(&state)), GateWaiter(state))
}

impl Gate {
    fn open(&self) {
        *self.0.0.lock().unwrap() = true;
        self.0.1.notify_all();
    }
}
impl Drop for Gate {
    fn drop(&mut self) {
        self.open();
    }
}
impl GateWaiter {
    fn wait(&self) {
        let (ready, timeout) = self
            .0
            .1
            .wait_timeout_while(self.0.0.lock().unwrap(), Duration::from_secs(10), |open| {
                !*open
            })
            .unwrap();
        assert!(*ready && !timeout.timed_out(), "native test gate timed out");
    }
}

async fn entered(receiver: oneshot::Receiver<()>) {
    tokio::time::timeout(Duration::from_secs(2), receiver)
        .await
        .unwrap()
        .unwrap();
}

async fn blocked_stream(waiter: GateWaiter, entered: oneshot::Sender<()>) {
    let mut builder = RecordBatchReceiverStreamBuilder::new(schema(), 1);
    builder.spawn_blocking(move || {
        entered.send(()).unwrap();
        waiter.wait();
        Ok(())
    });
    let mut stream = bind_stream(builder.build());
    while stream.next().await.is_some() {}
}

async fn assert_control_and_data_pressure(fixture: &Fixture) {
    assert_eq!(fixture.coordinator.observation().unwrap().active_work, 1);
    assert_eq!(fixture.budget.observation().used.running_jobs, 1);
    assert!(
        tokio::time::timeout(
            Duration::from_millis(20),
            fixture.coordinator.admit(request(
                EpochWorkClass::InteractiveQuery,
                Cancellation::default()
            )),
        )
        .await
        .is_err(),
        "live native work must retain the data slot"
    );
    let control = tokio::time::timeout(
        Duration::from_millis(100),
        fixture.coordinator.admit(request(
            EpochWorkClass::SecurityRecovery,
            Cancellation::default(),
        )),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(control.run(async { 7 }).await.unwrap(), 7);
    drop(control);
    assert_eq!(fixture.budget.observation().used.running_jobs, 1);
}

async fn await_idle(fixture: &Fixture) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while fixture.coordinator.observation().unwrap().active_work != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(fixture.budget.observation().used.running_jobs, 0);
    assert_eq!(
        fixture
            .coordinator
            .observation()
            .unwrap()
            .native_operations
            .live_tasks,
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wp79_native_cancel_drains_started_blocking_producer_before_slot_reuse() {
    let fixture = fixture(2);
    let cancellation = Cancellation::with_check_interval(1);
    let permit = fixture
        .coordinator
        .admit(request(
            EpochWorkClass::InteractiveQuery,
            cancellation.clone(),
        ))
        .await
        .unwrap();
    let (release, waiter) = gate();
    let (send, receive) = oneshot::channel();
    let task = tokio::spawn(async move { permit.run(blocked_stream(waiter, send)).await });
    entered(receive).await;
    cancellation.cancel();
    assert_control_and_data_pressure(&fixture).await;
    assert!(
        !task.is_finished(),
        "cancellation intent is not native termination"
    );
    release.open();
    assert!(matches!(
        task.await.unwrap(),
        Err(EpochResourceError::Cancelled)
    ));
    await_idle(&fixture).await;
    let observed = fixture.coordinator.observation().unwrap().native_operations;
    assert!(observed.cancelled_operations >= 1);
    assert_eq!(observed.completed_tasks, 1);
    assert_eq!(observed.completed_streams, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wp79_native_caller_abort_keeps_terminal_owner_without_detached_cleanup() {
    let fixture = fixture(2);
    let permit = fixture
        .coordinator
        .admit(request(
            EpochWorkClass::InteractiveQuery,
            Cancellation::default(),
        ))
        .await
        .unwrap();
    let (release, waiter) = gate();
    let (send, receive) = oneshot::channel();
    let task = tokio::spawn(async move { permit.run(blocked_stream(waiter, send)).await });
    entered(receive).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_control_and_data_pressure(&fixture).await;
    release.open();
    await_idle(&fixture).await;
    let next = fixture
        .coordinator
        .admit(request(
            EpochWorkClass::InteractiveQuery,
            Cancellation::default(),
        ))
        .await
        .unwrap();
    drop(next);
    await_idle(&fixture).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wp79_native_late_blocking_descendant_inherits_cancelled_parent_owner() {
    let fixture = fixture(2);
    let permit = fixture
        .coordinator
        .admit(request(
            EpochWorkClass::InteractiveQuery,
            Cancellation::default(),
        ))
        .await
        .unwrap();
    let (spawn_child, parent_waiter) = gate();
    let (release_child, child_waiter) = gate();
    let (parent_send, parent_receive) = oneshot::channel();
    let (child_send, child_receive) = oneshot::channel();
    let (parent_finished_send, parent_finished_receive) = oneshot::channel();
    let task = tokio::spawn(async move {
        permit
            .run(async move {
                let mut builder = RecordBatchReceiverStreamBuilder::new(schema(), 1);
                builder.spawn_blocking(move || {
                    parent_send.send(()).unwrap();
                    parent_waiter.wait();
                    let (started_send, started_receive) = std::sync::mpsc::sync_channel(1);
                    let child = SpawnedTask::spawn_blocking(move || {
                        started_send.send(()).unwrap();
                        child_send.send(()).unwrap();
                        child_waiter.wait();
                    });
                    started_receive
                        .recv_timeout(Duration::from_secs(2))
                        .unwrap();
                    drop(child); // native cancellation cannot abort this already-started child
                    parent_finished_send.send(()).unwrap();
                    Ok(())
                });
                let mut stream = bind_stream(builder.build());
                while stream.next().await.is_some() {}
            })
            .await
    });
    entered(parent_receive).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    spawn_child.open();
    entered(child_receive).await;
    entered(parent_finished_receive).await;
    assert_control_and_data_pressure(&fixture).await;
    release_child.open();
    await_idle(&fixture).await;
    assert!(
        fixture
            .coordinator
            .observation()
            .unwrap()
            .native_operations
            .completed_tasks
            >= 2
    );
}

#[tokio::test]
async fn wp79_native_unpolled_abort_and_panic_release_exactly_once() {
    let fixture = fixture(2);
    let permit = fixture
        .coordinator
        .admit(request(
            EpochWorkClass::InteractiveQuery,
            Cancellation::default(),
        ))
        .await
        .unwrap();
    let entered = Arc::new(AtomicUsize::new(0));
    let entered_task = Arc::clone(&entered);
    permit
        .run(async {
            let task = SpawnedTask::spawn(async move {
                entered_task.fetch_add(1, Ordering::AcqRel);
            });
            drop(task); // current-thread scheduler has not polled it
        })
        .await
        .unwrap();
    assert_eq!(entered.load(Ordering::Acquire), 0);
    drop(permit);
    await_idle(&fixture).await;
    let permit = fixture
        .coordinator
        .admit(request(
            EpochWorkClass::InteractiveQuery,
            Cancellation::default(),
        ))
        .await
        .unwrap();
    let result = std::panic::AssertUnwindSafe(permit.run(async {
        let mut builder = RecordBatchReceiverStreamBuilder::new(schema(), 1);
        builder.spawn(async { panic!("intentional native panic") });
        let mut stream = bind_stream(builder.build());
        while stream.next().await.is_some() {}
    }))
    .catch_unwind()
    .await;
    assert!(
        result.is_err(),
        "ordinary native panics preserve native failure semantics"
    );
    drop(permit);
    await_idle(&fixture).await;
    assert_eq!(
        fixture
            .coordinator
            .observation()
            .unwrap()
            .native_operations
            .completed_tasks,
        2
    );
}

#[tokio::test]
async fn wp79_native_capacity_rejects_before_excess_task_scheduling_and_drains() {
    let fixture = fixture(2);
    let permit = fixture
        .coordinator
        .admit(request(
            EpochWorkClass::InteractiveQuery,
            Cancellation::default(),
        ))
        .await
        .unwrap();
    let result = permit
        .run(async {
            let mut builder = RecordBatchReceiverStreamBuilder::new(schema(), 1);
            for _ in 0..=MAX_NATIVE_TASKS {
                builder.spawn(std::future::pending());
            }
            builder.build()
        })
        .await;
    assert!(matches!(
        result,
        Err(EpochResourceError::Native(
            NativeOperationError::TaskCapacity {
                maximum: MAX_NATIVE_TASKS
            }
        ))
    ));
    let observed = fixture.coordinator.observation().unwrap().native_operations;
    assert_eq!(observed.peak_tasks, MAX_NATIVE_TASKS);
    assert_eq!(observed.completed_tasks, MAX_NATIVE_TASKS as u64);
    assert_eq!(observed.capacity_failures, 1);
    drop(permit);
    await_idle(&fixture).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wp79_native_concurrent_owners_do_not_release_each_others_admission() {
    let fixture = fixture(3);
    let first = fixture
        .coordinator
        .admit(request(
            EpochWorkClass::InteractiveQuery,
            Cancellation::default(),
        ))
        .await
        .unwrap();
    let second = fixture
        .coordinator
        .admit(request(
            EpochWorkClass::InteractiveQuery,
            Cancellation::default(),
        ))
        .await
        .unwrap();
    let (first_release, first_waiter) = gate();
    let (second_release, second_waiter) = gate();
    let (first_send, first_receive) = oneshot::channel();
    let (second_send, second_receive) = oneshot::channel();
    let first_task =
        tokio::spawn(async move { first.run(blocked_stream(first_waiter, first_send)).await });
    let second_task =
        tokio::spawn(async move { second.run(blocked_stream(second_waiter, second_send)).await });
    entered(first_receive).await;
    entered(second_receive).await;
    first_task.abort();
    assert!(first_task.await.unwrap_err().is_cancelled());
    first_release.open();
    tokio::time::timeout(Duration::from_secs(2), async {
        while fixture.coordinator.observation().unwrap().active_work != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(fixture.budget.observation().used.running_jobs, 1);
    assert!(!second_task.is_finished());
    second_release.open();
    second_task.await.unwrap().unwrap();
    await_idle(&fixture).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wp79_native_repartition_executes_through_the_installed_tracer() {
    let fixture = fixture(2);
    let permit = fixture
        .coordinator
        .admit(request(
            EpochWorkClass::InteractiveQuery,
            Cancellation::default(),
        ))
        .await
        .unwrap();
    let context =
        SessionContext::new_with_config_rt(SessionConfig::new(), Arc::clone(&fixture.runtime));
    let batch =
        RecordBatch::try_new(schema(), vec![Arc::new(Int64Array::from(vec![1, 2, 3, 4]))]).unwrap();
    let rows = permit
        .run(async {
            // Exercise the concrete native operator; an optimizer may validly remove a
            // logical round-robin request for a single tiny in-memory input.
            let input = MemorySourceConfig::try_new_exec(&[vec![batch]], schema(), None).unwrap();
            let repartition =
                RepartitionExec::try_new(input, Partitioning::RoundRobinBatch(2)).unwrap();
            let result = collect(Arc::new(repartition), context.task_ctx())
                .await
                .unwrap();
            result.iter().map(RecordBatch::num_rows).sum::<usize>()
        })
        .await
        .unwrap();
    assert_eq!(rows, 4);
    let observed = fixture.coordinator.observation().unwrap().native_operations;
    assert!(
        observed.completed_tasks >= 2,
        "native repartition must causally invoke tracer"
    );
    drop(permit);
    await_idle(&fixture).await;
}

#[tokio::test]
async fn wp79_native_bookkeeping_is_admitted_before_a_native_operation_exists() {
    use crate::resource_budget::ResourceClass;

    let fixture = fixture(2);
    let policy = fixture.budget.policy();
    let pressure = fixture
        .budget
        .try_reserve(
            ResourceClass::Data,
            ResourceAmounts {
                memory_bytes: policy.limits.memory_bytes
                    - policy.control_reserve.memory_bytes
                    - 4096,
                ..ResourceAmounts::default()
            },
        )
        .unwrap();
    assert!(
        tokio::time::timeout(
            Duration::from_millis(20),
            fixture.coordinator.admit(request(
                EpochWorkClass::InteractiveQuery,
                Cancellation::default()
            )),
        )
        .await
        .is_err()
    );
    assert_eq!(fixture.coordinator.observation().unwrap().active_work, 0);
    assert_eq!(
        fixture.coordinator.observation().unwrap().native_operations,
        NativeOperationObservation::default()
    );
    let control = fixture
        .coordinator
        .admit(request(
            EpochWorkClass::SecurityRecovery,
            Cancellation::default(),
        ))
        .await
        .unwrap();
    assert_eq!(
        control.run(async { "responsive" }).await.unwrap(),
        "responsive"
    );
    drop(control);
    drop(pressure);
    await_idle(&fixture).await;
    assert_eq!(fixture.budget.observation().used.memory_bytes, 0);
}
