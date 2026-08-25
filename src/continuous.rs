//! Actor-owned continuous source pipeline from watcher hints to immutable hot-overlay state.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use thiserror::Error;

use crate::cancellation::Cancellation;
use crate::fabric::ConsolidatedOverlay;
use crate::git_state::{
    GitCandidatePlan, GitCandidatePlanner, GitCandidatePlanningRequest, GitStateAdapter,
    GitStateError, GitStateObservations, GitStateVector, RegisteredGitIdentity,
};
use crate::inventory::{
    InclusionState, InventoryError, InventoryFileUpsert, InventoryLimits, InventoryWalker,
    advance_inventory_generation,
};
use crate::lifecycle::{
    AcceptedUpdateWave, AuthoritativeCandidateSelection, ContinuousOverlayState,
    FastSyntaxFactOutput, FastSyntaxReconciler, LifecycleError, UpdateWaveScheduler,
    WatchHintBatch, fast_output_mutations, removed_owner_mutations,
};
use crate::operational_store::{OperationalStore, OperationalStoreError};
use crate::secure_path::open_workspace_root;
use crate::source_image::SourceImageStore;
use crate::tree_sitter_adapter::TreeSitterEdit;

/// Stable inputs that are not owned by watcher or Git libraries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinuousWorkspaceConfig {
    pub analysis_context_id: [u8; 16],
    pub registered_git_identity: Option<RegisteredGitIdentity>,
    pub git_observations: GitStateObservations,
    pub prior_git_vector: Option<GitStateVector>,
    pub overlay_memory_limit_bytes: usize,
    /// Whether the selected analysis contract requires semantic-provider completion.
    pub semantic_capabilities_required: bool,
}

fn semantic_lane_required<'a>(
    config: &ContinuousWorkspaceConfig,
    languages: impl IntoIterator<Item = &'a crate::source_image::SourceLanguage>,
) -> bool {
    config.semantic_capabilities_required
        && languages.into_iter().any(|language| {
            matches!(
                language,
                crate::source_image::SourceLanguage::Python
                    | crate::source_image::SourceLanguage::Rust
            )
        })
}

/// One successfully published source wave.
pub struct ContinuousWaveResult {
    pub wave: AcceptedUpdateWave,
    pub fast_outputs: Vec<FastSyntaxFactOutput>,
    pub overlay: Arc<ConsolidatedOverlay>,
    pub flush_required: bool,
    pub git_plan: Option<GitCandidatePlan>,
}

/// Single-writer continuous-update actor. Every library boundary returns detached application
/// models; only this actor advances wave, overlay, and freshness state.
pub struct ContinuousWorkspaceEngine<A> {
    scheduler: UpdateWaveScheduler,
    source_images: SourceImageStore,
    git_candidates: GitCandidatePlanner<A>,
    fast_syntax: FastSyntaxReconciler,
    overlays: ContinuousOverlayState,
    config: ContinuousWorkspaceConfig,
}

impl<A: GitStateAdapter> ContinuousWorkspaceEngine<A> {
    #[must_use]
    pub fn new(
        scheduler: UpdateWaveScheduler,
        source_images: SourceImageStore,
        git_candidates: GitCandidatePlanner<A>,
        config: ContinuousWorkspaceConfig,
    ) -> Self {
        Self {
            scheduler,
            source_images,
            git_candidates,
            fast_syntax: FastSyntaxReconciler::default(),
            overlays: ContinuousOverlayState::default(),
            config,
        }
    }

    #[must_use]
    pub const fn scheduler(&self) -> &UpdateWaveScheduler {
        &self.scheduler
    }

    #[must_use]
    pub fn current_overlay(&self) -> Option<Arc<ConsolidatedOverlay>> {
        self.overlays.current()
    }

