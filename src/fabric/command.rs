//! Pure contract and deterministic reducer for the single durable fabric-command path.
//!
//! This module deliberately knows nothing about SQLite, Delta Lake, task scheduling, or
//! workspace leases. Adapters persist [`CommandRecord`] values, establish the authoritative
//! [`AdmissionContext`], execute effects, and feed durable observations back as
//! [`CommandEvent`] values. The reducer never treats a timeout or lost acknowledgement as proof
//! that a commit did or did not happen.

use std::fmt;
use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};

macro_rules! byte_identity {
    ($(#[$meta:meta])* $name:ident, $width:expr) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        pub struct $name([u8; $width]);

        impl $name {
            /// Construct the typed identity from its canonical bytes.
            #[must_use]
            pub const fn from_bytes(bytes: [u8; $width]) -> Self {
                Self(bytes)
            }

            /// Borrow the canonical identity bytes.
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; $width] {
                &self.0
            }
        }
    };
}

byte_identity!(
    /// Stable operation identity used for lookup and durable reconciliation.
    OperationId,
    16
);
byte_identity!(
    /// Caller-stable key that prevents one logical intent from creating two operations.
    IdempotencyKey,
    32
);
byte_identity!(
    /// Workspace identity owning the command and all of its effects.
    WorkspaceId,
    16
);
byte_identity!(
    /// Authorized principal that submitted the command.
    PrincipalId,
    16
);
byte_identity!(
    /// Single-actor identity owning one execution attempt.
    ActorId,
    16
);
byte_identity!(
    /// OS-backed lease identity paired with a writer generation.
    LeaseId,
    16
);
byte_identity!(
    /// Immutable fabric-epoch identity.
    EpochId,
    16
);
byte_identity!(
    /// Authorization decision consumed at command admission.
    AuthorizationRef,
    32
);
byte_identity!(
    /// Exact reviewed explicit-input release pin.
    InputReleaseRef,
    32
);
byte_identity!(
    /// Exact typed program/transformation release pin.
    ProgramReleaseRef,
    32
);
byte_identity!(
    /// Exact application and analysis implementation release pin.
    ApplicationReleaseRef,
    32
);
byte_identity!(
    /// Exact source-image/inventory authority pin.
    SourceAuthorityRef,
    32
);
byte_identity!(
    /// Exact provider adapter and toolchain release pin.
    ProviderReleaseRef,
    32
);
byte_identity!(
    /// Exact admitted provider-run/configuration set authority.
    ProviderSetRef,
    32
);
byte_identity!(
    /// Bounded resource policy consumed by this command.
    ResourceEnvelopeRef,
    32
);
byte_identity!(
    /// Immutable source-image set for one source wave.
    SourceImageSetRef,
    32
);
byte_identity!(
    /// Exact provider run and its requested coverage.
    ProviderRunRef,
    32
);
byte_identity!(
    /// Exact owner set replaced by a relation publication.
    OwnerSetRef,
    32
);
byte_identity!(
    /// Relation-scoped Arrow input set.
    RelationSetRef,
    32
);
byte_identity!(
    /// Application-owned derived-analysis run.
    AnalysisRunRef,
    32
);
byte_identity!(
    /// Proof receipt for one exact candidate or equivalence claim.
    ProofReceiptRef,
    32
);
byte_identity!(
    /// Authorization for a governed rollback target.
    RollbackAuthorizationRef,
    32
);
byte_identity!(
    /// Retention policy version.
    RetentionPolicyRef,
    32
);
byte_identity!(
    /// Exact set protected from retention or vacuum.
    ProtectedSetRef,
    32
);
byte_identity!(
    /// Typed administrative request record.
    AdministrationRequestRef,
    32
);
byte_identity!(
    /// Durable operation-selection row written by the control relation.
    OperationSelectionRef,
    32
);
byte_identity!(
    /// Backend transaction or operation marker used for readback.
    TransactionRef,
    32
);
byte_identity!(
    /// Durable evidence produced by a reconciliation query.
    ReconciliationEvidenceRef,
    32
);
byte_identity!(
    /// Stable diagnostic fact referenced by a failure without embedding free-form payloads.
    DiagnosticRef,
    32
);

/// Monotonic single-writer generation. Zero is never a valid generation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct WriterGeneration(NonZeroU64);

impl WriterGeneration {
    /// Construct a nonzero writer generation.
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Return the numeric generation.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Exact source generation consumed or produced by a command.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct SourceGeneration(u64);

impl SourceGeneration {
    /// Construct a source generation. Generation zero is the valid empty-workspace baseline.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Return the numeric source generation.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Exact local-writer authority required at every durable write boundary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WriterFence {
    pub lease_id: LeaseId,
    pub generation: WriterGeneration,
}

/// Predecessor selected when the command was authored.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ExpectedHead {
    /// Bootstrap is legal only while no fabric epoch exists.
    Empty,
    /// The exact immutable epoch that must still be current.
    Epoch(EpochId),
}

/// Stable command identity and idempotency lookup keys.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandIdentity {
    pub operation_id: OperationId,
    pub idempotency_key: IdempotencyKey,
}

/// Submission ownership and the exact authorization fact admitted with the command.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandOwnership {
    pub workspace_id: WorkspaceId,
    pub principal_id: PrincipalId,
    pub authorization: AuthorizationRef,
}

/// Complete semantic input pins shared by every durable command variant.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandPins {
    pub input_release: InputReleaseRef,
    pub program_release: ProgramReleaseRef,
    pub application_release: ApplicationReleaseRef,
    pub source_authority: SourceAuthorityRef,
    pub source_generation: SourceGeneration,
    pub provider_release: ProviderReleaseRef,
    pub provider_set: ProviderSetRef,
}

/// Relation family being replaced by a typed publication command.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RelationClass {
    ProviderNative,
    Canonical,
    Derived,
    OperationalFact,
}

/// Typed relation-publication input. Variant structure rules out ambiguous provider/analysis
/// reference combinations.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RelationPublication {
    ProviderNative {
        provider_run: ProviderRunRef,
        owners: OwnerSetRef,
        relations: RelationSetRef,
    },
    Canonical {
        normalization_run: AnalysisRunRef,
        owners: OwnerSetRef,
        relations: RelationSetRef,
    },
    Derived {
        analysis_run: AnalysisRunRef,
        owners: OwnerSetRef,
        relations: RelationSetRef,
    },
    OperationalFact {
        request: AdministrationRequestRef,
        relations: RelationSetRef,
    },
}

impl RelationPublication {
    /// Return the relation family fixed by this typed input variant.
    #[must_use]
    pub const fn class(self) -> RelationClass {
        match self {
            Self::ProviderNative { .. } => RelationClass::ProviderNative,
            Self::Canonical { .. } => RelationClass::Canonical,
            Self::Derived { .. } => RelationClass::Derived,
            Self::OperationalFact { .. } => RelationClass::OperationalFact,
        }
    }
}

/// Closed administrative intent set. Request details remain in a typed referenced record.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AdministrationAction {
    RebuildCandidate,
    RepairTemporalCache,
    ReconcileOperation,
    /// Read the complete guarded retention closure for one exact selected Delta pin.
    InspectDeltaRetention,
    /// Validate a proposed deletion set without deleting any resource.
    ValidateDeltaRetention,
    /// Produce a native Delta vacuum dry run without deleting any file.
    PlanDeltaVacuum,
    /// Create a physical checkpoint for an exact selected Delta pin.
    CreateDeltaCheckpoint,
    /// Compact one exact selected Delta pin.
    CompactDelta,
    /// Execute a previously reviewed Delta vacuum plan.
    ExecuteDeltaVacuum,
}

/// Exhaustive command kind used by dispatch and result compatibility checks.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CommandKind {
    PublishSourceWave,
    PublishRelations,
    ActivateEpoch,
    RollbackEpoch,
    CompactRelations,
    ApplyRetention,
    Administer,
}

/// Typed command-specific intent. No variant admits opaque JSON, SQL, or untyped table names.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FabricCommandPayload {
    PublishSourceWave {
        source_images: SourceImageSetRef,
        target_generation: SourceGeneration,
    },
    PublishRelations {
        publication: RelationPublication,
    },
    ActivateEpoch {
        candidate_epoch: EpochId,
        proof_receipt: ProofReceiptRef,
    },
    RollbackEpoch {
        target_epoch: EpochId,
        authorization: RollbackAuthorizationRef,
    },
    CompactRelations {
        relations: RelationSetRef,
        equivalence_proof: ProofReceiptRef,
    },
    ApplyRetention {
        policy: RetentionPolicyRef,
        protected: ProtectedSetRef,
    },
    Administer {
        action: AdministrationAction,
        request: AdministrationRequestRef,
    },
}

