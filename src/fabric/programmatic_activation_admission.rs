//! Activation admission which installs successor query authority before epoch exposure.
//!
//! The underlying [`FabricAdmissionRuntime`] remains the sole atomic epoch-swap owner. This
//! wrapper adds one ordered prerequisite while its opaque barrier is closed: rebuild the
//! successor query/resource authority from explicit production inputs, validate it against the
//! exact candidate and durable activation pins, and install it idempotently. A failed subsequent
//! swap may leave an unreachable authority installed; it can never expose an epoch early.

use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

use super::activation::{ActivationChain, ActivationEvent, FabricEpochPins};
use super::activation_transaction::{ActivationAdmissionPort, ActivationRecoveryAdmissionPort};
use super::admission::{
    ActivationBarrier, AdmissionError, FabricAdmissionRuntime, RecoverySelectionPublication,
};
use super::arrow_result_resource::ArrowResultResourceLimits;
use super::child_session::resource_governance::{EpochResourceCoordinator, EpochResourcePolicy};
use super::command::{ExpectedHead, WorkspaceId, WriterFence};
use super::programmatic_epoch::ProgrammaticFabricEpoch;
use super::programmatic_workspace::{
    WorkspaceEpochQueryAuthority, WorkspaceEpochQueryAuthorityRegistry,
    WorkspaceEpochQueryAuthorityRegistryError, programmatic_fabric_epoch_authority_pin,
};
use super::relational_query_runtime::RelationalQueryAuthorization;
use super::request_owned_relation::RequestOwnedRelationLimits;
use crate::relational_semantic_query::{
    EpochBoundSemanticExecutionCatalog, EpochBoundSemanticIngressCatalog, ProducerClosureProof,
};

/// Exact material request presented to the release-owned successor authority recipe.
#[derive(Clone, Debug)]
pub struct ProgrammaticSuccessorQueryAuthorityRequest {
    pub workspace_id: WorkspaceId,
    pub pins: FabricEpochPins,
    pub candidate: Arc<ProgrammaticFabricEpoch>,
}

/// Failure to rebuild complete successor query/resource authority from explicit inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammaticSuccessorQueryAuthorityOutcome {
    Unavailable,
    Invalid,
}

/// Process-loss-safe recipe for rebuilding one successor's complete query authority.
///
/// Implementations receive the freshly exact-Delta-reconstructed candidate. They must rebuild
/// catalogs, resource governance, and authorization from durable/reviewed typed inputs, never
/// resolve an `Arc` from a process-local candidate map.
#[async_trait]
pub trait ProgrammaticSuccessorQueryAuthorityPort: Send + Sync {
    async fn rebuild_successor(
        &self,
        request: ProgrammaticSuccessorQueryAuthorityRequest,
    ) -> Result<Arc<WorkspaceEpochQueryAuthority>, ProgrammaticSuccessorQueryAuthorityOutcome>;
}

/// Complete release-owned inputs for exactly one successor query authority.
///
/// This recipe owns values rather than an already-built authority or candidate `Arc`. Rebuilding
/// always creates a fresh resource coordinator and binds a freshly exact-Delta-reconstructed
/// candidate. The catalog's fabric-epoch authority pin is an explicit causal input, not rewritten
/// from a template or inferred from an epoch label.
#[derive(Clone)]
pub struct ExactProgrammaticSuccessorQueryAuthorityRecipe {
    workspace_id: WorkspaceId,
    pins: FabricEpochPins,
    candidate_authority_pin: [u8; 32],
    resource_policy: EpochResourcePolicy,
    ingress_catalog: EpochBoundSemanticIngressCatalog,
    execution_catalog: EpochBoundSemanticExecutionCatalog,
    producer_closure: ProducerClosureProof,
    authorization: RelationalQueryAuthorization,
    request_owned_relation_limits: RequestOwnedRelationLimits,
    result_limits: ArrowResultResourceLimits,
    result_lease_millis: u64,
}

impl std::fmt::Debug for ExactProgrammaticSuccessorQueryAuthorityRecipe {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExactProgrammaticSuccessorQueryAuthorityRecipe")
            .field("workspace_id", &self.workspace_id)
            .field("epoch_id", &self.pins.epoch)
            .field("candidate_authority_pin", &"REDACTED_IDENTITY")
            .finish_non_exhaustive()
    }
}

