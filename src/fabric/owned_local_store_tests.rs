use std::future::Future;
use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use object_store::{CopyMode, PutMode, RenameTargetMode};
use tempfile::TempDir;

use super::*;
use crate::cancellation::StructuredCancellationScope;
use crate::fabric::native_execution_lane::{
    NativeExecutionLane, NativeLaneAdmission, NativeLaneCleanup, NativeLaneEnvelope,
    NativeLaneError, NativeLaneOutput,
};
use crate::resource_budget::ResourceBudgetPolicy;

fn budget(disk_bytes: u64) -> ResourceBudget {
    let policy = ResourceBudgetPolicy {
        limits: ResourceAmounts {
            memory_bytes: 512 * 1024 * 1024,
            disk_bytes,
            running_jobs: 8,
            queued_jobs: 1024,
            retained_generations: 16,
            retained_bytes: 1024 * 1024,
            rows: 1024,
            pages: 64,
        },
        control_reserve: ResourceAmounts {
            memory_bytes: 64 * 1024 * 1024,
            running_jobs: 2,
            queued_jobs: 128,
            ..ResourceAmounts::default()
        },
    };
    ResourceBudget::try_process([91; 16], policy)
        .unwrap()
        .workspace([92; 16], policy)
        .unwrap()
}

fn limits() -> OwnedLocalStoreLimits {
    OwnedLocalStoreLimits {
        max_roots: 2,
        max_object_bytes: 2 * 1024 * 1024,
        max_read_bytes: 2 * 1024 * 1024,
        max_path_bytes: 512,
        max_list_entries: 128,
        max_directory_depth: 8,
        max_pending_operations: 128,
        max_multipart_parts: 16,
    }
}

fn store(directory: &TempDir, budget: &ResourceBudget) -> OwnedLocalStore {
    let store = OwnedLocalStore::try_new(
        budget.clone(),
        LocalDiskHeadroom::open(directory.path()).unwrap(),
        limits(),
    )
    .unwrap();
    store
        .register_root(
            &Url::from_directory_path(directory.path()).unwrap(),
            ResourceClass::Data,
        )
        .unwrap();
    store
}

fn location(directory: &TempDir, name: &str) -> Path {
    Path::from_absolute_path(directory.path().join(name)).unwrap()
}

fn lane() -> NativeExecutionLane {
    NativeExecutionLane::try_new(NativeLaneEnvelope {
        worker_threads: NonZeroUsize::new(2).unwrap(),
        blocking_threads: NonZeroUsize::new(8).unwrap(),
        thread_stack_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        runtime_memory_bytes: NonZeroU64::new(1024 * 1024).unwrap(),
        native_buffer_bytes: 1024 * 1024,
        native_task_slots: NonZeroU64::new(32).unwrap(),
        task_memory_bytes: NonZeroU64::new(4096).unwrap(),
        parallel_blocking_roots: NonZeroUsize::new(2).unwrap(),
        blocking_nesting: NonZeroUsize::new(2).unwrap(),
    })
    .unwrap()
}

async fn run<T, O, F>(
    store: &OwnedLocalStore,
    budget: &ResourceBudget,
    mutation: bool,
    operation: O,
) -> Result<T, NativeLaneError>
where
    T: NativeLaneOutput,
    O: FnOnce(OwnedLocalStore) -> F + Send + 'static,
    F: Future<Output = object_store::Result<T>>,
{
    run_with_deadline(store, budget, mutation, Duration::from_secs(10), operation).await
}

