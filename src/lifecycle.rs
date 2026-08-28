//! Continuous-update lifecycle over current bytes, typed invalidation, and freshness barriers.
//!
//! Watcher and Git observations are hints only. Every admitted path is recaptured from the
//! registered root, and every accelerated result is fenced by the same canonical state digest
//! used by a full rebuild.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use arrow::util::display::array_value_to_string;
use arrow_array::RecordBatch;
use arrow_row::{RowConverter, SortField};
use arrow_schema::SchemaRef;
use notify_debouncer_full::notify::event::ModifyKind;
use notify_debouncer_full::notify::{Event, EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};
use petgraph::Direction;
use petgraph::algo::{tarjan_scc, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef as _;
use thiserror::Error;
use tokio::sync::{Notify, mpsc};

use crate::cancellation::Cancellation;
use crate::contracts::models::DeploymentProfileDocument;
use crate::core_facts::{CoreFactEngine, CoreFactError};
use crate::fabric::{
    ConsolidatedOverlay, FabricError, OverlayConsolidationRequest, OverlayMutation,
    ServingQueryError, ServingQuerySession,
};
use crate::fact_ingest::{CanonicalIngestOutput, FactScope};
use crate::git_state::{GitCandidatePlan, GitStateVector};
use crate::identity::PlatformCode;
use crate::model_generated::semantic_lane_fragments::SEMANTIC_INVALIDATION_CONTRACTS;
use crate::operational_store::{OperationalReader, OperationalStore, OperationalStoreError};
use crate::provider_types::ProviderText;
pub use crate::registries::FreshnessState;
use crate::registries::{
    EventStreamHealth, OperationalDependencyEdgeKind, OverlayTombstoneReason, PathEncoding,
    SourceTrustState, UPDATE_WAVE_STATE_TRANSITIONS, UPDATE_WAVE_STATE_VALUES,
    UpdateCandidateStrategy, UpdateWaveItemState, UpdateWaveState, generated_transition,
    registry_state_name,
};
use crate::ruff_adapter::{RuffAdapter, RuffSnapshot};
#[cfg(test)]
use crate::schema_registry::table_spec;
use crate::schema_registry::{
    ComparisonForeignKeyNormalization, MaterializationRole, OverlayMutationPolicy,
    comparison_projection_spec, serving_projection_specs, table_specs,
};
use crate::secure_path::PlatformPath;
use crate::source_image::{
    CaptureOutcome, CaptureRequest, SourceBlobHolderKind, SourceCapturePolicy, SourceImage,
    SourceImageError, SourceImageStore, SourceLanguage,
};
use crate::source_syntax::SourceSyntaxAdapterOutput;
use crate::tree_sitter_adapter::{
    TreeSitterAdapter, TreeSitterAdapterError, TreeSitterEdit, TreeSitterLanguage,
    TreeSitterSnapshot,
};

/// Deployment-owned watcher and continuous-update bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleConfig {
    pub debounce_timeout: std::time::Duration,
    pub tick_rate: std::time::Duration,
    pub ingress_capacity: usize,
    pub maximum_paths_per_batch: usize,
    pub gather_window: std::time::Duration,
    pub dirty_path_bulk_threshold: usize,
    pub await_current_timeout: std::time::Duration,
    pub maximum_capture_bytes: u64,
    pub stable_read_retry_count: u8,
    pub source_blob_lease_ttl: std::time::Duration,
    pub overlay_flush_policy: OverlayFlushPolicy,
}

impl LifecycleConfig {
    /// Resolve the runtime configuration from the closed deployment contract.
    ///
    /// # Errors
    ///
    /// Rejects zero capacities, impossible debounce relationships, or values that do not fit the
    /// platform's address space.
    pub fn from_deployment(profile: &DeploymentProfileDocument) -> Result<Self, LifecycleError> {
        let limits = profile.lifecycle_limits;
        let ingress_capacity = usize::from(limits.watch_ingress_capacity);
        let maximum_paths_per_batch = usize::try_from(limits.maximum_watch_paths_per_batch)
            .map_err(|_| LifecycleError::Configuration("watch path budget exceeds usize".into()))?;
        let dirty_path_bulk_threshold =
            usize::try_from(limits.dirty_path_bulk_threshold).map_err(|_| {
                LifecycleError::Configuration("dirty path threshold exceeds usize".into())
            })?;
        if limits.watch_debounce_timeout_ms == 0
            || limits.watch_tick_rate_ms == 0
            || limits.watch_tick_rate_ms > limits.watch_debounce_timeout_ms
            || ingress_capacity == 0
            || maximum_paths_per_batch == 0
            || dirty_path_bulk_threshold == 0
            || dirty_path_bulk_threshold > maximum_paths_per_batch
            || profile.source_image_limits.ordinary_maximum_bytes == 0
            || profile.source_image_limits.stable_read_retry_count == 0
            || limits.overlay_flush_maximum_rows == 0
            || limits.overlay_flush_maximum_bytes == 0
            || limits.overlay_flush_maximum_touched_owners == 0
            || limits.overlay_flush_maximum_generations == 0
        {
            return Err(LifecycleError::Configuration(
                "deployment lifecycle limits are inconsistent".into(),
            ));
        }
        Ok(Self {
            debounce_timeout: std::time::Duration::from_millis(limits.watch_debounce_timeout_ms),
            tick_rate: std::time::Duration::from_millis(limits.watch_tick_rate_ms),
            ingress_capacity,
            maximum_paths_per_batch,
            gather_window: std::time::Duration::from_millis(limits.gather_window_ms),
            dirty_path_bulk_threshold,
            await_current_timeout: std::time::Duration::from_millis(
                limits.default_await_current_timeout_ms,
            ),
            maximum_capture_bytes: profile.source_image_limits.ordinary_maximum_bytes,
            stable_read_retry_count: profile.source_image_limits.stable_read_retry_count,
            source_blob_lease_ttl: std::time::Duration::from_secs(
                profile.source_image_limits.orphan_grace_seconds,
            ),
            overlay_flush_policy: OverlayFlushPolicy {
                maximum_rows: limits.overlay_flush_maximum_rows,
                maximum_bytes: limits.overlay_flush_maximum_bytes,
                maximum_touched_owners: limits.overlay_flush_maximum_touched_owners,
                maximum_generations: limits.overlay_flush_maximum_generations,
            },
        })
    }

    #[must_use]
    pub const fn source_capture_policy(self) -> SourceCapturePolicy {
        SourceCapturePolicy {
            maximum_bytes: self.maximum_capture_bytes,
            stable_read_retries: self.stable_read_retry_count,
            lease_ttl: self.source_blob_lease_ttl,
        }
    }
}

/// Application-owned watcher hint; no notify value crosses this boundary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum WatchHintKind {
    CreateOrModify = 10,
    Remove = 20,
    RenameSource = 30,
    RenameTarget = 40,
    Unknown = 50,
}

/// Byte-native path hint relative to the registered root.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct WatchHint {
    pub path_bytes: Vec<u8>,
    pub kind: WatchHintKind,
}

/// One normalized backend batch. `rescan_required` supersedes every individual hint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchHintBatch {
    pub hints: Vec<WatchHint>,
    pub rescan_required: bool,
}

/// Bounded notify facade. Events never establish source truth.
pub struct WatcherFacade;

impl WatcherFacade {
    /// Normalize backend-dependent ordering and rename representation.
    ///
    /// # Errors
    ///
    /// Rejects paths outside the registered root or batches beyond the hard path budget.
    pub fn normalize(
        root: &Path,
        events: &[Event],
        maximum_paths_per_batch: usize,
    ) -> Result<WatchHintBatch, LifecycleError> {
        if events.iter().any(Event::need_rescan) {
            return Ok(WatchHintBatch {
                hints: Vec::new(),
                rescan_required: true,
            });
        }
        let mut hints = BTreeSet::new();
        for event in events {
            let rename = matches!(event.kind, EventKind::Modify(ModifyKind::Name(_)));
            for (index, path) in event.paths.iter().enumerate() {
                let relative = path.strip_prefix(root).map_err(|_| {
                    LifecycleError::Path("watch event escaped the registered root".into())
                })?;
                let path_bytes = safe_relative_bytes(relative)?;
                let kind = if rename && event.paths.len() >= 2 {
                    if index == 0 {
                        WatchHintKind::RenameSource
                    } else {
                        WatchHintKind::RenameTarget
                    }
                } else {
                    match event.kind {
                        EventKind::Create(_) | EventKind::Modify(_) => {
                            WatchHintKind::CreateOrModify
                        }
                        EventKind::Remove(_) => WatchHintKind::Remove,
                        EventKind::Access(_) | EventKind::Any | EventKind::Other => {
                            WatchHintKind::Unknown
                        }
                    }
                };
                hints.insert(WatchHint { path_bytes, kind });
                if hints.len() > maximum_paths_per_batch {
                    return Ok(WatchHintBatch {
                        hints: Vec::new(),
                        rescan_required: true,
                    });
                }
            }
        }
        Ok(WatchHintBatch {
            hints: hints.into_iter().collect(),
            rescan_required: false,
        })
    }
}

enum WatchDelivery {
    Events(Vec<Event>),
    ReconcileRequired,
}

/// Live recursive watcher whose callback performs only bounded non-blocking admission.
pub struct WorkspaceWatcher {
    root: PathBuf,
    maximum_paths_per_batch: usize,
    receiver: mpsc::Receiver<WatchDelivery>,
    overflowed: Arc<AtomicBool>,
    debouncer: Option<Debouncer<RecommendedWatcher, RecommendedCache>>,
}

impl WorkspaceWatcher {
    /// Start watching a registered root with deployment-owned bounds.
    ///
    /// # Errors
    ///
    /// Returns a stable watcher error if the root cannot be registered.
    pub fn start(root: &Path, config: LifecycleConfig) -> Result<Self, LifecycleError> {
        let root = fs::canonicalize(root).map_err(|source| LifecycleError::Io {
            path: root.to_owned(),
            source,
        })?;
        let (sender, receiver) = mpsc::channel(config.ingress_capacity);
        let overflowed = Arc::new(AtomicBool::new(false));
        let callback_overflowed = Arc::clone(&overflowed);
        let mut debouncer = new_debouncer(
            config.debounce_timeout,
            Some(config.tick_rate),
            move |result: DebounceEventResult| {
                let delivery = match result {
                    Ok(events) => {
                        WatchDelivery::Events(events.into_iter().map(|event| event.event).collect())
                    }
                    Err(_) => WatchDelivery::ReconcileRequired,
                };
                if sender.try_send(delivery).is_err() {
                    callback_overflowed.store(true, Ordering::Release);
                }
            },
        )
        .map_err(|error| LifecycleError::Watcher(error.to_string()))?;
        debouncer
            .watch(&root, RecursiveMode::Recursive)
            .map_err(|error| LifecycleError::Watcher(error.to_string()))?;
        Ok(Self {
            root,
            maximum_paths_per_batch: config.maximum_paths_per_batch,
            receiver,
            overflowed,
            debouncer: Some(debouncer),
        })
    }

    /// Receive one debounced application batch. Backend errors and callback overflow widen to a
    /// reconcile request; neither path fabricates file truth.
    ///
    /// # Errors
    ///
    /// Returns a watcher-closed error when the delivery channel ends unexpectedly.
    pub async fn next_batch(&mut self) -> Result<WatchHintBatch, LifecycleError> {
        if self.overflowed.swap(false, Ordering::AcqRel) {
            return Ok(reconcile_batch());
        }
        let delivery = self
            .receiver
            .recv()
            .await
            .ok_or(LifecycleError::WatcherClosed)?;
        if self.overflowed.swap(false, Ordering::AcqRel) {
            return Ok(reconcile_batch());
        }
        match delivery {
            WatchDelivery::Events(events) => {
                WatcherFacade::normalize(&self.root, &events, self.maximum_paths_per_batch)
            }
            WatchDelivery::ReconcileRequired => Ok(reconcile_batch()),
        }
    }

    /// Stop and join the debouncer thread before returning.
    pub fn shutdown(mut self) {
        if let Some(debouncer) = self.debouncer.take() {
            debouncer.stop();
        }
    }
}

impl Drop for WorkspaceWatcher {
    fn drop(&mut self) {
        if let Some(debouncer) = self.debouncer.take() {
            debouncer.stop_nonblocking();
        }
    }
}

fn reconcile_batch() -> WatchHintBatch {
    WatchHintBatch {
        hints: Vec::new(),
        rescan_required: true,
    }
}

fn safe_relative_bytes(path: &Path) -> Result<Vec<u8>, LifecycleError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(LifecycleError::Path("unsafe relative path".into()));
    }
    Ok(path.as_os_str().as_bytes().to_vec())
}

/// Coalesced dirty-path reason set.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DirtyReasonSet(BTreeSet<WatchHintKind>);

/// Actor-owned dirty registry with monotonic admitted sequence.
#[derive(Clone, Debug, Default)]
pub struct DirtyRegistry {
    dirty: BTreeMap<Vec<u8>, DirtyReasonSet>,
    admitted_sequence: u64,
    rescan_required: bool,
}

impl DirtyRegistry {
    /// Admit one normalized batch and coalesce repeated paths without loss.
    pub fn admit(&mut self, batch: WatchHintBatch) -> u64 {
        self.admitted_sequence = self.admitted_sequence.saturating_add(1);
        self.rescan_required |= batch.rescan_required;
        if self.rescan_required {
            self.dirty.clear();
        } else {
            for hint in batch.hints {
                self.dirty
                    .entry(hint.path_bytes)
                    .or_default()
                    .0
                    .insert(hint.kind);
            }
        }
        self.admitted_sequence
    }

    /// Freeze one update-wave candidate set. A rescan produces no misleading narrow paths.
    pub fn freeze(&mut self) -> FrozenDirtyWave {
        let paths = std::mem::take(&mut self.dirty);
        let rescan_required = std::mem::take(&mut self.rescan_required);
        FrozenDirtyWave {
            event_watermark: self.admitted_sequence,
            paths,
            rescan_required,
        }
    }

    #[must_use]
    pub const fn admitted_sequence(&self) -> u64 {
        self.admitted_sequence
    }

    fn restore(&mut self, wave: FrozenDirtyWave) {
        self.rescan_required |= wave.rescan_required;
        for (path, reasons) in wave.paths {
            self.dirty.entry(path).or_default().0.extend(reasons.0);
        }
    }

    fn restore_paths(&mut self, paths: impl IntoIterator<Item = Vec<u8>>) {
        for path in paths {
            self.dirty
                .entry(path)
                .or_default()
                .0
                .insert(WatchHintKind::Unknown);
        }
    }
}

/// Immutable input fence for one update wave.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrozenDirtyWave {
    pub event_watermark: u64,
    pub paths: BTreeMap<Vec<u8>, DirtyReasonSet>,
    pub rescan_required: bool,
}

/// Current-byte disposition for one persisted update-wave item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateWaveItem {
    pub path_bytes: Vec<u8>,
    pub state: UpdateWaveItemState,
    pub input_fingerprint: [u8; 32],
    pub output_fingerprint: Option<[u8; 32]>,
    pub captured: Option<Box<SourceImage>>,
}

/// One immutable, persisted wave input fence ready for classification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedUpdateWave {
    pub wave_id: [u8; 16],
    pub workspace_id: [u8; 16],
    pub source_generation: u64,
    pub event_watermark: u64,
    pub state: UpdateWaveState,
    pub candidate_strategy: UpdateCandidateStrategy,
    pub input_fingerprint: [u8; 32],
    pub candidate_state_vector: Option<GitStateVector>,
    pub items: Vec<UpdateWaveItem>,
}

/// Authoritative candidate-path selection supplied when a watcher batch widens beyond isolated
/// paths. A Git vector fences accelerated candidates; generic inventory has no Git dependency.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoritativeCandidateSelection {
    pub strategy: UpdateCandidateStrategy,
    pub paths: BTreeSet<Vec<u8>>,
    pub git_state_vector: Option<GitStateVector>,
}

