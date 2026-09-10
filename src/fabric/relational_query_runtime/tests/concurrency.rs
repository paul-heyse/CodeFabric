//! Controlled native provider scans prove bounded overlap without wall-clock timing assertions.

use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use datafusion::catalog::{ScanArgs, ScanResult, Session, TableProvider};
use datafusion::common::DataFusionError;
use datafusion::logical_expr::{Expr, TableType};
use datafusion::physical_plan::ExecutionPlan;
use tokio::sync::Semaphore;

use crate::fabric::streamed_result_package::{
    ObjectStoreResultSink, PendingResultObjectSet, ResultPublicationIntentError,
    StreamedResultPackageLimits,
};

#[derive(Debug, Default)]
struct ScanCounts {
    active: AtomicUsize,
    peak: AtomicUsize,
}

struct ActiveScan<'a>(&'a ScanCounts);

impl Drop for ActiveScan<'_> {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Debug)]
struct GatedProvider {
    native: Arc<dyn TableProvider>,
    enabled: AtomicBool,
    started: Semaphore,
    release: Semaphore,
    counts: Arc<ScanCounts>,
}

#[async_trait::async_trait]
impl TableProvider for GatedProvider {
    fn schema(&self) -> SchemaRef {
        self.native.schema()
    }
    fn table_type(&self) -> TableType {
        self.native.table_type()
    }
    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>, DataFusionError> {
        Ok(self
            .scan_with_args(
                state,
                ScanArgs::default()
                    .with_projection(projection.map(Vec::as_slice))
                    .with_filters(Some(filters))
                    .with_limit(limit),
            )
            .await?
            .into_inner())
    }
    async fn scan_with_args<'a>(
        &self,
        state: &dyn Session,
        args: ScanArgs<'a>,
    ) -> Result<ScanResult, DataFusionError> {
        let _active = if self.enabled.load(Ordering::SeqCst) {
            let active = self.counts.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.counts.peak.fetch_max(active, Ordering::SeqCst);
            let guard = ActiveScan(&self.counts);
            self.started.add_permits(1);
            self.release
                .acquire()
                .await
                .expect("test gate remains open")
                .forget();
            Some(guard)
        } else {
            None
        };
        self.native.scan_with_args(state, args).await
    }
}

#[derive(Debug)]
struct Recorder;

#[async_trait::async_trait]
impl ResultPublicationIntentRecorder for Recorder {
    async fn record_publication_intent(
        &self,
        _: PendingResultObjectSet,
    ) -> Result<(), ResultPublicationIntentError> {
        Ok(())
    }
}

#[tokio::test]
async fn ready_native_blocks_overlap_within_the_admitted_parallelism() {
    Box::pin(run_ready_blocks(false)).await;
}

#[tokio::test]
async fn cancelling_ready_native_blocks_drops_all_pending_planners() {
    Box::pin(run_ready_blocks(true)).await;
}

