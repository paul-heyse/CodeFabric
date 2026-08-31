//! Command-actor integration for the ordered fabric activation protocol.
//!
//! Preparation is a read-only resolution of one immutable
//! [`ActivationTransactionRequest`]. Commit delegates to the sole ordered
//! activation coordinator. Reconciliation has no append capability: it loads
//! the exact persisted request/ticket and delegates to marker-driven recovery.

use std::sync::Arc;

use async_trait::async_trait;

use super::activation::{ActivationAttempt, ActivationRecoveryAttempt};
use super::activation_transaction::{
    ActivatedEpochReceipt, ActivationAcknowledgementPort, ActivationAdmissionPort,
    ActivationAuthorityPort, ActivationCachePort, ActivationCandidateProofPort,
    ActivationEpochRebuilderPort, ActivationEventPort, ActivationNotSelected,
    ActivationNotSelectedReason, ActivationOperationMarkerPort, ActivationReconciliationTicket,
    ActivationRecoveryAdmissionPort, ActivationRecoveryCoordinator, ActivationRecoveryRequest,
    ActivationTransactionCoordinator, ActivationTransactionOutcome, ActivationTransactionRequest,
};
use super::command::{
    CommandFailure, CommandKind, CommandRecord, CommandResult, ExecutionOwner,
    ReconciliationEvidenceRef, ReconciliationObservation, ReductionContext, TransactionRef,
    UnknownCommit,
};
use super::command_actor::{CommandPortError, CommitEffectOutcome, PrepareEffectOutcome};
use super::command_effect_contract::{
    ValidatedCommandAttempt, executing_attempt, prepared_attempt, reconciliation_attempt,
};
use super::command_effect_router::ActivationCommandEffectPort;

/// Read-only prepared binding between one command and its deterministic
/// application transaction.
#[derive(Clone, Debug)]
pub struct ResolvedActivationTransaction {
    request: ActivationTransactionRequest,
    transaction: TransactionRef,
}

/// Candidate-free durable request binding used only by marker-driven
/// recovery.
#[derive(Clone, Debug)]
pub struct ResolvedActivationRecovery {
    request: ActivationRecoveryRequest,
    transaction: TransactionRef,
}

impl ResolvedActivationRecovery {
    /// Bind a durable recovery request to its transaction identity.
    pub fn try_new(
        request: ActivationRecoveryRequest,
        transaction: TransactionRef,
    ) -> Result<Self, ActivationCommandBindingError> {
        if request.transaction() != transaction {
            return Err(ActivationCommandBindingError::TransactionMismatch);
        }
        Ok(Self {
            request,
            transaction,
        })
    }

    #[must_use]
    pub const fn request(&self) -> &ActivationRecoveryRequest {
        &self.request
    }

    #[must_use]
    pub const fn transaction(&self) -> TransactionRef {
        self.transaction
    }

    fn into_request(self) -> ActivationRecoveryRequest {
        self.request
    }
}

impl ResolvedActivationTransaction {
    /// Bind an immutable request to the transaction identity it already
    /// carries.
    ///
    /// # Errors
    ///
    /// Rejects transaction drift between resolver output and request.
    pub fn try_new(
        request: ActivationTransactionRequest,
        transaction: TransactionRef,
    ) -> Result<Self, ActivationCommandBindingError> {
        if request.transaction() != transaction {
            return Err(ActivationCommandBindingError::TransactionMismatch);
        }
        Ok(Self {
            request,
            transaction,
        })
    }

    #[must_use]
    pub const fn request(&self) -> &ActivationTransactionRequest {
        &self.request
    }

    #[must_use]
    pub const fn transaction(&self) -> TransactionRef {
        self.transaction
    }
}

/// Exact request/ticket pair loaded for read-only recovery.
#[derive(Clone, Debug)]
pub struct LoadedActivationReconciliation {
    resolved: ResolvedActivationRecovery,
    ticket: ActivationReconciliationTicket,
}

impl LoadedActivationReconciliation {
    /// Validate that a persisted ticket names the exact immutable activation
    /// request and transaction.
    ///
    /// # Errors
    ///
    /// Rejects any identity, predecessor, fence, or selected-epoch drift.
    pub fn try_new(
        resolved: ResolvedActivationRecovery,
        ticket: ActivationReconciliationTicket,
    ) -> Result<Self, ActivationCommandBindingError> {
        let request = resolved.request();
        let command = request.command();
        if ticket.workspace_id != command.ownership.workspace_id
            || ticket.operation_id != command.identity.operation_id
            || ticket.candidate_epoch != request.pins().epoch
            || ticket.expected_head != command.expected_head
            || ticket.execution_fence != resolved.request().execution_fence()
            || ticket.event_id != request.event_id()
            || ticket.transaction != resolved.transaction()
            || ticket.operation_selection != request.operation_selection()
        {
            return Err(ActivationCommandBindingError::TicketMismatch);
        }
        Ok(Self { resolved, ticket })
    }

    #[must_use]
    pub const fn resolved(&self) -> &ResolvedActivationRecovery {
        &self.resolved
    }

    #[must_use]
    pub const fn ticket(&self) -> ActivationReconciliationTicket {
        self.ticket
    }

    fn into_parts(self) -> (ActivationRecoveryRequest, ActivationReconciliationTicket) {
        (self.resolved.into_request(), self.ticket)
    }
}

/// Temporal persistence result for one exact reconciliation ticket. The
/// diagnostic/evidence identities are owned by the application port; this
/// adapter never fabricates them from enum discriminants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistedActivationReconciliation {
    pub unknown: UnknownCommit,
    pub evidence: ReconciliationEvidenceRef,
}

/// Application-owned request, diagnostic, and reconciliation-ticket state.
/// Resolve/load methods are read-only. Persistence/classification methods may
/// write temporal diagnostics/tickets but never activation events.
#[async_trait]
pub trait ActivationCommandStatePort: Send + Sync {
    async fn resolve_request(
        &self,
        record: &CommandRecord,
        attempt: ActivationAttempt,
        context: ReductionContext,
    ) -> Result<ResolvedActivationTransaction, CommandPortError>;

    async fn classify_not_selected(
        &self,
        resolved: &ResolvedActivationTransaction,
        stopped: ActivationNotSelected,
    ) -> Result<CommandFailure, CommandPortError>;

    async fn persist_reconciliation(
        &self,
        resolved: &ResolvedActivationRecovery,
        ticket: ActivationReconciliationTicket,
    ) -> Result<PersistedActivationReconciliation, CommandPortError>;

    async fn load_reconciliation(
        &self,
        record: &CommandRecord,
        owner: ExecutionOwner,
        transaction: TransactionRef,
        context: ReductionContext,
    ) -> Result<LoadedActivationReconciliation, CommandPortError>;
}