impl AuthoritativeCandidateSelection {
    #[must_use]
    pub fn generic_inventory(paths: BTreeSet<Vec<u8>>) -> Self {
        Self {
            strategy: UpdateCandidateStrategy::GenericInventory,
            paths,
            git_state_vector: None,
        }
    }

    /// Convert a complete accelerated plan. Generic-inventory requests remain unresolved so the
    /// caller cannot mistake an empty fallback request for a complete inventory.
    #[must_use]
    pub fn from_git_plan(plan: GitCandidatePlan) -> Option<Self> {
        if plan.requires_generic_inventory() {
            return None;
        }
        Some(Self {
            strategy: plan.strategy,
            paths: plan.candidate_paths,
            git_state_vector: plan.state_vector,
        })
    }
}

/// Actor-owned dirty registry, source-generation allocator, and operational wave writer.
pub struct UpdateWaveScheduler {
    workspace_id: [u8; 16],
    root: PathBuf,
    config: LifecycleConfig,
    dirty: DirtyRegistry,
    freshness: FreshnessBarrier,
    next_source_generation: u64,
}

impl UpdateWaveScheduler {
    /// Create one scheduler from persisted monotonic counters.
    ///
    /// # Errors
    ///
    /// Rejects a source-generation overflow or an inaccessible root.
    pub fn new(
        workspace_id: [u8; 16],
        root: &Path,
        current_source_generation: u64,
        event_watermark: u64,
        reconciled_watermark: u64,
        config: LifecycleConfig,
    ) -> Result<Self, LifecycleError> {
        let root = fs::canonicalize(root).map_err(|source| LifecycleError::Io {
            path: root.to_owned(),
            source,
        })?;
        Ok(Self {
            workspace_id,
            root,
            config,
            dirty: DirtyRegistry {
                dirty: BTreeMap::new(),
                admitted_sequence: event_watermark,
                rescan_required: false,
            },
            freshness: FreshnessBarrier::with_watermarks(
                event_watermark,
                reconciled_watermark,
                false,
            ),
            next_source_generation: current_source_generation.checked_add(1).ok_or_else(|| {
                LifecycleError::Configuration("source generation exhausted".into())
            })?,
        })
    }

    /// Admit one normalized hint batch before any source or provider work.
    pub fn admit(&mut self, batch: WatchHintBatch) -> u64 {
        let watermark = self.dirty.admit(batch);
        let freshness_watermark = self.freshness.admit();
        debug_assert_eq!(watermark, freshness_watermark);
        watermark
    }

    /// Admit a batch and persist the query-visible stale/rescan state in the same coordinator
    /// turn.
    ///
    /// # Errors
    ///
    /// Returns a configuration or operational-store error when the watermark cannot be encoded or
    /// the registered workspace row cannot be advanced atomically.
    pub fn admit_persisted(
        &mut self,
        store: &mut OperationalStore,
        batch: WatchHintBatch,
    ) -> Result<u64, LifecycleError> {
        let rescan_required = batch.rescan_required;
        let watermark = self.admit(batch);
        let changed = store.write_transaction(|transaction| {
            Ok::<_, LifecycleError>(transaction.execute(
                "UPDATE worktree_state SET event_watermark=?1,source_trust_state_code=?2,event_stream_health_code=CASE WHEN ?3=1 THEN ?4 ELSE event_stream_health_code END,reconcile_required=CASE WHEN ?3=1 THEN 1 ELSE reconcile_required END,updated_at=?5 WHERE workspace_id=?6",
                rusqlite::params![
                    i64::try_from(watermark).map_err(|_| LifecycleError::Configuration("event watermark exceeds i64".into()))?,
                    i64::from(SourceTrustState::PotentiallyStale as u16),
                    i64::from(rescan_required),
                    i64::from(EventStreamHealth::RescanRequired as u16),
                    lifecycle_timestamp()?,
                    self.workspace_id.as_slice(),
                ],
            )?)
        })?;
        if changed != 1 {
            return Err(LifecycleError::Recovery(
                "workspace operational row is absent".into(),
            ));
        }
        Ok(watermark)
    }

    #[must_use]
    pub const fn freshness(&self) -> &FreshnessBarrier {
        &self.freshness
    }

    #[must_use]
    pub const fn workspace_id(&self) -> [u8; 16] {
        self.workspace_id
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub const fn config(&self) -> LifecycleConfig {
        self.config
    }

    #[must_use]
    pub const fn current_source_generation(&self) -> u64 {
        self.next_source_generation.saturating_sub(1)
    }

    /// Requeue a conservative reconcile after an accelerated candidate fence changes.
    ///
    /// # Errors
    ///
    /// Returns an operational-store or generated-transition error if the failed wave and new
    /// reconcile admission cannot both be persisted.
    pub fn reject_candidate_fence(
        &mut self,
        store: &mut OperationalStore,
        wave: &mut AcceptedUpdateWave,
    ) -> Result<(), LifecycleError> {
        self.transition(store, wave, "terminal-failure", "unrecoverable")?;
        self.admit_persisted(store, reconcile_batch())?;
        Ok(())
    }

    /// Restore unfinished current-byte work before a watcher begins delivering new hints.
    ///
    /// # Errors
    ///
    /// Returns an error when the recovery plan belongs to another workspace or its generation
    /// cannot be advanced safely.
    pub fn restore_recovery(
        &mut self,
        recovery: &WorkspaceRecoveryPlan,
    ) -> Result<(), LifecycleError> {
        if recovery.workspace_id != self.workspace_id {
            return Err(LifecycleError::Recovery(
                "recovery workspace differs from scheduler workspace".into(),
            ));
        }
        self.dirty
            .restore_paths(recovery.restart_paths.iter().cloned());
        self.dirty.rescan_required |= recovery.full_inventory_required;
        self.next_source_generation = recovery
            .source_generation
            .checked_add(1)
            .ok_or_else(|| LifecycleError::Configuration("source generation exhausted".into()))?;
        if !recovery.restart_paths.is_empty() || recovery.full_inventory_required {
            self.freshness.admitted.fetch_max(
                recovery.event_watermark.max(self.dirty.admitted_sequence),
                Ordering::AcqRel,
            );
        }
        Ok(())
    }

    /// Freeze, authoritatively capture, and persist one wave.
    ///
    /// `authoritative_candidates` is required when watcher uncertainty or the model-owned bulk
    /// threshold widens the wave. The caller obtains it from gix or the generic inventory; this
    /// scheduler still recaptures every current byte itself.
    ///
    /// # Errors
    ///
    /// Returns a source, budget, store, or inventory-required error. A failed call restores every
    /// frozen dirty path so no invalidation is lost.
    #[allow(clippy::too_many_lines)] // One scheduler transaction keeps freeze, capture, and durable admission visibly ordered.
    pub fn prepare_wave(
        &mut self,
        store: &mut OperationalStore,
        source_images: &mut SourceImageStore,
        authoritative_candidates: Option<AuthoritativeCandidateSelection>,
    ) -> Result<Option<AcceptedUpdateWave>, LifecycleError> {
        let frozen = self.dirty.freeze();
        if frozen.paths.is_empty() && !frozen.rescan_required {
            return Ok(None);
        }
        let widened =
            frozen.rescan_required || frozen.paths.len() >= self.config.dirty_path_bulk_threshold;
        let (candidate_strategy, candidate_paths, candidate_state_vector) = if widened {
            let Some(selection) = authoritative_candidates else {
                self.dirty.restore(frozen);
                return Err(LifecycleError::AuthoritativeInventoryRequired);
            };
            if selection.strategy == UpdateCandidateStrategy::IsolatedPaths {
                self.dirty.restore(frozen);
                return Err(LifecycleError::Configuration(
                    "widened wave cannot use isolated candidate strategy".into(),
                ));
            }
            (
                selection.strategy,
                selection.paths,
                selection.git_state_vector,
            )
        } else {
            (
                UpdateCandidateStrategy::IsolatedPaths,
                frozen.paths.keys().cloned().collect(),
                None,
            )
        };
        if candidate_paths.len() > self.config.maximum_paths_per_batch {
            self.dirty.restore(frozen);
            return Err(LifecycleError::Configuration(
                "candidate set exceeds deployment path budget".into(),
            ));
        }
        let source_generation = self.next_source_generation;
        let input_fingerprint = wave_input_fingerprint(
            self.workspace_id,
            source_generation,
            frozen.event_watermark,
            candidate_strategy,
            &candidate_paths,
        );
        let wave_id = wave_identity(
            self.workspace_id,
            source_generation,
            frozen.event_watermark,
            input_fingerprint,
        );
        let snapshotting = snapshotting_state(candidate_strategy)?;
        persist_wave_header(
            store,
            wave_id,
            self.workspace_id,
            source_generation,
            frozen.event_watermark,
            snapshotting,
            candidate_strategy,
            input_fingerprint,
        )?;
        let mut items = Vec::with_capacity(candidate_paths.len());
        let mut deferred = Vec::new();
        for path_bytes in candidate_paths {
            let item_input = wave_item_input_fingerprint(&path_bytes);
            let relative =
                PlatformPath::from_raw_relative_bytes(host_platform_code(), path_bytes.clone())?;
            let path = self
                .root
                .join(Path::new(std::ffi::OsStr::from_bytes(&path_bytes)));
            let item = match fs::symlink_metadata(&path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => UpdateWaveItem {
                    state: UpdateWaveItemState::Removed,
                    input_fingerprint: item_input,
                    output_fingerprint: Some(removal_fingerprint(&path_bytes)),
                    captured: None,
                    path_bytes,
                },
                Err(source) => return Err(LifecycleError::Io { path, source }),
                Ok(_) => match source_images.capture(
                    store,
                    &CaptureRequest {
                        workspace_id: self.workspace_id,
                        source_generation,
                        change_token: frozen.event_watermark,
                        path: relative,
                        language: source_language_for_path(&path_bytes),
                        holder_kind: SourceBlobHolderKind::ProviderRun,
                        holder_id: wave_id,
                    },
                )? {
                    CaptureOutcome::Published(captured) => UpdateWaveItem {
                        path_bytes,
                        state: UpdateWaveItemState::Captured,
                        input_fingerprint: item_input,
                        output_fingerprint: Some(captured.digest),
                        captured: Some(captured),
                    },
                    CaptureOutcome::Deferred => {
                        deferred.push(path_bytes.clone());
                        UpdateWaveItem {
                            path_bytes,
                            state: UpdateWaveItemState::DeferredSourceDrift,
                            input_fingerprint: item_input,
                            output_fingerprint: None,
                            captured: None,
                        }
                    }
                    CaptureOutcome::Excluded(_) => UpdateWaveItem {
                        path_bytes,
                        state: UpdateWaveItemState::Failed,
                        input_fingerprint: item_input,
                        output_fingerprint: None,
                        captured: None,
                    },
                },
            };
            items.push(item);
        }
        let state = if deferred.is_empty() {
            transition_state(snapshotting, "source-images-accepted", "reads-stable")?
        } else {
            UpdateWaveState::Failed
        };
        let wave = AcceptedUpdateWave {
            wave_id,
            workspace_id: self.workspace_id,
            source_generation,
            event_watermark: frozen.event_watermark,
            state,
            candidate_strategy,
            input_fingerprint,
            candidate_state_vector,
            items,
        };
        persist_wave_items(store, &wave)?;
        self.next_source_generation = self
            .next_source_generation
            .checked_add(1)
            .ok_or_else(|| LifecycleError::Configuration("source generation exhausted".into()))?;
        self.dirty.restore_paths(deferred);
        Ok(Some(wave))
    }

    /// Persist a generated state-machine transition and advance freshness only after hot
    /// publication is validated.
    ///
    /// # Errors
    ///
    /// Returns an error for an unregistered transition, stale persisted state, or an
    /// operational-store failure.
    pub fn transition(
        &self,
        store: &mut OperationalStore,
        wave: &mut AcceptedUpdateWave,
        event: &str,
        guard: &str,
    ) -> Result<(), LifecycleError> {
        let next = transition_state(wave.state, event, guard)?;
        store.write_transaction(|transaction| {
            let changed = transaction.execute(
                "UPDATE update_wave SET state_code=?1,terminal_at=CASE WHEN ?1 IN (?2,?3,?4,?5) THEN ?6 ELSE NULL END WHERE wave_id=?7 AND state_code=?8",
                rusqlite::params![
                    i64::from(next as u16),
                    i64::from(UpdateWaveState::DurablePublished as u16),
                    i64::from(UpdateWaveState::Failed as u16),
                    i64::from(UpdateWaveState::Superseded as u16),
                    i64::from(UpdateWaveState::Cancelled as u16),
                    lifecycle_timestamp()?,
                    wave.wave_id.as_slice(),
                    i64::from(wave.state as u16),
                ],
            )?;
            if changed != 1 {
                return Err(LifecycleError::Recovery(
                    "persisted wave state changed outside the coordinator".into(),
                ));
            }
            if next == UpdateWaveState::HotPublished
                || next == UpdateWaveState::DurablePublished
            {
                let durable_generation = if next == UpdateWaveState::DurablePublished {
                    i64::try_from(wave.source_generation).map_err(|_| {
                        LifecycleError::Configuration("source generation exceeds i64".into())
                    })?
                } else {
                    -1
                };
                transaction.execute(
                    "UPDATE worktree_state SET source_trust_state_code=CASE WHEN event_watermark=?1 THEN ?2 ELSE source_trust_state_code END,reconcile_required=CASE WHEN event_watermark=?1 THEN 0 ELSE reconcile_required END,event_stream_health_code=CASE WHEN event_watermark=?1 AND event_stream_health_code=?3 THEN ?4 ELSE event_stream_health_code END,durable_generation=CASE WHEN ?5>=0 THEN ?5 ELSE durable_generation END,updated_at=?6 WHERE workspace_id=?7 AND source_generation=?8",
                    rusqlite::params![
                        i64::try_from(wave.event_watermark).map_err(|_| LifecycleError::Configuration("event watermark exceeds i64".into()))?,
                        i64::from(SourceTrustState::Current as u16),
                        i64::from(EventStreamHealth::RescanRequired as u16),
                        i64::from(EventStreamHealth::Healthy as u16),
                        durable_generation,
                        lifecycle_timestamp()?,
                        wave.workspace_id.as_slice(),
                        i64::try_from(wave.source_generation).map_err(|_| LifecycleError::Configuration("source generation exceeds i64".into()))?,
                    ],
                )?;
            }
            Ok::<_, LifecycleError>(())
        })?;
        wave.state = next;
        if next == UpdateWaveState::HotPublished || next == UpdateWaveState::DurablePublished {
            self.freshness.reconcile(wave.event_watermark);
        }
        Ok(())
    }
}

fn transition_state(
    state: UpdateWaveState,
    event: &str,
    guard: &str,
) -> Result<UpdateWaveState, LifecycleError> {
    let state_name = registry_state_name(UPDATE_WAVE_STATE_VALUES, state as u16)
        .ok_or_else(|| LifecycleError::Recovery("generated wave state is absent".into()))?;
    let transition = generated_transition(UPDATE_WAVE_STATE_TRANSITIONS, state_name, event, guard)
        .map_err(|violation| {
            LifecycleError::Transition(format!(
                "{}:{}:{}",
                violation.prior_state, violation.event, violation.guard
            ))
        })?;
    let code = UPDATE_WAVE_STATE_VALUES
        .iter()
        .find(|entry| entry.name == transition.to)
        .map(|entry| entry.code)
        .ok_or_else(|| LifecycleError::Recovery("generated transition target is absent".into()))?;
    UpdateWaveState::try_from(code)
        .map_err(|_| LifecycleError::Recovery("generated transition code is invalid".into()))
}

fn snapshotting_state(
    strategy: UpdateCandidateStrategy,
) -> Result<UpdateWaveState, LifecycleError> {
    if matches!(
        strategy,
        UpdateCandidateStrategy::GitStatusIndex | UpdateCandidateStrategy::HeadTreeAndStatus
    ) {
        let building = transition_state(
            UpdateWaveState::Collecting,
            "git-candidates-required",
            "git-ready",
        )?;
        let verifying = transition_state(building, "candidates-built", "candidate-set-bounded")?;
        transition_state(verifying, "git-baseline-stable", "baseline-unchanged")
    } else {
        transition_state(
            UpdateWaveState::Collecting,
            "gather-barrier-closed",
            "stable-window",
        )
    }
}

fn wave_input_fingerprint(
    workspace_id: [u8; 16],
    source_generation: u64,
    event_watermark: u64,
    strategy: UpdateCandidateStrategy,
    paths: &BTreeSet<Vec<u8>>,
) -> [u8; 32] {
    let mut hasher = crate::identity::semantic_fingerprint(
        crate::identity::SemanticFingerprintDomain::UpdateWaveInput,
    );
    hasher.update(&workspace_id);
    hasher.update(&source_generation.to_be_bytes());
    hasher.update(&event_watermark.to_be_bytes());
    hasher.update(&(strategy as u16).to_be_bytes());
    for path in paths {
        hasher.update(&(path.len() as u64).to_be_bytes());
        hasher.update(path);
    }
    hasher.finalize()
}

fn wave_item_input_fingerprint(path: &[u8]) -> [u8; 32] {
    let mut hasher = crate::identity::semantic_fingerprint(
        crate::identity::SemanticFingerprintDomain::UpdateWaveItemInput,
    );
    hasher.update(&(path.len() as u64).to_be_bytes());
    hasher.update(path);
    hasher.finalize()
}

fn removal_fingerprint(path: &[u8]) -> [u8; 32] {
    let mut hasher = crate::identity::semantic_fingerprint(
        crate::identity::SemanticFingerprintDomain::UpdateWaveRemoval,
    );
    hasher.update(path);
    hasher.finalize()
}

fn wave_identity(
    workspace_id: [u8; 16],
    source_generation: u64,
    event_watermark: u64,
    input_fingerprint: [u8; 32],
) -> [u8; 16] {
    let mut hasher = crate::identity::semantic_fingerprint(
        crate::identity::SemanticFingerprintDomain::UpdateWave,
    );
    hasher.update(&workspace_id);
    hasher.update(&source_generation.to_be_bytes());
    hasher.update(&event_watermark.to_be_bytes());
    hasher.update(&input_fingerprint);
    hasher.finalize_id16()
}

#[allow(clippy::too_many_arguments)]
fn persist_wave_header(
    store: &mut OperationalStore,
    wave_id: [u8; 16],
    workspace_id: [u8; 16],
    source_generation: u64,
    event_watermark: u64,
    state: UpdateWaveState,
    candidate_strategy: UpdateCandidateStrategy,
    input_fingerprint: [u8; 32],
) -> Result<(), LifecycleError> {
    let timestamp = lifecycle_timestamp()?;
    let expected_generation = source_generation.checked_sub(1).ok_or_else(|| {
        LifecycleError::Configuration("source generation must be positive".into())
    })?;
    let source_generation = i64::try_from(source_generation)
        .map_err(|_| LifecycleError::Configuration("source generation exceeds i64".into()))?;
    let expected_generation = i64::try_from(expected_generation)
        .map_err(|_| LifecycleError::Configuration("source generation exceeds i64".into()))?;
    let event_watermark = i64::try_from(event_watermark)
        .map_err(|_| LifecycleError::Configuration("event watermark exceeds i64".into()))?;
    store.write_transaction(|transaction| {
        let generation = transaction.execute(
            "UPDATE workspace_generation SET source_generation=?1,updated_at=?2 WHERE workspace_id=?3 AND source_generation=?4",
            rusqlite::params![source_generation, timestamp, workspace_id.as_slice(), expected_generation],
        )?;
        let worktree = transaction.execute(
            "UPDATE worktree_state SET source_generation=?1,event_watermark=?2,newest_dirty_generation=?1,source_trust_state_code=?3,reconcile_required=?4,updated_at=?5 WHERE workspace_id=?6 AND source_generation=?7",
            rusqlite::params![
                source_generation,
                event_watermark,
                i64::from(SourceTrustState::Verifying as u16),
                i64::from(candidate_strategy == UpdateCandidateStrategy::GenericInventory),
                timestamp,
                workspace_id.as_slice(),
                expected_generation,
            ],
        )?;
        if generation != 1 || worktree != 1 {
            return Err(LifecycleError::Recovery(
                "workspace generation changed before wave admission".into(),
            ));
        }
        transaction.execute(
            "INSERT INTO update_wave(wave_id,workspace_id,source_generation,event_watermark,state_code,candidate_strategy_code,input_fingerprint,candidate_count,started_at,terminal_at,diagnostic_id) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,CASE WHEN ?5=?10 THEN ?9 ELSE NULL END,NULL)",
            rusqlite::params![
                wave_id.as_slice(),
                workspace_id.as_slice(),
                source_generation,
                event_watermark,
                i64::from(state as u16),
                i64::from(candidate_strategy as u16),
                input_fingerprint.as_slice(),
                0_i64,
                timestamp,
                i64::from(UpdateWaveState::Failed as u16),
            ],
        )?;
        Ok::<_, LifecycleError>(())
    })
}

fn persist_wave_items(
    store: &mut OperationalStore,
    wave: &AcceptedUpdateWave,
) -> Result<(), LifecycleError> {
    let timestamp = lifecycle_timestamp()?;
    store.write_transaction(|transaction| {
        for (ordinal, item) in wave.items.iter().enumerate() {
            transaction.execute(
                "INSERT INTO update_wave_item(wave_id,item_ordinal,path_bytes,path_display,path_encoding_code,state_code,input_fingerprint,output_fingerprint,diagnostic_id) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,NULL)",
                rusqlite::params![
                    wave.wave_id.as_slice(),
                    i64::try_from(ordinal).map_err(|_| LifecycleError::Configuration("wave item ordinal exceeds i64".into()))?,
                    &item.path_bytes,
                    String::from_utf8_lossy(&item.path_bytes),
                    i64::from(platform_path_encoding() as u16),
                    i64::from(item.state as u16),
                    item.input_fingerprint.as_slice(),
                    item.output_fingerprint.as_ref().map(<[u8; 32]>::as_slice),
                ],
            )?;
        }
        let changed = transaction.execute(
            "UPDATE update_wave SET state_code=?1,candidate_count=?2,terminal_at=CASE WHEN ?1=?3 THEN ?4 ELSE NULL END WHERE wave_id=?5 AND state_code=?6",
            rusqlite::params![
                i64::from(wave.state as u16),
                i64::try_from(wave.items.len()).map_err(|_| LifecycleError::Configuration("wave item count exceeds i64".into()))?,
                i64::from(UpdateWaveState::Failed as u16),
                timestamp,
                wave.wave_id.as_slice(),
                i64::from(UpdateWaveState::Snapshotting as u16),
            ],
        )?;
        if changed != 1 {
            return Err(LifecycleError::Recovery(
                "wave changed before source capture completed".into(),
            ));
        }
        Ok::<_, LifecycleError>(())
    })
}

#[cfg(target_os = "macos")]
const fn platform_path_encoding() -> PathEncoding {
    PathEncoding::MacosBytes
}

#[cfg(not(target_os = "macos"))]
const fn platform_path_encoding() -> PathEncoding {
    PathEncoding::UnixBytes
}

fn lifecycle_timestamp() -> Result<String, LifecycleError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| LifecycleError::Recovery("system clock precedes Unix epoch".into()))?
        .as_millis();
    Ok(millis.to_string())
}

