//! Workspace-owned change observation. Events request a census; they never establish facts.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use notify_debouncer_full::notify::{EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};
use tokio::sync::mpsc;

use crate::freshness::FreshnessBarrier;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SourceInventoryState {
    pub(crate) digest: [u8; 32],
    pub(crate) semantic_pending: bool,
}

pub(crate) async fn selected_inventory_state(
    epoch: &super::programmatic_epoch::ProgrammaticFabricEpoch,
    workspace: [u8; 16],
    generation: u64,
) -> Result<Option<SourceInventoryState>, String> {
    use arrow_array::{Array, BooleanArray, FixedSizeBinaryArray, UInt64Array};
    let id =
        super::programmatic_schema::ProgrammaticRelationId::new("source.input_inventory_state");
    let Some(binding) = epoch.relation(&id) else {
        return Ok(None);
    };
    let batches = epoch
        .context()
        .table(binding.table_reference.clone())
        .await
        .map_err(|error| error.to_string())?
        .limit(0, Some(2))
        .map_err(|error| error.to_string())?
        .collect()
        .await
        .map_err(|error| error.to_string())?;
    if batches
        .iter()
        .map(arrow_array::RecordBatch::num_rows)
        .sum::<usize>()
        != 1
    {
        return Err("selected source inventory must contain exactly one observation".to_owned());
    }
    let batch = batches
        .iter()
        .find(|batch| batch.num_rows() == 1)
        .expect("checked row count");
    let fixed = |name: &str| {
        batch
            .column_by_name(name)
            .and_then(|column| column.as_any().downcast_ref::<FixedSizeBinaryArray>())
            .filter(|column| column.null_count() == 0)
            .ok_or_else(|| format!("invalid inventory {name}"))
    };
    let generations = batch
        .column_by_name("source_generation")
        .and_then(|column| column.as_any().downcast_ref::<UInt64Array>())
        .ok_or("invalid inventory generation")?;
    if fixed("workspace_id")?.value(0) != workspace
        || generations.is_null(0)
        || generations.value(0) != generation
    {
        return Err("selected source inventory has another workspace/generation".to_owned());
    }
    let digest = fixed("inventory_digest")?
        .value(0)
        .try_into()
        .map_err(|_| "invalid inventory digest width".to_owned())?;
    // Exact epochs published before source-first staging are terminal provider observations.
    let semantic_pending = match batch.column_by_name("semantic_pending") {
        None => false,
        Some(column) => column
            .as_any()
            .downcast_ref::<BooleanArray>()
            .filter(|column| column.null_count() == 0)
            .ok_or("invalid inventory semantic stage")?
            .value(0),
    };
    Ok(Some(SourceInventoryState {
        digest,
        semantic_pending,
    }))
}

/// One coalesced reconciliation obligation survives an overflowing notification queue.
#[derive(Clone, Debug)]
pub(crate) struct WorkspaceObservation {
    pub(crate) freshness: FreshnessBarrier,
    pub(crate) source_freshness: FreshnessBarrier,
    wake: mpsc::Sender<()>,
    rescan_watermark: Arc<AtomicU64>,
    watch_healthy: Arc<AtomicBool>,
    events: Arc<AtomicU64>,
    published_generation: Arc<AtomicU64>,
}

impl WorkspaceObservation {
    pub(crate) fn new() -> (Self, mpsc::Receiver<()>) {
        let (wake, receiver) = mpsc::channel(1);
        (
            Self {
                freshness: FreshnessBarrier::default(),
                source_freshness: FreshnessBarrier::default(),
                wake,
                rescan_watermark: Arc::new(AtomicU64::new(1)),
                watch_healthy: Arc::new(AtomicBool::new(false)),
                events: Arc::new(AtomicU64::new(0)),
                published_generation: Arc::new(AtomicU64::new(0)),
            },
            receiver,
        )
    }

    /// Callback-safe: finite state, no source reads, parsing or blocking queue send.
    pub(crate) fn request(&self, rescan: bool) -> u64 {
        let watermark = self.freshness.admit();
        self.source_freshness.admit_through(watermark);
        if rescan {
            self.rescan_watermark.fetch_max(watermark, Ordering::AcqRel);
        }
        if self.wake.try_send(()).is_err() {
            self.rescan_watermark.fetch_max(watermark, Ordering::AcqRel);
        }
        watermark
    }

    fn event(&self, rescan: bool) {
        self.events.fetch_add(1, Ordering::AcqRel);
        self.request(rescan);
    }

    pub(crate) fn rescan_required(&self) -> bool {
        self.rescan_watermark.load(Ordering::Acquire) > self.source_freshness.reconciled()
    }

    pub(crate) fn watch_healthy(&self) -> bool {
        self.watch_healthy.load(Ordering::Acquire)
    }