impl FabricCommandPayload {
    /// Return the statically matched dispatch kind.
    #[must_use]
    pub const fn kind(self) -> CommandKind {
        match self {
            Self::PublishSourceWave { .. } => CommandKind::PublishSourceWave,
            Self::PublishRelations { .. } => CommandKind::PublishRelations,
            Self::ActivateEpoch { .. } => CommandKind::ActivateEpoch,
            Self::RollbackEpoch { .. } => CommandKind::RollbackEpoch,
            Self::CompactRelations { .. } => CommandKind::CompactRelations,
            Self::ApplyRetention { .. } => CommandKind::ApplyRetention,
            Self::Administer { .. } => CommandKind::Administer,
        }
    }
}

/// Immutable command envelope admitted before any side effect.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FabricCommand {
    pub identity: CommandIdentity,
    pub ownership: CommandOwnership,
    pub expected_head: ExpectedHead,
    pub writer_fence: WriterFence,
    pub pins: CommandPins,
    pub resources: ResourceEnvelopeRef,
    pub payload: FabricCommandPayload,
}

impl FabricCommand {
    /// Return the command's variant without maintaining a duplicate discriminant.
    #[must_use]
    pub const fn kind(&self) -> CommandKind {
        self.payload.kind()
    }
}

/// Authorization result supplied by the authoritative policy evaluator.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AuthorizationDecision {
    Authorized(AuthorizationRef),
    Denied(DiagnosticRef),
    Unknown(DiagnosticRef),
}

/// Exact read-before-admission state. Adapters must construct this under their writer lease.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdmissionContext {
    pub workspace_id: WorkspaceId,
    pub current_head: ExpectedHead,
    pub active_fence: WriterFence,
    pub authorization: AuthorizationDecision,
}

/// Authoritative mutable facts re-read for each reducer event.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReductionContext {
    pub current_head: ExpectedHead,
    pub active_fence: WriterFence,
}

/// Actor and fence owning one execution attempt.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExecutionOwner {
    pub actor_id: ActorId,
    pub fence: WriterFence,
}

/// Result of one command, typed by the command kind that may produce it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CommandResult {
    SourceWavePublished {
        source_generation: SourceGeneration,
        resulting_head: ExpectedHead,
        selection: OperationSelectionRef,
    },
    RelationsPublished {
        class: RelationClass,
        relations: RelationSetRef,
        resulting_head: ExpectedHead,
        selection: OperationSelectionRef,
    },
    EpochActivated {
        epoch: EpochId,
        selection: OperationSelectionRef,
    },
    EpochRolledBack {
        epoch: EpochId,
        selection: OperationSelectionRef,
    },
    RelationsCompacted {
        relations: RelationSetRef,
        resulting_head: ExpectedHead,
        selection: OperationSelectionRef,
    },
    RetentionApplied {
        protected: ProtectedSetRef,
        resulting_head: ExpectedHead,
        selection: OperationSelectionRef,
    },
    AdministrationApplied {
        request: AdministrationRequestRef,
        resulting_head: ExpectedHead,
        selection: OperationSelectionRef,
    },
}

impl CommandResult {
    /// Command kind that is allowed to produce this result variant.
    #[must_use]
    pub const fn kind(self) -> CommandKind {
        match self {
            Self::SourceWavePublished { .. } => CommandKind::PublishSourceWave,
            Self::RelationsPublished { .. } => CommandKind::PublishRelations,
            Self::EpochActivated { .. } => CommandKind::ActivateEpoch,
            Self::EpochRolledBack { .. } => CommandKind::RollbackEpoch,
            Self::RelationsCompacted { .. } => CommandKind::CompactRelations,
            Self::RetentionApplied { .. } => CommandKind::ApplyRetention,
            Self::AdministrationApplied { .. } => CommandKind::Administer,
        }
    }
}

/// Stable error category. Human detail lives in the referenced diagnostic relation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FailureCode {
    AuthorizationRevoked,
    InvalidInput,
    ResourceExhausted,
    BackendUnavailable,
    Cancelled,
    InternalInvariant,
}

/// Whether a known failure proves that the command did not commit.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FailureClass {
    Permanent,
    RetryableBeforeCommit,
}

/// Retry decision made only from a known failure or an unknown commit outcome.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RetryClassification {
    Never,
    RetrySameCommandAfterKnownNoCommit,
    ReconcileBeforeDecision,
}

/// Required durable query family for an unknown commit outcome.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ReconciliationClassification {
    NotRequired,
    OperationMarkerAndControlHistory,
}

/// Typed known failure. It cannot represent an unknown commit outcome.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandFailure {
    pub code: FailureCode,
    pub class: FailureClass,
    pub diagnostic: DiagnosticRef,
}

impl CommandFailure {
    /// Derive retry behavior without backend-specific guessing.
    #[must_use]
    pub const fn retry_classification(self) -> RetryClassification {
        match self.class {
            FailureClass::Permanent => RetryClassification::Never,
            FailureClass::RetryableBeforeCommit => {
                RetryClassification::RetrySameCommandAfterKnownNoCommit
            }
        }
    }

    /// Known failures never need commit reconciliation.
    #[must_use]
    pub const fn reconciliation_classification(self) -> ReconciliationClassification {
        ReconciliationClassification::NotRequired
    }
}

/// Why an adapter lost authoritative knowledge of a prepared commit.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum UnknownCommitReason {
    Timeout,
    ConnectionLost,
    ProcessInterrupted,
    ReadbackUnavailable,
}

/// Unknown commit outcomes are not failures and can never authorize a blind retry.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UnknownCommit {
    pub reason: UnknownCommitReason,
    pub diagnostic: DiagnosticRef,
}

impl UnknownCommit {
    /// Unknown outcomes always block retries until reconciliation.
    #[must_use]
    pub const fn retry_classification(self) -> RetryClassification {
        RetryClassification::ReconcileBeforeDecision
    }

    /// Return the durable evidence family required before another transition.
    #[must_use]
    pub const fn reconciliation_classification(self) -> ReconciliationClassification {
        ReconciliationClassification::OperationMarkerAndControlHistory
    }
}

/// Why a command was cancelled while no commit was in flight.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandCancellation {
    pub diagnostic: DiagnosticRef,
}

/// Evidence path that first established a successful terminal result.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CommitConfirmation {
    Direct,
    Reconciled(ReconciliationEvidenceRef),
}

/// Durable basis that makes a subsequent retry legal.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RetryBasis {
    KnownFailure(CommandFailure),
    ReconciledNotCommitted(ReconciliationEvidenceRef),
}

/// Persisted command progress. Only this reducer may construct a successor state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DurableCommandState {
    Admitted {
        attempt: u32,
    },
    Executing {
        attempt: u32,
        owner: ExecutionOwner,
    },
    CommitPrepared {
        attempt: u32,
        owner: ExecutionOwner,
        transaction: TransactionRef,
    },
    AwaitingReconciliation {
        attempt: u32,
        /// Exact actor/fence that prepared and attempted this transaction. Recovery must never
        /// replace this authority because committed readback is valid only for this executor.
        execution_owner: ExecutionOwner,
        /// Newest actor/fence that performed an exact recovery read. This may advance across
        /// indeterminate probes without changing the transaction's execution authority.
        recovery_owner: ExecutionOwner,
        transaction: TransactionRef,
        unknown: UnknownCommit,
        probe_count: u32,
        last_evidence: Option<ReconciliationEvidenceRef>,
    },
    RetryReady {
        next_attempt: u32,
        required_fence: WriterFence,
        basis: RetryBasis,
    },
    Succeeded {
        transaction: TransactionRef,
        result: CommandResult,
        confirmation: CommitConfirmation,
    },
    Failed {
        failure: CommandFailure,
    },
    Cancelled {
        cancellation: CommandCancellation,
    },
}

/// The only legal restart action implied by one nonterminal durable state.
///
/// This is classification, not execution. In particular, neither prepared nor unknown commits
/// become retryable until their exact transaction marker and control history prove non-commit.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CommandRecoveryObligation {
    /// No durable target may have been written; transaction preparation may resume.
    ResumePrecommit,
    /// The prior writer stopped after persisting the transaction but before a known outcome.
    MarkInterruptedCommit { transaction: TransactionRef },
    /// Durable unknown state already exists and requires exact marker/control-history readback.
    ReconcileCommit { transaction: TransactionRef },
    /// Exact reconciliation or a known pre-commit failure already proved retry safety.
    RetryProvedNotCommitted,
}

