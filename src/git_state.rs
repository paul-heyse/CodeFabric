//! Read-only Git discovery, topology, state, and inventory boundary.
//!
//! No `gix` value crosses this module. Each operation opens a fresh repository
//! handle with isolated configuration permissions and returns detached DTOs.

use std::collections::BTreeMap;
#[cfg(feature = "daemon")]
use std::collections::{BTreeSet, VecDeque};
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::{Path, PathBuf};
#[cfg(feature = "daemon")]
use std::sync::Arc;
#[cfg(feature = "daemon")]
use std::sync::atomic::Ordering;
#[cfg(feature = "daemon")]
use std::sync::atomic::{AtomicU64, AtomicUsize};
#[cfg(feature = "daemon")]
use std::time::Instant;

use thiserror::Error;

use gix::object::tree::diff::ChangeDetached;
#[cfg(feature = "daemon")]
use rusqlite::OptionalExtension as _;

use crate::cancellation::Cancellation;
use crate::registries::GitAccelerationStatus;
#[cfg(feature = "daemon")]
use crate::registries::UpdateCandidateStrategy;
pub use crate::registries::{
    GitCandidateMode, GitCandidateOrigin, GitHashAlgorithm, GitHeadKind as HeadKind,
    GitInventoryClassification, GitOperationState, GitRepositoryKind,
};

/// Canonical identities supplied by the workspace-registration boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredGitIdentity {
    pub repository_id: [u8; 16],
    pub worktree_id: [u8; 16],
}

impl GitHashAlgorithm {
    #[must_use]
    pub const fn digest_width_bytes(self) -> usize {
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

/// Detached Git inventory and its state fence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitInventoryResult {
    pub entries: Vec<GitInventoryEntry>,
    pub vector: GitStateVector,
}

/// One byte-safe path requiring authoritative current-byte confirmation.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct GitCandidate {
    pub repo_path_bytes: Vec<u8>,
    pub prior_repo_path_bytes: Option<Vec<u8>>,
    pub origin: GitCandidateOrigin,
}

/// Candidate set fenced by the exact post-operation Git state vector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitCandidateSet {
    pub candidates: Vec<GitCandidate>,
    pub vector: GitStateVector,
    pub full_rescan_required: bool,
}

#[cfg(feature = "daemon")]
impl GitCandidateMode {
    #[must_use]
    pub const fn code(self) -> i64 {
        self as i64
    }
}

/// Exact identity of one advisory candidate computation.
#[cfg(feature = "daemon")]
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct GitCandidateCacheKey {
    pub workspace_id: [u8; 16],
    pub worktree_id: [u8; 16],
    pub state_vector_digest: [u8; 32],
    pub topology_digest: [u8; 32],
    pub mode: GitCandidateMode,
}

/// Construct an exact cache key from application-owned state rather than gix handles.
#[cfg(feature = "daemon")]
#[must_use]
pub fn candidate_cache_key(
    workspace_id: [u8; 16],
    vector: &GitStateVector,
    topology_digest: [u8; 32],
    mode: GitCandidateMode,
) -> GitCandidateCacheKey {
    GitCandidateCacheKey {
        workspace_id,
        worktree_id: vector.worktree_id,
        state_vector_digest: state_vector_digest(vector),
        topology_digest,
        mode,
    }
}

