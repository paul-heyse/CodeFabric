//! Per-workspace actor, authoritative bootstrap, restart verification, and health state.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::os::unix::ffi::OsStringExt as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::params;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{Mutex, Semaphore, mpsc, oneshot};

use crate::contracts::models::DeploymentProfileDocument;
use crate::git_state::{
    GitCancellation, GitHashAlgorithm, GitOperationState, GitStateAdapter, GitStateError,
    GitStateObservations, GitStateSnapshot, GitStateVector, GitTrustPolicy, GixGitStateAdapter,
    HeadKind, RegisteredGitIdentity, apply_to_source_inventory, encode_object_id,
};
use crate::identity::{IdentityDomain, encode_public_id};
use crate::inventory::{
    InventoryCancellation, InventoryError, InventoryLimits, InventoryWalker, SourceInventory,
};
use crate::operational_store::{OperationalStore, OperationalStoreError};
use crate::registries::{
    EVENT_STREAM_HEALTH_TRANSITIONS, EVENT_STREAM_HEALTH_VALUES, EventStreamHealth,
    GIT_ACCELERATION_STATUS_TRANSITIONS, GIT_ACCELERATION_STATUS_VALUES, GitAccelerationStatus,
    SOURCE_TRUST_STATE_TRANSITIONS, SOURCE_TRUST_STATE_VALUES, SourceTrustState,
    WORKSPACE_REGISTRY_LIFECYCLE_VALUES, WorkspaceRegistryLifecycle, generated_transition,
    registry_state_name,
};
use crate::secure_path::{SecurePathError, SecureRoot, open_workspace_root};
use crate::workspace_registry::{WorkspaceRecord, WorkspaceRegistry, WorkspaceRegistryError};

const DEPLOYMENT_PROFILE: &[u8] =
    include_bytes!("../contracts/deployment/local-workstation-v1.yaml");

/// Deployment-owned actor and blocking-work limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoordinatorLimits {
    pub command_capacity: usize,
    pub maximum_concurrent_source_reads: usize,
    pub maximum_concurrent_gix_jobs: usize,
    pub inventory: InventoryLimits,
    pub stable_read_retry_count: u32,
}

impl CoordinatorLimits {
    /// Load the generated, closed local-workstation profile.
    ///
    /// # Errors
    ///
    /// Returns a typed profile error when a required bound is zero or cannot fit the host.
    pub fn local_workstation() -> Result<Self, CoordinatorError> {
        let profile: DeploymentProfileDocument =
            serde_yaml_ng::from_slice(DEPLOYMENT_PROFILE).map_err(|_| CoordinatorError::Profile)?;
        let command_capacity = usize::from(profile.coordinator_command_capacity);
        let maximum_concurrent_source_reads = usize::from(profile.maximum_concurrent_source_reads);
        let maximum_concurrent_gix_jobs = usize::from(profile.maximum_concurrent_gix_jobs);
        if command_capacity == 0
            || maximum_concurrent_source_reads == 0
            || maximum_concurrent_gix_jobs == 0
            || profile.source_image_limits.stable_read_retry_count == 0
        {
            return Err(CoordinatorError::Profile);
        }
        Ok(Self {
            command_capacity,
            maximum_concurrent_source_reads,
            maximum_concurrent_gix_jobs,
            inventory: InventoryLimits {
                maximum_file_count: profile.inventory_limits.maximum_file_count,
                maximum_directory_count: profile.inventory_limits.maximum_directory_count,
                maximum_directory_depth: profile.inventory_limits.maximum_directory_depth,
                maximum_total_bytes_considered: profile
                    .inventory_limits
                    .maximum_total_bytes_considered,
                maximum_duration: Duration::from_millis(
                    profile.inventory_limits.maximum_duration_ms,
                ),
                maximum_entries_per_directory: usize::try_from(
                    profile.inventory_limits.maximum_entries_per_directory,
                )
                .map_err(|_| CoordinatorError::Profile)?,
            },
            stable_read_retry_count: u32::from(profile.source_image_limits.stable_read_retry_count),
        })
    }
}

/// Wave-2 workspace state. `active_snapshot` remains `None` until WP24.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceCoordinatorState {
    pub workspace_id: [u8; 16],
    pub lifecycle: WorkspaceRegistryLifecycle,
    pub source_trust: SourceTrustState,
    pub event_stream_health: EventStreamHealth,
    pub git_acceleration: GitAccelerationStatus,
    pub source_generation: u64,
    pub inventory_digest: Option<[u8; 32]>,
    pub git_state: Option<GitStateVector>,
    pub active_snapshot: Option<[u8; 16]>,
    pub reconciliation_count: u64,
    pub mutations_applied: u64,
}