/// Invalid explicit successor-authority recipe input.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ExactProgrammaticSuccessorQueryAuthorityRecipeError {
    #[error("successor candidate authority pin uses the all-zero sentinel")]
    MissingCandidateAuthorityPin,
    #[error("successor ingress or execution catalog is not pinned to the exact candidate")]
    CandidateCatalogPinMismatch,
    #[error("successor ingress and execution catalogs disagree on {0}")]
    CatalogAuthorityMismatch(&'static str),
    #[error("successor producer closure differs from the catalog proof pin")]
    ProducerClosureMismatch,
    #[error("successor query authorization differs from the catalog policy")]
    QueryPolicyMismatch,
    #[error("successor query authorization differs from the activation resource-policy pin")]
    ResourcePolicyMismatch,
    #[error("successor result lease duration is zero")]
    ZeroResultLease,
}

impl ExactProgrammaticSuccessorQueryAuthorityRecipe {
    /// Bind one complete successor recipe before command runtime construction.
    ///
    /// Every catalog, policy, authorization, producer, and limit input is explicit. There is no
    /// default, latest-version lookup, process-local authority map, or stored candidate session.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        workspace_id: WorkspaceId,
        pins: FabricEpochPins,
        candidate_authority_pin: [u8; 32],
        resource_policy: EpochResourcePolicy,
        ingress_catalog: EpochBoundSemanticIngressCatalog,
        execution_catalog: EpochBoundSemanticExecutionCatalog,
        producer_closure: ProducerClosureProof,
        authorization: RelationalQueryAuthorization,
        request_owned_relation_limits: RequestOwnedRelationLimits,
        result_limits: ArrowResultResourceLimits,
        result_lease_millis: u64,
    ) -> Result<Self, ExactProgrammaticSuccessorQueryAuthorityRecipeError> {
        if candidate_authority_pin.iter().all(|byte| *byte == 0) {
            return Err(
                ExactProgrammaticSuccessorQueryAuthorityRecipeError::MissingCandidateAuthorityPin,
            );
        }
        if ingress_catalog.fabric_epoch_pin != candidate_authority_pin
            || execution_catalog.fabric_epoch_pin != candidate_authority_pin
        {
            return Err(
                ExactProgrammaticSuccessorQueryAuthorityRecipeError::CandidateCatalogPinMismatch,
            );
        }
        for (kind, matches) in [
            (
                "program-catalog",
                ingress_catalog.program_catalog_pin == execution_catalog.program_catalog_pin,
            ),
            (
                "source",
                ingress_catalog.source_pin == execution_catalog.source_pin,
            ),
            (
                "policy",
                ingress_catalog.policy_pin == execution_catalog.policy_pin,
            ),
            (
                "producer-closure",
                ingress_catalog.producer_closure_proof_pin
                    == execution_catalog.producer_closure_proof_pin,
            ),
        ] {
            if !matches {
                return Err(
                    ExactProgrammaticSuccessorQueryAuthorityRecipeError::CatalogAuthorityMismatch(
                        kind,
                    ),
                );
            }
        }
        if producer_closure.proof_pin != ingress_catalog.producer_closure_proof_pin {
            return Err(
                ExactProgrammaticSuccessorQueryAuthorityRecipeError::ProducerClosureMismatch,
            );
        }
        if authorization.query_policy() != &ingress_catalog.policy_pin {
            return Err(ExactProgrammaticSuccessorQueryAuthorityRecipeError::QueryPolicyMismatch);
        }
        if authorization.resource_policy() != pins.resource_envelope.as_bytes() {
            return Err(
                ExactProgrammaticSuccessorQueryAuthorityRecipeError::ResourcePolicyMismatch,
            );
        }
        if result_lease_millis == 0 {
            return Err(ExactProgrammaticSuccessorQueryAuthorityRecipeError::ZeroResultLease);
        }
        Ok(Self {
            workspace_id,
            pins,
            candidate_authority_pin,
            resource_policy,
            ingress_catalog,
            execution_catalog,
            producer_closure,
            authorization,
            request_owned_relation_limits,
            result_limits,
            result_lease_millis,
        })
    }
}

