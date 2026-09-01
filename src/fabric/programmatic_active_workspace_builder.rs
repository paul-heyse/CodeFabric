//! Release-owned reconstruction of one complete activation-selected workspace.
//!
//! Durable activation readback selects only an epoch identity, its complete reversible Delta
//! vector, and the associated control horizon. This module turns that record into executable
//! authority: every relation is reopened at its selected version in a fresh DataFusion session,
//! the compiled release executes and proves producer closure, the eight query programs are
//! rebuilt from compiled Rust definitions, and the complete query/Delta/command runtime is bound
//! before an [`ActiveWorkspace`] can enter the kernel slot.

use std::collections::BTreeSet;
use std::sync::{Arc, Weak};

use async_trait::async_trait;
use deltalake::DeltaTableBuilder;

use super::activation::ActivationChain;
use super::activation_control_delta::{
    ActivationControlDeltaProvider, DeltaActivationRuntimeAuthority,
};
use super::admission::FabricAdmissionRuntime;
use super::arrow_result_resource::ArrowResultResourceLimits;
use super::child_session::resource_governance::{EpochResourceCoordinator, EpochResourcePolicy};
use super::child_session::{ChildRegistryAllowlist, ChildTableGrant};
use super::derived_producer_closure::{ProducerClosureCancellation, ProducerClosureResourceBounds};
use super::epoch_runtime::FabricEpochRuntimeConfig;
use super::production_kernel::{
    ActiveWorkspace, CompiledSemanticRelease, SelectedEpochRecord, WorkspaceSlot,
};
use super::programmatic_activation_admission::{
    ActiveWorkspaceBuildError, ReleaseOwnedActiveWorkspaceBuilder,
};
use super::programmatic_delta_runtime::{ProgrammaticDeltaRuntime, ProgrammaticDeltaRuntimePorts};
use super::programmatic_epoch::{ProgrammaticFabricEpoch, ProgrammaticFabricEpochBuilder};
use super::programmatic_workspace::{
    ProgrammaticCommandRuntimePartsFactory, ProgrammaticWorkspaceRuntime,
    WorkspaceEpochQueryAuthority, programmatic_fabric_epoch_authority_pin,
};
use super::published_arrow_result::PublishedArrowResultRegistry;
use super::relational_query_runtime::{RelationalQueryAuthorization, RelationalQueryRuntime};
use super::request_owned_relation::RequestOwnedRelationLimits;
use super::writer_lease::DurableWriterGenerationPort;
use crate::production_query_recipe::ProductionSemanticQueryRecipeInput;
use crate::relational_semantic_query::EpochBoundSemanticIngressLimits;

/// Explicit operational bounds used while reconstructing released semantic authority.
///
/// No semantic relation, query form, schema, provider choice, program, or selected identity is
/// configurable here. Those are compiled into [`CompiledSemanticRelease`] or read from the exact
/// activation selection. The values below are bounded execution and retention policy only.
#[derive(Clone)]
pub(crate) struct ProductionActiveWorkspaceConfig {
    epoch_runtime: FabricEpochRuntimeConfig,
    producer_bounds: ProducerClosureResourceBounds,
    ingress_limits: EpochBoundSemanticIngressLimits,
    resource_policy: EpochResourcePolicy,
    request_relation_limits: RequestOwnedRelationLimits,
    result_limits: ArrowResultResourceLimits,
    maximum_output_rows: usize,
    result_lease_millis: u64,
}

impl ProductionActiveWorkspaceConfig {
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        epoch_runtime: FabricEpochRuntimeConfig,
        producer_bounds: ProducerClosureResourceBounds,
        ingress_limits: EpochBoundSemanticIngressLimits,
        resource_policy: EpochResourcePolicy,
        request_relation_limits: RequestOwnedRelationLimits,
        result_limits: ArrowResultResourceLimits,
        maximum_output_rows: usize,
        result_lease_millis: u64,
    ) -> Self {
        Self {
            epoch_runtime,
            producer_bounds,
            ingress_limits,
            resource_policy,
            request_relation_limits,
            result_limits,
            maximum_output_rows,
            result_lease_millis,
        }
    }
}