impl WorkspaceCoordinatorState {
    /// The sole readiness predicate. Wave 2 cannot satisfy it because no frozen snapshot exists.
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        matches!(self.lifecycle, WorkspaceRegistryLifecycle::Ready)
            && matches!(self.source_trust, SourceTrustState::Current)
            && self.active_snapshot.is_some()
    }

    /// Stable query rejection before WP24 activation.
    ///
    /// # Errors
    ///
    /// Returns `WORKSPACE_BOOTSTRAPPING` unless the full readiness invariant holds.
    pub fn require_ready(&self) -> Result<(), CoordinatorError> {
        if self.is_ready() {
            Ok(())
        } else {
            Err(CoordinatorError::WorkspaceBootstrapping)
        }
    }

    #[must_use]
    pub fn health(&self) -> WorkspaceHealthStatus {
        WorkspaceHealthStatus {
            workspace_id: public_id(IdentityDomain::Workspace, self.workspace_id),
            lifecycle: state_name(WORKSPACE_REGISTRY_LIFECYCLE_VALUES, self.lifecycle as u16),
            source_trust: state_name(SOURCE_TRUST_STATE_VALUES, self.source_trust as u16),
            event_stream_health: state_name(
                EVENT_STREAM_HEALTH_VALUES,
                self.event_stream_health as u16,
            ),
            git_acceleration: state_name(
                GIT_ACCELERATION_STATUS_VALUES,
                self.git_acceleration as u16,
            ),
            source_generation: self.source_generation,
            inventory_digest: self.inventory_digest.map(hex_digest),
            active_snapshot: self
                .active_snapshot
                .map(|id| public_id(IdentityDomain::ServingSnapshot, id)),
            readiness: if self.is_ready() {
                "READY"
            } else {
                "WORKSPACE_BOOTSTRAPPING"
            }
            .to_owned(),
            reconciliation_count: self.reconciliation_count,
        }
    }
}

/// Non-secret administrative health projection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceHealthStatus {
    pub workspace_id: String,
    pub lifecycle: String,
    pub source_trust: String,
    pub event_stream_health: String,
    pub git_acceleration: String,
    pub source_generation: u64,
    pub inventory_digest: Option<String>,
    pub active_snapshot: Option<String>,
    pub readiness: String,
    pub reconciliation_count: u64,
}

