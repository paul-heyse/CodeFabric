//! Read-only Git discovery, topology, state, and inventory boundary.
//!
//! No `gix` value crosses this module. Each operation opens a fresh repository
//! handle with isolated configuration permissions and returns detached DTOs.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "daemon")]
use std::sync::atomic::{AtomicU64, AtomicUsize};
#[cfg(feature = "daemon")]
use std::time::Instant;

use thiserror::Error;

use crate::registries::GitAccelerationStatus;

/// Canonical identities supplied by the workspace-registration boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredGitIdentity {
    pub repository_id: [u8; 16],
    pub worktree_id: [u8; 16],
}

/// Application-owned object-hash algorithm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitHashAlgorithm {
    Sha1,
    Sha256,
}

impl GitHashAlgorithm {
    #[must_use]
    pub const fn digest_bytes(self) -> usize {
        match self {
            Self::Sha1 => 20,
            Self::Sha256 => 32,
        }
    }
}

/// Algorithm-tagged object identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitObjectId {
    pub algorithm: GitHashAlgorithm,
    pub bytes: Vec<u8>,
}

/// Byte-native host path with a display-only view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitNativePath {
    pub raw_bytes: Vec<u8>,
    pub display: String,
    pub display_is_lossy: bool,
}

impl GitNativePath {
    fn to_path_buf(&self) -> PathBuf {
        PathBuf::from(OsString::from_vec(self.raw_bytes.clone()))
    }
}

/// Repository location class reported by gix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitRepositoryKind {
    Common,
    LinkedWorktree,
    Submodule,
}

/// Detached repository identity and topology facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitRepositoryIdentity {
    pub repository_id: [u8; 16],
    pub common_dir_key: GitNativePath,
    pub object_format: GitHashAlgorithm,
    pub kind: GitRepositoryKind,
}

/// Detached worktree identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitWorktreeIdentity {
    pub worktree_id: [u8; 16],
    pub repository_id: [u8; 16],
    pub work_dir: Option<GitNativePath>,
    pub git_dir: GitNativePath,
    pub common_dir: GitNativePath,
    pub administrative_name: Vec<u8>,
    pub is_main_worktree: bool,
    pub is_bare: bool,
}

/// Discovered linked-worktree topology before a separate workspace registration binds it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitWorktreeTopology {
    pub work_dir: Option<GitNativePath>,
    pub git_dir: GitNativePath,
    pub common_dir: GitNativePath,
    pub administrative_name: Vec<u8>,
    pub is_main_worktree: bool,
    pub is_bare: bool,
}

/// All topology discovered from one exact-path open.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitStateSnapshot {
    pub repository: GitRepositoryIdentity,
    pub selected_worktree: GitWorktreeIdentity,
    pub linked_worktrees: Vec<GitWorktreeTopology>,
    pub acceleration: GitAccelerationStatus,
}

/// Normalized HEAD state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeadKind {
    Symbolic,
    Detached,
    Unborn,
}

/// Normalized in-progress operation state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitOperationState {
    Clean,
    Merge,
    Rebase,
    CherryPick,
    Revert,
    Bisect,
    Apply,
    OtherOperation,
    Unknown,
}

/// Non-Git observations required to close the state-vector contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GitStateObservations {
    pub inclusion_policy_fingerprint: [u8; 32],
    pub attributes_fingerprint: [u8; 32],
    pub worktree_inventory_digest: [u8; 32],
}

/// Detached Lifecycle section 50 Git state vector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitStateVector {
    pub repository_id: [u8; 16],
    pub worktree_id: [u8; 16],
    pub head_kind: HeadKind,
    pub head_target: Option<GitObjectId>,
    pub head_tree: Option<GitObjectId>,
    pub index_fingerprint: Option<[u8; 32]>,
    pub index_entry_count: Option<u64>,
    pub has_conflict_stages: bool,
    pub repository_state: GitOperationState,
    pub inclusion_policy_fingerprint: [u8; 32],
    pub attributes_fingerprint: [u8; 32],
    pub worktree_inventory_digest: [u8; 32],
}

