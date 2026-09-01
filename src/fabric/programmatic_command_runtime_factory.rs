//! Exact post-authority composition of one production fabric-command runtime.
//!
//! The workspace composition root calls this factory only after it has reconstructed the exact
//! activation selection, reopened the selected epoch, installed the epoch resource coordinator,
//! and registered the epoch query authority. This module binds the already explicit durable
//! runtime configuration and complete production port closure to those installed authorities.
//! It does not infer paths, manufacture semantic state, install test probes, or provide a
//! fallback effect implementation.

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

use crate::forward_cutover_controller::ProductionForwardCutoverBinding;

use super::activation::ActivationEventId;
use super::activation_command_effect::ActivationCommandEffect;
use super::activation_transaction::{
    ActivationCacheReceipt, ActivationEpochRebuildOutcome, ActivationEpochRebuildRequest,
    ActivationEpochRebuilderPort, ActivationRecoveryCoordinator, ActivationTransactionCoordinator,
    IdempotentActivationAcknowledgements,
};
use super::command::{EpochId, ExpectedHead, WorkspaceId, WriterFence};
use super::command_effect_router::FabricCommandEffectRouter;
use super::command_effect_router::{
    AdministrationCommandEffectPort, CompactionCommandEffectPort,
    RelationPublicationCommandEffectPort, RetentionCommandEffectPort, RollbackCommandEffectPort,
    SourceWaveCommandEffectPort,
};
use super::command_runtime::{
    CommandSemanticContextPort, FabricCommandRuntimeConfig, InterruptedCommitDiagnosticPort,
};
use super::command_runtime_manager::{
    WorkspaceFabricCommandRuntimeFactoryError, WorkspaceFabricCommandRuntimeParts,
};
use super::command_runtime_ports::{
    CommandAuthorizationPort, InterruptedCommitDiagnosticRelationPort,
    RelationalCommandSemanticContext, RelationalInterruptedCommitDiagnostics,
};
use super::programmatic_activation_admission::{
    ProgrammaticActivationAdmission, ProgrammaticSuccessorQueryAuthorityPort,
};
use super::programmatic_activation_command_ports::{
    ActivationCandidateProofRelationsPort, ActivationCommandStateStore,
    ExactActivationCandidateProof, ExactActivationCommandState,
};
use super::programmatic_command_capability::{
    ProgrammaticAdministrationCapabilityGap, ProgrammaticCommandCapabilityDisposition,
    ProgrammaticCommandCapabilityError, ProgrammaticCompactionCapabilityGap,
    ProgrammaticRelationPublicationCapabilityGap, ProgrammaticRetentionCapabilityGap,
    ProgrammaticRollbackCapabilityGap, ProgrammaticSourceWaveCapabilityGap,
};
use super::programmatic_delta_maintenance_command::ProgrammaticDeltaMaintenanceAdministrationPorts;
use super::programmatic_workspace::{
    ProgrammaticCommandRuntimeContext, ProgrammaticCommandRuntimePartsFactory,
    WorkspaceEpochQueryAuthorityRegistryError,
};

/// Exact installed authority identities which one command-runtime closure must retain.
///
/// These values come from the same explicit workspace construction inputs as the activation and
/// epoch authorities. They are checked against live, already-installed objects when
/// [`ProgrammaticCommandRuntimePartsFactory::build`] runs; they are not observations that can
/// select a different epoch or discover a latest table version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgrammaticCommandRuntimeAuthorityBinding {
    workspace_id: WorkspaceId,
    epoch_id: EpochId,
    activation_event_id: ActivationEventId,
    activation_fence: WriterFence,
    activation_control_fingerprint: [u8; 32],
    resource_policy_pin: [u8; 32],
}

impl ProgrammaticCommandRuntimeAuthorityBinding {
    /// Bind the exact workspace, epoch, activation, and resource identities expected at the
    /// post-authority factory boundary.
    #[must_use]
    pub const fn new(
        workspace_id: WorkspaceId,
        epoch_id: EpochId,
        activation_event_id: ActivationEventId,
        activation_fence: WriterFence,
        activation_control_fingerprint: [u8; 32],
        resource_policy_pin: [u8; 32],
    ) -> Self {
        Self {
            workspace_id,
            epoch_id,
            activation_event_id,
            activation_fence,
            activation_control_fingerprint,
            resource_policy_pin,
        }
    }

    #[must_use]
    pub const fn workspace_id(self) -> WorkspaceId {
        self.workspace_id
    }

    #[must_use]
    pub const fn epoch_id(self) -> EpochId {
        self.epoch_id
    }

    #[must_use]
    pub const fn activation_event_id(self) -> ActivationEventId {
        self.activation_event_id
    }

    #[must_use]
    pub const fn activation_fence(self) -> WriterFence {
        self.activation_fence
    }

    #[must_use]
    pub const fn activation_control_fingerprint(self) -> [u8; 32] {
        self.activation_control_fingerprint
    }

    #[must_use]
    pub const fn resource_policy_pin(self) -> [u8; 32] {
        self.resource_policy_pin
    }
}