#[cfg(target_os = "macos")]
const fn host_platform_code() -> PlatformCode {
    PlatformCode::MacOs
}

#[cfg(not(target_os = "macos"))]
const fn host_platform_code() -> PlatformCode {
    PlatformCode::Unix
}

fn source_language_for_path(path: &[u8]) -> SourceLanguage {
    if path.ends_with(b".py") || path.ends_with(b".pyi") {
        SourceLanguage::Python
    } else if path.ends_with(b".rs") {
        SourceLanguage::Rust
    } else {
        SourceLanguage::Other
    }
}

/// Generated typed reason an owner depends on another owner.
pub type DependencyEdgeKind = OperationalDependencyEdgeKind;

/// One persisted prerequisite -> dependent edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyEdge {
    pub source_owner_id: [u8; 16],
    pub dependent_owner_id: [u8; 16],
    pub kind: DependencyEdgeKind,
    pub derivation_id: Option<String>,
    pub source_generation: i64,
    pub input_digest: [u8; 32],
}

/// Stable cycle diagnostic including both owners and the typed internal edges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyCycleWitness {
    pub owners: Vec<[u8; 16]>,
    pub edges: Vec<([u8; 16], [u8; 16], DependencyEdgeKind)>,
}

/// Conservative invalidation result. Unknown owners widen to every known graph owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidationPlan {
    pub affected_owners: BTreeSet<[u8; 16]>,
    pub unknown_changed_owners: BTreeSet<[u8; 16]>,
    pub traversed_edges: Vec<DependencyEdge>,
    pub invalidated_table_codes: BTreeSet<i16>,
    pub full_rebuild_required: bool,
}

/// Deterministic operational dependency graph. Edges point prerequisite -> dependent.
pub struct OperationalDependencyGraph {
    graph: DiGraph<[u8; 16], DependencyEdgeKind>,
    indices: BTreeMap<[u8; 16], NodeIndex>,
    records: Vec<DependencyEdge>,
}

impl OperationalDependencyGraph {
    /// Build a closed acyclic graph with deterministic duplicate and cycle diagnostics.
    ///
    /// # Errors
    ///
    /// Returns an error for self-edges, duplicate/conflicting dependencies, or any graph cycle.
    pub fn new(mut records: Vec<DependencyEdge>) -> Result<Self, LifecycleError> {
        records.sort_by(|left, right| {
            (
                left.source_owner_id,
                left.dependent_owner_id,
                left.kind as u16,
                &left.derivation_id,
                left.source_generation,
                left.input_digest,
            )
                .cmp(&(
                    right.source_owner_id,
                    right.dependent_owner_id,
                    right.kind as u16,
                    &right.derivation_id,
                    right.source_generation,
                    right.input_digest,
                ))
        });
        if records.windows(2).any(|pair| {
            pair[0].source_owner_id == pair[1].source_owner_id
                && pair[0].dependent_owner_id == pair[1].dependent_owner_id
                && pair[0].kind == pair[1].kind
        }) {
            return Err(LifecycleError::Graph(
                "duplicate typed dependency edge".into(),
            ));
        }
        let owners = records
            .iter()
            .flat_map(|edge| [edge.source_owner_id, edge.dependent_owner_id])
            .collect::<BTreeSet<_>>();
        let mut graph = DiGraph::new();
        let indices = owners
            .into_iter()
            .map(|owner| (owner, graph.add_node(owner)))
            .collect::<BTreeMap<_, _>>();
        for record in &records {
            graph.add_edge(
                indices[&record.source_owner_id],
                indices[&record.dependent_owner_id],
                record.kind,
            );
        }
        if toposort(&graph, None).is_err() {
            let mut owners = tarjan_scc(&graph)
                .into_iter()
                .filter(|component| {
                    component.len() > 1
                        || component.first().is_some_and(|node| {
                            graph.edges_connecting(*node, *node).next().is_some()
                        })
                })
                .map(|component| {
                    let mut owners = component
                        .into_iter()
                        .map(|node| graph[node])
                        .collect::<Vec<_>>();
                    owners.sort_unstable();
                    owners
                })
                .min()
                .unwrap_or_default();
            owners.sort_unstable();
            let owner_set = owners.iter().copied().collect::<BTreeSet<_>>();
            let edges = records
                .iter()
                .filter(|edge| {
                    owner_set.contains(&edge.source_owner_id)
                        && owner_set.contains(&edge.dependent_owner_id)
                })
                .map(|edge| (edge.source_owner_id, edge.dependent_owner_id, edge.kind))
                .collect();
            return Err(LifecycleError::Cycle(DependencyCycleWitness {
                owners,
                edges,
            }));
        }
        Ok(Self {
            graph,
            indices,
            records,
        })
    }

    /// Conservative outgoing dependent closure, including every changed owner.
    #[must_use]
    pub fn affected_closure(&self, changed: &BTreeSet<[u8; 16]>) -> BTreeSet<[u8; 16]> {
        self.plan_invalidation(changed).affected_owners
    }

    /// Produce a deterministic, explainable invalidation plan.
    #[must_use]
    pub fn plan_invalidation(&self, changed: &BTreeSet<[u8; 16]>) -> InvalidationPlan {
        let unknown_changed_owners = changed
            .iter()
            .filter(|owner| !self.indices.contains_key(*owner))
            .copied()
            .collect::<BTreeSet<_>>();
        let mut affected = if unknown_changed_owners.is_empty() {
            changed.clone()
        } else {
            self.indices
                .keys()
                .copied()
                .chain(changed.iter().copied())
                .collect()
        };
        let mut queue = changed.iter().copied().collect::<VecDeque<_>>();
        while let Some(owner) = queue.pop_front() {
            let Some(&index) = self.indices.get(&owner) else {
                continue;
            };
            let mut dependents = self
                .graph
                .edges_directed(index, Direction::Outgoing)
                .map(|edge| self.graph[edge.target()])
                .collect::<Vec<_>>();
            dependents.sort_unstable();
            for dependent in dependents {
                if affected.insert(dependent) {
                    queue.push_back(dependent);
                }
            }
        }
        let traversed_edges = self
            .records
            .iter()
            .filter(|edge| {
                affected.contains(&edge.source_owner_id)
                    && affected.contains(&edge.dependent_owner_id)
            })
            .cloned()
            .collect();
        InvalidationPlan {
            affected_owners: affected,
            unknown_changed_owners: unknown_changed_owners.clone(),
            traversed_edges,
            invalidated_table_codes: if changed.is_empty() {
                BTreeSet::new()
            } else {
                SEMANTIC_INVALIDATION_CONTRACTS
                    .iter()
                    .flat_map(|contract| contract.invalidated_table_codes.iter().copied())
                    .collect()
            },
            full_rebuild_required: !unknown_changed_owners.is_empty(),
        }
    }

    /// Replace one workspace graph atomically in the model-generated operational table.
    ///
    /// # Errors
    ///
    /// Returns a conversion or operational-store error; no partial graph replacement is visible.
    pub fn persist(
        &self,
        store: &mut OperationalStore,
        workspace_id: [u8; 16],
    ) -> Result<(), LifecycleError> {
        store.write_transaction(|transaction| {
            transaction.execute(
                "DELETE FROM operational_dependency_edge WHERE workspace_id=?1",
                [workspace_id.as_slice()],
            )?;
            for edge in &self.records {
                transaction.execute(
                    "INSERT INTO operational_dependency_edge(
                       workspace_id,source_owner_id,dependent_owner_id,edge_kind_code,
                       derivation_id,source_generation,input_digest,active
                     ) VALUES (?1,?2,?3,?4,?5,?6,?7,1)",
                    rusqlite::params![
                        workspace_id.as_slice(),
                        edge.source_owner_id.as_slice(),
                        edge.dependent_owner_id.as_slice(),
                        i64::from(edge.kind as u16),
                        edge.derivation_id,
                        edge.source_generation,
                        edge.input_digest.as_slice(),
                    ],
                )?;
            }
            Ok::<_, LifecycleError>(())
        })
    }

    /// Load the exact active graph through a read-only operational snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed persisted identities or kinds, storage failure, or an
    /// invalid reconstructed graph.
    pub fn load(
        reader: &OperationalReader,
        workspace_id: [u8; 16],
    ) -> Result<Self, LifecycleError> {
        let records = reader.with_connection_result(|connection| {
            let mut statement = connection.prepare(
                "SELECT source_owner_id,dependent_owner_id,edge_kind_code,derivation_id,
                        source_generation,input_digest
                   FROM operational_dependency_edge
                  WHERE workspace_id=?1 AND active=1
                  ORDER BY source_owner_id,dependent_owner_id,edge_kind_code",
            )?;
            statement
                .query_map([workspace_id.as_slice()], |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, Vec<u8>>(5)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()
                .map_err(LifecycleError::from)
        })?;
        let records = records
            .into_iter()
            .map(
                |(source, dependent, kind, derivation_id, source_generation, digest)| {
                    Ok(DependencyEdge {
                        source_owner_id: exact_bytes(source, "source owner")?,
                        dependent_owner_id: exact_bytes(dependent, "dependent owner")?,
                        kind: DependencyEdgeKind::try_from(u16::try_from(kind).map_err(|_| {
                            LifecycleError::Graph("dependency edge code exceeds u16".into())
                        })?)
                        .map_err(|_| {
                            LifecycleError::Graph("unknown dependency edge kind".into())
                        })?,
                        derivation_id,
                        source_generation,
                        input_digest: exact_bytes(digest, "input digest")?,
                    })
                },
            )
            .collect::<Result<Vec<_>, LifecycleError>>()?;
        Self::new(records)
    }
}

