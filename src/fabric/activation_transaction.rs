//! Ordered activation transaction for one already sealed fabric epoch.
//!
//! The coordinator is deliberately a protocol over application-owned ports. It does not own a
//! Delta writer, discover a latest table version, retry persistence, or treat the process-local
//! epoch cache as semantic history. A transaction reaches [`ActivationTransactionOutcome::Activated`]
//! only after exact proof, admission closure, predecessor/fence revalidation, durable append and
//! readback, atomic epoch publication, temporal-cache reconciliation, admission reopening, and an
//! exact acknowledgement have all completed in that order.

use std::fmt::Debug;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;

#[cfg(test)]
use super::activation::DurableActivationRow;
use super::activation::{
    ActivationAttempt, ActivationChain, ActivationCommit, ActivationControlRelationPin,
    ActivationError, ActivationEvent, ActivationEventId, ActivationOrdinal,
    ActivationRecoveryAttempt, CompatibilityClassRef, FabricEpochPins, TableVersionSet,
};
use super::admission::{
    ActivationBarrier, AdmissionError, FabricAdmissionRuntime, RecoverySelectionPublication,
};
use super::command::{
    DiagnosticRef, EpochId, ExpectedHead, FabricCommand, FabricCommandPayload, OperationId,
    OperationSelectionRef, ProofReceiptRef, ReconciliationEvidenceRef, RetentionPolicyRef,
    TransactionRef, WorkspaceId, WriterFence,
};
use super::programmatic_epoch::{ProgrammaticFabricEpoch, ProgrammaticFabricEpochBuilder};

/// Candidate proof result. Failure, unknown, and cancellation are all distinct from proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateProofOutcome {
    Proved { proof_receipt: ProofReceiptRef },
    Failed { diagnostic: DiagnosticRef },
    Unknown { diagnostic: DiagnosticRef },
    Cancelled { diagnostic: DiagnosticRef },
}

/// Exact immutable input presented to an independently owned candidate-proof implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateProofRequest {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub expected_head: ExpectedHead,
    pub execution_fence: WriterFence,
    pub pins: FabricEpochPins,
}

/// Independent proof authority. Implementations must prove the exact pins in the request.
#[async_trait]
pub trait ActivationCandidateProofPort: Send + Sync {
    async fn prove_candidate(&self, request: CandidateProofRequest) -> CandidateProofOutcome;
}

/// Validated durable activation history plus the current OS-backed writer fence, reread only
/// after admission has closed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivationAuthoritySnapshot {
    pub chain: ActivationChain,
    pub active_fence: WriterFence,
}

/// Revalidation result. `Stale`, `Unknown`, and `Cancelled` keep admission closed so recovery can
/// derive the authoritative outcome; none authorizes an append.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorityRevalidationOutcome {
    Valid(ActivationAuthoritySnapshot),
    Stale(ActivationAuthoritySnapshot),
    Unknown { diagnostic: DiagnosticRef },
    Cancelled { diagnostic: DiagnosticRef },
}

/// Exact authority lookup key supplied after the admission barrier has been established.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationAuthorityRequest {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub expected_head: ExpectedHead,
    pub execution_fence: WriterFence,
}

/// Durable activation-chain and lease/fence authority.
#[async_trait]
pub trait ActivationAuthorityPort: Send + Sync {
    async fn revalidate(&self, request: ActivationAuthorityRequest)
    -> AuthorityRevalidationOutcome;
}

/// Admission/cache operations needed by activation. The barrier type remains opaque to the
/// coordinator, which lets the production runtime bind it to one concrete runtime instance.
#[async_trait]
pub trait ActivationAdmissionPort: Send + Sync {
    type Barrier: Copy + Debug + Eq + Send + Sync;

    async fn close_admission(
        &self,
        expected_head: ExpectedHead,
        execution_fence: WriterFence,
    ) -> Result<Self::Barrier, AdmissionError>;

    async fn publish_selected_epoch(
        &self,
        barrier: Self::Barrier,
        chain_after_readback: &ActivationChain,
        candidate: Arc<ProgrammaticFabricEpoch>,
    ) -> Result<(), AdmissionError>;

    async fn reconcile_and_reopen(
        &self,
        barrier: Self::Barrier,
        reconciled_head: ExpectedHead,
    ) -> Result<(), AdmissionError>;

    async fn abort_proved_no_selection(
        &self,
        barrier: Self::Barrier,
        unchanged_chain: &ActivationChain,
    ) -> Result<(), AdmissionError>;
}

#[async_trait]
impl ActivationAdmissionPort for FabricAdmissionRuntime {
    type Barrier = ActivationBarrier;

    async fn close_admission(
        &self,
        expected_head: ExpectedHead,
        execution_fence: WriterFence,
    ) -> Result<Self::Barrier, AdmissionError> {
        FabricAdmissionRuntime::close_admission(self, expected_head, execution_fence)
    }

    async fn publish_selected_epoch(
        &self,
        barrier: Self::Barrier,
        chain_after_readback: &ActivationChain,
        candidate: Arc<ProgrammaticFabricEpoch>,
    ) -> Result<(), AdmissionError> {
        FabricAdmissionRuntime::publish_selected_epoch(
            self,
            barrier,
            chain_after_readback,
            candidate,
        )
    }

    async fn reconcile_and_reopen(
        &self,
        barrier: Self::Barrier,
        reconciled_head: ExpectedHead,
    ) -> Result<(), AdmissionError> {
        FabricAdmissionRuntime::reopen_after_reconciliation(self, barrier, reconciled_head)
    }

    async fn abort_proved_no_selection(
        &self,
        barrier: Self::Barrier,
        unchanged_chain: &ActivationChain,
    ) -> Result<(), AdmissionError> {
        FabricAdmissionRuntime::abort_before_selection(self, barrier, unchanged_chain)
    }
}

/// Exact append contract passed to the one durable activation-event writer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivationAppendContract {
    attempt: ActivationAttempt,
    command: FabricCommand,
    execution_fence: WriterFence,
    event_id: ActivationEventId,
    predecessor_event_id: Option<ActivationEventId>,
    ordinal: ActivationOrdinal,
    pins: FabricEpochPins,
    table_versions: Arc<TableVersionSet>,
    compatibility: CompatibilityClassRef,
    retention: RetentionPolicyRef,
    operation_selection: OperationSelectionRef,
    transaction: TransactionRef,
    control_relation: ActivationControlRelationPin,
}

impl ActivationAppendContract {
    #[cfg(test)]
    pub(crate) fn for_test(
        attempt: ActivationAttempt,
        row: DurableActivationRow,
        table_versions: Arc<TableVersionSet>,
        control_relation: ActivationControlRelationPin,
    ) -> Self {
        let command = *attempt.command();
        Self {
            attempt,
            command,
            execution_fence: row.execution_fence,
            event_id: row.event_id,
            predecessor_event_id: row.predecessor_event_id,
            ordinal: row.ordinal,
            pins: row.pins,
            table_versions,
            compatibility: row.compatibility,
            retention: row.retention,
            operation_selection: row.commit.operation_selection,
            transaction: row.commit.transaction,
            control_relation,
        }
    }

    #[must_use]
    pub const fn attempt(&self) -> ActivationAttempt {
        self.attempt
    }

    #[must_use]
    pub const fn command(&self) -> &FabricCommand {
        &self.command
    }

    #[must_use]
    pub const fn execution_fence(&self) -> WriterFence {
        self.execution_fence
    }

    #[must_use]
    pub const fn event_id(&self) -> ActivationEventId {
        self.event_id
    }

    #[must_use]
    pub const fn predecessor_event_id(&self) -> Option<ActivationEventId> {
        self.predecessor_event_id
    }

    #[must_use]
    pub const fn ordinal(&self) -> ActivationOrdinal {
        self.ordinal
    }

    #[must_use]
    pub const fn pins(&self) -> FabricEpochPins {
        self.pins
    }

    /// Reversible exact component vector whose canonical reference is carried
    /// by [`Self::pins`]. The durable adapter writes both in the same Delta
    /// control row.
    #[must_use]
    pub const fn table_versions(&self) -> &Arc<TableVersionSet> {
        &self.table_versions
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
    pub const fn operation_selection(&self) -> OperationSelectionRef {
        self.operation_selection
    }

    #[must_use]
    pub const fn transaction(&self) -> TransactionRef {
        self.transaction
    }

    /// Exact activation-control predecessor plus the sealed session/schema
    /// binding which must own the append plan.
    #[must_use]
    pub const fn control_relation(&self) -> &ActivationControlRelationPin {
        &self.control_relation
    }
}

/// Durable append/readback result. An adapter may report `NotCommitted` only with a validated
/// unchanged chain; every ambiguous transport, cancellation, timeout, or backend result is
/// `Unknown` and cannot be retried here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActivationAppendOutcome {
    Committed {
        event: ActivationEvent,
        table_versions: Arc<TableVersionSet>,
        chain_after_readback: ActivationChain,
    },
    NotCommitted {
        unchanged_chain: ActivationChain,
        reason: ActivationNotCommittedReason,
    },
    Unknown {
        reason: ActivationAppendUnknownReason,
        diagnostic: DiagnosticRef,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationNotCommittedReason {
    Rejected,
    CancelledBeforeCommit,
    PredecessorConflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationAppendUnknownReason {
    CommitOutcomeUnknown,
    ReadbackUnavailable,
    CancelledDuringCommit,
}

/// Sole append/readback authority for immutable activation events.
#[async_trait]
pub trait ActivationEventPort: Send + Sync {
    async fn append_and_readback(
        &self,
        contract: ActivationAppendContract,
    ) -> ActivationAppendOutcome;
}

/// Exact activation-reconciliation receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationCacheReceipt {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub event_id: ActivationEventId,
    pub selected_epoch: EpochId,
    pub active_fence: WriterFence,
    pub transaction: TransactionRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationCacheOutcome {
    Reconciled(ActivationCacheReceipt),
    Unknown { diagnostic: DiagnosticRef },
    Cancelled { diagnostic: DiagnosticRef },
}

/// Reconstructible receipt projection. It observes durable history and never selects semantic
/// current or caches any fabric data.
#[async_trait]
pub trait ActivationCachePort: Send + Sync {
    async fn reconcile_selected(
        &self,
        event: ActivationEvent,
        chain_after_readback: &ActivationChain,
        active_fence: WriterFence,
    ) -> ActivationCacheOutcome;
}

/// Exact terminal acknowledgement. This is downstream of reopening and cannot make a selection
/// authoritative by itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationAcknowledgementReceipt {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub event_id: ActivationEventId,
    pub selected_epoch: EpochId,
    pub active_fence: WriterFence,
    pub transaction: TransactionRef,
    pub operation_selection: OperationSelectionRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationAcknowledgementOutcome {
    Acknowledged(ActivationAcknowledgementReceipt),
    Unknown { diagnostic: DiagnosticRef },
    Cancelled { diagnostic: DiagnosticRef },
}

#[async_trait]
pub trait ActivationAcknowledgementPort: Send + Sync {
    async fn acknowledge(
        &self,
        event: ActivationEvent,
        active_fence: WriterFence,
    ) -> ActivationAcknowledgementOutcome;
}

/// Process-local, reconstructible projection of the last cache receipt.
/// Durable activation history remains the sole semantic authority.
pub struct ActivationReconciliationReceiptCache {
    workspace_id: WorkspaceId,
    current: RwLock<Option<ActivationCacheReceipt>>,
}

impl Debug for ActivationReconciliationReceiptCache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ActivationReconciliationReceiptCache")
            .field("workspace_id", &self.workspace_id)
            .field("current", &self.current_receipt().ok().flatten())
            .finish_non_exhaustive()
    }
}

impl ActivationReconciliationReceiptCache {
    #[must_use]
    pub const fn new(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            current: RwLock::new(None),
        }
    }