/// Digest linked-worktree topology using byte-native paths and stable ordering.
#[cfg(feature = "daemon")]
#[must_use]
pub fn topology_digest(snapshot: &GitStateSnapshot) -> [u8; 32] {
    fn hash_path(hasher: &mut crate::identity::SemanticFingerprintBuilder, path: &GitNativePath) {
        hasher.update(
            &u64::try_from(path.raw_bytes.len())
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
        hasher.update(&path.raw_bytes);
    }

    let mut hasher = crate::identity::semantic_fingerprint(
        crate::identity::SemanticFingerprintDomain::GitTopology,
    );
    hasher.update(&snapshot.repository.repository_id);
    hash_path(&mut hasher, &snapshot.repository.common_dir_key);
    let mut worktrees = snapshot.linked_worktrees.iter().collect::<Vec<_>>();
    worktrees.sort_by(|left, right| {
        left.administrative_name
            .cmp(&right.administrative_name)
            .then_with(|| left.git_dir.raw_bytes.cmp(&right.git_dir.raw_bytes))
    });
    for worktree in worktrees {
        hasher.update(
            &u64::try_from(worktree.administrative_name.len())
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
        hasher.update(&worktree.administrative_name);
        hash_path(&mut hasher, &worktree.git_dir);
        if let Some(work_dir) = &worktree.work_dir {
            hasher.update(&[1]);
            hash_path(&mut hasher, work_dir);
        } else {
            hasher.update(&[0]);
        }
    }
    hasher.finalize()
}

#[cfg(feature = "daemon")]
#[derive(Clone, Debug)]
struct GitCandidateCacheEntry {
    key: GitCandidateCacheKey,
    payload: Vec<u8>,
    payload_digest: [u8; 32],
}

/// Bounded advisory L1/L2 cache. A miss or corrupt payload always widens to recomputation.
#[cfg(feature = "daemon")]
#[derive(Debug)]
pub struct GitCandidateCache {
    l1: VecDeque<GitCandidateCacheEntry>,
    maximum_l1_entries: usize,
    maximum_l2_entries_per_workspace: usize,
}

#[cfg(feature = "daemon")]
impl GitCandidateCache {
    #[must_use]
    pub fn new(maximum_l1_entries: usize, maximum_l2_entries_per_workspace: usize) -> Self {
        Self {
            l1: VecDeque::new(),
            maximum_l1_entries: maximum_l1_entries.max(1),
            maximum_l2_entries_per_workspace: maximum_l2_entries_per_workspace.max(1),
        }
    }

    /// Read an exact entry from L1 and then the operational SQLite L2 table.
    ///
    /// Corrupt or stale data is an ordinary miss because this cache is never authoritative.
    ///
    /// # Errors
    ///
    /// Returns a SQLite availability error; malformed payloads degrade to a cache miss.
    pub fn get(
        &mut self,
        key: &GitCandidateCacheKey,
        current_vector: &GitStateVector,
        reader: Option<&crate::operational_store::OperationalReader>,
    ) -> Result<Option<GitCandidateSet>, GitStateError> {
        if key.worktree_id != current_vector.worktree_id
            || key.state_vector_digest != state_vector_digest(current_vector)
        {
            return Ok(None);
        }
        if let Some(position) = self.l1.iter().position(|entry| &entry.key == key) {
            let Some(entry) = self.l1.remove(position) else {
                return Ok(None);
            };
            let decoded =
                verified_candidate_payload(&entry.payload, entry.payload_digest, current_vector);
            self.l1.push_back(entry);
            return Ok(decoded);
        }
        let Some(reader) = reader else {
            return Ok(None);
        };
        let cached = reader
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT candidate_payload, payload_digest
                         FROM git_candidate_cache
                         WHERE workspace_id = ?1 AND worktree_id = ?2
                           AND state_vector_digest = ?3 AND topology_digest = ?4
                           AND mode_code = ?5",
                        rusqlite::params![
                            key.workspace_id,
                            key.worktree_id,
                            key.state_vector_digest,
                            key.topology_digest,
                            key.mode.code(),
                        ],
                        |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?)),
                    )
                    .optional()
            })
            .map_err(|_| GitStateError::Unavailable("git-candidate-cache-read"))?;
        let Some((payload, digest_bytes)) = cached else {
            return Ok(None);
        };
        let Ok(payload_digest) = <[u8; 32]>::try_from(digest_bytes) else {
            return Ok(None);
        };
        let Some(decoded) = verified_candidate_payload(&payload, payload_digest, current_vector)
        else {
            return Ok(None);
        };
        self.push_l1(GitCandidateCacheEntry {
            key: key.clone(),
            payload,
            payload_digest,
        });
        Ok(Some(decoded))
    }

    /// Populate L1 and optionally L2 after an authoritative candidate computation.
    ///
    /// # Errors
    ///
    /// Returns a state-fence or SQLite write error.
    pub fn put(
        &mut self,
        key: &GitCandidateCacheKey,
        candidates: &GitCandidateSet,
        source_generation: u64,
        store: Option<&mut crate::operational_store::OperationalStore>,
    ) -> Result<(), GitStateError> {
        if key.worktree_id != candidates.vector.worktree_id
            || key.state_vector_digest != state_vector_digest(&candidates.vector)
        {
            return Err(GitStateError::Unavailable(
                "git-candidate-cache-state-fence",
            ));
        }
        let payload = encode_candidate_payload(candidates)?;
        let payload_digest = candidate_payload_digest(&payload);
        self.push_l1(GitCandidateCacheEntry {
            key: key.clone(),
            payload: payload.clone(),
            payload_digest,
        });
        let Some(store) = store else {
            return Ok(());
        };
        let source_generation = i64::try_from(source_generation)
            .map_err(|_| GitStateError::Unavailable("git-cache-generation"))?;
        let maximum = i64::try_from(self.maximum_l2_entries_per_workspace)
            .map_err(|_| GitStateError::Unavailable("git-cache-capacity"))?;
        store
            .write_transaction(|transaction| {
                transaction.execute(
                    "INSERT INTO git_candidate_cache (
                       workspace_id, worktree_id, state_vector_digest, topology_digest,
                       mode_code, candidate_payload, payload_digest, source_generation
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT(workspace_id, worktree_id, state_vector_digest, topology_digest, mode_code)
                     DO UPDATE SET candidate_payload = excluded.candidate_payload,
                       payload_digest = excluded.payload_digest,
                       source_generation = excluded.source_generation",
                    rusqlite::params![
                        key.workspace_id,
                        key.worktree_id,
                        key.state_vector_digest,
                        key.topology_digest,
                        key.mode.code(),
                        payload,
                        payload_digest,
                        source_generation,
                    ],
                )?;
                transaction.execute(
                    "DELETE FROM git_candidate_cache WHERE rowid IN (
                       SELECT rowid FROM git_candidate_cache
                       WHERE workspace_id = ?1
                       ORDER BY source_generation DESC, state_vector_digest DESC,
                         topology_digest DESC, mode_code DESC
                       LIMIT -1 OFFSET ?2
                     )",
                    rusqlite::params![key.workspace_id, maximum],
                )?;
                Ok::<_, crate::operational_store::OperationalStoreError>(())
            })
            .map_err(|_| GitStateError::Unavailable("git-candidate-cache-write"))
    }

    fn push_l1(&mut self, entry: GitCandidateCacheEntry) {
        if let Some(position) = self.l1.iter().position(|current| current.key == entry.key) {
            self.l1.remove(position);
        }
        self.l1.push_back(entry);
        while self.l1.len() > self.maximum_l1_entries {
            self.l1.pop_front();
        }
    }
}