/// Coarse state identity used in stable transition errors.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CommandStateKind {
    Admitted,
    Executing,
    CommitPrepared,
    AwaitingReconciliation,
    RetryReady,
    Succeeded,
    Failed,
    Cancelled,
}

impl DurableCommandState {
    /// Stable state discriminant.
    #[must_use]
    pub const fn kind(self) -> CommandStateKind {
        match self {
            Self::Admitted { .. } => CommandStateKind::Admitted,
            Self::Executing { .. } => CommandStateKind::Executing,
            Self::CommitPrepared { .. } => CommandStateKind::CommitPrepared,
            Self::AwaitingReconciliation { .. } => CommandStateKind::AwaitingReconciliation,
            Self::RetryReady { .. } => CommandStateKind::RetryReady,
            Self::Succeeded { .. } => CommandStateKind::Succeeded,
            Self::Failed { .. } => CommandStateKind::Failed,
            Self::Cancelled { .. } => CommandStateKind::Cancelled,
        }
    }

    /// Whether the command has one immutable terminal answer.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded { .. } | Self::Failed { .. } | Self::Cancelled { .. }
        )
    }
}

/// One durable command record. Revision advances only for a material state transition.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandRecord {
    command: FabricCommand,
    state: DurableCommandState,
    revision: u64,
}

/// Reducer invariants rechecked after deserializing one temporal journal record.
///
/// Serde can reconstruct private fields directly, so canonical JSON bytes alone do not prove
/// that a record could have been emitted by [`CommandReducer`]. The temporal adapter maps every
/// violation to corrupt state and never resumes effects from it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandRecordInvariantError {
    InvalidAttempt(CommandStateKind),
    InvalidRevision(CommandStateKind),
    InvalidExecutionFence(CommandStateKind),
    InvalidRecoveryFence,
    InvalidReconciliationEvidence,
    InvalidRetryBasis,
    InvalidTerminalFailure,
    InvalidResultKind,
}

impl CommandRecord {
    /// Immutable admitted command.
    #[must_use]
    pub const fn command(&self) -> &FabricCommand {
        &self.command
    }

    /// Current durable reducer state.
    #[must_use]
    pub const fn state(self) -> DurableCommandState {
        self.state
    }

    /// Monotonic state revision for adapter-side compare-and-swap.
    #[must_use]
    pub const fn revision(self) -> u64 {
        self.revision
    }

    /// Prove that a deserialized record is reachable under reducer-owned structural invariants.
    ///
    /// This deliberately validates facts retained in the record itself. It does not claim to
    /// replay external semantic history; authoritative head, policy, and target-commit evidence
    /// are reread through their dedicated ports before any effect.
    pub(crate) fn validate_persisted_invariants(self) -> Result<(), CommandRecordInvariantError> {
        let state_kind = self.state.kind();
        match self.state {
            DurableCommandState::Admitted { attempt } => {
                if attempt != 1 {
                    return Err(CommandRecordInvariantError::InvalidAttempt(state_kind));
                }
                if self.revision != 0 {
                    return Err(CommandRecordInvariantError::InvalidRevision(state_kind));
                }
            }
            DurableCommandState::Executing { attempt, owner } => {
                validate_persisted_attempt(attempt, state_kind)?;
                validate_minimum_revision(self.revision, u64::from(attempt) * 2 - 1, state_kind)?;
                validate_persisted_execution_fence(self.command.writer_fence, owner, state_kind)?;
            }
            DurableCommandState::CommitPrepared { attempt, owner, .. } => {
                validate_persisted_attempt(attempt, state_kind)?;
                validate_minimum_revision(self.revision, u64::from(attempt) * 2, state_kind)?;
                validate_persisted_execution_fence(self.command.writer_fence, owner, state_kind)?;
            }
            DurableCommandState::AwaitingReconciliation {
                attempt,
                execution_owner,
                recovery_owner,
                probe_count,
                last_evidence,
                ..
            } => {
                validate_persisted_attempt(attempt, state_kind)?;
                validate_minimum_revision(
                    self.revision,
                    u64::from(attempt) * 2 + 1 + u64::from(probe_count),
                    state_kind,
                )?;
                validate_persisted_execution_fence(
                    self.command.writer_fence,
                    execution_owner,
                    state_kind,
                )?;
                if recovery_owner != execution_owner
                    && recovery_owner.fence.generation.get()
                        <= execution_owner.fence.generation.get()
                {
                    return Err(CommandRecordInvariantError::InvalidRecoveryFence);
                }
                if (probe_count == 0) != last_evidence.is_none() {
                    return Err(CommandRecordInvariantError::InvalidReconciliationEvidence);
                }
            }
            DurableCommandState::RetryReady {
                next_attempt,
                required_fence,
                basis,
            } => {
                if next_attempt < 2 {
                    return Err(CommandRecordInvariantError::InvalidAttempt(state_kind));
                }
                let minimum_revision = match basis {
                    RetryBasis::KnownFailure(_) => u64::from(next_attempt - 1) * 2,
                    RetryBasis::ReconciledNotCommitted(_) => u64::from(next_attempt) * 2,
                };
                validate_minimum_revision(self.revision, minimum_revision, state_kind)?;
                if required_fence != self.command.writer_fence
                    && required_fence.generation.get() <= self.command.writer_fence.generation.get()
                {
                    return Err(CommandRecordInvariantError::InvalidExecutionFence(
                        state_kind,
                    ));
                }
                if matches!(
                    basis,
                    RetryBasis::KnownFailure(CommandFailure {
                        class: FailureClass::Permanent,
                        ..
                    })
                ) {
                    return Err(CommandRecordInvariantError::InvalidRetryBasis);
                }
            }
            DurableCommandState::Succeeded {
                result,
                confirmation,
                ..
            } => {
                validate_minimum_revision(
                    self.revision,
                    match confirmation {
                        CommitConfirmation::Direct => 3,
                        CommitConfirmation::Reconciled(_) => 4,
                    },
                    state_kind,
                )?;
                if result.kind() != self.command.kind() {
                    return Err(CommandRecordInvariantError::InvalidResultKind);
                }
            }
            DurableCommandState::Failed { failure } => {
                validate_minimum_revision(self.revision, 2, state_kind)?;
                if failure.class != FailureClass::Permanent {
                    return Err(CommandRecordInvariantError::InvalidTerminalFailure);
                }
            }
            DurableCommandState::Cancelled { .. } => {
                validate_minimum_revision(self.revision, 1, state_kind)?;
            }
        }
        Ok(())
    }

    /// Return the one restart obligation encoded by this record, or `None` when terminal.
    #[must_use]
    pub const fn recovery_obligation(self) -> Option<CommandRecoveryObligation> {
        match self.state {
            DurableCommandState::Admitted { .. } | DurableCommandState::Executing { .. } => {
                Some(CommandRecoveryObligation::ResumePrecommit)
            }
            DurableCommandState::CommitPrepared { transaction, .. } => {
                Some(CommandRecoveryObligation::MarkInterruptedCommit { transaction })
            }
            DurableCommandState::AwaitingReconciliation { transaction, .. } => {
                Some(CommandRecoveryObligation::ReconcileCommit { transaction })
            }
            DurableCommandState::RetryReady { .. } => {
                Some(CommandRecoveryObligation::RetryProvedNotCommitted)
            }
            DurableCommandState::Succeeded { .. }
            | DurableCommandState::Failed { .. }
            | DurableCommandState::Cancelled { .. } => None,
        }
    }
}

fn validate_persisted_attempt(
    attempt: u32,
    state: CommandStateKind,
) -> Result<(), CommandRecordInvariantError> {
    if attempt == 0 {
        Err(CommandRecordInvariantError::InvalidAttempt(state))
    } else {
        Ok(())
    }
}

fn validate_minimum_revision(
    actual: u64,
    minimum: u64,
    state: CommandStateKind,
) -> Result<(), CommandRecordInvariantError> {
    if actual < minimum {
        Err(CommandRecordInvariantError::InvalidRevision(state))
    } else {
        Ok(())
    }
}

fn validate_persisted_execution_fence(
    admitted: WriterFence,
    owner: ExecutionOwner,
    state: CommandStateKind,
) -> Result<(), CommandRecordInvariantError> {
    if owner.fence == admitted || owner.fence.generation.get() > admitted.generation.get() {
        Ok(())
    } else {
        Err(CommandRecordInvariantError::InvalidExecutionFence(state))
    }
}

/// Admission either creates one record or returns the already-authoritative exact record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionOutcome {
    New(CommandRecord),
    Existing(CommandRecord),
}

