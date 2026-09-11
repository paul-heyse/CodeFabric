//! Workspace-owned change observation. Events request a census; they never establish facts.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use notify_debouncer_full::notify::{
    Config, EventKind, PollWatcher, RecommendedWatcher, RecursiveMode,
};

#[cfg(all(test, target_os = "linux"))]
mod backend_join_tests;
use notify_debouncer_full::{
    DebounceEventResult, Debouncer, NoCache, RecommendedCache, new_debouncer_opt,
};
use tokio::sync::mpsc;

use crate::daemon::SourceWatchProfile;
use crate::freshness::FreshnessBarrier;
use crate::git_state::watch_topology::{GitWatchTopology, SelectedGitWatchInputs};
use crate::source_inclusion::SourceInclusionPolicy;

/// Keep native error kinds and affected paths until the startup/control presentation boundary.
#[derive(Debug, thiserror::Error)]
enum WorkspaceWatchError {
    #[error("source watcher {profile:?} {operation}: {source}{capacity}")]
    Backend {
        profile: SourceWatchProfile,
        operation: &'static str,
        #[source]
        source: notify_debouncer_full::notify::Error,
        capacity: String,
    },
    #[error("{0}")]
    Setup(String),
}

impl From<String> for WorkspaceWatchError {
    fn from(error: String) -> Self {
        Self::Setup(error)
    }
}