/// Immutable inputs used to choose one Git acceleration strategy.
#[cfg(feature = "daemon")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitCandidatePlanningRequest {
    pub workspace_id: [u8; 16],
    pub registered_identity: RegisteredGitIdentity,
    pub observations: GitStateObservations,
    pub watcher_paths: BTreeSet<Vec<u8>>,
    pub rescan_required: bool,
    pub dirty_path_bulk_threshold: usize,
    pub maximum_candidate_paths: usize,
    pub source_generation: u64,
    pub prior_vector: Option<GitStateVector>,
    /// True only when `observations.worktree_inventory_digest` was independently refreshed for
    /// this planning turn. Without that proof the advisory cache is bypassed.
    pub cache_fence_verified: bool,
}

/// Application-owned candidate plan. Git paths remain hints until the source-image boundary recaptures
/// current bytes.
#[cfg(feature = "daemon")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitCandidatePlan {
    pub strategy: UpdateCandidateStrategy,
    pub candidate_paths: BTreeSet<Vec<u8>>,
    pub state_vector: Option<GitStateVector>,
    pub worktree_identity: Option<GitWorktreeIdentity>,
    pub topology_digest: Option<[u8; 32]>,
    pub acceleration: GitAccelerationStatus,
    pub cache_hit: bool,
    pub fallback_reason: Option<&'static str>,
}

#[cfg(feature = "daemon")]
impl GitCandidatePlan {
    #[must_use]
    pub const fn requires_generic_inventory(&self) -> bool {
        matches!(self.strategy, UpdateCandidateStrategy::GenericInventory)
    }

    /// Re-read the repository state after current-byte capture and before publication.
    /// Isolated watcher and generic-inventory plans have no Git fence and therefore succeed.
    fn verify_current<A: GitStateAdapter>(
        &self,
        adapter: &A,
        observations: GitStateObservations,
        cancellation: &Cancellation,
    ) -> Result<bool, GitStateError> {
        let Some(expected) = self.state_vector.as_ref() else {
            return Ok(true);
        };
        let identity = self
            .worktree_identity
            .as_ref()
            .ok_or(GitStateError::Unavailable("git-plan-worktree-identity"))?;
        if adapter.capture_state(identity, observations)? != *expected {
            return Ok(false);
        }
        let status = adapter.status_candidates(identity, observations, cancellation)?;
        if status.full_rescan_required || status.vector != *expected {
            return Ok(false);
        }
        Ok(status.candidates.iter().all(|candidate| {
            self.candidate_paths.contains(&candidate.repo_path_bytes)
                && candidate
                    .prior_repo_path_bytes
                    .as_ref()
                    .is_none_or(|prior| self.candidate_paths.contains(prior))
        }))
    }

    fn isolated(paths: BTreeSet<Vec<u8>>) -> Self {
        Self {
            strategy: UpdateCandidateStrategy::IsolatedPaths,
            candidate_paths: paths,
            state_vector: None,
            worktree_identity: None,
            topology_digest: None,
            acceleration: GitAccelerationStatus::GitReady,
            cache_hit: false,
            fallback_reason: None,
        }
    }