#[async_trait]
impl ProgrammaticSuccessorQueryAuthorityPort for ExactProgrammaticSuccessorQueryAuthorityRecipe {
    async fn rebuild_successor(
        &self,
        request: ProgrammaticSuccessorQueryAuthorityRequest,
    ) -> Result<Arc<WorkspaceEpochQueryAuthority>, ProgrammaticSuccessorQueryAuthorityOutcome> {
        if request.workspace_id != self.workspace_id
            || request.pins != self.pins
            || request.candidate.identity() != &self.pins.epoch
            || request
                .candidate
                .observation_publication()
                .table_version_set_ref()
                != self.pins.table_versions
            || programmatic_fabric_epoch_authority_pin(&request.candidate)
                != self.candidate_authority_pin
        {
            return Err(ProgrammaticSuccessorQueryAuthorityOutcome::Invalid);
        }
        let resources = Arc::new(
            EpochResourceCoordinator::try_new(
                self.pins.epoch,
                *self.pins.resource_envelope.as_bytes(),
                self.resource_policy.clone(),
            )
            .map_err(|_| ProgrammaticSuccessorQueryAuthorityOutcome::Invalid)?,
        );
        let authority = WorkspaceEpochQueryAuthority::try_new(
            self.workspace_id,
            request.pins,
            request.candidate,
            resources,
            Arc::new(self.ingress_catalog.clone()),
            Arc::new(self.execution_catalog.clone()),
            Arc::new(self.producer_closure.clone()),
            self.authorization.clone(),
            self.request_owned_relation_limits,
            self.result_limits,
            self.result_lease_millis,
        )
        .map_err(|_| ProgrammaticSuccessorQueryAuthorityOutcome::Invalid)?;
        Ok(Arc::new(authority))
    }
}

/// Admission wrapper enforcing successor-authority installation before atomic epoch swap.
pub struct ProgrammaticActivationAdmission {
    workspace_id: WorkspaceId,
    admission: Arc<FabricAdmissionRuntime>,
    query_authorities: Arc<WorkspaceEpochQueryAuthorityRegistry>,
    successors: Arc<dyn ProgrammaticSuccessorQueryAuthorityPort>,
}

impl std::fmt::Debug for ProgrammaticActivationAdmission {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProgrammaticActivationAdmission")
            .field("workspace_id", &self.workspace_id)
            .field("admission", &"exact-runtime")
            .field("query_authorities", &self.query_authorities)
            .field("successor_recipe", &"installed")
            .finish()
    }
}

impl ProgrammaticActivationAdmission {
    #[must_use]
    pub fn new(
        workspace_id: WorkspaceId,
        admission: Arc<FabricAdmissionRuntime>,
        query_authorities: Arc<WorkspaceEpochQueryAuthorityRegistry>,
        successors: Arc<dyn ProgrammaticSuccessorQueryAuthorityPort>,
    ) -> Self {
        Self {
            workspace_id,
            admission,
            query_authorities,
            successors,
        }
    }

    async fn install_successor(
        &self,
        pins: FabricEpochPins,
        candidate: Arc<ProgrammaticFabricEpoch>,
    ) -> Result<(), AdmissionError> {
        if candidate.identity() != &pins.epoch
            || candidate.observation_publication().table_version_set_ref() != pins.table_versions
        {
            return Err(AdmissionError::SuccessorQueryAuthorityMismatch(pins.epoch));
        }
        let authority = self
            .successors
            .rebuild_successor(ProgrammaticSuccessorQueryAuthorityRequest {
                workspace_id: self.workspace_id,
                pins,
                candidate: Arc::clone(&candidate),
            })
            .await
            .map_err(|outcome| match outcome {
                ProgrammaticSuccessorQueryAuthorityOutcome::Unavailable => {
                    AdmissionError::SuccessorQueryAuthorityUnavailable(pins.epoch)
                }
                ProgrammaticSuccessorQueryAuthorityOutcome::Invalid => {
                    AdmissionError::SuccessorQueryAuthorityMismatch(pins.epoch)
                }
            })?;
        if authority.workspace_id() != self.workspace_id
            || authority.epoch_id() != pins.epoch
            || !Arc::ptr_eq(authority.epoch(), &candidate)
            || authority.resources().epoch_id() != pins.epoch
            || authority.resources().resource_policy() != pins.resource_envelope.as_bytes()
            || authority.ingress_catalog().fabric_epoch_pin
                != super::programmatic_workspace::programmatic_fabric_epoch_authority_pin(
                    &candidate,
                )
            || authority.execution_catalog().fabric_epoch_pin
                != super::programmatic_workspace::programmatic_fabric_epoch_authority_pin(
                    &candidate,
                )
        {
            return Err(AdmissionError::SuccessorQueryAuthorityMismatch(pins.epoch));
        }
        self.query_authorities
            .install_or_validate(authority)
            .map_err(|error| match error {
                WorkspaceEpochQueryAuthorityRegistryError::WorkspaceMismatch
                | WorkspaceEpochQueryAuthorityRegistryError::AuthorityMismatch(_) => {
                    AdmissionError::SuccessorQueryAuthorityMismatch(pins.epoch)
                }
                WorkspaceEpochQueryAuthorityRegistryError::Poisoned => {
                    AdmissionError::SuccessorQueryAuthorityUnavailable(pins.epoch)
                }
                WorkspaceEpochQueryAuthorityRegistryError::DuplicateEpoch(_)
                | WorkspaceEpochQueryAuthorityRegistryError::UnknownEpoch(_) => {
                    AdmissionError::SuccessorQueryAuthorityInstallFailed(pins.epoch)
                }
            })?;
        Ok(())
    }
}