    /// Observe the current projection without interpreting absence as semantic
    /// non-selection.
    pub fn current_receipt(
        &self,
    ) -> Result<Option<ActivationCacheReceipt>, ActivationReconciliationReceiptCacheReadError> {
        self.current
            .read()
            .map(|current| *current)
            .map_err(|_| ActivationReconciliationReceiptCacheReadError::Poisoned)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ActivationReconciliationReceiptCacheReadError {
    #[error("activation reconciliation-receipt cache lock is poisoned")]
    Poisoned,
}

#[async_trait]
impl ActivationCachePort for ActivationReconciliationReceiptCache {
    async fn reconcile_selected(
        &self,
        event: ActivationEvent,
        chain_after_readback: &ActivationChain,
        active_fence: WriterFence,
    ) -> ActivationCacheOutcome {
        if event.workspace_id() != self.workspace_id
            || chain_after_readback.workspace_id() != self.workspace_id
            || chain_after_readback.head_event().copied() != Some(event)
            || chain_after_readback.current_head() != ExpectedHead::Epoch(event.pins().epoch)
            || !recovery_fence_authorizes(event.execution_fence(), active_fence)
        {
            return ActivationCacheOutcome::Unknown {
                diagnostic: activation_runtime_projection_diagnostic(
                    event,
                    active_fence,
                    b"cache-input-mismatch",
                ),
            };
        }
        let receipt = ActivationCacheReceipt {
            workspace_id: self.workspace_id,
            operation_id: event.operation_id(),
            event_id: event.event_id(),
            selected_epoch: event.pins().epoch,
            active_fence,
            transaction: event.commit().transaction,
        };
        let mut current = match self.current.write() {
            Ok(current) => current,
            Err(_) => {
                return ActivationCacheOutcome::Unknown {
                    diagnostic: activation_runtime_projection_diagnostic(
                        event,
                        active_fence,
                        b"cache-lock-poisoned",
                    ),
                };
            }
        };
        if current.is_some_and(|prior| {
            prior.workspace_id != self.workspace_id
                || !recovery_fence_authorizes(prior.active_fence, active_fence)
        }) {
            return ActivationCacheOutcome::Unknown {
                diagnostic: activation_runtime_projection_diagnostic(
                    event,
                    active_fence,
                    b"cache-fence-regression",
                ),
            };
        }
        *current = Some(receipt);
        ActivationCacheOutcome::Reconciled(receipt)
    }
}

/// Pure, retry-safe acknowledgement projection.
///
/// The surrounding command actor durably records the returned terminal
/// result. Repeating this projection after a crash has no side effect and
/// yields the same semantic receipt for the same event/fence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdempotentActivationAcknowledgements {
    workspace_id: WorkspaceId,
}

impl IdempotentActivationAcknowledgements {
    #[must_use]
    pub const fn new(workspace_id: WorkspaceId) -> Self {
        Self { workspace_id }
    }
}

#[async_trait]
impl ActivationAcknowledgementPort for IdempotentActivationAcknowledgements {
    async fn acknowledge(
        &self,
        event: ActivationEvent,
        active_fence: WriterFence,
    ) -> ActivationAcknowledgementOutcome {
        if event.workspace_id() != self.workspace_id
            || !recovery_fence_authorizes(event.execution_fence(), active_fence)
        {
            return ActivationAcknowledgementOutcome::Unknown {
                diagnostic: activation_runtime_projection_diagnostic(
                    event,
                    active_fence,
                    b"acknowledgement-input-mismatch",
                ),
            };
        }
        ActivationAcknowledgementOutcome::Acknowledged(ActivationAcknowledgementReceipt {
            workspace_id: self.workspace_id,
            operation_id: event.operation_id(),
            event_id: event.event_id(),
            selected_epoch: event.pins().epoch,
            active_fence,
            transaction: event.commit().transaction,
            operation_selection: event.commit().operation_selection,
        })
    }
}

fn activation_runtime_projection_diagnostic(
    event: ActivationEvent,
    active_fence: WriterFence,
    reason: &[u8],
) -> DiagnosticRef {
    fn frame(digest: &mut blake3::Hasher, bytes: &[u8]) {
        digest.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
        digest.update(bytes);
    }

    let mut digest = blake3::Hasher::new();
    digest.update(b"codefabric.activation-runtime-projection-diagnostic.v1\0");
    frame(&mut digest, event.workspace_id().as_bytes());
    frame(&mut digest, event.operation_id().as_bytes());
    frame(&mut digest, event.event_id().as_bytes());
    frame(&mut digest, active_fence.lease_id.as_bytes());
    frame(&mut digest, &active_fence.generation.get().to_be_bytes());
    frame(&mut digest, reason);
    DiagnosticRef::from_bytes(*digest.finalize().as_bytes())
}

/// Exact durable lookup key for reconciling an activation whose commit or
/// acknowledgement outcome is unknown. Recovery never retries the append
/// before this operation-marker/control-history query completes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivationOperationMarkerRequest {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub event_id: ActivationEventId,
    pub expected_head: ExpectedHead,
    pub execution_fence: WriterFence,
    pub active_recovery_fence: WriterFence,
    pub transaction: TransactionRef,
    pub operation_selection: OperationSelectionRef,
    pub control_relation: ActivationControlRelationPin,
}

/// Acknowledgement state observed in the same durable marker read. `Absent`
/// is authoritative only because it is paired with reconciliation evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationAcknowledgementMarker {
    Absent,
    Acknowledged(ActivationAcknowledgementReceipt),
}

/// Authoritative result of querying both the operation marker and the complete
/// activation control history. There is intentionally no bare `None`: proved
/// non-selection carries explicit durable evidence and the unchanged chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActivationOperationMarkerOutcome {
    Selected {
        event: ActivationEvent,
        table_versions: Arc<TableVersionSet>,
        chain_after_readback: ActivationChain,
        acknowledgement: ActivationAcknowledgementMarker,
        evidence: ReconciliationEvidenceRef,
    },
    ProvedNotSelected {
        unchanged_chain: ActivationChain,
        evidence: ReconciliationEvidenceRef,
    },
    Unknown {
        diagnostic: DiagnosticRef,
    },
}

/// Durable recovery query. Implementations bind the operation ID,
/// transaction marker, selection record, and complete activation chain in one
/// readback result.
#[async_trait]
pub trait ActivationOperationMarkerPort: Send + Sync {
    async fn read_operation_marker(
        &self,
        request: ActivationOperationMarkerRequest,
    ) -> ActivationOperationMarkerOutcome;
}

/// Exact durable selection presented to the epoch reconstruction boundary.
///
/// The selected event and reversible relation-version vector come from the
/// same activation-control readback. No process-local candidate is accepted as
/// an input, so restart reconstruction is causally downstream of durable
/// selection rather than a prerequisite for reading it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivationEpochRebuildRequest {
    pub event: ActivationEvent,
    pub table_versions: Arc<TableVersionSet>,
}

/// Result of rebuilding the selected sealed epoch from exact durable state.
#[derive(Clone, Debug)]
pub enum ActivationEpochRebuildOutcome {
    Rebuilt(Arc<ProgrammaticFabricEpoch>),
    Unknown { diagnostic: DiagnosticRef },
}

/// Cold-restart boundary for an activation-selected epoch.
#[async_trait]
pub trait ActivationEpochRebuilderPort: Send + Sync {
    async fn rebuild_selected_epoch(
        &self,
        request: ActivationEpochRebuildRequest,
    ) -> ActivationEpochRebuildOutcome;
}

/// Concrete exact-Delta rebuilder over an application-owned programmatic
/// builder recipe.
///
/// The recipe installs the exact provider inputs and transformations for the
/// selected compiler release. This adapter supplies only the durable epoch ID
/// and version vector, then delegates reconstruction to
/// [`ProgrammaticFabricEpochBuilder::reopen`].
pub struct ExactDeltaProgrammaticEpochRebuilder<F> {
    builder: F,
}

impl<F> ExactDeltaProgrammaticEpochRebuilder<F> {
    #[must_use]
    pub const fn new(builder: F) -> Self {
        Self { builder }
    }
}

#[async_trait]
impl<F, E> ActivationEpochRebuilderPort for ExactDeltaProgrammaticEpochRebuilder<F>
where
    F: Fn(EpochId) -> Result<ProgrammaticFabricEpochBuilder, E> + Send + Sync,
    E: std::fmt::Display + Send,
{
    async fn rebuild_selected_epoch(
        &self,
        request: ActivationEpochRebuildRequest,
    ) -> ActivationEpochRebuildOutcome {
        let selected_epoch = request.event.pins().epoch;
        let builder = match (self.builder)(selected_epoch) {
            Ok(builder) => builder,
            Err(error) => {
                return ActivationEpochRebuildOutcome::Unknown {
                    diagnostic: epoch_rebuild_diagnostic(&request, &error.to_string()),
                };
            }
        };
        match builder.reopen(Arc::clone(&request.table_versions)).await {
            Ok(epoch) => ActivationEpochRebuildOutcome::Rebuilt(Arc::new(epoch)),
            Err(error) => ActivationEpochRebuildOutcome::Unknown {
                diagnostic: epoch_rebuild_diagnostic(&request, &error.to_string()),
            },
        }
    }
}

/// Admission operations used only after marker/control-history reconciliation.
/// They can resume an in-process closed barrier or a fail-closed restarted
/// runtime without performing a second epoch swap.
#[async_trait]
pub trait ActivationRecoveryAdmissionPort: Send + Sync {
    async fn recover_selected_epoch(
        &self,
        expected_head: ExpectedHead,
        execution_fence: WriterFence,
        active_recovery_fence: WriterFence,
        event: ActivationEvent,
        chain_after_readback: &ActivationChain,
        candidate: Arc<ProgrammaticFabricEpoch>,
        allow_already_reopened: bool,
    ) -> Result<RecoverySelectionPublication, AdmissionError>;

    async fn reopen_recovered_selection(
        &self,
        event: ActivationEvent,
        chain_after_readback: &ActivationChain,
        active_recovery_fence: WriterFence,
    ) -> Result<(), AdmissionError>;

    async fn recover_proved_no_selection(
        &self,
        expected_head: ExpectedHead,
        execution_fence: WriterFence,
        active_recovery_fence: WriterFence,
        unchanged_chain: &ActivationChain,
    ) -> Result<(), AdmissionError>;
}

#[async_trait]
impl ActivationRecoveryAdmissionPort for FabricAdmissionRuntime {
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
        FabricAdmissionRuntime::recover_selected_epoch(
            self,
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
        FabricAdmissionRuntime::reopen_recovered_selection(
            self,
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
        FabricAdmissionRuntime::recover_proved_no_selection(
            self,
            expected_head,
            execution_fence,
            active_recovery_fence,
            unchanged_chain,
        )
    }
}

/// One sealed candidate and all event inputs fixed before transaction execution.
#[derive(Clone, Debug)]
pub struct ActivationTransactionRequest {
    attempt: ActivationAttempt,
    command: FabricCommand,
    execution_fence: WriterFence,
    candidate: Arc<ProgrammaticFabricEpoch>,
    pins: FabricEpochPins,
    event_id: ActivationEventId,
    compatibility: CompatibilityClassRef,
    retention: RetentionPolicyRef,
    operation_selection: OperationSelectionRef,
    transaction: TransactionRef,
    control_relation: ActivationControlRelationPin,
}

/// Durable activation inputs sufficient for marker-driven restart recovery.
///
/// Unlike [`ActivationTransactionRequest`], this value deliberately carries
/// no process-local candidate. Recovery first reads the durable marker and
/// version vector, then reconstructs the selected epoch through
/// [`ActivationEpochRebuilderPort`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivationRecoveryRequest {
    attempt: ActivationAttempt,
    command: FabricCommand,
    execution_fence: WriterFence,
    pins: FabricEpochPins,
    event_id: ActivationEventId,
    compatibility: CompatibilityClassRef,
    retention: RetentionPolicyRef,
    operation_selection: OperationSelectionRef,
    transaction: TransactionRef,
    control_relation: ActivationControlRelationPin,
}

impl ActivationTransactionRequest {
    /// Bind a reducer-validated execution attempt to one sealed candidate and exact event IDs.
    ///
    /// # Errors
    ///
    /// Rejects a non-selection attempt, candidate identity drift, proof drift, or disagreement
    /// between the event pins and the command's compiler/model/source/provider/resource pins. The
    /// caller cannot supply a raw fence: it is carried by the unforgeable activation attempt.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        attempt: ActivationAttempt,
        candidate: Arc<ProgrammaticFabricEpoch>,
        pins: FabricEpochPins,
        event_id: ActivationEventId,
        compatibility: CompatibilityClassRef,
        retention: RetentionPolicyRef,
        operation_selection: OperationSelectionRef,
        transaction: TransactionRef,
        control_relation: ActivationControlRelationPin,
    ) -> Result<Self, ActivationTransactionRequestError> {
        let (command, execution_fence, selected_epoch) = validate_request_contract(attempt, pins)?;
        if selected_epoch != pins.epoch || selected_epoch != *candidate.identity() {
            return Err(ActivationTransactionRequestError::CandidateIdentityMismatch);
        }
        if pins.table_versions != candidate.observation_publication().table_version_set_ref() {
            return Err(ActivationTransactionRequestError::CandidateTableVersionSetMismatch);
        }
        Ok(Self {
            attempt,
            command,
            execution_fence,
            candidate,
            pins,
            event_id,
            compatibility,
            retention,
            operation_selection,
            transaction,
            control_relation,
        })
    }

    #[must_use]
    pub const fn command(&self) -> &FabricCommand {
        &self.command
    }

    #[must_use]
    pub const fn execution_fence(&self) -> WriterFence {
        self.execution_fence
    }

    /// Reducer-validated attempt that authorizes this exact activation transaction.
    #[must_use]
    pub const fn attempt(&self) -> ActivationAttempt {
        self.attempt
    }

    #[must_use]
    pub const fn candidate(&self) -> &Arc<ProgrammaticFabricEpoch> {
        &self.candidate
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
    pub const fn operation_selection(&self) -> OperationSelectionRef {
        self.operation_selection
    }

    #[must_use]
    pub const fn transaction(&self) -> TransactionRef {
        self.transaction
    }

    /// Exact activation-control predecessor and sealed session/schema binding
    /// captured with this immutable transaction request.
    #[must_use]
    pub const fn control_relation(&self) -> &ActivationControlRelationPin {
        &self.control_relation
    }

    /// Drop process-local candidate authority while retaining the complete
    /// durable request contract needed by recovery.
    #[must_use]
    pub fn recovery_request(&self) -> ActivationRecoveryRequest {
        ActivationRecoveryRequest {
            attempt: self.attempt,
            command: self.command,
            execution_fence: self.execution_fence,
            pins: self.pins,
            event_id: self.event_id,
            compatibility: self.compatibility,
            retention: self.retention,
            operation_selection: self.operation_selection,
            transaction: self.transaction,
            control_relation: self.control_relation.clone(),
        }
    }
}

