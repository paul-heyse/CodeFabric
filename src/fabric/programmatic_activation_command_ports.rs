//! Production activation-command resolution over explicit typed state.
//!
//! The adapters in this module deliberately do not own activation transaction sequencing.
//! [`super::activation_transaction::ActivationTransactionCoordinator`] remains the only forward
//! coordinator, and its recovery counterpart remains the only marker-driven recovery
//! coordinator. This module supplies immutable command requests and reconciliation state.

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;

use super::activation::{
    ActivationAttempt, ActivationControlRelationPin, ActivationEventId, CompatibilityClassRef,
    FabricEpochPins,
};
use super::activation_command_effect::{
    ActivationCommandBindingError, ActivationCommandStatePort, LoadedActivationReconciliation,
    PersistedActivationReconciliation, ResolvedActivationRecovery, ResolvedActivationTransaction,
};
use super::activation_transaction::{
    ActivationNotSelected, ActivationReconciliationTicket, ActivationRecoveryRequest,
    ActivationTransactionRequest,
};
use super::command::{
    CommandFailure, CommandRecord, DurableCommandState, ExecutionOwner, FabricCommand,
    ReconciliationEvidenceRef, ReductionContext, RetentionPolicyRef, TransactionRef, UnknownCommit,
};
use super::command_actor::CommandPortError;
use super::command_effect_contract::reconciliation_attempt;
use super::programmatic_epoch::ProgrammaticFabricEpoch;

/// Exact immutable command key used to resolve one activation candidate.
///
/// The complete command is retained rather than only its operation ID. A storage adapter that
/// accidentally returns a row from another authorization, predecessor, fence, semantic pin, or
/// resource envelope is therefore rejected before a transaction request is constructed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationCommandRequestKey {
    command: FabricCommand,
}

impl ActivationCommandRequestKey {
    #[must_use]
    pub const fn new(command: FabricCommand) -> Self {
        Self { command }
    }

    #[must_use]
    pub const fn command(self) -> FabricCommand {
        self.command
    }
}

/// Explicit application-owned data needed to construct one immutable activation request.
///
/// This value is a typed relation/store result. It does not infer a candidate, transaction,
/// compatibility class, retention policy, operation selection, or control relation from a
/// command payload. The canonical request constructor revalidates every cross-field invariant at
/// resolution time.
#[derive(Clone, Debug)]
pub struct ActivationCommandRequestMaterial {
    key: ActivationCommandRequestKey,
    candidate: Arc<ProgrammaticFabricEpoch>,
    pins: FabricEpochPins,
    event_id: ActivationEventId,
    compatibility: CompatibilityClassRef,
    retention: RetentionPolicyRef,
    operation_selection: super::command::OperationSelectionRef,
    transaction: TransactionRef,
    control_relation: ActivationControlRelationPin,
}