impl AdmissionOutcome {
    /// Return the authoritative record for either admission outcome.
    #[must_use]
    pub const fn record(self) -> CommandRecord {
        match self {
            Self::New(record) | Self::Existing(record) => record,
        }
    }
}

/// Durable observation returned by operation-marker/control-history reconciliation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconciliationObservation {
    Committed {
        evidence: ReconciliationEvidenceRef,
        result: CommandResult,
    },
    NotCommitted {
        evidence: ReconciliationEvidenceRef,
    },
    Indeterminate {
        evidence: ReconciliationEvidenceRef,
    },
}

/// Reducer input produced by the actor or a backend adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandEvent {
    Start {
        owner: ExecutionOwner,
    },
    PrepareCommit {
        owner: ExecutionOwner,
        transaction: TransactionRef,
    },
    ConfirmCommit {
        owner: ExecutionOwner,
        transaction: TransactionRef,
        result: CommandResult,
    },
    ReportKnownFailure {
        owner: ExecutionOwner,
        failure: CommandFailure,
    },
    ReportUnknownCommit {
        owner: ExecutionOwner,
        transaction: TransactionRef,
        unknown: UnknownCommit,
    },
    ObserveReconciliation {
        owner: ExecutionOwner,
        transaction: TransactionRef,
        observation: ReconciliationObservation,
    },
    CancelBeforeCommit {
        owner: ExecutionOwner,
        cancellation: CommandCancellation,
    },
}

/// Coarse event identity used in stable transition errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandEventKind {
    Start,
    PrepareCommit,
    ConfirmCommit,
    ReportKnownFailure,
    ReportUnknownCommit,
    ObserveReconciliation,
    CancelBeforeCommit,
}

impl CommandEvent {
    /// Stable event discriminant.
    #[must_use]
    pub const fn kind(self) -> CommandEventKind {
        match self {
            Self::Start { .. } => CommandEventKind::Start,
            Self::PrepareCommit { .. } => CommandEventKind::PrepareCommit,
            Self::ConfirmCommit { .. } => CommandEventKind::ConfirmCommit,
            Self::ReportKnownFailure { .. } => CommandEventKind::ReportKnownFailure,
            Self::ReportUnknownCommit { .. } => CommandEventKind::ReportUnknownCommit,
            Self::ObserveReconciliation { .. } => CommandEventKind::ObserveReconciliation,
            Self::CancelBeforeCommit { .. } => CommandEventKind::CancelBeforeCommit,
        }
    }

    /// Actor/fence presenting this event to the reducer.
    #[must_use]
    pub const fn owner(self) -> ExecutionOwner {
        match self {
            Self::Start { owner }
            | Self::PrepareCommit { owner, .. }
            | Self::ConfirmCommit { owner, .. }
            | Self::ReportKnownFailure { owner, .. }
            | Self::ReportUnknownCommit { owner, .. }
            | Self::ObserveReconciliation { owner, .. }
            | Self::CancelBeforeCommit { owner, .. } => owner,
        }
    }
}

/// Whether reduction changed durable state or recognized an exact replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReductionEffect {
    StateChanged,
    IdempotentReplay,
    ReconciliationStillRequired,
}

/// Pure reducer output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Reduction {
    pub record: CommandRecord,
    pub effect: ReductionEffect,
    transition: Option<ReducerTransition>,
}

impl Reduction {
    /// Reducer-issued durable transition, present only when state materially changed.
    ///
    /// The witness binds both the exact predecessor and successor. It has no public constructor
    /// and is not deserializable, so a persistence adapter can reject a structurally valid but
    /// reducer-impossible same-command state jump.
    #[must_use]
    pub const fn transition(self) -> Option<ReducerTransition> {
        self.transition
    }
}

/// Unforgeable predecessor/successor pair emitted only by [`CommandReducer`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReducerTransition {
    predecessor: CommandRecord,
    successor: CommandRecord,
    effect: ReductionEffect,
}

impl ReducerTransition {
    /// Exact record on which the reducer evaluated the event.
    #[must_use]
    pub const fn predecessor(self) -> CommandRecord {
        self.predecessor
    }

    /// Exact material successor produced by the reducer.
    #[must_use]
    pub const fn successor(self) -> CommandRecord {
        self.successor
    }

    /// Material effect associated with the successor.
    #[must_use]
    pub const fn effect(self) -> ReductionEffect {
        self.effect
    }
}

/// Closed command-contract violations. None can be converted into a guessed success or retry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandContractError {
    DuplicateOperationConflict(OperationId),
    PayloadConflict(OperationId),
    IdempotencyKeyConflict(IdempotencyKey),
    ExistingLookupMismatch,
    WorkspaceMismatch,
    AuthorizationDenied,
    AuthorizationUnknown,
    AuthorizationMismatch,
    StaleExpectedHead {
        expected: ExpectedHead,
        observed: ExpectedHead,
    },
    StaleWriterFence {
        expected: WriterFence,
        observed: WriterFence,
    },
    ExecutionOwnerMismatch,
    RecoveryFenceNotAdvanced,
    IllegalTransition {
        state: CommandStateKind,
        event: CommandEventKind,
    },
    TransactionMismatch,
    ResultKindMismatch {
        command: CommandKind,
        result: CommandKind,
    },
    TerminalConflict,
    AttemptOverflow,
    RevisionOverflow,
}

impl fmt::Display for CommandContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateOperationConflict(_) => {
                formatter.write_str("operation identity is already bound to different intent")
            }
            Self::PayloadConflict(_) => {
                formatter.write_str("operation identity has a conflicting typed payload")
            }
            Self::IdempotencyKeyConflict(_) => {
                formatter.write_str("idempotency key is already bound to another operation")
            }
            Self::ExistingLookupMismatch => {
                formatter.write_str("existing record matches neither admission key")
            }
            Self::WorkspaceMismatch => formatter.write_str("workspace admission mismatch"),
            Self::AuthorizationDenied => formatter.write_str("command authorization denied"),
            Self::AuthorizationUnknown => formatter.write_str("command authorization is unknown"),
            Self::AuthorizationMismatch => formatter.write_str("authorization reference mismatch"),
            Self::StaleExpectedHead { .. } => formatter.write_str("expected head is stale"),
            Self::StaleWriterFence { .. } => formatter.write_str("writer fence is stale"),
            Self::ExecutionOwnerMismatch => {
                formatter.write_str("event does not belong to the execution owner")
            }
            Self::RecoveryFenceNotAdvanced => {
                formatter.write_str("recovery ownership requires a newer writer generation")
            }
            Self::IllegalTransition { .. } => {
                formatter.write_str("command state transition is illegal")
            }
            Self::TransactionMismatch => {
                formatter.write_str("event transaction does not match prepared commit")
            }
            Self::ResultKindMismatch { .. } => {
                formatter.write_str("result kind does not match command kind")
            }
            Self::TerminalConflict => {
                formatter.write_str("terminal command result cannot be changed")
            }
            Self::AttemptOverflow => formatter.write_str("command attempt overflow"),
            Self::RevisionOverflow => formatter.write_str("command revision overflow"),
        }
    }
}

impl std::error::Error for CommandContractError {}

/// Pure command contract and state reducer.
#[derive(Clone, Copy, Debug, Default)]
pub struct CommandReducer;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReconciliationProgress {
    attempt: u32,
    execution_owner: ExecutionOwner,
    recovery_owner: ExecutionOwner,
    transaction: TransactionRef,
    unknown: UnknownCommit,
    probe_count: u32,
}

