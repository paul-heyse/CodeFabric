//! One serialized workspace update owner reuses capture, providers and exact activation.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use super::{
    AuthorizationRef, CompiledSemanticRelease, DeltaActivationRuntimeAuthority, ExpectedHead,
    FabricCommandRuntime, OperationalStore, PrincipalId, ProductionWorkspaceResources,
    ProductionWorkspaceStartupError, PublicationStage, PublishedCandidateValidation,
    SqliteProgrammaticActivationCommandStateStore, StructuredCancellationScope, WorkspaceId,
    WorkspaceRecord, WorkspaceSlot, activate_source_candidate, advance_source_generation,
    build_fresh_candidate, current_source_generation, step,
};
use crate::fabric::workspace_updates::{WorkspaceObservation, WorkspaceWatchControl};
use crate::resource_budget::{ResourceAmounts, ResourceClass};

pub(super) struct SourceUpdateOwner {
    pub hold_semantic_publication: bool,
    pub state_root: PathBuf,
    pub database: PathBuf,
    pub record: WorkspaceRecord,
    pub release: Arc<CompiledSemanticRelease>,
    pub slot: Arc<WorkspaceSlot>,
    pub resources: ProductionWorkspaceResources,
    pub runtime: Arc<FabricCommandRuntime>,
    pub activation: Arc<DeltaActivationRuntimeAuthority>,
    pub state_store: Arc<SqliteProgrammaticActivationCommandStateStore>,
    pub validation: Arc<PublishedCandidateValidation>,
    pub principal: PrincipalId,
    pub authorization: AuthorizationRef,
}

impl SourceUpdateOwner {
    pub(super) async fn start(
        self,
        observation: Arc<WorkspaceObservation>,
        receiver: mpsc::Receiver<()>,
        watch: WorkspaceWatchControl,
        parent: &StructuredCancellationScope,
    ) -> Result<(), ProductionWorkspaceStartupError> {
        let scope = parent
            .child_control("source-updates")
            .map_err(|error| step("source-update-scope", error))?;
        let guard = self
            .resources
            .budget()
            .try_reserve(
                ResourceClass::Control,
                ResourceAmounts {
                    running_jobs: 1,
                    memory_bytes: 256 * 1024,
                    ..ResourceAmounts::default()
                },
            )
            .map_err(|error| step("source-update-admission", error))?;
        let worker_scope = scope.clone();
        scope
            .spawn_async_owned(
                "coordinator",
                crate::cancellation::TaskCancellationMode::Cooperative,
                guard,
                Box::pin(self.run(observation, receiver, watch, worker_scope)),
            )
            .await
            .map_err(|error| step("source-update-start", error))?;
        Ok(())
    }

    async fn run(
        self,
        observation: Arc<WorkspaceObservation>,
        mut receiver: mpsc::Receiver<()>,
        watch: WorkspaceWatchControl,
        scope: StructuredCancellationScope,
    ) {
        let mut periodic = tokio::time::interval(Duration::from_secs(30));
        periodic.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        periodic.tick().await;
        let mut needs_recovery = false;
        loop {
            tokio::select! {
                () = scope.cancelled() => break,
                hint = receiver.recv() => if hint.is_none() { break; },
                _ = periodic.tick() => {
                    // Rebuild directory registrations as well as source state after lost hints.
                    watch.reinstall();
                    observation.request(true);
                }
            }
            match Box::pin(self.reconcile(&observation, &mut receiver, &scope, &mut needs_recovery))
                .await
            {
                Ok(true) => {}
                Ok(false) => {
                    observation.request(true);
                }
                Err(error) if error.source_changed => {
                    // An edit racing a secure capture is still pending work, not an outage.
                    observation.request(true);
                }
                Err(error) => {
                    if scope.is_cancelled() {
                        break;
                    }
                    tracing::warn!(workspace = %self.record.public_id(), %error, "source reconciliation unavailable");
                    observation.freshness.mark_unavailable();
                    observation.source_freshness.mark_unavailable();
                    watch.reinstall();
                    tokio::select! {
                        () = scope.cancelled() => break,
                        () = tokio::time::sleep(Duration::from_millis(500)) => {},
                        _ = receiver.recv() => {},
                    }
                    observation.request(true);
                }
            }
        }
    }