/// One Git-native classification record. Paths remain advisory until WP15 revalidates them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitInventoryEntry {
    pub repo_path_bytes: Vec<u8>,
    pub classification: GitInventoryClassification,
    pub conflict_stages: Vec<u8>,
    pub index_mode: Option<u32>,
    pub blob_oid: Option<GitObjectId>,
    pub present_on_disk: bool,
}

/// Exact Lifecycle section 46 classification detached from gix and persistence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum GitInventoryClassification {
    Tracked = 10,
    UntrackedNotIgnored = 20,
    UntrackedIgnored = 30,
    TrackedButIgnoredPatternMatches = 40,
    ExcludedByCodeFabricPolicy = 50,
    SubmoduleGitlink = 60,
    NestedRepository = 70,
    SpecialFile = 80,
}

/// Detached Git inventory and its state fence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitInventoryResult {
    pub entries: Vec<GitInventoryEntry>,
    pub vector: GitStateVector,
}

/// Closed strict-trust policy. There is intentionally no permissive constructor.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the frozen trust matrix is an auditable set of independent capabilities"
)]
pub struct GitTrustPolicy {
    pub codefabric_configuration: bool,
    pub repository_local_configuration: bool,
    pub environment_configuration: bool,
    pub global_configuration: bool,
    pub system_configuration: bool,
    pub execute_hooks: bool,
    pub execute_filters: bool,
    pub load_credentials: bool,
    pub allow_network: bool,
    pub allow_repository_mutation: bool,
    pub allow_checkout: bool,
    pub allow_external_commands: bool,
}

impl GitTrustPolicy {
    #[must_use]
    pub const fn local_read_only() -> Self {
        Self {
            codefabric_configuration: true,
            repository_local_configuration: true,
            environment_configuration: false,
            global_configuration: false,
            system_configuration: false,
            execute_hooks: false,
            execute_filters: false,
            load_credentials: false,
            allow_network: false,
            allow_repository_mutation: false,
            allow_checkout: false,
            allow_external_commands: false,
        }
    }

    fn validate(&self) -> Result<(), GitStateError> {
        if self == &Self::local_read_only() {
            Ok(())
        } else {
            Err(GitStateError::TrustPolicy)
        }
    }
}

/// Cooperative cancellation shared with gix's native interrupt surface.
#[derive(Clone, Debug, Default)]
pub struct GitCancellation(Arc<AtomicBool>);

impl GitCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Stable adapter failures without host paths, config values, or source bytes.
#[derive(Debug, Error)]
pub enum GitStateError {
    #[error("NOT_A_GIT_WORKTREE")]
    NotAWorktree,
    #[error("UNSUPPORTED_GIT_OBJECT_FORMAT")]
    UnsupportedObjectFormat,
    #[error("GIT_TRUST_POLICY_REJECTED")]
    TrustPolicy,
    #[error("GIT_OPERATION_CANCELLED")]
    Cancelled,
    #[error("GIT_CANDIDATE_DELTAS_DEFERRED_TO_WAVE_7")]
    CandidateDeltasDeferred,
    #[error("GIT_STATE_UNAVAILABLE: {0}")]
    Unavailable(&'static str),
}

/// Wave-2 read-only subset of the lifecycle Git port.
pub trait GitStateAdapter: Send + Sync {
    /// Open exactly the registered worktree root under the closed trust policy.
    ///
    /// # Errors
    ///
    /// Returns a typed trust, repository, topology, or object-format failure.
    fn open_worktree(
        &self,
        root: &Path,
        registered: RegisteredGitIdentity,
        policy: &GitTrustPolicy,
    ) -> Result<GitStateSnapshot, GitStateError>;

    /// Capture one complete operational state vector.
    ///
    /// # Errors
    ///
    /// Returns a typed repository, HEAD, index, or object-format failure.
    fn capture_state(
        &self,
        identity: &GitWorktreeIdentity,
        observations: GitStateObservations,
    ) -> Result<GitStateVector, GitStateError>;

    /// Classify the current worktree with gix-native index and ignore semantics.
    ///
    /// # Errors
    ///
    /// Returns a typed repository, index, dirwalk, cancellation, or format failure.
    fn inventory(
        &self,
        identity: &GitWorktreeIdentity,
        observations: GitStateObservations,
        cancel: &GitCancellation,
    ) -> Result<GitInventoryResult, GitStateError>;