impl CommandReducer {
    /// Admit a new command or return the exact existing record for an idempotent retry.
    ///
    /// `existing` is the result of looking up both the operation ID and idempotency key. Passing
    /// an unrelated record is rejected so an adapter cannot accidentally suppress a command.
    ///
    /// # Errors
    ///
    /// Rejects conflicting identities/payloads, denied or unknown authorization, stale heads,
    /// stale fences, and an unrelated existing lookup result.
    pub fn admit(
        existing: Option<&CommandRecord>,
        command: &FabricCommand,
        context: AdmissionContext,
    ) -> Result<AdmissionOutcome, CommandContractError> {
        if let Some(existing) = existing {
            if existing.command == *command {
                return Ok(AdmissionOutcome::Existing(*existing));
            }
            if existing.command.identity.operation_id == command.identity.operation_id {
                if existing.command.identity.idempotency_key == command.identity.idempotency_key
                    && existing.command.payload != command.payload
                {
                    return Err(CommandContractError::PayloadConflict(
                        command.identity.operation_id,
                    ));
                }
                return Err(CommandContractError::DuplicateOperationConflict(
                    command.identity.operation_id,
                ));
            }
            if existing.command.identity.idempotency_key == command.identity.idempotency_key {
                return Err(CommandContractError::IdempotencyKeyConflict(
                    command.identity.idempotency_key,
                ));
            }
            return Err(CommandContractError::ExistingLookupMismatch);
        }

        if context.workspace_id != command.ownership.workspace_id {
            return Err(CommandContractError::WorkspaceMismatch);
        }
        match context.authorization {
            AuthorizationDecision::Authorized(reference)
                if reference == command.ownership.authorization => {}
            AuthorizationDecision::Authorized(_) => {
                return Err(CommandContractError::AuthorizationMismatch);
            }
            AuthorizationDecision::Denied(_) => {
                return Err(CommandContractError::AuthorizationDenied);
            }
            AuthorizationDecision::Unknown(_) => {
                return Err(CommandContractError::AuthorizationUnknown);
            }
        }
        if context.current_head != command.expected_head {
            return Err(CommandContractError::StaleExpectedHead {
                expected: command.expected_head,
                observed: context.current_head,
            });
        }
        if context.active_fence != command.writer_fence {
            return Err(CommandContractError::StaleWriterFence {
                expected: command.writer_fence,
                observed: context.active_fence,
            });
        }

        Ok(AdmissionOutcome::New(CommandRecord {
            command: *command,
            state: DurableCommandState::Admitted { attempt: 1 },
            revision: 0,
        }))
    }

    /// Apply one observed event to a durable record.
    ///
    /// # Errors
    ///
    /// Rejects illegal transitions, ownership/fence/transaction mismatches, incompatible result
    /// variants, conflicts with terminal state, and counter overflow.
    pub fn reduce(
        record: &CommandRecord,
        event: CommandEvent,
        context: ReductionContext,
    ) -> Result<Reduction, CommandContractError> {
        Self::validate_result_kind(&record.command, event)?;

        if let Some(reduction) = Self::terminal_replay(record, event)? {
            return Ok(reduction);
        }
        Self::validate_reduction_context(record, event, context)?;

        match record.state {
            DurableCommandState::Admitted { attempt } => {
                Self::reduce_admitted(record, attempt, event)
            }
            DurableCommandState::Executing { attempt, owner } => {
                Self::reduce_executing(record, attempt, owner, event)
            }
            DurableCommandState::CommitPrepared {
                attempt,
                owner,
                transaction,
            } => Self::reduce_prepared(record, attempt, owner, transaction, event),
            DurableCommandState::AwaitingReconciliation {
                attempt,
                execution_owner,
                recovery_owner,
                transaction,
                unknown,
                probe_count,
                ..
            } => Self::reduce_awaiting_reconciliation(
                record,
                ReconciliationProgress {
                    attempt,
                    execution_owner,
                    recovery_owner,
                    transaction,
                    unknown,
                    probe_count,
                },
                event,
            ),
            DurableCommandState::RetryReady {
                next_attempt,
                required_fence,
                basis,
            } => Self::reduce_retry_ready(record, next_attempt, required_fence, basis, event),
            state => Err(Self::illegal(state, event)),
        }
    }

    fn reduce_admitted(
        record: &CommandRecord,
        attempt: u32,
        event: CommandEvent,
    ) -> Result<Reduction, CommandContractError> {
        let next = match event {
            CommandEvent::Start { owner } => {
                Self::validate_new_or_recovery_owner(record.command.writer_fence, owner)?;
                DurableCommandState::Executing { attempt, owner }
            }
            CommandEvent::CancelBeforeCommit {
                owner,
                cancellation,
            } => {
                Self::validate_new_or_recovery_owner(record.command.writer_fence, owner)?;
                DurableCommandState::Cancelled { cancellation }
            }
            event => return Err(Self::illegal(record.state, event)),
        };
        Self::changed(record, next)
    }

    fn reduce_retry_ready(
        record: &CommandRecord,
        next_attempt: u32,
        required_fence: WriterFence,
        basis: RetryBasis,
        event: CommandEvent,
    ) -> Result<Reduction, CommandContractError> {
        match event {
            CommandEvent::Start { owner } => {
                Self::validate_new_or_recovery_owner(required_fence, owner)?;
                Self::changed(
                    record,
                    DurableCommandState::Executing {
                        attempt: next_attempt,
                        owner,
                    },
                )
            }
            CommandEvent::ReportKnownFailure { failure, .. }
                if basis == RetryBasis::KnownFailure(failure) =>
            {
                Ok(Self::idempotent(record))
            }
            event => Err(Self::illegal(record.state, event)),
        }
    }

    fn reduce_executing(
        record: &CommandRecord,
        attempt: u32,
        current: ExecutionOwner,
        event: CommandEvent,
    ) -> Result<Reduction, CommandContractError> {
        let next = match event {
            CommandEvent::Start { owner } => {
                if current == owner {
                    return Ok(Self::idempotent(record));
                }
                Self::validate_recovery_owner(current, owner)?;
                DurableCommandState::Executing { attempt, owner }
            }
            CommandEvent::PrepareCommit { owner, transaction } => {
                Self::validate_current_owner(current, owner)?;
                DurableCommandState::CommitPrepared {
                    attempt,
                    owner,
                    transaction,
                }
            }
            CommandEvent::ReportKnownFailure { owner, failure } => {
                Self::validate_current_owner(current, owner)?;
                Self::known_failure_state(attempt, current.fence, failure)?
            }
            CommandEvent::CancelBeforeCommit {
                owner,
                cancellation,
            } => {
                Self::validate_current_owner(current, owner)?;
                DurableCommandState::Cancelled { cancellation }
            }
            event => return Err(Self::illegal(record.state, event)),
        };
        Self::changed(record, next)
    }

    fn reduce_prepared(
        record: &CommandRecord,
        attempt: u32,
        current: ExecutionOwner,
        prepared: TransactionRef,
        event: CommandEvent,
    ) -> Result<Reduction, CommandContractError> {
        let next = match event {
            CommandEvent::PrepareCommit { owner, transaction } => {
                Self::validate_current_owner(current, owner)?;
                Self::validate_transaction(prepared, transaction)?;
                return Ok(Self::idempotent(record));
            }
            CommandEvent::ConfirmCommit {
                owner,
                transaction,
                result,
            } => {
                Self::validate_current_owner(current, owner)?;
                Self::validate_transaction(prepared, transaction)?;
                DurableCommandState::Succeeded {
                    transaction,
                    result,
                    confirmation: CommitConfirmation::Direct,
                }
            }
            CommandEvent::ReportKnownFailure { owner, failure } => {
                Self::validate_current_owner(current, owner)?;
                Self::known_failure_state(attempt, current.fence, failure)?
            }
            CommandEvent::ReportUnknownCommit {
                owner,
                transaction,
                unknown,
            } => {
                Self::validate_recovery_owner(current, owner)?;
                Self::validate_transaction(prepared, transaction)?;
                DurableCommandState::AwaitingReconciliation {
                    attempt,
                    execution_owner: current,
                    recovery_owner: owner,
                    transaction,
                    unknown,
                    probe_count: 0,
                    last_evidence: None,
                }
            }
            event => return Err(Self::illegal(record.state, event)),
        };
        Self::changed(record, next)
    }

    fn reduce_awaiting_reconciliation(
        record: &CommandRecord,
        progress: ReconciliationProgress,
        event: CommandEvent,
    ) -> Result<Reduction, CommandContractError> {
        match event {
            CommandEvent::ReportUnknownCommit {
                owner,
                transaction,
                unknown,
            } => {
                Self::validate_recovery_owner(progress.recovery_owner, owner)?;
                Self::validate_transaction(progress.transaction, transaction)?;
                if progress.unknown == unknown {
                    Ok(Self::idempotent(record))
                } else {
                    Err(Self::illegal(record.state, event))
                }
            }
            CommandEvent::ObserveReconciliation {
                owner,
                transaction,
                observation,
            } => {
                Self::validate_recovery_owner(progress.recovery_owner, owner)?;
                Self::validate_transaction(progress.transaction, transaction)?;
                Self::apply_reconciliation(
                    record,
                    ReconciliationProgress {
                        recovery_owner: owner,
                        ..progress
                    },
                    observation,
                )
            }
            event => Err(Self::illegal(record.state, event)),
        }
    }