    fn fallback(status: GitAccelerationStatus, reason: &'static str) -> Self {
        Self {
            strategy: UpdateCandidateStrategy::GenericInventory,
            candidate_paths: BTreeSet::new(),
            state_vector: None,
            worktree_identity: None,
            topology_digest: None,
            acceleration: status,
            cache_hit: false,
            fallback_reason: Some(reason),
        }
    }
}

/// Deterministic Git candidate planner. The adapter and both cache tiers are accelerators only:
/// every failure becomes a generic-inventory request and no path is treated as source truth.
#[cfg(feature = "daemon")]
#[derive(Debug)]
pub struct GitCandidatePlanner<A> {
    adapter: A,
    cache: Option<GitCandidateCache>,
}

#[cfg(feature = "daemon")]
impl<A: GitStateAdapter> GitCandidatePlanner<A> {
    #[must_use]
    pub fn new(
        adapter: A,
        maximum_l1_entries: usize,
        maximum_l2_entries_per_workspace: usize,
    ) -> Self {
        Self {
            adapter,
            cache: Some(GitCandidateCache::new(
                maximum_l1_entries,
                maximum_l2_entries_per_workspace,
            )),
        }
    }

    #[must_use]
    pub const fn without_cache(adapter: A) -> Self {
        Self {
            adapter,
            cache: None,
        }
    }

    /// Verify an accelerated plan against a fresh post-capture state vector.
    ///
    /// # Errors
    ///
    /// Returns a detached Git boundary error when the post-capture state or status cannot be read.
    pub fn verify_current(
        &self,
        plan: &GitCandidatePlan,
        observations: GitStateObservations,
        cancellation: &Cancellation,
    ) -> Result<bool, GitStateError> {
        plan.verify_current(&self.adapter, observations, cancellation)
    }