    async fn census(
        &self,
        observation: &WorkspaceObservation,
        scope: &StructuredCancellationScope,
    ) -> Result<Option<([u8; 32], u64, u64, u64)>, ProductionWorkspaceStartupError> {
        let database = self.database.clone();
        let record = self.record.clone();
        let budget = self.resources.budget().clone();
        let observed = observation.clone();
        let writer = Arc::clone(self.resources.operational_writer());
        let syntax_cache = Arc::clone(self.resources.syntax_cache());
        let pyrefly_cache = Arc::clone(self.resources.pyrefly_cache());
        let rust_toolchain_cache = Arc::clone(self.resources.rust_toolchain_cache());
        let guard = budget
            .try_reserve(
                ResourceClass::Control,
                ResourceAmounts {
                    running_jobs: 1,
                    memory_bytes: 128 * 1024,
                    ..ResourceAmounts::default()
                },
            )
            .map_err(|error| step("source-census-admission", error))?;
        scope
            .spawn_blocking_owned("census", guard, move |cancellation| {
                syntax_cache
                    .lock()
                    .map_err(|error| step("syntax-cache-owner", error))?
                    .evict_idle(Instant::now());
                // Census must not wait behind an active checker mutation. Idle maintenance
                // is opportunistic; source observation and obsolete-work cancellation proceed.
                match pyrefly_cache.try_lock() {
                    Ok(mut cache) => tokio::runtime::Handle::current()
                        .block_on(cache.evict_idle(Instant::now()))
                        .map_err(|error| step("pyrefly-idle-retirement", error))?,
                    Err(std::sync::TryLockError::WouldBlock) => {}
                    Err(error) => return Err(step("pyrefly-cache-owner", error)),
                }
                match rust_toolchain_cache.try_lock() {
                    Ok(mut cache) => cache.evict_idle(Instant::now()),
                    Err(std::sync::TryLockError::WouldBlock) => {}
                    Err(error) => return Err(step("rust-toolchain-cache-owner", error)),
                }
                let _writer = writer
                    .lock()
                    .map_err(|error| step("source-census-writer", error))?;
                let watermark = observed.freshness.requested();
                let events = observed.event_revision();
                let mut store = OperationalStore::open(&database)
                    .map_err(|error| step("source-census-store", error))?;
                let generation = current_source_generation(&store, record.workspace_id)
                    .map_err(|error| step("source-census-generation", error))?;
                let root = crate::secure_path::open_workspace_root(&mut store, record.workspace_id)
                    .map_err(|error| step("source-census-root", error))?;
                let inventory = crate::inventory::InventoryWalker::new_governed(
                    crate::inventory::InventoryLimits::default(),
                    budget,
                )
                .walk_selected_with_fence(
                    &root,
                    &mut store,
                    generation,
                    events,
                    &cancellation,
                    || Some(observed.event_revision()),
                );
                let inventory = match inventory {
                    Ok(inventory) => inventory,
                    Err(crate::inventory::InventoryError::SourceChanged) => return Ok(None),
                    Err(error) => return Err(step("source-census", error)),
                };
                Ok(Some((
                    inventory.inventory().digest,
                    generation,
                    watermark,
                    events,
                )))
            })
            .await
            .map_err(|error| step("source-census-start", error))?
            .wait()
            .await
            .map_err(|error| step("source-census-join", error))?
    }

    async fn advance_generation(
        &self,
        generation: u64,
        scope: &StructuredCancellationScope,
    ) -> Result<(), ProductionWorkspaceStartupError> {
        let database = self.database.clone();
        let workspace = self.record.workspace_id;
        let writer = Arc::clone(self.resources.operational_writer());
        let guard = self
            .resources
            .budget()
            .try_reserve(
                ResourceClass::Control,
                ResourceAmounts {
                    running_jobs: 1,
                    memory_bytes: 16 * 1024,
                    ..ResourceAmounts::default()
                },
            )
            .map_err(|error| step("source-generation-admission", error))?;
        scope
            .spawn_blocking_owned("advance-generation", guard, move |_| {
                let _writer = writer
                    .lock()
                    .map_err(|error| step("source-generation-writer", error))?;
                let mut store = OperationalStore::open(&database)
                    .map_err(|error| step("source-generation-store", error))?;
                advance_source_generation(&mut store, workspace, generation)
                    .map_err(|error| step("source-generation-advance", error))
            })
            .await
            .map_err(|error| step("source-generation-start", error))?
            .wait()
            .await
            .map_err(|error| step("source-generation-join", error))??;
        Ok(())
    }

