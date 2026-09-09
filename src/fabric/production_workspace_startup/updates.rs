//! One serialized workspace update owner reuses capture, providers and exact activation.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use super::{
    AuthorizationRef, CompiledSemanticRelease, DeltaActivationRuntimeAuthority, ExpectedHead,
    FabricCommandRuntime, OperationalStore, PrincipalId, ProductionWorkspaceResources,
    ProductionWorkspaceStartupError, PublishedCandidateValidation,
    SqliteProgrammaticActivationCommandStateStore, StructuredCancellationScope, WorkspaceId,
    WorkspaceRecord, WorkspaceSlot, activate_source_candidate, advance_source_generation,
    build_fresh_candidate, current_source_generation, step,
};
use crate::fabric::workspace_updates::{WorkspaceObservation, WorkspaceWatchControl};
use crate::resource_budget::{ResourceAmounts, ResourceClass};

pub(super) struct SourceUpdateOwner {
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
        loop {
            tokio::select! {
                () = scope.cancelled() => break,
                hint = receiver.recv() => if hint.is_none() { break; },
                _ = periodic.tick() => { observation.request(true); }
            }
            match Box::pin(self.reconcile(&observation, &mut receiver, &scope)).await {
                Ok(true) => {}
                Ok(false) => {
                    observation.request(true);
                }
                Err(error) => {
                    if scope.is_cancelled() {
                        break;
                    }
                    tracing::warn!(workspace = %self.record.public_id(), %error, "source reconciliation unavailable");
                    observation.freshness.mark_unavailable();
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

    async fn reconcile(
        &self,
        observation: &WorkspaceObservation,
        receiver: &mut mpsc::Receiver<()>,
        scope: &StructuredCancellationScope,
    ) -> Result<bool, ProductionWorkspaceStartupError> {
        let Some((digest, generation, watermark, events)) = self.census(observation, scope).await?
        else {
            return Ok(false);
        };
        let selected = self
            .slot
            .lease()
            .map_err(|error| step("source-update-selected", error))?;
        let authority = selected.workspace().runtime().query_authority();
        if authority.source_inventory_digest() == Some(digest)
            && authority.activation_pins().source_generation.get() == generation
        {
            observation.reconciled(
                watermark,
                authority.activation_pins().source_generation.get(),
            );
            return Ok(true);
        }
        let expected_head = ExpectedHead::Epoch(selected.workspace().selection().epoch_id());
        drop(selected);
        let workspace = self.record.workspace_id;
        self.advance_generation(generation, scope).await?;
        let build_scope = scope
            .child("capture-and-publish")
            .map_err(|error| step("source-build-scope", error))?;
        let build = build_fresh_candidate(
            &self.state_root,
            &self.database,
            &self.record,
            &self.release,
            self.runtime.fence(),
            &self.resources,
            &build_scope,
        );
        tokio::pin!(build);
        let fresh = loop {
            tokio::select! {
                result = &mut build => break result?,
                () = scope.cancelled() => { build_scope.cancel(); let _ = build.await; return Ok(false); },
                _ = receiver.recv() => {
                    if observation.event_revision() != events {
                        build_scope.cancel(); let _ = build.await; return Ok(false);
                    }
                }
            }
        };
        let Some((current_digest, _, checked_watermark, checked_events)) =
            self.census(observation, scope).await?
        else {
            return Ok(false);
        };
        let candidate_digest = crate::fabric::workspace_updates::selected_inventory_digest(
            &fresh.candidate,
            workspace,
            fresh.pins.source_generation.get(),
        )
        .await
        .map_err(|error| step("source-candidate-inventory", error))?;
        if candidate_digest != Some(current_digest)
            || observation.event_revision() != checked_events
        {
            return Ok(false);
        }
        self.validation.replace_with(&fresh.validation);
        activate_source_candidate(
            &fresh,
            expected_head,
            WorkspaceId::from_bytes(workspace),
            self.principal,
            self.authorization,
            &self.runtime,
            &self.activation,
            &self.state_store,
            &self.release,
        )
        .await?;
        observation.reconciled(checked_watermark, fresh.pins.source_generation.get());
        Ok(true)
    }
}