    /// Process one normalized final-state batch through candidate selection, authoritative byte
    /// capture, canonical reconciliation, generated-policy overlay staging, and hot publication.
    ///
    /// # Errors
    ///
    /// Returns a typed operational, inventory, Git, source, provider, or publication failure. A
    /// changed Git fence persists a failed wave and requeues a generic reconcile before returning.
    #[allow(clippy::too_many_lines)] // One actor turn keeps selection, capture, reconciliation, and publication ordered.
    pub fn process_batch(
        &mut self,
        store: &mut OperationalStore,
        batch: WatchHintBatch,
        edits: &BTreeMap<Vec<u8>, TreeSitterEdit>,
    ) -> Result<Option<ContinuousWaveResult>, ContinuousError> {
        let watcher_paths = batch
            .hints
            .iter()
            .map(|hint| hint.path_bytes.clone())
            .collect::<BTreeSet<_>>();
        let widened = batch.rescan_required
            || watcher_paths.len() >= self.scheduler.config().dirty_path_bulk_threshold;
        self.scheduler.admit_persisted(store, batch)?;

        let mut git_plan = None;
        let prior_owners;
        let selection = if widened {
            let plan = self
                .config
                .registered_git_identity
                .map(|registered_identity| {
                    self.git_candidates.plan(
                        self.scheduler.root(),
                        &GitCandidatePlanningRequest {
                            workspace_id: self.scheduler.workspace_id(),
                            registered_identity,
                            observations: self.config.git_observations,
                            watcher_paths: watcher_paths.clone(),
                            rescan_required: true,
                            dirty_path_bulk_threshold: self
                                .scheduler
                                .config()
                                .dirty_path_bulk_threshold,
                            maximum_candidate_paths: self
                                .scheduler
                                .config()
                                .maximum_paths_per_batch,
                            source_generation: self
                                .scheduler
                                .current_source_generation()
                                .saturating_add(1),
                            prior_vector: self.config.prior_git_vector.clone(),
                            cache_fence_verified: false,
                        },
                        Some(store),
                        &Cancellation::default(),
                    )
                });
            match plan {
                Some(plan) if !plan.requires_generic_inventory() => {
                    prior_owners = load_prior_owners(
                        store,
                        self.scheduler.workspace_id(),
                        self.scheduler.current_source_generation(),
                        &plan.candidate_paths,
                    )?;
                    let selection = AuthoritativeCandidateSelection::from_git_plan(plan.clone())
                        .ok_or_else(|| {
                            LifecycleError::Configuration(
                                "non-fallback Git plan lacks a complete selection".into(),
                            )
                        })?;
                    git_plan = Some(plan);
                    selection
                }
                fallback => {
                    if let Some(plan) = fallback {
                        git_plan = Some(plan);
                    }
                    let generic = generic_inventory_selection(
                        store,
                        self.scheduler.workspace_id(),
                        self.scheduler.current_source_generation(),
                    )?;
                    self.config.git_observations.worktree_inventory_digest = generic.digest;
                    prior_owners = generic.prior_owners;
                    AuthoritativeCandidateSelection::generic_inventory(generic.paths)
                }
            }
        } else {
            prior_owners = load_prior_owners(
                store,
                self.scheduler.workspace_id(),
                self.scheduler.current_source_generation(),
                &watcher_paths,
            )?;
            // Isolated paths are owned by the scheduler's frozen watcher batch.
            AuthoritativeCandidateSelection::generic_inventory(BTreeSet::new())
        };

        let supplied_selection = widened.then_some(selection);
        let Some(mut wave) =
            self.scheduler
                .prepare_wave(store, &mut self.source_images, supplied_selection)?
        else {
            return Ok(None);
        };
        if wave.state == crate::registries::UpdateWaveState::Failed {
            return Err(ContinuousError::DeferredSourceDrift);
        }

        let prior_generation = wave
            .source_generation
            .checked_sub(1)
            .ok_or(ContinuousError::GenerationOverflow)?;
        let upserts = wave
            .items
            .iter()
            .filter_map(|item| item.captured.as_deref())
            .map(|source| InventoryFileUpsert {
                path: source.path.clone(),
                file_id: source.file_id,
                content_digest: source.digest,
                byte_length: source.byte_length,
                language: match source.language {
                    crate::source_image::SourceLanguage::Python => Some("python"),
                    crate::source_image::SourceLanguage::Rust => Some("rust"),
                    crate::source_image::SourceLanguage::Other => None,
                },
            })
            .collect::<Vec<_>>();
        let removals = wave
            .items
            .iter()
            .filter(|item| item.captured.is_none())
            .map(|item| item.path_bytes.clone())
            .collect::<BTreeSet<_>>();
        let current_inventory_digest = advance_inventory_generation(
            store,
            wave.workspace_id,
            prior_generation,
            wave.source_generation,
            &upserts,
            &removals,
        )?;

        if let Some(plan) = git_plan.as_ref()
            && !plan.requires_generic_inventory()
            && !self.git_candidates.verify_current(
                plan,
                self.config.git_observations,
                &Cancellation::default(),
            )?
        {
            self.scheduler.reject_candidate_fence(store, &mut wave)?;
            return Err(ContinuousError::CandidateFenceChanged);
        }
        self.config.git_observations.worktree_inventory_digest = current_inventory_digest;

        self.scheduler.transition(
            store,
            &mut wave,
            "sources-classified",
            "classifications-closed",
        )?;
        let fast_outputs =
            self.fast_syntax
                .reconcile_wave(&wave, self.config.analysis_context_id, edits)?;
        self.scheduler.transition(
            store,
            &mut wave,
            "fast-outputs-staged",
            "fast-providers-terminal",
        )?;

        let mut mutations = fast_output_mutations(&fast_outputs)?;
        let removed_owners = wave
            .items
            .iter()
            .filter(|item| item.captured.is_none())
            .filter_map(|item| prior_owners.get(&item.path_bytes).copied())
            .collect::<BTreeSet<_>>();
        mutations.extend(removed_owner_mutations(
            wave.workspace_id,
            self.config.analysis_context_id,
            wave.source_generation,
            &removed_owners,
        )?);
        let staged = self.overlays.stage(
            wave.workspace_id,
            self.config.analysis_context_id,
            &mutations,
            self.config.overlay_memory_limit_bytes,
        )?;
        self.scheduler.transition(
            store,
            &mut wave,
            "fast-output-valid",
            "fast-contracts-satisfied",
        )?;
        if semantic_lane_required(
            &self.config,
            wave.items
                .iter()
                .filter_map(|item| item.captured.as_deref())
                .map(|source| &source.language),
        ) {
            self.scheduler.transition(
                store,
                &mut wave,
                "semantic-work-required",
                "semantic-capabilities-applicable",
            )?;
            return Ok(None);
        }
        self.scheduler.transition(
            store,
            &mut wave,
            "semantic-work-not-applicable",
            "semantic-capabilities-terminal",
        )?;
        self.scheduler.transition(
            store,
            &mut wave,
            "wave-output-valid",
            "required-capabilities-terminal",
        )?;
        self.overlays.publish_staged(Arc::clone(&staged))?;
        if let Some(vector) = wave.candidate_state_vector.clone() {
            self.config.prior_git_vector = Some(vector);
        }
        let flush_required = self
            .overlays
            .flush_candidate(self.scheduler.config().overlay_flush_policy)
            .is_some();
        Ok(Some(ContinuousWaveResult {
            wave,
            fast_outputs,
            overlay: staged,
            flush_required,
            git_plan,
        }))
    }
}