    /// The enclosing static configuration rejects this seam in release builds. It remains
    /// deadline/cancellation bounded and never bypasses providers, activation or source fences.
    async fn assurance_pause(
        &self,
        generation: u64,
        scope: &StructuredCancellationScope,
    ) -> Result<(), ProductionWorkspaceStartupError> {
        if !cfg!(debug_assertions) {
            return Err(step("semantic-assurance", "debug build required"));
        }
        let marker = self.state_root.join(format!(
            "semantic-update-{}-{generation}",
            super::lower_hex(&self.record.workspace_id)
        ));
        let ready = marker.with_extension("ready");
        let resume = marker.with_extension("resume");
        tokio::fs::write(&ready, generation.to_be_bytes())
            .await
            .map_err(|error| step("semantic-assurance-ready", error))?;
        let result = tokio::time::timeout(Duration::from_secs(120), async {
            loop {
                if tokio::fs::try_exists(&resume)
                    .await
                    .map_err(|error| step("semantic-assurance-resume", error))?
                {
                    return Ok(());
                }
                tokio::select! {
                    () = scope.cancelled() => return Err(step("semantic-assurance", "cancelled")),
                    () = tokio::time::sleep(Duration::from_millis(20)) => {}
                }
            }
        })
        .await
        .map_err(|_| step("semantic-assurance", "deadline"));
        let _ = tokio::fs::remove_file(ready).await;
        let _ = tokio::fs::remove_file(resume).await;
        result?
    }

    async fn recover_publication(&self) -> Result<(), ProductionWorkspaceStartupError> {
        // Preserve the exact candidate/validation while a durable activation is unresolved.
        // A newer census must not replace that authority or submit another publication.
        let diagnostics = super::RelationalInterruptedCommitDiagnostics::new(
            WorkspaceId::from_bytes(self.record.workspace_id),
            Arc::new(super::FailClosedInterruptionDiagnostics),
        );
        let recovery = self
            .runtime
            .recover_and_open_bounded(
                super::CommandRecoveryPageSize::new(128).expect("bounded recovery page"),
                std::num::NonZeroUsize::new(8).expect("nonzero recovery sweeps"),
                &diagnostics,
            )
            .await
            .map_err(|error| step("source-publication-recovery", error))?;
        if !matches!(
            recovery.state(),
            crate::fabric::command_runtime::FabricCommandStartupRecoveryState::Ready
        ) {
            return Err(step(
                "source-publication-recovery",
                "durable selection remains unresolved",
            ));
        }
        Ok(())
    }

    async fn build_observed_candidate(
        &self,
        stage: PublicationStage,
        digest: [u8; 32],
        events: u64,
        observation: &WorkspaceObservation,
        receiver: &mut mpsc::Receiver<()>,
        scope: &StructuredCancellationScope,
    ) -> Result<Option<super::FreshCandidate>, ProductionWorkspaceStartupError> {
        let build_scope = scope
            .child("capture-and-publish")
            .map_err(|error| step("source-build-scope", error))?;
        // Keep the selected workspace lease throughout preparation. Reused exact tables remain
        // protected alongside existing readers until the successor's activation fence is checked.
        let selected = self
            .slot
            .lease()
            .map_err(|error| step("source-reuse-selected", error))?;
        let build = async {
            let fresh = build_fresh_candidate(
                &self.state_root,
                &self.database,
                &self.record,
                &self.release,
                self.runtime.fence(),
                super::PublicationWork {
                    resources: &self.resources,
                    scope: &build_scope,
                    stage,
                },
                Some(
                    selected
                        .workspace()
                        .runtime()
                        .query_authority()
                        .epoch()
                        .relation_publication()
                        .clone(),
                ),
            )
            .await?;
            if stage == PublicationStage::Semantic && self.hold_semantic_publication {
                self.assurance_pause(fresh.pins.source_generation.get(), &build_scope)
                    .await?;
            }
            Ok::<_, ProductionWorkspaceStartupError>(fresh)
        };
        tokio::pin!(build);
        loop {
            tokio::select! {
                result = &mut build => return result.map(Some),
                () = scope.cancelled() => { build_scope.cancel(); let _ = build.await; return Ok(None); },
                _ = receiver.recv() => {
                    if observation.event_revision() != events {
                        build_scope.cancel(); let _ = build.await; return Ok(None);
                    }
                    // A source-current query can request another census during compiler work.
                    // Refresh requests do not cancel or postpone the semantic successor.
                    let census = self.census(observation, scope).await;
                    match census {
                        Ok(Some((current, current_generation, watermark, checked_events)))
                            if current == digest && checked_events == events => {
                            if let Ok(selected) = self.slot.lease() {
                                let authority = selected.workspace().runtime().query_authority();
                                if authority.source_inventory_digest() == Some(current)
                                    && authority.activation_pins().source_generation.get() == current_generation {
                                    observation.source_reconciled(watermark, current_generation);
                                }
                            }
                        }
                        Ok(_) => { build_scope.cancel(); let _ = build.await; return Ok(None); }
                        Err(error) => { build_scope.cancel(); let _ = build.await; return Err(error); }
                    }
                }
            }
        }
    }