impl ActivationCommandRequestMaterial {
    /// Retain one complete typed request row. Validity is checked against the reducer-issued
    /// [`ActivationAttempt`] by [`ExactActivationCommandState::resolve_request`].
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        key: ActivationCommandRequestKey,
        candidate: Arc<ProgrammaticFabricEpoch>,
        pins: FabricEpochPins,
        event_id: ActivationEventId,
        compatibility: CompatibilityClassRef,
        retention: RetentionPolicyRef,
        operation_selection: super::command::OperationSelectionRef,
        transaction: TransactionRef,
        control_relation: ActivationControlRelationPin,
    ) -> Self {
        Self {
            key,
            candidate,
            pins,
            event_id,
            compatibility,
            retention,
            operation_selection,
            transaction,
            control_relation,
        }
    }

    #[must_use]
    pub const fn key(&self) -> ActivationCommandRequestKey {
        self.key
    }

    #[must_use]
    pub const fn pins(&self) -> FabricEpochPins {
        self.pins
    }

    #[must_use]
    pub const fn candidate(&self) -> &Arc<ProgrammaticFabricEpoch> {
        &self.candidate
    }

    #[must_use]
    pub const fn event_id(&self) -> ActivationEventId {
        self.event_id
    }

    #[must_use]
    pub const fn compatibility(&self) -> CompatibilityClassRef {
        self.compatibility
    }

    #[must_use]
    pub const fn retention(&self) -> RetentionPolicyRef {
        self.retention
    }

    #[must_use]
    pub const fn operation_selection(&self) -> super::command::OperationSelectionRef {
        self.operation_selection
    }

    #[must_use]
    pub const fn transaction(&self) -> TransactionRef {
        self.transaction
    }

    #[must_use]
    pub const fn control_relation(&self) -> &ActivationControlRelationPin {
        &self.control_relation
    }

    fn resolve(
        self,
        attempt: &ActivationAttempt,
    ) -> Result<ResolvedActivationTransaction, CommandPortError> {
        if self.key.command != *attempt.command() {
            return Err(CommandPortError::CorruptRecord);
        }
        let transaction = self.transaction;
        let request = ActivationTransactionRequest::try_new(
            *attempt,
            self.candidate,
            self.pins,
            self.event_id,
            self.compatibility,
            self.retention,
            self.operation_selection,
            transaction,
            self.control_relation,
        )
        .map_err(|_| CommandPortError::CorruptRecord)?;
        ResolvedActivationTransaction::try_new(request, transaction)
            .map_err(|_| CommandPortError::CorruptRecord)
    }
}

/// Exact lookup key for an application-owned not-selected classification row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationNotSelectedClassificationQuery {
    request: ActivationCommandRequestKey,
    transaction: TransactionRef,
    stopped: ActivationNotSelected,
}

impl ActivationNotSelectedClassificationQuery {
    fn from_resolved(
        resolved: &ResolvedActivationTransaction,
        stopped: ActivationNotSelected,
    ) -> Self {
        Self {
            request: ActivationCommandRequestKey::new(*resolved.request().command()),
            transaction: resolved.transaction(),
            stopped,
        }
    }

    #[must_use]
    pub const fn request(self) -> ActivationCommandRequestKey {
        self.request
    }

    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.transaction
    }

    #[must_use]
    pub const fn stopped(self) -> ActivationNotSelected {
        self.stopped
    }
}

/// Explicit failure classification returned by the application relation/store.
///
/// The diagnostic identity is supplied in `failure`; this adapter never hashes a Rust enum name
/// or debug representation into a diagnostic reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationNotSelectedClassification {
    query: ActivationNotSelectedClassificationQuery,
    failure: CommandFailure,
}

impl ActivationNotSelectedClassification {
    #[must_use]
    pub const fn new(
        query: ActivationNotSelectedClassificationQuery,
        failure: CommandFailure,
    ) -> Self {
        Self { query, failure }
    }

    #[must_use]
    pub const fn query(self) -> ActivationNotSelectedClassificationQuery {
        self.query
    }

    #[must_use]
    pub const fn failure(self) -> CommandFailure {
        self.failure
    }
}

/// Persistable primitive request/ticket row submitted to temporal reconciliation storage.
///
/// The reducer-issued [`ActivationAttempt`] is deliberately absent. Durable adapters receive
/// only identities and immutable request values which can survive process restart; the exact
/// adapter reconstructs the recovery request from a newly reducer-validated attempt on load.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivationReconciliationWrite {
    command: FabricCommand,
    attempt: u32,
    execution_owner: ExecutionOwner,
    pins: FabricEpochPins,
    event_id: ActivationEventId,
    compatibility: CompatibilityClassRef,
    retention: RetentionPolicyRef,
    operation_selection: super::command::OperationSelectionRef,
    transaction: TransactionRef,
    control_relation: ActivationControlRelationPin,
    ticket: ActivationReconciliationTicket,
}