    /// Wave-7 status-candidate seam.
    ///
    /// # Errors
    ///
    /// Always returns the typed Wave-7 deferral in the Wave-2 implementation.
    fn status_candidates(&self) -> Result<(), GitStateError> {
        Err(GitStateError::CandidateDeltasDeferred)
    }

    /// Wave-7 tree-diff-candidate seam.
    ///
    /// # Errors
    ///
    /// Always returns the typed Wave-7 deferral in the Wave-2 implementation.
    fn tree_diff_candidates(&self) -> Result<(), GitStateError> {
        Err(GitStateError::CandidateDeltasDeferred)
    }
}

/// Stateless gix implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct GixGitStateAdapter;

impl GitStateAdapter for GixGitStateAdapter {
    fn open_worktree(
        &self,
        root: &Path,
        registered: RegisteredGitIdentity,
        policy: &GitTrustPolicy,
    ) -> Result<GitStateSnapshot, GitStateError> {
        policy.validate()?;
        let repository = open_isolated(root)?;
        let common_dir = native_path(repository.common_dir());
        let selected_is_main = repository.git_dir() == repository.common_dir();
        let selected_worktree = GitWorktreeIdentity {
            worktree_id: registered.worktree_id,
            repository_id: registered.repository_id,
            work_dir: repository.workdir().map(native_path),
            git_dir: native_path(repository.git_dir()),
            common_dir: common_dir.clone(),
            administrative_name: if selected_is_main {
                b"main".to_vec()
            } else {
                repository
                    .git_dir()
                    .file_name()
                    .map_or_else(Vec::new, |name| name.as_bytes().to_vec())
            },
            is_main_worktree: selected_is_main,
            is_bare: repository.is_bare(),
        };

        let mut linked_worktrees = repository
            .worktrees()
            .map_err(|_| GitStateError::Unavailable("worktree-list"))?
            .into_iter()
            .map(|proxy| GitWorktreeTopology {
                work_dir: proxy.base().ok().map(|path| native_path(&path)),
                git_dir: native_path(proxy.git_dir()),
                common_dir: common_dir.clone(),
                administrative_name: proxy.id()[..].to_vec(),
                is_main_worktree: false,
                is_bare: false,
            })
            .collect::<Vec<_>>();
        linked_worktrees
            .sort_by(|left, right| left.administrative_name.cmp(&right.administrative_name));
        Ok(GitStateSnapshot {
            repository: GitRepositoryIdentity {
                repository_id: registered.repository_id,
                common_dir_key: common_dir,
                object_format: object_format(repository.object_hash())?,
                kind: repository_kind(repository.kind()),
            },
            selected_worktree,
            linked_worktrees,
            acceleration: GitAccelerationStatus::GitReady,
        })
    }

    fn capture_state(
        &self,
        identity: &GitWorktreeIdentity,
        observations: GitStateObservations,
    ) -> Result<GitStateVector, GitStateError> {
        capture_state_from_repository(&open_identity(identity)?, identity, observations)
    }