    async fn reconcile(
        &self,
        observation: &WorkspaceObservation,
        receiver: &mut mpsc::Receiver<()>,
        scope: &StructuredCancellationScope,
        needs_recovery: &mut bool,
    ) -> Result<bool, ProductionWorkspaceStartupError> {
        if *needs_recovery {
            self.recover_publication().await?;
            *needs_recovery = false;
        }
        let Some((digest, generation, watermark, events)) = self.census(observation, scope).await?
        else {
            return Ok(false);
        };
        let selected = self
            .slot
            .lease()
            .map_err(|error| step("source-update-selected", error))?;
        let authority = selected.workspace().runtime().query_authority();
        let unchanged = authority.source_inventory_digest() == Some(digest)
            && authority.activation_pins().source_generation.get() == generation;
        if unchanged {
            observation.source_reconciled(watermark, generation);
            if !authority.semantic_pending() {
                observation.reconciled(watermark, generation);
                return Ok(true);
            }
        }
        let mut expected_head = ExpectedHead::Epoch(selected.workspace().selection().epoch_id());
        drop(selected);
        let stages = if unchanged {
            // Reopen or retry of a durable source-only epoch must resume semantic work.
            &[PublicationStage::Semantic][..]
        } else {
            self.advance_generation(generation, scope).await?;
            &[PublicationStage::Source, PublicationStage::Semantic][..]
        };
        for &stage in stages {
            let Some(fresh) = Box::pin(self.build_observed_candidate(
                stage,
                digest,
                events,
                observation,
                receiver,
                scope,
            ))
            .await?
            else {
                return Ok(false);
            };
            let Some((current_digest, current_generation, checked_watermark, checked_events)) =
                self.census(observation, scope).await?
            else {
                return Ok(false);
            };
            let candidate = crate::fabric::workspace_updates::selected_inventory_state(
                &fresh.candidate,
                self.record.workspace_id,
                fresh.pins.source_generation.get(),
            )
            .await
            .map_err(|error| step("source-candidate-inventory", error))?;
            if candidate.is_none_or(|state| state.digest != current_digest)
                || current_digest != digest
                || fresh.pins.source_generation.get() != current_generation
                || checked_events != events
                || observation.event_revision() != checked_events
            {
                return Ok(false);
            }
            self.validation.replace_with(&fresh.validation);
            *needs_recovery = true;
            activate_source_candidate(
                &fresh,
                expected_head,
                WorkspaceId::from_bytes(self.record.workspace_id),
                self.principal,
                self.authorization,
                &self.runtime,
                &self.activation,
                &self.state_store,
                &self.release,
            )
            .await?;
            *needs_recovery = false;
            expected_head = ExpectedHead::Epoch(fresh.pins.epoch);
            match stage {
                PublicationStage::Source => {
                    observation.source_reconciled(checked_watermark, current_generation);
                }
                PublicationStage::Semantic => {
                    observation.reconciled(checked_watermark, current_generation);
                }
            }
        }
        Ok(true)
    }
}