impl WorkspaceWatchError {
    fn backend(
        profile: SourceWatchProfile,
        operation: &'static str,
        source: notify_debouncer_full::notify::Error,
    ) -> Self {
        // Called only on the blocking watcher owner, never on the notification callback.
        // An unavailable diagnostic must not replace the original native error.
        #[cfg(target_os = "linux")]
        let capacity = {
            let limit = |name: &str| {
                std::fs::read_to_string(format!("/proc/sys/fs/inotify/{name}"))
                    .ok()
                    .and_then(|value| value.trim().parse::<u64>().ok())
            };
            format!(
                "; host limits: inotify max_user_instances={:?}, max_user_watches={:?}, \
                 max_queued_events={:?}, open_files={:?}",
                limit("max_user_instances"),
                limit("max_user_watches"),
                limit("max_queued_events"),
                rustix::process::getrlimit(rustix::process::Resource::Nofile),
            )
        };
        #[cfg(not(target_os = "linux"))]
        let capacity = String::new();
        Self::Backend {
            profile,
            operation,
            source,
            capacity,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SourceInventoryState {
    pub(crate) digest: [u8; 32],
    pub(crate) semantic_pending: bool,
    pub(crate) provider_deployment_digest: Option<[u8; 32]>,
    pub(crate) git_context_digest: Option<[u8; 32]>,
}

impl SourceInventoryState {
    pub(crate) fn matches_inputs(
        &self,
        digest: [u8; 32],
        deployment: [u8; 32],
        git_context: [u8; 32],
    ) -> bool {
        self.digest == digest
            && self.provider_deployment_digest == Some(deployment)
            && self.git_context_digest == Some(git_context)
    }
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
    // Older exact epochs have no deployment observation and must be reconciled before
    // current semantics are established. Their historical facts remain readable.
    let provider_deployment_digest = batch
        .column_by_name("provider_deployment_digest")
        .map(|_| {
            fixed("provider_deployment_digest")?
                .value(0)
                .try_into()
                .map_err(|_| "invalid provider deployment digest width".to_owned())
        })
        .transpose()?;
    let git_context_digest = batch
        .column_by_name("git_context_digest")
        .map(|_| {
            fixed("git_context_digest")?
                .value(0)
                .try_into()
                .map_err(|_| "invalid Git context digest width".to_owned())
        })
        .transpose()?;
    Ok(Some(SourceInventoryState {
        digest,
        semantic_pending,
        provider_deployment_digest,
        git_context_digest,
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
    topology_revision: Arc<AtomicU64>,
    topology_installations: Arc<AtomicU64>,
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
                topology_revision: Arc::new(AtomicU64::new(0)),
                topology_installations: Arc::new(AtomicU64::new(0)),
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
        profile: SourceWatchProfile,
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
        let budget = budget.clone();
        scope
            .spawn_blocking_owned("native-owner", resources, move |cancellation| {
                let mut revision = observed.topology_revision.load(Ordering::Acquire);
                let mut watch = match observed.watch(&root, &budget, &cancellation, profile) {
                    Ok(watch) => watch,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                observed
                    .topology_installations
                    .fetch_add(1, Ordering::AcqRel);
                if ready.send(Ok(())).is_err() {
                    watch.stop();
                    return;
                }
                let mut retry_at = Instant::now();
                while !cancellation.is_cancelled() {
                    match receiver.recv_timeout(Duration::from_millis(20)) {
                        Ok(()) => {
                            observed.topology_revision.fetch_add(1, Ordering::AcqRel);
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                    let requested = observed.topology_revision.load(Ordering::Acquire);
                    if (requested != revision || !observed.watch_healthy())
                        && Instant::now() >= retry_at
                    {
                        // Keep old registrations (especially the parent) until their replacement
                        // is installed. Events arriving during the walk remain a pending revision.
                        match observed.watch(&root, &budget, &cancellation, profile) {
                            Ok(replacement) => {
                                let old = std::mem::replace(&mut watch, replacement);
                                old.stop();
                                revision = requested;
                                observed
                                    .topology_installations
                                    .fetch_add(1, Ordering::AcqRel);
                            }
                            Err(error) => {
                                if observed.watch_healthy.swap(false, Ordering::AcqRel) {
                                    tracing::warn!(%error, "source watch topology unavailable");
                                }
                            }
                        }
                        observed.request(true);
                        retry_at = Instant::now() + Duration::from_millis(250);
                    }
                }
                watch.stop();
                observed.watch_healthy.store(false, Ordering::Release);
            })
            .await
            .map_err(|error| error.to_string())?;
        installed
            .await
            .map_err(|_| "native watcher ended before installation".to_owned())?
            .map_err(|error| error.to_string())?;
        Ok(WorkspaceWatchControl { commands })
    }

    fn observe_events(
        &self,
        root: &Path,
        git: &crate::git_state::watch_topology::SelectedGitWatchInputs,
        inclusion: &SourceInclusionPolicy,
        profile: SourceWatchProfile,
        events: DebounceEventResult,
    ) {
        if let Ok(events) = events {
            let rescan = events.iter().any(|event| event.need_rescan());
            let relevant = |event: &notify_debouncer_full::DebouncedEvent| {
                !matches!(event.kind, EventKind::Access(_))
                    && (event.paths.is_empty()
                        || event.paths.iter().any(|path| {
                            relevant_with_policy(root, path, inclusion)
                                || git.relevant(path)
                                || git_marker(root, path, inclusion)
                        }))
            };
            let topology_changed = rescan
                || events.iter().any(|event| {
                    relevant(event)
                        && (event.paths.iter().any(|path| {
                            git.topology_relevant(path)
                                || git_marker(root, path, inclusion)
                                || path.strip_prefix(root).is_ok_and(|relative| {
                                    inclusion
                                        .configuration_path(relative.as_os_str().as_encoded_bytes())
                                })
                        }) || matches!(
                            event.kind,
                            EventKind::Any
                                | EventKind::Other
                                | EventKind::Create(_)
                                | EventKind::Remove(_)
                                | EventKind::Modify(
                                    notify_debouncer_full::notify::event::ModifyKind::Name(_)
                                )
                        ))
                });
            if topology_changed {
                self.topology_revision.fetch_add(1, Ordering::AcqRel);
            }
            if rescan || events.iter().any(relevant) {
                self.event(rescan || topology_changed);
            }
        } else if let Err(errors) = events {
            for error in errors {
                tracing::warn!(?profile, ?error, "source watcher backend callback failed");
            }
            self.watch_healthy.store(false, Ordering::Release);
            self.topology_revision.fetch_add(1, Ordering::AcqRel);
            self.event(true);
        }
    }

    #[allow(clippy::too_many_lines)] // Setup, registered topology and callback publication share one coherence boundary.
    fn watch(
        &self,
        root: &Path,
        budget: &crate::resource_budget::ResourceBudget,
        cancellation: &crate::cancellation::Cancellation,
        profile: SourceWatchProfile,
    ) -> Result<WorkspaceWatch, WorkspaceWatchError> {
        let started = Instant::now();
        let observed = self.clone();
        let selected_root = root.to_owned();
        let initial_git =
            SelectedGitWatchInputs::from_topologies(&[GitWatchTopology::resolve(root)]);
        let selected_git = Arc::new(OnceLock::<SelectedGitWatchInputs>::new());
        let mut inclusion = SourceInclusionPolicy::capture_watch(root);
        let retained = budget
            .try_reserve(
                crate::resource_budget::ResourceClass::Control,
                crate::resource_budget::ResourceAmounts {
                    memory_bytes: initial_git.retained_bytes() + 2 * inclusion.retained_bytes(),
                    ..crate::resource_budget::ResourceAmounts::default()
                },
            )
            .map_err(|error| error.to_string())?;
        let metadata = Arc::clone(&selected_git);
        let selected_policy = Arc::new(OnceLock::<SourceInclusionPolicy>::new());
        let callback_policy = Arc::clone(&selected_policy);
        let handler = move |events: DebounceEventResult| {
            let Some(policy) = callback_policy.get() else {
                // Installation can discover new selected descendants. Retain events until the
                // complete immutable callback policy is published, including topology repair.
                match events {
                    Err(errors) => observed.observe_events(
                        &selected_root,
                        metadata.get().unwrap_or(&initial_git),
                        &SourceInclusionPolicy::default(),
                        profile,
                        Err(errors),
                    ),
                    Ok(events)
                        if events
                            .iter()
                            .any(|event| !matches!(event.kind, EventKind::Access(_))) =>
                    {
                        observed.topology_revision.fetch_add(1, Ordering::AcqRel);
                        observed.event(true);
                    }
                    Ok(_) => {}
                }
                return;
            };
            observed.observe_events(
                &selected_root,
                metadata.get().unwrap_or(&initial_git),
                policy,
                profile,
                events,
            );
        };
        let config = Config::default()
            .with_follow_symlinks(false)
            .with_join_on_drop(true);
        let watcher = match profile {
            SourceWatchProfile::Native => WatchBackend::Native(
                new_debouncer_opt(
                    Duration::from_millis(75),
                    Some(Duration::from_millis(20)),
                    handler,
                    RecommendedCache::default(),
                    config,
                )
                .map_err(|error| WorkspaceWatchError::backend(profile, "construction", error))?,
            ),
            SourceWatchProfile::Poll => WatchBackend::Poll(
                new_debouncer_opt(
                    Duration::from_millis(150),
                    Some(Duration::from_millis(50)),
                    handler,
                    NoCache,
                    config
                        .with_poll_interval(Duration::from_secs(2))
                        .with_compare_contents(false),
                )
                .map_err(|error| WorkspaceWatchError::backend(profile, "construction", error))?,
            ),
        };
        // Wrap immediately: even a failed traversal must join the native debouncer thread.
        let mut watch = WorkspaceWatch {
            watcher: Some(watcher),
            profile,
            retained,
            registered_paths: BTreeSet::new(),
            source_repository_roots: BTreeSet::from([root.to_owned()]),
            registered_directories: 0,
            excluded_directories: 0,
        };
        watch.install(root, &mut inclusion, cancellation)?;
        let git = watch.resolve_git_inputs(started, cancellation)?;
        let selection = SelectedGitWatchInputs::from_topologies(&git);
        watch
            .retained
            .try_grow(crate::resource_budget::ResourceAmounts {
                memory_bytes: selection.retained_bytes(),
                ..crate::resource_budget::ResourceAmounts::default()
            })
            .map_err(|error| error.to_string())?;
        for directory in selection.directories() {
            if cancellation.is_cancelled() {
                return Err("Git metadata watch installation cancelled"
                    .to_owned()
                    .into());
            }
            watch.register(directory)?;
        }
        // A .git pointer or info-directory change before its registration must retain a
        // repair obligation even if no backend event covered that installation interval.
        let checked_git = watch.resolve_git_inputs(started, cancellation)?;
        let coherent = git == checked_git
            && selection.directories()
                == SelectedGitWatchInputs::from_topologies(&checked_git).directories()
            && inclusion.unchanged_watch(root);
        // The installer publishes once. Later changes replace the complete owned watcher;
        // callbacks neither lock a mutable repository map nor perform discovery.
        let _ = selected_git.set(selection);
        let _ = selected_policy.set(inclusion);
        if !coherent {
            self.topology_revision.fetch_add(1, Ordering::AcqRel);
        }
        self.watch_healthy.store(coherent, Ordering::Release);
        self.request(true);
        tracing::info!(
            registered_directories = watch.registered_directories,
            excluded_directories = watch.excluded_directories,
            "source watch topology installed"
        );
        Ok(watch)
    }
}

fn relevant_with_policy(root: &Path, path: &Path, policy: &SourceInclusionPolicy) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    policy.includes(relative.as_os_str().as_encoded_bytes(), false)
}

fn git_marker(root: &Path, path: &Path, policy: &SourceInclusionPolicy) -> bool {
    path.file_name().is_some_and(|name| name == ".git")
        && path
            .parent()
            .and_then(|parent| parent.strip_prefix(root).ok())
            .is_some_and(|relative| policy.includes(relative.as_os_str().as_encoded_bytes(), true))
}

#[cfg(test)]
fn relevant(root: &Path, path: &Path) -> bool {
    relevant_with_policy(root, path, &SourceInclusionPolicy::default())
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

/// The owning blocking operation joins the debouncer and the selected Linux/poll backend.
/// Other platform backend joining remains outside the qualified Linux deployment profile.
enum WatchBackend {
    Native(Debouncer<RecommendedWatcher, RecommendedCache>),
    Poll(Debouncer<PollWatcher, NoCache>),
}

impl WatchBackend {
    fn watch(&mut self, path: &Path) -> notify_debouncer_full::notify::Result<()> {
        match self {
            Self::Native(watch) => watch.watch(path, RecursiveMode::NonRecursive),
            Self::Poll(watch) => watch.watch(path, RecursiveMode::NonRecursive),
        }
    }

    fn stop(self) {
        match self {
            Self::Native(watch) => watch.stop(),
            Self::Poll(watch) => watch.stop(),
        }
    }
}

/// Native registrations and declared bookkeeping share this owned blocking lifetime.
pub(crate) struct WorkspaceWatch {
    watcher: Option<WatchBackend>,
    profile: SourceWatchProfile,
    retained: crate::resource_budget::ResourceReservation,
    registered_paths: BTreeSet<PathBuf>,
    source_repository_roots: BTreeSet<PathBuf>,
    registered_directories: u64,
    excluded_directories: u64,
}

impl WorkspaceWatch {
    fn resolve_git_inputs(
        &self,
        started: Instant,
        cancellation: &crate::cancellation::Cancellation,
    ) -> Result<Vec<crate::git_state::watch_topology::GitWatchTopology>, WorkspaceWatchError> {
        self.source_repository_roots
            .iter()
            .map(|root| {
                if cancellation.is_cancelled()
                    || started.elapsed()
                        > crate::inventory::InventoryLimits::default().maximum_duration
                {
                    return Err(
                        "selected Git metadata watch resolution cancelled or timed out"
                            .to_owned()
                            .into(),
                    );
                }
                Ok(crate::git_state::watch_topology::GitWatchTopology::resolve(
                    root,
                ))
            })
            .collect()
    }

    fn reserve_path(&mut self, path: &Path) -> Result<(), String> {
        self.retained
            .try_grow(crate::resource_budget::ResourceAmounts {
                // Declared bookkeeping capacity for native/application paths and the walk stack.
                // This does not claim to measure kernel inotify memory or every native allocation.
                memory_bytes: 512 + 4 * path.as_os_str().len() as u64,
                ..crate::resource_budget::ResourceAmounts::default()
            })
            .map_err(|error| error.to_string())
    }

    fn register(&mut self, path: &Path) -> Result<(), WorkspaceWatchError> {
        if self.registered_paths.contains(path) {
            return Ok(());
        }
        if self.registered_directories
            >= crate::inventory::InventoryLimits::default().maximum_directory_count
        {
            return Err("source and metadata watch registration bound exceeded"
                .to_owned()
                .into());
        }
        self.reserve_path(path)?;
        self.watcher
            .as_mut()
            .ok_or_else(|| "watcher stopped".to_owned())?
            .watch(path)
            .map_err(|error| {
                WorkspaceWatchError::backend(
                    self.profile,
                    "registration",
                    error.add_path(path.to_owned()),
                )
            })?;
        self.registered_paths.insert(path.to_owned());
        self.registered_directories += 1;
        Ok(())
    }

    fn install(
        &mut self,
        root: &Path,
        inclusion: &mut SourceInclusionPolicy,
        cancellation: &crate::cancellation::Cancellation,
    ) -> Result<(), WorkspaceWatchError> {
        let limits = crate::inventory::InventoryLimits::default();
        let started = Instant::now();
        // The parent remains observed while a registered root disappears or is recreated.
        if let Some(parent) = root.parent() {
            self.register(parent)?;
        }
        self.reserve_path(root)?;
        let mut pending = vec![(root.to_owned(), 0)];
        let mut discovered = 1_u64;
        let mut files = 0_u64;
        while let Some((directory, depth)) = pending.pop() {
            if cancellation.is_cancelled() || started.elapsed() > limits.maximum_duration {
                return Err("source watch traversal cancelled or timed out"
                    .to_owned()
                    .into());
            }
            let metadata =
                std::fs::symlink_metadata(&directory).map_err(|error| error.to_string())?;
            if !metadata.is_dir() {
                return Err("source watch directory changed during traversal"
                    .to_owned()
                    .into());
            }
            // Registration precedes child enumeration. Events only request secure recapture;
            // lexical watch paths never authorize source reads or establish content identity.
            self.register(&directory)?;
            let relative = directory
                .strip_prefix(root)
                .map_err(|error| error.to_string())?;
            let before = inclusion.retained_bytes();
            inclusion.observe_watch_directory(root, relative.as_os_str().as_encoded_bytes());
            self.retained
                .try_grow(crate::resource_budget::ResourceAmounts {
                    memory_bytes: 2 * inclusion.retained_bytes().saturating_sub(before),
                    ..crate::resource_budget::ResourceAmounts::default()
                })
                .map_err(|error| error.to_string())?;

            for (index, entry) in std::fs::read_dir(&directory)
                .map_err(|error| error.to_string())?
                .enumerate()
            {
                if cancellation.is_cancelled()
                    || started.elapsed() > limits.maximum_duration
                    || index >= limits.maximum_entries_per_directory
                {
                    return Err("source watch traversal cancelled or exceeded its bound"
                        .to_owned()
                        .into());
                }
                let entry = entry.map_err(|error| error.to_string())?;
                if entry.file_name() == ".git" {
                    self.reserve_path(&directory)?;
                    self.source_repository_roots.insert(directory.clone());
                }
                if !entry
                    .file_type()
                    .map_err(|error| error.to_string())?
                    .is_dir()
                {
                    files += 1;
                    if files > limits.maximum_file_count {
                        return Err("source watch file bound exceeded".to_owned().into());
                    }
                    // PollWatcher retains metadata for immediate child files. Native platforms
                    // may retain IDs too. Keep a broad declared path budget for either backend.
                    self.reserve_path(&entry.path())?;
                    continue;
                }
                let path = entry.path();
                let relative = path.strip_prefix(root).map_err(|error| error.to_string())?;
                if !inclusion.includes(relative.as_os_str().as_encoded_bytes(), true) {
                    self.excluded_directories += 1;
                    continue;
                }
                discovered += 1;
                if discovered > limits.maximum_directory_count
                    || depth >= limits.maximum_directory_depth
                {
                    return Err("source watch directory/depth bound exceeded"
                        .to_owned()
                        .into());
                }
                self.reserve_path(&path)?;
                pending.push((path, depth + 1));
            }
        }
        Ok(())
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

    #[test]
    fn legacy_inventory_requires_deployment_reconciliation_and_changes_invalidate_matches() {
        let mut state = SourceInventoryState {
            digest: [1; 32],
            semantic_pending: false,
            provider_deployment_digest: None,
            git_context_digest: None,
        };
        assert!(!state.matches_inputs([1; 32], [0; 32], [4; 32]));
        state.provider_deployment_digest = Some([2; 32]);
        assert!(!state.matches_inputs([1; 32], [2; 32], [4; 32]));
        state.git_context_digest = Some([4; 32]);
        assert!(state.matches_inputs([1; 32], [2; 32], [4; 32]));
        assert!(!state.matches_inputs([1; 32], [3; 32], [4; 32]));
        assert!(!state.matches_inputs([3; 32], [2; 32], [4; 32]));
        assert!(!state.matches_inputs([1; 32], [2; 32], [5; 32]));
    }

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
            "/workspace/target",
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
        let budget = crate::provider_types::source_fixture_budget([2; 16]);
        let mut watch = observed
            .watch(
                &root,
                &budget,
                &crate::cancellation::Cancellation::default(),
                SourceWatchProfile::Native,
            )
            .unwrap();
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
        let missing = root.join("missing-directory");
        let error = watch.register(&missing).unwrap_err();
        assert!(error.to_string().contains("Native registration"));
        let WorkspaceWatchError::Backend { source, .. } = error else {
            panic!("native registration error lost its category");
        };
        assert!(matches!(
            source.kind,
            notify_debouncer_full::notify::ErrorKind::PathNotFound
        ));
        assert!(source.paths.contains(&missing));
        watch.stop();
    }

    #[tokio::test]
    async fn selected_nested_linked_repository_observes_external_metadata_without_object_watches() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("source");
        let nested = root.join("nested");
        let main = fixture.path().join("external-main");
        let administrative = crate::git_state::watch_topology::linked_fixture(&nested, &main);
        let exclude = main.join(".git/info/exclude");
        std::fs::write(&exclude, "initial\n").unwrap();
        let (observed, mut receiver) = WorkspaceObservation::new();
        let budget = crate::provider_types::source_fixture_budget([2; 16]);
        let watch = observed
            .watch(
                &root,
                &budget,
                &crate::cancellation::Cancellation::default(),
                SourceWatchProfile::Native,
            )
            .unwrap();
        assert!(watch.registered_paths.contains(&administrative));
        assert!(watch.registered_paths.contains(&main.join(".git/info")));
        assert!(!watch.registered_paths.contains(&main.join(".git/objects")));
        assert!(watch.source_repository_roots.contains(&nested));
        receiver.recv().await.unwrap();
        await_watched_edit(&observed, &mut receiver, &exclude).await;
        watch.stop();
    }

    #[tokio::test]
    async fn selected_dependency_topology_observes_edits_and_configuration_reselection() {
        selected_root_topology(
            "pyrefly.toml",
            "site-package-path=['.venv/lib/site-packages']\n",
            "site-package-path=[]\n",
            SourceWatchProfile::Native,
        )
        .await;
    }

    #[tokio::test]
    async fn selected_submodule_native_topology_observes_inputs_and_retracts_declarations() {
        selected_root_topology(
            ".gitmodules",
            "[submodule \"vendor\"]\npath = .venv/lib/site-packages\n",
            "",
            SourceWatchProfile::Native,
        )
        .await;
    }

    #[tokio::test]
    async fn selected_submodule_poll_topology_observes_inputs_and_retracts_declarations() {
        selected_root_topology(
            ".gitmodules",
            "[submodule \"vendor\"]\npath = .venv/lib/site-packages\n",
            "",
            SourceWatchProfile::Poll,
        )
        .await;
    }

    #[tokio::test]
    async fn recursive_submodule_native_topology_reselects_nested_configuration() {
        selected_root_topology(
            "nested/.gitmodules",
            "[submodule \"dependency\"]\npath = .venv/lib/site-packages\n",
            "",
            SourceWatchProfile::Native,
        )
        .await;
    }

    #[tokio::test]
    async fn recursive_submodule_poll_topology_reselects_nested_configuration() {
        selected_root_topology(
            "nested/.gitmodules",
            "[submodule \"dependency\"]\npath = .venv/lib/site-packages\n",
            "",
            SourceWatchProfile::Poll,
        )
        .await;
    }

    async fn selected_root_topology(
        config_name: &str,
        initial: &str,
        replacement: &str,
        profile: SourceWatchProfile,
    ) {
        use std::fs;
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("source");
        let config = root.join(config_name);
        let context_root = config.parent().unwrap();
        fs::create_dir_all(context_root.join(".venv/lib/site-packages/pkg")).unwrap();
        fs::create_dir_all(context_root.join(".venv/bin")).unwrap();
        fs::write(&config, initial).unwrap();
        let (observed, mut receiver) = WorkspaceObservation::new();
        let budget = crate::provider_types::source_fixture_budget([2; 16]);
        let watch = observed
            .watch(
                &root,
                &budget,
                &crate::cancellation::Cancellation::default(),
                profile,
            )
            .unwrap();
        assert!(
            watch
                .registered_paths
                .contains(&context_root.join(".venv/lib/site-packages/pkg"))
        );
        assert!(
            !watch
                .registered_paths
                .contains(&context_root.join(".venv/bin"))
        );
        receiver.recv().await.unwrap();
        let revision = observed.event_revision();
        fs::write(
            context_root.join(".venv/lib/site-packages/pkg/__init__.pyi"),
            "def changed() -> int: ...\n",
        )
        .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while observed.event_revision() == revision {
                receiver.recv().await.unwrap();
            }
        })
        .await
        .unwrap();
        let topology = observed.topology_revision.load(Ordering::Acquire);
        fs::write(&config, replacement).unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while observed.topology_revision.load(Ordering::Acquire) == topology {
                receiver.recv().await.unwrap();
            }
        })
        .await
        .unwrap();
        watch.stop();
        let replacement = observed
            .watch(
                &root,
                &budget,
                &crate::cancellation::Cancellation::default(),
                profile,
            )
            .unwrap();
        assert!(
            !replacement
                .registered_paths
                .contains(&context_root.join(".venv"))
        );
        replacement.stop();
    }

    #[tokio::test]
    async fn native_watch_prunes_cache_trees_and_symlinks_before_registration() {
        use std::fs;
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("source");
        fs::create_dir_all(root.join("src/nested")).unwrap();
        let excluded = ["target", ".venv", "node_modules", "__pycache__", ".git"];
        for name in excluded {
            for index in 0..20 {
                fs::create_dir_all(root.join(name).join(format!("cache-{index}/nested"))).unwrap();
            }
        }
        let outside = fixture.path().join("outside");
        fs::create_dir(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
        let (observed, mut receiver) = WorkspaceObservation::new();
        let budget = crate::provider_types::source_fixture_budget([2; 16]);
        let before = budget.observation().used.memory_bytes;
        let watch = observed
            .watch(
                &root,
                &budget,
                &crate::cancellation::Cancellation::default(),
                SourceWatchProfile::Native,
            )
            .unwrap();
        assert_eq!(watch.registered_directories, 4); // parent, root, src, nested
        assert_eq!(watch.excluded_directories, 5);
        receiver.recv().await.unwrap();
        let event = observed.event_revision();
        fs::write(root.join("target/cache-0/nested/generated.rs"), "ignored").unwrap();
        fs::write(outside.join("outside.py"), "unselected").unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(250), receiver.recv())
                .await
                .is_err()
        );
        assert_eq!(observed.event_revision(), event);
        fs::write(root.join("src/nested/current.py"), "selected").unwrap();
        tokio::time::timeout(Duration::from_secs(5), receiver.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(observed.event_revision() > event);
        watch.stop();
        assert_eq!(budget.observation().used.memory_bytes, before);
    }

    #[tokio::test]
    async fn native_watch_observes_linked_git_metadata_and_retargets_its_owned_topology() {
        exercise_linked_git_watch(SourceWatchProfile::Native).await;
    }

    #[tokio::test]
    async fn poll_watch_observes_linked_git_metadata_and_retargets_its_owned_topology() {
        exercise_linked_git_watch(SourceWatchProfile::Poll).await;
    }

    async fn exercise_linked_git_watch(profile: SourceWatchProfile) {
        use std::{fs, num::NonZeroUsize};
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("selected");
        let main = fixture.path().join("main");
        let administrative = crate::git_state::watch_topology::linked_fixture(&root, &main);
        fs::create_dir_all(main.join(".git/info")).unwrap();
        // The selected PollWatcher profile uses whole-second metadata hints. Make this
        // existing-file edit observable; same-second changes rely on periodic secure census.
        let exclude = main.join(".git/info/exclude");
        fs::write(&exclude, "initial policy\n").unwrap();
        fs::File::open(&exclude)
            .unwrap()
            .set_times(
                fs::FileTimes::new()
                    .set_modified(std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(10)),
            )
            .unwrap();
        let (observed, mut receiver) = WorkspaceObservation::new();
        let budget = crate::provider_types::source_fixture_budget([2; 16]);
        let scope =
            crate::cancellation::StructuredCancellationScope::try_root_with_control_reserve(
                "git-watch-test",
                NonZeroUsize::new(8).unwrap(),
                NonZeroUsize::new(4).unwrap(),
            )
            .unwrap();
        let control = observed
            .start_watch(root.clone(), &budget, profile, &scope)
            .await
            .unwrap();
        receiver.recv().await.unwrap();
        await_watched_edit(&observed, &mut receiver, &exclude).await;
        await_watched_edit(&observed, &mut receiver, &administrative.join("index")).await;
        let topology = observed.topology_installations.load(Ordering::Acquire);
        control.reinstall();
        await_new_watch(&observed, topology).await;
        while receiver.try_recv().is_ok() {}
        let before = observed.event_revision();
        fs::create_dir_all(main.join(".git/objects/pack")).unwrap();
        fs::write(main.join(".git/objects/pack/unrelated.pack"), "unselected").unwrap();
        // The poll backend must complete at least one configured two-second cycle too.
        tokio::time::sleep(Duration::from_millis(2500)).await;
        assert_eq!(observed.event_revision(), before);
        let topology = observed.topology_installations.load(Ordering::Acquire);
        let replacement =
            crate::git_state::watch_topology::linked_fixture(&root, &fixture.path().join("other"));
        await_new_watch(&observed, topology).await;
        await_watched_edit(&observed, &mut receiver, &replacement.join("index")).await;
        drop(control);
        scope.cancel_and_join(Duration::from_secs(5)).await.unwrap();
        assert!(!observed.watch_healthy());
        assert_eq!(budget.observation().used.memory_bytes, 0);
    }

    #[tokio::test]
    async fn owned_watch_recovers_new_directories_and_recreated_roots_then_joins() {
        exercise_owned_watch(SourceWatchProfile::Native).await;
    }

    #[tokio::test]
    async fn explicit_poll_watch_recovers_topology_and_joins() {
        exercise_owned_watch(SourceWatchProfile::Poll).await;
    }

    async fn exercise_owned_watch(profile: SourceWatchProfile) {
        use std::{fs, num::NonZeroUsize};
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("source");
        fs::create_dir(&root).unwrap();
        let (observed, mut receiver) = WorkspaceObservation::new();
        let budget = crate::provider_types::source_fixture_budget([2; 16]);
        let scope =
            crate::cancellation::StructuredCancellationScope::try_root_with_control_reserve(
                "watch-test",
                NonZeroUsize::new(8).unwrap(),
                NonZeroUsize::new(4).unwrap(),
            )
            .unwrap();
        let control = observed
            .start_watch(root.clone(), &budget, profile, &scope)
            .await
            .unwrap();
        receiver.recv().await.unwrap();
        let topology = observed.topology_installations.load(Ordering::Acquire);
        fs::create_dir_all(root.join("new/nested")).unwrap();
        await_new_watch(&observed, topology).await;
        let file = root.join("new/nested/a.py");
        await_watched_edit(&observed, &mut receiver, &file).await;
        let topology = observed.topology_installations.load(Ordering::Acquire);
        // A forced repair arriving inside the coalescing interval must remain pending.
        control.reinstall();
        await_new_watch(&observed, topology).await;

        fs::remove_dir_all(&root).unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while observed.watch_healthy() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let topology = observed.topology_installations.load(Ordering::Acquire);
        fs::create_dir_all(root.join("replacement")).unwrap();
        await_new_watch(&observed, topology).await;
        await_watched_edit(&observed, &mut receiver, &root.join("replacement/b.py")).await;
        drop(control);
        scope.cancel_and_join(Duration::from_secs(5)).await.unwrap();
        assert!(!observed.watch_healthy());
        assert_eq!(budget.observation().used.memory_bytes, 0);
    }

    async fn await_new_watch(observed: &WorkspaceObservation, previous: u64) {
        tokio::time::timeout(Duration::from_secs(10), async {
            while !observed.watch_healthy()
                || observed.topology_installations.load(Ordering::Acquire) <= previous
            {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }

    async fn await_watched_edit(
        observed: &WorkspaceObservation,
        receiver: &mut mpsc::Receiver<()>,
        file: &Path,
    ) {
        while receiver.try_recv().is_ok() {}
        let before = observed.event_revision();
        std::fs::write(file, "selected").unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while observed.event_revision() == before {
                receiver.recv().await.unwrap();
            }
        })
        .await
        .unwrap();
        assert!(observed.watch_healthy());
    }
}
