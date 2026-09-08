//! Original-backing resource access probes. Nothing is reconstructed through serialization.

use std::sync::Arc;

use deltalake_core::kernel::{
    Action, Add, DataType, EagerSnapshot, PrimitiveType, Remove, StructField,
};
use deltalake_core::operations::create::CreateBuilder;
use deltalake_core::{DeltaResult, DeltaTable, DeltaTableConfig};
use futures::TryStreamExt;

async fn table_with_actions(actions: Vec<Action>) -> DeltaResult<(tempfile::TempDir, DeltaTable)> {
    let directory = tempfile::tempdir()?;
    let table = CreateBuilder::new()
        .with_location(directory.path().to_string_lossy())
        .with_columns([StructField::new(
            "id",
            DataType::Primitive(PrimitiveType::Integer),
            true,
        )])
        .with_actions(actions)
        .await?;
    Ok((directory, table))
}

fn add(path: &str) -> Action {
    Action::Add(Add {
        path: path.into(),
        size: 1,
        data_change: true,
        stats: Some(
            r#"{"numRecords":1,"minValues":{"id":1},"maxValues":{"id":1},"nullCount":{"id":0}}"#
                .into(),
        ),
        ..Default::default()
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retained_state_distinguishes_unmaterialized_and_materialized_empty() -> DeltaResult<()> {
    let (_directory, table) = table_with_actions(vec![]).await?;
    let log_store = table.log_store();
    let lazy = EagerSnapshot::try_new(
        log_store.as_ref(),
        DeltaTableConfig {
            require_files: false,
            ..Default::default()
        },
        Some(0),
    )
    .await?;
    assert!(!lazy.retained_state().configuration().require_files);
    assert!(lazy.retained_state().materialized_files().is_none());
    // Compatibility log_data used to hide this distinction behind an empty handler.
    assert!(lazy.log_data().retained_batches().is_empty());
    assert!(
        lazy.retained_state().materialized_files().is_none(),
        "observation must not materialize"
    );

    let eager = EagerSnapshot::try_new(log_store.as_ref(), Default::default(), Some(0)).await?;
    let files = eager
        .retained_state()
        .materialized_files()
        .expect("materialized empty cache");
    assert!(files.matches_snapshot());
    assert_eq!(
        files
            .batches()
            .iter()
            .map(|batch| batch.num_rows())
            .sum::<usize>(),
        0
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retained_state_borrows_original_batches_and_file_view_survives_snapshot_drop()
-> DeltaResult<()> {
    let (_directory, table) =
        table_with_actions(vec![add("first.parquet"), add("second.parquet")]).await?;
    let log_store = table.log_store();
    let eager = EagerSnapshot::try_new(log_store.as_ref(), Default::default(), Some(0)).await?;
    drop(table);
    let cloned = eager.clone();
    assert!(Arc::ptr_eq(
        eager.retained_snapshot_owner(),
        cloned.retained_snapshot_owner()
    ));
    let snapshot_weak = Arc::downgrade(eager.retained_snapshot_owner());
    let retained = eager.retained_state().materialized_files().unwrap();
    let cache_weak = Arc::downgrade(retained.batches());
    assert!(std::ptr::eq(
        eager.log_data().retained_batches(),
        retained.batches().as_ref().as_ref()
    ));
    let original_array = Arc::downgrade(retained.batches()[0].column(0));
    let view = eager.log_data().iter().next().unwrap();
    assert!(Arc::ptr_eq(
        view.retained_native_batch(),
        &retained.batches()[0]
    ));
    for (original, viewed) in retained.batches()[0]
        .columns()
        .iter()
        .zip(view.retained_batch().columns())
    {
        assert!(
            Arc::ptr_eq(original, viewed),
            "the original array owner must be preserved"
        );
        let original = original.to_data();
        let viewed = viewed.to_data();
        for (left, right) in original.buffers().iter().zip(viewed.buffers()) {
            assert_eq!(
                left.as_ptr(),
                right.as_ptr(),
                "actual payload buffer identity"
            );
        }
    }
    let view_clone = view.clone();
    assert!(Arc::ptr_eq(
        view.retained_native_batch(),
        view_clone.retained_native_batch()
    ));
    assert!(Arc::ptr_eq(
        view.retained_batch().column(0),
        view_clone.retained_batch().column(0)
    ));
    drop(eager);
    assert!(snapshot_weak.upgrade().is_some());
    assert!(cache_weak.upgrade().is_some());
    drop(cloned);
    assert!(snapshot_weak.upgrade().is_none());
    assert!(cache_weak.upgrade().is_none());
    assert!(
        original_array.upgrade().is_some(),
        "row view still owns original Arrow backing"
    );
    assert!(
        view.retained_batch().num_rows() > 1,
        "one row view retains the full batch"
    );
    drop(view);
    assert!(original_array.upgrade().is_some());
    drop(view_clone);
    assert!(original_array.upgrade().is_none());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retained_state_exposes_distinct_original_identity_and_shared_kernel_roots()
-> DeltaResult<()> {
    let (_directory, table) = table_with_actions(vec![add("first.parquet")]).await?;
    let log_store = table.log_store();
    let eager = EagerSnapshot::try_new(log_store.as_ref(), Default::default(), Some(0)).await?;
    drop(table);
    let state = eager.retained_state();
    let files = state.materialized_files().unwrap();
    assert_eq!(files.identity_metadata(), eager.metadata());
    assert_ne!(
        files.identity_metadata().schema_string().as_ptr(),
        eager.metadata().schema_string().as_ptr(),
        "equal metadata in cache identity owns a separate schema string allocation"
    );
    assert!(std::ptr::eq(
        eager.log_data().table_configuration(),
        state.kernel_snapshot().table_configuration()
    ));
    assert_eq!(files.identity_protocol(), eager.protocol());
    assert_eq!(
        files.identity_table_root(),
        state.kernel_snapshot().table_root()
    );
    assert!(files.existing_predicate().is_none());

    let plain_clone = eager.retained_snapshot_owner().as_ref().clone();
    assert!(Arc::ptr_eq(
        state.kernel_snapshot(),
        plain_clone.retained_state().kernel_snapshot()
    ));
    assert!(Arc::ptr_eq(
        files.batches(),
        plain_clone
            .retained_state()
            .materialized_files()
            .unwrap()
            .batches()
    ));
    assert_eq!(
        files.identity_metadata().schema_string().as_ptr(),
        plain_clone
            .retained_state()
            .materialized_files()
            .unwrap()
            .identity_metadata()
            .schema_string()
            .as_ptr()
    );
    drop(plain_clone);

    let kernel = Arc::clone(state.kernel_snapshot());
    let kernel_weak = Arc::downgrade(&kernel);
    let schema = eager.schema();
    let schema_weak = Arc::downgrade(&schema);
    drop(eager);
    assert!(kernel_weak.upgrade().is_some());
    drop(kernel);
    assert!(kernel_weak.upgrade().is_none());
    assert!(
        schema_weak.upgrade().is_some(),
        "schema ownership independently escapes native snapshot"
    );
    drop(schema);
    assert!(schema_weak.upgrade().is_none());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retained_state_tombstone_view_clones_share_their_original_batch() -> DeltaResult<()> {
    let (_directory, table) = table_with_actions(vec![
        add("live.parquet"),
        Action::Remove(Remove {
            path: "removed.parquet".into(),
            deletion_timestamp: Some(1),
            data_change: true,
            ..Default::default()
        }),
    ])
    .await?;
    let log_store = table.log_store();
    let mut removes = table.snapshot()?.all_tombstones(log_store.as_ref());
    let view = removes.try_next().await?.expect("native tombstone");
    drop(removes);
    let cloned = view.clone();
    let array = Arc::downgrade(view.retained_batch().column(0));
    assert!(Arc::ptr_eq(
        view.retained_batch().column(0),
        cloned.retained_batch().column(0)
    ));
    drop(table);
    drop(view);
    assert!(array.upgrade().is_some());
    assert_eq!(cloned.path(), "removed.parquet");
    drop(cloned);
    assert!(array.upgrade().is_none());
    Ok(())
}

use std::sync::atomic::{AtomicUsize, Ordering};

use deltalake_core::DeltaTableError;
use deltalake_core::kernel::{
    MaterializedFilesAdmission, MaterializedFilesBatchRequest, MaterializedFilesIdentityRequest,
    MaterializedFilesLimits, MaterializedFilesReceipt, ResourceExhausted, Snapshot,
};

#[derive(Debug)]
struct LiveReceipt(Arc<AtomicUsize>);

impl LiveReceipt {
    fn acquire(live: &Arc<AtomicUsize>) -> MaterializedFilesReceipt {
        live.fetch_add(1, Ordering::SeqCst);
        MaterializedFilesReceipt::new(Self(live.clone()))
    }
}

impl Drop for LiveReceipt {
    fn drop(&mut self) {
        assert!(self.0.fetch_sub(1, Ordering::SeqCst) > 0);
    }
}

#[derive(Debug)]
struct Admission {
    limits: MaterializedFilesLimits,
    deny_identity: bool,
    deny_batches: bool,
    identity_live: Arc<AtomicUsize>,
    batches_live: Arc<AtomicUsize>,
    identity_requests: AtomicUsize,
    batch_requests: AtomicUsize,
    schema_pointer: AtomicUsize,
    descriptor_bytes: AtomicUsize,
}

impl Admission {
    fn new(max_batches: usize, max_rows: usize) -> Self {
        Self {
            limits: MaterializedFilesLimits {
                max_batches,
                max_rows,
            },
            deny_identity: false,
            deny_batches: false,
            identity_live: Default::default(),
            batches_live: Default::default(),
            identity_requests: AtomicUsize::new(0),
            batch_requests: AtomicUsize::new(0),
            schema_pointer: AtomicUsize::new(0),
            descriptor_bytes: AtomicUsize::new(0),
        }
    }

    fn live(&self) -> (usize, usize) {
        (
            self.identity_live.load(Ordering::SeqCst),
            self.batches_live.load(Ordering::SeqCst),
        )
    }
}

impl MaterializedFilesAdmission for Admission {
    fn limits(&self) -> MaterializedFilesLimits {
        self.limits
    }

    fn try_reserve_identity(
        &self,
        request: MaterializedFilesIdentityRequest<'_>,
    ) -> Result<MaterializedFilesReceipt, ResourceExhausted> {
        self.identity_requests.fetch_add(1, Ordering::SeqCst);
        self.schema_pointer.store(
            request.metadata.schema_string().as_ptr() as usize,
            Ordering::SeqCst,
        );
        assert!(request.owner_bytes > 0);
        if self.deny_identity {
            return Err(ResourceExhausted {
                kind: "test_cache_identity",
                requested: 1,
                limit: 0,
            });
        }
        Ok(LiveReceipt::acquire(&self.identity_live))
    }

    fn try_reserve_batches(
        &self,
        request: MaterializedFilesBatchRequest,
    ) -> Result<MaterializedFilesReceipt, ResourceExhausted> {
        self.batch_requests.fetch_add(1, Ordering::SeqCst);
        assert_eq!(request.limits.max_batches, self.limits.max_batches);
        assert!(request.owner_bytes > 0);
        self.descriptor_bytes
            .store(request.descriptor_bytes, Ordering::SeqCst);
        if self.deny_batches {
            return Err(ResourceExhausted {
                kind: "test_cache_descriptors",
                requested: request.descriptor_bytes,
                limit: 0,
            });
        }
        Ok(LiveReceipt::acquire(&self.batches_live))
    }
}

#[derive(Debug)]
struct UntouchedEngine(Arc<AtomicUsize>);

impl UntouchedEngine {
    fn accessed(&self) -> ! {
        self.0.fetch_add(1, Ordering::SeqCst);
        panic!("denied cache admission must precede native stream construction");
    }
}

impl delta_kernel::Engine for UntouchedEngine {
    fn evaluation_handler(&self) -> Arc<dyn delta_kernel::EvaluationHandler> {
        self.accessed()
    }
    fn storage_handler(&self) -> Arc<dyn delta_kernel::StorageHandler> {
        self.accessed()
    }
    fn json_handler(&self) -> Arc<dyn delta_kernel::JsonHandler> {
        self.accessed()
    }
    fn parquet_handler(&self) -> Arc<dyn delta_kernel::ParquetHandler> {
        self.accessed()
    }
}

fn exhaustion(error: &DeltaTableError) -> ResourceExhausted {
    match error {
        DeltaTableError::KernelError(delta_kernel::Error::ResourceExhausted(source)) => *source,
        other => panic!("resource pressure lost its typed native source: {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cache_admission_denial_precedes_stream_construction_and_releases_partial_receipts()
-> DeltaResult<()> {
    let (_directory, table) = table_with_actions(vec![add("first.parquet")]).await?;
    for deny_identity in [true, false] {
        let policy = Arc::new(Admission {
            deny_identity,
            deny_batches: !deny_identity,
            ..Admission::new(4, 10)
        });
        let snapshot = Snapshot::try_new(table.log_store().as_ref(), Default::default(), Some(0))
            .await?
            .with_materialized_files_admission(policy.clone())?;
        let original_schema = snapshot.metadata().schema_string().as_ptr() as usize;
        let calls = Arc::new(AtomicUsize::new(0));
        let result = Arc::new(snapshot)
            .try_materialize_files_with_engine(Arc::new(UntouchedEngine(calls.clone())))
            .await;
        let error = result.unwrap_err();
        assert_eq!(exhaustion(&error).limit, 0);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            policy.schema_pointer.load(Ordering::SeqCst),
            original_schema
        );
        assert_eq!(policy.live(), (0, 0));
        assert_eq!(policy.identity_requests.load(Ordering::SeqCst), 1);
        assert_eq!(
            policy.batch_requests.load(Ordering::SeqCst),
            usize::from(!deny_identity)
        );
        assert!(
            std::error::Error::source(&error).is_some(),
            "typed source retained"
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cache_identity_and_original_descriptor_receipts_follow_distinct_shared_owners()
-> DeltaResult<()> {
    let (_directory, table) =
        table_with_actions(vec![add("first.parquet"), add("second.parquet")]).await?;
    let policy = Arc::new(Admission::new(4, 10));
    let eager = EagerSnapshot::try_new_with_admission(
        table.log_store().as_ref(),
        Default::default(),
        Some(0),
        policy.clone(),
    )
    .await?;
    drop(table);
    assert_eq!(policy.live(), (1, 1));
    let eager_clone = eager.clone();
    let files = eager.retained_state().materialized_files().unwrap();
    let batches = files.batches().clone();
    assert_eq!(batches.descriptor_capacity(), 4);
    assert_eq!(
        policy.descriptor_bytes.load(Ordering::SeqCst),
        4 * std::mem::size_of::<deltalake_core::kernel::SharedNativeRecordBatch>()
    );
    assert_eq!(
        policy.schema_pointer.load(Ordering::SeqCst),
        eager.metadata().schema_string().as_ptr() as usize
    );
    assert_ne!(
        policy.schema_pointer.load(Ordering::SeqCst),
        files.identity_metadata().schema_string().as_ptr() as usize
    );
    let array = Arc::downgrade(batches[0].column(0));
    let view = eager.log_data().iter().next().unwrap();
    assert!(Arc::ptr_eq(
        batches[0].column(0),
        view.retained_batch().column(0)
    ));
    let view_clone = view.clone();
    drop(eager);
    assert_eq!(policy.live(), (1, 1));
    drop(eager_clone);
    assert_eq!(
        policy.live(),
        (0, 1),
        "identity freed, original descriptors still shared"
    );
    let batches_clone = batches.clone();
    drop(batches);
    assert_eq!(policy.live(), (0, 1));
    drop(batches_clone);
    assert_eq!(policy.live(), (0, 0));
    assert!(
        array.upgrade().is_some(),
        "Arrow backing still lives in file views; this is a separate admission gap"
    );
    drop(view);
    assert!(array.upgrade().is_some());
    drop(view_clone);
    assert!(array.upgrade().is_none());
    assert_eq!(policy.identity_requests.load(Ordering::SeqCst), 1);
    assert_eq!(policy.batch_requests.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cache_collector_enforces_batch_and_row_bounds_with_typed_pressure() -> DeltaResult<()> {
    let (_directory, table) =
        table_with_actions(vec![add("first.parquet"), add("second.parquet")]).await?;
    for (max_batches, max_rows, kind) in [
        (0, 10, "materialized_batch_count"),
        (4, 1, "materialized_row_count"),
    ] {
        let policy = Arc::new(Admission::new(max_batches, max_rows));
        let result = EagerSnapshot::try_new_with_admission(
            table.log_store().as_ref(),
            Default::default(),
            Some(0),
            policy.clone(),
        )
        .await;
        assert_eq!(exhaustion(&result.unwrap_err()).kind, kind);
        assert_eq!(policy.live(), (0, 0));
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cache_descriptor_overflow_rejects_before_admission_or_stream_construction()
-> DeltaResult<()> {
    let (_directory, table) = table_with_actions(vec![add("first.parquet")]).await?;
    let policy = Arc::new(Admission::new(usize::MAX, 10));
    let snapshot = Snapshot::try_new(table.log_store().as_ref(), Default::default(), Some(0))
        .await?
        .with_materialized_files_admission(policy.clone())?;
    let calls = Arc::new(AtomicUsize::new(0));
    let error = Arc::new(snapshot)
        .try_materialize_files_with_engine(Arc::new(UntouchedEngine(calls.clone())))
        .await
        .unwrap_err();
    assert_eq!(
        exhaustion(&error).kind,
        "materialized_batch_descriptor_count"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(policy.identity_requests.load(Ordering::SeqCst), 0);
    assert_eq!(policy.live(), (0, 0));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cache_update_admits_new_identity_and_descriptors_and_reuse_does_not_clone_receipts()
-> DeltaResult<()> {
    use deltalake_core::kernel::transaction::{CommitBuilder, TableReference};
    use deltalake_core::protocol::{DeltaOperation, SaveMode};

    let (_directory, table) = table_with_actions(vec![add("first.parquet")]).await?;
    let log_store = table.log_store();
    let policy = Arc::new(Admission::new(4, 10));
    let original = Arc::new(
        Snapshot::try_new(log_store.as_ref(), Default::default(), Some(0))
            .await?
            .with_materialized_files_admission(policy.clone())?,
    )
    .try_materialize_files_with_engine(log_store.engine(None)?)
    .await?;
    assert_eq!(policy.live(), (1, 1));
    let reused = original
        .clone()
        .try_materialize_files_with_engine(Arc::new(UntouchedEngine(Default::default())))
        .await?;
    assert!(Arc::ptr_eq(&original, &reused));
    drop(reused);
    let old_metadata_ptr = original
        .retained_state()
        .materialized_files()
        .unwrap()
        .identity_metadata()
        .schema_string()
        .as_ptr();
    let version = CommitBuilder::default()
        .with_actions(vec![add("second.parquet")])
        .build(
            Some(table.snapshot()? as &dyn TableReference),
            log_store.clone(),
            DeltaOperation::Write {
                mode: SaveMode::Append,
                partition_by: None,
                predicate: None,
            },
        )
        .await?
        .version();
    let updated = original
        .clone()
        .update(log_store.engine(None)?, Some(version))
        .await?;
    assert_eq!(updated.version(), 1);
    assert_eq!(policy.live(), (2, 2));
    assert_ne!(
        old_metadata_ptr,
        updated
            .retained_state()
            .materialized_files()
            .unwrap()
            .identity_metadata()
            .schema_string()
            .as_ptr()
    );
    assert_eq!(
        updated
            .retained_state()
            .materialized_files()
            .unwrap()
            .batches()
            .iter()
            .map(|batch| batch.num_rows())
            .sum::<usize>(),
        2
    );
    drop(original);
    assert_eq!(policy.live(), (1, 1));
    drop(updated);
    assert_eq!(policy.live(), (0, 0));
    assert_eq!(policy.identity_requests.load(Ordering::SeqCst), 2);
    assert_eq!(policy.batch_requests.load(Ordering::SeqCst), 2);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cache_policy_cannot_be_attached_after_original_materialization() -> DeltaResult<()> {
    let (_directory, table) = table_with_actions(vec![add("first.parquet")]).await?;
    let eager =
        EagerSnapshot::try_new(table.log_store().as_ref(), Default::default(), Some(0)).await?;
    let policy = Arc::new(Admission::new(4, 10));
    let original = eager.retained_snapshot_owner().as_ref().clone();
    assert!(
        original
            .with_materialized_files_admission(policy.clone())
            .is_err()
    );
    assert_eq!(policy.live(), (0, 0));
    assert_eq!(policy.identity_requests.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cache_checkpoint_refresh_keeps_admission_and_acquires_new_identity_receipts()
-> DeltaResult<()> {
    let (_directory, table) = table_with_actions(vec![add("first.parquet")]).await?;
    let log_store = table.log_store();
    let policy = Arc::new(Admission::new(4, 10));
    let original = Arc::new(
        Snapshot::try_new(log_store.as_ref(), Default::default(), Some(0))
            .await?
            .with_materialized_files_admission(policy.clone())?,
    )
    .try_materialize_files_with_engine(log_store.engine(None)?)
    .await?;
    deltalake_core::checkpoints::create_checkpoint(&table, None).await?;
    let refreshed = original
        .clone()
        .update(log_store.engine(None)?, Some(0))
        .await?;
    assert_eq!(refreshed.version(), original.version());
    assert!(
        !Arc::ptr_eq(&refreshed, &original),
        "new checkpoint replaces the kernel snapshot"
    );
    assert_eq!(policy.live(), (2, 2));
    assert_eq!(policy.identity_requests.load(Ordering::SeqCst), 2);
    assert_eq!(policy.batch_requests.load(Ordering::SeqCst), 2);
    drop(original);
    assert_eq!(policy.live(), (1, 1));
    drop(refreshed);
    assert_eq!(policy.live(), (0, 0));
    Ok(())
}