/// Fail-closed errors while binding a complete command runtime to installed workspace authority.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ProgrammaticCommandRuntimeFactoryError {
    #[error("command runtime configuration belongs to another workspace")]
    ConfigWorkspaceMismatch,
    #[error("command runtime factory received another workspace context")]
    ContextWorkspaceMismatch,
    #[error("activation authority belongs to another workspace")]
    ActivationWorkspaceMismatch,
    #[error("query admission is not pinned to the configured epoch")]
    AdmissionEpochMismatch,
    #[error("resource coordinator is not pinned to the configured epoch")]
    ResourceEpochMismatch,
    #[error("resource coordinator policy does not match the configured policy pin")]
    ResourcePolicyMismatch,
    #[error("activation control relation does not match the configured exact binding")]
    ActivationControlMismatch,
    #[error("query authority registry cannot resolve the configured epoch: {0}")]
    QueryAuthority(WorkspaceEpochQueryAuthorityRegistryError),
    #[error("resolved query authority belongs to another workspace")]
    QueryAuthorityWorkspaceMismatch,
    #[error("resolved query authority is not pinned to the configured epoch")]
    QueryAuthorityEpochMismatch,
    #[error("resolved query authority does not share the installed resource coordinator")]
    QueryAuthorityResourceMismatch,
    #[error("resolved query authority has another resource policy")]
    QueryAuthorityResourcePolicyMismatch,
    #[error("query runtime belongs to another workspace")]
    QueryRuntimeWorkspaceMismatch,
    #[error("query runtime does not share the installed admission authority")]
    QueryRuntimeAdmissionMismatch,
    #[error("query runtime does not share the installed resource coordinator")]
    QueryRuntimeResourceMismatch,
    #[error("query runtime does not share the daemon published-result registry")]
    QueryRuntimePublishedResultsMismatch,
    #[error("Delta runtime belongs to another workspace")]
    DeltaRuntimeWorkspaceMismatch,
    #[error("Delta runtime is not pinned to the configured epoch")]
    DeltaRuntimeEpochMismatch,
    #[error("Delta runtime table vector differs from the activation-selected exact vector")]
    DeltaRuntimeTableVersionsMismatch,
    #[error("activation reconciliation receipt cache is unavailable")]
    ReceiptCacheUnavailable,
    #[error("activation reconciliation receipt is absent")]
    ReceiptMissing,
    #[error("activation reconciliation receipt belongs to another workspace")]
    ReceiptWorkspaceMismatch,
    #[error("activation reconciliation receipt is not pinned to the configured epoch")]
    ReceiptEpochMismatch,
    #[error("activation reconciliation receipt names another activation event")]
    ReceiptEventMismatch,
    #[error("activation reconciliation receipt names another active writer fence")]
    ReceiptFenceMismatch,
}

/// One explicit non-activation command-family disposition and its persisted diagnostic identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgrammaticCommandCapabilityGapInput {
    disposition: ProgrammaticCommandCapabilityDisposition,
    diagnostic: super::command::DiagnosticRef,
}

impl ProgrammaticCommandCapabilityGapInput {
    #[must_use]
    pub const fn new(
        disposition: ProgrammaticCommandCapabilityDisposition,
        diagnostic: super::command::DiagnosticRef,
    ) -> Self {
        Self {
            disposition,
            diagnostic,
        }
    }
}

/// Exhaustive production capability-gap closure for command families not installed by a release.
#[derive(Clone)]
pub struct ProgrammaticNonActivationCommandEffects {
    source_wave: Arc<dyn SourceWaveCommandEffectPort>,
    relation_publication: Arc<dyn RelationPublicationCommandEffectPort>,
    rollback: Arc<dyn RollbackCommandEffectPort>,
    compaction: Arc<dyn CompactionCommandEffectPort>,
    retention: Arc<dyn RetentionCommandEffectPort>,
    administration: Arc<dyn AdministrationCommandEffectPort>,
}

impl fmt::Debug for ProgrammaticNonActivationCommandEffects {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProgrammaticNonActivationCommandEffects")
            .field("source_wave", &"installed")
            .field("relation_publication", &"installed")
            .field("rollback", &"installed")
            .field("compaction", &"installed")
            .field("retention", &"installed")
            .field("administration", &"installed")
            .finish()
    }
}

/// Failure to bind an explicit capability disposition to its durable diagnostic relation row.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("invalid {family} command capability gap: {source}")]
pub struct ProgrammaticNonActivationCommandEffectsError {
    family: &'static str,
    source: ProgrammaticCommandCapabilityError,
}