/// Concrete target-only builder shared by activation and clean restart.
pub(crate) struct ProductionActiveWorkspaceBuilder {
    release: CompiledSemanticRelease,
    config: ProductionActiveWorkspaceConfig,
    admission: Arc<FabricAdmissionRuntime>,
    published_results: Arc<PublishedArrowResultRegistry>,
    delta_ports: ProgrammaticDeltaRuntimePorts,
    writer_generations: Arc<dyn DurableWriterGenerationPort>,
    workspace_slot: Weak<WorkspaceSlot>,
    command_factory: Arc<dyn ProgrammaticCommandRuntimePartsFactory>,
}

impl ProductionActiveWorkspaceBuilder {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        release: CompiledSemanticRelease,
        config: ProductionActiveWorkspaceConfig,
        admission: Arc<FabricAdmissionRuntime>,
        published_results: Arc<PublishedArrowResultRegistry>,
        delta_ports: ProgrammaticDeltaRuntimePorts,
        writer_generations: Arc<dyn DurableWriterGenerationPort>,
        workspace_slot: Weak<WorkspaceSlot>,
        command_factory: Arc<dyn ProgrammaticCommandRuntimePartsFactory>,
    ) -> Self {
        Self {
            release,
            config,
            admission,
            published_results,
            delta_ports,
            writer_generations,
            workspace_slot,
            command_factory,
        }
    }

    fn validate_selection(
        selection: &SelectedEpochRecord,
        chain: &ActivationChain,
        epoch: &ProgrammaticFabricEpoch,
    ) -> Result<(), ActiveWorkspaceBuildError> {
        if chain.workspace_id() != selection.workspace_id()
            || chain.head_event().copied() != Some(selection.event())
            || chain.current_head() != super::command::ExpectedHead::Epoch(selection.epoch_id())
            || epoch.identity() != &selection.epoch_id()
            || epoch.table_version_set_ref() != selection.table_versions().reference()
            || selection.control_horizon().workspace_id() != selection.workspace_id()
            || selection.proof_reference() != selection.event().pins().proof_receipt
        {
            return Err(ActiveWorkspaceBuildError::Invalid);
        }
        Ok(())
    }

    async fn compose(
        &self,
        selection: SelectedEpochRecord,
        chain: &ActivationChain,
        epoch: Arc<ProgrammaticFabricEpoch>,
    ) -> Result<Arc<ActiveWorkspace>, ActiveWorkspaceBuildError> {
        Self::validate_selection(&selection, chain, &epoch)?;
        let pins = selection.event().pins();
        let cancellation = ProducerClosureCancellation::new();
        let proved = self
            .release
            .prove_producer_closure(&epoch, self.config.producer_bounds, &cancellation)
            .await
            .map_err(|_| ActiveWorkspaceBuildError::Invalid)?;
        let query_input = ProductionSemanticQueryRecipeInput::try_new(
            *pins.source_authority.as_bytes(),
            *pins.policy_set.as_bytes(),
            self.config.ingress_limits,
        )
        .map_err(|_| ActiveWorkspaceBuildError::Invalid)?;
        let recipe = self
            .release
            .compile_semantic_query_recipe(&epoch, query_input, proved.execution())
            .map_err(|_| ActiveWorkspaceBuildError::Invalid)?;

        let table_relations = epoch.relation_ids().cloned().collect::<BTreeSet<_>>();
        self.release
            .compose_semantic_query_ports(
                &recipe,
                self.config.ingress_limits,
                *pins.policy_set.as_bytes(),
                table_relations.clone(),
                self.config.maximum_output_rows,
            )
            .map_err(|_| ActiveWorkspaceBuildError::Invalid)?;

        let resources = Arc::new(
            EpochResourceCoordinator::try_new(
                selection.epoch_id(),
                *pins.resource_envelope.as_bytes(),
                self.config.resource_policy.clone(),
            )
            .map_err(|_| ActiveWorkspaceBuildError::Invalid)?,
        );
        let grants = table_relations
            .into_iter()
            .map(ChildTableGrant::try_new)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ActiveWorkspaceBuildError::Invalid)?;
        let authorization = RelationalQueryAuthorization::try_new(
            programmatic_fabric_epoch_authority_pin(&epoch),
            *pins.policy_set.as_bytes(),
            *pins.resource_envelope.as_bytes(),
            grants,
            self.config.resource_policy.datafusion_resources().clone(),
            self.config.maximum_output_rows,
            ChildRegistryAllowlist::default(),
        )
        .map_err(|_| ActiveWorkspaceBuildError::Invalid)?;
        let query_authority = Arc::new(
            WorkspaceEpochQueryAuthority::try_new(
                selection.workspace_id(),
                pins,
                Arc::clone(&epoch),
                Arc::clone(&resources),
                Arc::clone(recipe.ingress_catalog()),
                Arc::clone(recipe.execution_catalog()),
                Arc::clone(recipe.producer_closure()),
                authorization,
                self.config.request_relation_limits,
                self.config.result_limits,
                self.config.result_lease_millis,
            )
            .map_err(|_| ActiveWorkspaceBuildError::Invalid)?,
        );
        let query_runtime = Arc::new(RelationalQueryRuntime::new(
            selection.workspace_id(),
            Arc::clone(&self.admission),
            Arc::clone(&self.published_results),
            Arc::clone(&resources),
        ));
        let delta_runtime = Arc::new(
            ProgrammaticDeltaRuntime::try_new(
                &self.release,
                &selection,
                &epoch,
                self.delta_ports.clone(),
            )
            .map_err(|_| ActiveWorkspaceBuildError::Invalid)?,
        );

        let control_pin = selection
            .control_horizon()
            .control_relation()
            .table()
            .clone();
        let control_table = DeltaTableBuilder::from_url(control_pin.canonical_root().clone())
            .map_err(|_| ActiveWorkspaceBuildError::Invalid)?
            .with_version(control_pin.version())
            .load()
            .await
            .map_err(|_| ActiveWorkspaceBuildError::Invalid)?;
        let control = Arc::new(
            ActivationControlDeltaProvider::try_from_loaded_table(
                Arc::new(epoch.context().state()),
                control_pin,
                control_table,
            )
            .await
            .map_err(|_| ActiveWorkspaceBuildError::Invalid)?,
        );
        let activation_authority = Arc::new(DeltaActivationRuntimeAuthority::new(
            selection.workspace_id(),
            control,
            Arc::clone(&self.writer_generations),
        ));
        let runtime = Arc::new(
            ProgrammaticWorkspaceRuntime::try_from_selected(
                &self.release,
                &selection,
                Arc::clone(&self.admission),
                Arc::clone(&self.published_results),
                query_authority,
                query_runtime,
                delta_runtime,
                activation_authority,
                self.workspace_slot.clone(),
                self.command_factory.as_ref(),
            )
            .map_err(|_| ActiveWorkspaceBuildError::Invalid)?,
        );
        Ok(Arc::new(
            ActiveWorkspace::try_new(selection, runtime)
                .map_err(|_| ActiveWorkspaceBuildError::Invalid)?,
        ))
    }
}

#[async_trait]
impl ReleaseOwnedActiveWorkspaceBuilder for ProductionActiveWorkspaceBuilder {
    async fn build_activated(
        &self,
        selection: SelectedEpochRecord,
        chain_after_readback: &ActivationChain,
        candidate: Arc<ProgrammaticFabricEpoch>,
    ) -> Result<Arc<ActiveWorkspace>, ActiveWorkspaceBuildError> {
        self.compose(selection, chain_after_readback, candidate)
            .await
    }

    async fn rebuild_selected(
        &self,
        selection: SelectedEpochRecord,
        chain_after_readback: &ActivationChain,
    ) -> Result<Arc<ActiveWorkspace>, ActiveWorkspaceBuildError> {
        let epoch = ProgrammaticFabricEpochBuilder::try_new(
            selection.epoch_id(),
            self.config.epoch_runtime.clone(),
        )
        .map_err(|_| ActiveWorkspaceBuildError::Invalid)?
        .reopen(Arc::clone(selection.table_versions()))
        .await
        .map_err(|_| ActiveWorkspaceBuildError::Invalid)?;
        self.compose(selection, chain_after_readback, Arc::new(epoch))
            .await
    }
}