    fn inventory(
        &self,
        identity: &GitWorktreeIdentity,
        observations: GitStateObservations,
        cancel: &GitCancellation,
    ) -> Result<GitInventoryResult, GitStateError> {
        inventory(identity, observations, cancel)
    }
}

fn inventory(
    identity: &GitWorktreeIdentity,
    observations: GitStateObservations,
    cancel: &GitCancellation,
) -> Result<GitInventoryResult, GitStateError> {
    if cancel.is_cancelled() {
        return Err(GitStateError::Cancelled);
    }
    let repository = open_identity(identity)?;
    let index = repository
        .index_or_empty()
        .map_err(|_| GitStateError::Unavailable("index"))?;
    let mut paths = index_inventory_entries(&index)?;
    overlay_dirwalk_entries(&repository, &index, cancel, &mut paths)?;
    let mut entries = paths.into_values().collect::<Vec<_>>();
    for entry in &mut entries {
        entry.conflict_stages.sort_unstable();
        entry.conflict_stages.dedup();
    }
    let vector = capture_state_from_repository(&repository, identity, observations)?;
    Ok(GitInventoryResult { entries, vector })
}

fn index_inventory_entries(
    index: &gix::worktree::Index,
) -> Result<BTreeMap<Vec<u8>, GitInventoryEntry>, GitStateError> {
    let mut paths = BTreeMap::<Vec<u8>, GitInventoryEntry>::new();
    for entry in index.entries() {
        let path = entry.path(index)[..].to_vec();
        let classification = if entry.mode.is_submodule() {
            GitInventoryClassification::SubmoduleGitlink
        } else {
            GitInventoryClassification::Tracked
        };
        let index_mode = entry.mode.bits();
        let blob_oid = detach_object_id(entry.id)?;
        let record = paths
            .entry(path.clone())
            .or_insert_with(|| GitInventoryEntry {
                repo_path_bytes: path,
                classification,
                conflict_stages: Vec::new(),
                index_mode: Some(index_mode),
                blob_oid: Some(blob_oid),
                present_on_disk: false,
            });
        let stage = u8::try_from(entry.stage_raw())
            .map_err(|_| GitStateError::Unavailable("index-stage"))?;
        if stage != 0 {
            record.conflict_stages.push(stage);
        }
    }
    Ok(paths)
}

fn overlay_dirwalk_entries(
    repository: &gix::Repository,
    index: &gix::worktree::Index,
    cancel: &GitCancellation,
    paths: &mut BTreeMap<Vec<u8>, GitInventoryEntry>,
) -> Result<(), GitStateError> {
    let options = repository
        .dirwalk_options()
        .map_err(|_| GitStateError::Unavailable("dirwalk-options"))?
        .emit_tracked(true)
        .emit_ignored(Some(gix::dir::walk::EmissionMode::Matching))
        .emit_untracked(gix::dir::walk::EmissionMode::Matching)
        .emit_empty_directories(false)
        .recurse_repositories(false);
    let mut collect = gix::dir::walk::delegate::Collect::default();
    repository
        .dirwalk(
            index,
            [] as [gix::bstr::BString; 0],
            cancel.0.as_ref(),
            options,
            &mut collect,
        )
        .map_err(|_| {
            if cancel.is_cancelled() {
                GitStateError::Cancelled
            } else {
                GitStateError::Unavailable("dirwalk")
            }
        })?;
    for (entry, _collapsed) in collect.into_entries_by_path() {
        if cancel.is_cancelled() {
            return Err(GitStateError::Cancelled);
        }
        let path = entry.rela_path[..].to_vec();
        if path.is_empty() {
            continue;
        }
        let classification = match (entry.status, entry.disk_kind) {
            (_, Some(gix::dir::entry::Kind::Repository)) => {
                GitInventoryClassification::NestedRepository
            }
            (gix::dir::entry::Status::Tracked, _) => GitInventoryClassification::Tracked,
            (gix::dir::entry::Status::Ignored(_), _) => {
                GitInventoryClassification::UntrackedIgnored
            }
            (gix::dir::entry::Status::Untracked, _) => {
                GitInventoryClassification::UntrackedNotIgnored
            }
            (gix::dir::entry::Status::Pruned, _) => {
                GitInventoryClassification::ExcludedByCodeFabricPolicy
            }
        };
        let record = paths
            .entry(path.clone())
            .or_insert_with(|| GitInventoryEntry {
                repo_path_bytes: path,
                classification,
                conflict_stages: Vec::new(),
                index_mode: None,
                blob_oid: None,
                present_on_disk: true,
            });
        record.present_on_disk = true;
        if record.classification != GitInventoryClassification::SubmoduleGitlink {
            record.classification = classification;
        }
    }
    Ok(())
}

fn open_isolated(path: &Path) -> Result<gix::Repository, GitStateError> {
    gix::open_opts(
        path,
        gix::open::Options::isolated()
            .strict_config(true)
            .bail_if_untrusted(true),
    )
    .map_err(|_| GitStateError::NotAWorktree)
}

fn open_identity(identity: &GitWorktreeIdentity) -> Result<gix::Repository, GitStateError> {
    let path = identity
        .work_dir
        .as_ref()
        .unwrap_or(&identity.git_dir)
        .to_path_buf();
    open_isolated(&path)
}

fn native_path(path: &Path) -> GitNativePath {
    let display = path.to_string_lossy();
    GitNativePath {
        raw_bytes: path.as_os_str().as_bytes().to_vec(),
        display_is_lossy: matches!(display, std::borrow::Cow::Owned(_)),
        display: display.into_owned(),
    }
}

fn repository_kind(kind: gix::repository::Kind) -> GitRepositoryKind {
    match kind {
        gix::repository::Kind::Common => GitRepositoryKind::Common,
        gix::repository::Kind::LinkedWorkTree => GitRepositoryKind::LinkedWorktree,
        gix::repository::Kind::Submodule => GitRepositoryKind::Submodule,
    }
}

fn object_format(kind: gix::hash::Kind) -> Result<GitHashAlgorithm, GitStateError> {
    match kind {
        gix::hash::Kind::Sha1 => Ok(GitHashAlgorithm::Sha1),
        gix::hash::Kind::Sha256 => Ok(GitHashAlgorithm::Sha256),
        _ => Err(GitStateError::UnsupportedObjectFormat),
    }
}

fn detach_object_id(id: gix::hash::ObjectId) -> Result<GitObjectId, GitStateError> {
    let algorithm = object_format(id.kind())?;
    let bytes = id.as_slice().to_vec();
    if bytes.len() != algorithm.digest_bytes() {
        return Err(GitStateError::UnsupportedObjectFormat);
    }
    Ok(GitObjectId { algorithm, bytes })
}

fn capture_state_from_repository(
    repository: &gix::Repository,
    identity: &GitWorktreeIdentity,
    observations: GitStateObservations,
) -> Result<GitStateVector, GitStateError> {
    let head = repository
        .head()
        .map_err(|_| GitStateError::Unavailable("head"))?;
    let head_kind = if head.is_unborn() {
        HeadKind::Unborn
    } else if head.is_detached() {
        HeadKind::Detached
    } else {
        HeadKind::Symbolic
    };
    let head_target = head
        .id()
        .map(|id| detach_object_id(id.detach()))
        .transpose()?;
    let head_tree = if head_kind == HeadKind::Unborn {
        None
    } else {
        repository
            .head_tree_id()
            .ok()
            .map(|id| detach_object_id(id.detach()))
            .transpose()?
    };
    let index = repository
        .index_or_empty()
        .map_err(|_| GitStateError::Unavailable("index"))?;
    let index_entry_count = u64::try_from(index.entries().len())
        .map_err(|_| GitStateError::Unavailable("index-entry-count"))?;
    Ok(GitStateVector {
        repository_id: identity.repository_id,
        worktree_id: identity.worktree_id,
        head_kind,
        head_target,
        head_tree,
        index_fingerprint: Some(index_fingerprint(&index)?),
        index_entry_count: Some(index_entry_count),
        has_conflict_stages: index.entries().iter().any(|entry| entry.stage_raw() != 0),
        repository_state: operation_state(repository.state().as_ref()),
        inclusion_policy_fingerprint: observations.inclusion_policy_fingerprint,
        attributes_fingerprint: observations.attributes_fingerprint,
        worktree_inventory_digest: observations.worktree_inventory_digest,
    })
}

fn index_fingerprint(index: &gix::worktree::Index) -> Result<[u8; 32], GitStateError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.git.index.v1\0");
    if let Some(checksum) = index.checksum() {
        hasher.update(b"checksum\0");
        hash_object_id(&mut hasher, &detach_object_id(checksum)?);
    } else {
        hasher.update(b"entries\0");
        let mut entries = index.entries().iter().collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            left.path(index)
                .cmp(right.path(index))
                .then_with(|| left.stage_raw().cmp(&right.stage_raw()))
        });
        for entry in entries {
            let path = entry.path(index);
            hasher.update(&u64::try_from(path.len()).unwrap_or(u64::MAX).to_be_bytes());
            hasher.update(path);
            hash_object_id(&mut hasher, &detach_object_id(entry.id)?);
            hasher.update(&entry.stage_raw().to_be_bytes());
            hasher.update(&entry.mode.bits().to_be_bytes());
        }
    }
    Ok(*hasher.finalize().as_bytes())
}