impl ProgrammaticNonActivationCommandEffects {
    /// Install the complete target-owned non-activation command closure.
    ///
    /// This is the production route for concrete source-wave, publication, rollback,
    /// compaction, retention, and administration effects. Capability-gap effects remain an
    /// explicit release choice through [`Self::try_new`]; the runtime factory no longer forces
    /// compaction, retention, and administration to be unavailable.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn installed(
        source_wave: Arc<dyn SourceWaveCommandEffectPort>,
        relation_publication: Arc<dyn RelationPublicationCommandEffectPort>,
        rollback: Arc<dyn RollbackCommandEffectPort>,
        compaction: Arc<dyn CompactionCommandEffectPort>,
        retention: Arc<dyn RetentionCommandEffectPort>,
        administration: Arc<dyn AdministrationCommandEffectPort>,
    ) -> Self {
        Self {
            source_wave,
            relation_publication,
            rollback,
            compaction,
            retention,
            administration,
        }
    }

    /// Construct the complete six-family non-activation closure.
    ///
    /// Every family is a named input. A missing/zero diagnostic fails construction rather than
    /// installing an empty handler or deferring the error until command execution.
    pub fn try_new(
        source_wave: ProgrammaticCommandCapabilityGapInput,
        relation_publication: ProgrammaticCommandCapabilityGapInput,
        rollback: ProgrammaticCommandCapabilityGapInput,
        compaction: ProgrammaticCommandCapabilityGapInput,
        retention: ProgrammaticCommandCapabilityGapInput,
        administration: ProgrammaticCommandCapabilityGapInput,
    ) -> Result<Self, ProgrammaticNonActivationCommandEffectsError> {
        macro_rules! gap {
            ($family:literal, $constructor:ty, $input:expr) => {
                <$constructor>::try_new($input.disposition, $input.diagnostic).map_err(
                    |source| ProgrammaticNonActivationCommandEffectsError {
                        family: $family,
                        source,
                    },
                )?
            };
        }
        Ok(Self::installed(
            Arc::new(gap!(
                "source-wave",
                ProgrammaticSourceWaveCapabilityGap,
                source_wave
            )),
            Arc::new(gap!(
                "relation-publication",
                ProgrammaticRelationPublicationCapabilityGap,
                relation_publication
            )),
            Arc::new(gap!(
                "rollback",
                ProgrammaticRollbackCapabilityGap,
                rollback
            )),
            Arc::new(gap!(
                "compaction",
                ProgrammaticCompactionCapabilityGap,
                compaction
            )),
            Arc::new(gap!(
                "retention",
                ProgrammaticRetentionCapabilityGap,
                retention
            )),
            Arc::new(gap!(
                "administration",
                ProgrammaticAdministrationCapabilityGap,
                administration
            )),
        ))
    }
}

/// Production activation dependencies which do not belong to one process-local workspace context.
#[derive(Clone)]
pub struct ProgrammaticActivationCommandEffects {
    state: Arc<ExactActivationCommandState>,
    proof: Arc<ExactActivationCandidateProof>,
    epoch_rebuilder: Arc<dyn ActivationEpochRebuilderPort>,
    successor_query_authority: Arc<dyn ProgrammaticSuccessorQueryAuthorityPort>,
}

impl fmt::Debug for ProgrammaticActivationCommandEffects {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProgrammaticActivationCommandEffects")
            .field("state", &"exact-store-adapter")
            .field("proof", &"exact-proof-relation-adapter")
            .field("epoch_rebuilder", &"installed")
            .field("successor_query_authority", &"installed")
            .finish_non_exhaustive()
    }
}

impl ProgrammaticActivationCommandEffects {
    #[must_use]
    pub fn new(
        state_store: Arc<dyn ActivationCommandStateStore>,
        proof_relations: Arc<dyn ActivationCandidateProofRelationsPort>,
        proof_integrity_diagnostic: super::command::DiagnosticRef,
        epoch_rebuilder: Arc<dyn ActivationEpochRebuilderPort>,
        successor_query_authority: Arc<dyn ProgrammaticSuccessorQueryAuthorityPort>,
    ) -> Self {
        Self {
            state: Arc::new(ExactActivationCommandState::new(state_store)),
            proof: Arc::new(ExactActivationCandidateProof::new(
                proof_relations,
                proof_integrity_diagnostic,
            )),
            epoch_rebuilder,
            successor_query_authority,
        }
    }
}

/// Exhaustive effect closure which becomes a router only against the exact live workspace context.
#[derive(Clone, Debug)]
pub struct ExactProgrammaticCommandEffectClosure {
    activation: ProgrammaticActivationCommandEffects,
    unavailable: ProgrammaticNonActivationCommandEffects,
    delta_maintenance_administration: Option<ProgrammaticDeltaMaintenanceAdministrationPorts>,
    forward_cutover: Option<ProductionForwardCutoverBinding>,
}

impl ExactProgrammaticCommandEffectClosure {
    #[must_use]
    pub const fn new(
        activation: ProgrammaticActivationCommandEffects,
        unavailable: ProgrammaticNonActivationCommandEffects,
    ) -> Self {
        Self {
            activation,
            unavailable,
            delta_maintenance_administration: None,
            forward_cutover: None,
        }
    }

    /// Replace the administration-family disposition with the target-owned guarded Delta
    /// maintenance adapter once the live exact-version runtime is available during composition.
    ///
    /// Only the adapter's read-only maintenance subset becomes executable. Unsupported native
    /// checkpoint/optimize/destructive-vacuum actions still produce typed known failures.
    #[must_use]
    pub fn with_delta_maintenance_administration(
        mut self,
        ports: ProgrammaticDeltaMaintenanceAdministrationPorts,
    ) -> Self {
        self.delta_maintenance_administration = Some(ports);
        self
    }

    /// Wrap the installed administration family with the exact forward-cutover command effect.
    /// Non-cutover administrative actions continue to delegate to the existing effect.
    #[must_use]
    pub fn with_forward_cutover(mut self, binding: ProductionForwardCutoverBinding) -> Self {
        self.forward_cutover = Some(binding);
        self
    }