impl ActivationRecoveryRequest {
    /// Reconstitute one recovery request from durable command/ticket inputs.
    /// No candidate session or latest-version lookup participates.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        attempt: ActivationAttempt,
        pins: FabricEpochPins,
        event_id: ActivationEventId,
        compatibility: CompatibilityClassRef,
        retention: RetentionPolicyRef,
        operation_selection: OperationSelectionRef,
        transaction: TransactionRef,
        control_relation: ActivationControlRelationPin,
    ) -> Result<Self, ActivationTransactionRequestError> {
        let (command, execution_fence, _) = validate_request_contract(attempt, pins)?;
        Ok(Self {
            attempt,
            command,
            execution_fence,
            pins,
            event_id,
            compatibility,
            retention,
            operation_selection,
            transaction,
            control_relation,
        })
    }

    #[must_use]
    pub const fn command(&self) -> &FabricCommand {
        &self.command
    }

    #[must_use]
    pub const fn execution_fence(&self) -> WriterFence {
        self.execution_fence
    }

    #[must_use]
    pub const fn attempt(&self) -> ActivationAttempt {
        self.attempt
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
    pub const fn operation_selection(&self) -> OperationSelectionRef {
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
}

fn validate_request_contract(
    attempt: ActivationAttempt,
    pins: FabricEpochPins,
) -> Result<(FabricCommand, WriterFence, EpochId), ActivationTransactionRequestError> {
    let command = *attempt.command();
    let execution_fence = attempt.execution_owner().fence;
    let (selected_epoch, activation_proof) = match command.payload {
        FabricCommandPayload::ActivateEpoch {
            candidate_epoch,
            proof_receipt,
        } => (candidate_epoch, Some(proof_receipt)),
        FabricCommandPayload::RollbackEpoch { target_epoch, .. } => (target_epoch, None),
        _ => return Err(ActivationTransactionRequestError::CommandDoesNotSelectEpoch),
    };
    if selected_epoch != pins.epoch {
        return Err(ActivationTransactionRequestError::CandidateIdentityMismatch);
    }
    if activation_proof.is_some_and(|proof| proof != pins.proof_receipt) {
        return Err(ActivationTransactionRequestError::ProofReceiptMismatch);
    }
    if pins.compiler_release != command.pins.compiler_release
        || pins.model_head != command.pins.model_head
        || pins.source_generation != command.pins.source_generation
        || pins.provider_set != command.pins.provider_set
        || pins.resource_envelope != command.resources
    {
        return Err(ActivationTransactionRequestError::CommandPinMismatch);
    }
    Ok((command, execution_fence, selected_epoch))
}

/// Shared immutable request view used by forward and recovery result
/// validation. Candidate access is intentionally absent.
trait ActivationRequestView {
    fn command(&self) -> &FabricCommand;
    fn execution_fence(&self) -> WriterFence;
    fn pins(&self) -> FabricEpochPins;
    fn event_id(&self) -> ActivationEventId;
    fn operation_selection(&self) -> OperationSelectionRef;
    fn transaction(&self) -> TransactionRef;
    fn control_relation(&self) -> &ActivationControlRelationPin;
}

impl ActivationRequestView for ActivationTransactionRequest {
    fn command(&self) -> &FabricCommand {
        self.command()
    }

    fn execution_fence(&self) -> WriterFence {
        self.execution_fence()
    }

    fn pins(&self) -> FabricEpochPins {
        self.pins()
    }

    fn event_id(&self) -> ActivationEventId {
        self.event_id()
    }

    fn operation_selection(&self) -> OperationSelectionRef {
        self.operation_selection()
    }

    fn transaction(&self) -> TransactionRef {
        self.transaction()
    }

    fn control_relation(&self) -> &ActivationControlRelationPin {
        self.control_relation()
    }
}

impl ActivationRequestView for ActivationRecoveryRequest {
    fn command(&self) -> &FabricCommand {
        self.command()
    }

    fn execution_fence(&self) -> WriterFence {
        self.execution_fence()
    }

    fn pins(&self) -> FabricEpochPins {
        self.pins()
    }

    fn event_id(&self) -> ActivationEventId {
        self.event_id()
    }

    fn operation_selection(&self) -> OperationSelectionRef {
        self.operation_selection()
    }

    fn transaction(&self) -> TransactionRef {
        self.transaction()
    }

    fn control_relation(&self) -> &ActivationControlRelationPin {
        self.control_relation()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ActivationTransactionRequestError {
    #[error("activation transaction command does not select an epoch")]
    CommandDoesNotSelectEpoch,
    #[error("selected epoch, sealed candidate, and event pins differ")]
    CandidateIdentityMismatch,
    #[error("activation table-version-set pin differs from the sealed candidate publication")]
    CandidateTableVersionSetMismatch,
    #[error("activation command and candidate proof receipts differ")]
    ProofReceiptMismatch,
    #[error("activation command and candidate compiler/model/source/provider/resource pins differ")]
    CommandPinMismatch,
}

/// Ordered boundary reached when a transaction stops or requires recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationTransactionStage {
    CandidateProof,
    AdmissionClosure,
    AuthorityRevalidation,
    DurableAppendReadback,
    EpochRebuild,
    EpochSwap,
    CacheReconciliation,
    AdmissionReopen,
    Acknowledgement,
}

/// Exact knowledge about the durable selection at the point recovery became necessary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableSelectionKnowledge {
    NotAttempted,
    Unknown,
    ReadBack { event_id: ActivationEventId },
}

/// Process-local gate/cache posture retained in a recovery ticket.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationAdmissionPosture {
    NeverClosed,
    Closed,
    Swapped,
    Reopened,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationNotSelectedReason {
    ProofFailed(DiagnosticRef),
    ProofUnknown(DiagnosticRef),
    CancelledBeforeClosure(DiagnosticRef),
    ProofReceiptMismatch,
    AdmissionRejected(AdmissionError),
    AppendProvedNotCommitted(ActivationNotCommittedReason),
    OperationMarkerProvedNotSelected(ReconciliationEvidenceRef),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationReadbackViolation {
    EventContract(ActivationError),
    EventDiffersFromContract,
    TableVersionSetMismatch,
    ChainWorkspaceMismatch,
    EventIsNotUniqueHead,
    ChainHeadMismatch,
    UnchangedChainMismatch,
    CacheReceiptMismatch,
    AcknowledgementReceiptMismatch,
    AuthorityMismatch,
    RecoveryTicketMismatch,
    OperationMarkerMismatch,
    AcknowledgementMarkerMismatch,
    RecoveryAttemptMismatch,
    RecoveryFenceNotAuthorized,
    RebuiltEpochMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationReconciliationReason {
    AuthorityStale,
    AuthorityUnknown(DiagnosticRef),
    CancelledAfterClosure(DiagnosticRef),
    AppendUnknown {
        reason: ActivationAppendUnknownReason,
        diagnostic: DiagnosticRef,
    },
    ReadbackViolation(ActivationReadbackViolation),
    AdmissionFailure(AdmissionError),
    CacheUnknown(DiagnosticRef),
    CacheCancelled(DiagnosticRef),
    AcknowledgementUnknown(DiagnosticRef),
    AcknowledgementCancelled(DiagnosticRef),
    OperationMarkerUnknown(DiagnosticRef),
    EpochRebuildUnknown(DiagnosticRef),
}

/// Fully pinned instruction for deterministic recovery. It never authorizes a persistence retry;
/// recovery must reread the operation marker and activation chain first.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationReconciliationTicket {
    pub stage: ActivationTransactionStage,
    pub reason: ActivationReconciliationReason,
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub candidate_epoch: EpochId,
    pub expected_head: ExpectedHead,
    pub execution_fence: WriterFence,
    pub event_id: ActivationEventId,
    pub transaction: TransactionRef,
    pub operation_selection: OperationSelectionRef,
    pub durable_selection: DurableSelectionKnowledge,
    pub admission_posture: ActivationAdmissionPosture,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationNotSelected {
    pub stage: ActivationTransactionStage,
    pub reason: ActivationNotSelectedReason,
    pub operation_id: OperationId,
    pub candidate_epoch: EpochId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivatedEpochReceipt {
    pub event: ActivationEvent,
    pub cache: ActivationCacheReceipt,
    pub acknowledgement: ActivationAcknowledgementReceipt,
    pub reconciliation_evidence: Option<ReconciliationEvidenceRef>,
}

/// Terminal coordinator result. Only `Activated` is success; reconciliation tickets explicitly
/// retain whether durable selection is unknown or already read back.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationTransactionOutcome {
    Activated(ActivatedEpochReceipt),
    NotSelected(ActivationNotSelected),
    ReconciliationNeeded(ActivationReconciliationTicket),
}

/// Ordered activation protocol. Generic ports make sequencing independently testable while the
/// production admission implementation retains its opaque runtime-bound barrier.
pub struct ActivationTransactionCoordinator<A, P, H, D, C, K> {
    admission: Arc<A>,
    proof: Arc<P>,
    authority: Arc<H>,
    durable_events: Arc<D>,
    cache: Arc<C>,
    acknowledgements: Arc<K>,
}

impl<A, P, H, D, C, K> ActivationTransactionCoordinator<A, P, H, D, C, K> {
    #[must_use]
    pub const fn new(
        admission: Arc<A>,
        proof: Arc<P>,
        authority: Arc<H>,
        durable_events: Arc<D>,
        cache: Arc<C>,
        acknowledgements: Arc<K>,
    ) -> Self {
        Self {
            admission,
            proof,
            authority,
            durable_events,
            cache,
            acknowledgements,
        }
    }
}

impl<A, P, H, D, C, K> ActivationTransactionCoordinator<A, P, H, D, C, K>
where
    A: ActivationAdmissionPort,
    P: ActivationCandidateProofPort,
    H: ActivationAuthorityPort,
    D: ActivationEventPort,
    C: ActivationCachePort,
    K: ActivationAcknowledgementPort,
{
    /// Execute exactly one activation attempt without internal persistence retry.
    pub async fn activate(
        &self,
        request: ActivationTransactionRequest,
    ) -> ActivationTransactionOutcome {
        let proof = self.proof.prove_candidate(proof_request(&request)).await;
        match proof {
            CandidateProofOutcome::Proved { proof_receipt }
                if proof_receipt == request.pins.proof_receipt => {}
            CandidateProofOutcome::Proved { .. } => {
                return not_selected(
                    &request,
                    ActivationTransactionStage::CandidateProof,
                    ActivationNotSelectedReason::ProofReceiptMismatch,
                );
            }
            CandidateProofOutcome::Failed { diagnostic } => {
                return not_selected(
                    &request,
                    ActivationTransactionStage::CandidateProof,
                    ActivationNotSelectedReason::ProofFailed(diagnostic),
                );
            }
            CandidateProofOutcome::Unknown { diagnostic } => {
                return not_selected(
                    &request,
                    ActivationTransactionStage::CandidateProof,
                    ActivationNotSelectedReason::ProofUnknown(diagnostic),
                );
            }
            CandidateProofOutcome::Cancelled { diagnostic } => {
                return not_selected(
                    &request,
                    ActivationTransactionStage::CandidateProof,
                    ActivationNotSelectedReason::CancelledBeforeClosure(diagnostic),
                );
            }
        }

        let barrier = match self
            .admission
            .close_admission(request.command.expected_head, request.execution_fence)
            .await
        {
            Ok(barrier) => barrier,
            Err(error) => {
                return not_selected(
                    &request,
                    ActivationTransactionStage::AdmissionClosure,
                    ActivationNotSelectedReason::AdmissionRejected(error),
                );
            }
        };

        let authority = match self.authority.revalidate(authority_request(&request)).await {
            AuthorityRevalidationOutcome::Valid(snapshot)
                if authority_matches(&request, &snapshot) =>
            {
                snapshot
            }
            AuthorityRevalidationOutcome::Valid(_) | AuthorityRevalidationOutcome::Stale(_) => {
                return reconciliation(
                    &request,
                    ActivationTransactionStage::AuthorityRevalidation,
                    ActivationReconciliationReason::AuthorityStale,
                    DurableSelectionKnowledge::NotAttempted,
                    ActivationAdmissionPosture::Closed,
                );
            }
            AuthorityRevalidationOutcome::Unknown { diagnostic } => {
                return reconciliation(
                    &request,
                    ActivationTransactionStage::AuthorityRevalidation,
                    ActivationReconciliationReason::AuthorityUnknown(diagnostic),
                    DurableSelectionKnowledge::NotAttempted,
                    ActivationAdmissionPosture::Closed,
                );
            }
            AuthorityRevalidationOutcome::Cancelled { diagnostic } => {
                return reconciliation(
                    &request,
                    ActivationTransactionStage::AuthorityRevalidation,
                    ActivationReconciliationReason::CancelledAfterClosure(diagnostic),
                    DurableSelectionKnowledge::NotAttempted,
                    ActivationAdmissionPosture::Closed,
                );
            }
        };

        let Some(contract) = append_contract(&request, &authority) else {
            return reconciliation(
                &request,
                ActivationTransactionStage::AuthorityRevalidation,
                ActivationReconciliationReason::ReadbackViolation(
                    ActivationReadbackViolation::AuthorityMismatch,
                ),
                DurableSelectionKnowledge::NotAttempted,
                ActivationAdmissionPosture::Closed,
            );
        };

        let selection = match self
            .durable_events
            .append_and_readback(contract.clone())
            .await
        {
            ActivationAppendOutcome::Committed {
                event,
                table_versions,
                chain_after_readback,
            } => {
                if let Err(violation) = validate_committed_selection(
                    &contract,
                    event,
                    &table_versions,
                    &chain_after_readback,
                ) {
                    return reconciliation(
                        &request,
                        ActivationTransactionStage::DurableAppendReadback,
                        ActivationReconciliationReason::ReadbackViolation(violation),
                        DurableSelectionKnowledge::Unknown,
                        ActivationAdmissionPosture::Closed,
                    );
                }
                (event, chain_after_readback)
            }
            ActivationAppendOutcome::NotCommitted {
                unchanged_chain,
                reason,
            } => {
                if !unchanged_chain_matches(&request, &authority, &unchanged_chain) {
                    return reconciliation(
                        &request,
                        ActivationTransactionStage::DurableAppendReadback,
                        ActivationReconciliationReason::ReadbackViolation(
                            ActivationReadbackViolation::UnchangedChainMismatch,
                        ),
                        DurableSelectionKnowledge::Unknown,
                        ActivationAdmissionPosture::Closed,
                    );
                }
                if let Err(error) = self
                    .admission
                    .abort_proved_no_selection(barrier, &unchanged_chain)
                    .await
                {
                    return reconciliation(
                        &request,
                        ActivationTransactionStage::AdmissionReopen,
                        ActivationReconciliationReason::AdmissionFailure(error),
                        DurableSelectionKnowledge::NotAttempted,
                        ActivationAdmissionPosture::Closed,
                    );
                }
                return not_selected(
                    &request,
                    ActivationTransactionStage::DurableAppendReadback,
                    ActivationNotSelectedReason::AppendProvedNotCommitted(reason),
                );
            }
            ActivationAppendOutcome::Unknown { reason, diagnostic } => {
                return reconciliation(
                    &request,
                    ActivationTransactionStage::DurableAppendReadback,
                    ActivationReconciliationReason::AppendUnknown { reason, diagnostic },
                    DurableSelectionKnowledge::Unknown,
                    ActivationAdmissionPosture::Closed,
                );
            }
        };
        let (event, chain_after_readback) = selection;
        let durable_selection = DurableSelectionKnowledge::ReadBack {
            event_id: event.event_id(),
        };

        if let Err(error) = self
            .admission
            .publish_selected_epoch(
                barrier,
                &chain_after_readback,
                Arc::clone(&request.candidate),
            )
            .await
        {
            return reconciliation(
                &request,
                ActivationTransactionStage::EpochSwap,
                ActivationReconciliationReason::AdmissionFailure(error),
                durable_selection,
                ActivationAdmissionPosture::Closed,
            );
        }

        let cache = match self
            .cache
            .reconcile_selected(event, &chain_after_readback, request.execution_fence)
            .await
        {
            ActivationCacheOutcome::Reconciled(receipt)
                if cache_receipt_matches(&request, event, request.execution_fence, receipt) =>
            {
                receipt
            }
            ActivationCacheOutcome::Reconciled(_) => {
                return reconciliation(
                    &request,
                    ActivationTransactionStage::CacheReconciliation,
                    ActivationReconciliationReason::ReadbackViolation(
                        ActivationReadbackViolation::CacheReceiptMismatch,
                    ),
                    durable_selection,
                    ActivationAdmissionPosture::Swapped,
                );
            }
            ActivationCacheOutcome::Unknown { diagnostic } => {
                return reconciliation(
                    &request,
                    ActivationTransactionStage::CacheReconciliation,
                    ActivationReconciliationReason::CacheUnknown(diagnostic),
                    durable_selection,
                    ActivationAdmissionPosture::Swapped,
                );
            }
            ActivationCacheOutcome::Cancelled { diagnostic } => {
                return reconciliation(
                    &request,
                    ActivationTransactionStage::CacheReconciliation,
                    ActivationReconciliationReason::CacheCancelled(diagnostic),
                    durable_selection,
                    ActivationAdmissionPosture::Swapped,
                );
            }
        };

        if let Err(error) = self
            .admission
            .reconcile_and_reopen(barrier, ExpectedHead::Epoch(request.pins.epoch))
            .await
        {
            return reconciliation(
                &request,
                ActivationTransactionStage::AdmissionReopen,
                ActivationReconciliationReason::AdmissionFailure(error),
                durable_selection,
                ActivationAdmissionPosture::Swapped,
            );
        }

        let acknowledgement = match self
            .acknowledgements
            .acknowledge(event, request.execution_fence)
            .await
        {
            ActivationAcknowledgementOutcome::Acknowledged(receipt)
                if acknowledgement_matches(&request, event, request.execution_fence, receipt) =>
            {
                receipt
            }
            ActivationAcknowledgementOutcome::Acknowledged(_) => {
                return reconciliation(
                    &request,
                    ActivationTransactionStage::Acknowledgement,
                    ActivationReconciliationReason::ReadbackViolation(
                        ActivationReadbackViolation::AcknowledgementReceiptMismatch,
                    ),
                    durable_selection,
                    ActivationAdmissionPosture::Reopened,
                );
            }
            ActivationAcknowledgementOutcome::Unknown { diagnostic } => {
                return reconciliation(
                    &request,
                    ActivationTransactionStage::Acknowledgement,
                    ActivationReconciliationReason::AcknowledgementUnknown(diagnostic),
                    durable_selection,
                    ActivationAdmissionPosture::Reopened,
                );
            }
            ActivationAcknowledgementOutcome::Cancelled { diagnostic } => {
                return reconciliation(
                    &request,
                    ActivationTransactionStage::Acknowledgement,
                    ActivationReconciliationReason::AcknowledgementCancelled(diagnostic),
                    durable_selection,
                    ActivationAdmissionPosture::Reopened,
                );
            }
        };

        ActivationTransactionOutcome::Activated(ActivatedEpochReceipt {
            event,
            cache,
            acknowledgement,
            reconciliation_evidence: None,
        })
    }
}

/// Marker-driven recovery for one interrupted activation. It has no append
/// port by construction, so recovery cannot blindly retry the selection
/// commit. The only durable input is the operation-marker/control-history
/// readback.
pub struct ActivationRecoveryCoordinator<A, M, R, C, K> {
    admission: Arc<A>,
    operation_markers: Arc<M>,
    epoch_rebuilder: Arc<R>,
    cache: Arc<C>,
    acknowledgements: Arc<K>,
}

impl<A, M, R, C, K> ActivationRecoveryCoordinator<A, M, R, C, K> {
    #[must_use]
    pub const fn new(
        admission: Arc<A>,
        operation_markers: Arc<M>,
        epoch_rebuilder: Arc<R>,
        cache: Arc<C>,
        acknowledgements: Arc<K>,
    ) -> Self {
        Self {
            admission,
            operation_markers,
            epoch_rebuilder,
            cache,
            acknowledgements,
        }
    }
}

impl<A, M, R, C, K> ActivationRecoveryCoordinator<A, M, R, C, K>
where
    A: ActivationRecoveryAdmissionPort,
    M: ActivationOperationMarkerPort,
    R: ActivationEpochRebuilderPort,
    C: ActivationCachePort,
    K: ActivationAcknowledgementPort,
{
    /// Reconcile one interrupted activation without persistence retry.
    ///
    /// The ticket is treated only as a lookup instruction. Durable marker and
    /// chain readback decide whether the epoch was selected.
    pub async fn recover(
        &self,
        request: ActivationRecoveryRequest,
        ticket: ActivationReconciliationTicket,
        recovery: ActivationRecoveryAttempt,
    ) -> ActivationTransactionOutcome {
        if request.attempt != recovery.attempt() {
            return reconciliation(
                &request,
                ticket.stage,
                ActivationReconciliationReason::ReadbackViolation(
                    ActivationReadbackViolation::RecoveryAttemptMismatch,
                ),
                DurableSelectionKnowledge::Unknown,
                ticket.admission_posture,
            );
        }
        let active_recovery_fence = recovery.active_recovery_owner().fence;
        if !recovery_ticket_matches(&request, ticket) {
            return reconciliation(
                &request,
                ticket.stage,
                ActivationReconciliationReason::ReadbackViolation(
                    ActivationReadbackViolation::RecoveryTicketMismatch,
                ),
                DurableSelectionKnowledge::Unknown,
                ticket.admission_posture,
            );
        }
        if !recovery_fence_authorizes(request.execution_fence, active_recovery_fence) {
            return reconciliation(
                &request,
                ticket.stage,
                ActivationReconciliationReason::ReadbackViolation(
                    ActivationReadbackViolation::RecoveryFenceNotAuthorized,
                ),
                DurableSelectionKnowledge::Unknown,
                ticket.admission_posture,
            );
        }

        let marker = self
            .operation_markers
            .read_operation_marker(operation_marker_request(&request, active_recovery_fence))
            .await;
        match marker {
            ActivationOperationMarkerOutcome::Unknown { diagnostic } => reconciliation(
                &request,
                ActivationTransactionStage::DurableAppendReadback,
                ActivationReconciliationReason::OperationMarkerUnknown(diagnostic),
                ticket.durable_selection,
                ticket.admission_posture,
            ),
            ActivationOperationMarkerOutcome::ProvedNotSelected {
                unchanged_chain,
                evidence,
            } => {
                if matches!(
                    ticket.durable_selection,
                    DurableSelectionKnowledge::ReadBack { .. }
                ) {
                    return reconciliation(
                        &request,
                        ActivationTransactionStage::DurableAppendReadback,
                        ActivationReconciliationReason::ReadbackViolation(
                            ActivationReadbackViolation::OperationMarkerMismatch,
                        ),
                        DurableSelectionKnowledge::Unknown,
                        ticket.admission_posture,
                    );
                }
                if !recovered_unchanged_chain_matches(&request, &unchanged_chain) {
                    return reconciliation(
                        &request,
                        ActivationTransactionStage::DurableAppendReadback,
                        ActivationReconciliationReason::ReadbackViolation(
                            ActivationReadbackViolation::UnchangedChainMismatch,
                        ),
                        DurableSelectionKnowledge::Unknown,
                        ticket.admission_posture,
                    );
                }
                if let Err(error) = self
                    .admission
                    .recover_proved_no_selection(
                        request.command.expected_head,
                        request.execution_fence,
                        active_recovery_fence,
                        &unchanged_chain,
                    )
                    .await
                {
                    return reconciliation(
                        &request,
                        ActivationTransactionStage::AdmissionReopen,
                        ActivationReconciliationReason::AdmissionFailure(error),
                        DurableSelectionKnowledge::NotAttempted,
                        ticket.admission_posture,
                    );
                }
                not_selected(
                    &request,
                    ActivationTransactionStage::DurableAppendReadback,
                    ActivationNotSelectedReason::OperationMarkerProvedNotSelected(evidence),
                )
            }
            ActivationOperationMarkerOutcome::Selected {
                event,
                table_versions,
                chain_after_readback,
                acknowledgement,
                evidence,
            } => {
                if let Err(violation) = validate_recovered_selection(
                    &request,
                    event,
                    &table_versions,
                    &chain_after_readback,
                ) {
                    return reconciliation(
                        &request,
                        ActivationTransactionStage::DurableAppendReadback,
                        ActivationReconciliationReason::ReadbackViolation(violation),
                        DurableSelectionKnowledge::Unknown,
                        ticket.admission_posture,
                    );
                }
                if let ActivationAcknowledgementMarker::Acknowledged(receipt) = acknowledgement
                    && !recovered_acknowledgement_matches(
                        &request,
                        event,
                        active_recovery_fence,
                        receipt,
                    )
                {
                    return reconciliation(
                        &request,
                        ActivationTransactionStage::Acknowledgement,
                        ActivationReconciliationReason::ReadbackViolation(
                            ActivationReadbackViolation::AcknowledgementMarkerMismatch,
                        ),
                        DurableSelectionKnowledge::ReadBack {
                            event_id: event.event_id(),
                        },
                        ticket.admission_posture,
                    );
                }

                let durable_selection = DurableSelectionKnowledge::ReadBack {
                    event_id: event.event_id(),
                };
                let candidate = match self
                    .epoch_rebuilder
                    .rebuild_selected_epoch(ActivationEpochRebuildRequest {
                        event,
                        table_versions: Arc::clone(&table_versions),
                    })
                    .await
                {
                    ActivationEpochRebuildOutcome::Rebuilt(candidate)
                        if *candidate.identity() == event.pins().epoch
                            && candidate
                                .observation_publication()
                                .table_version_set()
                                .as_ref()
                                == table_versions.as_ref() =>
                    {
                        candidate
                    }
                    ActivationEpochRebuildOutcome::Rebuilt(_) => {
                        return reconciliation(
                            &request,
                            ActivationTransactionStage::EpochRebuild,
                            ActivationReconciliationReason::ReadbackViolation(
                                ActivationReadbackViolation::RebuiltEpochMismatch,
                            ),
                            durable_selection,
                            ticket.admission_posture,
                        );
                    }
                    ActivationEpochRebuildOutcome::Unknown { diagnostic } => {
                        return reconciliation(
                            &request,
                            ActivationTransactionStage::EpochRebuild,
                            ActivationReconciliationReason::EpochRebuildUnknown(diagnostic),
                            durable_selection,
                            ticket.admission_posture,
                        );
                    }
                };
                let publication = match self
                    .admission
                    .recover_selected_epoch(
                        request.command.expected_head,
                        request.execution_fence,
                        active_recovery_fence,
                        event,
                        &chain_after_readback,
                        candidate,
                        ticket.admission_posture == ActivationAdmissionPosture::Reopened,
                    )
                    .await
                {
                    Ok(publication) => publication,
                    Err(error) => {
                        return reconciliation(
                            &request,
                            ActivationTransactionStage::EpochSwap,
                            ActivationReconciliationReason::AdmissionFailure(error),
                            durable_selection,
                            ticket.admission_posture,
                        );
                    }
                };
                let recovery_posture = match publication {
                    RecoverySelectionPublication::PublishedClosed => {
                        ActivationAdmissionPosture::Swapped
                    }
                    RecoverySelectionPublication::AlreadyReopened => {
                        ActivationAdmissionPosture::Reopened
                    }
                };

                let cache = match self
                    .cache
                    .reconcile_selected(event, &chain_after_readback, active_recovery_fence)
                    .await
                {
                    ActivationCacheOutcome::Reconciled(receipt)
                        if cache_receipt_matches(
                            &request,
                            event,
                            active_recovery_fence,
                            receipt,
                        ) =>
                    {
                        receipt
                    }
                    ActivationCacheOutcome::Reconciled(_) => {
                        return reconciliation(
                            &request,
                            ActivationTransactionStage::CacheReconciliation,
                            ActivationReconciliationReason::ReadbackViolation(
                                ActivationReadbackViolation::CacheReceiptMismatch,
                            ),
                            durable_selection,
                            recovery_posture,
                        );
                    }
                    ActivationCacheOutcome::Unknown { diagnostic } => {
                        return reconciliation(
                            &request,
                            ActivationTransactionStage::CacheReconciliation,
                            ActivationReconciliationReason::CacheUnknown(diagnostic),
                            durable_selection,
                            recovery_posture,
                        );
                    }
                    ActivationCacheOutcome::Cancelled { diagnostic } => {
                        return reconciliation(
                            &request,
                            ActivationTransactionStage::CacheReconciliation,
                            ActivationReconciliationReason::CacheCancelled(diagnostic),
                            durable_selection,
                            recovery_posture,
                        );
                    }
                };

                if publication == RecoverySelectionPublication::PublishedClosed
                    && let Err(error) = self
                        .admission
                        .reopen_recovered_selection(
                            event,
                            &chain_after_readback,
                            active_recovery_fence,
                        )
                        .await
                {
                    return reconciliation(
                        &request,
                        ActivationTransactionStage::AdmissionReopen,
                        ActivationReconciliationReason::AdmissionFailure(error),
                        durable_selection,
                        ActivationAdmissionPosture::Swapped,
                    );
                }

                let acknowledgement = match acknowledgement {
                    ActivationAcknowledgementMarker::Acknowledged(receipt) => receipt,
                    ActivationAcknowledgementMarker::Absent => {
                        match self
                            .acknowledgements
                            .acknowledge(event, active_recovery_fence)
                            .await
                        {
                            ActivationAcknowledgementOutcome::Acknowledged(receipt)
                                if acknowledgement_matches(
                                    &request,
                                    event,
                                    active_recovery_fence,
                                    receipt,
                                ) =>
                            {
                                receipt
                            }
                            ActivationAcknowledgementOutcome::Acknowledged(_) => {
                                return reconciliation(
                                    &request,
                                    ActivationTransactionStage::Acknowledgement,
                                    ActivationReconciliationReason::ReadbackViolation(
                                        ActivationReadbackViolation::AcknowledgementReceiptMismatch,
                                    ),
                                    durable_selection,
                                    ActivationAdmissionPosture::Reopened,
                                );
                            }
                            ActivationAcknowledgementOutcome::Unknown { diagnostic } => {
                                return reconciliation(
                                    &request,
                                    ActivationTransactionStage::Acknowledgement,
                                    ActivationReconciliationReason::AcknowledgementUnknown(
                                        diagnostic,
                                    ),
                                    durable_selection,
                                    ActivationAdmissionPosture::Reopened,
                                );
                            }
                            ActivationAcknowledgementOutcome::Cancelled { diagnostic } => {
                                return reconciliation(
                                    &request,
                                    ActivationTransactionStage::Acknowledgement,
                                    ActivationReconciliationReason::AcknowledgementCancelled(
                                        diagnostic,
                                    ),
                                    durable_selection,
                                    ActivationAdmissionPosture::Reopened,
                                );
                            }
                        }
                    }
                };

                ActivationTransactionOutcome::Activated(ActivatedEpochReceipt {
                    event,
                    cache,
                    acknowledgement,
                    reconciliation_evidence: Some(evidence),
                })
            }
        }
    }
}

fn proof_request(request: &ActivationTransactionRequest) -> CandidateProofRequest {
    CandidateProofRequest {
        workspace_id: request.command.ownership.workspace_id,
        operation_id: request.command.identity.operation_id,
        expected_head: request.command.expected_head,
        execution_fence: request.execution_fence,
        pins: request.pins,
    }
}

fn authority_request(request: &ActivationTransactionRequest) -> ActivationAuthorityRequest {
    ActivationAuthorityRequest {
        workspace_id: request.command.ownership.workspace_id,
        operation_id: request.command.identity.operation_id,
        expected_head: request.command.expected_head,
        execution_fence: request.execution_fence,
    }
}

fn operation_marker_request<R: ActivationRequestView>(
    request: &R,
    active_recovery_fence: WriterFence,
) -> ActivationOperationMarkerRequest {
    ActivationOperationMarkerRequest {
        workspace_id: request.command().ownership.workspace_id,
        operation_id: request.command().identity.operation_id,
        event_id: request.event_id(),
        expected_head: request.command().expected_head,
        execution_fence: request.execution_fence(),
        active_recovery_fence,
        transaction: request.transaction(),
        operation_selection: request.operation_selection(),
        control_relation: request.control_relation().clone(),
    }
}

fn epoch_rebuild_diagnostic(
    request: &ActivationEpochRebuildRequest,
    detail: &str,
) -> DiagnosticRef {
    fn frame(hasher: &mut blake3::Hasher, bytes: &[u8]) {
        hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
        hasher.update(bytes);
    }

    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.activation-epoch-rebuild-diagnostic.v1\0");
    frame(&mut hasher, request.event.workspace_id().as_bytes());
    frame(&mut hasher, request.event.operation_id().as_bytes());
    frame(&mut hasher, request.event.event_id().as_bytes());
    frame(&mut hasher, request.event.pins().epoch.as_bytes());
    frame(&mut hasher, request.table_versions.reference().as_bytes());
    frame(&mut hasher, detail.as_bytes());
    DiagnosticRef::from_bytes(*hasher.finalize().as_bytes())
}

fn recovery_ticket_matches(
    request: &ActivationRecoveryRequest,
    ticket: ActivationReconciliationTicket,
) -> bool {
    ticket.workspace_id == request.command.ownership.workspace_id
        && ticket.operation_id == request.command.identity.operation_id
        && ticket.candidate_epoch == request.pins.epoch
        && ticket.expected_head == request.command.expected_head
        && ticket.execution_fence == request.execution_fence
        && ticket.event_id == request.event_id
        && ticket.transaction == request.transaction
        && ticket.operation_selection == request.operation_selection
        && match ticket.durable_selection {
            DurableSelectionKnowledge::ReadBack { event_id } => event_id == request.event_id,
            DurableSelectionKnowledge::NotAttempted | DurableSelectionKnowledge::Unknown => true,
        }
        && recovery_stage_is_causal(ticket)
}

fn recovery_stage_is_causal(ticket: ActivationReconciliationTicket) -> bool {
    match ticket.stage {
        ActivationTransactionStage::CandidateProof
        | ActivationTransactionStage::AdmissionClosure => {
            ticket.admission_posture == ActivationAdmissionPosture::NeverClosed
                && ticket.durable_selection == DurableSelectionKnowledge::NotAttempted
        }
        ActivationTransactionStage::AuthorityRevalidation => {
            ticket.admission_posture == ActivationAdmissionPosture::Closed
                && ticket.durable_selection == DurableSelectionKnowledge::NotAttempted
        }
        ActivationTransactionStage::DurableAppendReadback => {
            ticket.admission_posture == ActivationAdmissionPosture::Closed
                && matches!(
                    ticket.durable_selection,
                    DurableSelectionKnowledge::NotAttempted | DurableSelectionKnowledge::Unknown
                )
        }
        ActivationTransactionStage::EpochRebuild => {
            matches!(
                ticket.admission_posture,
                ActivationAdmissionPosture::Closed
                    | ActivationAdmissionPosture::Swapped
                    | ActivationAdmissionPosture::Reopened
            ) && matches!(
                ticket.durable_selection,
                DurableSelectionKnowledge::ReadBack { .. }
            )
        }
        ActivationTransactionStage::EpochSwap => {
            ticket.admission_posture == ActivationAdmissionPosture::Closed
                && matches!(
                    ticket.durable_selection,
                    DurableSelectionKnowledge::ReadBack { .. }
                )
        }
        ActivationTransactionStage::CacheReconciliation => {
            ticket.admission_posture == ActivationAdmissionPosture::Swapped
                && matches!(
                    ticket.durable_selection,
                    DurableSelectionKnowledge::ReadBack { .. }
                )
        }
        ActivationTransactionStage::AdmissionReopen => {
            (ticket.admission_posture == ActivationAdmissionPosture::Closed
                && ticket.durable_selection == DurableSelectionKnowledge::NotAttempted)
                || (ticket.admission_posture == ActivationAdmissionPosture::Swapped
                    && matches!(
                        ticket.durable_selection,
                        DurableSelectionKnowledge::ReadBack { .. }
                    ))
        }
        ActivationTransactionStage::Acknowledgement => {
            ticket.admission_posture == ActivationAdmissionPosture::Reopened
                && matches!(
                    ticket.durable_selection,
                    DurableSelectionKnowledge::ReadBack { .. }
                )
        }
    }
}

fn validate_recovered_selection(
    request: &ActivationRecoveryRequest,
    event: ActivationEvent,
    table_versions: &TableVersionSet,
    chain_after_readback: &ActivationChain,
) -> Result<(), ActivationReadbackViolation> {
    let contract = ActivationAppendContract {
        attempt: request.attempt,
        event_id: request.event_id,
        command: request.command,
        execution_fence: request.execution_fence,
        predecessor_event_id: event.predecessor_event_id(),
        ordinal: event.ordinal(),
        pins: request.pins,
        table_versions: Arc::new(table_versions.clone()),
        compatibility: request.compatibility,
        retention: request.retention,
        operation_selection: request.operation_selection,
        transaction: request.transaction,
        control_relation: request.control_relation.clone(),
    };
    validate_committed_selection(&contract, event, table_versions, chain_after_readback).map_err(
        |violation| match violation {
            ActivationReadbackViolation::TableVersionSetMismatch => violation,
            _ => ActivationReadbackViolation::OperationMarkerMismatch,
        },
    )
}

fn recovered_unchanged_chain_matches(
    request: &ActivationRecoveryRequest,
    chain: &ActivationChain,
) -> bool {
    chain.workspace_id() == request.command.ownership.workspace_id
        && chain.current_head() == request.command.expected_head
        && chain.events().iter().all(|event| {
            event.operation_id() != request.command.identity.operation_id
                && event.event_id() != request.event_id
                && event.commit().transaction != request.transaction
                && event.commit().operation_selection != request.operation_selection
        })
}

fn authority_matches(
    request: &ActivationTransactionRequest,
    snapshot: &ActivationAuthoritySnapshot,
) -> bool {
    snapshot.chain.workspace_id() == request.command.ownership.workspace_id
        && snapshot.chain.current_head() == request.command.expected_head
        && snapshot.active_fence == request.execution_fence
}

fn append_contract(
    request: &ActivationTransactionRequest,
    authority: &ActivationAuthoritySnapshot,
) -> Option<ActivationAppendContract> {
    let (predecessor_event_id, ordinal) = match authority.chain.head_event() {
        Some(head) => (
            Some(head.event_id()),
            ActivationOrdinal::new(head.ordinal().get().checked_add(1)?)?,
        ),
        None => (None, ActivationOrdinal::new(1)?),
    };
    Some(ActivationAppendContract {
        attempt: request.attempt,
        event_id: request.event_id,
        command: request.command,
        execution_fence: request.execution_fence,
        predecessor_event_id,
        ordinal,
        pins: request.pins,
        table_versions: Arc::clone(
            request
                .candidate
                .observation_publication()
                .table_version_set(),
        ),
        compatibility: request.compatibility,
        retention: request.retention,
        operation_selection: request.operation_selection,
        transaction: request.transaction,
        control_relation: request.control_relation.clone(),
    })
}

fn validate_committed_selection(
    contract: &ActivationAppendContract,
    event: ActivationEvent,
    table_versions: &TableVersionSet,
    chain: &ActivationChain,
) -> Result<(), ActivationReadbackViolation> {
    let expected = ActivationEvent::try_from_attempt(
        contract.event_id,
        contract.attempt,
        contract.predecessor_event_id,
        contract.ordinal,
        contract.pins,
        contract.compatibility,
        contract.retention,
        ActivationCommit {
            operation_selection: contract.operation_selection,
            transaction: contract.transaction,
            ..event.commit()
        },
    )
    .map_err(ActivationReadbackViolation::EventContract)?;
    if expected != event {
        return Err(ActivationReadbackViolation::EventDiffersFromContract);
    }
    if table_versions.reference() != contract.pins.table_versions
        || table_versions != contract.table_versions.as_ref()
    {
        return Err(ActivationReadbackViolation::TableVersionSetMismatch);
    }
    if chain.workspace_id() != contract.command.ownership.workspace_id {
        return Err(ActivationReadbackViolation::ChainWorkspaceMismatch);
    }
    if chain.head_event().copied() != Some(event) {
        return Err(ActivationReadbackViolation::EventIsNotUniqueHead);
    }
    if chain.current_head() != ExpectedHead::Epoch(contract.pins.epoch) {
        return Err(ActivationReadbackViolation::ChainHeadMismatch);
    }
    Ok(())
}

fn unchanged_chain_matches(
    request: &ActivationTransactionRequest,
    authority: &ActivationAuthoritySnapshot,
    chain: &ActivationChain,
) -> bool {
    chain == &authority.chain
        && chain.workspace_id() == request.command.ownership.workspace_id
        && chain.current_head() == request.command.expected_head
}

fn cache_receipt_matches<R: ActivationRequestView>(
    request: &R,
    event: ActivationEvent,
    active_fence: WriterFence,
    receipt: ActivationCacheReceipt,
) -> bool {
    receipt.workspace_id == request.command().ownership.workspace_id
        && receipt.operation_id == request.command().identity.operation_id
        && receipt.event_id == event.event_id()
        && receipt.selected_epoch == request.pins().epoch
        && receipt.active_fence == active_fence
        && receipt.transaction == request.transaction()
}

fn acknowledgement_matches<R: ActivationRequestView>(
    request: &R,
    event: ActivationEvent,
    active_fence: WriterFence,
    receipt: ActivationAcknowledgementReceipt,
) -> bool {
    receipt.workspace_id == request.command().ownership.workspace_id
        && receipt.operation_id == request.command().identity.operation_id
        && receipt.event_id == event.event_id()
        && receipt.selected_epoch == request.pins().epoch
        && receipt.active_fence == active_fence
        && receipt.transaction == request.transaction()
        && receipt.operation_selection == request.operation_selection()
}

fn recovered_acknowledgement_matches(
    request: &ActivationRecoveryRequest,
    event: ActivationEvent,
    active_recovery_fence: WriterFence,
    receipt: ActivationAcknowledgementReceipt,
) -> bool {
    receipt.workspace_id == request.command.ownership.workspace_id
        && receipt.operation_id == request.command.identity.operation_id
        && receipt.event_id == event.event_id()
        && receipt.selected_epoch == request.pins.epoch
        && recovery_fence_authorizes(request.execution_fence, receipt.active_fence)
        && recovery_fence_authorizes(receipt.active_fence, active_recovery_fence)
        && receipt.transaction == request.transaction
        && receipt.operation_selection == request.operation_selection
}

fn recovery_fence_authorizes(execution: WriterFence, active: WriterFence) -> bool {
    active == execution || active.generation.get() > execution.generation.get()
}

fn not_selected<R: ActivationRequestView>(
    request: &R,
    stage: ActivationTransactionStage,
    reason: ActivationNotSelectedReason,
) -> ActivationTransactionOutcome {
    ActivationTransactionOutcome::NotSelected(ActivationNotSelected {
        stage,
        reason,
        operation_id: request.command().identity.operation_id,
        candidate_epoch: request.pins().epoch,
    })
}

fn reconciliation<R: ActivationRequestView>(
    request: &R,
    stage: ActivationTransactionStage,
    reason: ActivationReconciliationReason,
    durable_selection: DurableSelectionKnowledge,
    admission_posture: ActivationAdmissionPosture,
) -> ActivationTransactionOutcome {
    ActivationTransactionOutcome::ReconciliationNeeded(ActivationReconciliationTicket {
        stage,
        reason,
        workspace_id: request.command().ownership.workspace_id,
        operation_id: request.command().identity.operation_id,
        candidate_epoch: request.pins().epoch,
        expected_head: request.command().expected_head,
        execution_fence: request.execution_fence(),
        event_id: request.event_id(),
        transaction: request.transaction(),
        operation_selection: request.operation_selection(),
        durable_selection,
        admission_posture,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;
    use crate::fabric::activation::{
        ActivationReadbackRef, BackendCommitRef, OverlaySegmentSetRef, PolicySetRef,
        SealedActivationControlBinding, TableVersionSet, TableVersionSetRef,
    };
    use crate::fabric::command::{
        ActorId, AuthorizationRef, CommandIdentity, CommandOwnership, CommandPins,
        CompilerReleaseRef, ExecutionOwner, IdempotencyKey, LeaseId, ModelHeadRef, PrincipalId,
        ProviderSetRef, ResourceEnvelopeRef, SourceGeneration, WriterGeneration,
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
        let root = Url::parse("memory:///codefabric/activation-transaction-control").unwrap();
        ActivationControlRelationPin::new(
            ExactDeltaPin::new(&root, 4).unwrap(),
            SealedActivationControlBinding::for_test(
                "activation-transaction-test-session",
                "binding.system.activation-control.delta",
            ),
        )
    }

    fn command(workspace: WorkspaceId, candidate: EpochId) -> FabricCommand {
        FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes(id16(2)),
                idempotency_key: IdempotencyKey::from_bytes(id32(3)),
            },
            ownership: CommandOwnership {
                workspace_id: workspace,
                principal_id: PrincipalId::from_bytes(id16(4)),
                authorization: AuthorizationRef::from_bytes(id32(5)),
            },
            expected_head: ExpectedHead::Empty,
            writer_fence: WriterFence {
                lease_id: LeaseId::from_bytes(id16(6)),
                generation: WriterGeneration::new(7).unwrap(),
            },
            pins: CommandPins {
                compiler_release: CompilerReleaseRef::from_bytes(id32(8)),
                model_head: ModelHeadRef::from_bytes(id32(9)),
                source_generation: SourceGeneration::new(10),
                provider_set: ProviderSetRef::from_bytes(id32(11)),
            },
            resources: ResourceEnvelopeRef::from_bytes(id32(12)),
            payload: FabricCommandPayload::ActivateEpoch {
                candidate_epoch: candidate,
                proof_receipt: ProofReceiptRef::from_bytes(id32(13)),
            },
        }
    }

    async fn candidate(epoch_id: EpochId) -> Arc<ProgrammaticFabricEpoch> {
        let config = FabricEpochRuntimeConfig::default();
        Arc::new(
            ProgrammaticFabricEpochBuilder::try_new(epoch_id, config)
                .unwrap()
                .seal_for_test()
                .await
                .unwrap(),
        )
    }

    fn request(candidate: Arc<ProgrammaticFabricEpoch>) -> ActivationTransactionRequest {
        request_at_fence(candidate, None)
    }

    fn request_at_fence(
        candidate: Arc<ProgrammaticFabricEpoch>,
        execution_fence: Option<WriterFence>,
    ) -> ActivationTransactionRequest {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let command = command(workspace, *candidate.identity());
        let execution_fence = execution_fence.unwrap_or(command.writer_fence);
        let attempt = ActivationAttempt::for_test(
            command,
            1,
            ExecutionOwner {
                actor_id: ActorId::from_bytes(id16(40)),
                fence: execution_fence,
            },
        );
        let table_versions = candidate.observation_publication().table_version_set_ref();
        ActivationTransactionRequest::try_new(
            attempt,
            candidate,
            FabricEpochPins {
                epoch: *candidate_identity(&command),
                compiler_release: command.pins.compiler_release,
                model_head: command.pins.model_head,
                source_generation: command.pins.source_generation,
                provider_set: command.pins.provider_set,
                table_versions,
                overlay_segments: OverlaySegmentSetRef::from_bytes(id32(15)),
                policy_set: PolicySetRef::from_bytes(id32(16)),
                resource_envelope: command.resources,
                proof_receipt: ProofReceiptRef::from_bytes(id32(13)),
            },
            ActivationEventId::from_bytes(id32(17)),
            CompatibilityClassRef::from_bytes(id32(18)),
            RetentionPolicyRef::from_bytes(id32(19)),
            OperationSelectionRef::from_bytes(id32(20)),
            TransactionRef::from_bytes(id32(21)),
            control_relation(),
        )
        .unwrap()
    }

    fn candidate_identity(command: &FabricCommand) -> &EpochId {
        let FabricCommandPayload::ActivateEpoch {
            candidate_epoch, ..
        } = &command.payload
        else {
            unreachable!()
        };
        candidate_epoch
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Fault {
        None,
        Proof,
        Close,
        Authority,
        AppendUnknown,
        AppendNotCommitted,
        CorruptReadback,
        Publish,
        Cache,
        Reopen,
        Acknowledge,
    }

    type CallLog = Arc<Mutex<Vec<&'static str>>>;

    fn record(log: &CallLog, call: &'static str) {
        log.lock().unwrap().push(call);
    }

    #[tokio::test]
    async fn request_rejects_a_table_version_set_not_derived_from_the_candidate() {
        let candidate = candidate(EpochId::from_bytes(id16(0x31))).await;
        let valid = request(Arc::clone(&candidate));
        let mut mismatched = valid.pins();
        mismatched.table_versions = TableVersionSetRef::from_bytes(id32(0xee));
        assert_eq!(
            ActivationTransactionRequest::try_new(
                valid.attempt(),
                candidate,
                mismatched,
                valid.event_id,
                valid.compatibility,
                valid.retention,
                valid.operation_selection(),
                valid.transaction(),
                valid.control_relation().clone(),
            )
            .unwrap_err(),
            ActivationTransactionRequestError::CandidateTableVersionSetMismatch
        );
    }

    struct MockProof {
        fault: Fault,
        log: CallLog,
    }

    #[async_trait]
    impl ActivationCandidateProofPort for MockProof {
        async fn prove_candidate(&self, request: CandidateProofRequest) -> CandidateProofOutcome {
            record(&self.log, "proof");
            if self.fault == Fault::Proof {
                CandidateProofOutcome::Unknown {
                    diagnostic: DiagnosticRef::from_bytes(id32(31)),
                }
            } else {
                CandidateProofOutcome::Proved {
                    proof_receipt: request.pins.proof_receipt,
                }
            }
        }
    }

    struct MockAdmission {
        fault: Fault,
        log: CallLog,
        closed: AtomicBool,
        yield_after_close: bool,
    }

    #[async_trait]
    impl ActivationAdmissionPort for MockAdmission {
        type Barrier = u64;

        async fn close_admission(
            &self,
            _expected_head: ExpectedHead,
            _writer_fence: WriterFence,
        ) -> Result<Self::Barrier, AdmissionError> {
            record(&self.log, "close");
            if self.fault == Fault::Close || self.closed.swap(true, Ordering::SeqCst) {
                return Err(AdmissionError::AdmissionAlreadyClosed);
            }
            if self.yield_after_close {
                tokio::task::yield_now().await;
            }
            Ok(1)
        }

        async fn publish_selected_epoch(
            &self,
            _barrier: Self::Barrier,
            _chain_after_readback: &ActivationChain,
            _candidate: Arc<ProgrammaticFabricEpoch>,
        ) -> Result<(), AdmissionError> {
            record(&self.log, "publish");
            if self.fault == Fault::Publish {
                Err(AdmissionError::InternalInvariant("injected publish fault"))
            } else {
                Ok(())
            }
        }

        async fn reconcile_and_reopen(
            &self,
            _barrier: Self::Barrier,
            _reconciled_head: ExpectedHead,
        ) -> Result<(), AdmissionError> {
            record(&self.log, "reopen");
            if self.fault == Fault::Reopen {
                Err(AdmissionError::InternalInvariant("injected reopen fault"))
            } else {
                self.closed.store(false, Ordering::SeqCst);
                Ok(())
            }
        }

        async fn abort_proved_no_selection(
            &self,
            _barrier: Self::Barrier,
            _unchanged_chain: &ActivationChain,
        ) -> Result<(), AdmissionError> {
            record(&self.log, "abort");
            self.closed.store(false, Ordering::SeqCst);
            Ok(())
        }
    }

    struct MockAuthority {
        fault: Fault,
        log: CallLog,
        snapshot: ActivationAuthoritySnapshot,
    }

    #[async_trait]
    impl ActivationAuthorityPort for MockAuthority {
        async fn revalidate(
            &self,
            _request: ActivationAuthorityRequest,
        ) -> AuthorityRevalidationOutcome {
            record(&self.log, "authority");
            if self.fault == Fault::Authority {
                AuthorityRevalidationOutcome::Unknown {
                    diagnostic: DiagnosticRef::from_bytes(id32(32)),
                }
            } else {
                AuthorityRevalidationOutcome::Valid(self.snapshot.clone())
            }
        }
    }

    struct MockEvents {
        fault: Fault,
        log: CallLog,
        unchanged_chain: ActivationChain,
    }

    #[async_trait]
    impl ActivationEventPort for MockEvents {
        async fn append_and_readback(
            &self,
            contract: ActivationAppendContract,
        ) -> ActivationAppendOutcome {
            record(&self.log, "append");
            match self.fault {
                Fault::AppendUnknown => ActivationAppendOutcome::Unknown {
                    reason: ActivationAppendUnknownReason::CommitOutcomeUnknown,
                    diagnostic: DiagnosticRef::from_bytes(id32(33)),
                },
                Fault::AppendNotCommitted => ActivationAppendOutcome::NotCommitted {
                    unchanged_chain: self.unchanged_chain.clone(),
                    reason: ActivationNotCommittedReason::CancelledBeforeCommit,
                },
                _ => {
                    let event = ActivationEvent::try_from_attempt(
                        contract.event_id,
                        contract.attempt,
                        contract.predecessor_event_id,
                        contract.ordinal,
                        contract.pins,
                        contract.compatibility,
                        contract.retention,
                        ActivationCommit {
                            operation_selection: contract.operation_selection,
                            transaction: contract.transaction,
                            backend_commit: BackendCommitRef::from_bytes(id32(34)),
                            readback: ActivationReadbackRef::from_bytes(id32(35)),
                        },
                    )
                    .unwrap();
                    let chain_after_readback = if self.fault == Fault::CorruptReadback {
                        self.unchanged_chain.clone()
                    } else {
                        let mut events = self.unchanged_chain.events().to_vec();
                        events.push(event);
                        ActivationChain::derive(contract.command.ownership.workspace_id, events)
                            .unwrap()
                    };
                    ActivationAppendOutcome::Committed {
                        event,
                        table_versions: Arc::clone(&contract.table_versions),
                        chain_after_readback,
                    }
                }
            }
        }
    }

    struct MockCache {
        fault: Fault,
        log: CallLog,
    }

    #[async_trait]
    impl ActivationCachePort for MockCache {
        async fn reconcile_selected(
            &self,
            event: ActivationEvent,
            _chain_after_readback: &ActivationChain,
            active_fence: WriterFence,
        ) -> ActivationCacheOutcome {
            record(&self.log, "cache");
            if self.fault == Fault::Cache {
                ActivationCacheOutcome::Unknown {
                    diagnostic: DiagnosticRef::from_bytes(id32(36)),
                }
            } else {
                ActivationCacheOutcome::Reconciled(ActivationCacheReceipt {
                    workspace_id: event.workspace_id(),
                    operation_id: event.operation_id(),
                    event_id: event.event_id(),
                    selected_epoch: event.pins().epoch,
                    active_fence,
                    transaction: event.commit().transaction,
                })
            }
        }
    }

    struct MockAcknowledgements {
        fault: Fault,
        log: CallLog,
    }

    #[async_trait]
    impl ActivationAcknowledgementPort for MockAcknowledgements {
        async fn acknowledge(
            &self,
            event: ActivationEvent,
            active_fence: WriterFence,
        ) -> ActivationAcknowledgementOutcome {
            record(&self.log, "acknowledge");
            if self.fault == Fault::Acknowledge {
                ActivationAcknowledgementOutcome::Unknown {
                    diagnostic: DiagnosticRef::from_bytes(id32(37)),
                }
            } else {
                ActivationAcknowledgementOutcome::Acknowledged(ActivationAcknowledgementReceipt {
                    workspace_id: event.workspace_id(),
                    operation_id: event.operation_id(),
                    event_id: event.event_id(),
                    selected_epoch: event.pins().epoch,
                    active_fence,
                    transaction: event.commit().transaction,
                    operation_selection: event.commit().operation_selection,
                })
            }
        }
    }

    type TestCoordinator = ActivationTransactionCoordinator<
        MockAdmission,
        MockProof,
        MockAuthority,
        MockEvents,
        MockCache,
        MockAcknowledgements,
    >;

    fn coordinator(
        fault: Fault,
        request: &ActivationTransactionRequest,
        log: CallLog,
        yield_after_close: bool,
    ) -> TestCoordinator {
        let unchanged_chain = ActivationChain::derive(request.command.ownership.workspace_id, [])
            .expect("empty bootstrap chain is valid");
        ActivationTransactionCoordinator::new(
            Arc::new(MockAdmission {
                fault,
                log: Arc::clone(&log),
                closed: AtomicBool::new(false),
                yield_after_close,
            }),
            Arc::new(MockProof {
                fault,
                log: Arc::clone(&log),
            }),
            Arc::new(MockAuthority {
                fault,
                log: Arc::clone(&log),
                snapshot: ActivationAuthoritySnapshot {
                    chain: unchanged_chain.clone(),
                    active_fence: request.execution_fence,
                },
            }),
            Arc::new(MockEvents {
                fault,
                log: Arc::clone(&log),
                unchanged_chain,
            }),
            Arc::new(MockCache {
                fault,
                log: Arc::clone(&log),
            }),
            Arc::new(MockAcknowledgements { fault, log }),
        )
    }

    #[tokio::test]
    async fn exact_success_obeys_the_required_boundary_order() {
        let candidate = candidate(EpochId::from_bytes(id16(40))).await;
        let request = request(candidate);
        let log = Arc::new(Mutex::new(Vec::new()));
        let coordinator = coordinator(Fault::None, &request, Arc::clone(&log), false);

        let outcome = coordinator.activate(request).await;

        assert!(matches!(
            outcome,
            ActivationTransactionOutcome::Activated(_)
        ));
        assert_eq!(
            *log.lock().unwrap(),
            [
                "proof",
                "close",
                "authority",
                "append",
                "publish",
                "cache",
                "reopen",
                "acknowledge",
            ]
        );
    }

    #[tokio::test]
    async fn forward_retry_writes_the_active_execution_fence_not_the_admitted_fence() {
        let candidate = candidate(EpochId::from_bytes(id16(49))).await;
        let execution_fence = WriterFence {
            lease_id: LeaseId::from_bytes(id16(50)),
            generation: WriterGeneration::new(9).unwrap(),
        };
        let request = request_at_fence(candidate, Some(execution_fence));
        assert_ne!(request.command.writer_fence, execution_fence);
        let log = Arc::new(Mutex::new(Vec::new()));
        let coordinator = coordinator(Fault::None, &request, log, false);

        let ActivationTransactionOutcome::Activated(receipt) = coordinator.activate(request).await
        else {
            panic!("newer-fence forward retry did not activate")
        };

        assert_eq!(receipt.event.execution_fence(), execution_fence);
        assert_eq!(receipt.cache.active_fence, execution_fence);
        assert_eq!(receipt.acknowledgement.active_fence, execution_fence);
    }

    #[tokio::test]
    async fn fault_matrix_never_promotes_partial_progress_to_success() {
        let candidate = candidate(EpochId::from_bytes(id16(41))).await;
        let cases = [
            (Fault::Proof, ActivationTransactionStage::CandidateProof, 1),
            (
                Fault::Close,
                ActivationTransactionStage::AdmissionClosure,
                2,
            ),
            (
                Fault::Authority,
                ActivationTransactionStage::AuthorityRevalidation,
                3,
            ),
            (
                Fault::AppendUnknown,
                ActivationTransactionStage::DurableAppendReadback,
                4,
            ),
            (
                Fault::CorruptReadback,
                ActivationTransactionStage::DurableAppendReadback,
                4,
            ),
            (Fault::Publish, ActivationTransactionStage::EpochSwap, 5),
            (
                Fault::Cache,
                ActivationTransactionStage::CacheReconciliation,
                6,
            ),
            (
                Fault::Reopen,
                ActivationTransactionStage::AdmissionReopen,
                7,
            ),
            (
                Fault::Acknowledge,
                ActivationTransactionStage::Acknowledgement,
                8,
            ),
        ];

        for (fault, expected_stage, expected_calls) in cases {
            let request = request(Arc::clone(&candidate));
            let log = Arc::new(Mutex::new(Vec::new()));
            let coordinator = coordinator(fault, &request, Arc::clone(&log), false);
            let outcome = coordinator.activate(request).await;
            let actual_stage = match outcome {
                ActivationTransactionOutcome::NotSelected(stopped) => stopped.stage,
                ActivationTransactionOutcome::ReconciliationNeeded(ticket) => ticket.stage,
                ActivationTransactionOutcome::Activated(_) => {
                    panic!("fault {fault:?} fabricated activation success")
                }
            };
            assert_eq!(actual_stage, expected_stage, "fault {fault:?}");
            assert_eq!(log.lock().unwrap().len(), expected_calls, "fault {fault:?}");
        }
    }

    #[tokio::test]
    async fn proved_not_committed_is_the_only_post_close_abort_path() {
        let candidate = candidate(EpochId::from_bytes(id16(42))).await;
        let request = request(candidate);
        let log = Arc::new(Mutex::new(Vec::new()));
        let coordinator = coordinator(Fault::AppendNotCommitted, &request, Arc::clone(&log), false);

        assert!(matches!(
            coordinator.activate(request).await,
            ActivationTransactionOutcome::NotSelected(ActivationNotSelected {
                reason: ActivationNotSelectedReason::AppendProvedNotCommitted(
                    ActivationNotCommittedReason::CancelledBeforeCommit
                ),
                ..
            })
        ));
        assert_eq!(
            *log.lock().unwrap(),
            ["proof", "close", "authority", "append", "abort"]
        );
    }

    #[tokio::test]
    async fn competing_activator_cannot_cross_the_same_admission_barrier() {
        let candidate = candidate(EpochId::from_bytes(id16(43))).await;
        let first = request(Arc::clone(&candidate));
        let second = first.clone();
        let log = Arc::new(Mutex::new(Vec::new()));
        let coordinator = coordinator(Fault::None, &first, Arc::clone(&log), true);

        let (left, right) = tokio::join!(coordinator.activate(first), coordinator.activate(second));
        let activated = [left, right]
            .into_iter()
            .filter(|outcome| matches!(outcome, ActivationTransactionOutcome::Activated(_)))
            .count();

        assert_eq!(activated, 1);
        assert!(
            log.lock()
                .unwrap()
                .iter()
                .filter(|call| **call == "append")
                .count()
                == 1
        );
    }

    fn recovered_event_and_chain(
        request: &ActivationTransactionRequest,
    ) -> (ActivationEvent, ActivationChain) {
        let event = ActivationEvent::try_from_attempt(
            request.event_id,
            request.attempt,
            None,
            ActivationOrdinal::new(1).unwrap(),
            request.pins,
            request.compatibility,
            request.retention,
            ActivationCommit {
                operation_selection: request.operation_selection,
                transaction: request.transaction,
                backend_commit: BackendCommitRef::from_bytes(id32(34)),
                readback: ActivationReadbackRef::from_bytes(id32(35)),
            },
        )
        .unwrap();
        let chain = ActivationChain::derive(request.command.ownership.workspace_id, [event])
            .expect("one exact root event is a valid activation chain");
        (event, chain)
    }

    #[tokio::test]
    async fn recovery_rejects_a_reversible_version_vector_substitution() {
        let candidate = candidate(EpochId::from_bytes(id16(54))).await;
        let request = request(candidate);
        let (event, chain) = recovered_event_and_chain(&request);
        let substituted = TableVersionSet::try_new(
            request
                .candidate
                .observation_publication()
                .table_version_set()
                .components()
                .enumerate()
                .map(|(index, (relation_id, pin))| {
                    let version = if index == 0 {
                        pin.version().checked_add(1).unwrap()
                    } else {
                        pin.version()
                    };
                    (
                        Arc::<str>::from(relation_id),
                        ExactDeltaPin::new(pin.canonical_root(), version).unwrap(),
                    )
                }),
        )
        .unwrap();
        assert_eq!(
            validate_recovered_selection(&request.recovery_request(), event, &substituted, &chain,),
            Err(ActivationReadbackViolation::TableVersionSetMismatch)
        );
    }

    #[tokio::test]
    async fn concrete_cache_and_acknowledgement_are_idempotent_and_fenced() {
        let candidate = candidate(EpochId::from_bytes(id16(55))).await;
        let request = request(candidate);
        let (event, chain) = recovered_event_and_chain(&request);
        let active_fence = WriterFence {
            lease_id: LeaseId::from_bytes(id16(56)),
            generation: WriterGeneration::new(request.execution_fence.generation.get() + 1)
                .unwrap(),
        };
        let cache =
            ActivationReconciliationReceiptCache::new(request.command.ownership.workspace_id);
        let first_cache = cache.reconcile_selected(event, &chain, active_fence).await;
        let ActivationCacheOutcome::Reconciled(first_receipt) = first_cache else {
            panic!("valid durable chain did not reconcile the activation receipt")
        };
        assert_eq!(cache.current_receipt().unwrap(), Some(first_receipt));
        assert_eq!(
            cache.reconcile_selected(event, &chain, active_fence).await,
            ActivationCacheOutcome::Reconciled(first_receipt),
            "cache replay must be idempotent"
        );

        let acknowledgements =
            IdempotentActivationAcknowledgements::new(request.command.ownership.workspace_id);
        let first_acknowledgement = acknowledgements.acknowledge(event, active_fence).await;
        assert!(matches!(
            first_acknowledgement,
            ActivationAcknowledgementOutcome::Acknowledged(_)
        ));
        assert_eq!(
            acknowledgements.acknowledge(event, active_fence).await,
            first_acknowledgement,
            "acknowledgement replay must be a pure deterministic projection"
        );

        let regressed_fence = WriterFence {
            lease_id: LeaseId::from_bytes(id16(57)),
            generation: WriterGeneration::new(request.execution_fence.generation.get() - 1)
                .unwrap(),
        };
        assert!(matches!(
            cache
                .reconcile_selected(event, &chain, regressed_fence)
                .await,
            ActivationCacheOutcome::Unknown { .. }
        ));
        assert!(matches!(
            acknowledgements.acknowledge(event, regressed_fence).await,
            ActivationAcknowledgementOutcome::Unknown { .. }
        ));
        assert_eq!(
            cache.current_receipt().unwrap(),
            Some(first_receipt),
            "a rejected fence cannot mutate the last reconciled projection"
        );
    }

    fn recovery_ticket(
        request: &ActivationTransactionRequest,
        stage: ActivationTransactionStage,
        durable_selection: DurableSelectionKnowledge,
        admission_posture: ActivationAdmissionPosture,
    ) -> ActivationReconciliationTicket {
        ActivationReconciliationTicket {
            stage,
            reason: ActivationReconciliationReason::AppendUnknown {
                reason: ActivationAppendUnknownReason::CommitOutcomeUnknown,
                diagnostic: DiagnosticRef::from_bytes(id32(70)),
            },
            workspace_id: request.command.ownership.workspace_id,
            operation_id: request.command.identity.operation_id,
            candidate_epoch: request.pins.epoch,
            expected_head: request.command.expected_head,
            execution_fence: request.execution_fence,
            event_id: request.event_id,
            transaction: request.transaction,
            operation_selection: request.operation_selection,
            durable_selection,
            admission_posture,
        }
    }

    fn recovery_attempt(
        request: &ActivationTransactionRequest,
        active_recovery_fence: WriterFence,
    ) -> ActivationRecoveryAttempt {
        let execution_owner = request.attempt().execution_owner();
        let active_recovery_owner = if execution_owner.fence == active_recovery_fence {
            execution_owner
        } else {
            ExecutionOwner {
                actor_id: ActorId::from_bytes(id16(52)),
                fence: active_recovery_fence,
            }
        };
        ActivationRecoveryAttempt::for_test(request.attempt(), active_recovery_owner)
    }

    struct StaticOperationMarker {
        expected: ActivationOperationMarkerRequest,
        outcome: ActivationOperationMarkerOutcome,
        log: CallLog,
    }

    #[async_trait]
    impl ActivationOperationMarkerPort for StaticOperationMarker {
        async fn read_operation_marker(
            &self,
            request: ActivationOperationMarkerRequest,
        ) -> ActivationOperationMarkerOutcome {
            record(&self.log, "marker");
            assert_eq!(request, self.expected);
            self.outcome.clone()
        }
    }

    struct RecoveryEpochRebuilder {
        candidate: Arc<ProgrammaticFabricEpoch>,
    }

    #[async_trait]
    impl ActivationEpochRebuilderPort for RecoveryEpochRebuilder {
        async fn rebuild_selected_epoch(
            &self,
            request: ActivationEpochRebuildRequest,
        ) -> ActivationEpochRebuildOutcome {
            if request.event.pins().epoch == *self.candidate.identity()
                && request.table_versions.as_ref()
                    == self
                        .candidate
                        .observation_publication()
                        .table_version_set()
                        .as_ref()
            {
                ActivationEpochRebuildOutcome::Rebuilt(Arc::clone(&self.candidate))
            } else {
                ActivationEpochRebuildOutcome::Unknown {
                    diagnostic: DiagnosticRef::from_bytes(id32(79)),
                }
            }
        }
    }

    fn recovery_rebuilder(request: &ActivationTransactionRequest) -> Arc<RecoveryEpochRebuilder> {
        Arc::new(RecoveryEpochRebuilder {
            candidate: Arc::clone(request.candidate()),
        })
    }

    struct RecoveryCache {
        admission: Arc<FabricAdmissionRuntime>,
        log: CallLog,
    }

    #[async_trait]
    impl ActivationCachePort for RecoveryCache {
        async fn reconcile_selected(
            &self,
            event: ActivationEvent,
            _chain_after_readback: &ActivationChain,
            active_fence: WriterFence,
        ) -> ActivationCacheOutcome {
            record(&self.log, "cache");
            assert_eq!(
                self.admission.admit().unwrap_err(),
                AdmissionError::AdmissionClosed,
                "cache recovery must run before admission reopens"
            );
            ActivationCacheOutcome::Reconciled(ActivationCacheReceipt {
                workspace_id: event.workspace_id(),
                operation_id: event.operation_id(),
                event_id: event.event_id(),
                selected_epoch: event.pins().epoch,
                active_fence,
                transaction: event.commit().transaction,
            })
        }
    }

    struct RecoveryAcknowledgements {
        admission: Arc<FabricAdmissionRuntime>,
        log: CallLog,
        must_not_be_called: bool,
    }

    #[async_trait]
    impl ActivationAcknowledgementPort for RecoveryAcknowledgements {
        async fn acknowledge(
            &self,
            event: ActivationEvent,
            active_fence: WriterFence,
        ) -> ActivationAcknowledgementOutcome {
            assert!(
                !self.must_not_be_called,
                "durable acknowledgement was repeated"
            );
            record(&self.log, "acknowledge");
            assert_eq!(
                self.admission.admit().unwrap().epoch_id(),
                event.pins().epoch,
                "acknowledgement must follow recovery reopening"
            );
            ActivationAcknowledgementOutcome::Acknowledged(ActivationAcknowledgementReceipt {
                workspace_id: event.workspace_id(),
                operation_id: event.operation_id(),
                event_id: event.event_id(),
                selected_epoch: event.pins().epoch,
                active_fence,
                transaction: event.commit().transaction,
                operation_selection: event.commit().operation_selection,
            })
        }
    }

    fn acknowledgement_receipt(
        event: ActivationEvent,
        active_fence: WriterFence,
    ) -> ActivationAcknowledgementReceipt {
        ActivationAcknowledgementReceipt {
            workspace_id: event.workspace_id(),
            operation_id: event.operation_id(),
            event_id: event.event_id(),
            selected_epoch: event.pins().epoch,
            active_fence,
            transaction: event.commit().transaction,
            operation_selection: event.commit().operation_selection,
        }
    }

    #[tokio::test]
    async fn unknown_commit_recovers_only_from_exact_operation_marker_and_chain() {
        let candidate = candidate(EpochId::from_bytes(id16(44))).await;
        let original_session_id = candidate.context().state().session_id().to_owned();
        let request = request(Arc::clone(&candidate));
        let active_recovery_fence = WriterFence {
            lease_id: LeaseId::from_bytes(id16(51)),
            generation: WriterGeneration::new(8).unwrap(),
        };
        let (event, chain) = recovered_event_and_chain(&request);
        let admission = Arc::new(
            FabricAdmissionRuntime::recover_unmaterialized_for_reconciliation(&chain).unwrap(),
        );
        assert_eq!(
            admission.admit().unwrap_err(),
            AdmissionError::AdmissionClosed
        );
        let log = Arc::new(Mutex::new(Vec::new()));
        let recovery = ActivationRecoveryCoordinator::new(
            Arc::clone(&admission),
            Arc::new(StaticOperationMarker {
                expected: operation_marker_request(&request, active_recovery_fence),
                outcome: ActivationOperationMarkerOutcome::Selected {
                    event,
                    table_versions: Arc::clone(
                        request
                            .candidate
                            .observation_publication()
                            .table_version_set(),
                    ),
                    chain_after_readback: chain,
                    acknowledgement: ActivationAcknowledgementMarker::Absent,
                    evidence: ReconciliationEvidenceRef::from_bytes(id32(71)),
                },
                log: Arc::clone(&log),
            }),
            Arc::new(ExactDeltaProgrammaticEpochRebuilder::new(|epoch_id| {
                ProgrammaticFabricEpochBuilder::try_new(
                    epoch_id,
                    FabricEpochRuntimeConfig::default(),
                )
            })),
            Arc::new(RecoveryCache {
                admission: Arc::clone(&admission),
                log: Arc::clone(&log),
            }),
            Arc::new(RecoveryAcknowledgements {
                admission: Arc::clone(&admission),
                log: Arc::clone(&log),
                must_not_be_called: false,
            }),
        );
        let ticket = recovery_ticket(
            &request,
            ActivationTransactionStage::DurableAppendReadback,
            DurableSelectionKnowledge::Unknown,
            ActivationAdmissionPosture::Closed,
        );

        let ActivationTransactionOutcome::Activated(receipt) = recovery
            .recover(
                request.recovery_request(),
                ticket,
                recovery_attempt(&request, active_recovery_fence),
            )
            .await
        else {
            panic!("marker-selected activation did not recover")
        };
        assert_eq!(receipt.event.execution_fence(), request.execution_fence);
        assert_eq!(receipt.cache.active_fence, active_recovery_fence);
        assert_eq!(receipt.acknowledgement.active_fence, active_recovery_fence);
        let admitted = admission.admit().unwrap();
        assert_eq!(admitted.epoch_id(), *candidate.identity());
        assert_ne!(
            admitted.epoch().context().state().session_id(),
            original_session_id,
            "restart recovery must install a freshly reconstructed DataFusion session"
        );
        assert_eq!(*log.lock().unwrap(), ["marker", "cache", "acknowledge"]);
    }

    #[tokio::test]
    async fn durable_ack_marker_prevents_duplicate_acknowledgement_on_recovery() {
        let candidate = candidate(EpochId::from_bytes(id16(45))).await;
        let request = request(Arc::clone(&candidate));
        let (event, chain) = recovered_event_and_chain(&request);
        let admission = Arc::new(
            FabricAdmissionRuntime::recover_for_reconciliation(&chain, |_| {
                Some(Arc::clone(&candidate))
            })
            .unwrap(),
        );
        let log = Arc::new(Mutex::new(Vec::new()));
        let recovery = ActivationRecoveryCoordinator::new(
            Arc::clone(&admission),
            Arc::new(StaticOperationMarker {
                expected: operation_marker_request(&request, request.execution_fence),
                outcome: ActivationOperationMarkerOutcome::Selected {
                    event,
                    table_versions: Arc::clone(
                        request
                            .candidate
                            .observation_publication()
                            .table_version_set(),
                    ),
                    chain_after_readback: chain,
                    acknowledgement: ActivationAcknowledgementMarker::Acknowledged(
                        acknowledgement_receipt(event, request.execution_fence),
                    ),
                    evidence: ReconciliationEvidenceRef::from_bytes(id32(72)),
                },
                log: Arc::clone(&log),
            }),
            recovery_rebuilder(&request),
            Arc::new(RecoveryCache {
                admission: Arc::clone(&admission),
                log: Arc::clone(&log),
            }),
            Arc::new(RecoveryAcknowledgements {
                admission: Arc::clone(&admission),
                log: Arc::clone(&log),
                must_not_be_called: true,
            }),
        );
        let ticket = recovery_ticket(
            &request,
            ActivationTransactionStage::Acknowledgement,
            DurableSelectionKnowledge::ReadBack {
                event_id: event.event_id(),
            },
            ActivationAdmissionPosture::Reopened,
        );

        assert!(matches!(
            recovery
                .recover(
                    request.recovery_request(),
                    ticket,
                    recovery_attempt(&request, request.execution_fence),
                )
                .await,
            ActivationTransactionOutcome::Activated(_)
        ));
        assert_eq!(*log.lock().unwrap(), ["marker", "cache"]);
    }

    #[tokio::test]
    async fn recovery_rejects_a_token_for_another_reducer_attempt_before_marker_readback() {
        let candidate = candidate(EpochId::from_bytes(id16(53))).await;
        let request = request(Arc::clone(&candidate));
        let (event, chain) = recovered_event_and_chain(&request);
        let admission = Arc::new(
            FabricAdmissionRuntime::recover_for_reconciliation(&chain, |_| Some(candidate))
                .unwrap(),
        );
        let log = Arc::new(Mutex::new(Vec::new()));
        let recovery = ActivationRecoveryCoordinator::new(
            Arc::clone(&admission),
            Arc::new(StaticOperationMarker {
                expected: operation_marker_request(&request, request.execution_fence),
                outcome: ActivationOperationMarkerOutcome::Selected {
                    event,
                    table_versions: Arc::clone(
                        request
                            .candidate
                            .observation_publication()
                            .table_version_set(),
                    ),
                    chain_after_readback: chain,
                    acknowledgement: ActivationAcknowledgementMarker::Absent,
                    evidence: ReconciliationEvidenceRef::from_bytes(id32(76)),
                },
                log: Arc::clone(&log),
            }),
            recovery_rebuilder(&request),
            Arc::new(RecoveryCache {
                admission: Arc::clone(&admission),
                log: Arc::clone(&log),
            }),
            Arc::new(RecoveryAcknowledgements {
                admission,
                log: Arc::clone(&log),
                must_not_be_called: true,
            }),
        );
        let ticket = recovery_ticket(
            &request,
            ActivationTransactionStage::DurableAppendReadback,
            DurableSelectionKnowledge::Unknown,
            ActivationAdmissionPosture::Closed,
        );
        let mismatched_attempt = ActivationAttempt::for_test(
            *request.command(),
            request.attempt().attempt() + 1,
            request.attempt().execution_owner(),
        );
        let mismatched_recovery = ActivationRecoveryAttempt::for_test(
            mismatched_attempt,
            request.attempt().execution_owner(),
        );

        assert!(matches!(
            recovery
                .recover(request.recovery_request(), ticket, mismatched_recovery)
                .await,
            ActivationTransactionOutcome::ReconciliationNeeded(ActivationReconciliationTicket {
                reason: ActivationReconciliationReason::ReadbackViolation(
                    ActivationReadbackViolation::RecoveryAttemptMismatch
                ),
                ..
            })
        ));
        assert!(log.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn explicit_nonselection_marker_is_the_only_recovery_abort_path() {
        let candidate = candidate(EpochId::from_bytes(id16(46))).await;
        let request = request(candidate);
        let unchanged_chain =
            ActivationChain::derive(request.command.ownership.workspace_id, []).unwrap();
        let admission = Arc::new(
            FabricAdmissionRuntime::recover_for_reconciliation(&unchanged_chain, |_| None).unwrap(),
        );
        let log = Arc::new(Mutex::new(Vec::new()));
        let recovery = ActivationRecoveryCoordinator::new(
            Arc::clone(&admission),
            Arc::new(StaticOperationMarker {
                expected: operation_marker_request(&request, request.execution_fence),
                outcome: ActivationOperationMarkerOutcome::ProvedNotSelected {
                    unchanged_chain,
                    evidence: ReconciliationEvidenceRef::from_bytes(id32(73)),
                },
                log: Arc::clone(&log),
            }),
            recovery_rebuilder(&request),
            Arc::new(RecoveryCache {
                admission: Arc::clone(&admission),
                log: Arc::clone(&log),
            }),
            Arc::new(RecoveryAcknowledgements {
                admission: Arc::clone(&admission),
                log: Arc::clone(&log),
                must_not_be_called: true,
            }),
        );
        let ticket = recovery_ticket(
            &request,
            ActivationTransactionStage::DurableAppendReadback,
            DurableSelectionKnowledge::Unknown,
            ActivationAdmissionPosture::Closed,
        );

        assert!(matches!(
            recovery
                .recover(
                    request.recovery_request(),
                    ticket,
                    recovery_attempt(&request, request.execution_fence),
                )
                .await,
            ActivationTransactionOutcome::NotSelected(ActivationNotSelected {
                reason: ActivationNotSelectedReason::OperationMarkerProvedNotSelected(_),
                ..
            })
        ));
        assert_eq!(
            admission.admit().unwrap_err(),
            AdmissionError::NoActiveEpoch
        );
        assert_eq!(*log.lock().unwrap(), ["marker"]);
    }

    #[tokio::test]
    async fn exact_readback_knowledge_cannot_regress_to_marker_nonselection() {
        let candidate = candidate(EpochId::from_bytes(id16(48))).await;
        let request = request(candidate);
        let unchanged_chain =
            ActivationChain::derive(request.command.ownership.workspace_id, []).unwrap();
        let admission = Arc::new(
            FabricAdmissionRuntime::recover_for_reconciliation(&unchanged_chain, |_| None).unwrap(),
        );
        let log = Arc::new(Mutex::new(Vec::new()));
        let recovery = ActivationRecoveryCoordinator::new(
            Arc::clone(&admission),
            Arc::new(StaticOperationMarker {
                expected: operation_marker_request(&request, request.execution_fence),
                outcome: ActivationOperationMarkerOutcome::ProvedNotSelected {
                    unchanged_chain,
                    evidence: ReconciliationEvidenceRef::from_bytes(id32(75)),
                },
                log: Arc::clone(&log),
            }),
            recovery_rebuilder(&request),
            Arc::new(RecoveryCache {
                admission: Arc::clone(&admission),
                log: Arc::clone(&log),
            }),
            Arc::new(RecoveryAcknowledgements {
                admission: Arc::clone(&admission),
                log: Arc::clone(&log),
                must_not_be_called: true,
            }),
        );
        let ticket = recovery_ticket(
            &request,
            ActivationTransactionStage::EpochSwap,
            DurableSelectionKnowledge::ReadBack {
                event_id: request.event_id,
            },
            ActivationAdmissionPosture::Closed,
        );

        assert!(matches!(
            recovery
                .recover(
                    request.recovery_request(),
                    ticket,
                    recovery_attempt(&request, request.execution_fence),
                )
                .await,
            ActivationTransactionOutcome::ReconciliationNeeded(ActivationReconciliationTicket {
                reason: ActivationReconciliationReason::ReadbackViolation(
                    ActivationReadbackViolation::OperationMarkerMismatch
                ),
                ..
            })
        ));
        assert_eq!(
            admission.admit().unwrap_err(),
            AdmissionError::AdmissionClosed
        );
        assert_eq!(*log.lock().unwrap(), ["marker"]);
    }

    #[tokio::test]
    async fn unknown_marker_keeps_restart_admission_closed_and_requires_reconciliation() {
        let candidate = candidate(EpochId::from_bytes(id16(47))).await;
        let request = request(Arc::clone(&candidate));
        let (event, chain) = recovered_event_and_chain(&request);
        let admission = Arc::new(
            FabricAdmissionRuntime::recover_for_reconciliation(&chain, |_| Some(candidate))
                .unwrap(),
        );
        let log = Arc::new(Mutex::new(Vec::new()));
        let recovery = ActivationRecoveryCoordinator::new(
            Arc::clone(&admission),
            Arc::new(StaticOperationMarker {
                expected: operation_marker_request(&request, request.execution_fence),
                outcome: ActivationOperationMarkerOutcome::Unknown {
                    diagnostic: DiagnosticRef::from_bytes(id32(74)),
                },
                log: Arc::clone(&log),
            }),
            recovery_rebuilder(&request),
            Arc::new(RecoveryCache {
                admission: Arc::clone(&admission),
                log: Arc::clone(&log),
            }),
            Arc::new(RecoveryAcknowledgements {
                admission: Arc::clone(&admission),
                log: Arc::clone(&log),
                must_not_be_called: true,
            }),
        );
        let ticket = recovery_ticket(
            &request,
            ActivationTransactionStage::DurableAppendReadback,
            DurableSelectionKnowledge::Unknown,
            ActivationAdmissionPosture::Closed,
        );

        assert!(matches!(
            recovery
                .recover(
                    request.recovery_request(),
                    ticket,
                    recovery_attempt(&request, request.execution_fence),
                )
                .await,
            ActivationTransactionOutcome::ReconciliationNeeded(ActivationReconciliationTicket {
                reason: ActivationReconciliationReason::OperationMarkerUnknown(_),
                ..
            })
        ));
        assert_eq!(
            admission.admit().unwrap_err(),
            AdmissionError::AdmissionClosed
        );
        assert_eq!(*log.lock().unwrap(), ["marker"]);
        assert_eq!(
            admission.active_head(),
            ExpectedHead::Epoch(event.pins().epoch)
        );
    }
}