fn hash_object_id(hasher: &mut blake3::Hasher, object_id: &GitObjectId) {
    hasher.update(&[match object_id.algorithm {
        GitHashAlgorithm::Sha1 => 1,
        GitHashAlgorithm::Sha256 => 2,
    }]);
    hasher.update(&object_id.bytes);
}

fn operation_state(state: Option<&gix::state::InProgress>) -> GitOperationState {
    match state {
        None => GitOperationState::Clean,
        Some(gix::state::InProgress::Merge) => GitOperationState::Merge,
        Some(
            gix::state::InProgress::Rebase
            | gix::state::InProgress::RebaseInteractive
            | gix::state::InProgress::ApplyMailboxRebase,
        ) => GitOperationState::Rebase,
        Some(gix::state::InProgress::CherryPick | gix::state::InProgress::CherryPickSequence) => {
            GitOperationState::CherryPick
        }
        Some(gix::state::InProgress::Revert | gix::state::InProgress::RevertSequence) => {
            GitOperationState::Revert
        }
        Some(gix::state::InProgress::Bisect) => GitOperationState::Bisect,
        Some(gix::state::InProgress::ApplyMailbox) => GitOperationState::Apply,
    }
}

/// Bounded blocking-job observations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GitJobMetrics {
    pub queue_depth: usize,
    pub active_jobs: usize,
    pub completed_jobs: u64,
    pub interrupted_jobs: u64,
    pub total_duration_micros: u64,
}