    fn build(&self, context: &ProgrammaticCommandRuntimeContext) -> Arc<FabricCommandEffectRouter> {
        let acknowledgements = Arc::new(IdempotentActivationAcknowledgements::new(
            context.workspace_id(),
        ));
        let admission = Arc::new(ProgrammaticActivationAdmission::new(
            context.workspace_id(),
            context.admission().clone(),
            context.query_authorities().clone(),
            Arc::clone(&self.activation.successor_query_authority),
        ));
        let commit = Arc::new(ActivationTransactionCoordinator::new(
            Arc::clone(&admission),
            Arc::clone(&self.activation.proof),
            context.activation_authority().clone(),
            context.activation_authority().clone(),
            context.receipt_cache().clone(),
            Arc::clone(&acknowledgements),
        ));
        let recovery = Arc::new(ActivationRecoveryCoordinator::new(
            admission,
            context.activation_authority().clone(),
            Arc::new(SharedActivationEpochRebuilder(Arc::clone(
                &self.activation.epoch_rebuilder,
            ))),
            context.receipt_cache().clone(),
            acknowledgements,
        ));
        let activation = Arc::new(ActivationCommandEffect::new(
            self.activation.state.clone(),
            commit,
            recovery,
        ));
        let administration: Arc<dyn AdministrationCommandEffectPort> =
            self.delta_maintenance_administration.as_ref().map_or_else(
                || Arc::clone(&self.unavailable.administration),
                |ports| ports.build(Arc::clone(context.delta_runtime())),
            );
        let administration = match &self.forward_cutover {
            Some(binding) => binding.wrap_administration(context, administration),
            None => administration,
        };
        Arc::new(FabricCommandEffectRouter::new(
            Arc::clone(&self.unavailable.source_wave),
            Arc::clone(&self.unavailable.relation_publication),
            activation,
            Arc::clone(&self.unavailable.rollback),
            Arc::clone(&self.unavailable.compaction),
            Arc::clone(&self.unavailable.retention),
            administration,
        ))
    }
}

#[derive(Clone)]
struct SharedActivationEpochRebuilder(Arc<dyn ActivationEpochRebuilderPort>);

#[async_trait]
impl ActivationEpochRebuilderPort for SharedActivationEpochRebuilder {
    async fn rebuild_selected_epoch(
        &self,
        request: ActivationEpochRebuildRequest,
    ) -> ActivationEpochRebuildOutcome {
        self.0.rebuild_selected_epoch(request).await
    }
}

/// Concrete production factory for one complete workspace command-runtime closure.
///
/// Every dependency is required and non-optional. In particular, the effect closure can become a
/// [`FabricCommandEffectRouter`] only after it receives the live workspace context, and its
/// constructor exhaustively requires every closed command family. The test-probe effect path in
/// the lower-level runtime parts is not expressible here.
#[derive(Clone)]
pub struct ExactProgrammaticCommandRuntimePartsFactory {
    config: FabricCommandRuntimeConfig,
    authority: ProgrammaticCommandRuntimeAuthorityBinding,
    authorization: Arc<dyn CommandAuthorizationPort>,
    effects: ExactProgrammaticCommandEffectClosure,
    interruption_diagnostics: Arc<RelationalInterruptedCommitDiagnostics>,
}

impl fmt::Debug for ExactProgrammaticCommandRuntimePartsFactory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactProgrammaticCommandRuntimePartsFactory")
            .field("workspace_id", &self.config.workspace_id)
            .field("epoch_id", &self.authority.epoch_id)
            .field("semantics", &"context-activation-bound")
            .field("effects", &"complete-production-router")
            .field("interruption_diagnostics", &"installed")
            .finish_non_exhaustive()
    }
}

impl ExactProgrammaticCommandRuntimePartsFactory {
    /// Install one explicit durable runtime configuration and its complete production ports.
    ///
    /// No dependency has a default, optional, dynamic-registry, or test-probe representation.
    #[must_use]
    pub fn new(
        config: FabricCommandRuntimeConfig,
        authority: ProgrammaticCommandRuntimeAuthorityBinding,
        authorization: Arc<dyn CommandAuthorizationPort>,
        effects: ExactProgrammaticCommandEffectClosure,
        interruption_diagnostic_relation: Arc<dyn InterruptedCommitDiagnosticRelationPort>,
    ) -> Self {
        let workspace_id = authority.workspace_id;
        Self {
            config,
            authority,
            authorization,
            effects,
            interruption_diagnostics: Arc::new(RelationalInterruptedCommitDiagnostics::new(
                workspace_id,
                interruption_diagnostic_relation,
            )),
        }
    }

    fn validate_workspace_binding(
        &self,
        context_workspace_id: WorkspaceId,
    ) -> Result<(), ProgrammaticCommandRuntimeFactoryError> {
        if self.config.workspace_id != self.authority.workspace_id {
            return Err(ProgrammaticCommandRuntimeFactoryError::ConfigWorkspaceMismatch);
        }
        if context_workspace_id != self.authority.workspace_id {
            return Err(ProgrammaticCommandRuntimeFactoryError::ContextWorkspaceMismatch);
        }
        Ok(())
    }