struct GenericInventorySelection {
    paths: BTreeSet<Vec<u8>>,
    prior_owners: BTreeMap<Vec<u8>, [u8; 16]>,
    digest: [u8; 32],
}

fn generic_inventory_selection(
    store: &mut OperationalStore,
    workspace_id: [u8; 16],
    source_generation: u64,
) -> Result<GenericInventorySelection, ContinuousError> {
    let prior_paths = load_prior_paths(store, workspace_id, source_generation)?;
    let prior_owners = load_prior_owners(store, workspace_id, source_generation, &prior_paths)?;
    let root = open_workspace_root(store, workspace_id)?;
    let inventory = InventoryWalker::new(InventoryLimits::default()).walk_and_persist(
        &root,
        store,
        source_generation,
        &Cancellation::default(),
    )?;
    let mut paths = prior_paths;
    paths.extend(
        inventory
            .records
            .iter()
            .filter(|record| record.inclusion == InclusionState::Included)
            .map(|record| record.path.raw_relative_path_bytes.clone()),
    );
    Ok(GenericInventorySelection {
        paths,
        prior_owners,
        digest: inventory.digest,
    })
}

fn load_prior_paths(
    store: &OperationalStore,
    workspace_id: [u8; 16],
    source_generation: u64,
) -> Result<BTreeSet<Vec<u8>>, ContinuousError> {
    let generation =
        i64::try_from(source_generation).map_err(|_| ContinuousError::GenerationOverflow)?;
    let reader = store.reader_factory().open()?;
    reader
        .with_connection_result(|connection| {
            let mut statement = connection.prepare(
                "SELECT path_bytes FROM source_inventory WHERE workspace_id=?1 AND source_generation=?2 AND inclusion_state_code=?3 ORDER BY path_bytes",
            )?;
            statement
                .query_map(
                    rusqlite::params![
                        workspace_id.as_slice(),
                        generation,
                        i64::from(InclusionState::Included as u16)
                    ],
                    |row| row.get::<_, Vec<u8>>(0),
                )?
                .collect::<Result<BTreeSet<_>, _>>()
                .map_err(ContinuousError::from)
        })
}

