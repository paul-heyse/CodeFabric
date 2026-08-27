//! Actor-owned continuous source pipeline from watcher hints to immutable hot-overlay state.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use thiserror::Error;

use crate::cancellation::Cancellation;
use crate::fabric::ConsolidatedOverlay;
use crate::fabric::OverlayMutation;
use crate::git_state::{
    GitCandidatePlan, GitCandidatePlanner, GitCandidatePlanningRequest, GitStateAdapter,
    GitStateError, GitStateObservations, GitStateVector, RegisteredGitIdentity,
};
use crate::identity::PlatformCode;
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
use crate::secure_path::{PlatformPath, open_workspace_root};
use crate::source_image::SourceImageStore;
use crate::tree_sitter_adapter::TreeSitterEdit;

#[cfg(target_os = "macos")]
const fn host_platform_code() -> PlatformCode {
    PlatformCode::MacOs
}

#[cfg(not(target_os = "macos"))]
const fn host_platform_code() -> PlatformCode {
    PlatformCode::Unix
}

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

/// Detached immutable source identity supplied to semantic scheduling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticLaneSource {
    pub file_id: [u8; 16],
    pub raw_relative_path_bytes: Vec<u8>,
    pub language: crate::source_image::SourceLanguage,
    pub content_digest: [u8; 32],
}

/// One generation-fenced semantic scheduling transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticLaneRequest {
    pub wave_id: [u8; 16],
    pub workspace_id: [u8; 16],
    pub analysis_context_id: [u8; 16],
    pub source_generation: u64,
    pub provider_ids: BTreeSet<&'static str>,
    pub sources: Vec<SemanticLaneSource>,
}

/// Terminal receipt consumed by the continuous actor; provider-library output never crosses it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticLaneTerminal {
    pub provider_id: &'static str,
    pub provider_run_id: [u8; 16],
    pub source_generation: u64,
    pub state: crate::registries::ProviderRunState,
    pub output_fingerprint: Option<[u8; 32]>,
}

/// Closed scheduler result. Stale work is counted but cannot be staged or published.
#[derive(Clone, Debug, Default)]
pub struct SemanticLaneReport {
    pub terminals: Vec<SemanticLaneTerminal>,
    pub discarded_stale_runs: u64,
    pub semantic_mutations: Vec<OverlayMutation>,
}

/// Application port that owns ProviderRuntime submission and terminal-event collection.
pub trait SemanticLaneScheduler: Send + Sync {
    /// Schedule every required semantic provider and return only generation-current terminals.
    ///
    /// # Errors
    ///
    /// Returns a stable detail when admission, journaling, containment, or terminal verification
    /// fails. The continuous actor retains the non-current wave state for recovery.
    fn schedule(
        &self,
        store: &mut OperationalStore,
        request: &SemanticLaneRequest,
    ) -> Result<SemanticLaneReport, String>;
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
    semantic_scheduler: Option<Arc<dyn SemanticLaneScheduler>>,
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
            semantic_scheduler: None,
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

    /// Current authoritative inventory digest after the last accepted source capture.
    #[must_use]
    pub const fn current_inventory_digest(&self) -> [u8; 32] {
        self.config.git_observations.worktree_inventory_digest
    }

    /// Build one workspace from a genuine zero state by forcing the authoritative inventory
    /// walker, recapturing current bytes, and reconciling without watcher or Git candidates.
    ///
    /// # Errors
    ///
    /// Rejects a reused engine or operational state that has already advanced. The returned wave
    /// follows the ordinary source-capture and publication path; this is not a replay seam.
    pub fn rebuild_from_zero(
        &mut self,
        store: &mut OperationalStore,
    ) -> Result<Option<ContinuousWaveResult>, ContinuousError> {
        let recovery = crate::lifecycle::recover_workspace(
            &store.reader_factory().open()?,
            self.scheduler.workspace_id(),
        )?;
        if self.scheduler.current_source_generation() != 0
            || self.current_overlay().is_some()
            || recovery.source_generation != 0
            || !recovery.waves.is_empty()
            || recovery.overlay_recovery_required
        {
            return Err(ContinuousError::Lifecycle(LifecycleError::Configuration(
                "clean rebuild requires zero-generation engine and operational state with no prior wave or overlay"
                    .into(),
            )));
        }
        self.process_batch(
            store,
            WatchHintBatch {
                hints: Vec::new(),
                rescan_required: true,
            },
            &BTreeMap::new(),
        )
    }