    pub(crate) fn event_revision(&self) -> u64 {
        self.events.load(Ordering::Acquire)
    }

    pub(crate) fn state_for(&self, generation: u64) -> crate::freshness::FreshnessState {
        if self.freshness.state() == crate::freshness::FreshnessState::Unavailable {
            crate::freshness::FreshnessState::Unavailable
        } else if generation != self.published_generation.load(Ordering::Acquire) {
            crate::freshness::FreshnessState::PotentiallyStale
        } else {
            self.freshness.state()
        }
    }

    pub(crate) fn source_state_for(&self, generation: u64) -> crate::freshness::FreshnessState {
        if self.source_freshness.state() == crate::freshness::FreshnessState::Unavailable {
            crate::freshness::FreshnessState::Unavailable
        } else if generation != self.published_generation.load(Ordering::Acquire) {
            crate::freshness::FreshnessState::PotentiallyStale
        } else {
            self.source_freshness.state()
        }
    }

    pub(crate) fn source_reconciled(&self, watermark: u64, generation: u64) {
        self.published_generation
            .store(generation, Ordering::Release);
        self.source_freshness.restore(watermark);
    }

    pub(crate) fn reconciled(&self, watermark: u64, generation: u64) {
        self.source_reconciled(watermark, generation);
        self.published_generation
            .store(generation, Ordering::Release);
        self.freshness.restore(watermark);
    }