#[async_trait]
impl ActivationAdmissionPort for ProgrammaticActivationAdmission {
    type Barrier = ActivationBarrier;

    async fn close_admission(
        &self,
        expected_head: ExpectedHead,
        execution_fence: WriterFence,
    ) -> Result<Self::Barrier, AdmissionError> {
        self.admission
            .close_admission(expected_head, execution_fence)
    }

    async fn publish_selected_epoch(
        &self,
        barrier: Self::Barrier,
        chain_after_readback: &ActivationChain,
        candidate: Arc<ProgrammaticFabricEpoch>,
    ) -> Result<(), AdmissionError> {
        let pins = chain_after_readback
            .head_event()
            .ok_or(AdmissionError::MissingSelectedEvent)?
            .pins();
        self.install_successor(pins, Arc::clone(&candidate)).await?;
        self.admission
            .publish_selected_epoch(barrier, chain_after_readback, candidate)
    }

    async fn reconcile_and_reopen(
        &self,
        barrier: Self::Barrier,
        reconciled_head: ExpectedHead,
    ) -> Result<(), AdmissionError> {
        self.admission
            .reopen_after_reconciliation(barrier, reconciled_head)
    }

    async fn abort_proved_no_selection(
        &self,
        barrier: Self::Barrier,
        unchanged_chain: &ActivationChain,
    ) -> Result<(), AdmissionError> {
        self.admission
            .abort_before_selection(barrier, unchanged_chain)
    }
}

#[async_trait]
impl ActivationRecoveryAdmissionPort for ProgrammaticActivationAdmission {
    async fn recover_selected_epoch(
        &self,
        expected_head: ExpectedHead,
        execution_fence: WriterFence,
        active_recovery_fence: WriterFence,
        event: ActivationEvent,
        chain_after_readback: &ActivationChain,
        candidate: Arc<ProgrammaticFabricEpoch>,
        allow_already_reopened: bool,
    ) -> Result<RecoverySelectionPublication, AdmissionError> {
        self.install_successor(event.pins(), Arc::clone(&candidate))
            .await?;
        self.admission.recover_selected_epoch(
            expected_head,
            execution_fence,
            active_recovery_fence,
            event,
            chain_after_readback,
            candidate,
            allow_already_reopened,
        )
    }

    async fn reopen_recovered_selection(
        &self,
        event: ActivationEvent,
        chain_after_readback: &ActivationChain,
        active_recovery_fence: WriterFence,
    ) -> Result<(), AdmissionError> {
        if self.query_authorities.resolve(event.pins().epoch).is_err() {
            return Err(AdmissionError::SuccessorQueryAuthorityUnavailable(
                event.pins().epoch,
            ));
        }
        self.admission.reopen_recovered_selection(
            event,
            chain_after_readback,
            active_recovery_fence,
        )
    }

    async fn recover_proved_no_selection(
        &self,
        expected_head: ExpectedHead,
        execution_fence: WriterFence,
        active_recovery_fence: WriterFence,
        unchanged_chain: &ActivationChain,
    ) -> Result<(), AdmissionError> {
        self.admission.recover_proved_no_selection(
            expected_head,
            execution_fence,
            active_recovery_fence,
            unchanged_chain,
        )
    }
}