impl ActivationReconciliationWrite {
    /// Validate the ticket against the complete candidate-free recovery request before any
    /// temporal record is written.
    ///
    /// # Errors
    ///
    /// Returns [`ActivationCommandBindingError`] when the transaction or any exact ticket
    /// identity differs from the candidate-free recovery request.
    pub fn try_new(
        request: ActivationRecoveryRequest,
        ticket: ActivationReconciliationTicket,
    ) -> Result<Self, ActivationCommandBindingError> {
        let resolved = ResolvedActivationRecovery::try_new(request.clone(), request.transaction())?;
        LoadedActivationReconciliation::try_new(resolved, ticket)?;
        Ok(Self {
            command: *request.command(),
            attempt: request.attempt().attempt(),
            execution_owner: request.attempt().execution_owner(),
            pins: request.pins(),
            event_id: request.event_id(),
            compatibility: request.compatibility(),
            retention: request.retention(),
            operation_selection: request.operation_selection(),
            transaction: request.transaction(),
            control_relation: request.control_relation().clone(),
            ticket,
        })
    }

    /// Recreate a persistable row after exact SQLite decoding.
    ///
    /// This does not recreate or authorize an [`ActivationAttempt`]. The exact state adapter
    /// validates every field against a freshly reducer-issued attempt before constructing a
    /// recovery request.
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn from_persisted_primitives(
        command: FabricCommand,
        attempt: u32,
        execution_owner: ExecutionOwner,
        pins: FabricEpochPins,
        event_id: ActivationEventId,
        compatibility: CompatibilityClassRef,
        retention: RetentionPolicyRef,
        operation_selection: super::command::OperationSelectionRef,
        transaction: TransactionRef,
        control_relation: ActivationControlRelationPin,
        ticket: ActivationReconciliationTicket,
    ) -> Self {
        Self {
            command,
            attempt,
            execution_owner,
            pins,
            event_id,
            compatibility,
            retention,
            operation_selection,
            transaction,
            control_relation,
            ticket,
        }
    }

    #[must_use]
    pub const fn command(&self) -> FabricCommand {
        self.command
    }

    #[must_use]
    pub const fn attempt(&self) -> u32 {
        self.attempt
    }

    #[must_use]
    pub const fn execution_owner(&self) -> ExecutionOwner {
        self.execution_owner
    }

    #[must_use]
    pub const fn pins(&self) -> FabricEpochPins {
        self.pins
    }

    #[must_use]
    pub const fn event_id(&self) -> ActivationEventId {
        self.event_id
    }

    #[must_use]
    pub const fn compatibility(&self) -> CompatibilityClassRef {
        self.compatibility
    }

    #[must_use]
    pub const fn retention(&self) -> RetentionPolicyRef {
        self.retention
    }

    #[must_use]
    pub const fn operation_selection(&self) -> super::command::OperationSelectionRef {
        self.operation_selection
    }

    #[must_use]
    pub const fn transaction(&self) -> TransactionRef {
        self.transaction
    }

    #[must_use]
    pub const fn control_relation(&self) -> &ActivationControlRelationPin {
        &self.control_relation
    }

    #[must_use]
    pub const fn ticket(&self) -> ActivationReconciliationTicket {
        self.ticket
    }
}

/// Exact temporal row returned after reconciliation persistence or lookup.
///
/// Both the unknown-commit diagnostic and evidence identity are application-owned data. They are
/// deliberately required even though the command effect uses only the evidence on a subsequent
/// indeterminate recovery pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivationReconciliationRecord {
    write: ActivationReconciliationWrite,
    unknown: UnknownCommit,
    evidence: ReconciliationEvidenceRef,
}

impl ActivationReconciliationRecord {
    #[must_use]
    pub const fn new(
        write: ActivationReconciliationWrite,
        unknown: UnknownCommit,
        evidence: ReconciliationEvidenceRef,
    ) -> Self {
        Self {
            write,
            unknown,
            evidence,
        }
    }

    #[must_use]
    pub const fn write(&self) -> &ActivationReconciliationWrite {
        &self.write
    }