    fn validate_observation(
        &self,
        observation: &ProgrammaticCommandRuntimeAuthorityObservation,
    ) -> Result<(), ProgrammaticCommandRuntimeFactoryError> {
        let expected = self.authority;
        self.validate_workspace_binding(observation.context_workspace_id)?;
        if observation.activation_workspace_id != expected.workspace_id {
            return Err(ProgrammaticCommandRuntimeFactoryError::ActivationWorkspaceMismatch);
        }
        if observation.admission_head != ExpectedHead::Epoch(expected.epoch_id) {
            return Err(ProgrammaticCommandRuntimeFactoryError::AdmissionEpochMismatch);
        }
        if observation.resource_epoch_id != expected.epoch_id {
            return Err(ProgrammaticCommandRuntimeFactoryError::ResourceEpochMismatch);
        }
        if observation.resource_policy_pin != expected.resource_policy_pin {
            return Err(ProgrammaticCommandRuntimeFactoryError::ResourcePolicyMismatch);
        }
        if observation.activation_control_fingerprint != expected.activation_control_fingerprint {
            return Err(ProgrammaticCommandRuntimeFactoryError::ActivationControlMismatch);
        }
        if observation.query_authority_workspace_id != expected.workspace_id {
            return Err(ProgrammaticCommandRuntimeFactoryError::QueryAuthorityWorkspaceMismatch);
        }
        if observation.query_authority_epoch_id != expected.epoch_id {
            return Err(ProgrammaticCommandRuntimeFactoryError::QueryAuthorityEpochMismatch);
        }
        if !observation.query_authority_shares_resources {
            return Err(ProgrammaticCommandRuntimeFactoryError::QueryAuthorityResourceMismatch);
        }
        if observation.query_authority_resource_policy_pin != expected.resource_policy_pin {
            return Err(
                ProgrammaticCommandRuntimeFactoryError::QueryAuthorityResourcePolicyMismatch,
            );
        }
        if observation.query_runtime_workspace_id != expected.workspace_id {
            return Err(ProgrammaticCommandRuntimeFactoryError::QueryRuntimeWorkspaceMismatch);
        }
        if !observation.query_runtime_shares_admission {
            return Err(ProgrammaticCommandRuntimeFactoryError::QueryRuntimeAdmissionMismatch);
        }
        if !observation.query_runtime_shares_resources {
            return Err(ProgrammaticCommandRuntimeFactoryError::QueryRuntimeResourceMismatch);
        }
        if !observation.query_runtime_shares_published_results {
            return Err(
                ProgrammaticCommandRuntimeFactoryError::QueryRuntimePublishedResultsMismatch,
            );
        }
        if observation.delta_runtime_workspace_id != expected.workspace_id {
            return Err(ProgrammaticCommandRuntimeFactoryError::DeltaRuntimeWorkspaceMismatch);
        }
        if observation.delta_runtime_epoch_id != expected.epoch_id {
            return Err(ProgrammaticCommandRuntimeFactoryError::DeltaRuntimeEpochMismatch);
        }
        if observation.delta_runtime_table_versions != observation.query_authority_table_versions {
            return Err(ProgrammaticCommandRuntimeFactoryError::DeltaRuntimeTableVersionsMismatch);
        }
        let receipt = observation
            .receipt
            .ok_or(ProgrammaticCommandRuntimeFactoryError::ReceiptMissing)?;
        if receipt.workspace_id != expected.workspace_id {
            return Err(ProgrammaticCommandRuntimeFactoryError::ReceiptWorkspaceMismatch);
        }
        if receipt.selected_epoch != expected.epoch_id {
            return Err(ProgrammaticCommandRuntimeFactoryError::ReceiptEpochMismatch);
        }
        if receipt.event_id != expected.activation_event_id {
            return Err(ProgrammaticCommandRuntimeFactoryError::ReceiptEventMismatch);
        }
        if receipt.active_fence != expected.activation_fence {
            return Err(ProgrammaticCommandRuntimeFactoryError::ReceiptFenceMismatch);
        }
        Ok(())
    }

    fn validate_authority_view(
        &self,
        view: &impl ProgrammaticCommandRuntimeAuthorityView,
    ) -> Result<(), ProgrammaticCommandRuntimeFactoryError> {
        self.validate_workspace_binding(view.context_workspace_id())?;
        let observation = view.observe(self.authority.epoch_id)?;
        self.validate_observation(&observation)
    }
}