#[allow(clippy::too_many_lines)] // One native admission spans overlap, completion, publication and cleanup.
async fn run_ready_blocks(cancel: bool) {
    let workspace = WorkspaceId::from_bytes(id16(1));
    let epoch_id = EpochId::from_bytes(id16(20));
    let counts = Arc::new(ScanCounts::default());
    let mut builder =
        ProgrammaticFabricEpochBuilder::try_new(epoch_id, FabricEpochRuntimeConfig::default())
            .unwrap();
    let mut gates = Vec::new();
    let relations = ["query.a", "query.b", "query.c"];
    for relation in relations {
        let field = format!("{relation}.value");
        let mut input = provider_input(
            relation,
            TableReference::full(FABRIC_CATALOG, "fact", relation),
            &[("value", &field)],
            &[vec![relation]],
        );
        let gate = Arc::new(GatedProvider {
            native: input.provider,
            enabled: AtomicBool::new(false),
            started: Semaphore::new(0),
            release: Semaphore::new(0),
            counts: counts.clone(),
        });
        input.provider = gate.clone();
        gates.push(gate);
        builder.register_provider(input).unwrap();
    }
    let epoch = Arc::new(builder.seal_for_test().await.unwrap());
    for gate in &gates {
        gate.enabled.store(true, Ordering::SeqCst);
    }
    let (admission, _) = admitted_runtime(workspace, epoch.clone());
    let parallelism =
        ChildResourceLimits::try_new(8 * 1024 * 1024, 32 * 1024 * 1024, 4, 2, 128, 2).unwrap();
    let resources = resource_coordinator_with_limits(&epoch, parallelism);
    let runtime = test_query_runtime(workspace, admission.clone(), resources.clone());
    let mut authority = authorization(&epoch, &relations, 100);
    authority.child_resources =
        ChildResourceLimits::try_new(8 * 1024 * 1024, 32 * 1024 * 1024, 4, 2, 128, 2).unwrap();
    let cancellation = Cancellation::default();
    let transaction = RelationalQueryTransaction::try_new(
        owner(workspace, 0x31),
        QueryExecutionPin::from_bytes(id32(0x51)),
        authority,
        relations
            .iter()
            .rev()
            .map(|id| selected_output(&epoch, id))
            .collect(),
        ResultResourceLease::try_new(LeaseId::from_bytes(id16(0x61)), 1_000, 2_000).unwrap(),
        token(0x71),
        result_limits(),
        1_500,
        cancellation.clone(),
    )
    .unwrap()
    .with_canonical_semantic_response(b"{}".to_vec())
    .with_deadline(Instant::now() + Duration::from_secs(10));
    let budget = resources.resource_budget().clone();
    let packages = StreamedResultPackageBuilder::new(
        Arc::new(ObjectStoreResultSink::new(Arc::new(
            object_store::memory::InMemory::new(),
        ))),
        StreamedResultPackageLimits::try_new(
            8,
            64,
            10,
            64 * 1024,
            1024,
            4 * 1024 * 1024,
            128 * 1024,
            64,
            1024,
        )
        .unwrap(),
        budget.clone(),
    );
    let execute = Box::pin(runtime.execute_admitted_and_seal(
        admission.admit_selected(epoch).unwrap(),
        resources.clone(),
        transaction,
        &packages,
        Arc::new(Recorder),
    ));
    let coordinate = async {
        gates[0].started.acquire().await.unwrap().forget();
        gates[1].started.acquire().await.unwrap().forget();
        assert_eq!(counts.active.load(Ordering::SeqCst), 2);
        assert_eq!(gates[2].started.available_permits(), 0);
        if cancel {
            cancellation.cancel();
            return;
        }
        gates[1].release.add_permits(1);
        // The next root starts while the first remains blocked, with no wave barrier.
        gates[2].started.acquire().await.unwrap().forget();
        assert_eq!(counts.active.load(Ordering::SeqCst), 2);
        gates[2].release.add_permits(1);
        gates[0].release.add_permits(1);
    };
    let (publication, ()) = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::join!(execute, coordinate)
    })
    .await
    .expect("native ready scans must overlap");
    assert_eq!(counts.peak.load(Ordering::SeqCst), 2);
    assert_eq!(counts.active.load(Ordering::SeqCst), 0);
    if cancel {
        assert!(matches!(
            publication,
            Err(RelationalQueryRuntimeError::ResourceGovernance(
                EpochResourceError::Cancelled
            ))
        ));
        let observation = resources.observation().unwrap();
        assert_eq!(observation.active_work, 0);
        assert_eq!(observation.queued_work, 0);
        assert_eq!(observation.live_result_leases, 0);
        assert_eq!(budget.observation().used.disk_bytes, 0);
        return;
    }
    let publication = publication.unwrap();
    let manifest = publication.package().manifest();
    assert_eq!(manifest.total_rows, 3);
    assert_eq!(
        manifest
            .relations
            .iter()
            .map(|row| row.relation_id.as_str())
            .collect::<Vec<_>>(),
        relations
    );
    assert_eq!(
        publication
            .output_observations()
            .iter()
            .map(|row| row.relation_id.as_str())
            .collect::<Vec<_>>(),
        relations
    );
    let package = publication.into_released_package();
    package.delete_objects().await.unwrap();
    assert_eq!(budget.observation().used.disk_bytes, 0);
    assert_eq!(budget.observation().used.pages, 0);
}