async fn run_with_deadline<T, O, F>(
    store: &OwnedLocalStore,
    budget: &ResourceBudget,
    mutation: bool,
    duration: Duration,
    operation: O,
) -> Result<T, NativeLaneError>
where
    T: NativeLaneOutput,
    O: FnOnce(OwnedLocalStore) -> F + Send + 'static,
    F: Future<Output = object_store::Result<T>>,
{
    let scope = StructuredCancellationScope::try_root_with_control_reserve(
        "owned-local-tests",
        NonZeroUsize::new(4).unwrap(),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap();
    let lease = Arc::new(Mutex::new(None::<OwnedLocalMutation>));
    let begin_lease = Arc::clone(&lease);
    let cleanup_lease = Arc::clone(&lease);
    let operation_store = store.clone();
    let reconcile_store = store.clone();
    lane()
        .spawn(
            NativeLaneAdmission {
                scope: &scope,
                name: "native-local-store",
                budget,
                class: ResourceClass::Data,
                deadline: Some(Instant::now() + duration),
                cancellation_mode:
                    crate::fabric::native_execution_lane::NativeCancellationMode::DropFuture,
            },
            move |_| async move {
                if mutation {
                    *begin_lease.lock().unwrap() =
                        Some(operation_store.begin_mutation().map_err(object_error)?);
                }
                operation(operation_store).await
            },
            NativeLaneCleanup {
                before_join: move || async move {
                    let lease = cleanup_lease.lock().unwrap().clone();
                    if let Some(lease) = lease {
                        lease.drain_cleanup().await?;
                    }
                    Ok::<(), OwnedLocalStoreError>(())
                },
                after_join: move |joined| {
                    let lease = lease.lock().unwrap().take();
                    if let Some(lease) = lease {
                        lease.reconcile_after_join(joined)
                    } else {
                        reconcile_store.reconcile_after_join(joined)
                    }
                },
            },
        )
        .await?
        .wait()
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_local_restart_census_counts_abandoned_temps_and_native_empty_directories() {
    let directory = TempDir::new().unwrap();
    std::fs::write(directory.path().join("part.parquet#42"), vec![7; 8193]).unwrap();
    std::fs::create_dir(directory.path().join("empty")).unwrap();
    let resources = budget(8 * 1024 * 1024);
    let owner = store(&directory, &resources);
    let first = owner.observe().unwrap();
    assert_eq!(first.files, 1);
    assert_eq!(first.directories, 2);
    assert!(first.physical_bytes >= 8193);
    assert_eq!(first.reserved_disk_bytes, first.physical_bytes);
    let root = location(&directory, "");
    run(&owner, &resources, false, move |store| async move {
        let listing = store.list_with_delimiter(Some(&root)).await?;
        assert!(
            listing.objects.is_empty(),
            "native listing hides staging files"
        );
        assert_eq!(listing.common_prefixes, vec![root.join("empty")]);
        Ok(())
    })
    .await
    .unwrap();
    drop(owner);
    assert_eq!(resources.observation().used.disk_bytes, 0);
    let reopened = store(&directory, &resources);
    assert_eq!(
        reopened.observe().unwrap().physical_bytes,
        first.physical_bytes
    );
    assert_eq!(
        resources.observation().used.disk_bytes,
        u128::from(first.physical_bytes)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_local_preserves_native_create_copy_rename_conditions_and_overwrite() {
    let directory = TempDir::new().unwrap();
    let resources = budget(8 * 1024 * 1024);
    let owner = store(&directory, &resources);
    let first = location(&directory, "a");
    let second = location(&directory, "b");
    let third = location(&directory, "c");
    run(&owner, &resources, true, move |store| async move {
        store
            .put(&first, Bytes::from_static(b"original").into())
            .await?;
        let failure = store
            .put_opts(
                &first,
                Bytes::from_static(b"wrong").into(),
                PutOptions {
                    mode: PutMode::Create,
                    ..PutOptions::default()
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(failure, object_store::Error::AlreadyExists { .. }));
        store.copy(&first, &second).await?;
        assert!(matches!(
            store
                .copy_opts(
                    &first,
                    &second,
                    CopyOptions {
                        mode: CopyMode::Create,
                        ..CopyOptions::default()
                    }
                )
                .await,
            Err(object_store::Error::AlreadyExists { .. })
        ));
        assert!(matches!(
            store
                .rename_opts(
                    &first,
                    &second,
                    RenameOptions {
                        target_mode: RenameTargetMode::Create,
                        ..RenameOptions::default()
                    }
                )
                .await,
            Err(object_store::Error::AlreadyExists { .. })
        ));
        store.rename(&first, &third).await?;
        assert!(matches!(
            store.head(&first).await,
            Err(object_store::Error::NotFound { .. })
        ));
        assert_eq!(&store.get(&second).await?.bytes().await?[..], b"original");
        store
            .put(&third, Bytes::from_static(b"replacement").into())
            .await?;
        assert_eq!(&store.get(&third).await?.bytes().await?[..], b"replacement");
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(owner.observe().unwrap().files, 2);
    assert!(!owner.observe().unwrap().mutation_active);
    assert_eq!(
        owner.observe().unwrap().reserved_disk_bytes,
        owner.observe().unwrap().physical_bytes
    );
    assert_eq!(
        owner
            .inner
            .headroom
            .observe()
            .unwrap()
            .in_flight_growth_bytes,
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_local_disk_rejects_overwrite_before_native_mutation_and_delete_frees_capacity() {
    let directory = TempDir::new().unwrap();
    let unit = LocalDiskHeadroom::open(directory.path())
        .unwrap()
        .observe()
        .unwrap()
        .allocation_unit;
    let resources = budget(unit * 4);
    let owner = store(&directory, &resources);
    let target = location(&directory, "object");
    let path = target.clone();
    run(&owner, &resources, true, move |store| async move {
        store.put(&path, Bytes::from_static(b"kept").into()).await?;
        Ok(())
    })
    .await
    .unwrap();
    let path = target.clone();
    run(&owner, &resources, true, move |store| async move {
        assert!(
            store
                .put(&path, vec![9; usize::try_from(unit * 2).unwrap()].into())
                .await
                .is_err()
        );
        assert_eq!(&store.get(&path).await?.bytes().await?[..], b"kept");
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(owner.observe().unwrap().files, 1);
    run(&owner, &resources, true, move |store| async move {
        store.delete(&target).await?;
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(owner.observe().unwrap().files, 0);
    assert_eq!(owner.observe().unwrap().reserved_disk_bytes, unit);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_local_read_bytes_clone_slice_and_ranges_retain_workspace_charge_after_join() {
    let directory = TempDir::new().unwrap();
    std::fs::write(directory.path().join("object"), vec![3; 8192]).unwrap();
    let resources = budget(8 * 1024 * 1024);
    let owner = store(&directory, &resources);
    let baseline = resources.observation().used.memory_bytes;
    let retained = Arc::new(Mutex::new(Vec::<Bytes>::new()));
    let out = Arc::clone(&retained);
    let target = location(&directory, "object");
    run(&owner, &resources, false, move |store| async move {
        let result = store.get(&target).await?;
        assert!(matches!(&result.payload, GetResultPayload::Stream(_)));
        let bytes = result.bytes().await?;
        let slice = bytes.slice(100..200);
        let clone = bytes.clone();
        let ranges = store.get_ranges(&target, &[0..8, 4096..4104]).await?;
        out.lock().unwrap().extend([slice, clone]);
        out.lock().unwrap().extend(ranges);
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(owner.observe().unwrap().pending_read_operations, 0);
    assert!(resources.observation().used.memory_bytes >= baseline + 8192 + 16);
    retained.lock().unwrap().remove(1);
    assert!(resources.observation().used.memory_bytes >= baseline + 8192);
    retained.lock().unwrap().clear();
    assert_eq!(resources.observation().used.memory_bytes, baseline);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_local_multipart_complete_abort_and_cancelled_part_are_terminal_at_join() {
    let directory = TempDir::new().unwrap();
    let resources = budget(8 * 1024 * 1024);
    let owner = store(&directory, &resources);
    let complete = location(&directory, "complete");
    let aborted = location(&directory, "aborted");
    let cancelled = location(&directory, "cancelled");
    run(&owner, &resources, true, move |store| async move {
        let mut upload = store.put_multipart(&complete).await?;
        upload.put_part(Bytes::from_static(b"first").into()).await?;
        upload
            .put_part(Bytes::from_static(b"second").into())
            .await?;
        upload.complete().await?;
        drop(upload);
        assert_eq!(
            &store.get(&complete).await?.bytes().await?[..],
            b"firstsecond"
        );
        let mut upload = store.put_multipart(&aborted).await?;
        upload.put_part(Bytes::from_static(b"abort").into()).await?;
        upload.abort().await?;
        drop(upload);
        let mut upload = store.put_multipart(&cancelled).await?;
        // One poll launches native blocking IO, then dropping Pending loses the public
        // part observer while the native runtime and charged Bytes still own the write.
        let _ = upload.put_part(vec![4; 1024 * 1024].into()).now_or_never();
        drop(upload);
        assert_eq!(store.observe().unwrap().pending_uploads, 3);
        Ok(())
    })
    .await
    .unwrap();
    let observation = owner.observe().unwrap();
    assert_eq!(observation.files, 1);
    assert_eq!(observation.pending_uploads, 0);
    assert!(!observation.mutation_active);
    assert!(!directory.path().join("aborted").exists());
    assert!(!directory.path().join("cancelled").exists());
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    assert_eq!(
        owner
            .inner
            .headroom
            .observe()
            .unwrap()
            .in_flight_growth_bytes,
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_local_rejects_mutations_from_other_runtime_and_symlink_roots() {
    let directory = TempDir::new().unwrap();
    let resources = budget(8 * 1024 * 1024);
    let owner = store(&directory, &resources);
    let path = location(&directory, "forbidden");
    assert!(
        owner
            .put(&path, Bytes::from_static(b"x").into())
            .await
            .is_err()
    );
    let foreign = owner.clone();
    let foreign_path = path.clone();
    let rejected = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&rejected);
    run(&owner, &resources, true, move |store| async move {
        let worker = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async {
                assert!(
                    foreign
                        .put(&foreign_path, Bytes::from_static(b"x").into())
                        .await
                        .is_err()
                );
                seen.store(true, Ordering::Release);
            });
        });
        worker.join().unwrap();
        store
            .put(&path, Bytes::from_static(b"valid").into())
            .await?;
        Ok(())
    })
    .await
    .unwrap();
    assert!(rejected.load(Ordering::Acquire));
    let external = TempDir::new().unwrap();
    std::os::unix::fs::symlink(external.path(), directory.path().join("escape")).unwrap();
    let path = location(&directory, "escape/file");
    // Reads cannot follow a newly introduced symlink either.
    assert!(owner.get(&path).await.is_err());
    assert!(
        owner
            .register_root(
                &Url::from_directory_path(directory.path()).unwrap(),
                ResourceClass::Data
            )
            .is_err()
    );
    assert!(!owner.observe().unwrap().ready);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_local_deadline_drops_public_upload_then_joins_and_reconciles_cleanup() {
    let directory = TempDir::new().unwrap();
    let resources = budget(8 * 1024 * 1024);
    let owner = store(&directory, &resources);
    let baseline = resources.observation().used.memory_bytes;
    let target = location(&directory, "deadline");
    let started = Arc::new(AtomicBool::new(false));
    let ready = Arc::clone(&started);
    let result = run_with_deadline(
        &owner,
        &resources,
        true,
        Duration::from_secs(1),
        move |store| async move {
            let mut upload = store.put_multipart(&target).await?;
            upload.put_part(vec![4; 4096].into()).await?;
            ready.store(true, Ordering::Release);
            let result = futures::future::pending::<object_store::Result<()>>().await;
            drop(upload);
            result
        },
    )
    .await;
    assert!(started.load(Ordering::Acquire));
    assert!(matches!(result, Err(NativeLaneError::Deadline)));
    let observation = owner.observe().unwrap();
    assert!(observation.ready);
    assert!(!observation.mutation_active);
    assert_eq!(observation.files, 0);
    assert_eq!(observation.pending_uploads, 0);
    assert_eq!(
        owner
            .inner
            .headroom
            .observe()
            .unwrap()
            .in_flight_growth_bytes,
        0
    );
    assert_eq!(resources.observation().used.memory_bytes, baseline);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_local_failed_multipart_completion_retains_staging_bytes_until_confirmed_delete() {
    let directory = TempDir::new().unwrap();
    std::fs::create_dir(directory.path().join("destination")).unwrap();
    let resources = budget(8 * 1024 * 1024);
    let owner = store(&directory, &resources);
    let baseline = owner.observe().unwrap().physical_bytes;
    let target = location(&directory, "destination");
    run(&owner, &resources, true, move |store| async move {
        let mut upload = store.put_multipart(&target).await?;
        upload.put_part(vec![5; 8192].into()).await?;
        assert!(upload.complete().await.is_err());
        Ok(())
    })
    .await
    .unwrap();
    let observed = owner.observe().unwrap();
    assert_eq!(
        observed.files, 1,
        "native failed rename leaves staging file"
    );
    assert!(observed.physical_bytes >= baseline + 8192);
    assert_eq!(observed.reserved_disk_bytes, observed.physical_bytes);
    let staging = std::fs::read_dir(directory.path())
        .unwrap()
        .map(Result::unwrap)
        .find(|entry| entry.file_name().as_bytes().contains(&b'#'))
        .unwrap();
    // Native ObjectStore deliberately rejects its reserved staging names even for delete.
    // Recovery may remove the physical orphan; admission stays charged until a fresh census
    // confirms removal. The ledger never turns native listing invisibility into free bytes.
    std::fs::remove_file(staging.path()).unwrap();
    assert_eq!(
        owner.observe().unwrap().reserved_disk_bytes,
        observed.physical_bytes
    );
    owner
        .register_root(
            &Url::from_directory_path(directory.path()).unwrap(),
            ResourceClass::Data,
        )
        .unwrap();
    assert_eq!(owner.observe().unwrap().physical_bytes, baseline);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_local_failed_after_join_census_keeps_charge_and_join_evidence_for_control_retry() {
    let directory = TempDir::new().unwrap();
    let external = TempDir::new().unwrap();
    let resources = budget(8 * 1024 * 1024);
    let owner = store(&directory, &resources);
    let target = location(&directory, "file");
    let link = directory.path().join("changed");
    let introduce = link.clone();
    let outside = external.path().to_owned();
    let result = run(&owner, &resources, true, move |store| async move {
        assert!(matches!(
            store.retry_after_join_reconciliation(),
            Err(OwnedLocalStoreError::ForeignJoin)
        ));
        store.put(&target, vec![6; 8192].into()).await?;
        std::os::unix::fs::symlink(outside, introduce).unwrap();
        Ok(())
    })
    .await;
    assert!(matches!(result, Err(NativeLaneError::Cleanup { .. })));
    let failed = owner.observe().unwrap();
    assert!(!failed.ready);
    assert!(failed.mutation_active);
    assert!(failed.reserved_disk_bytes >= 8192);
    assert!(
        owner
            .inner
            .headroom
            .observe()
            .unwrap()
            .in_flight_growth_bytes
            > 0
    );
    std::fs::remove_file(link).unwrap();
    owner.retry_after_join_reconciliation().unwrap();
    let recovered = owner.observe().unwrap();
    assert!(recovered.ready);
    assert!(!recovered.mutation_active);
    assert_eq!(recovered.files, 1);
    assert_eq!(recovered.reserved_disk_bytes, recovered.physical_bytes);
    assert_eq!(
        owner
            .inner
            .headroom
            .observe()
            .unwrap()
            .in_flight_growth_bytes,
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_local_tiny_control_listing_does_not_reserve_global_history_ceiling() {
    let directory = TempDir::new().unwrap();
    std::fs::write(directory.path().join("00000000000000000000.json"), b"{}").unwrap();
    let resources = budget(8 * 1024 * 1024);
    let mut policy = limits();
    policy.max_list_entries = 1_000_000;
    let owner = OwnedLocalStore::try_new(
        resources.clone(),
        LocalDiskHeadroom::open(directory.path()).unwrap(),
        policy,
    )
    .unwrap();
    owner
        .register_root(
            &Url::from_directory_path(directory.path()).unwrap(),
            ResourceClass::Control,
        )
        .unwrap();
    let baseline = resources.observation().used.memory_bytes;
    let root = location(&directory, "");
    run(&owner, &resources, false, move |store| async move {
        for _ in 0..8 {
            let listed = store.list(Some(&root)).try_collect::<Vec<_>>().await?;
            assert_eq!(listed.len(), 1);
            let delimiter = store.list_with_delimiter(Some(&root)).await?;
            assert_eq!(delimiter.objects.len(), 1);
        }
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(resources.observation().used.memory_bytes, baseline);
    assert!(
        resources.observation().peak.memory_bytes
            < baseline + u128::from(lane().admitted_capacity().memory_bytes) + 8 * 1024 * 1024
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_local_read_bound_rejects_before_materialization_and_releases_on_completion() {
    let directory = TempDir::new().unwrap();
    std::fs::write(directory.path().join("large"), vec![7; 8192]).unwrap();
    let resources = budget(8 * 1024 * 1024);
    let mut policy = limits();
    policy.max_read_bytes = 1024;
    let owner = OwnedLocalStore::try_new(
        resources.clone(),
        LocalDiskHeadroom::open(directory.path()).unwrap(),
        policy,
    )
    .unwrap();
    owner
        .register_root(
            &Url::from_directory_path(directory.path()).unwrap(),
            ResourceClass::Data,
        )
        .unwrap();
    let baseline = resources.observation().used.memory_bytes;
    let target = location(&directory, "large");
    run(&owner, &resources, false, move |store| async move {
        assert!(store.get(&target).await.is_err());
        assert_eq!(store.observe().unwrap().pending_read_operations, 0);
        assert!(store.get_ranges(&target, &[0..2048]).await.is_err());
        assert_eq!(store.observe().unwrap().pending_read_operations, 0);
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(owner.observe().unwrap().pending_read_operations, 0);
    assert_eq!(resources.observation().used.memory_bytes, baseline);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_local_active_stream_grows_before_concurrent_native_mutation_and_releases_batch() {
    let directory = TempDir::new().unwrap();
    std::fs::write(directory.path().join("first"), b"first").unwrap();
    let resources = budget(8 * 1024 * 1024);
    let owner = store(&directory, &resources);
    let root = location(&directory, "");
    let target = location(&directory, "second");
    run(&owner, &resources, true, move |store| async move {
        let mut input = store.list(Some(&root));
        assert!(input.try_next().await?.is_some());
        let charge = || {
            store
                .inner
                .state
                .lock()
                .unwrap()
                .pending_reads
                .values()
                .find(|read| read.list.is_some())
                .unwrap()
                ._charge
                .reservation
                .lock()
                .unwrap()
                .amounts()
                .memory_bytes
        };
        let buffered = charge();
        store
            .put(&target, Bytes::from_static(b"second").into())
            .await?;
        assert!(
            charge() > buffered,
            "reserve new path metadata before its native mutation"
        );
        while input.try_next().await?.is_some() {}
        let state = store.inner.state.lock().unwrap();
        assert!(state.pending_reads.values().all(|read| read.list.is_none()));
        let retained: u64 = state
            .pending_reads
            .values()
            .map(|read| {
                read._charge
                    .reservation
                    .lock()
                    .unwrap()
                    .amounts()
                    .memory_bytes
            })
            .sum();
        assert!(
            retained < buffered,
            "completed stream releases native batch and iterator scratch"
        );
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(owner.observe().unwrap().files, 2);
}

fn reserve_data_except(resources: &ResourceBudget, available: u64) -> ResourceReservation {
    let observed = resources.observation();
    let ceiling =
        observed.policy.limits.memory_bytes - observed.policy.control_reserve.memory_bytes;
    let used = u64::try_from(observed.data_used.memory_bytes).unwrap();
    resources
        .try_reserve(ResourceClass::Data, memory(ceiling - used - available))
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_local_long_nested_listing_admits_actual_maximum_path_before_native_results() {
    let directory = TempDir::new().unwrap();
    let resources = budget(8 * 1024 * 1024);
    let owner = store(&directory, &resources);
    let root_bytes = directory.path().as_os_str().as_bytes().len();
    let descriptor = open_absolute_directory_nofollow(directory.path()).unwrap();
    let mut census = PhysicalCensus::default();
    census_directory(
        &descriptor,
        0,
        root_bytes,
        &limits(),
        owner.inner.allocation_unit,
        &mut census,
    )
    .unwrap();
    assert_eq!(
        census.maximum_path_bytes, root_bytes,
        "empty roots still contribute their full path"
    );
    let nested = directory.path().join("a".repeat(150)).join("b".repeat(150));
    std::fs::create_dir_all(&nested).unwrap();
    let file = nested.join("c".repeat(140));
    std::fs::write(&file, b"long path").unwrap();
    let longest = file.as_os_str().as_bytes().len();
    let mut census = PhysicalCensus::default();
    census_directory(
        &descriptor,
        0,
        root_bytes,
        &limits(),
        owner.inner.allocation_unit,
        &mut census,
    )
    .unwrap();
    assert_eq!(census.maximum_path_bytes, longest);
    owner
        .register_root(
            &Url::from_directory_path(directory.path()).unwrap(),
            ResourceClass::Data,
        )
        .unwrap();
    let root = location(&directory, "");
    let pressure = resources.clone();
    run(&owner, &resources, false, move |store| async move {
        let scratch = u64::try_from((limits().max_directory_depth + 1) * crate::secure_path::DIRECTORY_ITERATION_MEMORY_BOUND).unwrap();
        let ticket_bytes = u64::try_from(limits().max_path_bytes * 3 + 512).unwrap();
        let batch = u64::try_from(1024 * std::mem::size_of::<object_store::Result<ObjectMeta>>()).unwrap();
        // This capacity admits the old prefix-only estimate, but cannot admit the real
        // nested path metadata. No native result may escape at this pressure level.
        let short_only = metadata_capacity(4, root_bytes).unwrap();
        let available = ticket_bytes + 2 * scratch + batch + short_only + 1;
        let reservation = reserve_data_except(&pressure, available);
        let mut input = store.list(Some(&root));
        assert!(matches!(input.try_next().await, Err(object_store::Error::Generic { source, .. })
            if matches!(source.downcast_ref::<OwnedLocalStoreError>(), Some(OwnedLocalStoreError::Budget(_)))));
        drop(input);
        drop(reservation);
        let listed = store.list(Some(&root)).try_collect::<Vec<_>>().await?;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].location.as_ref().len() + 1, longest);
        Ok(())
    }).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_local_open_read_retains_deleted_inode_disk_and_headroom_until_read_runtime_joins() {
    let directory = TempDir::new().unwrap();
    std::fs::write(directory.path().join("old"), vec![8; 8192]).unwrap();
    let resources = budget(8 * 1024 * 1024);
    let owner = store(&directory, &resources);
    let before = owner.observe().unwrap().reserved_disk_bytes;
    let old = location(&directory, "old");
    let read_path = old.clone();
    let reader = owner.clone();
    let reader_resources = resources.clone();
    let (opened, ready) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let read = tokio::spawn(async move {
        run(&reader, &reader_resources, false, move |store| async move {
            // Pause at the real get_opts boundary: LocalFileSystem has returned an open
            // file but OwnedLocalStore has not finished converting it to charged Bytes.
            let ticket = store.read_ticket(&read_path, 8192).map_err(object_error)?;
            let native = store
                .inner
                .backend
                .get_opts(&read_path, GetOptions::default())
                .await?;
            assert!(matches!(&native.payload, GetResultPayload::File(..)));
            opened.send(()).unwrap();
            let _ = released.await;
            let bytes = native.bytes().await?;
            assert_eq!(bytes.len(), 8192);
            assert!(bytes.iter().all(|byte| *byte == 8));
            store.complete_read(&ticket).map_err(object_error)?;
            Ok(())
        })
        .await
    });
    ready.await.unwrap();
    let new = location(&directory, "new");
    let written = new.clone();
    run(&owner, &resources, true, move |store| async move {
        store.delete(&old).await?;
        store
            .put(&written, Bytes::from_static(b"new").into())
            .await?;
        Ok(())
    })
    .await
    .unwrap();
    assert!(!directory.path().join("old").exists());
    let deferred = owner.observe().unwrap();
    assert!(
        deferred.mutation_active,
        "joined writer keeps deferred physical ownership"
    );
    assert!(deferred.reserved_disk_bytes > before);
    assert!(
        owner
            .inner
            .headroom
            .observe()
            .unwrap()
            .in_flight_growth_bytes
            > 0
    );
    run(&owner, &resources, false, move |store| async move {
        assert_eq!(&store.get(&new).await?.bytes().await?[..], b"new");
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(
        owner.observe().unwrap().reserved_disk_bytes,
        deferred.reserved_disk_bytes
    );
    release.send(()).unwrap();
    read.await.unwrap().unwrap();
    let joined = owner.observe().unwrap();
    assert!(!joined.mutation_active);
    assert_eq!(joined.files, 1);
    assert!(joined.reserved_disk_bytes < before);
    assert_eq!(joined.reserved_disk_bytes, joined.physical_bytes);
    assert_eq!(
        owner
            .inner
            .headroom
            .observe()
            .unwrap()
            .in_flight_growth_bytes,
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_local_cleanup_closes_admission_before_late_native_child_upload() {
    let directory = TempDir::new().unwrap();
    let resources = budget(8 * 1024 * 1024);
    let owner = store(&directory, &resources);
    let existing = location(&directory, "existing");
    let late = location(&directory, "late");
    run(&owner, &resources, true, move |store| async move {
        let lease = {
            let state = store.state().unwrap();
            let active = state.active.as_ref().unwrap();
            OwnedLocalMutation { store: store.clone(), generation: active.generation, runtime: active.runtime }
        };
        let mut upload = store.put_multipart(&existing).await?;
        upload.put_part(Bytes::from_static(b"staged").into()).await?;
        let (release, released) = tokio::sync::oneshot::channel();
        let child_store = store.clone();
        let child = tokio::spawn(async move {
            released.await.unwrap();
            child_store.put_multipart(&late).await
        });
        lease.drain_cleanup().await.map_err(object_error)?;
        release.send(()).unwrap();
        let result = child.await.unwrap();
        assert!(matches!(result, Err(object_store::Error::Generic { source, .. })
            if matches!(source.downcast_ref::<OwnedLocalStoreError>(), Some(OwnedLocalStoreError::MutationClosed))));
        drop(upload);
        Ok(())
    }).await.unwrap();
    assert_eq!(owner.observe().unwrap().files, 0);
    assert_eq!(owner.observe().unwrap().pending_uploads, 0);
    assert!(!owner.observe().unwrap().mutation_active);
    assert_eq!(
        owner
            .inner
            .headroom
            .observe()
            .unwrap()
            .in_flight_growth_bytes,
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_local_empty_ranges_admit_each_descriptor_and_retain_owners_after_join() {
    let directory = TempDir::new().unwrap();
    std::fs::write(directory.path().join("empty-ranges"), b"x").unwrap();
    let resources = budget(8 * 1024 * 1024);
    let owner = store(&directory, &resources);
    let baseline = resources.observation().used.memory_bytes;
    let target = location(&directory, "empty-ranges");
    let pressure = resources.clone();
    let retained = Arc::new(Mutex::new(Vec::<Bytes>::new()));
    let output = Arc::clone(&retained);
    run(&owner, &resources, false, move |store| async move {
        let ranges = vec![0..0; 64];
        let ticket = u64::try_from(limits().max_path_bytes * 3 + 512).unwrap();
        let reservation = reserve_data_except(&pressure, ticket + 64 * 32);
        assert!(matches!(store.get_ranges(&target, &ranges).await,
            Err(object_store::Error::Generic { source, .. })
            if source.is::<ResourceBudgetError>()
                || matches!(source.downcast_ref::<OwnedLocalStoreError>(), Some(OwnedLocalStoreError::Budget(_)))));
        drop(reservation);
        let bytes = store.get_ranges(&target, &ranges).await?;
        assert_eq!(bytes.len(), 64);
        assert!(bytes.iter().all(Bytes::is_empty));
        *output.lock().unwrap() = bytes;
        Ok(())
    }).await.unwrap();
    assert!(resources.observation().used.memory_bytes >= baseline + 64 * 32);
    let last_owner = retained.lock().unwrap()[0].clone();
    retained.lock().unwrap().clear();
    assert!(resources.observation().used.memory_bytes >= baseline + 64 * 32);
    drop(last_owner);
    assert_eq!(resources.observation().used.memory_bytes, baseline);
}

fn bootstrap_owner(directory: &TempDir, resources: &ResourceBudget) -> OwnedLocalStore {
    OwnedLocalStore::try_new(
        resources.clone(),
        LocalDiskHeadroom::open(directory.path()).unwrap(),
        limits(),
    )
    .unwrap()
}

fn bootstrap_workspace_path(directory: &TempDir, resources: &ResourceBudget) -> PathBuf {
    let hex = resources
        .owner()
        .id
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    directory.path().join("fabric").join(hex)
}

#[test]
fn owned_local_bootstrap_admits_before_mkdir_and_rejects_foreign_workspace() {
    let directory = TempDir::new().unwrap();
    let unit = LocalDiskHeadroom::open(directory.path())
        .unwrap()
        .observe()
        .unwrap()
        .allocation_unit;
    let resources = budget(unit * 3);
    let owner = bootstrap_owner(&directory, &resources);
    assert!(matches!(
        owner.bootstrap_workspace_roots(directory.path(), [1; 16]),
        Err(OwnedLocalStoreError::ForeignOwner)
    ));
    assert!(matches!(
        owner.bootstrap_workspace_roots(directory.path(), resources.owner().id),
        Err(OwnedLocalStoreError::Budget(_))
    ));
    assert!(!directory.path().join("fabric").exists());
    assert_eq!(owner.observe().unwrap().reserved_disk_bytes, 0);
    assert_eq!(
        owner
            .inner
            .headroom
            .observe()
            .unwrap()
            .in_flight_growth_bytes,
        0
    );
}

#[test]
fn owned_local_bootstrap_failure_retains_charge_until_complete_retry() {
    let directory = TempDir::new().unwrap();
    let resources = budget(8 * 1024 * 1024);
    let owner = bootstrap_owner(&directory, &resources);
    owner.inner.bootstrap_fail_step.store(4, Ordering::Release);
    assert!(
        owner
            .bootstrap_workspace_roots(directory.path(), resources.owner().id)
            .is_err()
    );
    let workspace = bootstrap_workspace_path(&directory, &resources);
    assert!(workspace.join("activation-control").is_dir());
    assert!(!workspace.join("epochs").exists());
    let failed = owner.observe().unwrap();
    assert!(failed.bootstrap_pending);
    assert!(!failed.ready);
    assert!(failed.reserved_disk_bytes > 0);
    assert_eq!(
        resources.observation().used.disk_bytes,
        u128::from(failed.reserved_disk_bytes)
    );
    assert!(
        owner
            .inner
            .headroom
            .observe()
            .unwrap()
            .in_flight_growth_bytes
            > 0
    );
    let retained = owner.clone();
    drop(owner);
    assert_eq!(
        retained.observe().unwrap().reserved_disk_bytes,
        failed.reserved_disk_bytes
    );
    retained
        .inner
        .bootstrap_fail_step
        .store(0, Ordering::Release);
    retained
        .bootstrap_workspace_roots(directory.path(), resources.owner().id)
        .unwrap();
    let complete = retained.observe().unwrap();
    assert!(complete.ready);
    assert!(!complete.bootstrap_pending);
    assert_eq!(complete.roots, 2);
    assert_eq!(complete.directories, 4);
    assert_eq!(complete.reserved_disk_bytes, complete.physical_bytes);
    assert_eq!(
        retained
            .inner
            .headroom
            .observe()
            .unwrap()
            .in_flight_growth_bytes,
        0
    );
}

#[test]
fn owned_local_bootstrap_retry_rejects_substituted_parent_and_symlink() {
    let directory = TempDir::new().unwrap();
    let foreign = TempDir::new().unwrap();
    let resources = budget(8 * 1024 * 1024);
    let owner = bootstrap_owner(&directory, &resources);
    owner.inner.bootstrap_fail_step.store(3, Ordering::Release);
    assert!(
        owner
            .bootstrap_workspace_roots(directory.path(), resources.owner().id)
            .is_err()
    );
    let workspace = bootstrap_workspace_path(&directory, &resources);
    std::fs::rename(&workspace, workspace.with_extension("retained")).unwrap();
    std::os::unix::fs::symlink(foreign.path(), &workspace).unwrap();
    owner.inner.bootstrap_fail_step.store(0, Ordering::Release);
    assert!(
        owner
            .bootstrap_workspace_roots(directory.path(), resources.owner().id)
            .is_err()
    );
    assert!(!foreign.path().join("activation-control").exists());
    assert!(!foreign.path().join("epochs").exists());
    assert!(owner.observe().unwrap().bootstrap_pending);
    assert!(
        owner
            .inner
            .headroom
            .observe()
            .unwrap()
            .in_flight_growth_bytes
            > 0
    );
    std::fs::remove_file(&workspace).unwrap();
    std::fs::rename(workspace.with_extension("retained"), &workspace).unwrap();
    owner
        .bootstrap_workspace_roots(directory.path(), resources.owner().id)
        .unwrap();
    assert!(owner.observe().unwrap().ready);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_local_production_registry_bootstrap_and_joined_mutation_share_one_owner() {
    use crate::fabric::command::WorkspaceId;
    use crate::fabric::workspace_resources::{ProductionWorkspaceResources, local_resource_policy};
    let directory = TempDir::new().unwrap();
    let process = ResourceBudget::try_process([77; 16], local_resource_policy()).unwrap();
    let resources = ProductionWorkspaceResources::try_new(
        &process,
        WorkspaceId::from_bytes([92; 16]),
        directory.path(),
    )
    .unwrap();
    assert!(
        !directory.path().join("fabric").exists(),
        "constructor does not drop bootstrap failures as strings"
    );
    let bootstrap_scope = StructuredCancellationScope::try_root_with_control_reserve(
        "production-bootstrap-test",
        NonZeroUsize::new(4).unwrap(),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap();
    resources
        .local_store()
        .inner
        .bootstrap_fail_step
        .store(4, Ordering::Release);
    let failure = resources
        .bootstrap_local_store_owned(&bootstrap_scope)
        .await
        .unwrap_err();
    assert!(matches!(
        failure,
        crate::fabric::workspace_resources::WorkspaceStoreBootstrapError::Store(_)
    ));
    assert!(resources.local_store().observe().unwrap().bootstrap_pending);
    assert!(
        resources
            .local_store()
            .observe()
            .unwrap()
            .reserved_disk_bytes
            > 0
    );
    assert_eq!(resources.budget().observation().used.running_jobs, 0);
    resources
        .local_store()
        .inner
        .bootstrap_fail_step
        .store(0, Ordering::Release);
    resources
        .bootstrap_local_store_owned(&bootstrap_scope)
        .await
        .unwrap();
    let owner = resources.local_store().as_ref().clone();
    assert!(owner.budget().same_scope(resources.budget()));
    let url = Url::parse("file:///").unwrap();
    let expected: Arc<dyn ObjectStore> = resources.local_store().clone();
    let runtime = resources.native().runtime_env();
    let actual = runtime.object_store_registry.get_store(&url).unwrap();
    assert!(Arc::ptr_eq(&actual, &expected));
    let control = resources
        .native()
        .control_runtime_with_registry(Arc::clone(&runtime.object_store_registry))
        .unwrap();
    assert!(Arc::ptr_eq(
        &control.object_store_registry.get_store(&url).unwrap(),
        &expected
    ));
    let workspace = bootstrap_workspace_path(&directory, resources.budget());
    let path =
        Path::from_absolute_path(workspace.join("epochs/01/relations/value.parquet")).unwrap();
    let unowned = actual
        .put(&path, Bytes::from_static(b"rejected").into())
        .await
        .unwrap_err();
    let object_store::Error::Generic { source, .. } = unowned else {
        panic!("typed owned error expected")
    };
    assert!(matches!(
        source.downcast_ref::<OwnedLocalStoreError>(),
        Some(OwnedLocalStoreError::MutationRuntime)
    ));
    let write_path = path.clone();
    run(&owner, resources.budget(), true, move |store| async move {
        store
            .put(&write_path, Bytes::from_static(b"owned").into())
            .await?;
        Ok(())
    })
    .await
    .unwrap();
    assert!(!owner.observe().unwrap().mutation_active);
    assert_eq!(
        std::fs::read(workspace.join("epochs/01/relations/value.parquet")).unwrap(),
        b"owned"
    );
    let source_blobs = workspace.join("source-blobs");
    std::fs::create_dir(&source_blobs).unwrap();
    std::fs::write(source_blobs.join("foreign-ledger"), vec![0; 32_768]).unwrap();
    let denied = Path::from_absolute_path(source_blobs.join("foreign-ledger")).unwrap();
    assert!(actual.head(&denied).await.is_err());
    resources
        .bootstrap_local_store_owned(&bootstrap_scope)
        .await
        .unwrap();
    assert_eq!(
        owner.observe().unwrap().files,
        1,
        "source blobs never enter the Delta disk census"
    );
    assert_eq!(
        owner.observe().unwrap().physical_bytes,
        owner.observe().unwrap().reserved_disk_bytes
    );
    bootstrap_scope
        .cancel_and_join(Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(resources.budget().observation().used.running_jobs, 0);
}

#[test]
fn owned_local_bootstrap_restart_census_separates_data_control_and_source_blobs() {
    let directory = TempDir::new().unwrap();
    let resources = budget(8 * 1024 * 1024);
    let workspace = bootstrap_workspace_path(&directory, &resources);
    for name in ["activation-control", "epochs", "source-blobs"] {
        std::fs::create_dir_all(workspace.join(name)).unwrap();
        std::fs::write(workspace.join(name).join("retained"), vec![3; 8193]).unwrap();
    }
    let owner = bootstrap_owner(&directory, &resources);
    owner
        .bootstrap_workspace_roots(directory.path(), resources.owner().id)
        .unwrap();
    let census = owner.observe().unwrap();
    assert_eq!(census.roots, 2);
    assert_eq!(census.files, 2);
    assert_eq!(census.directories, 4);
    assert_eq!(census.reserved_disk_bytes, census.physical_bytes);
    let usage = resources.observation();
    assert!(usage.data_used.disk_bytes > 8193);
    assert!(usage.used.disk_bytes - usage.data_used.disk_bytes > 8193);
    assert_eq!(usage.used.disk_bytes, u128::from(census.physical_bytes));
    drop(owner);
    let reopened = bootstrap_owner(&directory, &resources);
    reopened
        .bootstrap_workspace_roots(directory.path(), resources.owner().id)
        .unwrap();
    assert_eq!(reopened.observe().unwrap(), census);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_local_completed_head_does_not_block_reconciliation() {
    let directory = TempDir::new().unwrap();
    std::fs::write(directory.path().join("value"), b"retained").unwrap();
    let resources = budget(8 * 1024 * 1024);
    let owner = store(&directory, &resources);
    let target = location(&directory, "value");
    let root = Url::from_directory_path(directory.path()).unwrap();
    let during = root.clone();
    run(&owner, &resources, false, move |store| async move {
        store.head(&target).await?;
        assert_eq!(store.observe().unwrap().pending_read_operations, 0);
        store.register_root(&during, ResourceClass::Data).unwrap();
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(owner.observe().unwrap().pending_read_operations, 0);
    owner.register_root(&root, ResourceClass::Data).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_local_production_registry_provisions_and_reopens_exact_native_delta() {
    use crate::fabric::command::WorkspaceId;
    use crate::fabric::workspace_resources::{ProductionWorkspaceResources, local_resource_policy};
    let directory = TempDir::new().unwrap();
    let process = ResourceBudget::try_process([76; 16], local_resource_policy()).unwrap();
    let resources = ProductionWorkspaceResources::try_new(
        &process,
        WorkspaceId::from_bytes([92; 16]),
        directory.path(),
    )
    .unwrap();
    let bootstrap_scope = StructuredCancellationScope::try_root_with_control_reserve(
        "production-bootstrap-test",
        NonZeroUsize::new(4).unwrap(),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap();
    resources
        .bootstrap_local_store_owned(&bootstrap_scope)
        .await
        .unwrap();
    let root_path =
        bootstrap_workspace_path(&directory, resources.budget()).join("activation-control");
    let root = Url::from_directory_path(&root_path).unwrap();
    let runtime = resources.native().runtime_env();
    run(
        resources.local_store(),
        resources.budget(),
        true,
        move |_| async move {
            let session = datafusion::execution::session_state::SessionStateBuilder::new()
                .with_default_features()
                .with_runtime_env(runtime)
                .build();
            let (pin, table) =
                crate::fabric::activation_control_delta::provision_activation_control_history(
                    root.clone(),
                    &session,
                )
                .await
                .map_err(object_error)?;
            assert_eq!(pin.version(), 0);
            assert_eq!(table.version(), Some(0));
            drop(table);
            let reopened = crate::fabric::delta_exact::session_delta_table_builder(root, &session)
                .map_err(object_error)?
                .with_version(0)
                .load()
                .await
                .map_err(object_error)?;
            assert_eq!(reopened.version(), Some(0));
            Ok(())
        },
    )
    .await
    .unwrap();
    assert!(
        root_path
            .join("_delta_log/00000000000000000000.json")
            .is_file()
    );
    let observation = resources.local_store().observe().unwrap();
    assert!(observation.ready);
    assert!(!observation.mutation_active);
    assert_eq!(observation.pending_read_operations, 0);
    assert_eq!(observation.reserved_disk_bytes, observation.physical_bytes);
    bootstrap_scope
        .cancel_and_join(Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(resources.budget().observation().used.running_jobs, 0);
}