#[cfg(feature = "daemon")]
#[derive(Debug)]
struct GitJobCounters {
    queued: AtomicUsize,
    active: AtomicUsize,
    completed: AtomicU64,
    interrupted: AtomicU64,
    total_duration_micros: AtomicU64,
}

/// Tokio-to-blocking boundary for gix work.
#[cfg(feature = "daemon")]
#[derive(Clone, Debug)]
pub struct GitBlockingExecutor {
    permits: Arc<tokio::sync::Semaphore>,
    counters: Arc<GitJobCounters>,
}

#[cfg(feature = "daemon")]
impl GitBlockingExecutor {
    #[must_use]
    pub fn new(maximum_concurrent_jobs: usize) -> Self {
        Self {
            permits: Arc::new(tokio::sync::Semaphore::new(maximum_concurrent_jobs.max(1))),
            counters: Arc::new(GitJobCounters {
                queued: AtomicUsize::new(0),
                active: AtomicUsize::new(0),
                completed: AtomicU64::new(0),
                interrupted: AtomicU64::new(0),
                total_duration_micros: AtomicU64::new(0),
            }),
        }
    }

    /// Run one owned blocking job under the outer concurrency bound.
    ///
    /// # Errors
    ///
    /// Returns cancellation, semaphore closure, join failure, or the job error.
    pub async fn run<T, F>(&self, cancellation: GitCancellation, job: F) -> Result<T, GitStateError>
    where
        T: Send + 'static,
        F: FnOnce(GitCancellation) -> Result<T, GitStateError> + Send + 'static,
    {
        self.counters.queued.fetch_add(1, Ordering::AcqRel);
        let permit = self
            .permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| GitStateError::Unavailable("blocking-semaphore"))?;
        self.counters.queued.fetch_sub(1, Ordering::AcqRel);
        if cancellation.is_cancelled() {
            self.counters.interrupted.fetch_add(1, Ordering::AcqRel);
            return Err(GitStateError::Cancelled);
        }
        self.counters.active.fetch_add(1, Ordering::AcqRel);
        let started = Instant::now();
        let result = tokio::task::spawn_blocking(move || job(cancellation))
            .await
            .map_err(|_| GitStateError::Unavailable("blocking-join"))?;
        drop(permit);
        self.counters.active.fetch_sub(1, Ordering::AcqRel);
        self.counters.completed.fetch_add(1, Ordering::AcqRel);
        self.counters.total_duration_micros.fetch_add(
            u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
            Ordering::AcqRel,
        );
        if matches!(result, Err(GitStateError::Cancelled)) {
            self.counters.interrupted.fetch_add(1, Ordering::AcqRel);
        }
        result
    }

    #[must_use]
    pub fn metrics(&self) -> GitJobMetrics {
        GitJobMetrics {
            queue_depth: self.counters.queued.load(Ordering::Acquire),
            active_jobs: self.counters.active.load(Ordering::Acquire),
            completed_jobs: self.counters.completed.load(Ordering::Acquire),
            interrupted_jobs: self.counters.interrupted.load(Ordering::Acquire),
            total_duration_micros: self.counters.total_duration_micros.load(Ordering::Acquire),
        }
    }
}