fn load_prior_owners(
    store: &OperationalStore,
    workspace_id: [u8; 16],
    source_generation: u64,
    paths: &BTreeSet<Vec<u8>>,
) -> Result<BTreeMap<Vec<u8>, [u8; 16]>, ContinuousError> {
    if paths.is_empty() {
        return Ok(BTreeMap::new());
    }
    let generation =
        i64::try_from(source_generation).map_err(|_| ContinuousError::GenerationOverflow)?;
    let reader = store.reader_factory().open()?;
    let rows = reader.with_connection_result(|connection| {
        let mut statement = connection.prepare(
            "SELECT path_bytes,COALESCE(current_file_owner,file_id) FROM source_inventory WHERE workspace_id=?1 AND source_generation=?2 AND COALESCE(current_file_owner,file_id) IS NOT NULL ORDER BY path_bytes",
        )?;
        statement
            .query_map(
                rusqlite::params![workspace_id.as_slice(), generation],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?)),
            )?
            .collect::<Result<Vec<_>, _>>()
            .map_err(ContinuousError::from)
    })?;
    rows.into_iter()
        .filter(|(path, _)| paths.contains(path))
        .map(|(path, owner)| {
            let owner =
                <[u8; 16]>::try_from(owner).map_err(|_| ContinuousError::CorruptOwnerIdentity)?;
            Ok((path, owner))
        })
        .collect()
}