/// Narrow forward execution seam used by the command adapter.
#[async_trait]
pub trait ActivationCommitExecutionPort: Send + Sync {
    async fn activate(&self, request: ActivationTransactionRequest)
    -> ActivationTransactionOutcome;
}

#[async_trait]
impl<A, P, H, D, C, K> ActivationCommitExecutionPort
    for ActivationTransactionCoordinator<A, P, H, D, C, K>
where
    A: ActivationAdmissionPort,
    P: ActivationCandidateProofPort,
    H: ActivationAuthorityPort,
    D: ActivationEventPort,
    C: ActivationCachePort,
    K: ActivationAcknowledgementPort,
{
    async fn activate(
        &self,
        request: ActivationTransactionRequest,
    ) -> ActivationTransactionOutcome {
        ActivationTransactionCoordinator::activate(self, request).await
    }
}

/// Narrow marker-only recovery seam used by the command adapter.
#[async_trait]
pub trait ActivationRecoveryExecutionPort: Send + Sync {
    async fn recover(
        &self,
        request: ActivationRecoveryRequest,
        ticket: ActivationReconciliationTicket,
        recovery: ActivationRecoveryAttempt,
    ) -> ActivationTransactionOutcome;
}

#[async_trait]
impl<A, M, R, C, K> ActivationRecoveryExecutionPort for ActivationRecoveryCoordinator<A, M, R, C, K>
where
    A: ActivationRecoveryAdmissionPort,
    M: ActivationOperationMarkerPort,
    R: ActivationEpochRebuilderPort,
    C: ActivationCachePort,
    K: ActivationAcknowledgementPort,
{
    async fn recover(
        &self,
        request: ActivationRecoveryRequest,
        ticket: ActivationReconciliationTicket,
        recovery: ActivationRecoveryAttempt,
    ) -> ActivationTransactionOutcome {
        ActivationRecoveryCoordinator::recover(self, request, ticket, recovery).await
    }
}

/// Concrete activation family installed into [`super::command_effect_router::FabricCommandEffectRouter`].
pub struct ActivationCommandEffect {
    state: Arc<dyn ActivationCommandStatePort>,
    commits: Arc<dyn ActivationCommitExecutionPort>,
    recovery: Arc<dyn ActivationRecoveryExecutionPort>,
}

impl std::fmt::Debug for ActivationCommandEffect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ActivationCommandEffect")
            .field("state", &"installed")
            .field("commits", &"installed")
            .field("recovery", &"installed")
            .finish()
    }
}

impl ActivationCommandEffect {
    #[must_use]
    pub fn new(
        state: Arc<dyn ActivationCommandStatePort>,
        commits: Arc<dyn ActivationCommitExecutionPort>,
        recovery: Arc<dyn ActivationRecoveryExecutionPort>,
    ) -> Self {
        Self {
            state,
            commits,
            recovery,
        }
    }

    async fn resolved(
        &self,
        record: &CommandRecord,
        attempt: ValidatedCommandAttempt,
        context: ReductionContext,
        transaction: Option<TransactionRef>,
    ) -> Result<ResolvedActivationTransaction, CommandPortError> {
        let activation_attempt = ActivationAttempt::from_validated(attempt);
        let resolved = self
            .state
            .resolve_request(record, activation_attempt, context)
            .await?;
        if resolved.request().attempt() != activation_attempt
            || transaction.is_some_and(|value| value != resolved.transaction())
        {
            return Err(CommandPortError::CorruptRecord);
        }
        Ok(resolved)
    }
}

#[async_trait]
impl ActivationCommandEffectPort for ActivationCommandEffect {
    async fn prepare(
        &self,
        executing: &CommandRecord,
        owner: ExecutionOwner,
        context: ReductionContext,
    ) -> Result<PrepareEffectOutcome, CommandPortError> {
        let attempt = executing_attempt(executing, owner, context, CommandKind::ActivateEpoch)?;
        let resolved = self.resolved(executing, attempt, context, None).await?;
        Ok(PrepareEffectOutcome::Prepared {
            transaction: resolved.transaction(),
        })
    }

    async fn commit(
        &self,
        prepared: &CommandRecord,
        owner: ExecutionOwner,
        transaction: TransactionRef,
        context: ReductionContext,
    ) -> Result<CommitEffectOutcome, CommandPortError> {
        let attempt = prepared_attempt(
            prepared,
            owner,
            transaction,
            context,
            CommandKind::ActivateEpoch,
        )?;
        let resolved = self
            .resolved(prepared, attempt, context, Some(transaction))
            .await?;
        match self.commits.activate(resolved.request().clone()).await {
            ActivationTransactionOutcome::Activated(receipt) => {
                if receipt.reconciliation_evidence.is_some()
                    || !activated_receipt_matches(
                        resolved.request(),
                        receipt,
                        resolved.request().execution_fence(),
                    )
                {
                    return Err(CommandPortError::CorruptRecord);
                }
                Ok(CommitEffectOutcome::Committed {
                    result: activated_result(receipt),
                })
            }
            ActivationTransactionOutcome::NotSelected(stopped) => {
                if !not_selected_matches(resolved.request(), stopped) {
                    return Err(CommandPortError::CorruptRecord);
                }
                let failure = self.state.classify_not_selected(&resolved, stopped).await?;
                Ok(CommitEffectOutcome::KnownFailure { failure })
            }
            ActivationTransactionOutcome::ReconciliationNeeded(ticket) => {
                let recovery_resolved = ResolvedActivationRecovery::try_new(
                    resolved.request().recovery_request(),
                    transaction,
                )
                .map_err(|_| CommandPortError::CorruptRecord)?;
                LoadedActivationReconciliation::try_new(recovery_resolved.clone(), ticket)
                    .map_err(|_| CommandPortError::CorruptRecord)?;
                let persisted = self
                    .state
                    .persist_reconciliation(&recovery_resolved, ticket)
                    .await?;
                Ok(CommitEffectOutcome::Unknown {
                    unknown: persisted.unknown,
                })
            }
        }
    }