/// Return the generic-walker fallback status for an acceleration failure.
#[must_use]
pub const fn fallback_acceleration_status(error: &GitStateError) -> GitAccelerationStatus {
    match error {
        GitStateError::NotAWorktree => GitAccelerationStatus::NotAGitWorktree,
        GitStateError::Cancelled => GitAccelerationStatus::GitScanning,
        GitStateError::TrustPolicy
        | GitStateError::UnsupportedObjectFormat
        | GitStateError::CandidateDeltasDeferred
        | GitStateError::Unavailable(_) => GitAccelerationStatus::GitDegraded,
    }
}

/// Keep the feature graph honest without exposing gix types.
#[must_use]
pub const fn supported_hash_algorithms() -> [GitHashAlgorithm; 2] {
    [GitHashAlgorithm::Sha1, GitHashAlgorithm::Sha256]
}

/// Overlay Git-native classification on a WP16 authoritative-byte inventory and persist
/// the resulting Merkle state. Git never supplies source content through this path.
///
/// # Errors
///
/// Returns the existing inventory persistence or source-generation fence error.
#[cfg(feature = "daemon")]
pub fn apply_to_source_inventory(
    git: &GitInventoryResult,
    source: &mut crate::inventory::SourceInventory,
    store: &mut crate::operational_store::OperationalStore,
) -> Result<(), crate::inventory::InventoryError> {
    let by_path = git
        .entries
        .iter()
        .map(|entry| (entry.repo_path_bytes.as_slice(), entry))
        .collect::<BTreeMap<_, _>>();
    for record in &mut source.records {
        let Some(git_entry) = by_path.get(record.path.raw_relative_path_bytes.as_slice()) else {
            continue;
        };
        record.git_repo_path_bytes = Some(git_entry.repo_path_bytes.clone());
        record.classification = match git_entry.classification {
            GitInventoryClassification::Tracked => {
                crate::inventory::InventoryClassification::Tracked
            }
            GitInventoryClassification::UntrackedNotIgnored => {
                crate::inventory::InventoryClassification::UntrackedNotIgnored
            }
            GitInventoryClassification::UntrackedIgnored => {
                crate::inventory::InventoryClassification::UntrackedIgnored
            }
            GitInventoryClassification::TrackedButIgnoredPatternMatches => {
                crate::inventory::InventoryClassification::TrackedButIgnoredPatternMatches
            }
            GitInventoryClassification::ExcludedByCodeFabricPolicy => {
                crate::inventory::InventoryClassification::ExcludedByCodeFabricPolicy
            }
            GitInventoryClassification::SubmoduleGitlink => {
                crate::inventory::InventoryClassification::SubmoduleGitlink
            }
            GitInventoryClassification::NestedRepository => {
                crate::inventory::InventoryClassification::NestedRepository
            }
            GitInventoryClassification::SpecialFile => {
                crate::inventory::InventoryClassification::SpecialFile
            }
        };
        record.git_blob_oid = git_entry.blob_oid.as_ref().map(encode_object_id);
    }
    source.digest = crate::inventory::merkle_inventory_digest(&source.records);
    crate::inventory::persist_inventory(
        store,
        source.workspace_id,
        source.source_generation,
        &source.records,
    )
}

#[cfg(feature = "daemon")]
fn encode_object_id(object_id: &GitObjectId) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(object_id.bytes.len() + 1);
    encoded.push(match object_id.algorithm {
        GitHashAlgorithm::Sha1 => 1,
        GitHashAlgorithm::Sha256 => 2,
    });
    encoded.extend_from_slice(&object_id.bytes);
    encoded
}
