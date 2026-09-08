use std::sync::atomic::AtomicUsize;
use std::sync::mpsc;

use super::*;

struct Guard(Arc<AtomicUsize>);

impl Drop for Guard {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn root() -> StructuredCancellationScope {
    StructuredCancellationScope::try_root_with_control_reserve(
        "daemon",
        NonZeroUsize::new(1).unwrap(),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn rt_cpg_wp79_operations() {
    use crate::resource_budget::{ResourceAmounts, ResourceClass};
    let budget = crate::fabric::workspace_resources::test_workspace_budget();
    let policy = budget.policy();
    let data = budget
        .try_reserve(
            ResourceClass::Data,
            ResourceAmounts {
                memory_bytes: policy.limits.memory_bytes - policy.control_reserve.memory_bytes,
                running_jobs: 1,
                ..ResourceAmounts::default()
            },
        )
        .unwrap();
    let root = root();
    let data_scope = root.child("native").unwrap();
    let (started, ready) = oneshot::channel();
    let native = data_scope
        .spawn_blocking_owned("bounded-poll", data, move |cancellation| {
            let _ = started.send(());
            while !cancellation.is_cancelled() {
                std::thread::yield_now();
            }
            true
        })
        .await
        .unwrap();
    ready.await.unwrap();
    assert_eq!(
        root.process_operation_observation().await.owned_data_tasks,
        1
    );
    let control_guard = budget
        .try_reserve(
            ResourceClass::Control,
            ResourceAmounts {
                memory_bytes: 1024,
                running_jobs: 1,
                ..ResourceAmounts::default()
            },
        )
        .unwrap();
    let control = root
        .child_control("status")
        .unwrap()
        .spawn_async_owned(
            "observe",
            TaskCancellationMode::AbortableAsync,
            control_guard,
            async { 7_u8 },
        )
        .await
        .unwrap();
    assert_eq!(
        control.wait().await.unwrap(),
        7,
        "control runs under full data memory admission"
    );
    assert_eq!(
        budget.observation().used.memory_bytes,
        u128::from(policy.limits.memory_bytes - policy.control_reserve.memory_bytes)
    );
    root.cancel();
    assert!(native.wait().await.unwrap());
    root.cancel_and_join(Duration::from_secs(1)).await.unwrap();
    let observed = root.process_operation_observation().await;
    assert_eq!(observed.owned_data_tasks + observed.owned_control_tasks, 0);
    assert_eq!(observed.joined_tasks, 2);
    assert_eq!(observed.cancelled_tasks_joined, 1);
    assert!(observed.last_cancellation_to_join_millis.is_some());
    assert_eq!(budget.observation().used.memory_bytes, 0);
    assert_eq!(budget.observation().used.running_jobs, 0);
}

async fn blocking(
    scope: &StructuredCancellationScope,
    drops: Arc<AtomicUsize>,
) -> (TaskObservation<u8>, mpsc::Sender<()>, Arc<AtomicBool>) {
    let (started, ready) = oneshot::channel();
    let (finish, gate) = mpsc::channel();
    let cancelled = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&cancelled);
    let observation = scope
        .spawn_blocking_owned("native", Guard(drops), move |probe| {
            let _ = started.send(());
            loop {
                if probe.interrupt_flag().load(Ordering::Acquire) {
                    seen.store(true, Ordering::Release);
                }
                match gate.recv_timeout(Duration::from_millis(1)) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
            17
        })
        .await
        .unwrap();
    ready.await.unwrap();
    (observation, finish, cancelled)
}

#[tokio::test]
async fn wp79_owned_parent_cancel_reaches_live_native_atomic_polling_without_relay_task() {
    let root = root();
    let leaf = root.child("workspace").unwrap().child("provider").unwrap();
    let drops = Arc::new(AtomicUsize::new(0));
    let (observation, finish, cancelled) = blocking(&leaf, Arc::clone(&drops)).await;
    root.cancel();
    tokio::time::timeout(Duration::from_secs(1), async {
        while !cancelled.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(root.live_task_count().await.unwrap(), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(root.native_flags.lock().unwrap().len(), 1);
    finish.send(()).unwrap();
    assert_eq!(observation.wait().await.unwrap(), 17);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(root.native_flags.lock().unwrap().is_empty());
}

#[tokio::test]
async fn wp79_owned_cancelled_join_preserves_live_native_work_and_control_capacity() {
    let root = root();
    let work = root.child("provider").unwrap();
    let drops = Arc::new(AtomicUsize::new(0));
    let (observation, finish, cancelled) = blocking(&work, Arc::clone(&drops)).await;
    assert!(
        tokio::time::timeout(
            Duration::from_millis(20),
            work.cancel_and_join(Duration::from_secs(2))
        )
        .await
        .is_err()
    );
    assert!(cancelled.load(Ordering::Acquire));
    assert_eq!(work.live_task_count().await.unwrap(), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert!(matches!(
        work.spawn("closed", async {}).await,
        Err(StructuredTaskError::ScopeClosed(_))
    ));
    let sibling = root.child("other-provider").unwrap();
    assert!(matches!(
        sibling.spawn("full", async {}).await,
        Err(StructuredTaskError::TaskCapacity { maximum: 1 })
    ));
    let control = root.child_control("status").unwrap();
    let status = tokio::time::timeout(Duration::from_millis(200), async {
        control
            .spawn_observed("responsive", async { 29 })
            .await
            .unwrap()
            .wait()
            .await
            .unwrap()
    })
    .await
    .unwrap();
    assert_eq!(status, 29);
    finish.send(()).unwrap();
    assert_eq!(observation.wait().await.unwrap(), 17);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    sibling
        .spawn_observed("reused", async { 31 })
        .await
        .unwrap()
        .wait()
        .await
        .unwrap();
    root.cancel_and_join(Duration::from_secs(1)).await.unwrap();
}

#[tokio::test]
async fn wp79_owned_timeout_keeps_cooperative_reservation_until_a_later_join() {
    for reserve in [Duration::ZERO, Duration::from_millis(1)] {
        let root = root();
        let work = root.child("provider").unwrap();
        let drops = Arc::new(AtomicUsize::new(0));
        let (observation, finish, _) = blocking(&work, Arc::clone(&drops)).await;
        assert!(matches!(
            work.cancel_and_join(reserve).await,
            Err(StructuredTaskError::CleanupReserveExhausted { .. })
        ));
        assert!(work.is_cancelled());
        assert_eq!(work.live_task_count().await.unwrap(), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        finish.send(()).unwrap();
        work.cancel_and_join(Duration::from_secs(1)).await.unwrap();
        work.cancel_and_join(Duration::from_secs(1)).await.unwrap();
        assert_eq!(observation.wait().await.unwrap(), 17);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn wp79_owned_duplicate_joins_cache_panic_and_release_once() {
    let root = root();
    let work = root.child("provider").unwrap();
    let drops = Arc::new(AtomicUsize::new(0));
    let (release, gate) = oneshot::channel();
    let observation = work
        .spawn_async_owned(
            "panic",
            TaskCancellationMode::Cooperative,
            Guard(Arc::clone(&drops)),
            async move {
                let _ = gate.await;
                panic!("actual owned worker panic");
            },
        )
        .await
        .unwrap();
    let (left, right, ()) = tokio::join!(
        work.cancel_and_join(Duration::from_secs(1)),
        work.cancel_and_join(Duration::from_secs(1)),
        async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            release.send(()).unwrap();
        },
    );
    let (
        Err(StructuredTaskError::Join { source: left, .. }),
        Err(StructuredTaskError::Join { source: right, .. }),
    ) = (left, right)
    else {
        panic!("both concurrent joins must observe the same worker panic");
    };
    assert!(left.is_panic());
    assert!(Arc::ptr_eq(&left, &right));
    assert!(
        matches!(observation.wait().await, Err(StructuredTaskError::Join { source, .. }) if source.is_panic())
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(root.live_task_count().await.unwrap(), 0);
    root.spawn_observed("reused", async { 7 })
        .await
        .unwrap()
        .wait()
        .await
        .unwrap();
}

#[tokio::test]
async fn wp79_owned_abortable_async_is_joined_before_guard_release() {
    let root = root();
    let work = root.child("async").unwrap();
    let drops = Arc::new(AtomicUsize::new(0));
    let observation = work
        .spawn_async_owned(
            "pending",
            TaskCancellationMode::AbortableAsync,
            Guard(Arc::clone(&drops)),
            std::future::pending::<u8>(),
        )
        .await
        .unwrap();
    assert!(matches!(
        work.cancel_and_join(Duration::from_millis(1)).await,
        Err(StructuredTaskError::CleanupReserveExhausted { .. })
    ));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(work.live_task_count().await.unwrap(), 0);
    assert!(
        matches!(observation.wait().await, Err(StructuredTaskError::Join { source, .. }) if source.is_cancelled())
    );
}

#[tokio::test]
async fn wp79_owned_rejected_duplicate_and_capacity_never_start_native_work() {
    let root = root();
    let drops = Arc::new(AtomicUsize::new(0));
    let (observation, finish, _) = blocking(&root, Arc::clone(&drops)).await;
    let rejected = Arc::new(AtomicUsize::new(0));
    for (name, duplicate) in [("native", true), ("over-capacity", false)] {
        let result = root
            .spawn_blocking_owned(name, Guard(Arc::clone(&rejected)), |_| {
                panic!("rejected native work ran")
            })
            .await;
        assert!(matches!(
            (&result, duplicate),
            (Err(StructuredTaskError::DuplicateTask(_)), true)
                | (Err(StructuredTaskError::TaskCapacity { maximum: 1 }), false)
        ));
    }
    assert_eq!(rejected.load(Ordering::SeqCst), 2);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    finish.send(()).unwrap();
    observation.wait().await.unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    root.spawn_blocking_owned("native", (), |_| 33)
        .await
        .unwrap()
        .wait()
        .await
        .unwrap();
}

#[tokio::test]
async fn wp79_owned_finished_guard_waits_for_join_and_dropped_wait_is_safe() {
    let root = root();
    let drops = Arc::new(AtomicUsize::new(0));
    let observation = root
        .spawn_blocking_owned("completed", Guard(Arc::clone(&drops)), |_| 11)
        .await
        .unwrap();
    while !observation.entry.abort.is_finished() {
        tokio::task::yield_now().await;
    }
    assert_eq!(
        drops.load(Ordering::SeqCst),
        0,
        "actual completion is not yet observed termination"
    );
    assert_eq!(observation.wait().await.unwrap(), 11);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    let (observation, finish, _) = blocking(&root, Arc::clone(&drops)).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(5), observation.wait())
            .await
            .is_err()
    );
    assert_eq!(root.live_task_count().await.unwrap(), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    finish.send(()).unwrap();
    root.cancel_and_join(Duration::from_secs(1)).await.unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn wp79_owned_old_observer_cannot_remove_reused_task_name() {
    let root = root();
    let first = root.spawn_observed("same", async { 1 }).await.unwrap();
    while !first.entry.abort.is_finished() {
        tokio::task::yield_now().await;
    }
    root.reap_finished().await.unwrap();
    let (finish, gate) = oneshot::channel();
    let second = root
        .spawn_observed("same", async move {
            gate.await.unwrap();
            2
        })
        .await
        .unwrap();
    assert_eq!(first.wait().await.unwrap(), 1);
    assert_eq!(root.live_task_count().await.unwrap(), 1);
    finish.send(()).unwrap();
    assert_eq!(second.wait().await.unwrap(), 2);
}

#[tokio::test]
async fn wp79_owned_control_capacity_is_finite_and_scope_close_is_final() {
    let root = root();
    let control = root.child_control("control").unwrap();
    let (finish, gate) = oneshot::channel();
    let observation = control
        .spawn_observed("one", async move { gate.await.unwrap() })
        .await
        .unwrap();
    assert!(matches!(
        control.spawn("two", async {}).await,
        Err(StructuredTaskError::TaskCapacity { maximum: 1 })
    ));
    // Full control capacity does not consume the separately bounded heavy slot.
    root.spawn_observed("data", async {})
        .await
        .unwrap()
        .wait()
        .await
        .unwrap();
    finish.send(()).unwrap();
    observation.wait().await.unwrap();
    root.cancel_and_join(Duration::from_secs(1)).await.unwrap();
    assert!(matches!(
        root.child_control("new"),
        Err(StructuredTaskError::ScopeClosed(_))
    ));
    let legacy =
        StructuredCancellationScope::try_root("legacy", NonZeroUsize::new(1).unwrap()).unwrap();
    assert!(matches!(
        legacy.child_control("control"),
        Err(StructuredTaskError::ControlCapacityUnavailable)
    ));
}