    async fn reconcile(
        &self,
        awaiting: &CommandRecord,
        owner: ExecutionOwner,
        transaction: TransactionRef,
        context: ReductionContext,
    ) -> Result<ReconciliationObservation, CommandPortError> {
        let recovery = reconciliation_attempt(
            awaiting,
            owner,
            transaction,
            context,
            CommandKind::ActivateEpoch,
        )?;
        let attempt = ActivationAttempt::from_validated(recovery.attempt());
        let recovery_attempt = ActivationRecoveryAttempt::from_validated(recovery);
        let loaded = self
            .state
            .load_reconciliation(awaiting, owner, transaction, context)
            .await?;
        if loaded.resolved().request().attempt() != attempt
            || loaded.resolved().request().command() != attempt.command()
            || loaded.resolved().transaction() != transaction
            || loaded.resolved().request().execution_fence() != attempt.execution_owner().fence
        {
            return Err(CommandPortError::CorruptRecord);
        }
        let resolved = loaded.resolved().clone();
        let (request, ticket) = loaded.into_parts();
        match self
            .recovery
            .recover(request.clone(), ticket, recovery_attempt)
            .await
        {
            ActivationTransactionOutcome::Activated(receipt) => {
                let evidence = receipt
                    .reconciliation_evidence
                    .ok_or(CommandPortError::CorruptRecord)?;
                if !recovered_activated_receipt_matches(&request, receipt, owner.fence) {
                    return Err(CommandPortError::CorruptRecord);
                }
                Ok(ReconciliationObservation::Committed {
                    evidence,
                    result: activated_result(receipt),
                })
            }
            ActivationTransactionOutcome::NotSelected(ActivationNotSelected {
                reason: ActivationNotSelectedReason::OperationMarkerProvedNotSelected(evidence),
                stage,
                operation_id,
                candidate_epoch,
            }) if not_selected_matches(
                &request,
                ActivationNotSelected {
                    reason: ActivationNotSelectedReason::OperationMarkerProvedNotSelected(evidence),
                    stage,
                    operation_id,
                    candidate_epoch,
                },
            ) =>
            {
                Ok(ReconciliationObservation::NotCommitted { evidence })
            }
            ActivationTransactionOutcome::NotSelected(_) => Err(CommandPortError::CorruptRecord),
            ActivationTransactionOutcome::ReconciliationNeeded(next_ticket) => {
                LoadedActivationReconciliation::try_new(resolved.clone(), next_ticket)
                    .map_err(|_| CommandPortError::CorruptRecord)?;
                let persisted = self
                    .state
                    .persist_reconciliation(&resolved, next_ticket)
                    .await?;
                Ok(ReconciliationObservation::Indeterminate {
                    evidence: persisted.evidence,
                })
            }
        }
    }
}

fn activated_result(receipt: ActivatedEpochReceipt) -> CommandResult {
    CommandResult::EpochActivated {
        epoch: receipt.event.pins().epoch,
        selection: receipt.event.commit().operation_selection,
    }
}

fn activated_receipt_matches(
    request: &ActivationTransactionRequest,
    receipt: ActivatedEpochReceipt,
    active_fence: super::command::WriterFence,
) -> bool {
    activated_receipt_identity_matches(request, receipt)
        && receipt.cache.active_fence == active_fence
        && receipt.acknowledgement.active_fence == active_fence
}

fn recovered_activated_receipt_matches(
    request: &ActivationRecoveryRequest,
    receipt: ActivatedEpochReceipt,
    active_recovery_fence: super::command::WriterFence,
) -> bool {
    activated_receipt_identity_matches(request, receipt)
        && receipt.cache.active_fence == active_recovery_fence
        && recovery_fence_authorizes(
            request.execution_fence(),
            receipt.acknowledgement.active_fence,
        )
        && recovery_fence_authorizes(receipt.acknowledgement.active_fence, active_recovery_fence)
}

fn activated_receipt_identity_matches<R: ActivationCommandRequestView>(
    request: &R,
    receipt: ActivatedEpochReceipt,
) -> bool {
    let event = receipt.event;
    event.workspace_id() == request.command().ownership.workspace_id
        && event.operation_id() == request.command().identity.operation_id
        && event.event_id() == request.event_id()
        && event.predecessor_epoch() == request.command().expected_head
        && event.execution_fence() == request.execution_fence()
        && event.pins() == request.pins()
        && event.commit().transaction == request.transaction()
        && event.commit().operation_selection == request.operation_selection()
        && receipt.cache.workspace_id == event.workspace_id()
        && receipt.cache.operation_id == event.operation_id()
        && receipt.cache.event_id == event.event_id()
        && receipt.cache.selected_epoch == request.pins().epoch
        && receipt.cache.transaction == request.transaction()
        && receipt.acknowledgement.workspace_id == event.workspace_id()
        && receipt.acknowledgement.operation_id == event.operation_id()
        && receipt.acknowledgement.event_id == event.event_id()
        && receipt.acknowledgement.selected_epoch == request.pins().epoch
        && receipt.acknowledgement.transaction == request.transaction()
        && receipt.acknowledgement.operation_selection == request.operation_selection()
}

fn recovery_fence_authorizes(
    execution: super::command::WriterFence,
    active: super::command::WriterFence,
) -> bool {
    active == execution || active.generation.get() > execution.generation.get()
}

fn not_selected_matches<R: ActivationCommandRequestView>(
    request: &R,
    stopped: ActivationNotSelected,
) -> bool {
    stopped.operation_id == request.command().identity.operation_id
        && stopped.candidate_epoch == request.pins().epoch
}

trait ActivationCommandRequestView {
    fn command(&self) -> &super::command::FabricCommand;
    fn execution_fence(&self) -> super::command::WriterFence;
    fn pins(&self) -> super::activation::FabricEpochPins;
    fn event_id(&self) -> super::activation::ActivationEventId;
    fn operation_selection(&self) -> super::command::OperationSelectionRef;
    fn transaction(&self) -> TransactionRef;
}

impl ActivationCommandRequestView for ActivationTransactionRequest {
    fn command(&self) -> &super::command::FabricCommand {
        self.command()
    }

    fn execution_fence(&self) -> super::command::WriterFence {
        self.execution_fence()
    }

    fn pins(&self) -> super::activation::FabricEpochPins {
        self.pins()
    }

    fn event_id(&self) -> super::activation::ActivationEventId {
        self.event_id()
    }

    fn operation_selection(&self) -> super::command::OperationSelectionRef {
        self.operation_selection()
    }

    fn transaction(&self) -> TransactionRef {
        self.transaction()
    }
}

impl ActivationCommandRequestView for ActivationRecoveryRequest {
    fn command(&self) -> &super::command::FabricCommand {
        self.command()
    }

    fn execution_fence(&self) -> super::command::WriterFence {
        self.execution_fence()
    }

    fn pins(&self) -> super::activation::FabricEpochPins {
        self.pins()
    }

    fn event_id(&self) -> super::activation::ActivationEventId {
        self.event_id()
    }

    fn operation_selection(&self) -> super::command::OperationSelectionRef {
        self.operation_selection()
    }

    fn transaction(&self) -> TransactionRef {
        self.transaction()
    }
}