impl ProgrammaticCommandRuntimePartsFactory for ExactProgrammaticCommandRuntimePartsFactory {
    fn build(
        &self,
        context: ProgrammaticCommandRuntimeContext,
    ) -> Result<WorkspaceFabricCommandRuntimeParts, WorkspaceFabricCommandRuntimeFactoryError> {
        self.validate_authority_view(&LiveProgrammaticCommandRuntimeAuthorityView(&context))
            .map_err(|error| Box::new(error) as WorkspaceFabricCommandRuntimeFactoryError)?;
        let semantics: Arc<dyn CommandSemanticContextPort> =
            Arc::new(RelationalCommandSemanticContext::new(
                self.authority.workspace_id,
                context.activation_authority().clone(),
                Arc::clone(&self.authorization),
            ));
        let interruption_diagnostics: Arc<dyn InterruptedCommitDiagnosticPort> =
            self.interruption_diagnostics.clone();
        Ok(WorkspaceFabricCommandRuntimeParts::new(
            self.config.clone(),
            semantics,
            self.effects.build(&context),
            interruption_diagnostics,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProgrammaticCommandRuntimeAuthorityObservation {
    context_workspace_id: WorkspaceId,
    activation_workspace_id: WorkspaceId,
    admission_head: ExpectedHead,
    resource_epoch_id: EpochId,
    resource_policy_pin: [u8; 32],
    activation_control_fingerprint: [u8; 32],
    query_authority_workspace_id: WorkspaceId,
    query_authority_epoch_id: EpochId,
    query_authority_shares_resources: bool,
    query_authority_resource_policy_pin: [u8; 32],
    query_runtime_workspace_id: WorkspaceId,
    query_runtime_shares_admission: bool,
    query_runtime_shares_resources: bool,
    query_runtime_shares_published_results: bool,
    query_authority_table_versions: super::activation::TableVersionSetRef,
    delta_runtime_workspace_id: WorkspaceId,
    delta_runtime_epoch_id: EpochId,
    delta_runtime_table_versions: super::activation::TableVersionSetRef,
    receipt: Option<ActivationCacheReceipt>,
}

trait ProgrammaticCommandRuntimeAuthorityView {
    fn context_workspace_id(&self) -> WorkspaceId;

    fn observe(
        &self,
        expected_epoch: EpochId,
    ) -> Result<
        ProgrammaticCommandRuntimeAuthorityObservation,
        ProgrammaticCommandRuntimeFactoryError,
    >;
}

struct LiveProgrammaticCommandRuntimeAuthorityView<'a>(&'a ProgrammaticCommandRuntimeContext);

impl ProgrammaticCommandRuntimeAuthorityView for LiveProgrammaticCommandRuntimeAuthorityView<'_> {
    fn context_workspace_id(&self) -> WorkspaceId {
        self.0.workspace_id()
    }

    fn observe(
        &self,
        expected_epoch: EpochId,
    ) -> Result<
        ProgrammaticCommandRuntimeAuthorityObservation,
        ProgrammaticCommandRuntimeFactoryError,
    > {
        let context = self.0;
        let query_authority = context
            .query_authorities()
            .resolve(expected_epoch)
            .map_err(ProgrammaticCommandRuntimeFactoryError::QueryAuthority)?;
        let receipt = context
            .receipt_cache()
            .current_receipt()
            .map_err(|_| ProgrammaticCommandRuntimeFactoryError::ReceiptCacheUnavailable)?;

        let query_runtime = context.query_runtime();

        Ok(ProgrammaticCommandRuntimeAuthorityObservation {
            context_workspace_id: context.workspace_id(),
            activation_workspace_id: context.activation_authority().workspace_id(),
            admission_head: context.admission().active_head(),
            resource_epoch_id: context.resources().epoch_id(),
            resource_policy_pin: *context.resources().resource_policy(),
            activation_control_fingerprint: *context
                .activation_authority()
                .control_relation()
                .fingerprint(),
            query_authority_workspace_id: query_authority.workspace_id(),
            query_authority_epoch_id: query_authority.epoch_id(),
            query_authority_shares_resources: Arc::ptr_eq(
                query_authority.resources(),
                context.resources(),
            ),
            query_authority_resource_policy_pin: *query_authority.resources().resource_policy(),
            query_runtime_workspace_id: query_runtime.workspace_id(),
            query_runtime_shares_admission: Arc::ptr_eq(
                query_runtime.admission(),
                context.admission(),
            ),
            query_runtime_shares_resources: Arc::ptr_eq(
                query_runtime.resources(),
                context.resources(),
            ),
            query_runtime_shares_published_results: Arc::ptr_eq(
                query_runtime.published_results(),
                context.published_results(),
            ),
            query_authority_table_versions: query_authority.activation_pins().table_versions,
            delta_runtime_workspace_id: context.delta_runtime().workspace_id(),
            delta_runtime_epoch_id: context.delta_runtime().epoch_id(),
            delta_runtime_table_versions: context.delta_runtime().table_version_set_ref(),
            receipt,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use async_trait::async_trait;

    use super::*;
    use crate::fabric::command::{
        ActorId, AuthorizationDecision, DiagnosticRef, FabricCommand, LeaseId, OperationId,
        TransactionRef, WriterGeneration,
    };
    use crate::fabric::command_actor::CommandPortError;
    use crate::fabric::command_actor::FabricCommandActorConfig;
    use crate::fabric::command_runtime_ports::{
        CommandAuthorizationPort, InterruptedCommitDiagnosticQuery,
        InterruptedCommitDiagnosticRelationPort,
    };
    use crate::fabric::programmatic_activation_admission::{
        ProgrammaticSuccessorQueryAuthorityOutcome, ProgrammaticSuccessorQueryAuthorityRequest,
    };
    use crate::fabric::programmatic_activation_command_ports::{
        ActivationCandidateProofObservation, ActivationCommandRequestKey,
        ActivationCommandRequestMaterial, ActivationNotSelectedClassification,
        ActivationNotSelectedClassificationQuery, ActivationReconciliationRead,
        ActivationReconciliationRecord, ActivationReconciliationWrite,
    };
    use crate::fabric::programmatic_workspace::WorkspaceEpochQueryAuthority;

    fn id16(seed: u8) -> [u8; 16] {
        [seed; 16]
    }

    fn id32(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn workspace(seed: u8) -> WorkspaceId {
        WorkspaceId::from_bytes(id16(seed))
    }

    fn epoch(seed: u8) -> EpochId {
        EpochId::from_bytes(id16(seed))
    }

    fn fence(seed: u8, generation: u64) -> WriterFence {
        WriterFence {
            lease_id: LeaseId::from_bytes(id16(seed)),
            generation: WriterGeneration::new(generation).expect("test generation is nonzero"),
        }
    }

    fn config(root: &Path, workspace_id: WorkspaceId) -> FabricCommandRuntimeConfig {
        FabricCommandRuntimeConfig::new(
            root,
            root.join("writer-generations.sqlite3"),
            root.join("commands.sqlite3"),
            workspace_id,
            LeaseId::from_bytes(id16(4)),
            ActorId::from_bytes(id16(5)),
            FabricCommandActorConfig::default(),
        )
    }

    struct AuthorizationPort;

    #[async_trait]
    impl CommandAuthorizationPort for AuthorizationPort {
        async fn authorize(
            &self,
            _command: &FabricCommand,
            _current_head: ExpectedHead,
        ) -> Result<AuthorizationDecision, CommandPortError> {
            Err(CommandPortError::ContextUnavailable)
        }
    }

    struct DiagnosticRelation;

    #[async_trait]
    impl InterruptedCommitDiagnosticRelationPort for DiagnosticRelation {
        async fn read_interruption_diagnostic(
            &self,
            _query: InterruptedCommitDiagnosticQuery,
        ) -> Result<Option<DiagnosticRef>, CommandPortError> {
            Err(CommandPortError::ContextUnavailable)
        }
    }

    struct MissingActivationState;

    #[async_trait]
    impl ActivationCommandStateStore for MissingActivationState {
        async fn read_request(
            &self,
            _key: ActivationCommandRequestKey,
        ) -> Result<Option<ActivationCommandRequestMaterial>, CommandPortError> {
            Ok(None)
        }

        async fn read_not_selected_classification(
            &self,
            _query: ActivationNotSelectedClassificationQuery,
        ) -> Result<Option<ActivationNotSelectedClassification>, CommandPortError> {
            Ok(None)
        }

        async fn persist_reconciliation(
            &self,
            _write: ActivationReconciliationWrite,
        ) -> Result<ActivationReconciliationRecord, CommandPortError> {
            Err(CommandPortError::ContextUnavailable)
        }

        async fn read_reconciliation(
            &self,
            _query: ActivationReconciliationRead,
        ) -> Result<Option<ActivationReconciliationRecord>, CommandPortError> {
            Ok(None)
        }
    }

    struct MissingProofRelations;

    #[async_trait]
    impl ActivationCandidateProofRelationsPort for MissingProofRelations {
        async fn observe_candidate(
            &self,
            request: super::super::activation_transaction::CandidateProofRequest,
        ) -> ActivationCandidateProofObservation {
            ActivationCandidateProofObservation::Missing {
                request,
                diagnostic: DiagnosticRef::from_bytes(id32(0x41)),
            }
        }
    }

    struct MissingEpochRebuilder;

    #[async_trait]
    impl ActivationEpochRebuilderPort for MissingEpochRebuilder {
        async fn rebuild_selected_epoch(
            &self,
            _request: ActivationEpochRebuildRequest,
        ) -> ActivationEpochRebuildOutcome {
            ActivationEpochRebuildOutcome::Unknown {
                diagnostic: DiagnosticRef::from_bytes(id32(0x42)),
            }
        }
    }

    struct MissingSuccessorQueryAuthority;

    #[async_trait]
    impl ProgrammaticSuccessorQueryAuthorityPort for MissingSuccessorQueryAuthority {
        async fn rebuild_successor(
            &self,
            _request: ProgrammaticSuccessorQueryAuthorityRequest,
        ) -> Result<Arc<WorkspaceEpochQueryAuthority>, ProgrammaticSuccessorQueryAuthorityOutcome>
        {
            Err(ProgrammaticSuccessorQueryAuthorityOutcome::Unavailable)
        }
    }

    fn effects() -> ExactProgrammaticCommandEffectClosure {
        let gap = |seed| {
            ProgrammaticCommandCapabilityGapInput::new(
                ProgrammaticCommandCapabilityDisposition::Unavailable,
                DiagnosticRef::from_bytes(id32(seed)),
            )
        };
        let unavailable = ProgrammaticNonActivationCommandEffects::try_new(
            gap(0x51),
            gap(0x52),
            gap(0x53),
            gap(0x54),
            gap(0x55),
            gap(0x56),
        )
        .unwrap();
        ExactProgrammaticCommandEffectClosure::new(
            ProgrammaticActivationCommandEffects::new(
                Arc::new(MissingActivationState),
                Arc::new(MissingProofRelations),
                DiagnosticRef::from_bytes(id32(0x57)),
                Arc::new(MissingEpochRebuilder),
                Arc::new(MissingSuccessorQueryAuthority),
            ),
            unavailable,
        )
    }

    fn binding(workspace_id: WorkspaceId) -> ProgrammaticCommandRuntimeAuthorityBinding {
        ProgrammaticCommandRuntimeAuthorityBinding::new(
            workspace_id,
            epoch(2),
            ActivationEventId::from_bytes(id32(3)),
            fence(6, 7),
            id32(8),
            id32(9),
        )
    }

    fn receipt(authority: ProgrammaticCommandRuntimeAuthorityBinding) -> ActivationCacheReceipt {
        ActivationCacheReceipt {
            workspace_id: authority.workspace_id(),
            operation_id: OperationId::from_bytes(id16(10)),
            event_id: authority.activation_event_id(),
            selected_epoch: authority.epoch_id(),
            active_fence: authority.activation_fence(),
            transaction: TransactionRef::from_bytes(id32(11)),
        }
    }

    fn observation(
        authority: ProgrammaticCommandRuntimeAuthorityBinding,
    ) -> ProgrammaticCommandRuntimeAuthorityObservation {
        ProgrammaticCommandRuntimeAuthorityObservation {
            context_workspace_id: authority.workspace_id(),
            activation_workspace_id: authority.workspace_id(),
            admission_head: ExpectedHead::Epoch(authority.epoch_id()),
            resource_epoch_id: authority.epoch_id(),
            resource_policy_pin: authority.resource_policy_pin(),
            activation_control_fingerprint: authority.activation_control_fingerprint(),
            query_authority_workspace_id: authority.workspace_id(),
            query_authority_epoch_id: authority.epoch_id(),
            query_authority_shares_resources: true,
            query_authority_resource_policy_pin: authority.resource_policy_pin(),
            query_runtime_workspace_id: authority.workspace_id(),
            query_runtime_shares_admission: true,
            query_runtime_shares_resources: true,
            query_runtime_shares_published_results: true,
            query_authority_table_versions:
                super::super::activation::TableVersionSetRef::from_bytes(id32(12)),
            delta_runtime_workspace_id: authority.workspace_id(),
            delta_runtime_epoch_id: authority.epoch_id(),
            delta_runtime_table_versions: super::super::activation::TableVersionSetRef::from_bytes(
                id32(12),
            ),
            receipt: Some(receipt(authority)),
        }
    }

    struct AuthorityView(ProgrammaticCommandRuntimeAuthorityObservation);

    impl ProgrammaticCommandRuntimeAuthorityView for AuthorityView {
        fn context_workspace_id(&self) -> WorkspaceId {
            self.0.context_workspace_id
        }

        fn observe(
            &self,
            _expected_epoch: EpochId,
        ) -> Result<
            ProgrammaticCommandRuntimeAuthorityObservation,
            ProgrammaticCommandRuntimeFactoryError,
        > {
            Ok(self.0)
        }
    }

    fn factory(
        root: &Path,
        configured_workspace: WorkspaceId,
        authority: ProgrammaticCommandRuntimeAuthorityBinding,
    ) -> ExactProgrammaticCommandRuntimePartsFactory {
        ExactProgrammaticCommandRuntimePartsFactory::new(
            config(root, configured_workspace),
            authority,
            Arc::new(AuthorizationPort),
            effects(),
            Arc::new(DiagnosticRelation),
        )
    }

    #[test]
    fn complete_authority_observation_passes_production_factory_validation() {
        let root = tempfile::tempdir().unwrap();
        let authority = binding(workspace(1));
        let factory = factory(root.path(), authority.workspace_id(), authority);

        factory
            .validate_authority_view(&AuthorityView(observation(authority)))
            .unwrap();
    }

    #[test]
    fn workspace_config_mismatch_is_rejected_before_parts_are_returned() {
        let root = tempfile::tempdir().unwrap();
        let authority = binding(workspace(1));
        let factory = factory(root.path(), workspace(99), authority);

        let error = factory
            .validate_authority_view(&AuthorityView(observation(authority)))
            .unwrap_err();

        assert_eq!(
            error,
            ProgrammaticCommandRuntimeFactoryError::ConfigWorkspaceMismatch
        );
    }

    #[test]
    fn production_construction_requires_router_semantics_and_diagnostics() {
        let root = tempfile::tempdir().unwrap();
        let authority = binding(workspace(1));
        let authorization: Arc<dyn CommandAuthorizationPort> = Arc::new(AuthorizationPort);
        let factory = ExactProgrammaticCommandRuntimePartsFactory::new(
            config(root.path(), authority.workspace_id()),
            authority,
            Arc::clone(&authorization),
            effects(),
            Arc::new(DiagnosticRelation),
        );

        assert!(Arc::ptr_eq(&factory.authorization, &authorization));
        let diagnostics: Arc<dyn InterruptedCommitDiagnosticPort> =
            factory.interruption_diagnostics.clone();
        assert!(Arc::strong_count(&diagnostics) >= 2);
        assert!(format!("{factory:?}").contains("context-activation-bound"));
        assert!(format!("{factory:?}").contains("complete-production-router"));
    }

    #[test]
    fn substituted_query_runtime_authorities_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let authority = binding(workspace(1));
        let factory = factory(root.path(), authority.workspace_id(), authority);

        for (substituted, expected) in [
            (
                observation(authority),
                ProgrammaticCommandRuntimeFactoryError::QueryRuntimeAdmissionMismatch,
            ),
            (
                observation(authority),
                ProgrammaticCommandRuntimeFactoryError::QueryRuntimeResourceMismatch,
            ),
            (
                observation(authority),
                ProgrammaticCommandRuntimeFactoryError::QueryRuntimePublishedResultsMismatch,
            ),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (mut observation, expected))| {
            match index {
                0 => observation.query_runtime_shares_admission = false,
                1 => observation.query_runtime_shares_resources = false,
                2 => observation.query_runtime_shares_published_results = false,
                _ => unreachable!(),
            }
            (observation, expected)
        }) {
            assert_eq!(factory.validate_observation(&substituted), Err(expected));
        }
    }
}