    #[must_use]
    pub const fn unknown(&self) -> UnknownCommit {
        self.unknown
    }

    #[must_use]
    pub const fn evidence(&self) -> ReconciliationEvidenceRef {
        self.evidence
    }
}

/// Complete fenced read request for one previously persisted reconciliation row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationReconciliationRead {
    command: FabricCommand,
    attempt: u32,
    execution_owner: ExecutionOwner,
    active_recovery_owner: ExecutionOwner,
    transaction: TransactionRef,
    context: ReductionContext,
    unknown: UnknownCommit,
    last_evidence: Option<ReconciliationEvidenceRef>,
}

impl ActivationReconciliationRead {
    #[must_use]
    pub const fn command(self) -> FabricCommand {
        self.command
    }

    #[must_use]
    pub const fn attempt(self) -> u32 {
        self.attempt
    }

    #[must_use]
    pub const fn execution_owner(self) -> ExecutionOwner {
        self.execution_owner
    }

    #[must_use]
    pub const fn active_recovery_owner(self) -> ExecutionOwner {
        self.active_recovery_owner
    }

    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.transaction
    }

    #[must_use]
    pub const fn context(self) -> ReductionContext {
        self.context
    }

    #[must_use]
    pub const fn unknown(self) -> UnknownCommit {
        self.unknown
    }

    #[must_use]
    pub const fn last_evidence(self) -> Option<ReconciliationEvidenceRef> {
        self.last_evidence
    }
}

/// Application-owned immutable request/classification relations plus temporal reconciliation
/// storage.
///
/// There is intentionally no in-memory or default implementation here. A production composition
/// must supply the durable implementation and must distinguish a missing row (`Ok(None)`) from a
/// proved negative result.
#[async_trait]
pub trait ActivationCommandStateStore: Send + Sync {
    async fn read_request(
        &self,
        key: ActivationCommandRequestKey,
    ) -> Result<Option<ActivationCommandRequestMaterial>, CommandPortError>;

    async fn read_not_selected_classification(
        &self,
        query: ActivationNotSelectedClassificationQuery,
    ) -> Result<Option<ActivationNotSelectedClassification>, CommandPortError>;

    async fn persist_reconciliation(
        &self,
        write: ActivationReconciliationWrite,
    ) -> Result<ActivationReconciliationRecord, CommandPortError>;

    async fn read_reconciliation(
        &self,
        query: ActivationReconciliationRead,
    ) -> Result<Option<ActivationReconciliationRecord>, CommandPortError>;
}

/// Fail-closed production adapter from typed application state to the activation command effect.
pub struct ExactActivationCommandState {
    store: Arc<dyn ActivationCommandStateStore>,
}

impl fmt::Debug for ExactActivationCommandState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactActivationCommandState")
            .field("store", &"installed")
            .finish()
    }
}