    /// Publish source unavailability through the sole workspace freshness barrier.
    ///
    /// Daemon lifecycle code calls this when authoritative source capture cannot continue; query
    /// admission then reports the generated `Unavailable` state instead of implying an empty fact
    /// set is current.
    pub fn mark_source_unavailable(&self) {
        self.scheduler.freshness().mark_unavailable();
    }

    /// Replace the active analysis context and invalidate the context-scoped hot overlay.
    ///
    /// The next admitted batch must rebuild the affected context through the ordinary wave path;
    /// no facts from the predecessor context remain query-visible.
    pub fn replace_analysis_context(&mut self, analysis_context_id: [u8; 16]) {
        if self.config.analysis_context_id != analysis_context_id {
            self.config.analysis_context_id = analysis_context_id;
            self.overlays = ContinuousOverlayState::default();
        }
    }

    /// Change whether semantic-provider terminal evidence is required for publication.
    ///
    /// This is the actor-owned capability-withdrawal seam used when an analysis contract changes
    /// or a required provider becomes unavailable.
    pub const fn set_semantic_capabilities_required(&mut self, required: bool) {
        self.config.semantic_capabilities_required = required;
    }

    /// Install the sole semantic lane scheduler used for subsequent source generations.
    pub fn install_semantic_scheduler(&mut self, scheduler: Arc<dyn SemanticLaneScheduler>) {
        self.semantic_scheduler = Some(scheduler);
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
        let current_owners = fast_outputs
            .iter()
            .flat_map(|output| output.canonical.batches.values())
            .map(|batch| batch.scope().owner_id)
            .collect::<BTreeSet<_>>();
        let removed_owners = wave
            .items
            .iter()
            .filter(|item| item.captured.is_none())
            .filter_map(|item| prior_owners.get(&item.path_bytes).copied())
            // A rename can preserve canonical owner identity. In that case the same wave's
            // current-byte output is authoritative and an old-path tombstone would conflict
            // with that replacement at an equal generation.
            .filter(|owner_id| !current_owners.contains(owner_id))
            .collect::<BTreeSet<_>>();
        mutations.extend(removed_owner_mutations(
            wave.workspace_id,
            self.config.analysis_context_id,
            wave.source_generation,
            &removed_owners,
        )?);
        let mut staged = self.overlays.stage(
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
            let scheduler = self
                .semantic_scheduler
                .as_ref()
                .ok_or(ContinuousError::SemanticLaneUnavailable)?;
            let mut provider_ids = BTreeSet::new();
            let sources = wave
                .items
                .iter()
                .filter_map(|item| item.captured.as_deref())
                .filter_map(|source| {
                    let provider_id = match source.language {
                        crate::source_image::SourceLanguage::Python => "pyrefly-python",
                        crate::source_image::SourceLanguage::Rust => "rustc-mir",
                        crate::source_image::SourceLanguage::Other => return None,
                    };
                    provider_ids.insert(provider_id);
                    Some(SemanticLaneSource {
                        file_id: source.file_id,
                        raw_relative_path_bytes: source.path.raw_relative_path_bytes.clone(),
                        language: source.language.clone(),
                        content_digest: source.digest,
                    })
                })
                .collect::<Vec<_>>();
            let request = SemanticLaneRequest {
                wave_id: wave.wave_id,
                workspace_id: wave.workspace_id,
                analysis_context_id: self.config.analysis_context_id,
                source_generation: wave.source_generation,
                provider_ids: provider_ids.clone(),
                sources,
            };
            let report = scheduler
                .schedule(store, &request)
                .map_err(ContinuousError::SemanticLane)?;
            let terminal_ids = report
                .terminals
                .iter()
                .map(|terminal| terminal.provider_id)
                .collect::<BTreeSet<_>>();
            if terminal_ids != provider_ids
                || report.terminals.len() != provider_ids.len()
                || report.terminals.iter().any(|terminal| {
                    terminal.source_generation != wave.source_generation
                        || !matches!(
                            terminal.state,
                            crate::registries::ProviderRunState::Succeeded
                                | crate::registries::ProviderRunState::Partial
                        )
                })
            {
                return Err(ContinuousError::SemanticGenerationFence);
            }
            mutations.extend(report.semantic_mutations);
            staged = self.overlays.stage(
                wave.workspace_id,
                self.config.analysis_context_id,
                &mutations,
                self.config.overlay_memory_limit_bytes,
            )?;
            self.scheduler.transition(
                store,
                &mut wave,
                "semantic-outputs-staged",
                "semantic-providers-terminal",
            )?;
            self.scheduler.transition(
                store,
                &mut wave,
                "derivations-staged",
                "derivations-terminal",
            )?;
        } else {
            self.scheduler.transition(
                store,
                &mut wave,
                "semantic-work-not-applicable",
                "semantic-capabilities-terminal",
            )?;
        }
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
    // Union by the governed path comparison key, not raw display bytes. On a case-insensitive
    // filesystem an old spelling can still open the renamed file; admitting both spellings would
    // capture the same canonical owner twice and create equal-generation replacement conflicts.
    // Current authoritative inventory wins over a prior spelling for the same comparison key.
    let mut paths_by_comparison_key = BTreeMap::<Vec<u8>, Vec<u8>>::new();
    for path in prior_paths {
        let platform_path =
            PlatformPath::from_raw_relative_bytes(host_platform_code(), path.clone())?;
        let workspace_path = root.workspace_path(&platform_path)?;
        paths_by_comparison_key.insert(workspace_path.comparison_key_bytes, path);
    }
    for record in inventory
        .records
        .iter()
        .filter(|record| record.inclusion == InclusionState::Included)
    {
        paths_by_comparison_key.insert(
            record.path.comparison_key_bytes.clone(),
            record.path.raw_relative_path_bytes.clone(),
        );
    }
    let paths = paths_by_comparison_key.into_values().collect();
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
    #[error("SEMANTIC_LANE_UNAVAILABLE")]
    SemanticLaneUnavailable,
    #[error("SEMANTIC_LANE_FAILED: {0}")]
    SemanticLane(String),
    #[error("SEMANTIC_GENERATION_FENCE")]
    SemanticGenerationFence,
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

    struct RecordingSemanticScheduler;

    impl SemanticLaneScheduler for RecordingSemanticScheduler {
        fn schedule(
            &self,
            store: &mut OperationalStore,
            request: &SemanticLaneRequest,
        ) -> Result<SemanticLaneReport, String> {
            let mut terminals = Vec::new();
            for provider_id in &request.provider_ids {
                let provider = crate::registries::PROVIDER_ENTRIES
                    .iter()
                    .find(|entry| entry.provider_id == *provider_id)
                    .ok_or_else(|| "provider registry entry absent".to_owned())?;
                let code = u8::try_from(provider.provider_code).unwrap_or_default();
                let run_id = [code; 16];
                let mut record = crate::operational_store::ProviderRunRecord {
                    provider_run_id: run_id.to_vec(),
                    workspace_id: request.workspace_id.to_vec(),
                    analysis_context_id: request.analysis_context_id.to_vec(),
                    wave_id: request.wave_id.to_vec(),
                    provider_code: i64::from(provider.provider_code),
                    owner_id: None,
                    build_unit_id: None,
                    source_generation: i64::try_from(request.source_generation)
                        .map_err(|_| "generation overflow".to_owned())?,
                    input_fingerprint: vec![code; 32],
                    output_fingerprint: None,
                    sandbox_profile_digest: Some(format!("b3:{}", "33".repeat(32))),
                    state_code: i64::from(crate::registries::ProviderRunState::Queued as u16),
                    accepted_at: "1".into(),
                    terminal_at: None,
                    diagnostic_id: None,
                };
                store
                    .record_provider_run(&record)
                    .map_err(|error| error.to_string())?;
                let output = [code; 32];
                record.output_fingerprint = Some(output.to_vec());
                record.state_code =
                    i64::from(crate::registries::ProviderRunState::Succeeded as u16);
                record.terminal_at = Some("2".into());
                store
                    .record_provider_run(&record)
                    .map_err(|error| error.to_string())?;
                terminals.push(SemanticLaneTerminal {
                    provider_id,
                    provider_run_id: run_id,
                    source_generation: request.source_generation,
                    state: crate::registries::ProviderRunState::Succeeded,
                    output_fingerprint: Some(output),
                });
            }
            Ok(SemanticLaneReport {
                terminals,
                discarded_stale_runs: 1,
                semantic_mutations: Vec::new(),
            })
        }
    }

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
        let error = match engine.process_batch(
            &mut store,
            WatchHintBatch {
                hints: vec![WatchHint {
                    path_bytes: b"lib.rs".to_vec(),
                    kind: WatchHintKind::CreateOrModify,
                }],
                rescan_required: false,
            },
            &BTreeMap::new(),
        ) {
            Err(error) => error,
            Ok(_) => panic!("semantic lane without a scheduler must fail closed"),
        };
        assert!(matches!(error, ContinuousError::SemanticLaneUnavailable));
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
    fn semantic_lane_scheduling_conformance() {
        let directory = tempfile::tempdir().expect("semantic scheduling fixture");
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
                    inclusion_policy_fingerprint: [0x51; 32],
                    attributes_fingerprint: [0x52; 32],
                    worktree_inventory_digest: [0; 32],
                },
                prior_git_vector: None,
                overlay_memory_limit_bytes: 64 * 1024 * 1024,
                semantic_capabilities_required: true,
            },
        );
        engine.install_semantic_scheduler(Arc::new(RecordingSemanticScheduler));
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
            .expect("semantic wave succeeds")
            .expect("semantic wave publishes");
        assert_eq!(
            published.wave.state,
            crate::registries::UpdateWaveState::HotPublished
        );
        assert_eq!(
            engine.scheduler().freshness().state(),
            FreshnessState::Current
        );
        let (state_code, terminal_at): (i64, Option<String>) = store
            .reader_factory()
            .open()
            .unwrap()
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT state_code,terminal_at FROM provider_run WHERE workspace_id=?1",
                    [workspace_id.as_slice()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
            })
            .unwrap();
        assert_eq!(
            state_code,
            i64::from(crate::registries::ProviderRunState::Succeeded as u16)
        );
        assert!(terminal_at.is_some());
    }

    #[test]
    fn semantic_lane_operational_journal_gate() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("provider-recovery.sqlite");
        let mut store = OperationalStore::open(&path).unwrap();
        for (byte, state) in [
            (0x71, crate::registries::ProviderRunState::Queued),
            (0x72, crate::registries::ProviderRunState::Running),
        ] {
            store
                .record_provider_run(&crate::operational_store::ProviderRunRecord {
                    provider_run_id: vec![byte; 16],
                    workspace_id: vec![0x73; 16],
                    analysis_context_id: vec![0x74; 16],
                    wave_id: vec![0x75; 16],
                    provider_code: 40,
                    owner_id: None,
                    build_unit_id: None,
                    source_generation: 3,
                    input_fingerprint: vec![byte; 32],
                    output_fingerprint: None,
                    sandbox_profile_digest: Some(format!("b3:{}", "44".repeat(32))),
                    state_code: i64::from(state as u16),
                    accepted_at: "1".into(),
                    terminal_at: None,
                    diagnostic_id: None,
                })
                .unwrap();
        }
        drop(store);
        let mut recovered = OperationalStore::open(&path).unwrap();
        assert_eq!(recovered.recover_incomplete_provider_runs("2").unwrap(), 2);
        let rows = recovered
            .reader_factory()
            .open()
            .unwrap()
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT state_code,terminal_at FROM provider_run ORDER BY provider_run_id",
                )?;
                statement
                    .query_map([], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()
            })
            .unwrap();
        assert_eq!(
            rows,
            vec![
                (
                    i64::from(crate::registries::ProviderRunState::Cancelled as u16),
                    Some("2".into())
                ),
                (
                    i64::from(crate::registries::ProviderRunState::Crashed as u16),
                    Some("2".into())
                ),
            ]
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
    fn wp48_core_edit_corpus_publication_stays_current() {
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
        assert!(!initial.fast_outputs.is_empty());

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
        assert!(!broken.fast_outputs.is_empty());

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
        assert!(!repaired.fast_outputs.is_empty());
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
        assert!(!replayed.fast_outputs.is_empty());
        assert_eq!(
            restarted.scheduler().freshness().state(),
            FreshnessState::Current
        );
    }
}