fn exact_bytes<const N: usize>(value: Vec<u8>, label: &str) -> Result<[u8; N], LifecycleError> {
    value
        .try_into()
        .map_err(|_| LifecycleError::Graph(format!("{label} has invalid width")))
}

/// Query-side admission policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FreshnessAdmission {
    BestAvailable,
    AwaitLatest,
    RequireCurrent,
}

/// Sole workspace freshness barrier.
#[derive(Clone, Debug)]
pub struct FreshnessBarrier {
    admitted: Arc<AtomicU64>,
    reconciled: Arc<AtomicU64>,
    unavailable: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl Default for FreshnessBarrier {
    fn default() -> Self {
        Self {
            admitted: Arc::new(AtomicU64::new(0)),
            reconciled: Arc::new(AtomicU64::new(0)),
            unavailable: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
        }
    }
}

impl FreshnessBarrier {
    #[must_use]
    pub fn with_watermarks(admitted: u64, reconciled: u64, unavailable: bool) -> Self {
        Self {
            admitted: Arc::new(AtomicU64::new(admitted)),
            reconciled: Arc::new(AtomicU64::new(reconciled.min(admitted))),
            unavailable: Arc::new(AtomicBool::new(unavailable)),
            notify: Arc::new(Notify::new()),
        }
    }

    /// Admit a relevant hint before any provider work begins.
    #[must_use]
    pub fn admit(&self) -> u64 {
        self.admitted.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Advance the reconciliation watermark monotonically.
    pub fn reconcile(&self, watermark: u64) {
        self.reconciled.fetch_max(watermark, Ordering::AcqRel);
        self.notify.notify_waiters();
    }

    pub fn mark_unavailable(&self) {
        self.unavailable.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }

    #[must_use]
    pub fn state(&self) -> FreshnessState {
        if self.unavailable.load(Ordering::Acquire) {
            FreshnessState::Unavailable
        } else if self.reconciled.load(Ordering::Acquire) >= self.admitted.load(Ordering::Acquire) {
            FreshnessState::Current
        } else {
            FreshnessState::PotentiallyStale
        }
    }

    /// Await the exact watermark captured at query admission.
    ///
    /// # Errors
    ///
    /// Returns stale or unavailable for strict policies, or a timeout when the admitted watermark
    /// cannot become current within the supplied duration.
    pub async fn admit_query(
        &self,
        policy: FreshnessAdmission,
        timeout: std::time::Duration,
    ) -> Result<FreshnessState, LifecycleError> {
        let target = self.admitted.load(Ordering::Acquire);
        match policy {
            FreshnessAdmission::BestAvailable => return Ok(self.state()),
            FreshnessAdmission::RequireCurrent if self.state() != FreshnessState::Current => {
                return Err(LifecycleError::Stale);
            }
            FreshnessAdmission::RequireCurrent => return Ok(FreshnessState::Current),
            FreshnessAdmission::AwaitLatest => {}
        }
        tokio::time::timeout(timeout, async {
            loop {
                if self.unavailable.load(Ordering::Acquire) {
                    return Err(LifecycleError::Unavailable);
                }
                if self.reconciled.load(Ordering::Acquire) >= target {
                    return Ok(FreshnessState::Current);
                }
                self.notify.notified().await;
            }
        })
        .await
        .map_err(|_| LifecycleError::Stale)?
    }
}

/// Per-file incremental Tree-sitter lane with deterministic full-parse fallback.
pub struct FastSyntaxLane {
    parsers: BTreeMap<Vec<u8>, TreeSitterAdapter>,
}

impl FastSyntaxLane {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            parsers: BTreeMap::new(),
        }
    }

    /// Parse a stable UTF-8 capture, using the prior tree only when an exact edit is available.
    ///
    /// # Errors
    ///
    /// Returns UTF-8 or bounded adapter failures. Invalid incremental geometry widens to a full
    /// parse; it never publishes the partial incremental candidate.
    pub fn apply(
        &mut self,
        path_bytes: Vec<u8>,
        language: TreeSitterLanguage,
        revision: u64,
        source: &[u8],
        edit: Option<TreeSitterEdit>,
    ) -> Result<TreeSitterSnapshot, LifecycleError> {
        let text = std::str::from_utf8(source)
            .map_err(|_| LifecycleError::Path("syntax lane requires UTF-8 source".into()))?;
        let provider_text = ProviderText {
            text: Arc::<str>::from(text),
            original_byte_offsets: text
                .char_indices()
                .map(|(offset, _)| offset as u64)
                .chain(std::iter::once(text.len() as u64))
                .collect::<Vec<_>>()
                .into(),
        };
        let parser = match self.parsers.entry(path_bytes) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(TreeSitterAdapter::new(language)?)
            }
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
        };
        if let Some(edit) = edit {
            match parser.parse_incremental(
                revision,
                provider_text.clone(),
                edit,
                &Cancellation::default(),
            ) {
                Ok(snapshot) => Ok(snapshot),
                Err(TreeSitterAdapterError::InvalidEdit(_)) => parser
                    .parse_full(revision, provider_text, &Cancellation::default())
                    .map_err(Into::into),
                Err(error) => Err(error.into()),
            }
        } else {
            parser
                .parse_full(revision, provider_text, &Cancellation::default())
                .map_err(Into::into)
        }
    }
}

impl Default for FastSyntaxLane {
    fn default() -> Self {
        Self::new()
    }
}

/// Canonical fast-lane output for one immutable source image.
#[derive(Debug)]
pub struct FastSyntaxFactOutput {
    pub path_bytes: Vec<u8>,
    pub tree_sitter: TreeSitterSnapshot,
    pub ruff_python: Option<RuffSnapshot>,
    pub canonical: CanonicalIngestOutput,
}

/// Stateful incremental parser plus the sole canonical reconciliation engine.
#[derive(Default)]
pub struct FastSyntaxReconciler {
    syntax: FastSyntaxLane,
    core: CoreFactEngine,
}

impl FastSyntaxReconciler {
    /// Reconcile every captured source in one generated-state wave.
    ///
    /// # Errors
    ///
    /// Rejects the wrong wave phase, missing provider text, unsupported language, invalid edit,
    /// provider failure, or canonical fact validation failure. No partial result is returned.
    pub fn reconcile_wave(
        &mut self,
        wave: &AcceptedUpdateWave,
        analysis_context_id: [u8; 16],
        edits: &BTreeMap<Vec<u8>, TreeSitterEdit>,
    ) -> Result<Vec<FastSyntaxFactOutput>, LifecycleError> {
        if wave.state != UpdateWaveState::FastAnalyzing {
            return Err(LifecycleError::Transition(
                "fast syntax reconciliation requires FAST_ANALYZING".into(),
            ));
        }
        let mut outputs = Vec::new();
        for item in &wave.items {
            let Some(source) = item.captured.as_deref() else {
                continue;
            };
            let language = match source.language {
                SourceLanguage::Python => TreeSitterLanguage::Python,
                SourceLanguage::Rust => TreeSitterLanguage::Rust,
                SourceLanguage::Other => continue,
            };
            let tree_sitter = self.syntax.apply(
                item.path_bytes.clone(),
                language,
                source.source_generation,
                &source.bytes,
                edits.get(&item.path_bytes).copied(),
            )?;
            let ruff_python = if source.language == SourceLanguage::Python {
                let provider_text = source
                    .provider_text
                    .clone()
                    .ok_or(LifecycleError::ProviderTextUnavailable)?;
                let mut ruff = RuffAdapter::new()?;
                Some(ruff.parse(
                    source.source_generation,
                    provider_text,
                    &tree_sitter,
                    &Cancellation::default(),
                )?)
            } else {
                None
            };
            let source_generation = i64::try_from(source.source_generation).map_err(|_| {
                LifecycleError::Configuration("source generation exceeds i64".into())
            })?;
            let mut provider_outputs = vec![SourceSyntaxAdapterOutput::TreeSitter {
                provider_run_id: provider_run_identity(
                    wave.wave_id,
                    &item.path_bytes,
                    b"tree-sitter",
                ),
                snapshot: &tree_sitter,
            }];
            if let Some(ruff) = ruff_python.as_ref() {
                provider_outputs.push(SourceSyntaxAdapterOutput::RuffPython {
                    provider_run_id: provider_run_identity(
                        wave.wave_id,
                        &item.path_bytes,
                        b"ruff-python",
                    ),
                    snapshot: ruff,
                });
            }
            let canonical = self.core.reconcile_source_syntax(
                FactScope {
                    workspace_id: source.workspace_id,
                    analysis_context_id,
                    source_generation,
                    owner_id: source.file_id,
                },
                source,
                &provider_outputs,
            )?;
            outputs.push(FastSyntaxFactOutput {
                path_bytes: item.path_bytes.clone(),
                tree_sitter,
                ruff_python,
                canonical,
            });
        }
        outputs.sort_by(|left, right| left.path_bytes.cmp(&right.path_bytes));
        Ok(outputs)
    }
}

/// Convert the sole canonical reconciliation output into generated-policy overlay mutations.
/// Empty owner batches become explicit model-coded tombstones so older rows cannot leak through.
///
/// # Errors
///
/// Returns an error when a canonical batch cannot be represented by the registered overlay policy.
pub fn fast_output_mutations(
    outputs: &[FastSyntaxFactOutput],
) -> Result<Vec<OverlayMutation>, LifecycleError> {
    let mut mutations = Vec::new();
    for output in outputs {
        for validated in output.canonical.batches.values() {
            let scope = validated.scope();
            let mutation = if validated.num_rows() == 0 {
                OverlayMutation::owner_tombstone(
                    scope.workspace_id,
                    scope.analysis_context_id,
                    validated.table_code(),
                    scope.source_generation,
                    scope.owner_id,
                    i16::from(scope.owner_id[0]),
                    OverlayTombstoneReason::OwnerReplacedEmpty as i16,
                )?
            } else {
                OverlayMutation::owner_replacement(
                    scope.workspace_id,
                    scope.analysis_context_id,
                    validated.table_code(),
                    scope.source_generation,
                    Arc::new(validated.batch().clone()),
                )?
            };
            mutations.push(mutation);
        }
    }
    mutations.sort_by_key(OverlayMutation::payload_digest);
    Ok(mutations)
}

/// Construct the complete generated owner-tombstone set for removed current sources.
///
/// # Errors
///
/// Returns an error when the source generation exceeds the schema range or a registered tombstone
/// cannot be constructed.
pub fn removed_owner_mutations(
    workspace_id: [u8; 16],
    analysis_context_id: [u8; 16],
    source_generation: u64,
    owner_ids: &BTreeSet<[u8; 16]>,
) -> Result<Vec<OverlayMutation>, LifecycleError> {
    let generation = i64::try_from(source_generation)
        .map_err(|_| LifecycleError::Configuration("source generation exceeds i64".into()))?;
    let owner_tables = table_specs().iter().filter(|table| {
        table.overlay_mutation == OverlayMutationPolicy::OwnerReplace
            && table.materialization_role != MaterializationRole::OperationalProjection
            && crate::schema_registry::table_scope_spec(table.table_code)
                .is_some_and(|scope| scope.owner_column.is_some())
    });
    let mut mutations = Vec::new();
    for owner_id in owner_ids {
        for table in owner_tables.clone() {
            mutations.push(OverlayMutation::owner_tombstone(
                workspace_id,
                analysis_context_id,
                table.table_code,
                generation,
                *owner_id,
                i16::from(owner_id[0]),
                OverlayTombstoneReason::SourceRemoved as i16,
            )?);
        }
    }
    mutations.sort_by_key(OverlayMutation::payload_digest);
    Ok(mutations)
}

fn provider_run_identity(wave_id: [u8; 16], path: &[u8], provider: &[u8]) -> [u8; 16] {
    let mut hasher = crate::identity::semantic_fingerprint(
        crate::identity::SemanticFingerprintDomain::FastProviderRun,
    );
    hasher.update(&wave_id);
    hasher.update(&(path.len() as u64).to_be_bytes());
    hasher.update(path);
    hasher.update(provider);
    hasher.finalize_id16()
}

/// Model-selected overlay flush thresholds. No duration measurement is an acceptance criterion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OverlayFlushPolicy {
    pub maximum_rows: u64,
    pub maximum_bytes: u64,
    pub maximum_touched_owners: u64,
    pub maximum_generations: u64,
}

/// Current consolidated overlay pressure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OverlayPressure {
    pub rows: u64,
    pub bytes: u64,
    pub touched_owners: u64,
    pub generations_since_flush: u64,
}

/// Actor-owned immutable overlay generations and the last durable rebase fence.
#[derive(Default)]
pub struct ContinuousOverlayState {
    current: Option<Arc<ConsolidatedOverlay>>,
    last_durable_overlay_generation: u64,
}

impl ContinuousOverlayState {
    #[must_use]
    pub fn current(&self) -> Option<Arc<ConsolidatedOverlay>> {
        self.current.clone()
    }

    /// Consolidate one generation through the generated table mutation policies.
    ///
    /// # Errors
    ///
    /// Propagates scope, generation, schema, policy, duplicate-key, and DataFusion memory
    /// reservation failures.
    pub fn apply(
        &mut self,
        workspace_id: [u8; 16],
        analysis_context_id: [u8; 16],
        incoming: &[OverlayMutation],
        memory_limit_bytes: usize,
    ) -> Result<Arc<ConsolidatedOverlay>, LifecycleError> {
        let overlay = self.stage(
            workspace_id,
            analysis_context_id,
            incoming,
            memory_limit_bytes,
        )?;
        self.publish_staged(Arc::clone(&overlay))?;
        Ok(overlay)
    }

    /// Build and validate the next immutable overlay without making it query-visible.
    ///
    /// # Errors
    ///
    /// Returns an error for generation exhaustion or an invalid/over-budget consolidation.
    pub fn stage(
        &self,
        workspace_id: [u8; 16],
        analysis_context_id: [u8; 16],
        incoming: &[OverlayMutation],
        memory_limit_bytes: usize,
    ) -> Result<Arc<ConsolidatedOverlay>, LifecycleError> {
        let generation = self
            .current
            .as_ref()
            .map_or(self.last_durable_overlay_generation, |overlay| {
                overlay.overlay_generation()
            })
            .checked_add(1)
            .ok_or_else(|| LifecycleError::Configuration("overlay generation exhausted".into()))?;
        Ok(Arc::new(ConsolidatedOverlay::consolidate(
            OverlayConsolidationRequest {
                workspace_id,
                analysis_context_id,
                overlay_generation: generation,
                prior: self.current.as_deref(),
                incoming,
                memory_limit_bytes,
            },
        )?))
    }

    /// Atomically install one previously staged overlay generation in actor-owned memory.
    ///
    /// # Errors
    ///
    /// Returns an error when the staged overlay is not the exact next validated generation.
    pub fn publish_staged(
        &mut self,
        overlay: Arc<ConsolidatedOverlay>,
    ) -> Result<(), LifecycleError> {
        let expected = self
            .current
            .as_ref()
            .map_or(self.last_durable_overlay_generation, |current| {
                current.overlay_generation()
            })
            .checked_add(1)
            .ok_or_else(|| LifecycleError::Configuration("overlay generation exhausted".into()))?;
        if overlay.overlay_generation() != expected {
            return Err(LifecycleError::OverlayRebase(
                "staged overlay generation is not the next actor generation".into(),
            ));
        }
        if let Some(current) = self.current.as_ref()
            && (current.workspace_id() != overlay.workspace_id()
                || current.analysis_context_id() != overlay.analysis_context_id())
        {
            return Err(LifecycleError::OverlayRebase(
                "staged overlay scope drifted".into(),
            ));
        }
        self.current = Some(overlay);
        Ok(())
    }