    /// Select isolated, status/index, HEAD-tree, or generic-inventory work from current state.
    ///
    /// A second state capture fences the selected result against repository changes during the
    /// scan. Cache read/write failures are safe misses; Git failures request the generic walker.
    #[allow(clippy::too_many_lines)] // One deterministic planner keeps both state fences and every safe fallback visible.
    pub fn plan(
        &mut self,
        root: &Path,
        request: &GitCandidatePlanningRequest,
        store: Option<&mut crate::operational_store::OperationalStore>,
        cancellation: &Cancellation,
    ) -> GitCandidatePlan {
        if !request.rescan_required
            && request.watcher_paths.len() < request.dirty_path_bulk_threshold
        {
            return GitCandidatePlan::isolated(request.watcher_paths.clone());
        }
        if request.maximum_candidate_paths == 0 || request.dirty_path_bulk_threshold == 0 {
            return GitCandidatePlan::fallback(
                GitAccelerationStatus::GitDegraded,
                "candidate-budget-invalid",
            );
        }

        let policy = GitTrustPolicy::local_read_only();
        let snapshot = match self
            .adapter
            .open_worktree(root, request.registered_identity, &policy)
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return GitCandidatePlan::fallback(
                    fallback_acceleration_status(&error),
                    "git-open-unavailable",
                );
            }
        };
        let topology = topology_digest(&snapshot);
        let current = match self
            .adapter
            .capture_state(&snapshot.selected_worktree, request.observations)
        {
            Ok(vector) => vector,
            Err(error) => {
                return GitCandidatePlan::fallback(
                    fallback_acceleration_status(&error),
                    "git-state-unavailable",
                );
            }
        };
        let head_transition = request.prior_vector.as_ref().is_some_and(|prior| {
            prior.repository_id == current.repository_id
                && prior.worktree_id == current.worktree_id
                && prior.head_tree != current.head_tree
        });
        let (mode, strategy) = if head_transition {
            (
                GitCandidateMode::HeadTree,
                UpdateCandidateStrategy::HeadTreeAndStatus,
            )
        } else {
            (
                GitCandidateMode::Status,
                UpdateCandidateStrategy::GitStatusIndex,
            )
        };
        let key = candidate_cache_key(request.workspace_id, &current, topology, mode);

        let mut cache_hit = false;
        let cached = if request.cache_fence_verified
            && let Some(cache) = self.cache.as_mut()
        {
            let reader = store
                .as_deref()
                .and_then(|operational| operational.reader_factory().open().ok());
            let result = cache.get(&key, &current, reader.as_ref()).ok().flatten();
            cache_hit = result.is_some();
            result
        } else {
            None
        };
        let candidates = if let Some(cached) = cached {
            cached
        } else {
            let computed = if head_transition {
                let Some(prior) = request.prior_vector.as_ref() else {
                    return GitCandidatePlan::fallback(
                        GitAccelerationStatus::GitDegraded,
                        "prior-state-vector-absent",
                    );
                };
                let mut tree = match self.adapter.tree_diff_candidates(
                    &snapshot.selected_worktree,
                    prior,
                    request.observations,
                    cancellation,
                ) {
                    Ok(candidates) => candidates,
                    Err(error) => {
                        return GitCandidatePlan::fallback(
                            fallback_acceleration_status(&error),
                            "git-tree-candidate-scan-unavailable",
                        );
                    }
                };
                let status = match self.adapter.status_candidates(
                    &snapshot.selected_worktree,
                    request.observations,
                    cancellation,
                ) {
                    Ok(candidates) => candidates,
                    Err(error) => {
                        return GitCandidatePlan::fallback(
                            fallback_acceleration_status(&error),
                            "git-status-candidate-scan-unavailable",
                        );
                    }
                };
                if tree.vector != status.vector {
                    return GitCandidatePlan::fallback(
                        GitAccelerationStatus::GitMetadataDirty,
                        "git-state-changed-between-candidate-scans",
                    );
                }
                tree.full_rescan_required |= status.full_rescan_required;
                tree.candidates.extend(status.candidates);
                tree.candidates.sort();
                tree.candidates.dedup();
                Ok(tree)
            } else {
                self.adapter.status_candidates(
                    &snapshot.selected_worktree,
                    request.observations,
                    cancellation,
                )
            };
            match computed {
                Ok(candidates) => candidates,
                Err(error) => {
                    return GitCandidatePlan::fallback(
                        fallback_acceleration_status(&error),
                        "git-candidate-scan-unavailable",
                    );
                }
            }
        };
        if candidates.full_rescan_required {
            return GitCandidatePlan::fallback(
                GitAccelerationStatus::GitOperationInProgress,
                "git-operation-requires-inventory",
            );
        }
        let post_scan = match self
            .adapter
            .capture_state(&snapshot.selected_worktree, request.observations)
        {
            Ok(vector) => vector,
            Err(error) => {
                return GitCandidatePlan::fallback(
                    fallback_acceleration_status(&error),
                    "git-post-scan-state-unavailable",
                );
            }
        };
        if post_scan != candidates.vector {
            return GitCandidatePlan::fallback(
                GitAccelerationStatus::GitMetadataDirty,
                "git-state-changed-during-scan",
            );
        }

        let mut paths = BTreeSet::new();
        for candidate in &candidates.candidates {
            paths.insert(candidate.repo_path_bytes.clone());
            if let Some(prior) = &candidate.prior_repo_path_bytes {
                paths.insert(prior.clone());
            }
        }
        if paths.len() > request.maximum_candidate_paths {
            return GitCandidatePlan::fallback(
                GitAccelerationStatus::GitBulkReconciling,
                "git-candidate-budget-exceeded",
            );
        }
        if request.cache_fence_verified
            && !cache_hit
            && let Some(cache) = self.cache.as_mut()
        {
            let stable_key =
                candidate_cache_key(request.workspace_id, &candidates.vector, topology, mode);
            let _ = cache.put(&stable_key, &candidates, request.source_generation, store);
        }
        GitCandidatePlan {
            strategy,
            candidate_paths: paths,
            state_vector: Some(candidates.vector),
            worktree_identity: Some(snapshot.selected_worktree),
            topology_digest: Some(topology),
            acceleration: snapshot.acceleration,
            cache_hit,
            fallback_reason: None,
        }
    }
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
    #[error("CAPABILITY_UNAVAILABLE:GIT_STATE_UNAVAILABLE:{0}")]
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
        cancel: &Cancellation,
    ) -> Result<GitInventoryResult, GitStateError>;

    /// Produce status/index candidates, fenced by a fresh state vector.
    ///
    /// # Errors
    ///
    fn status_candidates(
        &self,
        identity: &GitWorktreeIdentity,
        observations: GitStateObservations,
        cancel: &Cancellation,
    ) -> Result<GitCandidateSet, GitStateError>;

    /// Wave-7 tree-diff-candidate seam.
    ///
    /// # Errors
    ///
    fn tree_diff_candidates(
        &self,
        identity: &GitWorktreeIdentity,
        prior: &GitStateVector,
        observations: GitStateObservations,
        cancel: &Cancellation,
    ) -> Result<GitCandidateSet, GitStateError>;
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
        cancel: &Cancellation,
    ) -> Result<GitInventoryResult, GitStateError> {
        inventory(identity, observations, cancel)
    }

    fn status_candidates(
        &self,
        identity: &GitWorktreeIdentity,
        observations: GitStateObservations,
        cancel: &Cancellation,
    ) -> Result<GitCandidateSet, GitStateError> {
        status_candidates(identity, observations, cancel)
    }

    fn tree_diff_candidates(
        &self,
        identity: &GitWorktreeIdentity,
        prior: &GitStateVector,
        observations: GitStateObservations,
        cancel: &Cancellation,
    ) -> Result<GitCandidateSet, GitStateError> {
        tree_diff_candidates(identity, prior, observations, cancel)
    }
}