impl ExactActivationCommandState {
    /// Install the required typed state store. No fallback or empty-success representation exists.
    #[must_use]
    pub const fn new(store: Arc<dyn ActivationCommandStateStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl ActivationCommandStatePort for ExactActivationCommandState {
    async fn resolve_request(
        &self,
        record: &CommandRecord,
        attempt: ActivationAttempt,
        _context: ReductionContext,
    ) -> Result<ResolvedActivationTransaction, CommandPortError> {
        if record.command() != attempt.command() {
            return Err(CommandPortError::CorruptRecord);
        }
        let key = ActivationCommandRequestKey::new(*record.command());
        let material = self
            .store
            .read_request(key)
            .await?
            .ok_or(CommandPortError::ContextUnavailable)?;
        if material.key() != key {
            return Err(CommandPortError::CorruptRecord);
        }
        material.resolve(&attempt)
    }

    async fn classify_not_selected(
        &self,
        resolved: &ResolvedActivationTransaction,
        stopped: ActivationNotSelected,
    ) -> Result<CommandFailure, CommandPortError> {
        let query = ActivationNotSelectedClassificationQuery::from_resolved(resolved, stopped);
        let classification = self
            .store
            .read_not_selected_classification(query)
            .await?
            .ok_or(CommandPortError::ContextUnavailable)?;
        if classification.query() != query {
            return Err(CommandPortError::CorruptRecord);
        }
        Ok(classification.failure())
    }

    async fn persist_reconciliation(
        &self,
        resolved: &ResolvedActivationRecovery,
        ticket: ActivationReconciliationTicket,
    ) -> Result<PersistedActivationReconciliation, CommandPortError> {
        let write = ActivationReconciliationWrite::try_new(resolved.request().clone(), ticket)
            .map_err(|_| CommandPortError::CorruptRecord)?;
        let persisted = self.store.persist_reconciliation(write.clone()).await?;
        if persisted.write() != &write {
            return Err(CommandPortError::CorruptRecord);
        }
        Ok(PersistedActivationReconciliation {
            unknown: persisted.unknown(),
            evidence: persisted.evidence(),
        })
    }

    async fn load_reconciliation(
        &self,
        record: &CommandRecord,
        owner: ExecutionOwner,
        transaction: TransactionRef,
        context: ReductionContext,
    ) -> Result<LoadedActivationReconciliation, CommandPortError> {
        let recovery = reconciliation_attempt(
            record,
            owner,
            transaction,
            context,
            super::command::CommandKind::ActivateEpoch,
        )?;
        let DurableCommandState::AwaitingReconciliation {
            attempt,
            execution_owner,
            unknown,
            last_evidence,
            ..
        } = record.state()
        else {
            return Err(CommandPortError::CorruptRecord);
        };
        let query = ActivationReconciliationRead {
            command: *record.command(),
            attempt,
            execution_owner,
            active_recovery_owner: owner,
            transaction,
            context,
            unknown,
            last_evidence,
        };
        let stored = self
            .store
            .read_reconciliation(query)
            .await?
            .ok_or(CommandPortError::ContextUnavailable)?;
        if stored.unknown() != unknown
            || last_evidence.is_some_and(|evidence| evidence != stored.evidence())
        {
            return Err(CommandPortError::CorruptRecord);
        }

        let write = stored.write();
        let expected_attempt = ActivationAttempt::from_validated(recovery.attempt());
        if write.command() != *record.command()
            || write.attempt() != attempt
            || write.attempt() != expected_attempt.attempt()
            || write.execution_owner() != execution_owner
            || write.execution_owner() != expected_attempt.execution_owner()
            || write.transaction() != transaction
        {
            return Err(CommandPortError::CorruptRecord);
        }
        let request = ActivationRecoveryRequest::try_new(
            expected_attempt,
            write.pins(),
            write.event_id(),
            write.compatibility(),
            write.retention(),
            write.operation_selection(),
            write.transaction(),
            write.control_relation().clone(),
        )
        .map_err(|_| CommandPortError::CorruptRecord)?;
        if request.command() != record.command()
            || request.transaction() != transaction
            || request.execution_fence() != execution_owner.fence
            || request.pins() != write.pins()
            || request.event_id() != write.event_id()
            || request.compatibility() != write.compatibility()
            || request.retention() != write.retention()
            || request.operation_selection() != write.operation_selection()
            || request.control_relation() != write.control_relation()
        {
            return Err(CommandPortError::CorruptRecord);
        }
        let resolved = ResolvedActivationRecovery::try_new(request, transaction)
            .map_err(|_| CommandPortError::CorruptRecord)?;
        LoadedActivationReconciliation::try_new(resolved, write.ticket())
            .map_err(|_| CommandPortError::CorruptRecord)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::fabric::activation::{
        ActivationControlRelationPin, OverlaySegmentSetRef, PolicySetRef,
        SealedActivationControlBinding, TableVersionSetRef,
    };
    use crate::fabric::activation_transaction::{
        ActivationAdmissionPosture, ActivationReconciliationReason, ActivationTransactionStage,
        DurableSelectionKnowledge,
    };
    use crate::fabric::command::{
        ActorId, AdmissionContext, AuthorizationDecision, AuthorizationRef, CommandEvent,
        CommandIdentity, CommandOwnership, CommandPins, CommandReducer, DiagnosticRef, EpochId,
        ExpectedHead, FabricCommandPayload, IdempotencyKey, InputReleaseRef, LeaseId, OperationId,
        OperationSelectionRef, PrincipalId, ProgramReleaseRef, ProofReceiptRef, ProviderSetRef,
        ResourceEnvelopeRef, SourceGeneration, UnknownCommitReason, WorkspaceId, WriterFence,
        WriterGeneration,
    };
    use crate::fabric::delta_exact::ExactDeltaPin;
    use url::Url;

    const fn id16(seed: u8) -> [u8; 16] {
        [seed; 16]
    }

    const fn id32(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn fence(seed: u8, generation: u64) -> WriterFence {
        WriterFence {
            lease_id: LeaseId::from_bytes(id16(seed)),
            generation: WriterGeneration::new(generation).expect("nonzero generation"),
        }
    }

    fn control_relation() -> ActivationControlRelationPin {
        ActivationControlRelationPin::new(
            ExactDeltaPin::new(
                &Url::parse("memory:///codefabric/programmatic-activation-state").unwrap(),
                7,
            )
            .unwrap(),
            SealedActivationControlBinding::for_test(
                "programmatic-activation-state-session",
                "binding.system.activation-control.delta",
            ),
        )
    }

    struct StateFixture {
        command: FabricCommand,
        attempt: ActivationAttempt,
        pins: FabricEpochPins,
        event_id: ActivationEventId,
        compatibility: CompatibilityClassRef,
        retention: RetentionPolicyRef,
        selection: OperationSelectionRef,
        control_relation: ActivationControlRelationPin,
        owner: ExecutionOwner,
        context: ReductionContext,
        transaction: TransactionRef,
        ticket: ActivationReconciliationTicket,
        unknown: UnknownCommit,
        evidence: ReconciliationEvidenceRef,
    }

    fn state_fixture(seed: u8) -> StateFixture {
        let epoch = EpochId::from_bytes(id16(seed.wrapping_add(1)));
        let writer_fence = fence(seed.wrapping_add(2), u64::from(seed) + 1);
        let command = FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes(id16(seed.wrapping_add(3))),
                idempotency_key: IdempotencyKey::from_bytes(id32(seed.wrapping_add(4))),
            },
            ownership: CommandOwnership {
                workspace_id: WorkspaceId::from_bytes(id16(seed)),
                principal_id: PrincipalId::from_bytes(id16(seed.wrapping_add(5))),
                authorization: AuthorizationRef::from_bytes(id32(seed.wrapping_add(6))),
            },
            expected_head: ExpectedHead::Empty,
            writer_fence,
            pins: CommandPins {
                input_release: InputReleaseRef::from_bytes(id32(seed.wrapping_add(7))),
                program_release: ProgramReleaseRef::from_bytes(id32(seed.wrapping_add(8))),
                application_release: crate::fabric::command::ApplicationReleaseRef::from_bytes(
                    id32(seed.wrapping_add(8)),
                ),
                source_authority: crate::fabric::command::SourceAuthorityRef::from_bytes(id32(
                    seed.wrapping_add(8),
                )),
                provider_release: crate::fabric::command::ProviderReleaseRef::from_bytes(id32(
                    seed.wrapping_add(8),
                )),
                source_generation: SourceGeneration::new(u64::from(seed) + 9),
                provider_set: ProviderSetRef::from_bytes(id32(seed.wrapping_add(10))),
            },
            resources: ResourceEnvelopeRef::from_bytes(id32(seed.wrapping_add(11))),
            payload: FabricCommandPayload::ActivateEpoch {
                candidate_epoch: epoch,
                proof_receipt: ProofReceiptRef::from_bytes(id32(seed.wrapping_add(12))),
            },
        };
        let owner = ExecutionOwner {
            actor_id: ActorId::from_bytes(id16(seed.wrapping_add(13))),
            fence: writer_fence,
        };
        let context = ReductionContext {
            current_head: command.expected_head,
            active_fence: writer_fence,
        };
        let attempt = ActivationAttempt::for_test(command, 1, owner);
        let pins = FabricEpochPins {
            epoch,
            input_release: command.pins.input_release,
            program_release: command.pins.program_release,
            application_release: command.pins.application_release,
            source_authority: command.pins.source_authority,
            provider_release: command.pins.provider_release,
            source_generation: command.pins.source_generation,
            provider_set: command.pins.provider_set,
            table_versions: TableVersionSetRef::from_bytes(id32(seed.wrapping_add(23))),
            overlay_segments: OverlaySegmentSetRef::from_bytes(id32(seed.wrapping_add(14))),
            policy_set: PolicySetRef::from_bytes(id32(seed.wrapping_add(15))),
            resource_envelope: command.resources,
            proof_receipt: ProofReceiptRef::from_bytes(id32(seed.wrapping_add(12))),
        };
        let event_id = ActivationEventId::from_bytes(id32(seed.wrapping_add(16)));
        let transaction = TransactionRef::from_bytes(id32(seed.wrapping_add(17)));
        let selection = OperationSelectionRef::from_bytes(id32(seed.wrapping_add(18)));
        let compatibility = CompatibilityClassRef::from_bytes(id32(seed.wrapping_add(19)));
        let retention = RetentionPolicyRef::from_bytes(id32(seed.wrapping_add(20)));
        let control_relation = control_relation();
        let ticket = ActivationReconciliationTicket {
            stage: ActivationTransactionStage::AuthorityRevalidation,
            reason: ActivationReconciliationReason::AuthorityStale,
            workspace_id: command.ownership.workspace_id,
            operation_id: command.identity.operation_id,
            candidate_epoch: epoch,
            expected_head: command.expected_head,
            execution_fence: writer_fence,
            event_id,
            transaction,
            operation_selection: selection,
            durable_selection: DurableSelectionKnowledge::NotAttempted,
            admission_posture: ActivationAdmissionPosture::Closed,
        };
        StateFixture {
            command,
            attempt,
            pins,
            event_id,
            compatibility,
            retention,
            selection,
            control_relation,
            owner,
            context,
            transaction,
            ticket,
            unknown: UnknownCommit {
                reason: UnknownCommitReason::ReadbackUnavailable,
                diagnostic: DiagnosticRef::from_bytes(id32(seed.wrapping_add(21))),
            },
            evidence: ReconciliationEvidenceRef::from_bytes(id32(seed.wrapping_add(22))),
        }
    }

    struct StaticStateStore {
        request: Mutex<Option<ActivationCommandRequestMaterial>>,
        reconciliation: Mutex<Option<ActivationReconciliationRecord>>,
        unknown: UnknownCommit,
        evidence: ReconciliationEvidenceRef,
    }

    #[async_trait]
    impl ActivationCommandStateStore for StaticStateStore {
        async fn read_request(
            &self,
            _key: ActivationCommandRequestKey,
        ) -> Result<Option<ActivationCommandRequestMaterial>, CommandPortError> {
            Ok(self.request.lock().unwrap().clone())
        }

        async fn read_not_selected_classification(
            &self,
            _query: ActivationNotSelectedClassificationQuery,
        ) -> Result<Option<ActivationNotSelectedClassification>, CommandPortError> {
            Ok(None)
        }

        async fn persist_reconciliation(
            &self,
            write: ActivationReconciliationWrite,
        ) -> Result<ActivationReconciliationRecord, CommandPortError> {
            let record = ActivationReconciliationRecord::new(write, self.unknown, self.evidence);
            *self.reconciliation.lock().unwrap() = Some(record.clone());
            Ok(record)
        }

        async fn read_reconciliation(
            &self,
            _query: ActivationReconciliationRead,
        ) -> Result<Option<ActivationReconciliationRecord>, CommandPortError> {
            Ok(self.reconciliation.lock().unwrap().clone())
        }
    }

    fn executing(fixture: &StateFixture) -> CommandRecord {
        let admitted = CommandReducer::admit(
            None,
            &fixture.command,
            AdmissionContext {
                workspace_id: fixture.command.ownership.workspace_id,
                current_head: fixture.context.current_head,
                active_fence: fixture.context.active_fence,
                authorization: AuthorizationDecision::Authorized(
                    fixture.command.ownership.authorization,
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

    #[tokio::test]
    async fn exact_state_rejects_absent_request_state() {
        let fixture = state_fixture(70);
        let store = Arc::new(StaticStateStore {
            request: Mutex::new(None),
            reconciliation: Mutex::new(None),
            unknown: fixture.unknown,
            evidence: fixture.evidence,
        });
        let state = ExactActivationCommandState::new(store);
        assert!(matches!(
            state
                .resolve_request(&executing(&fixture), fixture.attempt, fixture.context)
                .await,
            Err(CommandPortError::ContextUnavailable)
        ));
    }

    #[tokio::test]
    async fn reconciliation_storage_rejects_missing_and_mismatched_rows() {
        let fixture = state_fixture(90);
        let store = Arc::new(StaticStateStore {
            request: Mutex::new(None),
            reconciliation: Mutex::new(None),
            unknown: fixture.unknown,
            evidence: fixture.evidence,
        });
        let state = ExactActivationCommandState::new(store.clone());
        let recovery_request = ActivationRecoveryRequest::try_new(
            fixture.attempt,
            fixture.pins,
            fixture.event_id,
            fixture.compatibility,
            fixture.retention,
            fixture.selection,
            fixture.transaction,
            fixture.control_relation.clone(),
        )
        .unwrap();
        let recovery =
            ResolvedActivationRecovery::try_new(recovery_request, fixture.transaction).unwrap();
        let persisted = state
            .persist_reconciliation(&recovery, fixture.ticket)
            .await
            .unwrap();
        assert_eq!(persisted.unknown, fixture.unknown);
        assert_eq!(persisted.evidence, fixture.evidence);

        let prepared = CommandReducer::reduce(
            &executing(&fixture),
            CommandEvent::PrepareCommit {
                owner: fixture.owner,
                transaction: fixture.transaction,
            },
            fixture.context,
        )
        .unwrap()
        .record;
        let awaiting = CommandReducer::reduce(
            &prepared,
            CommandEvent::ReportUnknownCommit {
                owner: fixture.owner,
                transaction: fixture.transaction,
                unknown: fixture.unknown,
            },
            fixture.context,
        )
        .unwrap()
        .record;
        assert!(
            state
                .load_reconciliation(
                    &awaiting,
                    fixture.owner,
                    fixture.transaction,
                    fixture.context,
                )
                .await
                .is_ok()
        );

        let exact = store.reconciliation.lock().unwrap().clone().unwrap();
        *store.reconciliation.lock().unwrap() = Some(ActivationReconciliationRecord::new(
            exact.write().clone(),
            UnknownCommit {
                reason: UnknownCommitReason::ConnectionLost,
                diagnostic: DiagnosticRef::from_bytes(id32(123)),
            },
            exact.evidence(),
        ));
        assert!(matches!(
            state
                .load_reconciliation(
                    &awaiting,
                    fixture.owner,
                    fixture.transaction,
                    fixture.context,
                )
                .await,
            Err(CommandPortError::CorruptRecord)
        ));

        *store.reconciliation.lock().unwrap() = None;
        assert!(matches!(
            state
                .load_reconciliation(
                    &awaiting,
                    fixture.owner,
                    fixture.transaction,
                    fixture.context,
                )
                .await,
            Err(CommandPortError::ContextUnavailable)
        ));
    }
}