    #[must_use]
    pub fn pressure(&self) -> OverlayPressure {
        self.current.as_ref().map_or(
            OverlayPressure {
                rows: 0,
                bytes: 0,
                touched_owners: 0,
                generations_since_flush: 0,
            },
            |overlay| OverlayPressure {
                rows: overlay.row_count(),
                bytes: overlay.memory_bytes(),
                touched_owners: overlay.touched_scope_count(),
                generations_since_flush: overlay
                    .overlay_generation()
                    .saturating_sub(self.last_durable_overlay_generation),
            },
        )
    }

    #[must_use]
    pub fn flush_candidate(&self, policy: OverlayFlushPolicy) -> Option<Arc<ConsolidatedOverlay>> {
        policy
            .requires_flush(self.pressure())
            .then(|| self.current())
            .flatten()
    }

    /// Install the validated delta returned by `ConsolidatedOverlay::execute_rebase`.
    ///
    /// # Errors
    ///
    /// Rejects scope drift or a generation older than the captured flush.
    pub fn accept_rebase(
        &mut self,
        flushed: &ConsolidatedOverlay,
        rebased_delta: Arc<ConsolidatedOverlay>,
    ) -> Result<(), LifecycleError> {
        if flushed.workspace_id() != rebased_delta.workspace_id()
            || flushed.analysis_context_id() != rebased_delta.analysis_context_id()
            || rebased_delta.overlay_generation() < flushed.overlay_generation()
        {
            return Err(LifecycleError::OverlayRebase(
                "rebased overlay scope or generation drifted".into(),
            ));
        }
        self.last_durable_overlay_generation = flushed.overlay_generation();
        self.current = Some(rebased_delta);
        Ok(())
    }
}

impl OverlayFlushPolicy {
    /// Evaluate only declared structural bounds.
    #[must_use]
    pub const fn requires_flush(self, pressure: OverlayPressure) -> bool {
        pressure.rows >= self.maximum_rows
            || pressure.bytes >= self.maximum_bytes
            || pressure.touched_owners >= self.maximum_touched_owners
            || pressure.generations_since_flush >= self.maximum_generations
    }
}

/// Durable state decoded during startup. Historical intermediate codes are never resumed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistedWaveDisposition {
    ResumeCollection,
    ResumeSnapshot,
    RestartFromCurrentBytes,
    RetireHistoricalTerminal,
    RetainFailure,
    RetainSuperseded,
    RetainCancelled,
}

/// Decode-only recovery policy for the historical update-wave state registry.
///
/// # Errors
///
/// Returns an error when the persisted code is not present in the historical registry.
pub fn recover_wave_state(code: i64) -> Result<PersistedWaveDisposition, LifecycleError> {
    match code {
        10 => Ok(PersistedWaveDisposition::ResumeCollection),
        20 => Ok(PersistedWaveDisposition::ResumeSnapshot),
        // Historical RUNNING/PUBLISHING/intermediate states are never resumed. A hot overlay is
        // process-local until the durable flush commits, so its bytes must also be replayed.
        30 | 40 | 80..=180 => Ok(PersistedWaveDisposition::RestartFromCurrentBytes),
        // Historical COMPLETE is decode-only and eligible for bounded-history retirement.
        50 | 190 => Ok(PersistedWaveDisposition::RetireHistoricalTerminal),
        60 => Ok(PersistedWaveDisposition::RetainFailure),
        70 => Ok(PersistedWaveDisposition::RetainSuperseded),
        200 => Ok(PersistedWaveDisposition::RetainCancelled),
        _ => Err(LifecycleError::Recovery(
            "unknown persisted update-wave code".into(),
        )),
    }
}

/// One durable wave decoded for startup recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveredWave {
    pub wave_id: [u8; 16],
    pub source_generation: u64,
    pub event_watermark: u64,
    pub disposition: PersistedWaveDisposition,
    pub item_paths: BTreeSet<Vec<u8>>,
}

/// Exact startup instructions derived from operational rows, never process-local memory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceRecoveryPlan {
    pub workspace_id: [u8; 16],
    pub source_generation: u64,
    pub event_watermark: u64,
    pub durable_generation: u64,
    pub restart_paths: BTreeSet<Vec<u8>>,
    pub full_inventory_required: bool,
    pub active_snapshot_id: Option<[u8; 16]>,
    pub overlay_recovery_required: bool,
    pub waves: Vec<RecoveredWave>,
}

/// Decode all persisted lifecycle surfaces needed before watcher activation.
///
/// # Errors
///
/// Returns an error for missing/corrupt operational rows, unknown state codes, invalid identities,
/// or a storage failure.
pub fn recover_workspace(
    reader: &OperationalReader,
    workspace_id: [u8; 16],
) -> Result<WorkspaceRecoveryPlan, LifecycleError> {
    let (
        source_generation,
        event_watermark,
        durable_generation,
        reconcile_required,
        active_snapshot_id,
    ) = reader.with_connection_result(|connection| {
        connection.query_row(
            "SELECT source_generation,event_watermark,durable_generation,reconcile_required,active_snapshot_id FROM worktree_state WHERE workspace_id=?1",
            [workspace_id.as_slice()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<Vec<u8>>>(4)?,
                ))
            },
        )
        .map_err(LifecycleError::from)
    })?;
    let raw_waves = reader.with_connection_result(|connection| {
        let mut statement = connection.prepare(
            "SELECT wave_id,source_generation,event_watermark,state_code FROM update_wave WHERE workspace_id=?1 ORDER BY source_generation,wave_id",
        )?;
        statement
            .query_map([workspace_id.as_slice()], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(LifecycleError::from)
    })?;
    let mut waves = Vec::with_capacity(raw_waves.len());
    let mut restart_paths = BTreeSet::new();
    for (wave_id, generation, watermark, state) in raw_waves {
        let wave_id = exact_bytes(wave_id, "wave identity")?;
        let disposition = recover_wave_state(state)?;
        let item_paths = reader.with_connection_result(|connection| {
            let mut statement = connection.prepare(
                "SELECT path_bytes FROM update_wave_item WHERE wave_id=?1 ORDER BY item_ordinal",
            )?;
            statement
                .query_map([wave_id.as_slice()], |row| row.get::<_, Vec<u8>>(0))?
                .collect::<Result<BTreeSet<_>, _>>()
                .map_err(LifecycleError::from)
        })?;
        if matches!(
            disposition,
            PersistedWaveDisposition::ResumeCollection
                | PersistedWaveDisposition::ResumeSnapshot
                | PersistedWaveDisposition::RestartFromCurrentBytes
        ) {
            restart_paths.extend(item_paths.iter().cloned());
        }
        waves.push(RecoveredWave {
            wave_id,
            source_generation: nonnegative_u64(generation, "wave source generation")?,
            event_watermark: nonnegative_u64(watermark, "wave event watermark")?,
            disposition,
            item_paths,
        });
    }
    let overlay_count = reader.with_connection_result(|connection| {
        connection
            .query_row(
                "SELECT COUNT(*) FROM hot_overlay_manifest WHERE workspace_id=?1",
                [workspace_id.as_slice()],
                |row| row.get::<_, i64>(0),
            )
            .map_err(LifecycleError::from)
    })?;
    Ok(WorkspaceRecoveryPlan {
        workspace_id,
        source_generation: nonnegative_u64(source_generation, "source generation")?,
        event_watermark: nonnegative_u64(event_watermark, "event watermark")?,
        durable_generation: nonnegative_u64(durable_generation, "durable generation")?,
        restart_paths,
        full_inventory_required: reconcile_required != 0,
        active_snapshot_id: active_snapshot_id
            .map(|identity| exact_bytes(identity, "active snapshot identity"))
            .transpose()?,
        overlay_recovery_required: overlay_count > 0,
        waves,
    })
}

fn nonnegative_u64(value: i64, label: &str) -> Result<u64, LifecycleError> {
    u64::try_from(value).map_err(|_| LifecycleError::Recovery(format!("{label} is negative")))
}

/// Canonical comparison state for one effective table.
///
/// `row_multiplicities` is an exact bag rather than a set. `governed_rows` independently proves
/// that every projected governed key is unique and maps to exactly one complete projected row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalTableState {
    pub canonical_schema: Vec<u8>,
    pub row_count: u64,
    pub row_multiplicities: BTreeMap<Vec<u8>, u64>,
    pub governed_rows: BTreeMap<Vec<u8>, Vec<u8>>,
}

/// Canonical full/incremental effective state, deliberately independent of timing, partitioning,
/// physical Delta layout, publication identity, and overlay generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalState {
    pub tables: BTreeMap<String, CanonicalTableState>,
}

