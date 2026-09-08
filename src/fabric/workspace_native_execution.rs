//! Workspace-owned composition of native execution, local storage, and joined cleanup.
//!
//! Callers supply one complete native phase as a factory. Long-lived daemon actors remain
//! on the host runtime. The wrapper owns only execution and resource lifetime; table versions,
//! semantic authority, transaction reconciliation, and release policy remain with callers.

use std::fmt::Display;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::cancellation::{Cancellation, StructuredCancellationScope};
use crate::resource_budget::{
    ResourceAmounts, ResourceBudget, ResourceBudgetError, ResourceClass, ResourceReservation,
    ResourceScopeKind,
};

use super::native_execution_lane::{
    NativeAdmissionFailure, NativeExecutionLane, NativeLaneAdmission, NativeLaneCleanup,
    NativeLaneEnvelope, NativeLaneError, NativeLaneOutput,
};
use super::owned_local_store::{OwnedLocalMutation, OwnedLocalStore, OwnedLocalStoreError};

#[derive(Clone)]
pub(crate) struct WorkspaceNativeExecution {
    owner: Arc<WorkspaceNativeOwner>,
}

struct WorkspaceNativeOwner {
    budget: ResourceBudget,
    data_scope: StructuredCancellationScope,
    control_scope: StructuredCancellationScope,
    store: Arc<OwnedLocalStore>,
    data_lane: NativeExecutionLane,
    control_lane: NativeExecutionLane,
    next_operation: AtomicU64,
    _reservation: ResourceReservation,
}

struct WorkspaceNativeRequest<'a> {
    name: &'a str,
    class: ResourceClass,
    deadline: Instant,
    mutation: bool,
}

impl WorkspaceNativeExecution {
    /// Bind an existing lifecycle scope to the exact workspace/store owner. A structured
    /// scope does not itself carry a budget identity; this constructor establishes that binding.
    /// Both profiles are explicit admission policies, never guessed optimal defaults.
    pub(crate) fn try_new(
        budget: ResourceBudget,
        scope: &StructuredCancellationScope,
        store: Arc<OwnedLocalStore>,
        data_profile: NativeLaneEnvelope,
        control_profile: NativeLaneEnvelope,
    ) -> Result<Self, NativeLaneError> {
        if budget.owner().kind != ResourceScopeKind::Workspace || !budget.same_scope(store.budget())
        {
            return Err(ResourceBudgetError::ForeignOwner.into());
        }
        let data_lane = NativeExecutionLane::try_new(data_profile)?;
        let control_lane = NativeExecutionLane::try_new(control_profile)?;
        let reservation = budget.try_reserve(
            ResourceClass::Data,
            ResourceAmounts {
                memory_bytes: u64::try_from(
                    std::mem::size_of::<WorkspaceNativeOwner>() + 2 * 4096 + 64,
                )
                .map_err(|_| ResourceBudgetError::Overflow)?,
                ..ResourceAmounts::default()
            },
        )?;
        let data_scope = scope.child("native-data")?;
        let control_scope = scope.child_control("native-control")?;
        Ok(Self {
            owner: Arc::new(WorkspaceNativeOwner {
                budget,
                data_scope,
                control_scope,
                store,
                data_lane,
                control_lane,
                next_operation: AtomicU64::new(0),
                _reservation: reservation,
            }),
        })
    }

    pub(crate) async fn run_read<T, E, F, O>(
        &self,
        name: &str,
        class: ResourceClass,
        deadline: Instant,
        operation: O,
    ) -> Result<T, NativeLaneError>
    where
        T: NativeLaneOutput,
        E: Display,
        F: Future<Output = Result<T, E>>,
        O: FnOnce(Cancellation, Arc<OwnedLocalStore>) -> F + Send + 'static,
    {
        self.run_read_classified(name, class, deadline, operation, |error| {
            NativeLaneError::operation(&error)
        })
        .await
    }

    /// The classifier preserves typed application pressure/failure outcomes. Use it when an
    /// application error enum wraps a budget, task-capacity, or native-admission error; callers
    /// must not infer those categories from the diagnostic-only convenience method's text.
    pub(crate) async fn run_read_classified<T, E, F, O, C>(
        &self,
        name: &str,
        class: ResourceClass,
        deadline: Instant,
        operation: O,
        classify: C,
    ) -> Result<T, NativeLaneError>
    where
        T: NativeLaneOutput,
        F: Future<Output = Result<T, E>>,
        O: FnOnce(Cancellation, Arc<OwnedLocalStore>) -> F + Send + 'static,
        C: FnOnce(E) -> NativeLaneError + Send + 'static,
    {
        self.run(
            WorkspaceNativeRequest {
                name,
                class,
                deadline,
                mutation: false,
            },
            operation,
            classify,
        )
        .await
    }