/// Stable coordinator errors.
#[derive(Debug, Error)]
pub enum CoordinatorError {
    #[error(transparent)]
    Store(#[from] OperationalStoreError),
    #[error(transparent)]
    Registry(#[from] WorkspaceRegistryError),
    #[error(transparent)]
    SecurePath(#[from] SecurePathError),
    #[error(transparent)]
    Inventory(#[from] InventoryError),
    #[error(transparent)]
    Git(#[from] GitStateError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("COORDINATOR_ALREADY_RUNNING")]
    DuplicateCoordinator,
    #[error("COORDINATOR_CHANNEL_CLOSED")]
    ChannelClosed,
    #[error("COORDINATOR_TASK_FAILED")]
    TaskFailed,
    #[error("WORKSPACE_BOOTSTRAPPING")]
    WorkspaceBootstrapping,
    #[error("SOURCE_CHANGED_DURING_BOOTSTRAP")]
    SourceChanged,
    #[error("invalid coordinator deployment profile")]
    Profile,
    #[error("invalid persisted coordinator state")]
    Persisted,
}

/// Deterministic test seam invoked once between the G0 and G1 bootstrap captures.
pub type BootstrapHook = Arc<dyn Fn() + Send + Sync + 'static>;

enum CoordinatorCommand {
    Bootstrap {
        hook: Option<BootstrapHook>,
        response: oneshot::Sender<Result<WorkspaceCoordinatorState, CoordinatorError>>,
    },
    Status {
        response: oneshot::Sender<WorkspaceCoordinatorState>,
    },
    Shutdown {
        response: oneshot::Sender<()>,
    },
}

/// Cloneable bounded ingress to one workspace's sole mutation task.
#[derive(Clone)]
pub struct WorkspaceCoordinatorHandle {
    workspace_id: [u8; 16],
    sender: mpsc::Sender<CoordinatorCommand>,
}

impl WorkspaceCoordinatorHandle {
    #[must_use]
    pub const fn workspace_id(&self) -> [u8; 16] {
        self.workspace_id
    }

    /// Run authoritative bootstrap and return the persisted pre-ready state.
    ///
    /// # Errors
    ///
    /// Returns a channel, task, source, Git, or persistence error.
    pub async fn bootstrap(&self) -> Result<WorkspaceCoordinatorState, CoordinatorError> {
        self.bootstrap_with_optional_hook(None).await
    }

    /// Test seam for a deterministic mutation between G0 and G1.
    #[doc(hidden)]
    pub async fn bootstrap_with_hook(
        &self,
        hook: BootstrapHook,
    ) -> Result<WorkspaceCoordinatorState, CoordinatorError> {
        self.bootstrap_with_optional_hook(Some(hook)).await
    }

    async fn bootstrap_with_optional_hook(
        &self,
        hook: Option<BootstrapHook>,
    ) -> Result<WorkspaceCoordinatorState, CoordinatorError> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(CoordinatorCommand::Bootstrap { hook, response })
            .await
            .map_err(|_| CoordinatorError::ChannelClosed)?;
        receiver.await.map_err(|_| CoordinatorError::TaskFailed)?
    }

    /// Read the actor-owned state through its receiver.
    ///
    /// # Errors
    ///
    /// Returns a channel or task failure.
    pub async fn status(&self) -> Result<WorkspaceCoordinatorState, CoordinatorError> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(CoordinatorCommand::Status { response })
            .await
            .map_err(|_| CoordinatorError::ChannelClosed)?;
        receiver.await.map_err(|_| CoordinatorError::TaskFailed)
    }

    /// Join the sole actor task.
    ///
    /// # Errors
    ///
    /// Returns a channel or task failure.
    pub async fn shutdown(&self) -> Result<(), CoordinatorError> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(CoordinatorCommand::Shutdown { response })
            .await
            .map_err(|_| CoordinatorError::ChannelClosed)?;
        receiver.await.map_err(|_| CoordinatorError::TaskFailed)
    }
}

/// Process registry enforcing exactly one coordinator task per workspace.
pub struct WorkspaceCoordinatorManager {
    store: Arc<Mutex<OperationalStore>>,
    limits: CoordinatorLimits,
    source_read_permits: Arc<Semaphore>,
    git_job_permits: Arc<Semaphore>,
    handles: BTreeMap<[u8; 16], WorkspaceCoordinatorHandle>,
}

impl WorkspaceCoordinatorManager {
    /// Construct a manager over the daemon's sole operational writer.
    ///
    /// # Errors
    ///
    /// Returns an invalid generated deployment-profile error.
    pub fn new(store: Arc<Mutex<OperationalStore>>) -> Result<Self, CoordinatorError> {
        let limits = CoordinatorLimits::local_workstation()?;
        Ok(Self {
            store,
            limits,
            source_read_permits: Arc::new(Semaphore::new(limits.maximum_concurrent_source_reads)),
            git_job_permits: Arc::new(Semaphore::new(limits.maximum_concurrent_gix_jobs)),
            handles: BTreeMap::new(),
        })
    }

    /// Spawn the sole actor for a registered workspace and mark restart state unverified.
    ///
    /// # Errors
    ///
    /// Returns duplicate, registration, persistence, or state-decoding errors.
    pub async fn spawn(
        &mut self,
        workspace_id: [u8; 16],
    ) -> Result<WorkspaceCoordinatorHandle, CoordinatorError> {
        if self.handles.contains_key(&workspace_id) {
            return Err(CoordinatorError::DuplicateCoordinator);
        }
        let state = {
            let mut store = self.store.lock().await;
            load_restart_state(&mut store, workspace_id)?
        };
        let (sender, receiver) = mpsc::channel(self.limits.command_capacity);
        let handle = WorkspaceCoordinatorHandle {
            workspace_id,
            sender,
        };
        tokio::spawn(coordinator_task(
            state,
            receiver,
            Arc::clone(&self.store),
            Arc::clone(&self.source_read_permits),
            Arc::clone(&self.git_job_permits),
            self.limits,
        ));
        self.handles.insert(workspace_id, handle.clone());
        Ok(handle)
    }

    #[must_use]
    pub fn handle(&self, workspace_id: [u8; 16]) -> Option<WorkspaceCoordinatorHandle> {
        self.handles.get(&workspace_id).cloned()
    }

    #[must_use]
    pub fn active_count(&self) -> usize {
        self.handles.len()
    }

    /// Stop and remove the actor for one workspace, if present.
    ///
    /// # Errors
    ///
    /// Returns a channel or task failure while joining the actor.
    pub async fn stop(&mut self, workspace_id: [u8; 16]) -> Result<(), CoordinatorError> {
        if let Some(handle) = self.handles.remove(&workspace_id) {
            handle.shutdown().await?;
        }
        Ok(())
    }

    /// Restore and reverify every bootstrapping registration.
    ///
    /// # Errors
    ///
    /// Returns a store, spawn, or bootstrap failure.
    pub async fn restore_and_bootstrap(
        &mut self,
    ) -> Result<Vec<WorkspaceCoordinatorState>, CoordinatorError> {
        let workspace_ids = {
            let store = self.store.lock().await;
            bootstrapping_workspace_ids(&store)?
        };
        let mut states = Vec::with_capacity(workspace_ids.len());
        for workspace_id in workspace_ids {
            let handle = self.spawn(workspace_id).await?;
            states.push(handle.bootstrap().await?);
        }
        Ok(states)
    }

    /// Stop and forget every actor.
    ///
    /// # Errors
    ///
    /// Returns the first actor join failure.
    pub async fn shutdown_all(&mut self) -> Result<(), CoordinatorError> {
        let handles = std::mem::take(&mut self.handles).into_values();
        for handle in handles {
            handle.shutdown().await?;
        }
        Ok(())
    }
}

async fn coordinator_task(
    mut state: WorkspaceCoordinatorState,
    mut receiver: mpsc::Receiver<CoordinatorCommand>,
    store: Arc<Mutex<OperationalStore>>,
    source_read_permits: Arc<Semaphore>,
    git_job_permits: Arc<Semaphore>,
    limits: CoordinatorLimits,
) {
    while let Some(command) = receiver.recv().await {
        match command {
            CoordinatorCommand::Bootstrap { hook, response } => {
                let store = Arc::clone(&store);
                let workspace_id = state.workspace_id;
                let permits = async {
                    let source = Arc::clone(&source_read_permits)
                        .acquire_owned()
                        .await
                        .map_err(|_| CoordinatorError::TaskFailed)?;
                    // Conservatively reserve a Git slot for the complete stable-read fence.
                    // This also bounds registrations whose source kind changes during relink.
                    let git = Arc::clone(&git_job_permits)
                        .acquire_owned()
                        .await
                        .map_err(|_| CoordinatorError::TaskFailed)?;
                    Ok::<_, CoordinatorError>((source, git))
                }
                .await;
                let result = match permits {
                    Ok((_source_permit, _git_permit)) => tokio::task::spawn_blocking(move || {
                        let mut store = store.blocking_lock();
                        bootstrap_sync(&mut store, workspace_id, limits, hook)
                    })
                    .await
                    .map_err(|_| CoordinatorError::TaskFailed)
                    .and_then(std::convert::identity),
                    Err(error) => Err(error),
                };
                if let Ok(next) = &result {
                    state = next.clone();
                    state.mutations_applied = state.mutations_applied.saturating_add(1);
                }
                let _ = response.send(result.map(|_| state.clone()));
            }
            CoordinatorCommand::Status { response } => {
                let _ = response.send(state.clone());
            }
            CoordinatorCommand::Shutdown { response } => {
                let _ = response.send(());
                break;
            }
        }
    }
}

fn bootstrap_sync(
    store: &mut OperationalStore,
    workspace_id: [u8; 16],
    limits: CoordinatorLimits,
    hook: Option<BootstrapHook>,
) -> Result<WorkspaceCoordinatorState, CoordinatorError> {
    let record = WorkspaceRegistry::new(store).show(workspace_id)?;
    if record.status != WorkspaceRegistryLifecycle::Bootstrapping {
        return Err(CoordinatorError::WorkspaceBootstrapping);
    }
    let source_generation = begin_verification(store, &record)?;
    let root = open_workspace_root(store, workspace_id)?;
    let git = open_registered_git(&record)?;

    let mut prior = capture_pass(
        store,
        &record,
        &root,
        git.as_ref(),
        source_generation,
        limits,
    )?;
    if let Some(hook) = hook {
        hook();
    }
    let mut reconciliation_count = 0_u64;
    let mut current = capture_pass(
        store,
        &record,
        &root,
        git.as_ref(),
        source_generation,
        limits,
    )?;
    while prior != current {
        reconciliation_count = reconciliation_count.saturating_add(1);
        if reconciliation_count >= u64::from(limits.stable_read_retry_count) {
            mark_reconcile_required(store, workspace_id)?;
            return Err(CoordinatorError::SourceChanged);
        }
        prior = current;
        current = capture_pass(
            store,
            &record,
            &root,
            git.as_ref(),
            source_generation,
            limits,
        )?;
    }
    persist_bootstrap_current(store, &record, source_generation, &current, git.as_ref())?;
    Ok(WorkspaceCoordinatorState {
        workspace_id,
        lifecycle: WorkspaceRegistryLifecycle::Bootstrapping,
        source_trust: SourceTrustState::Current,
        event_stream_health: EventStreamHealth::Unavailable,
        git_acceleration: current.acceleration,
        source_generation,
        inventory_digest: Some(current.inventory.digest),
        git_state: current.git_state,
        active_snapshot: None,
        reconciliation_count,
        mutations_applied: 0,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BootstrapPass {
    inventory: SourceInventory,
    git_state: Option<GitStateVector>,
    acceleration: GitAccelerationStatus,
}

fn capture_pass(
    store: &mut OperationalStore,
    record: &WorkspaceRecord,
    root: &SecureRoot,
    git: Option<&GitStateSnapshot>,
    source_generation: u64,
    limits: CoordinatorLimits,
) -> Result<BootstrapPass, CoordinatorError> {
    let mut inventory = InventoryWalker::new(limits.inventory).walk_and_persist(
        root,
        store,
        source_generation,
        &InventoryCancellation::default(),
    )?;
    let (git_state, acceleration) = if let Some(git) = git {
        let (inclusion, attributes) = policy_fingerprints(record, &inventory);
        let observations = GitStateObservations {
            inclusion_policy_fingerprint: inclusion,
            attributes_fingerprint: attributes,
            worktree_inventory_digest: inventory.digest,
        };
        let adapter = GixGitStateAdapter;
        let git_inventory = adapter.inventory(
            &git.selected_worktree,
            observations,
            &GitCancellation::default(),
        )?;
        apply_to_source_inventory(&git_inventory, &mut inventory, store)?;
        let observations = GitStateObservations {
            worktree_inventory_digest: inventory.digest,
            ..observations
        };
        (
            Some(adapter.capture_state(&git.selected_worktree, observations)?),
            GitAccelerationStatus::GitReady,
        )
    } else {
        (None, GitAccelerationStatus::NotAGitWorktree)
    };
    Ok(BootstrapPass {
        inventory,
        git_state,
        acceleration,
    })
}

fn open_registered_git(
    record: &WorkspaceRecord,
) -> Result<Option<GitStateSnapshot>, CoordinatorError> {
    let Some((repository_id, worktree_id)) = record.repository_id.zip(record.worktree_id) else {
        return Ok(None);
    };
    let root = PathBuf::from(OsString::from_vec(record.root_path_bytes.clone()));
    GixGitStateAdapter
        .open_worktree(
            &root,
            RegisteredGitIdentity {
                repository_id,
                worktree_id,
            },
            &GitTrustPolicy::local_read_only(),
        )
        .map(Some)
        .map_err(Into::into)
}

fn policy_fingerprints(
    record: &WorkspaceRecord,
    inventory: &SourceInventory,
) -> ([u8; 32], [u8; 32]) {
    let mut inclusion = crate::identity::semantic_fingerprint(
        crate::identity::SemanticFingerprintDomain::InclusionPolicy,
    );
    inclusion.update(&record.authorization_fingerprint);
    let mut attributes = crate::identity::semantic_fingerprint(
        crate::identity::SemanticFingerprintDomain::AttributesPolicy,
    );
    for source in &inventory.records {
        let path = source.path.raw_relative_path_bytes.as_slice();
        if path == b".gitignore" || path.ends_with(b"/.gitignore") {
            hash_policy_record(&mut inclusion, path, source.content_digest);
        }
        if path == b".gitattributes" || path.ends_with(b"/.gitattributes") {
            hash_policy_record(&mut attributes, path, source.content_digest);
        }
    }
    (inclusion.finalize(), attributes.finalize())
}

fn hash_policy_record(
    hasher: &mut crate::identity::SemanticFingerprintBuilder,
    path: &[u8],
    digest: Option<[u8; 32]>,
) {
    hasher.update(&u64::try_from(path.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(path);
    hasher.update(&digest.unwrap_or([0; 32]));
}

fn begin_verification(
    store: &mut OperationalStore,
    record: &WorkspaceRecord,
) -> Result<u64, CoordinatorError> {
    assert_transition(
        SOURCE_TRUST_STATE_TRANSITIONS,
        "UNVERIFIED",
        "verification-started",
        "root-authorized",
    )?;
    assert_transition(
        EVENT_STREAM_HEALTH_TRANSITIONS,
        "HEALTHY",
        "backend-lost",
        "backend-terminal",
    )?;
    let now = timestamp()?;
    store.write_transaction(|transaction| {
        let current = transaction.query_row(
            "SELECT source_generation FROM workspace_generation WHERE workspace_id=?1",
            [record.workspace_id.as_slice()],
            |row| row.get::<_, i64>(0),
        )?;
        let next = current.checked_add(1).ok_or(CoordinatorError::Persisted)?;
        transaction.execute(
            "UPDATE workspace_generation SET source_generation=?2, updated_at=?3 WHERE workspace_id=?1",
            params![record.workspace_id.as_slice(), next, &now],
        )?;
        transaction.execute(
            "UPDATE worktree_state SET lifecycle_state_code=?2, source_trust_state_code=?3,
             event_stream_health_code=?4, active_snapshot_id=NULL, source_generation=?5,
             reconcile_required=1, updated_at=?6 WHERE workspace_id=?1",
            params![
                record.workspace_id.as_slice(),
                10_u16,
                SourceTrustState::Verifying as u16,
                EventStreamHealth::Unavailable as u16,
                next,
                &now,
            ],
        )?;
        u64::try_from(next).map_err(|_| CoordinatorError::Persisted)
    })
}

#[allow(clippy::too_many_lines)] // One SQLite transaction keeps bootstrap state and Git topology atomic.
fn persist_bootstrap_current(
    store: &mut OperationalStore,
    record: &WorkspaceRecord,
    source_generation: u64,
    pass: &BootstrapPass,
    topology: Option<&GitStateSnapshot>,
) -> Result<(), CoordinatorError> {
    assert_transition(
        SOURCE_TRUST_STATE_TRANSITIONS,
        "VERIFYING",
        "source-reconciled",
        "stable-reads-accepted",
    )?;
    if topology.is_some() {
        assert_transition(
            GIT_ACCELERATION_STATUS_TRANSITIONS,
            "GIT_SCANNING",
            "scan-complete",
            "repository-stable",
        )?;
    }
    let generation = i64::try_from(source_generation).map_err(|_| CoordinatorError::Persisted)?;
    let now = timestamp()?;
    store.write_transaction(|transaction| {
        transaction.execute(
            "UPDATE worktree_state SET source_trust_state_code=?2,
             event_stream_health_code=?3, git_acceleration_status_code=?4,
             active_snapshot_id=NULL, source_generation=?5, reconcile_required=0,
             inventory_digest=?6, updated_at=?7 WHERE workspace_id=?1",
            params![
                record.workspace_id.as_slice(),
                SourceTrustState::Current as u16,
                EventStreamHealth::Unavailable as u16,
                pass.acceleration as u16,
                generation,
                pass.inventory.digest.as_slice(),
                &now,
            ],
        )?;
        transaction.execute(
            "DELETE FROM git_state_vector WHERE workspace_id=?1 AND source_generation=?2",
            params![record.workspace_id.as_slice(), generation],
        )?;
        if let Some(vector) = &pass.git_state {
            transaction.execute(
                "INSERT INTO git_state_vector(
                   workspace_id, source_generation, repository_id, worktree_id,
                   head_kind_code, head_target, head_tree, index_fingerprint,
                   index_entry_count, has_conflict_stages, repository_state_code,
                   inclusion_policy_fingerprint, attributes_fingerprint,
                   worktree_inventory_digest, captured_at
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
                params![
                    record.workspace_id.as_slice(),
                    generation,
                    vector.repository_id.as_slice(),
                    vector.worktree_id.as_slice(),
                    head_kind_code(vector.head_kind),
                    vector.head_target.as_ref().map(encode_object_id),
                    vector.head_tree.as_ref().map(encode_object_id),
                    vector.index_fingerprint.as_ref().map(<[u8; 32]>::as_slice),
                    vector
                        .index_entry_count
                        .and_then(|value| i64::try_from(value).ok()),
                    i64::from(vector.has_conflict_stages),
                    repository_state_code(vector.repository_state),
                    vector.inclusion_policy_fingerprint.as_slice(),
                    vector.attributes_fingerprint.as_slice(),
                    vector.worktree_inventory_digest.as_slice(),
                    &now,
                ],
            )?;
        }
        if let Some(topology) = topology {
            transaction.execute(
                "INSERT INTO common_repository_state(
                   repository_id, common_dir_path_bytes, common_dir_path_display,
                   object_format_code, gix_version, trust_policy_fingerprint,
                   worktree_count, git_health_code, updated_at, last_diagnostic_id
                 ) VALUES (?1,?2,?3,?4,'0.86.0',?5,?6,?7,?8,NULL)
                 ON CONFLICT(repository_id) DO UPDATE SET
                   common_dir_path_bytes=excluded.common_dir_path_bytes,
                   common_dir_path_display=excluded.common_dir_path_display,
                   object_format_code=excluded.object_format_code,
                   trust_policy_fingerprint=excluded.trust_policy_fingerprint,
                   worktree_count=excluded.worktree_count,
                   git_health_code=excluded.git_health_code,
                   updated_at=excluded.updated_at",
                params![
                    topology.repository.repository_id.as_slice(),
                    &topology.repository.common_dir_key.raw_bytes,
                    &topology.repository.common_dir_key.display,
                    object_format_code(topology.repository.object_format),
                    trust_policy_fingerprint().as_slice(),
                    i64::try_from(topology.linked_worktrees.len().saturating_add(1))
                        .map_err(|_| CoordinatorError::Persisted)?,
                    GitAccelerationStatus::GitReady as u16,
                    &now,
                ],
            )?;
            transaction.execute(
                "UPDATE worktree_state SET git_dir_path_bytes=?2, git_dir_path_display=?3
                 WHERE workspace_id=?1",
                params![
                    record.workspace_id.as_slice(),
                    &topology.selected_worktree.git_dir.raw_bytes,
                    &topology.selected_worktree.git_dir.display,
                ],
            )?;
        }
        Ok(())
    })
}

fn mark_reconcile_required(
    store: &mut OperationalStore,
    workspace_id: [u8; 16],
) -> Result<(), CoordinatorError> {
    store.write_transaction(|transaction| {
        transaction.execute(
            "UPDATE worktree_state SET reconcile_required=1 WHERE workspace_id=?1",
            [workspace_id.as_slice()],
        )?;
        Ok(())
    })
}

fn load_restart_state(
    store: &mut OperationalStore,
    workspace_id: [u8; 16],
) -> Result<WorkspaceCoordinatorState, CoordinatorError> {
    let record = WorkspaceRegistry::new(store).show(workspace_id)?;
    let now = timestamp()?;
    store.write_transaction(|transaction| {
        transaction.execute(
            "UPDATE worktree_state SET source_trust_state_code=?2,
             event_stream_health_code=?3, active_snapshot_id=NULL,
             reconcile_required=1, updated_at=?4
             WHERE workspace_id=?1",
            params![
                workspace_id.as_slice(),
                SourceTrustState::Unverified as u16,
                EventStreamHealth::Unavailable as u16,
                &now
            ],
        )?;
        Ok::<(), CoordinatorError>(())
    })?;
    let (source_generation, git_acceleration, inventory_digest) = store
        .reader_factory()
        .open()?
        .with_connection(|connection| {
            connection.query_row(
                "SELECT source_generation, git_acceleration_status_code, inventory_digest
                 FROM worktree_state WHERE workspace_id=?1",
                [workspace_id.as_slice()],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, u16>(1)?,
                        row.get::<_, Option<Vec<u8>>>(2)?,
                    ))
                },
            )
        })?;
    Ok(WorkspaceCoordinatorState {
        workspace_id,
        lifecycle: record.status,
        source_trust: SourceTrustState::Unverified,
        event_stream_health: EventStreamHealth::Unavailable,
        git_acceleration: GitAccelerationStatus::try_from(git_acceleration)
            .map_err(|_| CoordinatorError::Persisted)?,
        source_generation: u64::try_from(source_generation)
            .map_err(|_| CoordinatorError::Persisted)?,
        inventory_digest: inventory_digest.and_then(|bytes| bytes.try_into().ok()),
        git_state: None,
        active_snapshot: None,
        reconciliation_count: 0,
        mutations_applied: 0,
    })
}

fn bootstrapping_workspace_ids(
    store: &OperationalStore,
) -> Result<Vec<[u8; 16]>, CoordinatorError> {
    store
        .reader_factory()
        .open()?
        .with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT workspace_id FROM workspace_registration
                 WHERE status_code=?1 ORDER BY workspace_id",
            )?;
            statement
                .query_map([WorkspaceRegistryLifecycle::Bootstrapping as u16], |row| {
                    fixed_id(row.get(0)?)
                })?
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(Into::into)
}

/// Read current health without creating an actor or trusting an active snapshot claim.
///
/// # Errors
///
/// Returns a store or persisted-enum failure.
pub fn persisted_workspace_health(
    store: &OperationalStore,
) -> Result<Vec<WorkspaceHealthStatus>, CoordinatorError> {
    store
        .reader_factory()
        .open()?
        .with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT registration.workspace_id, registration.status_code,
                  state.source_trust_state_code, state.event_stream_health_code,
                  state.git_acceleration_status_code, state.source_generation,
                  state.active_snapshot_id, state.inventory_digest
                 FROM workspace_registration AS registration
                 JOIN worktree_state AS state USING (workspace_id)
                 WHERE registration.status_code != 90 ORDER BY registration.workspace_id",
            )?;
            statement
                .query_map([], |row| {
                    let workspace_id = fixed_id(row.get(0)?)?;
                    let lifecycle = WorkspaceRegistryLifecycle::try_from(row.get::<_, u16>(1)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                    let source_trust = SourceTrustState::try_from(row.get::<_, u16>(2)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                    let event_stream = EventStreamHealth::try_from(row.get::<_, u16>(3)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                    let git = GitAccelerationStatus::try_from(row.get::<_, u16>(4)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                    let generation = u64::try_from(row.get::<_, i64>(5)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                    let active: Option<Vec<u8>> = row.get(6)?;
                    let inventory: Option<Vec<u8>> = row.get(7)?;
                    Ok(WorkspaceCoordinatorState {
                        workspace_id,
                        lifecycle,
                        source_trust,
                        event_stream_health: event_stream,
                        git_acceleration: git,
                        source_generation: generation,
                        inventory_digest: inventory.and_then(|bytes| bytes.try_into().ok()),
                        git_state: None,
                        active_snapshot: active.and_then(|bytes| bytes.try_into().ok()),
                        reconciliation_count: 0,
                        mutations_applied: 0,
                    }
                    .health())
                })?
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(Into::into)
}

fn assert_transition(
    transitions: &'static [crate::registries::StateTransitionEntry],
    from: &str,
    event: &str,
    guard: &str,
) -> Result<(), CoordinatorError> {
    generated_transition(transitions, from, event, guard)
        .map(|_| ())
        .map_err(|_| CoordinatorError::Persisted)
}

fn fixed_id(bytes: Vec<u8>) -> rusqlite::Result<[u8; 16]> {
    bytes.try_into().map_err(|_| rusqlite::Error::InvalidQuery)
}

fn timestamp() -> Result<String, CoordinatorError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CoordinatorError::Persisted)?
        .as_millis();
    Ok(format!("unix-ms:{millis}"))
}

fn state_name(values: &[crate::registries::RegistryEntry], code: u16) -> String {
    registry_state_name(values, code)
        .unwrap_or("UNKNOWN")
        .to_owned()
}

fn public_id(domain: IdentityDomain, id: [u8; 16]) -> String {
    encode_public_id(domain, None, id).unwrap_or_else(|_| "<invalid-id>".to_owned())
}

fn hex_digest(digest: [u8; 32]) -> String {
    let mut encoded = String::with_capacity(67);
    encoded.push_str("b3:");
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

const fn head_kind_code(kind: HeadKind) -> u16 {
    kind as u16
}

const fn repository_state_code(state: GitOperationState) -> u16 {
    state as u16
}

const fn object_format_code(format: GitHashAlgorithm) -> u16 {
    format as u16
}

fn trust_policy_fingerprint() -> [u8; 32] {
    crate::identity::semantic_fingerprint(
        crate::identity::SemanticFingerprintDomain::GitTrustPolicy,
    )
    .finalize()
}