fn comparison_ignore_fields() -> Result<BTreeSet<String>, LifecycleError> {
    let registry: crate::contracts::registry_models::AcceptedRegistry<
        crate::contracts::registry_models::ComparisonIgnoreRecord,
    > = serde_yaml_ng::from_str(include_str!(
        "../contracts/comparison/comparison-ignore-registry.yaml"
    ))
    .map_err(|error| LifecycleError::RebuildExtraction(error.to_string()))?;
    crate::contracts::registry_models::validate_comparison_ignores(&registry.records)
        .map_err(LifecycleError::RebuildExtraction)?;
    Ok(registry
        .records
        .into_iter()
        .map(|record| record.field_name)
        .collect())
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    reason = "one comparator extraction keeps schema projection, exact bags, and governed-key uniqueness atomic"
)]
#[cfg(test)]
fn canonical_table_state(
    table_name: &str,
    schema: SchemaRef,
    primary_key: &[&str],
    batches: &[RecordBatch],
    ignored_fields: &BTreeSet<String>,
) -> Result<CanonicalTableState, LifecycleError> {
    canonical_table_state_with_normalization(
        table_name,
        schema,
        primary_key,
        batches,
        ignored_fields,
        None,
    )
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    reason = "one comparator extraction keeps schema projection, exact bags, and governed-key uniqueness atomic"
)]
fn canonical_table_state_with_normalization(
    table_name: &str,
    schema: SchemaRef,
    primary_key: &[&str],
    batches: &[RecordBatch],
    ignored_fields: &BTreeSet<String>,
    foreign_key_normalization: Option<ComparisonForeignKeyNormalization>,
) -> Result<CanonicalTableState, LifecycleError> {
    if batches
        .iter()
        .any(|batch| batch.schema().as_ref() != schema.as_ref())
    {
        return Err(LifecycleError::SchemaMismatch(format!(
            "{table_name}: query batches do not share the planned Arrow schema"
        )));
    }
    let projected_indices = schema
        .fields()
        .iter()
        .enumerate()
        .filter_map(|(index, field)| (!ignored_fields.contains(field.name())).then_some(index))
        .collect::<Vec<_>>();
    let projected_schema = Arc::new(
        schema
            .project(&projected_indices)
            .map_err(FabricError::from)?,
    );
    let canonical_schema = crate::contracts::jcs::canonicalize_value(
        &serde_json::to_value(projected_schema.as_ref())
            .map_err(|error| LifecycleError::RebuildExtraction(error.to_string()))?,
    )
    .map_err(|error| LifecycleError::RebuildExtraction(error.to_string()))?;

    let key_names = primary_key
        .iter()
        .filter(|name| !ignored_fields.contains(**name))
        .copied()
        .collect::<Vec<_>>();
    if !primary_key.is_empty() && key_names.is_empty() {
        return Err(LifecycleError::RebuildExtraction(format!(
            "{table_name}: comparison projection removed the complete governed key"
        )));
    }
    let key_indices = key_names
        .iter()
        .map(|name| {
            projected_schema.index_of(name).map_err(|_| {
                LifecycleError::SchemaMismatch(format!(
                    "{table_name}: governed key field {name} is absent from the effective projection"
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let row_converter = RowConverter::new(
        projected_schema
            .fields()
            .iter()
            .map(|field| SortField::new(field.data_type().clone()))
            .collect(),
    )
    .map_err(FabricError::from)?;
    let key_converter = (!key_indices.is_empty())
        .then(|| {
            RowConverter::new(
                key_indices
                    .iter()
                    .map(|&index| SortField::new(projected_schema.field(index).data_type().clone()))
                    .collect(),
            )
            .map_err(FabricError::from)
        })
        .transpose()?;

    let mut row_count = 0_u64;
    let mut row_multiplicities = BTreeMap::<Vec<u8>, u64>::new();
    let mut governed_rows = BTreeMap::<Vec<u8>, Vec<u8>>::new();
    for batch in batches {
        let normalized = normalize_operational_foreign_keys(
            foreign_key_normalization,
            Arc::clone(&schema),
            batch,
            ignored_fields,
        )?;
        let projected = normalized
            .project(&projected_indices)
            .map_err(FabricError::from)?;
        let full_rows = row_converter
            .convert_columns(projected.columns())
            .map_err(FabricError::from)?;
        let key_rows = if let Some(converter) = key_converter.as_ref() {
            let key_columns = key_indices
                .iter()
                .map(|&index| Arc::clone(projected.column(index)))
                .collect::<Vec<_>>();
            Some(
                converter
                    .convert_columns(&key_columns)
                    .map_err(FabricError::from)?,
            )
        } else {
            None
        };
        row_count = row_count
            .checked_add(u64::try_from(batch.num_rows()).map_err(|_| {
                LifecycleError::RebuildExtraction(format!("{table_name}: row count exceeds u64"))
            })?)
            .ok_or_else(|| {
                LifecycleError::RebuildExtraction(format!("{table_name}: row count overflow"))
            })?;
        for (row_index, full) in full_rows.iter().enumerate() {
            let full = full.data().to_vec();
            *row_multiplicities.entry(full.clone()).or_default() += 1;
            if let Some(key_rows) = key_rows.as_ref() {
                let key = key_rows.row(row_index).data().to_vec();
                if let Some(previous) = governed_rows.insert(key, full.clone()) {
                    let key_values = key_names
                        .iter()
                        .zip(&key_indices)
                        .map(|(name, index)| {
                            array_value_to_string(projected.column(*index).as_ref(), row_index)
                                .map(|value| format!("{name}={value}"))
                                .map_err(|error| {
                                    LifecycleError::RebuildExtraction(format!(
                                        "{table_name}: governed key formatting failed: {error}"
                                    ))
                                })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    return Err(LifecycleError::RebuildExtraction(format!(
                        "{table_name}: duplicate projected governed key [{}], identical rows: {}",
                        key_values.join(", "),
                        previous == full
                    )));
                }
            }
        }
    }
    Ok(CanonicalTableState {
        canonical_schema,
        row_count,
        row_multiplicities,
        governed_rows,
    })
}

fn normalize_operational_foreign_keys(
    normalization: Option<ComparisonForeignKeyNormalization>,
    schema: SchemaRef,
    batch: &RecordBatch,
    ignored_fields: &BTreeSet<String>,
) -> Result<RecordBatch, LifecycleError> {
    if normalization != Some(ComparisonForeignKeyNormalization::EvidenceContentIdentityV1) {
        return Ok(batch.clone());
    }
    let evidence_index = schema
        .index_of("evidence_id")
        .map_err(|_| LifecycleError::SchemaMismatch("evidence: evidence_id is absent".into()))?;
    let observation_index = schema
        .index_of("observation_id")
        .map_err(|_| LifecycleError::SchemaMismatch("evidence: observation_id is absent".into()))?;
    let semantic_indices = schema
        .fields()
        .iter()
        .enumerate()
        .filter_map(|(index, field)| {
            (!ignored_fields.contains(field.name())
                && index != evidence_index
                && index != observation_index)
                .then_some(index)
        })
        .collect::<Vec<_>>();
    let semantic_converter = RowConverter::new(
        semantic_indices
            .iter()
            .map(|&index| SortField::new(schema.field(index).data_type().clone()))
            .collect(),
    )
    .map_err(FabricError::from)?;
    let semantic_columns = semantic_indices
        .iter()
        .map(|&index| Arc::clone(batch.column(index)))
        .collect::<Vec<_>>();
    let semantic_rows = semantic_converter
        .convert_columns(&semantic_columns)
        .map_err(FabricError::from)?;
    let normalized_ids = semantic_rows
        .iter()
        .map(|row| {
            let mut hasher = crate::integrity::IntegrityHasher::for_domain(
                crate::integrity::IntegrityDomain::ContinuousState,
            );
            hasher.update(b"normalized-evidence-identity-v1");
            hasher.update(&(row.data().len() as u64).to_be_bytes());
            hasher.update(row.data());
            let digest = hasher.finalize();
            let mut identity = [0_u8; 16];
            identity.copy_from_slice(&digest[..16]);
            identity
        })
        .collect::<Vec<_>>();
    let normalized_array = crate::fabric::id16_array(normalized_ids.iter().map(Some));
    let mut columns = batch.columns().to_vec();
    columns[evidence_index] = Arc::clone(&normalized_array);
    columns[observation_index] = normalized_array;
    RecordBatch::try_new(schema, columns)
        .map_err(FabricError::from)
        .map_err(Into::into)
}

impl CanonicalState {
    /// Extract the effective DataFusion state and diagnostics from one pinned serving session.
    ///
    /// Arrow's canonical row encoder makes the comparison independent of partitioning and output
    /// order without deduplicating rows. Schema bytes retain field order, nullability, field
    /// metadata, and schema metadata. Only fields named by the released ignore registry are
    /// projected away.
    ///
    /// # Errors
    ///
    /// Returns an error when a registered serving projection cannot be queried, encoded, or mapped
    /// to its generated schema.
    pub async fn from_serving_session(
        session: &ServingQuerySession,
    ) -> Result<Self, LifecycleError> {
        let ignored_fields = comparison_ignore_fields()?;
        let mut tables = BTreeMap::new();
        for projection in serving_projection_specs() {
            let plan = session.table_plan(projection.view_name).await?;
            let schema = Arc::new(plan.schema().as_arrow().clone());
            let result = session
                .query(&format!(
                    "SELECT * FROM cpg_serving.\"{}\"",
                    projection.view_name
                ))
                .await?;
            let comparison =
                comparison_projection_spec(projection.source_table_code).ok_or_else(|| {
                    LifecycleError::RebuildExtraction(format!(
                        "generated comparison projection {} is absent",
                        projection.source_table_code
                    ))
                })?;
            tables.insert(
                projection.view_name.to_owned(),
                canonical_table_state_with_normalization(
                    projection.view_name,
                    schema,
                    comparison.primary_sort_key,
                    &result.batches,
                    &ignored_fields,
                    comparison.foreign_key_normalization,
                )?,
            );
        }
        let diagnostic_plan = session.table_plan("cpg_base.diagnostic").await?;
        let diagnostic_schema = Arc::new(diagnostic_plan.schema().as_arrow().clone());
        let diagnostic = session.query("SELECT * FROM cpg_base.diagnostic").await?;
        let diagnostic_projection = comparison_projection_spec(10).ok_or_else(|| {
            LifecycleError::RebuildExtraction("diagnostic comparison projection is absent".into())
        })?;
        tables.insert(
            "diagnostic".to_owned(),
            canonical_table_state_with_normalization(
                "diagnostic",
                diagnostic_schema,
                diagnostic_projection.primary_sort_key,
                &diagnostic.batches,
                &ignored_fields,
                diagnostic_projection.foreign_key_normalization,
            )?,
        );
        Ok(Self { tables })
    }

    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        let mut hasher = crate::integrity::IntegrityHasher::for_domain(
            crate::integrity::IntegrityDomain::ContinuousState,
        );
        for (table, state) in &self.tables {
            hasher.update(&(table.len() as u64).to_be_bytes());
            hasher.update(table.as_bytes());
            hasher.update(&(state.canonical_schema.len() as u64).to_be_bytes());
            hasher.update(&state.canonical_schema);
            hasher.update(&state.row_count.to_be_bytes());
            for (row, multiplicity) in &state.row_multiplicities {
                hasher.update(&(row.len() as u64).to_be_bytes());
                hasher.update(row);
                hasher.update(&multiplicity.to_be_bytes());
            }
        }
        hasher.finalize()
    }

    /// Compare exact normalized rows and diagnostics with a clean rebuild.
    ///
    /// # Errors
    ///
    /// Returns a registered schema or row mismatch when either canonical state differs.
    pub fn prove_equivalent(&self, rebuilt: &Self) -> Result<(), LifecycleError> {
        if self.tables.keys().ne(rebuilt.tables.keys()) {
            return Err(LifecycleError::RowMismatch(
                "effective table census differs".into(),
            ));
        }
        for (name, incremental) in &self.tables {
            let clean = &rebuilt.tables[name];
            if incremental.canonical_schema != clean.canonical_schema {
                return Err(LifecycleError::SchemaMismatch(name.clone()));
            }
            if incremental.row_count != clean.row_count
                || incremental.row_multiplicities != clean.row_multiplicities
                || incremental.governed_rows != clean.governed_rows
            {
                return Err(LifecycleError::RowMismatch(name.clone()));
            }
        }
        Ok(())
    }
}

/// Compare two independently pinned serving sessions after proving that they describe the same
/// semantic comparison domain. Publication, snapshot, generation, physical-layout, and Git
/// acceleration locators are intentionally excluded; the released field registry governs row
/// projection.
///
/// # Errors
///
/// Rejects non-current or different semantic domains before extracting either effective state,
/// then returns exact schema, governed-key, row-count, or bag-multiplicity differences.
pub async fn prove_serving_rebuild_equivalence(
    incremental: &ServingQuerySession,
    rebuilt: &ServingQuerySession,
) -> Result<(), LifecycleError> {
    let left = incremental.snapshot_manifest();
    let right = rebuilt.snapshot_manifest();
    if left.body.source.source_trust_state != "CURRENT"
        || right.body.source.source_trust_state != "CURRENT"
    {
        return Err(LifecycleError::ComparisonDomainMismatch(
            "both snapshots must carry CURRENT source trust".into(),
        ));
    }
    let same_domain = left.body.manifest_version == right.body.manifest_version
        && left.body.workspace_id == right.body.workspace_id
        && left.body.repository_id == right.body.repository_id
        && left.body.worktree_id == right.body.worktree_id
        && left.body.registration_revision == right.body.registration_revision
        && left.body.source.inventory_digest == right.body.source.inventory_digest
        && left.body.source.authorization_fingerprint
            == right.body.source.authorization_fingerprint
        && left.body.source.inclusion_policy_fingerprint
            == right.body.source.inclusion_policy_fingerprint
        && left.body.source.path_profile_version == right.body.source.path_profile_version
        && left.body.contexts == right.body.contexts
        && left.body.indexes == right.body.indexes
        && left.body.bundles == right.body.bundles
        && left.body.limits_profile_digest == right.body.limits_profile_digest
        && left.body.source_blob_digests == right.body.source_blob_digests;
    if !same_domain {
        return Err(LifecycleError::ComparisonDomainMismatch(
            "snapshot semantic inputs differ".into(),
        ));
    }
    let incremental = CanonicalState::from_serving_session(incremental).await?;
    let rebuilt = CanonicalState::from_serving_session(rebuilt).await?;
    incremental.prove_equivalent(&rebuilt)
}

#[derive(Debug, Error)]
pub enum LifecycleError {
    #[error("INVALID_REQUEST_SCHEMA:LIFECYCLE_CONFIGURATION_INVALID:{0}")]
    Configuration(String),
    #[error("PATH_OUTSIDE_AUTHORIZED_ROOT:LIFECYCLE_PATH_INVALID:{0}")]
    Path(String),
    #[error("LIFECYCLE_SOURCE_DRIFT")]
    SourceDrift,
    #[error("INTERNAL_INVARIANT_VIOLATION:LIFECYCLE_GRAPH_INVALID:{0}")]
    Graph(String),
    #[error("INTERNAL_INVARIANT_VIOLATION:LIFECYCLE_GRAPH_CYCLE:{0:?}")]
    Cycle(DependencyCycleWitness),
    #[error("LIFECYCLE_STALE")]
    Stale,
    #[error("LIFECYCLE_UNAVAILABLE")]
    Unavailable,
    #[error("CAPABILITY_UNAVAILABLE:LIFECYCLE_WATCHER:{0}")]
    Watcher(String),
    #[error("LIFECYCLE_WATCHER_CLOSED")]
    WatcherClosed,
    #[error("LIFECYCLE_AUTHORITATIVE_INVENTORY_REQUIRED")]
    AuthoritativeInventoryRequired,
    #[error("STATE_TRANSITION_VIOLATION:LIFECYCLE_TRANSITION_INVALID:{0}")]
    Transition(String),
    #[error("COMPARISON_DOMAIN_MISMATCH:{0}")]
    ComparisonDomainMismatch(String),
    #[error("SCHEMA_MISMATCH:{0}")]
    SchemaMismatch(String),
    #[error("ROW_MISMATCH:{0}")]
    RowMismatch(String),
    #[error("COMPARATOR_ERROR:LIFECYCLE_REBUILD_EXTRACTION:{0}")]
    RebuildExtraction(String),
    #[error("INTERNAL_INVARIANT_VIOLATION:LIFECYCLE_RECOVERY_INVALID:{0}")]
    Recovery(String),
    #[error("lifecycle I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Store(#[from] OperationalStoreError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Syntax(#[from] TreeSitterAdapterError),
    #[error(transparent)]
    Ruff(#[from] crate::ruff_adapter::RuffAdapterError),
    #[error(transparent)]
    CoreFacts(#[from] CoreFactError),
    #[error("LIFECYCLE_PROVIDER_TEXT_UNAVAILABLE")]
    ProviderTextUnavailable,
    #[error(transparent)]
    SourceImage(#[from] SourceImageError),
    #[error(transparent)]
    SecurePath(#[from] crate::secure_path::SecurePathError),
    #[error(transparent)]
    Fabric(#[from] FabricError),
    #[error(transparent)]
    Serving(#[from] ServingQueryError),
    #[error("CURRENT_FACTS_UNAVAILABLE:LIFECYCLE_OVERLAY_REBASE:{0}")]
    OverlayRebase(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fabric::SnapshotOverlayProviderFactory as _;
    use crate::workspace_registry::{WorkspaceRegistry, WorkspaceSourceRegistration};
    use notify_debouncer_full::notify::event::{CreateKind, Flag, RenameMode};

    fn test_lifecycle_config() -> LifecycleConfig {
        LifecycleConfig {
            debounce_timeout: std::time::Duration::from_millis(20),
            tick_rate: std::time::Duration::from_millis(5),
            ingress_capacity: 8,
            maximum_paths_per_batch: 32,
            gather_window: std::time::Duration::from_millis(5),
            dirty_path_bulk_threshold: 16,
            await_current_timeout: std::time::Duration::from_secs(1),
            maximum_capture_bytes: 1024 * 1024,
            stable_read_retry_count: 3,
            source_blob_lease_ttl: std::time::Duration::from_secs(300),
            overlay_flush_policy: OverlayFlushPolicy {
                maximum_rows: 100,
                maximum_bytes: 1024 * 1024,
                maximum_touched_owners: 32,
                maximum_generations: 8,
            },
        }
    }

    #[test]
    fn wp41_behavioral_acceptance() {
        let root = Path::new("/workspace");
        let first = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
            .add_path(root.join("old.py"))
            .add_path(root.join("new.py"));
        let second = Event::new(EventKind::Create(CreateKind::File)).add_path(root.join("z.rs"));
        let left =
            WatcherFacade::normalize(root, &[first.clone(), second.clone()], 10_000).unwrap();
        let right = WatcherFacade::normalize(root, &[second, first], 10_000).unwrap();
        assert_eq!(left, right);
        let mut overflow = Event::new(EventKind::Any);
        overflow.attrs.set_flag(Flag::Rescan);
        assert!(
            WatcherFacade::normalize(root, &[overflow], 10_000)
                .unwrap()
                .rescan_required
        );
    }

    #[tokio::test]
    async fn wp41_operational_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let config = test_lifecycle_config();
        let mut watcher = WorkspaceWatcher::start(root.path(), config).unwrap();
        fs::write(root.path().join("live.py"), b"answer = 42\n").unwrap();

        let observed = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                let batch = watcher.next_batch().await.unwrap();
                if batch.rescan_required
                    || batch.hints.iter().any(|hint| hint.path_bytes == b"live.py")
                {
                    break batch;
                }
            }
        })
        .await
        .unwrap();
        assert!(
            observed.rescan_required
                || observed
                    .hints
                    .iter()
                    .any(|hint| hint.path_bytes == b"live.py")
        );
        watcher.shutdown();
    }

    #[test]
    fn wp41_structural_acceptance() {
        let config = test_lifecycle_config();
        assert!(config.ingress_capacity > 0);
        assert!(config.maximum_paths_per_batch >= config.ingress_capacity);
        assert!(config.tick_rate <= config.debounce_timeout);
    }

    #[test]
    fn wp41_negative_zero_state() {
        let root = Path::new("/workspace");
        let outside = Event::new(EventKind::Create(CreateKind::File))
            .add_path(Path::new("/outside/source.py").to_path_buf());
        assert!(WatcherFacade::normalize(root, &[outside], 10).is_err());
        let overflow = (0..11)
            .map(|index| {
                Event::new(EventKind::Create(CreateKind::File))
                    .add_path(root.join(format!("{index}.py")))
            })
            .collect::<Vec<_>>();
        assert!(
            WatcherFacade::normalize(root, &overflow, 10)
                .unwrap()
                .rescan_required
        );
    }

    #[test]
    fn wp42_behavioral_acceptance() {
        let mut dirty = DirtyRegistry::default();
        let hint = WatchHint {
            path_bytes: b"a.py".to_vec(),
            kind: WatchHintKind::CreateOrModify,
        };
        dirty.admit(WatchHintBatch {
            hints: vec![hint.clone(), hint],
            rescan_required: false,
        });
        let frozen = dirty.freeze();
        assert_eq!(frozen.paths.len(), 1);
        assert_eq!(frozen.paths[b"a.py".as_slice()].0.len(), 1);
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One oracle proves the complete persisted transition and recovery contract.
    fn wp42_operational_acceptance() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("workspace");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("a.py"), b"x = 1\n").unwrap();
        let database = directory.path().join("operational.sqlite");
        let mut store = OperationalStore::open(&database).unwrap();
        let workspace_id = WorkspaceRegistry::new(&mut store)
            .add(&root, WorkspaceSourceRegistration::Directory)
            .unwrap();
        let workspace_id = workspace_id.workspace_id;
        let mut source_images = SourceImageStore::open(
            &directory.path().join("source-blobs"),
            test_lifecycle_config().source_capture_policy(),
        )
        .unwrap();
        let mut scheduler =
            UpdateWaveScheduler::new(workspace_id, &root, 0, 0, 0, test_lifecycle_config())
                .unwrap();
        scheduler
            .admit_persisted(
                &mut store,
                WatchHintBatch {
                    hints: vec![
                        WatchHint {
                            path_bytes: b"a.py".to_vec(),
                            kind: WatchHintKind::CreateOrModify,
                        },
                        WatchHint {
                            path_bytes: b"removed.py".to_vec(),
                            kind: WatchHintKind::Remove,
                        },
                    ],
                    rescan_required: false,
                },
            )
            .unwrap();
        assert_eq!(
            scheduler.freshness().state(),
            FreshnessState::PotentiallyStale
        );
        let mut wave = scheduler
            .prepare_wave(&mut store, &mut source_images, None)
            .unwrap()
            .unwrap();
        assert_eq!(wave.state, UpdateWaveState::Classifying);
        assert_eq!(wave.items.len(), 2);
        assert!(wave.items.iter().any(|item| {
            item.path_bytes == b"a.py" && item.state == UpdateWaveItemState::Captured
        }));
        assert!(wave.items.iter().any(|item| {
            item.path_bytes == b"removed.py" && item.state == UpdateWaveItemState::Removed
        }));
        for (event, guard, expected) in [
            (
                "sources-classified",
                "classifications-closed",
                UpdateWaveState::FastAnalyzing,
            ),
            (
                "fast-outputs-staged",
                "fast-providers-terminal",
                UpdateWaveState::FastValidating,
            ),
            (
                "fast-output-valid",
                "fast-contracts-satisfied",
                UpdateWaveState::FastPublished,
            ),
            (
                "semantic-work-not-applicable",
                "semantic-capabilities-terminal",
                UpdateWaveState::Validating,
            ),
            (
                "wave-output-valid",
                "required-capabilities-terminal",
                UpdateWaveState::HotPublished,
            ),
        ] {
            scheduler
                .transition(&mut store, &mut wave, event, guard)
                .unwrap();
            assert_eq!(wave.state, expected);
        }
        assert_eq!(scheduler.freshness().state(), FreshnessState::Current);
        let reader = store.reader_factory().open().unwrap();
        let persisted = reader
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT state_code,candidate_strategy_code FROM update_wave WHERE wave_id=?1",
                    [wave.wave_id.as_slice()],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
                )
            })
            .unwrap();
        assert_eq!(persisted.0, i64::from(UpdateWaveState::HotPublished as u16));
        assert_eq!(
            persisted.1,
            i64::from(UpdateCandidateStrategy::IsolatedPaths as u16)
        );
        assert!(![30_i64, 40, 50].contains(&persisted.0));
        let source_trust = reader
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT source_trust_state_code,reconcile_required FROM worktree_state WHERE workspace_id=?1",
                    [workspace_id.as_slice()],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
                )
            })
            .unwrap();
        assert_eq!(
            source_trust,
            (i64::from(SourceTrustState::Current as u16), 0)
        );
    }

    #[test]
    fn wp42_structural_acceptance() {
        let historical = BTreeSet::from(["RUNNING", "PUBLISHING", "COMPLETE"]);
        assert!(UPDATE_WAVE_STATE_TRANSITIONS.iter().all(|transition| {
            !historical.contains(transition.from) && !historical.contains(transition.to)
        }));
        assert!(
            UPDATE_WAVE_STATE_VALUES
                .iter()
                .any(|state| state.code == 30)
        );
        assert!(
            UPDATE_WAVE_STATE_VALUES
                .iter()
                .any(|state| state.code == 40)
        );
        assert!(
            UPDATE_WAVE_STATE_VALUES
                .iter()
                .any(|state| state.code == 50)
        );
    }

    #[test]
    fn wp42_negative_zero_state() {
        assert!(transition_state(UpdateWaveState::Collecting, "invalid", "invalid").is_err());
        let mut dirty = DirtyRegistry::default();
        dirty.admit(WatchHintBatch {
            hints: vec![WatchHint {
                path_bytes: b"a.py".to_vec(),
                kind: WatchHintKind::CreateOrModify,
            }],
            rescan_required: true,
        });
        let frozen = dirty.freeze();
        dirty.restore(frozen.clone());
        assert_eq!(dirty.freeze(), frozen);
    }

    fn edge(source: u8, dependent: u8) -> DependencyEdge {
        DependencyEdge {
            source_owner_id: [source; 16],
            dependent_owner_id: [dependent; 16],
            kind: DependencyEdgeKind::Derivation,
            derivation_id: Some("SYNTAX_TREE_V1".into()),
            source_generation: 1,
            input_digest: [source ^ dependent; 32],
        }
    }

    #[test]
    fn wp43_behavioral_acceptance() {
        let graph = OperationalDependencyGraph::new(vec![edge(2, 3), edge(1, 2)]).unwrap();
        assert_eq!(
            graph.affected_closure(&BTreeSet::from([[1; 16]])),
            BTreeSet::from([[1; 16], [2; 16], [3; 16]])
        );
        assert!(OperationalDependencyGraph::new(vec![edge(1, 2), edge(2, 1)]).is_err());
        let Err(self_cycle) = OperationalDependencyGraph::new(vec![edge(4, 4)]) else {
            panic!("self-cycle was accepted");
        };
        let LifecycleError::Cycle(witness) = self_cycle else {
            panic!("expected typed cycle witness");
        };
        assert_eq!(witness.owners, vec![[4; 16]]);
        assert_eq!(witness.edges.len(), 1);

        let widened = graph.plan_invalidation(&BTreeSet::from([[9; 16]]));
        assert!(widened.full_rebuild_required);
        assert_eq!(
            widened.affected_owners,
            BTreeSet::from([[1; 16], [2; 16], [3; 16], [9; 16]])
        );
        assert_eq!(widened.traversed_edges.len(), 2);
        assert_eq!(
            widened.invalidated_table_codes,
            BTreeSet::from([
                10, 180, 190, 200, 210, 220, 230, 240, 250, 260, 270, 280, 290, 300, 310, 320, 330,
                340, 350,
            ])
        );
    }

    #[test]
    fn wp43_operational_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let database = root.path().join("operational.sqlite");
        let mut store = OperationalStore::open(&database).unwrap();
        let workspace_id = [5; 16];
        let graph = OperationalDependencyGraph::new(vec![edge(2, 3), edge(1, 2)]).unwrap();
        graph.persist(&mut store, workspace_id).unwrap();

        let failed: Result<(), LifecycleError> = store.write_transaction(|transaction| {
            transaction.execute(
                "DELETE FROM operational_dependency_edge WHERE workspace_id=?1",
                [workspace_id.as_slice()],
            )?;
            Err(LifecycleError::Graph("injected rollback".into()))
        });
        assert!(failed.is_err());

        let reader = store.reader_factory().open().unwrap();
        let restored = OperationalDependencyGraph::load(&reader, workspace_id).unwrap();
        assert_eq!(restored.records, graph.records);
        assert_eq!(
            restored.affected_closure(&BTreeSet::from([[1; 16]])),
            BTreeSet::from([[1; 16], [2; 16], [3; 16]])
        );
    }

    #[test]
    fn wp43_structural_acceptance() {
        let graph = OperationalDependencyGraph::new(vec![edge(3, 4), edge(1, 2)]).unwrap();
        assert!(
            graph
                .records
                .windows(2)
                .all(|pair| pair[0].source_owner_id < pair[1].source_owner_id)
        );
        let plan = graph.plan_invalidation(&BTreeSet::from([[1; 16]]));
        assert_eq!(plan.affected_owners, BTreeSet::from([[1; 16], [2; 16]]));
        assert!(!plan.full_rebuild_required);
        assert_eq!(
            plan.invalidated_table_codes,
            BTreeSet::from([
                10, 180, 190, 200, 210, 220, 230, 240, 250, 260, 270, 280, 290, 300, 310, 320, 330,
                340, 350,
            ])
        );
    }

    #[test]
    fn wp43_negative_zero_state() {
        assert!(OperationalDependencyGraph::new(vec![edge(1, 2), edge(1, 2)]).is_err());
        let graph = OperationalDependencyGraph::new(vec![edge(1, 2)]).unwrap();
        let plan = graph.plan_invalidation(&BTreeSet::from([[9; 16]]));
        assert!(plan.full_rebuild_required);
        assert!(plan.unknown_changed_owners.contains(&[9; 16]));
    }

    #[test]
    fn wp44_behavioral_acceptance() {
        let mut incremental = FastSyntaxLane::new();
        incremental
            .apply(
                b"a.py".to_vec(),
                TreeSitterLanguage::Python,
                1,
                b"x = 1\n",
                None,
            )
            .unwrap();
        let changed = incremental
            .apply(
                b"a.py".to_vec(),
                TreeSitterLanguage::Python,
                2,
                b"x = 12\n",
                Some(TreeSitterEdit {
                    start_byte: 5,
                    old_end_byte: 5,
                    new_end_byte: 6,
                }),
            )
            .unwrap();
        let mut full = FastSyntaxLane::new();
        let rebuilt = full
            .apply(
                b"a.py".to_vec(),
                TreeSitterLanguage::Python,
                2,
                b"x = 12\n",
                None,
            )
            .unwrap();
        assert_eq!(changed.facts, rebuilt.facts);
    }

    #[test]
    fn wp44_operational_acceptance() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("workspace");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("a.py"), b"x = 1\n").unwrap();
        let mut store =
            OperationalStore::open(&directory.path().join("operational.sqlite")).unwrap();
        let workspace_id = WorkspaceRegistry::new(&mut store)
            .add(&root, WorkspaceSourceRegistration::Directory)
            .unwrap()
            .workspace_id;
        let mut source_images = SourceImageStore::open(
            &directory.path().join("source-blobs"),
            test_lifecycle_config().source_capture_policy(),
        )
        .unwrap();
        let mut scheduler =
            UpdateWaveScheduler::new(workspace_id, &root, 0, 0, 0, test_lifecycle_config())
                .unwrap();
        let hint = || WatchHintBatch {
            hints: vec![WatchHint {
                path_bytes: b"a.py".to_vec(),
                kind: WatchHintKind::CreateOrModify,
            }],
            rescan_required: false,
        };
        scheduler.admit_persisted(&mut store, hint()).unwrap();
        let mut first_wave = scheduler
            .prepare_wave(&mut store, &mut source_images, None)
            .unwrap()
            .unwrap();
        scheduler
            .transition(
                &mut store,
                &mut first_wave,
                "sources-classified",
                "classifications-closed",
            )
            .unwrap();
        let mut incremental = FastSyntaxReconciler::default();
        incremental
            .reconcile_wave(
                &first_wave,
                crate::identity::SOURCE_CONTEXT_ID,
                &BTreeMap::new(),
            )
            .unwrap();

        fs::write(root.join("a.py"), b"x = 12\n").unwrap();
        scheduler.admit_persisted(&mut store, hint()).unwrap();
        let mut second_wave = scheduler
            .prepare_wave(&mut store, &mut source_images, None)
            .unwrap()
            .unwrap();
        scheduler
            .transition(
                &mut store,
                &mut second_wave,
                "sources-classified",
                "classifications-closed",
            )
            .unwrap();
        let edits = BTreeMap::from([(
            b"a.py".to_vec(),
            TreeSitterEdit {
                start_byte: 5,
                old_end_byte: 5,
                new_end_byte: 6,
            },
        )]);
        let changed = incremental
            .reconcile_wave(&second_wave, crate::identity::SOURCE_CONTEXT_ID, &edits)
            .unwrap();
        let rebuilt = FastSyntaxReconciler::default()
            .reconcile_wave(
                &second_wave,
                crate::identity::SOURCE_CONTEXT_ID,
                &BTreeMap::new(),
            )
            .unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].tree_sitter.facts, rebuilt[0].tree_sitter.facts);
        let digests = |output: &FastSyntaxFactOutput| {
            output
                .canonical
                .batches
                .iter()
                .map(|(code, batch)| (*code, crate::fabric::batch_checksum(batch.batch()).unwrap()))
                .collect::<BTreeMap<_, _>>()
        };
        assert_eq!(digests(&changed[0]), digests(&rebuilt[0]));
    }

    #[test]
    fn wp44_structural_acceptance() {
        let mut lane = FastSyntaxLane::new();
        let parsed = lane
            .apply(
                b"a.py".to_vec(),
                TreeSitterLanguage::Python,
                1,
                b"def answer():\n    return 42\n",
                None,
            )
            .unwrap();
        assert!(!parsed.facts.is_empty());
        assert!(
            parsed
                .facts
                .iter()
                .all(|fact| fact.end_byte >= fact.start_byte)
        );
    }

    #[test]
    fn wp44_negative_zero_state() {
        let mut lane = FastSyntaxLane::new();
        assert!(
            lane.apply(
                b"a.py".to_vec(),
                TreeSitterLanguage::Python,
                1,
                b"\xff",
                None,
            )
            .is_err()
        );
        lane.apply(
            b"a.py".to_vec(),
            TreeSitterLanguage::Python,
            2,
            b"x = 1\n",
            None,
        )
        .unwrap();
        let fallback = lane
            .apply(
                b"a.py".to_vec(),
                TreeSitterLanguage::Python,
                3,
                b"x = 2\n",
                Some(TreeSitterEdit {
                    start_byte: 999,
                    old_end_byte: 999,
                    new_end_byte: 999,
                }),
            )
            .unwrap();
        assert!(!fallback.facts.is_empty());
    }

    #[tokio::test]
    async fn wp45_behavioral_acceptance() {
        let barrier = FreshnessBarrier::default();
        let watermark = barrier.admit();
        assert_eq!(barrier.state(), FreshnessState::PotentiallyStale);
        assert!(
            barrier
                .admit_query(
                    FreshnessAdmission::RequireCurrent,
                    std::time::Duration::from_millis(10)
                )
                .await
                .is_err()
        );
        barrier.reconcile(watermark);
        assert_eq!(
            barrier
                .admit_query(
                    FreshnessAdmission::AwaitLatest,
                    std::time::Duration::from_millis(10)
                )
                .await
                .unwrap(),
            FreshnessState::Current
        );
    }

    #[test]
    fn wp45_structural_acceptance() {
        let barrier = FreshnessBarrier::with_watermarks(7, 7, false);
        assert_eq!(barrier.state(), FreshnessState::Current);
        let recovered = FreshnessBarrier::with_watermarks(8, 7, false);
        assert_eq!(recovered.state(), FreshnessState::PotentiallyStale);
    }

    #[tokio::test]
    async fn wp45_negative_zero_state() {
        let barrier = FreshnessBarrier::default();
        let _ = barrier.admit();
        assert!(
            barrier
                .admit_query(
                    FreshnessAdmission::RequireCurrent,
                    std::time::Duration::from_millis(5),
                )
                .await
                .is_err()
        );
        barrier.mark_unavailable();
        assert_eq!(barrier.state(), FreshnessState::Unavailable);
        assert!(
            barrier
                .admit_query(
                    FreshnessAdmission::AwaitLatest,
                    std::time::Duration::from_millis(5),
                )
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn wp45_operational_acceptance() {
        let barrier = FreshnessBarrier::default();
        let watermark = barrier.admit();
        let left = barrier.clone();
        let right = barrier.clone();
        let first = tokio::spawn(async move {
            left.admit_query(
                FreshnessAdmission::AwaitLatest,
                std::time::Duration::from_secs(1),
            )
            .await
        });
        let second = tokio::spawn(async move {
            right
                .admit_query(
                    FreshnessAdmission::AwaitLatest,
                    std::time::Duration::from_secs(1),
                )
                .await
        });
        tokio::task::yield_now().await;
        barrier.reconcile(watermark);
        assert_eq!(first.await.unwrap().unwrap(), FreshnessState::Current);
        assert_eq!(second.await.unwrap().unwrap(), FreshnessState::Current);
    }

    #[test]
    fn wp46_structural_acceptance() {
        let policy = OverlayFlushPolicy {
            maximum_rows: 10,
            maximum_bytes: 1_000,
            maximum_touched_owners: 4,
            maximum_generations: 8,
        };
        assert!(!policy.requires_flush(OverlayPressure {
            rows: 9,
            bytes: 999,
            touched_owners: 3,
            generations_since_flush: 7,
        }));
        assert!(policy.requires_flush(OverlayPressure {
            rows: 10,
            bytes: 1,
            touched_owners: 1,
            generations_since_flush: 1,
        }));

        let mut overlays = ContinuousOverlayState::default();
        let first_mutation =
            OverlayMutation::owner_tombstone([1; 16], [2; 16], 8, 1, [3; 16], 1, 1).unwrap();
        let first = overlays
            .apply([1; 16], [2; 16], &[first_mutation], 1024 * 1024)
            .unwrap();
        assert_eq!(overlays.pressure().touched_owners, 1);
        assert!(
            overlays
                .flush_candidate(OverlayFlushPolicy {
                    maximum_rows: 1,
                    maximum_bytes: u64::MAX,
                    maximum_touched_owners: u64::MAX,
                    maximum_generations: u64::MAX,
                })
                .is_some()
        );
        let second_mutation =
            OverlayMutation::owner_tombstone([1; 16], [2; 16], 8, 2, [4; 16], 2, 1).unwrap();
        let second = overlays
            .apply([1; 16], [2; 16], &[second_mutation], 1024 * 1024)
            .unwrap();
        overlays.accept_rebase(&first, Arc::clone(&second)).unwrap();
        assert_eq!(overlays.pressure().generations_since_flush, 1);
    }

    #[test]
    fn wp46_behavioral_acceptance() {
        let mut overlays = ContinuousOverlayState::default();
        let flushed = overlays
            .apply(
                [1; 16],
                [2; 16],
                &[
                    OverlayMutation::owner_tombstone([1; 16], [2; 16], 8, 1, [3; 16], 1, 1)
                        .unwrap(),
                ],
                1024 * 1024,
            )
            .unwrap();
        let delta = overlays
            .apply(
                [1; 16],
                [2; 16],
                &[
                    OverlayMutation::owner_tombstone([1; 16], [2; 16], 8, 2, [4; 16], 2, 1)
                        .unwrap(),
                ],
                1024 * 1024,
            )
            .unwrap();
        overlays
            .accept_rebase(&flushed, Arc::clone(&delta))
            .unwrap();
        assert_eq!(overlays.current().unwrap().checksum(), delta.checksum());
        assert_eq!(overlays.pressure().generations_since_flush, 1);
    }

    #[test]
    fn wp46_negative_zero_state() {
        let mut overlays = ContinuousOverlayState::default();
        let flushed = overlays
            .apply(
                [1; 16],
                [2; 16],
                &[
                    OverlayMutation::owner_tombstone([1; 16], [2; 16], 8, 1, [3; 16], 1, 1)
                        .unwrap(),
                ],
                1024 * 1024,
            )
            .unwrap();
        let foreign = Arc::new(
            ConsolidatedOverlay::consolidate(OverlayConsolidationRequest {
                workspace_id: [9; 16],
                analysis_context_id: [2; 16],
                overlay_generation: 1,
                prior: None,
                incoming: &[],
                memory_limit_bytes: 1024 * 1024,
            })
            .unwrap(),
        );
        assert!(overlays.accept_rebase(&flushed, foreign).is_err());
        assert_eq!(overlays.current().unwrap().checksum(), flushed.checksum());
    }

    #[test]
    fn wp46_operational_acceptance() {
        for point in crate::fabric::OverlayRebaseFaultPoint::ALL {
            assert!(matches!(
                crate::fabric::test_rebase_fault(point),
                Err(FabricError::OverlayRebaseRestartRequired(_))
            ));
        }
    }

    #[test]
    fn wp47_structural_acceptance() {
        assert_eq!(
            recover_wave_state(30).unwrap(),
            PersistedWaveDisposition::RestartFromCurrentBytes
        );
        assert_eq!(
            recover_wave_state(50).unwrap(),
            PersistedWaveDisposition::RetireHistoricalTerminal
        );
        assert_eq!(
            recover_wave_state(i64::from(UpdateWaveState::DurableFlushing as u16)).unwrap(),
            PersistedWaveDisposition::RestartFromCurrentBytes
        );
        assert!(recover_wave_state(999).is_err());
    }

    #[test]
    fn wp47_operational_acceptance() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("workspace");
        fs::create_dir(&root).unwrap();
        let mut store =
            OperationalStore::open(&directory.path().join("operational.sqlite")).unwrap();
        let workspace_id = WorkspaceRegistry::new(&mut store)
            .add(&root, WorkspaceSourceRegistration::Directory)
            .unwrap()
            .workspace_id;
        let wave_id = [9_u8; 16];
        store
            .write_transaction(|transaction| {
                transaction.execute(
                    "UPDATE worktree_state SET event_watermark=7,reconcile_required=1 WHERE workspace_id=?1",
                    [workspace_id.as_slice()],
                )?;
                transaction.execute(
                    "INSERT INTO update_wave(wave_id,workspace_id,source_generation,event_watermark,state_code,candidate_strategy_code,input_fingerprint,candidate_count,started_at,terminal_at,diagnostic_id) VALUES (?1,?2,0,7,30,10,?3,1,'0',NULL,NULL)",
                    rusqlite::params![wave_id.as_slice(), workspace_id.as_slice(), [4_u8; 32].as_slice()],
                )?;
                transaction.execute(
                    "INSERT INTO update_wave_item(wave_id,item_ordinal,path_bytes,path_display,path_encoding_code,state_code,input_fingerprint,output_fingerprint,diagnostic_id) VALUES (?1,0,?2,'a.py',?3,10,?4,NULL,NULL)",
                    rusqlite::params![
                        wave_id.as_slice(),
                        b"a.py".as_slice(),
                        i64::from(platform_path_encoding() as u16),
                        [5_u8; 32].as_slice(),
                    ],
                )?;
                Ok::<_, OperationalStoreError>(())
            })
            .unwrap();
        let reader = store.reader_factory().open().unwrap();
        let recovery = recover_workspace(&reader, workspace_id).unwrap();
        assert!(recovery.full_inventory_required);
        assert_eq!(recovery.restart_paths, BTreeSet::from([b"a.py".to_vec()]));
        assert_eq!(
            recovery.waves[0].disposition,
            PersistedWaveDisposition::RestartFromCurrentBytes
        );
        let mut scheduler = UpdateWaveScheduler::new(
            workspace_id,
            &root,
            recovery.source_generation,
            recovery.event_watermark,
            0,
            test_lifecycle_config(),
        )
        .unwrap();
        scheduler.restore_recovery(&recovery).unwrap();
        let restored = scheduler.dirty.freeze();
        assert!(restored.rescan_required);
        assert!(restored.paths.contains_key(b"a.py".as_slice()));
    }

    #[test]
    fn wp47_behavioral_acceptance() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("workspace");
        fs::create_dir(&root).unwrap();
        let mut store = OperationalStore::open(&directory.path().join("state.sqlite")).unwrap();
        let workspace_id = WorkspaceRegistry::new(&mut store)
            .add(&root, WorkspaceSourceRegistration::Directory)
            .unwrap()
            .workspace_id;
        let recovery =
            recover_workspace(&store.reader_factory().open().unwrap(), workspace_id).unwrap();
        assert!(recovery.waves.is_empty());
        assert!(recovery.restart_paths.is_empty());
        assert!(!recovery.overlay_recovery_required);
    }

    #[test]
    fn wp47_negative_zero_state() {
        assert!(recover_wave_state(-1).is_err());
        assert!(recover_wave_state(999).is_err());
    }

    #[test]
    fn wp48_operational_acceptance() {
        let schema = Arc::new(arrow_schema::Schema::new(vec![arrow_schema::Field::new(
            "id",
            arrow_schema::DataType::Utf8,
            false,
        )]));
        let left_batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![Arc::new(arrow_array::StringArray::from(vec!["b", "a"]))],
        )
        .unwrap();
        let right_batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![Arc::new(arrow_array::StringArray::from(vec!["a", "b"]))],
        )
        .unwrap();
        let ignored = BTreeSet::new();
        let left = CanonicalState {
            tables: BTreeMap::from([(
                "entity".into(),
                canonical_table_state(
                    "entity",
                    Arc::clone(&schema),
                    &["id"],
                    &[left_batch],
                    &ignored,
                )
                .unwrap(),
            )]),
        };
        let right = CanonicalState {
            tables: BTreeMap::from([(
                "entity".into(),
                canonical_table_state("entity", schema, &["id"], &[right_batch], &ignored).unwrap(),
            )]),
        };
        assert_eq!(left.digest(), right.digest());
        assert!(left.prove_equivalent(&right).is_ok());
        let exact = left.clone();
        assert!(left.prove_equivalent(&exact).is_ok());
    }

    #[test]
    fn wp48_negative_zero_state() {
        let schema = Arc::new(arrow_schema::Schema::new(vec![arrow_schema::Field::new(
            "id",
            arrow_schema::DataType::Utf8,
            false,
        )]));
        let batch = |value: &'static str| {
            RecordBatch::try_new(
                Arc::clone(&schema),
                vec![Arc::new(arrow_array::StringArray::from(vec![value]))],
            )
            .unwrap()
        };
        let ignored = BTreeSet::new();
        let incremental = CanonicalState {
            tables: BTreeMap::from([(
                "entity".into(),
                canonical_table_state(
                    "entity",
                    Arc::clone(&schema),
                    &["id"],
                    &[batch("a")],
                    &ignored,
                )
                .unwrap(),
            )]),
        };
        let rebuilt = CanonicalState {
            tables: BTreeMap::from([(
                "entity".into(),
                canonical_table_state(
                    "entity",
                    Arc::clone(&schema),
                    &["id"],
                    &[batch("b")],
                    &ignored,
                )
                .unwrap(),
            )]),
        };
        assert!(incremental.prove_equivalent(&rebuilt).is_err());
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One negative oracle holds the complete AC-G-79 counterexample matrix.
    fn clean_rebuild_equivalence_adversarial() {
        use arrow_array::{Float64Array, StringArray};
        use arrow_schema::{DataType, Field, Schema};

        let schema_with_metadata = |metadata_value: &str| {
            Arc::new(Schema::new_with_metadata(
                vec![
                    Field::new("id", DataType::Utf8, false),
                    Field::new("value", DataType::Float64, true),
                    Field::new("operation_id", DataType::Utf8, false),
                ],
                std::collections::HashMap::from([("contract".into(), metadata_value.into())]),
            ))
        };
        let batch =
            |schema: SchemaRef, ids: Vec<&'static str>, values: Vec<Option<f64>>, op: &str| {
                let row_count = ids.len();
                RecordBatch::try_new(
                    schema,
                    vec![
                        Arc::new(StringArray::from(ids)),
                        Arc::new(Float64Array::from(values)),
                        Arc::new(StringArray::from(vec![op; row_count])),
                    ],
                )
                .unwrap()
            };
        let ignored = BTreeSet::from(["operation_id".to_owned()]);
        let schema = schema_with_metadata("v1");

        // Equal distinct sets and counts are insufficient: multiplicity is part of state.
        let left_bag = batch(
            Arc::clone(&schema),
            vec!["a", "a", "b"],
            vec![Some(1.0), Some(1.0), Some(2.0)],
            "left",
        );
        let right_bag = batch(
            Arc::clone(&schema),
            vec!["a", "b", "b"],
            vec![Some(1.0), Some(2.0), Some(2.0)],
            "right",
        );
        let left = canonical_table_state(
            "ungoverned",
            Arc::clone(&schema),
            &[],
            &[left_bag],
            &ignored,
        )
        .unwrap();
        let right = canonical_table_state(
            "ungoverned",
            Arc::clone(&schema),
            &[],
            &[right_bag],
            &ignored,
        )
        .unwrap();
        assert_ne!(left.row_multiplicities, right.row_multiplicities);

        // Null, NaN, missing/tombstoned rows, extra overlay rows, and Git-path divergence remain
        // ordinary logical row differences.
        let state = |ids, values, op| CanonicalState {
            tables: BTreeMap::from([(
                "facts".into(),
                canonical_table_state(
                    "facts",
                    Arc::clone(&schema),
                    &["id"],
                    &[batch(Arc::clone(&schema), ids, values, op)],
                    &ignored,
                )
                .unwrap(),
            )]),
        };
        let baseline = state(
            vec!["a", "b", "c"],
            vec![Some(1.0), None, Some(f64::NAN)],
            "one",
        );
        assert!(
            baseline
                .prove_equivalent(&state(
                    vec!["a", "b", "c"],
                    vec![Some(1.0), Some(0.0), Some(f64::NAN)],
                    "two",
                ))
                .is_err()
        );
        assert!(
            baseline
                .prove_equivalent(&state(
                    vec!["a", "b", "c"],
                    vec![Some(1.0), None, Some(3.0)],
                    "two",
                ))
                .is_err()
        );
        assert!(
            baseline
                .prove_equivalent(&state(
                    vec!["a", "b", "git-diverged"],
                    vec![Some(1.0), None, Some(f64::NAN)],
                    "two",
                ))
                .is_err()
        );
        assert!(
            baseline
                .prove_equivalent(&state(
                    vec!["a", "b"],
                    vec![Some(1.0), None],
                    "tombstone-corruption",
                ))
                .is_err()
        );
        assert!(
            baseline
                .prove_equivalent(&state(
                    vec!["a", "b", "c", "overlay-extra"],
                    vec![Some(1.0), None, Some(f64::NAN), Some(4.0)],
                    "overlay-extra",
                ))
                .is_err()
        );
        assert!(
            baseline
                .prove_equivalent(&state(
                    vec!["a", "b", "c"],
                    vec![Some(1.0), None, Some(f64::NAN)],
                    "different-operation",
                ))
                .is_ok()
        );

        let schema_v2 = schema_with_metadata("v2");
        let schema_state = CanonicalState {
            tables: BTreeMap::from([(
                "facts".into(),
                canonical_table_state(
                    "facts",
                    Arc::clone(&schema_v2),
                    &["id"],
                    &[batch(
                        schema_v2,
                        vec!["a", "b", "c"],
                        vec![Some(1.0), None, Some(f64::NAN)],
                        "one",
                    )],
                    &ignored,
                )
                .unwrap(),
            )]),
        };
        assert!(matches!(
            baseline.prove_equivalent(&schema_state),
            Err(LifecycleError::SchemaMismatch(_))
        ));

        let duplicate = batch(
            Arc::clone(&schema),
            vec!["a", "a", "b"],
            vec![Some(1.0), Some(2.0), Some(3.0)],
            "dup",
        );
        assert!(canonical_table_state("facts", schema, &["id"], &[duplicate], &ignored).is_err());
        assert_eq!(comparison_ignore_fields().unwrap(), {
            let parsed: crate::contracts::registry_models::AcceptedRegistry<
                crate::contracts::registry_models::ComparisonIgnoreRecord,
            > = serde_yaml_ng::from_str(include_str!(
                "../contracts/comparison/comparison-ignore-registry.yaml"
            ))
            .unwrap();
            parsed
                .records
                .into_iter()
                .map(|record| record.field_name)
                .collect()
        });
    }

    #[test]
    fn wp72_structural_acceptance() {
        let registry: crate::contracts::registry_models::AcceptedRegistry<
            crate::contracts::registry_models::ComparisonIgnoreRecord,
        > = serde_yaml_ng::from_str(include_str!(
            "../contracts/comparison/comparison-ignore-registry.yaml"
        ))
        .unwrap();
        crate::contracts::registry_models::validate_comparison_ignores(&registry.records).unwrap();
        assert_eq!(registry.records.len(), 32);
        assert!(registry.records.iter().all(|record| !record.semantic));
        let ignored = comparison_ignore_fields().unwrap();
        for projection in serving_projection_specs() {
            let spec = table_spec(projection.source_table_code).unwrap();
            let comparison = comparison_projection_spec(projection.source_table_code).unwrap();
            assert_eq!(comparison.primary_sort_key, spec.primary_key);
            assert!(
                comparison
                    .primary_sort_key
                    .iter()
                    .any(|field| !ignored.contains(*field)),
                "{} loses its complete governed key after comparison projection",
                projection.view_name
            );
            let canonical_schema = crate::contracts::jcs::canonicalize_value(
                &serde_json::to_value(spec.arrow_schema.as_ref()).unwrap(),
            )
            .unwrap();
            assert!(!canonical_schema.is_empty());
        }
        let diagnostic = comparison_projection_spec(10).unwrap();
        assert!(!diagnostic.primary_sort_key.is_empty());
        assert_eq!(
            comparison_projection_spec(130)
                .unwrap()
                .foreign_key_normalization,
            Some(ComparisonForeignKeyNormalization::EvidenceContentIdentityV1)
        );
        assert!(ignored.contains("provider_run_id"));
        assert!(ignored.contains("source_generation"));
        assert!(!ignored.contains("fact_id"));
    }
}