/// Invalid binding returned by an application-owned request/ticket store.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ActivationCommandBindingError {
    #[error("activation request transaction differs from deterministic transaction")]
    TransactionMismatch,
    #[error("activation reconciliation ticket differs from immutable request")]
    TicketMismatch,
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::fabric::activation::{
        ActivationCommit, ActivationControlRelationPin, ActivationEvent, ActivationEventId,
        ActivationOrdinal, ActivationReadbackRef, BackendCommitRef, CompatibilityClassRef,
        FabricEpochPins, OverlaySegmentSetRef, PolicySetRef, SealedActivationControlBinding,
    };
    use crate::fabric::activation_transaction::{
        ActivationAcknowledgementReceipt, ActivationAdmissionPosture,
        ActivationAppendUnknownReason, ActivationCacheReceipt, ActivationReconciliationReason,
        ActivationTransactionStage, DurableSelectionKnowledge,
    };
    use crate::fabric::command::{
        ActorId, AdmissionContext, AuthorizationDecision, AuthorizationRef, CommandEvent,
        CommandIdentity, CommandOwnership, CommandPins, CommandReducer, CompilerReleaseRef,
        DiagnosticRef, EpochId, ExpectedHead, FabricCommand, FabricCommandPayload, FailureClass,
        FailureCode, IdempotencyKey, LeaseId, ModelHeadRef, OperationId, OperationSelectionRef,
        PrincipalId, ProofReceiptRef, ProviderSetRef, ResourceEnvelopeRef, RetentionPolicyRef,
        SourceGeneration, UnknownCommitReason, WorkspaceId, WriterFence, WriterGeneration,
    };
    use crate::fabric::delta_exact::ExactDeltaPin;
    use crate::fabric::epoch::FabricEpochRuntimeConfig;
    use crate::fabric::programmatic_epoch::{
        ProgrammaticFabricEpoch, ProgrammaticFabricEpochBuilder,
    };
    use url::Url;

    const fn id16(seed: u8) -> [u8; 16] {
        [seed; 16]
    }

    const fn id32(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn control_relation() -> ActivationControlRelationPin {
        let root = Url::parse("memory:///codefabric/activation-effect-control").unwrap();
        ActivationControlRelationPin::new(
            ExactDeltaPin::new(&root, 4).unwrap(),
            SealedActivationControlBinding::for_test(
                "activation-effect-test-session",
                "binding.system.activation-control.delta",
            ),
        )
    }

    async fn candidate(epoch: EpochId) -> Arc<ProgrammaticFabricEpoch> {
        let config = FabricEpochRuntimeConfig::default();
        Arc::new(
            ProgrammaticFabricEpochBuilder::try_new(epoch, config)
                .unwrap()
                .seal_for_test()
                .await
                .unwrap(),
        )
    }

    struct Fixture {
        request: ActivationTransactionRequest,
        event: ActivationEvent,
        ticket: ActivationReconciliationTicket,
        owner: ExecutionOwner,
        context: ReductionContext,
        unknown: UnknownCommit,
        failure: CommandFailure,
        evidence: ReconciliationEvidenceRef,
    }

    async fn fixture(seed: u8) -> Fixture {
        let workspace_id = WorkspaceId::from_bytes(id16(seed));
        let epoch_id = EpochId::from_bytes(id16(seed.wrapping_add(1)));
        let candidate = candidate(epoch_id).await;
        let writer_fence = WriterFence {
            lease_id: LeaseId::from_bytes(id16(seed.wrapping_add(2))),
            generation: WriterGeneration::new(u64::from(seed) + 1).unwrap(),
        };
        let command = FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes(id16(seed.wrapping_add(3))),
                idempotency_key: IdempotencyKey::from_bytes(id32(seed.wrapping_add(4))),
            },
            ownership: CommandOwnership {
                workspace_id,
                principal_id: PrincipalId::from_bytes(id16(seed.wrapping_add(5))),
                authorization: AuthorizationRef::from_bytes(id32(seed.wrapping_add(6))),
            },
            expected_head: ExpectedHead::Empty,
            writer_fence,
            pins: CommandPins {
                compiler_release: CompilerReleaseRef::from_bytes(id32(seed.wrapping_add(7))),
                model_head: ModelHeadRef::from_bytes(id32(seed.wrapping_add(8))),
                source_generation: SourceGeneration::new(u64::from(seed) + 9),
                provider_set: ProviderSetRef::from_bytes(id32(seed.wrapping_add(10))),
            },
            resources: ResourceEnvelopeRef::from_bytes(id32(seed.wrapping_add(11))),
            payload: FabricCommandPayload::ActivateEpoch {
                candidate_epoch: epoch_id,
                proof_receipt: ProofReceiptRef::from_bytes(id32(seed.wrapping_add(12))),
            },
        };
        let pins = FabricEpochPins {
            epoch: epoch_id,
            compiler_release: command.pins.compiler_release,
            model_head: command.pins.model_head,
            source_generation: command.pins.source_generation,
            provider_set: command.pins.provider_set,
            table_versions: candidate.observation_publication().table_version_set_ref(),
            overlay_segments: OverlaySegmentSetRef::from_bytes(id32(seed.wrapping_add(14))),
            policy_set: PolicySetRef::from_bytes(id32(seed.wrapping_add(15))),
            resource_envelope: command.resources,
            proof_receipt: ProofReceiptRef::from_bytes(id32(seed.wrapping_add(12))),
        };
        let event_id = ActivationEventId::from_bytes(id32(seed.wrapping_add(16)));
        let compatibility = CompatibilityClassRef::from_bytes(id32(seed.wrapping_add(17)));
        let retention = RetentionPolicyRef::from_bytes(id32(seed.wrapping_add(18)));
        let operation_selection = OperationSelectionRef::from_bytes(id32(seed.wrapping_add(19)));
        let transaction = TransactionRef::from_bytes(id32(seed.wrapping_add(20)));
        let owner = ExecutionOwner {
            actor_id: ActorId::from_bytes(id16(seed.wrapping_add(24))),
            fence: writer_fence,
        };
        let attempt = ActivationAttempt::for_test(command, 1, owner);
        let request = ActivationTransactionRequest::try_new(
            attempt,
            candidate,
            pins,
            event_id,
            compatibility,
            retention,
            operation_selection,
            transaction,
            control_relation(),
        )
        .unwrap();
        let event = ActivationEvent::try_from_attempt(
            event_id,
            attempt,
            None,
            ActivationOrdinal::new(1).unwrap(),
            pins,
            compatibility,
            retention,
            ActivationCommit {
                operation_selection,
                transaction,
                backend_commit: BackendCommitRef::from_bytes(id32(seed.wrapping_add(21))),
                readback: ActivationReadbackRef::from_bytes(id32(seed.wrapping_add(22))),
            },
        )
        .unwrap();
        let ticket = ActivationReconciliationTicket {
            stage: ActivationTransactionStage::DurableAppendReadback,
            reason: ActivationReconciliationReason::AppendUnknown {
                reason: ActivationAppendUnknownReason::CommitOutcomeUnknown,
                diagnostic: DiagnosticRef::from_bytes(id32(seed.wrapping_add(23))),
            },
            workspace_id,
            operation_id: command.identity.operation_id,
            candidate_epoch: epoch_id,
            expected_head: command.expected_head,
            execution_fence: writer_fence,
            event_id,
            transaction,
            operation_selection,
            durable_selection: DurableSelectionKnowledge::Unknown,
            admission_posture: ActivationAdmissionPosture::Closed,
        };
        Fixture {
            request,
            event,
            ticket,
            owner,
            context: ReductionContext {
                current_head: ExpectedHead::Empty,
                active_fence: writer_fence,
            },
            unknown: UnknownCommit {
                reason: UnknownCommitReason::ReadbackUnavailable,
                diagnostic: DiagnosticRef::from_bytes(id32(seed.wrapping_add(25))),
            },
            failure: CommandFailure {
                code: FailureCode::BackendUnavailable,
                class: FailureClass::RetryableBeforeCommit,
                diagnostic: DiagnosticRef::from_bytes(id32(seed.wrapping_add(26))),
            },
            evidence: ReconciliationEvidenceRef::from_bytes(id32(seed.wrapping_add(27))),
        }
    }

    fn executing(fixture: &Fixture) -> CommandRecord {
        let admitted = CommandReducer::admit(
            None,
            fixture.request.command(),
            AdmissionContext {
                workspace_id: fixture.request.command().ownership.workspace_id,
                current_head: fixture.context.current_head,
                active_fence: fixture.context.active_fence,
                authorization: AuthorizationDecision::Authorized(
                    fixture.request.command().ownership.authorization,
                ),
            },
        )
        .unwrap()
        .record();
        CommandReducer::reduce(
            &admitted,
            CommandEvent::Start {
                owner: fixture.owner,
            },
            fixture.context,
        )
        .unwrap()
        .record
    }

    fn prepared(fixture: &Fixture) -> CommandRecord {
        CommandReducer::reduce(
            &executing(fixture),
            CommandEvent::PrepareCommit {
                owner: fixture.owner,
                transaction: fixture.request.transaction(),
            },
            fixture.context,
        )
        .unwrap()
        .record
    }

    fn awaiting(fixture: &Fixture) -> CommandRecord {
        CommandReducer::reduce(
            &prepared(fixture),
            CommandEvent::ReportUnknownCommit {
                owner: fixture.owner,
                transaction: fixture.request.transaction(),
                unknown: fixture.unknown,
            },
            fixture.context,
        )
        .unwrap()
        .record
    }

    struct MockState {
        resolved: ResolvedActivationTransaction,
        loaded: LoadedActivationReconciliation,
        failure: CommandFailure,
        persisted: PersistedActivationReconciliation,
        resolves: AtomicUsize,
        classifications: AtomicUsize,
        persists: Mutex<Vec<ActivationReconciliationTicket>>,
        loads: AtomicUsize,
    }

    impl MockState {
        fn new(fixture: &Fixture) -> Self {
            let resolved = ResolvedActivationTransaction::try_new(
                fixture.request.clone(),
                fixture.request.transaction(),
            )
            .unwrap();
            let recovery = ResolvedActivationRecovery::try_new(
                fixture.request.recovery_request(),
                fixture.request.transaction(),
            )
            .unwrap();
            let loaded = LoadedActivationReconciliation::try_new(recovery, fixture.ticket).unwrap();
            Self {
                resolved,
                loaded,
                failure: fixture.failure,
                persisted: PersistedActivationReconciliation {
                    unknown: fixture.unknown,
                    evidence: fixture.evidence,
                },
                resolves: AtomicUsize::new(0),
                classifications: AtomicUsize::new(0),
                persists: Mutex::new(Vec::new()),
                loads: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl ActivationCommandStatePort for MockState {
        async fn resolve_request(
            &self,
            record: &CommandRecord,
            attempt: ActivationAttempt,
            _context: ReductionContext,
        ) -> Result<ResolvedActivationTransaction, CommandPortError> {
            self.resolves.fetch_add(1, Ordering::SeqCst);
            assert_eq!(record.command(), self.resolved.request().command());
            assert_eq!(attempt, self.resolved.request().attempt());
            Ok(self.resolved.clone())
        }

        async fn classify_not_selected(
            &self,
            resolved: &ResolvedActivationTransaction,
            stopped: ActivationNotSelected,
        ) -> Result<CommandFailure, CommandPortError> {
            self.classifications.fetch_add(1, Ordering::SeqCst);
            assert_eq!(resolved.transaction(), self.resolved.transaction());
            assert_eq!(
                stopped.operation_id,
                resolved.request().command().identity.operation_id
            );
            Ok(self.failure)
        }

        async fn persist_reconciliation(
            &self,
            resolved: &ResolvedActivationRecovery,
            ticket: ActivationReconciliationTicket,
        ) -> Result<PersistedActivationReconciliation, CommandPortError> {
            assert_eq!(resolved.transaction(), self.resolved.transaction());
            self.persists.lock().unwrap().push(ticket);
            Ok(self.persisted)
        }

        async fn load_reconciliation(
            &self,
            record: &CommandRecord,
            _owner: ExecutionOwner,
            transaction: TransactionRef,
            _context: ReductionContext,
        ) -> Result<LoadedActivationReconciliation, CommandPortError> {
            self.loads.fetch_add(1, Ordering::SeqCst);
            assert_eq!(record.command(), self.loaded.resolved().request().command());
            assert_eq!(transaction, self.loaded.resolved().transaction());
            Ok(self.loaded.clone())
        }
    }

    struct StaticCommit {
        outcome: ActivationTransactionOutcome,
        calls: AtomicUsize,
    }

    #[async_trait]
    impl ActivationCommitExecutionPort for StaticCommit {
        async fn activate(
            &self,
            _request: ActivationTransactionRequest,
        ) -> ActivationTransactionOutcome {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.outcome
        }
    }

    struct StaticRecovery {
        outcome: ActivationTransactionOutcome,
        expected_ticket: ActivationReconciliationTicket,
        expected_active_fence: WriterFence,
        calls: AtomicUsize,
    }

    #[async_trait]
    impl ActivationRecoveryExecutionPort for StaticRecovery {
        async fn recover(
            &self,
            request: ActivationRecoveryRequest,
            ticket: ActivationReconciliationTicket,
            recovery: ActivationRecoveryAttempt,
        ) -> ActivationTransactionOutcome {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(request.transaction(), self.expected_ticket.transaction);
            assert_eq!(ticket, self.expected_ticket);
            assert_eq!(
                recovery.active_recovery_owner().fence,
                self.expected_active_fence
            );
            self.outcome
        }
    }

    fn activated_receipt(
        fixture: &Fixture,
        active_fence: WriterFence,
        reconciliation_evidence: Option<ReconciliationEvidenceRef>,
    ) -> ActivatedEpochReceipt {
        activated_receipt_with_acknowledgement_fence(
            fixture,
            active_fence,
            active_fence,
            reconciliation_evidence,
        )
    }

    fn activated_receipt_with_acknowledgement_fence(
        fixture: &Fixture,
        cache_fence: WriterFence,
        acknowledgement_fence: WriterFence,
        reconciliation_evidence: Option<ReconciliationEvidenceRef>,
    ) -> ActivatedEpochReceipt {
        ActivatedEpochReceipt {
            event: fixture.event,
            cache: ActivationCacheReceipt {
                workspace_id: fixture.event.workspace_id(),
                operation_id: fixture.event.operation_id(),
                event_id: fixture.event.event_id(),
                selected_epoch: fixture.event.pins().epoch,
                active_fence: cache_fence,
                transaction: fixture.event.commit().transaction,
            },
            acknowledgement: ActivationAcknowledgementReceipt {
                workspace_id: fixture.event.workspace_id(),
                operation_id: fixture.event.operation_id(),
                event_id: fixture.event.event_id(),
                selected_epoch: fixture.event.pins().epoch,
                active_fence: acknowledgement_fence,
                transaction: fixture.event.commit().transaction,
                operation_selection: fixture.event.commit().operation_selection,
            },
            reconciliation_evidence,
        }
    }

    #[tokio::test]
    async fn prepare_only_resolves_the_immutable_request_and_transaction() {
        let fixture = fixture(80).await;
        let state = Arc::new(MockState::new(&fixture));
        let commits = Arc::new(StaticCommit {
            outcome: ActivationTransactionOutcome::ReconciliationNeeded(fixture.ticket),
            calls: AtomicUsize::new(0),
        });
        let recovery = Arc::new(StaticRecovery {
            outcome: ActivationTransactionOutcome::ReconciliationNeeded(fixture.ticket),
            expected_ticket: fixture.ticket,
            expected_active_fence: fixture.owner.fence,
            calls: AtomicUsize::new(0),
        });
        let effect = ActivationCommandEffect::new(
            Arc::clone(&state) as Arc<dyn ActivationCommandStatePort>,
            Arc::clone(&commits) as Arc<dyn ActivationCommitExecutionPort>,
            Arc::clone(&recovery) as Arc<dyn ActivationRecoveryExecutionPort>,
        );

        assert_eq!(
            effect
                .prepare(&executing(&fixture), fixture.owner, fixture.context)
                .await
                .unwrap(),
            PrepareEffectOutcome::Prepared {
                transaction: fixture.request.transaction()
            }
        );
        assert_eq!(state.resolves.load(Ordering::SeqCst), 1);
        assert_eq!(state.classifications.load(Ordering::SeqCst), 0);
        assert!(state.persists.lock().unwrap().is_empty());
        assert_eq!(state.loads.load(Ordering::SeqCst), 0);
        assert_eq!(commits.calls.load(Ordering::SeqCst), 0);
        assert_eq!(recovery.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn prepare_and_commit_reject_a_substituted_recovery_owner() {
        let fixture = fixture(85).await;
        let substituted = ExecutionOwner {
            actor_id: ActorId::from_bytes(id16(86)),
            fence: WriterFence {
                lease_id: LeaseId::from_bytes(id16(87)),
                generation: WriterGeneration::new(
                    fixture.owner.fence.generation.get().checked_add(1).unwrap(),
                )
                .unwrap(),
            },
        };
        let substituted_context = ReductionContext {
            current_head: fixture.context.current_head,
            active_fence: substituted.fence,
        };
        let state = Arc::new(MockState::new(&fixture));
        let commits = Arc::new(StaticCommit {
            outcome: ActivationTransactionOutcome::ReconciliationNeeded(fixture.ticket),
            calls: AtomicUsize::new(0),
        });
        let recovery = Arc::new(StaticRecovery {
            outcome: ActivationTransactionOutcome::ReconciliationNeeded(fixture.ticket),
            expected_ticket: fixture.ticket,
            expected_active_fence: substituted.fence,
            calls: AtomicUsize::new(0),
        });
        let effect = ActivationCommandEffect::new(
            Arc::clone(&state) as Arc<dyn ActivationCommandStatePort>,
            Arc::clone(&commits) as Arc<dyn ActivationCommitExecutionPort>,
            Arc::clone(&recovery) as Arc<dyn ActivationRecoveryExecutionPort>,
        );

        assert_eq!(
            effect
                .prepare(&executing(&fixture), substituted, substituted_context)
                .await
                .unwrap_err(),
            CommandPortError::ContextUnavailable
        );
        assert_eq!(
            effect
                .commit(
                    &prepared(&fixture),
                    substituted,
                    fixture.request.transaction(),
                    substituted_context,
                )
                .await
                .unwrap_err(),
            CommandPortError::ContextUnavailable
        );
        assert_eq!(state.resolves.load(Ordering::SeqCst), 0);
        assert_eq!(commits.calls.load(Ordering::SeqCst), 0);
        assert_eq!(recovery.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn commit_persists_the_exact_unknown_ticket_without_recovery() {
        let fixture = fixture(90).await;
        let state = Arc::new(MockState::new(&fixture));
        let commits = Arc::new(StaticCommit {
            outcome: ActivationTransactionOutcome::ReconciliationNeeded(fixture.ticket),
            calls: AtomicUsize::new(0),
        });
        let recovery = Arc::new(StaticRecovery {
            outcome: ActivationTransactionOutcome::ReconciliationNeeded(fixture.ticket),
            expected_ticket: fixture.ticket,
            expected_active_fence: fixture.owner.fence,
            calls: AtomicUsize::new(0),
        });
        let effect = ActivationCommandEffect::new(
            Arc::clone(&state) as Arc<dyn ActivationCommandStatePort>,
            Arc::clone(&commits) as Arc<dyn ActivationCommitExecutionPort>,
            Arc::clone(&recovery) as Arc<dyn ActivationRecoveryExecutionPort>,
        );

        assert_eq!(
            effect
                .commit(
                    &prepared(&fixture),
                    fixture.owner,
                    fixture.request.transaction(),
                    fixture.context,
                )
                .await
                .unwrap(),
            CommitEffectOutcome::Unknown {
                unknown: fixture.unknown
            }
        );
        assert_eq!(commits.calls.load(Ordering::SeqCst), 1);
        assert_eq!(recovery.calls.load(Ordering::SeqCst), 0);
        assert_eq!(*state.persists.lock().unwrap(), [fixture.ticket]);
    }

    #[tokio::test]
    async fn commit_maps_only_the_exact_direct_activation_receipt() {
        let fixture = fixture(93).await;
        let state = Arc::new(MockState::new(&fixture));
        let commits = Arc::new(StaticCommit {
            outcome: ActivationTransactionOutcome::Activated(activated_receipt(
                &fixture,
                fixture.request.execution_fence(),
                None,
            )),
            calls: AtomicUsize::new(0),
        });
        let recovery = Arc::new(StaticRecovery {
            outcome: ActivationTransactionOutcome::ReconciliationNeeded(fixture.ticket),
            expected_ticket: fixture.ticket,
            expected_active_fence: fixture.owner.fence,
            calls: AtomicUsize::new(0),
        });
        let effect = ActivationCommandEffect::new(
            Arc::clone(&state) as Arc<dyn ActivationCommandStatePort>,
            Arc::clone(&commits) as Arc<dyn ActivationCommitExecutionPort>,
            Arc::clone(&recovery) as Arc<dyn ActivationRecoveryExecutionPort>,
        );

        assert_eq!(
            effect
                .commit(
                    &prepared(&fixture),
                    fixture.owner,
                    fixture.request.transaction(),
                    fixture.context,
                )
                .await
                .unwrap(),
            CommitEffectOutcome::Committed {
                result: CommandResult::EpochActivated {
                    epoch: fixture.request.pins().epoch,
                    selection: fixture.request.operation_selection(),
                }
            }
        );
        assert_eq!(commits.calls.load(Ordering::SeqCst), 1);
        assert!(state.persists.lock().unwrap().is_empty());
        assert_eq!(recovery.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn commit_rejects_a_plausible_receipt_bound_to_another_transaction() {
        let fixture = fixture(95).await;
        let mut mismatched = activated_receipt(&fixture, fixture.request.execution_fence(), None);
        mismatched.cache.transaction = TransactionRef::from_bytes(id32(96));
        let state = Arc::new(MockState::new(&fixture));
        let commits = Arc::new(StaticCommit {
            outcome: ActivationTransactionOutcome::Activated(mismatched),
            calls: AtomicUsize::new(0),
        });
        let recovery = Arc::new(StaticRecovery {
            outcome: ActivationTransactionOutcome::ReconciliationNeeded(fixture.ticket),
            expected_ticket: fixture.ticket,
            expected_active_fence: fixture.owner.fence,
            calls: AtomicUsize::new(0),
        });
        let effect = ActivationCommandEffect::new(
            Arc::clone(&state) as Arc<dyn ActivationCommandStatePort>,
            Arc::clone(&commits) as Arc<dyn ActivationCommitExecutionPort>,
            Arc::clone(&recovery) as Arc<dyn ActivationRecoveryExecutionPort>,
        );

        assert_eq!(
            effect
                .commit(
                    &prepared(&fixture),
                    fixture.owner,
                    fixture.request.transaction(),
                    fixture.context,
                )
                .await
                .unwrap_err(),
            CommandPortError::CorruptRecord
        );
        assert_eq!(commits.calls.load(Ordering::SeqCst), 1);
        assert_eq!(state.classifications.load(Ordering::SeqCst), 0);
        assert!(state.persists.lock().unwrap().is_empty());
        assert_eq!(recovery.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn known_nonselection_uses_the_application_owned_failure_mapping() {
        let fixture = fixture(100).await;
        let stopped = ActivationNotSelected {
            stage: ActivationTransactionStage::CandidateProof,
            reason: ActivationNotSelectedReason::ProofUnknown(DiagnosticRef::from_bytes(id32(110))),
            operation_id: fixture.request.command().identity.operation_id,
            candidate_epoch: fixture.request.pins().epoch,
        };
        let state = Arc::new(MockState::new(&fixture));
        let commits = Arc::new(StaticCommit {
            outcome: ActivationTransactionOutcome::NotSelected(stopped),
            calls: AtomicUsize::new(0),
        });
        let recovery = Arc::new(StaticRecovery {
            outcome: ActivationTransactionOutcome::ReconciliationNeeded(fixture.ticket),
            expected_ticket: fixture.ticket,
            expected_active_fence: fixture.owner.fence,
            calls: AtomicUsize::new(0),
        });
        let effect = ActivationCommandEffect::new(
            Arc::clone(&state) as Arc<dyn ActivationCommandStatePort>,
            Arc::clone(&commits) as Arc<dyn ActivationCommitExecutionPort>,
            Arc::clone(&recovery) as Arc<dyn ActivationRecoveryExecutionPort>,
        );

        assert_eq!(
            effect
                .commit(
                    &prepared(&fixture),
                    fixture.owner,
                    fixture.request.transaction(),
                    fixture.context,
                )
                .await
                .unwrap(),
            CommitEffectOutcome::KnownFailure {
                failure: fixture.failure
            }
        );
        assert_eq!(state.classifications.load(Ordering::SeqCst), 1);
        assert!(state.persists.lock().unwrap().is_empty());
        assert_eq!(recovery.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn reconcile_uses_marker_recovery_only_and_preserves_evidence() {
        let fixture = fixture(120).await;
        let recovery_owner = ExecutionOwner {
            actor_id: ActorId::from_bytes(id16(121)),
            fence: WriterFence {
                lease_id: LeaseId::from_bytes(id16(122)),
                generation: WriterGeneration::new(
                    fixture.owner.fence.generation.get().checked_add(1).unwrap(),
                )
                .unwrap(),
            },
        };
        let recovery_context = ReductionContext {
            current_head: fixture.context.current_head,
            active_fence: recovery_owner.fence,
        };
        let state = Arc::new(MockState::new(&fixture));
        let commits = Arc::new(StaticCommit {
            outcome: ActivationTransactionOutcome::ReconciliationNeeded(fixture.ticket),
            calls: AtomicUsize::new(0),
        });
        let recovery = Arc::new(StaticRecovery {
            outcome: ActivationTransactionOutcome::Activated(activated_receipt(
                &fixture,
                recovery_owner.fence,
                Some(fixture.evidence),
            )),
            expected_ticket: fixture.ticket,
            expected_active_fence: recovery_owner.fence,
            calls: AtomicUsize::new(0),
        });
        let effect = ActivationCommandEffect::new(
            Arc::clone(&state) as Arc<dyn ActivationCommandStatePort>,
            Arc::clone(&commits) as Arc<dyn ActivationCommitExecutionPort>,
            Arc::clone(&recovery) as Arc<dyn ActivationRecoveryExecutionPort>,
        );

        assert_eq!(
            effect
                .reconcile(
                    &awaiting(&fixture),
                    recovery_owner,
                    fixture.request.transaction(),
                    recovery_context,
                )
                .await
                .unwrap(),
            ReconciliationObservation::Committed {
                evidence: fixture.evidence,
                result: CommandResult::EpochActivated {
                    epoch: fixture.request.pins().epoch,
                    selection: fixture.request.operation_selection(),
                },
            }
        );
        assert_eq!(state.loads.load(Ordering::SeqCst), 1);
        assert_eq!(state.resolves.load(Ordering::SeqCst), 0);
        assert!(state.persists.lock().unwrap().is_empty());
        assert_eq!(commits.calls.load(Ordering::SeqCst), 0);
        assert_eq!(recovery.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn reconcile_accepts_a_durable_acknowledgement_from_an_earlier_recovery_generation() {
        let fixture = fixture(125).await;
        let acknowledgement_fence = WriterFence {
            lease_id: LeaseId::from_bytes(id16(126)),
            generation: WriterGeneration::new(fixture.owner.fence.generation.get() + 1).unwrap(),
        };
        let recovery_owner = ExecutionOwner {
            actor_id: ActorId::from_bytes(id16(127)),
            fence: WriterFence {
                lease_id: LeaseId::from_bytes(id16(128)),
                generation: WriterGeneration::new(fixture.owner.fence.generation.get() + 2)
                    .unwrap(),
            },
        };
        let recovery_context = ReductionContext {
            current_head: fixture.context.current_head,
            active_fence: recovery_owner.fence,
        };
        let state = Arc::new(MockState::new(&fixture));
        let commits = Arc::new(StaticCommit {
            outcome: ActivationTransactionOutcome::ReconciliationNeeded(fixture.ticket),
            calls: AtomicUsize::new(0),
        });
        let recovery = Arc::new(StaticRecovery {
            outcome: ActivationTransactionOutcome::Activated(
                activated_receipt_with_acknowledgement_fence(
                    &fixture,
                    recovery_owner.fence,
                    acknowledgement_fence,
                    Some(fixture.evidence),
                ),
            ),
            expected_ticket: fixture.ticket,
            expected_active_fence: recovery_owner.fence,
            calls: AtomicUsize::new(0),
        });
        let effect = ActivationCommandEffect::new(
            Arc::clone(&state) as Arc<dyn ActivationCommandStatePort>,
            Arc::clone(&commits) as Arc<dyn ActivationCommitExecutionPort>,
            Arc::clone(&recovery) as Arc<dyn ActivationRecoveryExecutionPort>,
        );

        assert!(matches!(
            effect
                .reconcile(
                    &awaiting(&fixture),
                    recovery_owner,
                    fixture.request.transaction(),
                    recovery_context,
                )
                .await,
            Ok(ReconciliationObservation::Committed { .. })
        ));
        assert_eq!(recovery.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn reconcile_rejects_a_loaded_request_from_another_reducer_attempt() {
        let seed = 170;
        let fixture = fixture(seed).await;
        let stale_attempt = ActivationAttempt::for_test(
            *fixture.request.command(),
            2,
            ExecutionOwner {
                actor_id: ActorId::from_bytes(id16(198)),
                fence: fixture.owner.fence,
            },
        );
        let stale_request = ActivationTransactionRequest::try_new(
            stale_attempt,
            Arc::clone(fixture.request.candidate()),
            fixture.request.pins(),
            fixture.request.event_id(),
            CompatibilityClassRef::from_bytes(id32(seed.wrapping_add(17))),
            RetentionPolicyRef::from_bytes(id32(seed.wrapping_add(18))),
            fixture.request.operation_selection(),
            fixture.request.transaction(),
            fixture.request.control_relation().clone(),
        )
        .unwrap();
        let stale_resolved = ResolvedActivationRecovery::try_new(
            stale_request.recovery_request(),
            fixture.request.transaction(),
        )
        .unwrap();
        let mut state = MockState::new(&fixture);
        state.loaded =
            LoadedActivationReconciliation::try_new(stale_resolved, fixture.ticket).unwrap();
        let state = Arc::new(state);
        let commits = Arc::new(StaticCommit {
            outcome: ActivationTransactionOutcome::ReconciliationNeeded(fixture.ticket),
            calls: AtomicUsize::new(0),
        });
        let recovery = Arc::new(StaticRecovery {
            outcome: ActivationTransactionOutcome::Activated(activated_receipt(
                &fixture,
                fixture.owner.fence,
                Some(fixture.evidence),
            )),
            expected_ticket: fixture.ticket,
            expected_active_fence: fixture.owner.fence,
            calls: AtomicUsize::new(0),
        });
        let effect = ActivationCommandEffect::new(
            Arc::clone(&state) as Arc<dyn ActivationCommandStatePort>,
            Arc::clone(&commits) as Arc<dyn ActivationCommitExecutionPort>,
            Arc::clone(&recovery) as Arc<dyn ActivationRecoveryExecutionPort>,
        );

        assert_eq!(
            effect
                .reconcile(
                    &awaiting(&fixture),
                    fixture.owner,
                    fixture.request.transaction(),
                    fixture.context,
                )
                .await,
            Err(CommandPortError::CorruptRecord)
        );
        assert_eq!(state.loads.load(Ordering::SeqCst), 1);
        assert_eq!(recovery.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn reconcile_maps_only_explicit_marker_nonselection_to_not_committed() {
        let fixture = fixture(140).await;
        let evidence = ReconciliationEvidenceRef::from_bytes(id32(141));
        let state = Arc::new(MockState::new(&fixture));
        let commits = Arc::new(StaticCommit {
            outcome: ActivationTransactionOutcome::ReconciliationNeeded(fixture.ticket),
            calls: AtomicUsize::new(0),
        });
        let recovery = Arc::new(StaticRecovery {
            outcome: ActivationTransactionOutcome::NotSelected(ActivationNotSelected {
                stage: ActivationTransactionStage::DurableAppendReadback,
                reason: ActivationNotSelectedReason::OperationMarkerProvedNotSelected(evidence),
                operation_id: fixture.request.command().identity.operation_id,
                candidate_epoch: fixture.request.pins().epoch,
            }),
            expected_ticket: fixture.ticket,
            expected_active_fence: fixture.owner.fence,
            calls: AtomicUsize::new(0),
        });
        let effect = ActivationCommandEffect::new(
            Arc::clone(&state) as Arc<dyn ActivationCommandStatePort>,
            Arc::clone(&commits) as Arc<dyn ActivationCommitExecutionPort>,
            Arc::clone(&recovery) as Arc<dyn ActivationRecoveryExecutionPort>,
        );

        assert_eq!(
            effect
                .reconcile(
                    &awaiting(&fixture),
                    fixture.owner,
                    fixture.request.transaction(),
                    fixture.context,
                )
                .await
                .unwrap(),
            ReconciliationObservation::NotCommitted { evidence }
        );
        assert_eq!(commits.calls.load(Ordering::SeqCst), 0);
        assert_eq!(recovery.calls.load(Ordering::SeqCst), 1);
        assert!(state.persists.lock().unwrap().is_empty());
    }

    #[test]
    fn reconciliation_binding_rejects_ticket_identity_drift() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let fixture = runtime.block_on(fixture(160));
        let resolved = ResolvedActivationRecovery::try_new(
            fixture.request.recovery_request(),
            fixture.request.transaction(),
        )
        .unwrap();
        let mut drifted = fixture.ticket;
        drifted.event_id = ActivationEventId::from_bytes(id32(161));

        assert_eq!(
            LoadedActivationReconciliation::try_new(resolved, drifted).unwrap_err(),
            ActivationCommandBindingError::TicketMismatch
        );
    }
}