    pub(crate) async fn run_mutation<T, E, F, O>(
        &self,
        name: &str,
        class: ResourceClass,
        deadline: Instant,
        operation: O,
    ) -> Result<T, NativeLaneError>
    where
        T: NativeLaneOutput,
        E: Display,
        F: Future<Output = Result<T, E>>,
        O: FnOnce(Cancellation, Arc<OwnedLocalStore>) -> F + Send + 'static,
    {
        self.run_mutation_classified(name, class, deadline, operation, |error| {
            NativeLaneError::operation(&error)
        })
        .await
    }

    pub(crate) async fn run_mutation_classified<T, E, F, O, C>(
        &self,
        name: &str,
        class: ResourceClass,
        deadline: Instant,
        operation: O,
        classify: C,
    ) -> Result<T, NativeLaneError>
    where
        T: NativeLaneOutput,
        F: Future<Output = Result<T, E>>,
        O: FnOnce(Cancellation, Arc<OwnedLocalStore>) -> F + Send + 'static,
        C: FnOnce(E) -> NativeLaneError + Send + 'static,
    {
        self.run(
            WorkspaceNativeRequest {
                name,
                class,
                deadline,
                mutation: true,
            },
            operation,
            classify,
        )
        .await
    }

    async fn run<T, E, F, O, C>(
        &self,
        request: WorkspaceNativeRequest<'_>,
        operation: O,
        classify: C,
    ) -> Result<T, NativeLaneError>
    where
        T: NativeLaneOutput,
        F: Future<Output = Result<T, E>>,
        O: FnOnce(Cancellation, Arc<OwnedLocalStore>) -> F + Send + 'static,
        C: FnOnce(E) -> NativeLaneError + Send + 'static,
    {
        let (parent, lane) = match request.class {
            ResourceClass::Data => (&self.owner.data_scope, &self.owner.data_lane),
            ResourceClass::Control => (&self.owner.control_scope, &self.owner.control_lane),
        };
        // Validate the descriptive name before allocating an operation identity. Distinct
        // calls can safely share the description while their registry keys stay unique.
        let named = parent.child(request.name)?;
        let id = self
            .owner
            .next_operation
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| NativeAdmissionFailure::NativeLimit("workspace operation identities"))?;
        let scope = named.child(&format!("operation-{id}"))?;
        let lease = Arc::new(Mutex::new(None::<OwnedLocalMutation>));
        let begin_lease = Arc::clone(&lease);
        let cleanup_lease = Arc::clone(&lease);
        let operation_store = Arc::clone(&self.owner.store);
        let reconcile_store = Arc::clone(&self.owner.store);
        lane.spawn_classified(
            NativeLaneAdmission {
                scope: &scope,
                name: "native",
                budget: &self.owner.budget,
                class: request.class,
                deadline: request.deadline,
            },
            move |cancellation| async move {
                if request.mutation {
                    let mutation = operation_store
                        .begin_mutation()
                        .map_err(Self::store_admission_failure)?;
                    *begin_lease
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(mutation);
                }
                operation(cancellation, operation_store)
                    .await
                    .map_err(classify)
            },
            NativeLaneCleanup {
                before_join: move || async move {
                    let mutation = cleanup_lease
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .clone();
                    if let Some(mutation) = mutation {
                        mutation.drain_cleanup().await?;
                    }
                    Ok::<(), OwnedLocalStoreError>(())
                },
                after_join: move |joined| {
                    let mutation = lease
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .clone();
                    // Release joined read tickets even when the user callback panicked, the
                    // deadline expired before mutation admission, or a census retry is needed.
                    reconcile_store.reconcile_after_join(joined)?;
                    if let Some(mutation) = mutation {
                        mutation.reconcile_after_join(joined)?;
                    }
                    // Store persists the token before fallible census. An explicit later
                    // control operation may call retry_after_join_reconciliation on failure.
                    Ok::<(), OwnedLocalStoreError>(())
                },
            },
        )
        .await?
        .wait()
        .await?
    }

    fn store_admission_failure(error: OwnedLocalStoreError) -> NativeLaneError {
        match error {
            OwnedLocalStoreError::Budget(error) => error.into(),
            OwnedLocalStoreError::ForeignOwner => ResourceBudgetError::ForeignOwner.into(),
            OwnedLocalStoreError::MutationBusy => NativeAdmissionFailure::MutationBusy.into(),
            OwnedLocalStoreError::CensusUnavailable => {
                NativeAdmissionFailure::StoreNotReconciled.into()
            }
            OwnedLocalStoreError::Bound(bound) => NativeAdmissionFailure::NativeLimit(bound).into(),
            error => NativeLaneError::operation(&error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::future::{pending, ready};
    use std::num::{NonZeroU64, NonZeroUsize};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use object_store::path::Path;
    use object_store::{ObjectStore, ObjectStoreExt, PutMultipartOptions, PutOptions};
    use tempfile::TempDir;

    use super::*;
    use crate::cancellation::StructuredTaskError;
    use crate::disk_headroom::LocalDiskHeadroom;
    use crate::fabric::owned_local_store::OwnedLocalStoreLimits;
    use crate::resource_budget::ResourceBudgetPolicy;

    fn budget() -> ResourceBudget {
        let policy = ResourceBudgetPolicy {
            limits: ResourceAmounts {
                memory_bytes: 512 * 1024 * 1024,
                disk_bytes: 16 * 1024 * 1024,
                running_jobs: 4,
                queued_jobs: 32,
                retained_generations: 4,
                retained_bytes: 1024 * 1024,
                rows: 4096,
                pages: 128,
            },
            control_reserve: ResourceAmounts {
                memory_bytes: 64 * 1024 * 1024,
                disk_bytes: 1024 * 1024,
                running_jobs: 1,
                queued_jobs: 16,
                ..ResourceAmounts::default()
            },
        };
        ResourceBudget::try_process([84; 16], policy)
            .unwrap()
            .workspace([85; 16], policy)
            .unwrap()
    }

    // Test-only geometry for tiny local objects. It is not a production or measured profile.
    fn profile() -> NativeLaneEnvelope {
        NativeLaneEnvelope {
            worker_threads: NonZeroUsize::new(2).unwrap(),
            blocking_threads: NonZeroUsize::new(8).unwrap(),
            thread_stack_bytes: NonZeroUsize::new(2 * 1024 * 1024).unwrap(),
            runtime_memory_bytes: NonZeroU64::new(1024 * 1024).unwrap(),
            native_buffer_bytes: 1024 * 1024,
            native_task_slots: NonZeroU64::new(4096).unwrap(),
            task_memory_bytes: NonZeroU64::new(512).unwrap(),
            parallel_blocking_roots: NonZeroUsize::new(2).unwrap(),
            blocking_nesting: NonZeroUsize::new(2).unwrap(),
        }
    }

    struct Fixture {
        _directory: TempDir,
        budget: ResourceBudget,
        scope: StructuredCancellationScope,
        store: Arc<OwnedLocalStore>,
        executor: WorkspaceNativeExecution,
        data: std::path::PathBuf,
        control: std::path::PathBuf,
    }
    impl Fixture {
        fn new(data_tasks: usize) -> Self {
            let directory = tempfile::tempdir().unwrap();
            let data = directory.path().join("data");
            let control = directory.path().join("control");
            std::fs::create_dir(&data).unwrap();
            std::fs::create_dir(&control).unwrap();
            std::fs::write(data.join("initial"), b"old").unwrap();
            std::fs::write(control.join("status"), b"live").unwrap();
            let budget = budget();
            let scope = StructuredCancellationScope::try_root_with_control_reserve(
                "workspace-native-test",
                NonZeroUsize::new(data_tasks).unwrap(),
                NonZeroUsize::new(2).unwrap(),
            )
            .unwrap();
            let store = Arc::new(
                OwnedLocalStore::try_new(
                    budget.clone(),
                    LocalDiskHeadroom::open(directory.path()).unwrap(),
                    OwnedLocalStoreLimits {
                        max_roots: 2,
                        max_object_bytes: 4096,
                        max_read_bytes: 4096,
                        max_path_bytes: 512,
                        max_list_entries: 64,
                        max_directory_depth: 4,
                        max_pending_operations: 16,
                        max_multipart_parts: 8,
                    },
                )
                .unwrap(),
            );
            for (path, class) in [
                (&data, ResourceClass::Data),
                (&control, ResourceClass::Control),
            ] {
                store
                    .register_root(&url::Url::from_directory_path(path).unwrap(), class)
                    .unwrap();
            }
            let executor = WorkspaceNativeExecution::try_new(
                budget.clone(),
                &scope,
                store.clone(),
                profile(),
                profile(),
            )
            .unwrap();
            Self {
                _directory: directory,
                budget,
                scope,
                store,
                executor,
                data,
                control,
            }
        }

        fn location(&self, name: &str) -> Path {
            Path::from_absolute_path(self.data.join(name)).unwrap()
        }
    }

    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(10)
    }

    #[test]
    fn workspace_native_rejects_foreign_owner_and_does_not_charge_native_tasks_as_queue_entries() {
        let fixture = Fixture::new(4);
        assert!(matches!(
            WorkspaceNativeExecution::try_new(
                budget(),
                &fixture.scope,
                fixture.store.clone(),
                profile(),
                profile()
            ),
            Err(NativeLaneError::Budget(ResourceBudgetError::ForeignOwner))
        ));
        assert_eq!(
            fixture
                .executor
                .owner
                .data_lane
                .admitted_capacity()
                .queued_jobs,
            0
        );
        assert_eq!(
            fixture
                .executor
                .owner
                .control_lane
                .admitted_capacity()
                .queued_jobs,
            0
        );
        assert_eq!(
            fixture
                .executor
                .owner
                .data_lane
                .admitted_capacity()
                .running_jobs,
            1
        );
        assert!(
            fixture
                .executor
                .owner
                .data_lane
                .admitted_capacity()
                .memory_bytes
                >= 4096 * 512
        );
    }

    async fn read_while_rejecting_mutation(fixture: &Fixture) -> u64 {
        let source = fixture.location("initial");
        let forbidden = fixture.location("reader-write");
        fixture
            .executor
            .run_read(
                "read",
                ResourceClass::Data,
                deadline(),
                move |_, store| async move {
                    let bytes = store.get(&source).await?.bytes().await?;
                    assert_eq!(bytes.as_ref(), b"old");
                    let error = store
                        .put_opts(
                            &forbidden,
                            b"invalid".to_vec().into(),
                            PutOptions::default(),
                        )
                        .await
                        .unwrap_err();
                    let object_store::Error::Generic { source, .. } = error else {
                        panic!("untyped ownership failure")
                    };
                    assert!(matches!(
                        source.downcast_ref::<OwnedLocalStoreError>(),
                        Some(OwnedLocalStoreError::MutationRuntime)
                    ));
                    Ok::<u64, object_store::Error>(u64::try_from(bytes.len()).unwrap())
                },
            )
            .await
            .unwrap()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn workspace_native_readers_cannot_borrow_writer_and_control_survives_data_pressure() {
        let fixture = Fixture::new(4);
        let writer = fixture.executor.clone();
        let target = fixture.location("writer");
        let (started, running) = tokio::sync::oneshot::channel();
        let (release, released) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            writer
                .run_mutation(
                    "write",
                    ResourceClass::Data,
                    deadline(),
                    move |_, store| async move {
                        store
                            .put_opts(&target, b"new".to_vec().into(), PutOptions::default())
                            .await?;
                        started.send(()).unwrap();
                        let _ = released.await;
                        Ok::<(), object_store::Error>(())
                    },
                )
                .await
        });
        running.await.unwrap();
        let read = read_while_rejecting_mutation(&fixture).await;
        assert_eq!(read, 3);
        let invoked = Arc::new(AtomicBool::new(false));
        let operation_invoked = invoked.clone();
        let busy = fixture
            .executor
            .run_mutation("write", ResourceClass::Data, deadline(), move |_, _| {
                operation_invoked.store(true, Ordering::Release);
                ready(Ok::<(), &'static str>(()))
            })
            .await
            .unwrap_err();
        assert!(matches!(
            busy,
            NativeLaneError::AdmissionDenied(NativeAdmissionFailure::MutationBusy)
        ));
        assert!(!invoked.load(Ordering::Acquire));
        assert_eq!(fixture.budget.observation().used.running_jobs, 1);

        let policy = fixture.budget.policy();
        let remaining =
            u128::from(policy.limits.memory_bytes - policy.control_reserve.memory_bytes)
                - fixture.budget.observation().data_used.memory_bytes;
        let pressure = fixture
            .budget
            .try_reserve(
                ResourceClass::Data,
                ResourceAmounts {
                    memory_bytes: u64::try_from(remaining).unwrap(),
                    ..ResourceAmounts::default()
                },
            )
            .unwrap();
        let denied = fixture
            .executor
            .run_read("full", ResourceClass::Data, deadline(), |_, _| {
                ready(Ok::<(), &'static str>(()))
            })
            .await
            .unwrap_err();
        assert!(matches!(denied, NativeLaneError::Budget(_)));
        let status = Path::from_absolute_path(fixture.control.join("status")).unwrap();
        let count = fixture
            .executor
            .run_read(
                "status",
                ResourceClass::Control,
                deadline(),
                move |_, store| async move {
                    let bytes = store.get(&status).await?.bytes().await?;
                    Ok::<u64, object_store::Error>(u64::try_from(bytes.len()).unwrap())
                },
            )
            .await
            .unwrap();
        assert_eq!(count, 4);
        drop(pressure);
        release.send(()).unwrap();
        task.await.unwrap().unwrap();
        assert_eq!(fixture.budget.observation().used.running_jobs, 0);
        assert!(!fixture.data.join("reader-write").exists());
        let observed = fixture.store.observe().unwrap();
        assert!(!observed.mutation_active);
        assert_eq!(observed.pending_read_operations, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn workspace_native_cancellation_aborts_orphan_upload_and_releases_writer_after_join() {
        let fixture = Fixture::new(4);
        let executor = fixture.executor.clone();
        let location = fixture.location("cancelled");
        let (started, cancellation) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            executor
                .run_mutation(
                    "multipart",
                    ResourceClass::Data,
                    deadline(),
                    move |cancel, store| async move {
                        let mut upload = store
                            .put_multipart_opts(&location, PutMultipartOptions::default())
                            .await?;
                        upload.put_part(b"part".to_vec().into()).await?;
                        started.send(cancel).unwrap();
                        pending::<object_store::Result<()>>().await
                    },
                )
                .await
        });
        cancellation.await.unwrap().cancel();
        assert!(matches!(
            task.await.unwrap(),
            Err(NativeLaneError::Cancelled)
        ));
        let observed = fixture.store.observe().unwrap();
        assert!(!observed.mutation_active);
        assert_eq!(observed.pending_uploads, 0);
        assert_eq!(observed.pending_read_operations, 0);
        assert!(!fixture.data.join("cancelled").exists());
        assert_eq!(fixture.budget.observation().used.running_jobs, 0);
        let target = fixture.location("after-cancel");
        fixture
            .executor
            .run_mutation(
                "multipart",
                ResourceClass::Data,
                deadline(),
                move |_, store| async move {
                    store
                        .put_opts(&target, b"complete".to_vec().into(), PutOptions::default())
                        .await?;
                    Ok::<(), object_store::Error>(())
                },
            )
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(fixture.data.join("after-cancel")).unwrap(),
            b"complete"
        );
    }

    #[derive(Debug)]
    struct OpaqueNativeFailure {
        runtime: tokio::runtime::Handle,
        _payload: Vec<u8>,
        dropped: Arc<AtomicBool>,
    }
    impl Display for OpaqueNativeFailure {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("opaque native failure")
        }
    }
    impl std::error::Error for OpaqueNativeFailure {}
    impl Drop for OpaqueNativeFailure {
        fn drop(&mut self) {
            assert_eq!(tokio::runtime::Handle::current().id(), self.runtime.id());
            self.dropped.store(true, Ordering::Release);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn workspace_native_classifier_cannot_export_opaque_runtime_payload() {
        let fixture = Fixture::new(4);
        let dropped = Arc::new(AtomicBool::new(false));
        let payload_dropped = dropped.clone();
        let error = fixture
            .executor
            .run_read_classified(
                "opaque-error",
                ResourceClass::Data,
                deadline(),
                |_, _| ready(Err::<(), _>(())),
                move |()| {
                    NativeLaneError::Runtime(std::io::Error::other(OpaqueNativeFailure {
                        runtime: tokio::runtime::Handle::current(),
                        _payload: vec![0; 256 * 1024],
                        dropped: payload_dropped,
                    }))
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(error, NativeLaneError::NativeFailure {
            category: crate::fabric::native_execution_lane::NativeDiagnosticCategory::RuntimeConstruction, ..
        }));
        assert!(dropped.load(Ordering::Acquire));
        assert_eq!(fixture.budget.observation().used.running_jobs, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn workspace_native_classifier_bounds_messages_and_flattens_cleanup_chains() {
        let fixture = Fixture::new(4);
        let error = fixture
            .executor
            .run_read_classified(
                "large-error",
                ResourceClass::Data,
                deadline(),
                |_, _| ready(Err::<(), _>(())),
                |()| NativeLaneError::Operation("x".repeat(64 * 1024)),
            )
            .await
            .unwrap_err();
        let NativeLaneError::Operation(message) = error else {
            panic!("wrong error category")
        };
        assert_eq!(message.len(), 4096);
        assert_eq!(message.capacity(), 4096);
        let error = fixture
            .executor
            .run_read_classified(
                "nested-error",
                ResourceClass::Data,
                deadline(),
                |_, _| ready(Err::<(), _>(())),
                |()| {
                    let mut error = NativeLaneError::Budget(ResourceBudgetError::ForeignOwner);
                    for _ in 0..256 {
                        error = NativeLaneError::Cleanup {
                            before_join: Some("nested".into()),
                            after_join: None,
                            operation: Some(Box::new(error)),
                        };
                    }
                    NativeLaneError::Cleanup {
                        before_join: Some("b".repeat(16 * 1024)),
                        after_join: Some("a".repeat(16 * 1024)),
                        operation: Some(Box::new(error)),
                    }
                },
            )
            .await
            .unwrap_err();
        let NativeLaneError::Cleanup {
            before_join,
            after_join,
            operation,
        } = error
        else {
            panic!("wrong error category")
        };
        assert_eq!(before_join.unwrap().len(), 4096);
        assert!(after_join.unwrap().len() <= 4096);
        assert!(matches!(
            operation.as_deref(),
            Some(NativeLaneError::Budget(ResourceBudgetError::ForeignOwner))
        ));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn workspace_native_cleanup_failure_preserves_typed_rejection_and_joined_retry() {
        let fixture = Fixture::new(4);
        let original = fixture.data.clone();
        let moved = fixture.data.with_file_name("moved-data");
        let target = moved.clone();
        let error = fixture
            .executor
            .run_mutation_classified(
                "rejection-and-census-failure",
                ResourceClass::Data,
                deadline(),
                move |_, _| async move {
                    // External path replacement is a fault injected after mutation admission.
                    std::fs::rename(original, target).unwrap();
                    Err::<(), _>(ResourceBudgetError::ForeignOwner)
                },
                NativeLaneError::Budget,
            )
            .await
            .unwrap_err();
        let NativeLaneError::Cleanup {
            before_join,
            after_join,
            operation,
        } = error
        else {
            panic!("wrong error category")
        };
        assert!(before_join.is_none());
        assert!(after_join.is_some());
        assert!(matches!(
            operation.as_deref(),
            Some(NativeLaneError::Budget(ResourceBudgetError::ForeignOwner))
        ));
        assert!(fixture.store.observe().unwrap().mutation_active);
        std::fs::rename(moved, &fixture.data).unwrap();
        fixture
            .executor
            .run_read(
                "joined-retry",
                ResourceClass::Control,
                deadline(),
                |_, store| ready(store.retry_after_join_reconciliation()),
            )
            .await
            .unwrap();
        assert!(!fixture.store.observe().unwrap().mutation_active);
        assert_eq!(fixture.budget.observation().used.running_jobs, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn workspace_native_registry_pressure_and_classified_budget_errors_remain_typed() {
        let fixture = Fixture::new(1);
        let executor = fixture.executor.clone();
        let source = fixture.location("initial");
        let (started, running) = tokio::sync::oneshot::channel();
        let (release, released) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            executor
                .run_read(
                    "same-description",
                    ResourceClass::Data,
                    deadline(),
                    move |_, store| async move {
                        let _bytes = store.get(&source).await?.bytes().await?;
                        started.send(()).unwrap();
                        let _ = released.await;
                        Ok::<(), object_store::Error>(())
                    },
                )
                .await
        });
        running.await.unwrap();
        let error = fixture
            .executor
            .run_read(
                "same-description",
                ResourceClass::Data,
                deadline(),
                |_, _| ready(Ok::<(), &'static str>(())),
            )
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            NativeLaneError::Registry(StructuredTaskError::TaskCapacity { maximum: 1 })
        ));
        release.send(()).unwrap();
        task.await.unwrap().unwrap();
        let error = fixture
            .executor
            .run_read_classified(
                "typed",
                ResourceClass::Data,
                deadline(),
                |_, _| ready(Err::<(), _>(ResourceBudgetError::ForeignOwner)),
                NativeLaneError::Budget,
            )
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            NativeLaneError::Budget(ResourceBudgetError::ForeignOwner)
        ));
        assert_eq!(fixture.budget.observation().used.running_jobs, 0);
    }
}