#[derive(Debug, Error)]
pub enum ContinuousError {
    #[error(transparent)]
    Lifecycle(#[from] LifecycleError),
    #[error(transparent)]
    Inventory(#[from] InventoryError),
    #[error(transparent)]
    Git(#[from] GitStateError),
    #[error(transparent)]
    Store(#[from] OperationalStoreError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    SecurePath(#[from] crate::secure_path::SecurePathError),
    #[error("CONTINUOUS_CANDIDATE_FENCE_CHANGED")]
    CandidateFenceChanged,
    #[error("CONTINUOUS_SOURCE_DRIFT_DEFERRED")]
    DeferredSourceDrift,
    #[error("CONTINUOUS_SOURCE_GENERATION_OVERFLOW")]
    GenerationOverflow,
    #[error("CONTINUOUS_CORRUPT_OWNER_IDENTITY")]
    CorruptOwnerIdentity,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_state::GixGitStateAdapter;
    use crate::lifecycle::{
        FreshnessState, LifecycleConfig, OverlayFlushPolicy, WatchHint, WatchHintKind,
    };
    use crate::source_image::SourceCapturePolicy;
    use crate::workspace_registry::{WorkspaceRegistry, WorkspaceSourceRegistration};

    fn lifecycle_config() -> LifecycleConfig {
        LifecycleConfig {
            debounce_timeout: std::time::Duration::from_millis(20),
            tick_rate: std::time::Duration::from_millis(5),
            ingress_capacity: 32,
            maximum_paths_per_batch: 128,
            gather_window: std::time::Duration::from_millis(5),
            dirty_path_bulk_threshold: 8,
            await_current_timeout: std::time::Duration::from_secs(1),
            maximum_capture_bytes: 1024 * 1024,
            stable_read_retry_count: 2,
            source_blob_lease_ttl: std::time::Duration::from_secs(60),
            overlay_flush_policy: OverlayFlushPolicy {
                maximum_rows: 100_000,
                maximum_bytes: 64 * 1024 * 1024,
                maximum_touched_owners: 1_000,
                maximum_generations: 32,
            },
        }
    }

    fn assert_matches_clean_rebuild(result: &ContinuousWaveResult) {
        let mut wave = result.wave.clone();
        wave.state = crate::registries::UpdateWaveState::FastAnalyzing;
        let rebuilt = crate::lifecycle::FastSyntaxReconciler::default()
            .reconcile_wave(&wave, crate::identity::SOURCE_CONTEXT_ID, &BTreeMap::new())
            .expect("clean fast-lane rebuild");
        let digests = |outputs: &[FastSyntaxFactOutput]| {
            outputs
                .iter()
                .map(|output| {
                    (
                        output.path_bytes.clone(),
                        output
                            .canonical
                            .batches
                            .iter()
                            .map(|(code, batch)| {
                                (*code, crate::fabric::batch_checksum(batch.batch()).unwrap())
                            })
                            .collect::<BTreeMap<_, _>>(),
                    )
                })
                .collect::<BTreeMap<_, _>>()
        };
        assert_eq!(digests(&result.fast_outputs), digests(&rebuilt));
    }

    #[test]
    fn wp61_negative_zero_state() {
        let directory = tempfile::tempdir().expect("semantic guard fixture");
        let root = directory.path().join("workspace");
        std::fs::create_dir(&root).expect("workspace root");
        std::fs::write(root.join("lib.rs"), b"pub fn answer() -> u32 { 42 }\n")
            .expect("Rust source");
        let mut store = OperationalStore::open(&directory.path().join("operational.sqlite"))
            .expect("operational store");
        let workspace_id = WorkspaceRegistry::new(&mut store)
            .add(&root, WorkspaceSourceRegistration::Directory)
            .expect("workspace registration")
            .workspace_id;
        let lifecycle = lifecycle_config();
        let scheduler = UpdateWaveScheduler::new(workspace_id, &root, 0, 0, 0, lifecycle).unwrap();
        let source_images = SourceImageStore::open(
            &directory.path().join("source-blobs"),
            SourceCapturePolicy {
                maximum_bytes: lifecycle.maximum_capture_bytes,
                stable_read_retries: lifecycle.stable_read_retry_count,
                lease_ttl: lifecycle.source_blob_lease_ttl,
            },
        )
        .unwrap();
        let mut engine = ContinuousWorkspaceEngine::new(
            scheduler,
            source_images,
            GitCandidatePlanner::without_cache(GixGitStateAdapter),
            ContinuousWorkspaceConfig {
                analysis_context_id: crate::identity::SOURCE_CONTEXT_ID,
                registered_git_identity: None,
                git_observations: GitStateObservations {
                    inclusion_policy_fingerprint: [0x41; 32],
                    attributes_fingerprint: [0x42; 32],
                    worktree_inventory_digest: [0; 32],
                },
                prior_git_vector: None,
                overlay_memory_limit_bytes: 64 * 1024 * 1024,
                semantic_capabilities_required: true,
            },
        );
        let published = engine
            .process_batch(
                &mut store,
                WatchHintBatch {
                    hints: vec![WatchHint {
                        path_bytes: b"lib.rs".to_vec(),
                        kind: WatchHintKind::CreateOrModify,
                    }],
                    rescan_required: false,
                },
                &BTreeMap::new(),
            )
            .expect("guard evaluation");
        assert!(published.is_none());
        let (state_code, terminal_at): (i64, Option<String>) = store
            .reader_factory()
            .open()
            .unwrap()
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT state_code,terminal_at FROM update_wave WHERE workspace_id=?1 ORDER BY source_generation DESC LIMIT 1",
                    [workspace_id.as_slice()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
            })
            .unwrap();
        assert_eq!(
            state_code,
            i64::from(crate::registries::UpdateWaveState::SemanticAnalyzing as u16)
        );
        assert!(terminal_at.is_none());
        assert_eq!(
            engine.scheduler().freshness().state(),
            FreshnessState::PotentiallyStale
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One oracle proves the coupled rebase, publication, and recovery invariants.
    fn wp48_behavioral_acceptance() {
        let directory = tempfile::tempdir().expect("continuous fixture");
        let root = directory.path().join("workspace");
        std::fs::create_dir(&root).expect("workspace root");
        std::fs::write(root.join("a.py"), b"value = 1\n").expect("initial source");
        let mut store = OperationalStore::open(&directory.path().join("operational.sqlite"))
            .expect("operational store");
        let workspace_id = WorkspaceRegistry::new(&mut store)
            .add(&root, WorkspaceSourceRegistration::Directory)
            .expect("workspace registration")
            .workspace_id;
        let lifecycle = lifecycle_config();
        let scheduler =
            UpdateWaveScheduler::new(workspace_id, &root, 0, 0, 0, lifecycle).expect("scheduler");
        let source_images = SourceImageStore::open(
            &directory.path().join("source-blobs"),
            SourceCapturePolicy {
                maximum_bytes: lifecycle.maximum_capture_bytes,
                stable_read_retries: lifecycle.stable_read_retry_count,
                lease_ttl: lifecycle.source_blob_lease_ttl,
            },
        )
        .expect("source-image store");
        let mut engine = ContinuousWorkspaceEngine::new(
            scheduler,
            source_images,
            GitCandidatePlanner::without_cache(GixGitStateAdapter),
            ContinuousWorkspaceConfig {
                analysis_context_id: crate::identity::SOURCE_CONTEXT_ID,
                registered_git_identity: None,
                git_observations: GitStateObservations {
                    inclusion_policy_fingerprint: [0x31; 32],
                    attributes_fingerprint: [0x32; 32],
                    worktree_inventory_digest: [0; 32],
                },
                prior_git_vector: None,
                overlay_memory_limit_bytes: 64 * 1024 * 1024,
                semantic_capabilities_required: false,
            },
        );

        let created = engine
            .process_batch(
                &mut store,
                WatchHintBatch {
                    hints: vec![WatchHint {
                        path_bytes: b"a.py".to_vec(),
                        kind: WatchHintKind::CreateOrModify,
                    }],
                    rescan_required: false,
                },
                &BTreeMap::new(),
            )
            .expect("create wave")
            .expect("published create wave");
        assert_eq!(
            created.wave.state,
            crate::registries::UpdateWaveState::HotPublished
        );
        assert!(!created.fast_outputs.is_empty());
        assert_eq!(
            engine.scheduler().freshness().state(),
            FreshnessState::Current
        );
        assert_eq!(created.overlay.overlay_generation(), 1);

        std::fs::remove_file(root.join("a.py")).expect("remove source");
        let removed = engine
            .process_batch(
                &mut store,
                WatchHintBatch {
                    hints: vec![WatchHint {
                        path_bytes: b"a.py".to_vec(),
                        kind: WatchHintKind::Remove,
                    }],
                    rescan_required: false,
                },
                &BTreeMap::new(),
            )
            .expect("remove wave")
            .expect("published remove wave");
        assert!(removed.fast_outputs.is_empty());
        assert_eq!(removed.overlay.overlay_generation(), 2);
        assert_eq!(
            removed
                .overlay
                .table(140)
                .expect("source-file overlay")
                .owner_tombstones()
                .num_rows(),
            1
        );
        let remaining = store
            .reader_factory()
            .open()
            .expect("inventory reader")
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT COUNT(*) FROM source_inventory WHERE workspace_id=?1 AND source_generation=2",
                    [workspace_id.as_slice()],
                    |row| row.get::<_, i64>(0),
                )
            })
            .expect("inventory count");
        assert_eq!(remaining, 0);
        assert_eq!(
            engine.scheduler().freshness().state(),
            FreshnessState::Current
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // The ordered edit corpus is deliberately exercised as one continuous actor history.
    fn wp48_core_edit_corpus_matches_clean_rebuild_after_each_publication() {
        let directory = tempfile::tempdir().expect("core edit fixture");
        let root = directory.path().join("workspace");
        std::fs::create_dir_all(root.join("generated")).expect("workspace roots");
        std::fs::write(root.join("a.py"), b"value = 1\n").expect("Python source");
        std::fs::write(root.join("b.rs"), b"pub fn value() -> i32 { 1 }\n").expect("Rust source");
        std::fs::write(root.join("generated/bindings.py"), b"BOUND = 1\n")
            .expect("generated source");
        let mut store = OperationalStore::open(&directory.path().join("operational.sqlite"))
            .expect("operational store");
        let workspace_id = WorkspaceRegistry::new(&mut store)
            .add(&root, WorkspaceSourceRegistration::Directory)
            .expect("workspace registration")
            .workspace_id;
        let lifecycle = lifecycle_config();
        let scheduler =
            UpdateWaveScheduler::new(workspace_id, &root, 0, 0, 0, lifecycle).expect("scheduler");
        let source_images = SourceImageStore::open(
            &directory.path().join("source-blobs"),
            SourceCapturePolicy {
                maximum_bytes: lifecycle.maximum_capture_bytes,
                stable_read_retries: lifecycle.stable_read_retry_count,
                lease_ttl: lifecycle.source_blob_lease_ttl,
            },
        )
        .expect("source-image store");
        let mut engine = ContinuousWorkspaceEngine::new(
            scheduler,
            source_images,
            GitCandidatePlanner::without_cache(GixGitStateAdapter),
            ContinuousWorkspaceConfig {
                analysis_context_id: crate::identity::SOURCE_CONTEXT_ID,
                registered_git_identity: None,
                git_observations: GitStateObservations {
                    inclusion_policy_fingerprint: [0x31; 32],
                    attributes_fingerprint: [0x32; 32],
                    worktree_inventory_digest: [0; 32],
                },
                prior_git_vector: None,
                overlay_memory_limit_bytes: 64 * 1024 * 1024,
                semantic_capabilities_required: false,
            },
        );

        let initial = engine
            .process_batch(
                &mut store,
                WatchHintBatch {
                    hints: Vec::new(),
                    rescan_required: true,
                },
                &BTreeMap::new(),
            )
            .expect("initial rescan")
            .expect("initial publication");
        assert_matches_clean_rebuild(&initial);

        std::fs::write(root.join("a.py"), b"def broken(:\n").expect("parse break");
        let broken = engine
            .process_batch(
                &mut store,
                WatchHintBatch {
                    hints: vec![WatchHint {
                        path_bytes: b"a.py".to_vec(),
                        kind: WatchHintKind::CreateOrModify,
                    }],
                    rescan_required: false,
                },
                &BTreeMap::new(),
            )
            .expect("parse-break wave")
            .expect("parse-break publication");
        assert_matches_clean_rebuild(&broken);

        std::fs::write(root.join("a.py"), b"value = 2\n").expect("parse repair");
        std::fs::rename(root.join("b.rs"), root.join("renamed.rs")).expect("rename source");
        std::fs::write(root.join("generated/bindings.py"), b"BOUND = 2\n")
            .expect("generated burst");
        let repaired = engine
            .process_batch(
                &mut store,
                WatchHintBatch {
                    hints: vec![
                        WatchHint {
                            path_bytes: b"a.py".to_vec(),
                            kind: WatchHintKind::CreateOrModify,
                        },
                        WatchHint {
                            path_bytes: b"b.rs".to_vec(),
                            kind: WatchHintKind::RenameSource,
                        },
                        WatchHint {
                            path_bytes: b"renamed.rs".to_vec(),
                            kind: WatchHintKind::RenameTarget,
                        },
                        WatchHint {
                            path_bytes: b"generated/bindings.py".to_vec(),
                            kind: WatchHintKind::CreateOrModify,
                        },
                    ],
                    rescan_required: true,
                },
                &BTreeMap::new(),
            )
            .expect("repair/rename/burst wave")
            .expect("repair/rename/burst publication");
        assert_matches_clean_rebuild(&repaired);
        assert_eq!(
            engine.scheduler().freshness().state(),
            FreshnessState::Current
        );

        let recovery = crate::lifecycle::recover_workspace(
            &store.reader_factory().open().expect("recovery reader"),
            workspace_id,
        )
        .expect("restart recovery");
        assert!(!recovery.restart_paths.is_empty());
        assert!(!recovery.full_inventory_required);
        drop(engine);

        let mut scheduler = UpdateWaveScheduler::new(
            workspace_id,
            &root,
            recovery.source_generation,
            recovery.event_watermark,
            recovery.event_watermark,
            lifecycle,
        )
        .expect("restarted scheduler");
        scheduler
            .restore_recovery(&recovery)
            .expect("restore unfinished hot waves");
        let source_images = SourceImageStore::open(
            &directory.path().join("source-blobs"),
            SourceCapturePolicy {
                maximum_bytes: lifecycle.maximum_capture_bytes,
                stable_read_retries: lifecycle.stable_read_retry_count,
                lease_ttl: lifecycle.source_blob_lease_ttl,
            },
        )
        .expect("restarted source-image store");
        let mut restarted = ContinuousWorkspaceEngine::new(
            scheduler,
            source_images,
            GitCandidatePlanner::without_cache(GixGitStateAdapter),
            ContinuousWorkspaceConfig {
                analysis_context_id: crate::identity::SOURCE_CONTEXT_ID,
                registered_git_identity: None,
                git_observations: GitStateObservations {
                    inclusion_policy_fingerprint: [0x31; 32],
                    attributes_fingerprint: [0x32; 32],
                    worktree_inventory_digest: [0; 32],
                },
                prior_git_vector: None,
                overlay_memory_limit_bytes: 64 * 1024 * 1024,
                semantic_capabilities_required: false,
            },
        );
        let replayed = restarted
            .process_batch(
                &mut store,
                WatchHintBatch {
                    hints: Vec::new(),
                    rescan_required: false,
                },
                &BTreeMap::new(),
            )
            .expect("restart replay")
            .expect("restart publication");
        assert_matches_clean_rebuild(&replayed);
        assert_eq!(
            restarted.scheduler().freshness().state(),
            FreshnessState::Current
        );
    }
}