fn status_candidates(
    identity: &GitWorktreeIdentity,
    observations: GitStateObservations,
    cancel: &Cancellation,
) -> Result<GitCandidateSet, GitStateError> {
    if cancel.is_cancelled() {
        return Err(GitStateError::Cancelled);
    }
    let repository = open_identity(identity)?;
    let mut candidates = Vec::new();
    let status = repository
        .status(gix::progress::Discard)
        .map_err(|_| GitStateError::Unavailable("status-platform"))?;
    let iter = status
        .into_iter(Vec::<gix::bstr::BString>::new())
        .map_err(|_| GitStateError::Unavailable("status-iterator"))?;
    for item in iter {
        if cancel.is_cancelled() {
            return Err(GitStateError::Cancelled);
        }
        let item = item.map_err(|_| GitStateError::Unavailable("status-item"))?;
        let origin = match &item {
            gix::status::Item::IndexWorktree(_) => GitCandidateOrigin::IndexWorktree,
            gix::status::Item::TreeIndex(_) => GitCandidateOrigin::HeadIndex,
        };
        candidates.push(GitCandidate {
            repo_path_bytes: item.location()[..].to_vec(),
            prior_repo_path_bytes: None,
            origin,
        });
    }
    candidates.sort();
    candidates.dedup();
    let vector = capture_state_from_repository(&repository, identity, observations)?;
    Ok(GitCandidateSet {
        full_rescan_required: vector.repository_state != GitOperationState::Clean
            || vector.has_conflict_stages,
        candidates,
        vector,
    })
}

fn tree_diff_candidates(
    identity: &GitWorktreeIdentity,
    prior: &GitStateVector,
    observations: GitStateObservations,
    cancel: &Cancellation,
) -> Result<GitCandidateSet, GitStateError> {
    if cancel.is_cancelled() {
        return Err(GitStateError::Cancelled);
    }
    if prior.repository_id != identity.repository_id || prior.worktree_id != identity.worktree_id {
        return Err(GitStateError::Unavailable("stale-state-vector-identity"));
    }
    let old_tree = prior
        .head_tree
        .as_ref()
        .ok_or(GitStateError::Unavailable("prior-head-tree"))?;
    let repository = open_identity(identity)?;
    if object_format(repository.object_hash())? != old_tree.algorithm {
        return Err(GitStateError::UnsupportedObjectFormat);
    }
    let current = capture_state_from_repository(&repository, identity, observations)?;
    let Some(new_tree) = current.head_tree.as_ref() else {
        return Ok(GitCandidateSet {
            candidates: Vec::new(),
            vector: current,
            full_rescan_required: true,
        });
    };
    let old_id = gix::ObjectId::try_from(old_tree.bytes.as_slice())
        .map_err(|_| GitStateError::UnsupportedObjectFormat)?;
    let new_id = gix::ObjectId::try_from(new_tree.bytes.as_slice())
        .map_err(|_| GitStateError::UnsupportedObjectFormat)?;
    let old_tree = repository
        .find_tree(old_id)
        .map_err(|_| GitStateError::Unavailable("prior-tree-object"))?;
    let new_tree = repository
        .find_tree(new_id)
        .map_err(|_| GitStateError::Unavailable("current-tree-object"))?;
    let changes = repository
        .diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None)
        .map_err(|_| GitStateError::Unavailable("head-tree-diff"))?;
    let mut candidates = Vec::new();
    for change in changes {
        if cancel.is_cancelled() {
            return Err(GitStateError::Cancelled);
        }
        match change {
            ChangeDetached::Addition { location, .. }
            | ChangeDetached::Deletion { location, .. }
            | ChangeDetached::Modification { location, .. } => candidates.push(GitCandidate {
                repo_path_bytes: location[..].to_vec(),
                prior_repo_path_bytes: None,
                origin: GitCandidateOrigin::HeadTree,
            }),
            ChangeDetached::Rewrite {
                source_location,
                location,
                ..
            } => candidates.push(GitCandidate {
                repo_path_bytes: location[..].to_vec(),
                prior_repo_path_bytes: Some(source_location[..].to_vec()),
                origin: GitCandidateOrigin::HeadTree,
            }),
        }
    }
    candidates.sort();
    candidates.dedup();
    Ok(GitCandidateSet {
        full_rescan_required: current.repository_state != GitOperationState::Clean
            || current.has_conflict_stages,
        candidates,
        vector: current,
    })
}