    pub(crate) async fn start_watch(
        &self,
        root: PathBuf,
        budget: &crate::resource_budget::ResourceBudget,
        parent: &crate::cancellation::StructuredCancellationScope,
    ) -> Result<WorkspaceWatchControl, String> {
        use crate::resource_budget::{ResourceAmounts, ResourceClass};
        let scope = parent
            .child_control("source-watch")
            .map_err(|error| error.to_string())?;
        let resources = budget
            .try_reserve(
                ResourceClass::Control,
                ResourceAmounts {
                    running_jobs: 1,
                    memory_bytes: 256 * 1024,
                    ..ResourceAmounts::default()
                },
            )
            .map_err(|error| error.to_string())?;
        let (ready, installed) = tokio::sync::oneshot::channel();
        let (commands, receiver) = std::sync::mpsc::sync_channel(1);
        let observed = self.clone();
        scope
            .spawn_blocking_owned("native-owner", resources, move |cancellation| {
                let mut watch = match observed.watch(&root) {
                    Ok(watch) => watch,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                if ready.send(Ok(())).is_err() {
                    watch.stop();
                    return;
                }
                while !cancellation.is_cancelled() {
                    match receiver.recv_timeout(Duration::from_millis(20)) {
                        Ok(()) => {
                            let healthy = watch.reinstall().is_ok();
                            observed.watch_healthy.store(healthy, Ordering::Release);
                            observed.request(true);
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
                watch.stop();
                observed.watch_healthy.store(false, Ordering::Release);
            })
            .await
            .map_err(|error| error.to_string())?;
        installed
            .await
            .map_err(|_| "native watcher ended before installation".to_owned())??;
        Ok(WorkspaceWatchControl { commands })
    }

    pub(crate) fn watch(&self, root: &Path) -> Result<WorkspaceWatch, String> {
        let observed = self.clone();
        let selected_root = root.to_owned();
        let mut watcher = new_debouncer(
            Duration::from_millis(75),
            Some(Duration::from_millis(20)),
            move |events: DebounceEventResult| {
                if let Ok(events) = events {
                    let rescan = events.iter().any(|event| event.need_rescan());
                    if rescan
                        || events.iter().any(|event| {
                            !matches!(event.kind, EventKind::Access(_))
                                && (event.paths.is_empty()
                                    || event
                                        .paths
                                        .iter()
                                        .any(|path| relevant(&selected_root, path)))
                        })
                    {
                        observed.event(rescan);
                    }
                } else {
                    observed.watch_healthy.store(false, Ordering::Release);
                    observed.event(true);
                }
            },
        )
        .map_err(|error| error.to_string())?;
        watcher
            .watch(root, RecursiveMode::Recursive)
            .map_err(|error| error.to_string())?;
        // Parent observation catches removal/recreation of the registered root itself.
        if let Some(parent) = root.parent() {
            watcher
                .watch(parent, RecursiveMode::NonRecursive)
                .map_err(|error| error.to_string())?;
        }
        self.watch_healthy.store(true, Ordering::Release);
        self.request(true);
        Ok(WorkspaceWatch {
            watcher: Some(watcher),
            root: root.to_owned(),
        })
    }
}

fn relevant(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    !relative.components().any(|part| matches!(part, Component::Normal(name) if
        matches!(name.as_encoded_bytes(), b"target" | b".venv" | b"node_modules" | b"__pycache__" | b".git")))
}

/// Bounded control channel; the structured blocking owner performs native joins.
pub(crate) struct WorkspaceWatchControl {
    commands: std::sync::mpsc::SyncSender<()>,
}

impl WorkspaceWatchControl {
    pub(crate) fn reinstall(&self) {
        let _ = self.commands.try_send(());
    }
}

/// The owning blocking operation stops and joins native watcher threads on shutdown.
pub(crate) struct WorkspaceWatch {
    watcher: Option<Debouncer<RecommendedWatcher, RecommendedCache>>,
    root: PathBuf,
}

impl WorkspaceWatch {
    pub(crate) fn reinstall(&mut self) -> Result<(), String> {
        let watcher = self.watcher.as_mut().ok_or("watcher stopped")?;
        let _ = watcher.unwatch(&self.root);
        watcher
            .watch(&self.root, RecursiveMode::Recursive)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn stop(mut self) {
        if let Some(watcher) = self.watcher.take() {
            watcher.stop();
        }
    }
}

impl Drop for WorkspaceWatch {
    fn drop(&mut self) {
        if let Some(watcher) = self.watcher.take() {
            watcher.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::freshness::FreshnessState;

    #[tokio::test]
    async fn overflowing_hints_retain_census_obligation_and_old_epochs_stay_stale() {
        let (observed, mut receiver) = WorkspaceObservation::new();
        let first = observed.request(false);
        observed.reconciled(first, 7);
        assert_eq!(observed.state_for(7), FreshnessState::Current);
        for _ in 0..100 {
            observed.event(false);
        }
        assert_eq!(receiver.len(), 1);
        assert!(observed.rescan_required());
        assert_eq!(observed.state_for(7), FreshnessState::PotentiallyStale);
        receiver.recv().await.unwrap();
        // Draining notifications is not reconciliation, nor is completing an older census.
        assert!(observed.rescan_required());
        observed.reconciled(first, 7);
        assert_eq!(observed.state_for(7), FreshnessState::PotentiallyStale);
        assert!(observed.rescan_required());
        observed.reconciled(observed.freshness.requested(), 8);
        assert_eq!(observed.state_for(8), FreshnessState::Current);
        assert_eq!(observed.state_for(7), FreshnessState::PotentiallyStale);
        assert!(!observed.rescan_required());
    }

    #[tokio::test]
    async fn source_publication_does_not_complete_the_semantic_barrier() {
        let (observed, _) = WorkspaceObservation::new();
        let first = observed.request(true);
        observed.reconciled(first, 1);
        observed.event(false);
        let edit = observed.freshness.requested();
        observed.source_reconciled(edit, 2);
        assert_eq!(observed.source_state_for(2), FreshnessState::Current);
        assert_eq!(observed.state_for(2), FreshnessState::PotentiallyStale);
        assert_eq!(
            observed.source_state_for(1),
            FreshnessState::PotentiallyStale
        );
        assert!(!observed.rescan_required());
        observed.reconciled(edit, 2);
        assert_eq!(observed.state_for(2), FreshnessState::Current);
        observed.event(false);
        observed.reconciled(edit, 2);
        assert_eq!(
            observed.source_state_for(2),
            FreshnessState::PotentiallyStale
        );
        assert_eq!(observed.state_for(2), FreshnessState::PotentiallyStale);
    }

    #[test]
    fn source_config_and_root_changes_are_relevant_but_build_caches_are_not() {
        let root = Path::new("/workspace");
        for path in [
            "/workspace",
            "/workspace/src/a.py",
            "/workspace/.cargo/config.toml",
            "/workspace/pyproject.toml",
        ] {
            assert!(relevant(root, Path::new(path)), "{path}");
        }
        for path in [
            "/other/a.py",
            "/workspace/target/a.rs",
            "/workspace/.venv/a.py",
            "/workspace/pkg/__pycache__/a.pyc",
            "/workspace/.git/index",
        ] {
            assert!(!relevant(root, Path::new(path)), "{path}");
        }
    }

    #[tokio::test]
    async fn native_watcher_observes_atomic_replacement_and_joins() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("source");
        std::fs::create_dir(&root).unwrap();
        let source = root.join("a.py");
        std::fs::write(&source, b"old").unwrap();
        let (observed, mut receiver) = WorkspaceObservation::new();
        let watch = observed.watch(&root).unwrap();
        receiver.recv().await.unwrap();
        let before = observed.event_revision();
        let temporary = root.join("save.tmp");
        std::fs::write(&temporary, b"new").unwrap();
        std::fs::rename(&temporary, &source).unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while observed.event_revision() == before {
                receiver.recv().await.unwrap();
            }
        })
        .await
        .unwrap();
        assert_eq!(observed.freshness.state(), FreshnessState::PotentiallyStale);
        watch.stop();
    }
}