    fn apply_reconciliation(
        record: &CommandRecord,
        progress: ReconciliationProgress,
        observation: ReconciliationObservation,
    ) -> Result<Reduction, CommandContractError> {
        let (state, effect) = match observation {
            ReconciliationObservation::Committed { evidence, result } => (
                DurableCommandState::Succeeded {
                    transaction: progress.transaction,
                    result,
                    confirmation: CommitConfirmation::Reconciled(evidence),
                },
                ReductionEffect::StateChanged,
            ),
            ReconciliationObservation::NotCommitted { evidence } => (
                DurableCommandState::RetryReady {
                    next_attempt: Self::next_attempt(progress.attempt)?,
                    required_fence: progress.recovery_owner.fence,
                    basis: RetryBasis::ReconciledNotCommitted(evidence),
                },
                ReductionEffect::StateChanged,
            ),
            ReconciliationObservation::Indeterminate { evidence } => (
                DurableCommandState::AwaitingReconciliation {
                    attempt: progress.attempt,
                    execution_owner: progress.execution_owner,
                    recovery_owner: progress.recovery_owner,
                    transaction: progress.transaction,
                    unknown: progress.unknown,
                    probe_count: progress
                        .probe_count
                        .checked_add(1)
                        .ok_or(CommandContractError::AttemptOverflow)?,
                    last_evidence: Some(evidence),
                },
                ReductionEffect::ReconciliationStillRequired,
            ),
        };
        let mut reduction = Self::changed(record, state)?;
        reduction.effect = effect;
        reduction
            .transition
            .as_mut()
            .expect("a changed reconciliation result carries a durable transition")
            .effect = effect;
        Ok(reduction)
    }

    fn known_failure_state(
        attempt: u32,
        required_fence: WriterFence,
        failure: CommandFailure,
    ) -> Result<DurableCommandState, CommandContractError> {
        match failure.retry_classification() {
            RetryClassification::Never => Ok(DurableCommandState::Failed { failure }),
            RetryClassification::RetrySameCommandAfterKnownNoCommit => {
                Ok(DurableCommandState::RetryReady {
                    next_attempt: Self::next_attempt(attempt)?,
                    required_fence,
                    basis: RetryBasis::KnownFailure(failure),
                })
            }
            RetryClassification::ReconcileBeforeDecision => {
                unreachable!("CommandFailure cannot represent an unknown commit outcome")
            }
        }
    }

    const fn illegal(state: DurableCommandState, event: CommandEvent) -> CommandContractError {
        CommandContractError::IllegalTransition {
            state: state.kind(),
            event: event.kind(),
        }
    }

    fn validate_result_kind(
        command: &FabricCommand,
        event: CommandEvent,
    ) -> Result<(), CommandContractError> {
        let result = match event {
            CommandEvent::ConfirmCommit { result, .. }
            | CommandEvent::ObserveReconciliation {
                observation: ReconciliationObservation::Committed { result, .. },
                ..
            } => Some(result),
            _ => None,
        };
        if let Some(result) = result
            && command.kind() != result.kind()
        {
            return Err(CommandContractError::ResultKindMismatch {
                command: command.kind(),
                result: result.kind(),
            });
        }
        Ok(())
    }

    fn validate_reduction_context(
        record: &CommandRecord,
        event: CommandEvent,
        context: ReductionContext,
    ) -> Result<(), CommandContractError> {
        let owner = event.owner();
        if owner.fence != context.active_fence {
            return Err(CommandContractError::StaleWriterFence {
                expected: owner.fence,
                observed: context.active_fence,
            });
        }
        if matches!(
            event,
            CommandEvent::Start { .. } | CommandEvent::PrepareCommit { .. }
        ) && context.current_head != record.command.expected_head
        {
            return Err(CommandContractError::StaleExpectedHead {
                expected: record.command.expected_head,
                observed: context.current_head,
            });
        }
        Ok(())
    }

    fn validate_new_or_recovery_owner(
        required: WriterFence,
        owner: ExecutionOwner,
    ) -> Result<(), CommandContractError> {
        if owner.fence == required || owner.fence.generation.get() > required.generation.get() {
            Ok(())
        } else {
            Err(CommandContractError::RecoveryFenceNotAdvanced)
        }
    }

    fn validate_current_owner(
        current: ExecutionOwner,
        observed: ExecutionOwner,
    ) -> Result<(), CommandContractError> {
        if current == observed {
            Ok(())
        } else {
            Err(CommandContractError::ExecutionOwnerMismatch)
        }
    }

    fn validate_recovery_owner(
        current: ExecutionOwner,
        observed: ExecutionOwner,
    ) -> Result<(), CommandContractError> {
        if current == observed || observed.fence.generation.get() > current.fence.generation.get() {
            Ok(())
        } else {
            Err(CommandContractError::RecoveryFenceNotAdvanced)
        }
    }

    fn validate_transaction(
        prepared: TransactionRef,
        observed: TransactionRef,
    ) -> Result<(), CommandContractError> {
        if prepared == observed {
            Ok(())
        } else {
            Err(CommandContractError::TransactionMismatch)
        }
    }

    fn terminal_replay(
        record: &CommandRecord,
        event: CommandEvent,
    ) -> Result<Option<Reduction>, CommandContractError> {
        let exact = match (record.state, event) {
            (
                DurableCommandState::Succeeded {
                    transaction: committed,
                    result: current,
                    ..
                },
                CommandEvent::ConfirmCommit {
                    transaction,
                    result,
                    ..
                }
                | CommandEvent::ObserveReconciliation {
                    transaction,
                    observation: ReconciliationObservation::Committed { result, .. },
                    ..
                },
            ) => committed == transaction && current == result,
            (
                DurableCommandState::Failed { failure: current },
                CommandEvent::ReportKnownFailure { failure, .. },
            ) => current == failure,
            (
                DurableCommandState::Cancelled {
                    cancellation: current,
                },
                CommandEvent::CancelBeforeCommit { cancellation, .. },
            ) => current == cancellation,
            (state, _) if state.is_terminal() => false,
            _ => return Ok(None),
        };
        if exact {
            Ok(Some(Self::idempotent(record)))
        } else {
            Err(CommandContractError::TerminalConflict)
        }
    }

    fn next_attempt(attempt: u32) -> Result<u32, CommandContractError> {
        attempt
            .checked_add(1)
            .ok_or(CommandContractError::AttemptOverflow)
    }

    fn changed(
        record: &CommandRecord,
        state: DurableCommandState,
    ) -> Result<Reduction, CommandContractError> {
        let predecessor = *record;
        let mut successor = predecessor;
        successor.revision = successor
            .revision
            .checked_add(1)
            .ok_or(CommandContractError::RevisionOverflow)?;
        successor.state = state;
        Ok(Reduction {
            record: successor,
            effect: ReductionEffect::StateChanged,
            transition: Some(ReducerTransition {
                predecessor,
                successor,
                effect: ReductionEffect::StateChanged,
            }),
        })
    }