fn inventory(
    identity: &GitWorktreeIdentity,
    observations: GitStateObservations,
    cancel: &Cancellation,
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
    cancel: &Cancellation,
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
            cancel.interrupt_flag(),
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
        let prior_classification = paths.get(&path).map(|record| record.classification);
        let classification = match (entry.status, entry.disk_kind, prior_classification) {
            (_, Some(gix::dir::entry::Kind::Untrackable), _) => {
                GitInventoryClassification::SpecialFile
            }
            (_, Some(gix::dir::entry::Kind::Repository), _) => {
                GitInventoryClassification::NestedRepository
            }
            (gix::dir::entry::Status::Tracked, _, _) => GitInventoryClassification::Tracked,
            (gix::dir::entry::Status::Ignored(_), _, Some(GitInventoryClassification::Tracked)) => {
                GitInventoryClassification::TrackedButIgnoredPatternMatches
            }
            (gix::dir::entry::Status::Ignored(_), _, _) => {
                GitInventoryClassification::UntrackedIgnored
            }
            (gix::dir::entry::Status::Untracked, _, _) => {
                GitInventoryClassification::UntrackedNotIgnored
            }
            (gix::dir::entry::Status::Pruned, _, _) => {
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
    if bytes.len() != algorithm.digest_width_bytes() {
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
    let mut hasher =
        crate::identity::semantic_fingerprint(crate::identity::SemanticFingerprintDomain::GitIndex);
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
    Ok(hasher.finalize())
}

fn hash_object_id(
    hasher: &mut crate::identity::SemanticFingerprintBuilder,
    object_id: &GitObjectId,
) {
    hasher.update(&[match object_id.algorithm {
        GitHashAlgorithm::Sha1 => 1,
        GitHashAlgorithm::Sha256 => 2,
    }]);
    hasher.update(&object_id.bytes);
}

#[cfg(feature = "daemon")]
fn state_vector_digest(vector: &GitStateVector) -> [u8; 32] {
    fn hash_optional_object(
        hasher: &mut crate::identity::SemanticFingerprintBuilder,
        value: Option<&GitObjectId>,
    ) {
        if let Some(value) = value {
            hasher.update(&[1]);
            hash_object_id(hasher, value);
        } else {
            hasher.update(&[0]);
        }
    }

    let mut hasher = crate::identity::semantic_fingerprint(
        crate::identity::SemanticFingerprintDomain::GitStateVector,
    );
    hasher.update(&vector.repository_id);
    hasher.update(&vector.worktree_id);
    hasher.update(&[vector.head_kind as u8]);
    hash_optional_object(&mut hasher, vector.head_target.as_ref());
    hash_optional_object(&mut hasher, vector.head_tree.as_ref());
    if let Some(value) = vector.index_fingerprint {
        hasher.update(&[1]);
        hasher.update(&value);
    } else {
        hasher.update(&[0]);
    }
    hasher.update(&vector.index_entry_count.unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(&[u8::from(vector.has_conflict_stages)]);
    hasher.update(&[vector.repository_state as u8]);
    hasher.update(&vector.inclusion_policy_fingerprint);
    hasher.update(&vector.attributes_fingerprint);
    hasher.update(&vector.worktree_inventory_digest);
    hasher.finalize()
}

#[cfg(feature = "daemon")]
fn candidate_payload_digest(payload: &[u8]) -> [u8; 32] {
    let mut hasher = crate::integrity::CacheKeyHasher::for_domain(
        crate::integrity::CacheKeyDomain::GitCandidateCachePayload,
    );
    hasher.update(payload);
    hasher.finalize()
}

#[cfg(feature = "daemon")]
fn encode_candidate_payload(candidates: &GitCandidateSet) -> Result<Vec<u8>, GitStateError> {
    const MAXIMUM_CANDIDATES: usize = 100_000;
    const MAXIMUM_PATH_BYTES: usize = 1_048_576;
    if candidates.candidates.len() > MAXIMUM_CANDIDATES {
        return Err(GitStateError::Unavailable("git-cache-candidate-budget"));
    }
    let mut payload = b"CFGC1".to_vec();
    payload.push(u8::from(candidates.full_rescan_required));
    payload.extend_from_slice(
        &u32::try_from(candidates.candidates.len())
            .map_err(|_| GitStateError::Unavailable("git-cache-candidate-count"))?
            .to_be_bytes(),
    );
    for candidate in &candidates.candidates {
        if candidate.repo_path_bytes.len() > MAXIMUM_PATH_BYTES
            || candidate
                .prior_repo_path_bytes
                .as_ref()
                .is_some_and(|path| path.len() > MAXIMUM_PATH_BYTES)
        {
            return Err(GitStateError::Unavailable("git-cache-path-budget"));
        }
        payload.push(candidate.origin as u8);
        append_cache_bytes(&mut payload, &candidate.repo_path_bytes)?;
        if let Some(prior) = &candidate.prior_repo_path_bytes {
            payload.push(1);
            append_cache_bytes(&mut payload, prior)?;
        } else {
            payload.push(0);
        }
    }
    Ok(payload)
}

#[cfg(feature = "daemon")]
fn append_cache_bytes(payload: &mut Vec<u8>, value: &[u8]) -> Result<(), GitStateError> {
    payload.extend_from_slice(
        &u32::try_from(value.len())
            .map_err(|_| GitStateError::Unavailable("git-cache-path-length"))?
            .to_be_bytes(),
    );
    payload.extend_from_slice(value);
    Ok(())
}

#[cfg(feature = "daemon")]
fn verified_candidate_payload(
    payload: &[u8],
    expected_digest: [u8; 32],
    current_vector: &GitStateVector,
) -> Option<GitCandidateSet> {
    if candidate_payload_digest(payload) != expected_digest {
        return None;
    }
    decode_candidate_payload(payload, current_vector)
}

#[cfg(feature = "daemon")]
fn decode_candidate_payload(
    payload: &[u8],
    current_vector: &GitStateVector,
) -> Option<GitCandidateSet> {
    const MAXIMUM_CANDIDATES: usize = 100_000;
    const MAXIMUM_PATH_BYTES: usize = 1_048_576;

    fn take<'a>(payload: &'a [u8], cursor: &mut usize, count: usize) -> Option<&'a [u8]> {
        let end = cursor.checked_add(count)?;
        let bytes = payload.get(*cursor..end)?;
        *cursor = end;
        Some(bytes)
    }
    fn take_u32(payload: &[u8], cursor: &mut usize) -> Option<u32> {
        Some(u32::from_be_bytes(
            take(payload, cursor, 4)?.try_into().ok()?,
        ))
    }
    fn take_path(payload: &[u8], cursor: &mut usize) -> Option<Vec<u8>> {
        let length = usize::try_from(take_u32(payload, cursor)?).ok()?;
        if length > MAXIMUM_PATH_BYTES {
            return None;
        }
        Some(take(payload, cursor, length)?.to_vec())
    }

    if !payload.starts_with(b"CFGC1") {
        return None;
    }
    let mut cursor = 5;
    let full_rescan_required = match take(payload, &mut cursor, 1)?[0] {
        0 => false,
        1 => true,
        _ => return None,
    };
    let count = usize::try_from(take_u32(payload, &mut cursor)?).ok()?;
    if count > MAXIMUM_CANDIDATES {
        return None;
    }
    let mut candidates = Vec::with_capacity(count);
    for _ in 0..count {
        let origin =
            GitCandidateOrigin::try_from(u16::from(take(payload, &mut cursor, 1)?[0])).ok()?;
        let repo_path_bytes = take_path(payload, &mut cursor)?;
        let prior_repo_path_bytes = match take(payload, &mut cursor, 1)?[0] {
            0 => None,
            1 => Some(take_path(payload, &mut cursor)?),
            _ => return None,
        };
        candidates.push(GitCandidate {
            repo_path_bytes,
            prior_repo_path_bytes,
            origin,
        });
    }
    if cursor != payload.len() {
        return None;
    }
    Some(GitCandidateSet {
        candidates,
        vector: current_vector.clone(),
        full_rescan_required,
    })
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
    pub async fn run<T, F>(&self, cancellation: Cancellation, job: F) -> Result<T, GitStateError>
    where
        T: Send + 'static,
        F: FnOnce(Cancellation) -> Result<T, GitStateError> + Send + 'static,
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
        record.classification = git_entry.classification;
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
pub(crate) fn encode_object_id(object_id: &GitObjectId) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(object_id.bytes.len() + 1);
    encoded.push(match object_id.algorithm {
        GitHashAlgorithm::Sha1 => 1,
        GitHashAlgorithm::Sha256 => 2,
    });
    encoded.extend_from_slice(&object_id.bytes);
    encoded
}