    const fn idempotent(record: &CommandRecord) -> Reduction {
        Reduction {
            record: *record,
            effect: ReductionEffect::IdempotentReplay,
            transition: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes16(seed: u8) -> [u8; 16] {
        [seed; 16]
    }

    fn bytes32(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn fence(seed: u8, generation: u64) -> WriterFence {
        WriterFence {
            lease_id: LeaseId::from_bytes(bytes16(seed)),
            generation: WriterGeneration::new(generation).expect("nonzero test generation"),
        }
    }

    fn owner(seed: u8, writer_fence: WriterFence) -> ExecutionOwner {
        ExecutionOwner {
            actor_id: ActorId::from_bytes(bytes16(seed)),
            fence: writer_fence,
        }
    }

    fn activation_command() -> FabricCommand {
        FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes(bytes16(1)),
                idempotency_key: IdempotencyKey::from_bytes(bytes32(2)),
            },
            ownership: CommandOwnership {
                workspace_id: WorkspaceId::from_bytes(bytes16(3)),
                principal_id: PrincipalId::from_bytes(bytes16(4)),
                authorization: AuthorizationRef::from_bytes(bytes32(5)),
            },
            expected_head: ExpectedHead::Epoch(EpochId::from_bytes(bytes16(6))),
            writer_fence: fence(7, 11),
            pins: CommandPins {
                input_release: InputReleaseRef::from_bytes(bytes32(8)),
                program_release: ProgramReleaseRef::from_bytes(bytes32(9)),
                application_release: crate::fabric::command::ApplicationReleaseRef::from_bytes(
                    bytes32(9),
                ),
                source_authority: crate::fabric::command::SourceAuthorityRef::from_bytes(bytes32(
                    9,
                )),
                provider_release: crate::fabric::command::ProviderReleaseRef::from_bytes(bytes32(
                    9,
                )),
                source_generation: SourceGeneration::new(12),
                provider_set: ProviderSetRef::from_bytes(bytes32(10)),
            },
            resources: ResourceEnvelopeRef::from_bytes(bytes32(11)),
            payload: FabricCommandPayload::ActivateEpoch {
                candidate_epoch: EpochId::from_bytes(bytes16(12)),
                proof_receipt: ProofReceiptRef::from_bytes(bytes32(13)),
            },
        }
    }

    fn admission_context(command: &FabricCommand) -> AdmissionContext {
        AdmissionContext {
            workspace_id: command.ownership.workspace_id,
            current_head: command.expected_head,
            active_fence: command.writer_fence,
            authorization: AuthorizationDecision::Authorized(command.ownership.authorization),
        }
    }

    fn admitted(command: &FabricCommand) -> CommandRecord {
        CommandReducer::admit(None, command, admission_context(command))
            .expect("valid command")
            .record()
    }

    fn reduction_context(record: &CommandRecord) -> ReductionContext {
        ReductionContext {
            current_head: record.command().expected_head,
            active_fence: record.command().writer_fence,
        }
    }

    fn context_with_fence(record: &CommandRecord, active_fence: WriterFence) -> ReductionContext {
        ReductionContext {
            current_head: record.command().expected_head,
            active_fence,
        }
    }

    fn reduce(
        record: &CommandRecord,
        event: CommandEvent,
    ) -> Result<Reduction, CommandContractError> {
        CommandReducer::reduce(record, event, reduction_context(record))
    }

    fn transaction(seed: u8) -> TransactionRef {
        TransactionRef::from_bytes(bytes32(seed))
    }

    fn evidence(seed: u8) -> ReconciliationEvidenceRef {
        ReconciliationEvidenceRef::from_bytes(bytes32(seed))
    }

    fn activation_result(command: &FabricCommand) -> CommandResult {
        let FabricCommandPayload::ActivateEpoch {
            candidate_epoch, ..
        } = command.payload
        else {
            panic!("activation fixture must carry activation payload");
        };
        CommandResult::EpochActivated {
            epoch: candidate_epoch,
            selection: OperationSelectionRef::from_bytes(bytes32(30)),
        }
    }

    fn start_and_prepare(
        record: &CommandRecord,
        execution_owner: ExecutionOwner,
        transaction: TransactionRef,
    ) -> CommandRecord {
        let started = reduce(
            record,
            CommandEvent::Start {
                owner: execution_owner,
            },
        )
        .expect("start");
        reduce(
            &started.record,
            CommandEvent::PrepareCommit {
                owner: execution_owner,
                transaction,
            },
        )
        .expect("prepare")
        .record
    }

    #[test]
    fn material_reductions_issue_exact_transition_witnesses_but_replays_do_not() {
        let command = activation_command();
        let admitted = admitted(&command);
        let execution_owner = owner(40, command.writer_fence);
        let started = reduce(
            &admitted,
            CommandEvent::Start {
                owner: execution_owner,
            },
        )
        .expect("start changes durable state");
        let transition = started
            .transition()
            .expect("material reduction carries a transition witness");
        assert_eq!(transition.predecessor(), admitted);
        assert_eq!(transition.successor(), started.record);
        assert_eq!(transition.effect(), started.effect);

        let replay = reduce(
            &started.record,
            CommandEvent::Start {
                owner: execution_owner,
            },
        )
        .expect("exact start replay");
        assert_eq!(replay.effect, ReductionEffect::IdempotentReplay);
        assert_eq!(replay.transition(), None);
    }

    #[test]
    fn persisted_invariants_charge_every_reconciliation_transition_to_revision() {
        let command = activation_command();
        let execution_owner = owner(41, command.writer_fence);
        let transaction = transaction(42);
        let unknown = UnknownCommit {
            reason: UnknownCommitReason::ReadbackUnavailable,
            diagnostic: DiagnosticRef::from_bytes(bytes32(43)),
        };
        let awaiting_after_one_probe = CommandRecord {
            command,
            state: DurableCommandState::AwaitingReconciliation {
                attempt: 1,
                execution_owner,
                recovery_owner: execution_owner,
                transaction,
                unknown,
                probe_count: 1,
                last_evidence: Some(evidence(44)),
            },
            revision: 3,
        };
        assert_eq!(
            awaiting_after_one_probe.validate_persisted_invariants(),
            Err(CommandRecordInvariantError::InvalidRevision(
                CommandStateKind::AwaitingReconciliation
            ))
        );

        let retry_after_reconciliation = CommandRecord {
            command,
            state: DurableCommandState::RetryReady {
                next_attempt: 2,
                required_fence: command.writer_fence,
                basis: RetryBasis::ReconciledNotCommitted(evidence(45)),
            },
            revision: 2,
        };
        assert_eq!(
            retry_after_reconciliation.validate_persisted_invariants(),
            Err(CommandRecordInvariantError::InvalidRevision(
                CommandStateKind::RetryReady
            ))
        );

        let reconciled_success = CommandRecord {
            command,
            state: DurableCommandState::Succeeded {
                transaction,
                result: activation_result(&command),
                confirmation: CommitConfirmation::Reconciled(evidence(46)),
            },
            revision: 3,
        };
        assert_eq!(
            reconciled_success.validate_persisted_invariants(),
            Err(CommandRecordInvariantError::InvalidRevision(
                CommandStateKind::Succeeded
            ))
        );
    }

    #[test]
    fn duplicate_identity_is_idempotent_but_conflicting_intent_fails() {
        let command = activation_command();
        let record = admitted(&command);
        let replay = CommandReducer::admit(Some(&record), &command, admission_context(&command))
            .expect("exact retry returns the durable record");
        assert_eq!(replay, AdmissionOutcome::Existing(record));

        let mut changed_payload = command;
        changed_payload.payload = FabricCommandPayload::ActivateEpoch {
            candidate_epoch: EpochId::from_bytes(bytes16(99)),
            proof_receipt: ProofReceiptRef::from_bytes(bytes32(13)),
        };
        assert_eq!(
            CommandReducer::admit(
                Some(&record),
                &changed_payload,
                admission_context(&changed_payload),
            ),
            Err(CommandContractError::PayloadConflict(
                command.identity.operation_id
            ))
        );

        let mut reused_key = command;
        reused_key.identity.operation_id = OperationId::from_bytes(bytes16(88));
        assert_eq!(
            CommandReducer::admit(Some(&record), &reused_key, admission_context(&reused_key),),
            Err(CommandContractError::IdempotencyKeyConflict(
                command.identity.idempotency_key
            ))
        );
    }

    #[test]
    fn admission_rejects_stale_head_fence_and_non_authority() {
        let command = activation_command();

        let mut context = admission_context(&command);
        context.current_head = ExpectedHead::Epoch(EpochId::from_bytes(bytes16(90)));
        assert!(matches!(
            CommandReducer::admit(None, &command, context),
            Err(CommandContractError::StaleExpectedHead { .. })
        ));

        let mut context = admission_context(&command);
        context.active_fence = fence(91, 12);
        assert!(matches!(
            CommandReducer::admit(None, &command, context),
            Err(CommandContractError::StaleWriterFence { .. })
        ));

        let mut context = admission_context(&command);
        context.authorization =
            AuthorizationDecision::Unknown(DiagnosticRef::from_bytes(bytes32(92)));
        assert_eq!(
            CommandReducer::admit(None, &command, context),
            Err(CommandContractError::AuthorizationUnknown)
        );
    }

    #[test]
    fn terminal_success_is_idempotent_and_cannot_be_rewritten() {
        let command = activation_command();
        let execution_owner = owner(20, command.writer_fence);
        let transaction = transaction(21);
        let prepared = start_and_prepare(&admitted(&command), execution_owner, transaction);
        assert_eq!(
            prepared.recovery_obligation(),
            Some(CommandRecoveryObligation::MarkInterruptedCommit { transaction })
        );
        let result = activation_result(&command);
        let event = CommandEvent::ConfirmCommit {
            owner: execution_owner,
            transaction,
            result,
        };
        let succeeded = reduce(&prepared, event).expect("commit");
        assert!(matches!(
            succeeded.record.state(),
            DurableCommandState::Succeeded { .. }
        ));

        let replay = reduce(&succeeded.record, event).expect("terminal replay");
        assert_eq!(replay.effect, ReductionEffect::IdempotentReplay);
        assert_eq!(replay.record.revision(), succeeded.record.revision());

        let conflict = CommandEvent::ConfirmCommit {
            owner: execution_owner,
            transaction,
            result: CommandResult::EpochActivated {
                epoch: EpochId::from_bytes(bytes16(77)),
                selection: OperationSelectionRef::from_bytes(bytes32(30)),
            },
        };
        assert_eq!(
            reduce(&succeeded.record, conflict),
            Err(CommandContractError::TerminalConflict)
        );
    }

    #[test]
    fn illegal_transitions_owner_changes_and_result_kind_mismatches_fail() {
        let command = activation_command();
        let record = admitted(&command);
        let execution_owner = owner(20, command.writer_fence);
        let transaction = transaction(21);
        assert!(matches!(
            reduce(
                &record,
                CommandEvent::ConfirmCommit {
                    owner: execution_owner,
                    transaction,
                    result: activation_result(&command),
                }
            ),
            Err(CommandContractError::IllegalTransition { .. })
        ));

        let started = reduce(
            &record,
            CommandEvent::Start {
                owner: execution_owner,
            },
        )
        .expect("start")
        .record;
        assert_eq!(
            reduce(
                &started,
                CommandEvent::PrepareCommit {
                    owner: owner(99, command.writer_fence),
                    transaction,
                }
            ),
            Err(CommandContractError::ExecutionOwnerMismatch)
        );

        let prepared = reduce(
            &started,
            CommandEvent::PrepareCommit {
                owner: execution_owner,
                transaction,
            },
        )
        .expect("prepare")
        .record;
        let wrong_result = CommandResult::RetentionApplied {
            protected: ProtectedSetRef::from_bytes(bytes32(31)),
            resulting_head: command.expected_head,
            selection: OperationSelectionRef::from_bytes(bytes32(32)),
        };
        assert!(matches!(
            reduce(
                &prepared,
                CommandEvent::ConfirmCommit {
                    owner: execution_owner,
                    transaction,
                    result: wrong_result,
                }
            ),
            Err(CommandContractError::ResultKindMismatch { .. })
        ));
    }

    #[test]
    fn known_failure_class_controls_retry_without_reconciliation() {
        let command = activation_command();
        let execution_owner = owner(20, command.writer_fence);
        let started = reduce(
            &admitted(&command),
            CommandEvent::Start {
                owner: execution_owner,
            },
        )
        .expect("start")
        .record;
        let retryable = CommandFailure {
            code: FailureCode::BackendUnavailable,
            class: FailureClass::RetryableBeforeCommit,
            diagnostic: DiagnosticRef::from_bytes(bytes32(40)),
        };
        let retry = reduce(
            &started,
            CommandEvent::ReportKnownFailure {
                owner: execution_owner,
                failure: retryable,
            },
        )
        .expect("known non-commit may retry")
        .record;
        assert!(matches!(
            retry.state(),
            DurableCommandState::RetryReady {
                next_attempt: 2,
                required_fence,
                basis: RetryBasis::KnownFailure(failure),
            } if failure == retryable && required_fence == command.writer_fence
        ));

        let restarted = reduce(
            &retry,
            CommandEvent::Start {
                owner: execution_owner,
            },
        )
        .expect("retry")
        .record;
        let permanent = CommandFailure {
            code: FailureCode::InvalidInput,
            class: FailureClass::Permanent,
            diagnostic: DiagnosticRef::from_bytes(bytes32(41)),
        };
        let failed = reduce(
            &restarted,
            CommandEvent::ReportKnownFailure {
                owner: execution_owner,
                failure: permanent,
            },
        )
        .expect("permanent failure is terminal")
        .record;
        assert_eq!(
            failed.state(),
            DurableCommandState::Failed { failure: permanent }
        );
    }

    #[test]
    fn unknown_commit_waits_through_indeterminate_reconciliation_then_retries() {
        let command = activation_command();
        let execution_owner = owner(20, command.writer_fence);
        let first_recovery_owner = owner(
            21,
            fence(22, command.writer_fence.generation.get().saturating_add(1)),
        );
        let second_recovery_owner = owner(
            23,
            fence(24, command.writer_fence.generation.get().saturating_add(2)),
        );
        let transaction = transaction(21);
        let prepared = start_and_prepare(&admitted(&command), execution_owner, transaction);
        let unknown = UnknownCommit {
            reason: UnknownCommitReason::ConnectionLost,
            diagnostic: DiagnosticRef::from_bytes(bytes32(50)),
        };
        assert_eq!(
            unknown.retry_classification(),
            RetryClassification::ReconcileBeforeDecision
        );
        let awaiting = reduce(
            &prepared,
            CommandEvent::ReportUnknownCommit {
                owner: execution_owner,
                transaction,
                unknown,
            },
        )
        .expect("unknown outcome is durable")
        .record;

        let still_unknown = CommandReducer::reduce(
            &awaiting,
            CommandEvent::ObserveReconciliation {
                owner: first_recovery_owner,
                transaction,
                observation: ReconciliationObservation::Indeterminate {
                    evidence: evidence(51),
                },
            },
            context_with_fence(&awaiting, first_recovery_owner.fence),
        )
        .expect("indeterminate readback remains blocked");
        assert_eq!(
            still_unknown.effect,
            ReductionEffect::ReconciliationStillRequired
        );
        assert!(matches!(
            still_unknown.record.state(),
            DurableCommandState::AwaitingReconciliation {
                execution_owner: observed_execution,
                recovery_owner: observed_recovery,
                probe_count: 1,
                last_evidence: Some(value),
                ..
            } if observed_execution == execution_owner
                && observed_recovery == first_recovery_owner
                && value == evidence(51)
        ));

        let absent = CommandReducer::reduce(
            &still_unknown.record,
            CommandEvent::ObserveReconciliation {
                owner: second_recovery_owner,
                transaction,
                observation: ReconciliationObservation::NotCommitted {
                    evidence: evidence(52),
                },
            },
            context_with_fence(&still_unknown.record, second_recovery_owner.fence),
        )
        .expect("proved absence permits retry")
        .record;
        assert_eq!(
            absent.state(),
            DurableCommandState::RetryReady {
                next_attempt: 2,
                required_fence: second_recovery_owner.fence,
                basis: RetryBasis::ReconciledNotCommitted(evidence(52)),
            }
        );
        assert_eq!(
            absent.recovery_obligation(),
            Some(CommandRecoveryObligation::RetryProvedNotCommitted)
        );
    }

    #[test]
    fn crash_reload_reconciles_committed_operation_without_reexecution() {
        let command = activation_command();
        let execution_owner = owner(20, command.writer_fence);
        let recovery_owner = owner(22, fence(23, command.writer_fence.generation.get() + 1));
        let transaction = transaction(21);
        let prepared = start_and_prepare(&admitted(&command), execution_owner, transaction);
        let unadvanced_owner = owner(24, command.writer_fence);
        assert_eq!(
            CommandReducer::reduce(
                &prepared,
                CommandEvent::ReportUnknownCommit {
                    owner: unadvanced_owner,
                    transaction,
                    unknown: UnknownCommit {
                        reason: UnknownCommitReason::ProcessInterrupted,
                        diagnostic: DiagnosticRef::from_bytes(bytes32(60)),
                    },
                },
                context_with_fence(&prepared, unadvanced_owner.fence),
            ),
            Err(CommandContractError::RecoveryFenceNotAdvanced)
        );
        let awaiting = CommandReducer::reduce(
            &prepared,
            CommandEvent::ReportUnknownCommit {
                owner: recovery_owner,
                transaction,
                unknown: UnknownCommit {
                    reason: UnknownCommitReason::ProcessInterrupted,
                    diagnostic: DiagnosticRef::from_bytes(bytes32(60)),
                },
            },
            context_with_fence(&prepared, recovery_owner.fence),
        )
        .expect("persist unknown state")
        .record;
        assert_eq!(
            awaiting.recovery_obligation(),
            Some(CommandRecoveryObligation::ReconcileCommit { transaction })
        );

        // Copying the value models an adapter loading the exact durable record after restart.
        let reloaded = awaiting;
        let result = activation_result(&command);
        let observation = CommandEvent::ObserveReconciliation {
            owner: recovery_owner,
            transaction,
            observation: ReconciliationObservation::Committed {
                evidence: evidence(61),
                result,
            },
        };
        let recovery_context = context_with_fence(&reloaded, recovery_owner.fence);
        let reconciled = CommandReducer::reduce(&reloaded, observation, recovery_context)
            .expect("durable readback decides committed")
            .record;
        assert_eq!(
            reconciled.state(),
            DurableCommandState::Succeeded {
                transaction,
                result,
                confirmation: CommitConfirmation::Reconciled(evidence(61)),
            }
        );
        assert_eq!(reconciled.recovery_obligation(), None);
        assert_eq!(
            CommandReducer::reduce(&reconciled, observation, recovery_context)
                .expect("replayed readback")
                .effect,
            ReductionEffect::IdempotentReplay
        );
    }
}
