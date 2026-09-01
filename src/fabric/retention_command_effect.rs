//! Typed `ApplyRetention` command effect and exact deletion-history reconciliation seam.
//!
//! Retention policy evaluation, the complete protected-resource closure, the library-native
//! dry run, and proof selection are immutable relational work. Preparation resolves those facts
//! into one deterministic application transaction. Commit reloads that exact prepared selection
//! and crosses one fenced durable boundary once. A conflict, ambiguous response, or visible
//! marker never authorizes an internal retry or guessed success.

use std::sync::Arc;

use async_trait::async_trait;

use super::command::{
    CommandCancellation, CommandFailure, CommandKind, CommandRecord, CommandResult, DiagnosticRef,
    ExecutionOwner, ExpectedHead, FabricCommand, FabricCommandPayload, OperationId,
    OperationSelectionRef, ProofReceiptRef, ProtectedSetRef, ReconciliationEvidenceRef,
    ReconciliationObservation, ReductionContext, RetentionPolicyRef, TransactionRef, UnknownCommit,
    UnknownCommitReason, WorkspaceId, WriterGeneration,
};
use super::command_actor::{CommandPortError, CommitEffectOutcome, PrepareEffectOutcome};
use super::command_effect_contract::{
    ValidatedCommandAttempt, executing_attempt, prepared_attempt, reconciliation_attempt,
};
use super::command_effect_router::RetentionCommandEffectPort;

/// Exact immutable relation row identifying one dry-run deletion plan.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RetentionDeletionPlanRef([u8; 32]);

impl RetentionDeletionPlanRef {
    /// Construct a reference from its canonical relation identity.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Borrow the canonical relation identity.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Exact immutable retention attempt supplied to relational resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionAttempt {
    validated: ValidatedCommandAttempt,
}

impl RetentionAttempt {
    fn from_validated(validated: ValidatedCommandAttempt) -> Result<Self, CommandPortError> {
        if matches!(
            validated.command().payload,
            FabricCommandPayload::ApplyRetention { .. }
        ) {
            Ok(Self { validated })
        } else {
            Err(CommandPortError::CorruptRecord)
        }
    }

    /// Immutable admitted `ApplyRetention` command.
    #[must_use]
    pub const fn command(self) -> FabricCommand {
        self.validated.command()
    }

    /// Reducer-owned attempt number.
    #[must_use]
    pub const fn attempt(self) -> u32 {
        self.validated.attempt()
    }

    /// Immutable actor/fence that executed this transaction attempt.
    #[must_use]
    pub const fn owner(self) -> ExecutionOwner {
        self.validated.execution_owner()
    }

    /// Exact policy-selected retention contract.
    #[must_use]
    pub fn policy(self) -> RetentionPolicyRef {
        let FabricCommandPayload::ApplyRetention { policy, .. } = self.command().payload else {
            unreachable!("retention attempt is constructed only after payload checks")
        };
        policy
    }

    /// Exact complete protected set which the deletion plan must preserve.
    #[must_use]
    pub fn protected(self) -> ProtectedSetRef {
        let FabricCommandPayload::ApplyRetention { protected, .. } = self.command().payload else {
            unreachable!("retention attempt is constructed only after payload checks")
        };
        protected
    }
}

/// Fully bound immutable retention selection.
///
/// `deletion_plan` identifies the exact library-native dry-run result and `proof_receipt`
/// identifies the proof that the plan excludes the complete protected closure. The transaction
/// is deterministic over this selection and is the durable lookup key used after preparation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedRetention {
    attempt: RetentionAttempt,
    deletion_plan: RetentionDeletionPlanRef,
    proof_receipt: ProofReceiptRef,
    transaction: TransactionRef,
}

impl ResolvedRetention {
    /// Bind one exact dry-run/proof selection to its deterministic transaction.
    #[must_use]
    pub const fn new(
        attempt: RetentionAttempt,
        deletion_plan: RetentionDeletionPlanRef,
        proof_receipt: ProofReceiptRef,
        transaction: TransactionRef,
    ) -> Self {
        Self {
            attempt,
            deletion_plan,
            proof_receipt,
            transaction,
        }
    }

    #[must_use]
    pub const fn attempt(self) -> RetentionAttempt {
        self.attempt
    }

    #[must_use]
    pub const fn deletion_plan(self) -> RetentionDeletionPlanRef {
        self.deletion_plan
    }

    #[must_use]
    pub const fn proof_receipt(self) -> ProofReceiptRef {
        self.proof_receipt
    }

    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.transaction
    }
}

/// Immutable retention-resolution result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetentionResolution {
    Resolved(ResolvedRetention),
    KnownFailure(CommandFailure),
    Cancelled(CommandCancellation),
}

/// Read-only policy, protection-closure, dry-run, and proof resolver.
///
/// Implementations query exact pinned relations. They must observe every retention-authority
/// source, evaluate the complete protected closure, validate a library-native dry run, prove the
/// selected deletion plan, and derive one deterministic transaction. They must not delete a
/// resource, write an operation marker, discover current/latest state, or mutate temporal state.
/// Re-resolving the same attempt after preparation must return the exact same selection.
#[async_trait]
pub trait RetentionResolverPort: Send + Sync {
    async fn resolve(
        &self,
        attempt: RetentionAttempt,
    ) -> Result<RetentionResolution, CommandPortError>;
}

/// Exact prepared selection presented to the sole durable retention boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionCommitRequest {
    resolved: ResolvedRetention,
}

impl RetentionCommitRequest {
    #[must_use]
    pub const fn attempt(self) -> RetentionAttempt {
        self.resolved.attempt
    }

    #[must_use]
    pub const fn deletion_plan(self) -> RetentionDeletionPlanRef {
        self.resolved.deletion_plan
    }

    #[must_use]
    pub const fn proof_receipt(self) -> ProofReceiptRef {
        self.resolved.proof_receipt
    }

    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.resolved.transaction
    }
}

/// Complete readback receipt for one retention application.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionCommitReceipt {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub transaction: TransactionRef,
    pub writer_generation: WriterGeneration,
    pub policy: RetentionPolicyRef,
    pub protected: ProtectedSetRef,
    pub resulting_head: ExpectedHead,
    pub operation_selection: OperationSelectionRef,
}

/// Exact marker visibility after one attempted retention transaction.
///
/// A marker is not the dry-run/proof-selected deletion history and cannot confirm success.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionMarkerReceipt {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub transaction: TransactionRef,
    pub writer_generation: WriterGeneration,
}

/// Exhaustive observation from one zero-retry retention attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetentionCommitObservation {
    Committed(RetentionCommitReceipt),
    MarkerAlreadyCommitted {
        marker: RetentionMarkerReceipt,
        diagnostic: DiagnosticRef,
    },
    Conflict {
        diagnostic: DiagnosticRef,
    },
    Unknown {
        reason: UnknownCommitReason,
        diagnostic: DiagnosticRef,
    },
}

/// Sole durable retention authority.
///
/// Implementations execute only the request's already-proved deletion plan, under its exact
/// operation, transaction, immutable execution generation, policy, and protected-set binding.
/// They perform one controlled application attempt and must not retry, rebase, recompute the
/// protected closure, rediscover latest state, or reduce command state.
#[async_trait]
pub trait RetentionCommitPort: Send + Sync {
    async fn commit(
        &self,
        request: RetentionCommitRequest,
    ) -> Result<RetentionCommitObservation, CommandPortError>;
}

/// Exact read-only marker/control-history lookup key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionReconciliationRequest {
    attempt: RetentionAttempt,
    active_recovery_owner: ExecutionOwner,
    transaction: TransactionRef,
}

impl RetentionReconciliationRequest {
    /// Original attempt, retaining the immutable transaction executor.
    #[must_use]
    pub const fn attempt(self) -> RetentionAttempt {
        self.attempt
    }

    /// Current actor/fence authorized to perform the read-only recovery query.
    #[must_use]
    pub const fn active_recovery_owner(self) -> ExecutionOwner {
        self.active_recovery_owner
    }

    /// Exact actor-persisted transaction marker.
    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.transaction
    }
}

/// Exact result of reading both the application marker and complete retention control history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetentionMarkerObservation {
    Committed {
        receipt: RetentionCommitReceipt,
        evidence: ReconciliationEvidenceRef,
    },
    ProvedNotCommitted {
        evidence: ReconciliationEvidenceRef,
    },
    Indeterminate {
        evidence: ReconciliationEvidenceRef,
    },
}

/// Read-only application-marker and complete retention-history authority.
///
/// Implementations query the exact transaction and [`RetentionAttempt::owner`] execution
/// generation while fencing the read with
/// [`RetentionReconciliationRequest::active_recovery_owner`]. They never write or delete while
/// reconciling and never infer absence from a current/latest lookup.
#[async_trait]
pub trait RetentionMarkerPort: Send + Sync {
    async fn read_exact(
        &self,
        request: RetentionReconciliationRequest,
    ) -> Result<RetentionMarkerObservation, CommandPortError>;
}

/// Concrete typed effect for `FabricCommandPayload::ApplyRetention`.
pub struct RetentionCommandEffect {
    resolver: Arc<dyn RetentionResolverPort>,
    commits: Arc<dyn RetentionCommitPort>,
    markers: Arc<dyn RetentionMarkerPort>,
}

impl std::fmt::Debug for RetentionCommandEffect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RetentionCommandEffect")
            .field("resolver", &"installed")
            .field("commits", &"installed")
            .field("markers", &"installed")
            .finish()
    }
}

impl RetentionCommandEffect {
    /// Install the three non-overlapping retention authorities.
    #[must_use]
    pub fn new(
        resolver: Arc<dyn RetentionResolverPort>,
        commits: Arc<dyn RetentionCommitPort>,
        markers: Arc<dyn RetentionMarkerPort>,
    ) -> Self {
        Self {
            resolver,
            commits,
            markers,
        }
    }
}

#[async_trait]
impl RetentionCommandEffectPort for RetentionCommandEffect {
    async fn prepare(
        &self,
        executing: &CommandRecord,
        owner: ExecutionOwner,
        context: ReductionContext,
    ) -> Result<PrepareEffectOutcome, CommandPortError> {
        let validated = executing_attempt(executing, owner, context, CommandKind::ApplyRetention)?;
        let attempt = RetentionAttempt::from_validated(validated)?;
        match self.resolver.resolve(attempt).await? {
            RetentionResolution::Resolved(resolved) => {
                if resolved.attempt() != attempt {
                    return Err(CommandPortError::CorruptRecord);
                }
                Ok(PrepareEffectOutcome::Prepared {
                    transaction: resolved.transaction(),
                })
            }
            RetentionResolution::KnownFailure(failure) => {
                Ok(PrepareEffectOutcome::KnownFailure { failure })
            }
            RetentionResolution::Cancelled(cancellation) => {
                Ok(PrepareEffectOutcome::Cancelled { cancellation })
            }
        }
    }

    async fn commit(
        &self,
        prepared: &CommandRecord,
        owner: ExecutionOwner,
        transaction: TransactionRef,
        context: ReductionContext,
    ) -> Result<CommitEffectOutcome, CommandPortError> {
        let validated = prepared_attempt(
            prepared,
            owner,
            transaction,
            context,
            CommandKind::ApplyRetention,
        )?;
        let attempt = RetentionAttempt::from_validated(validated)?;
        let RetentionResolution::Resolved(resolved) = self.resolver.resolve(attempt).await? else {
            return Err(CommandPortError::CorruptRecord);
        };
        if resolved.attempt() != attempt || resolved.transaction() != transaction {
            return Err(CommandPortError::CorruptRecord);
        }
        match self
            .commits
            .commit(RetentionCommitRequest { resolved })
            .await?
        {
            RetentionCommitObservation::Committed(receipt) => {
                validate_commit_receipt(attempt, transaction, receipt)?;
                Ok(CommitEffectOutcome::Committed {
                    result: result_from_receipt(receipt),
                })
            }
            RetentionCommitObservation::MarkerAlreadyCommitted { marker, diagnostic } => {
                validate_direct_marker(attempt, transaction, marker)?;
                Ok(unknown(
                    UnknownCommitReason::ReadbackUnavailable,
                    diagnostic,
                ))
            }
            RetentionCommitObservation::Conflict { diagnostic } => Ok(unknown(
                UnknownCommitReason::ReadbackUnavailable,
                diagnostic,
            )),
            RetentionCommitObservation::Unknown { reason, diagnostic } => {
                Ok(unknown(reason, diagnostic))
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
            CommandKind::ApplyRetention,
        )?;
        let attempt = RetentionAttempt::from_validated(recovery.attempt())?;
        let request = RetentionReconciliationRequest {
            attempt,
            active_recovery_owner: recovery.active_recovery_owner(),
            transaction,
        };
        match self.markers.read_exact(request).await? {
            RetentionMarkerObservation::Committed { receipt, evidence } => {
                validate_commit_receipt(attempt, transaction, receipt)?;
                Ok(ReconciliationObservation::Committed {
                    evidence,
                    result: result_from_receipt(receipt),
                })
            }
            RetentionMarkerObservation::ProvedNotCommitted { evidence } => {
                Ok(ReconciliationObservation::NotCommitted { evidence })
            }
            RetentionMarkerObservation::Indeterminate { evidence } => {
                Ok(ReconciliationObservation::Indeterminate { evidence })
            }
        }
    }
}

fn validate_commit_receipt(
    attempt: RetentionAttempt,
    transaction: TransactionRef,
    receipt: RetentionCommitReceipt,
) -> Result<(), CommandPortError> {
    attempt
        .validated
        .validate_receipt_generation(receipt.writer_generation)?;
    let command = attempt.command();
    if receipt.workspace_id != command.ownership.workspace_id
        || receipt.operation_id != command.identity.operation_id
        || receipt.transaction != transaction
        || receipt.policy != attempt.policy()
        || receipt.protected != attempt.protected()
        || receipt.resulting_head != command.expected_head
    {
        return Err(CommandPortError::CorruptRecord);
    }
    Ok(())
}

fn validate_direct_marker(
    attempt: RetentionAttempt,
    transaction: TransactionRef,
    marker: RetentionMarkerReceipt,
) -> Result<(), CommandPortError> {
    attempt
        .validated
        .validate_receipt_generation(marker.writer_generation)?;
    let command = attempt.command();
    if marker.workspace_id != command.ownership.workspace_id
        || marker.operation_id != command.identity.operation_id
        || marker.transaction != transaction
    {
        return Err(CommandPortError::CorruptRecord);
    }
    Ok(())
}

const fn result_from_receipt(receipt: RetentionCommitReceipt) -> CommandResult {
    CommandResult::RetentionApplied {
        protected: receipt.protected,
        resulting_head: receipt.resulting_head,
        selection: receipt.operation_selection,
    }
}

const fn unknown(reason: UnknownCommitReason, diagnostic: DiagnosticRef) -> CommitEffectOutcome {
    CommitEffectOutcome::Unknown {
        unknown: UnknownCommit { reason, diagnostic },
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::fabric::command::{
        ActorId, AdmissionContext, AdmissionOutcome, AuthorizationDecision, AuthorizationRef,
        CommandEvent, CommandIdentity, CommandOwnership, CommandPins, CommandReducer,
        IdempotencyKey, InputReleaseRef, LeaseId, PrincipalId, ProgramReleaseRef, ProviderSetRef,
        ResourceEnvelopeRef, SourceGeneration, WriterFence,
    };

    struct ResolverProbe {
        transaction: Mutex<TransactionRef>,
        resolved_attempt: Mutex<Option<RetentionAttempt>>,
        requests: Mutex<Vec<RetentionAttempt>>,
    }

    #[async_trait]
    impl RetentionResolverPort for ResolverProbe {
        async fn resolve(
            &self,
            attempt: RetentionAttempt,
        ) -> Result<RetentionResolution, CommandPortError> {
            self.requests
                .lock()
                .expect("resolver requests lock")
                .push(attempt);
            let resolved_attempt = self
                .resolved_attempt
                .lock()
                .expect("resolved-attempt lock")
                .unwrap_or(attempt);
            let transaction = *self.transaction.lock().expect("transaction lock");
            Ok(RetentionResolution::Resolved(ResolvedRetention::new(
                resolved_attempt,
                deletion_plan(),
                proof(),
                transaction,
            )))
        }
    }

    struct CommitProbe {
        observation: RetentionCommitObservation,
        requests: Mutex<Vec<RetentionCommitRequest>>,
    }

    #[async_trait]
    impl RetentionCommitPort for CommitProbe {
        async fn commit(
            &self,
            request: RetentionCommitRequest,
        ) -> Result<RetentionCommitObservation, CommandPortError> {
            self.requests
                .lock()
                .expect("commit requests lock")
                .push(request);
            Ok(self.observation)
        }
    }

    struct MarkerProbe {
        observation: RetentionMarkerObservation,
        requests: Mutex<Vec<RetentionReconciliationRequest>>,
    }

    #[async_trait]
    impl RetentionMarkerPort for MarkerProbe {
        async fn read_exact(
            &self,
            request: RetentionReconciliationRequest,
        ) -> Result<RetentionMarkerObservation, CommandPortError> {
            self.requests
                .lock()
                .expect("marker requests lock")
                .push(request);
            Ok(self.observation)
        }
    }

    struct Harness {
        effect: RetentionCommandEffect,
        resolver: Arc<ResolverProbe>,
        commits: Arc<CommitProbe>,
        markers: Arc<MarkerProbe>,
    }

    impl Harness {
        fn new(commit: RetentionCommitObservation, marker: RetentionMarkerObservation) -> Self {
            let resolver = Arc::new(ResolverProbe {
                transaction: Mutex::new(transaction()),
                resolved_attempt: Mutex::new(None),
                requests: Mutex::new(Vec::new()),
            });
            let commits = Arc::new(CommitProbe {
                observation: commit,
                requests: Mutex::new(Vec::new()),
            });
            let markers = Arc::new(MarkerProbe {
                observation: marker,
                requests: Mutex::new(Vec::new()),
            });
            let effect =
                RetentionCommandEffect::new(resolver.clone(), commits.clone(), markers.clone());
            Self {
                effect,
                resolver,
                commits,
                markers,
            }
        }
    }

    #[tokio::test]
    async fn prepare_is_read_only_and_commit_receives_exact_proved_plan() {
        let owner = owner(1, 1, 1);
        let executing = executing_record(command(owner.fence), owner);
        let harness = Harness::new(
            RetentionCommitObservation::Committed(receipt(owner.fence.generation)),
            indeterminate_marker(),
        );

        assert_eq!(
            harness
                .effect
                .prepare(&executing, owner, context(owner))
                .await
                .expect("read-only retention resolution"),
            PrepareEffectOutcome::Prepared {
                transaction: transaction()
            }
        );
        assert!(
            harness
                .commits
                .requests
                .lock()
                .expect("commit requests lock")
                .is_empty()
        );

        let prepared = prepared_record(executing, owner);
        assert_eq!(
            harness
                .effect
                .commit(&prepared, owner, transaction(), context(owner))
                .await
                .expect("exact retention readback"),
            CommitEffectOutcome::Committed {
                result: expected_result()
            }
        );
        let request = harness
            .commits
            .requests
            .lock()
            .expect("commit requests lock")[0];
        assert_eq!(request.attempt().policy(), policy());
        assert_eq!(request.attempt().protected(), protected());
        assert_eq!(request.deletion_plan(), deletion_plan());
        assert_eq!(request.proof_receipt(), proof());
        assert_eq!(request.transaction(), transaction());
        assert_eq!(
            harness
                .resolver
                .requests
                .lock()
                .expect("resolver requests lock")
                .len(),
            2,
            "commit reloads the exact immutable prepared selection instead of discovering latest"
        );
    }

    #[tokio::test]
    async fn resolver_cannot_substitute_policy_or_protected_set() {
        let owner = owner(1, 1, 1);
        let executing = executing_record(command(owner.fence), owner);
        let harness = Harness::new(
            RetentionCommitObservation::Committed(receipt(owner.fence.generation)),
            indeterminate_marker(),
        );
        let mut substituted = command(owner.fence);
        substituted.payload = FabricCommandPayload::ApplyRetention {
            policy: RetentionPolicyRef::from_bytes([0x81; 32]),
            protected: ProtectedSetRef::from_bytes([0x82; 32]),
        };
        let substituted_executing = executing_record(substituted, owner);
        let substituted_attempt = RetentionAttempt::from_validated(
            executing_attempt(
                &substituted_executing,
                owner,
                context(owner),
                CommandKind::ApplyRetention,
            )
            .expect("validate substituted retention attempt"),
        )
        .expect("typed substituted retention attempt");
        *harness
            .resolver
            .resolved_attempt
            .lock()
            .expect("resolved-attempt lock") = Some(substituted_attempt);

        assert_eq!(
            harness
                .effect
                .prepare(&executing, owner, context(owner))
                .await,
            Err(CommandPortError::CorruptRecord)
        );
        assert!(
            harness
                .commits
                .requests
                .lock()
                .expect("commit requests lock")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn receipt_requires_policy_protected_set_and_execution_generation() {
        let owner = owner(1, 1, 1);
        let mut missing_protected = receipt(owner.fence.generation);
        missing_protected.protected = ProtectedSetRef::from_bytes([0; 32]);
        let mut wrong_policy = receipt(owner.fence.generation);
        wrong_policy.policy = RetentionPolicyRef::from_bytes([0x83; 32]);
        let wrong_generation = receipt(generation(2));

        for invalid in [missing_protected, wrong_policy, wrong_generation] {
            let harness = Harness::new(
                RetentionCommitObservation::Committed(invalid),
                indeterminate_marker(),
            );
            let prepared = prepared_record(executing_record(command(owner.fence), owner), owner);
            assert_eq!(
                harness
                    .effect
                    .commit(&prepared, owner, transaction(), context(owner))
                    .await,
                Err(CommandPortError::CorruptRecord)
            );
        }
    }

    #[tokio::test]
    async fn marker_visibility_and_conflict_remain_unknown_without_retry() {
        let owner = owner(1, 1, 1);
        let observations = [
            RetentionCommitObservation::MarkerAlreadyCommitted {
                marker: marker_receipt(owner.fence.generation),
                diagnostic: diagnostic(41),
            },
            RetentionCommitObservation::Conflict {
                diagnostic: diagnostic(42),
            },
        ];

        for observation in observations {
            let harness = Harness::new(observation, indeterminate_marker());
            let prepared = prepared_record(executing_record(command(owner.fence), owner), owner);
            assert!(matches!(
                harness
                    .effect
                    .commit(&prepared, owner, transaction(), context(owner))
                    .await
                    .expect("ambiguous retention result"),
                CommitEffectOutcome::Unknown { .. }
            ));
            assert_eq!(
                harness
                    .commits
                    .requests
                    .lock()
                    .expect("commit requests lock")
                    .len(),
                1,
                "the effect never retries retention"
            );
        }
    }

    #[tokio::test]
    async fn recovery_reads_exact_old_execution_under_new_active_fence() {
        let execution = owner(1, 1, 1);
        let recovery = owner(2, 2, 2);
        let awaiting = awaiting_record(
            prepared_record(
                executing_record(command(execution.fence), execution),
                execution,
            ),
            execution,
        );
        let evidence = ReconciliationEvidenceRef::from_bytes([0x91; 32]);
        let harness = Harness::new(
            RetentionCommitObservation::Unknown {
                reason: UnknownCommitReason::ProcessInterrupted,
                diagnostic: diagnostic(51),
            },
            RetentionMarkerObservation::Committed {
                receipt: receipt(execution.fence.generation),
                evidence,
            },
        );

        assert_eq!(
            harness
                .effect
                .reconcile(
                    &awaiting,
                    recovery,
                    transaction(),
                    ReductionContext {
                        current_head: ExpectedHead::Empty,
                        active_fence: recovery.fence,
                    },
                )
                .await
                .expect("old execution is proved under active recovery fence"),
            ReconciliationObservation::Committed {
                evidence,
                result: expected_result()
            }
        );
        let request = harness
            .markers
            .requests
            .lock()
            .expect("marker requests lock")[0];
        assert_eq!(request.attempt().owner(), execution);
        assert_eq!(request.active_recovery_owner(), recovery);
        assert_eq!(request.transaction(), transaction());
    }

    #[tokio::test]
    async fn recovery_rejects_intermediate_generation_as_commit_authority() {
        let admitted = owner(1, 1, 1);
        let execution = owner(2, 2, 2);
        let intermediate = owner(3, 3, 3);
        let recovery = owner(4, 4, 4);
        let awaiting = awaiting_record(
            prepared_record(
                executing_record(command(admitted.fence), execution),
                execution,
            ),
            execution,
        );
        let harness = Harness::new(
            RetentionCommitObservation::Unknown {
                reason: UnknownCommitReason::ProcessInterrupted,
                diagnostic: diagnostic(52),
            },
            RetentionMarkerObservation::Committed {
                receipt: receipt(intermediate.fence.generation),
                evidence: ReconciliationEvidenceRef::from_bytes([0x92; 32]),
            },
        );

        assert_eq!(
            harness
                .effect
                .reconcile(
                    &awaiting,
                    recovery,
                    transaction(),
                    ReductionContext {
                        current_head: ExpectedHead::Empty,
                        active_fence: recovery.fence,
                    },
                )
                .await,
            Err(CommandPortError::CorruptRecord)
        );
    }

    #[tokio::test]
    async fn proved_noncommit_requires_and_preserves_evidence() {
        let execution = owner(1, 1, 1);
        let recovery = owner(2, 2, 2);
        let awaiting = awaiting_record(
            prepared_record(
                executing_record(command(execution.fence), execution),
                execution,
            ),
            execution,
        );
        let evidence = ReconciliationEvidenceRef::from_bytes([0x93; 32]);
        let harness = Harness::new(
            RetentionCommitObservation::Unknown {
                reason: UnknownCommitReason::ProcessInterrupted,
                diagnostic: diagnostic(53),
            },
            RetentionMarkerObservation::ProvedNotCommitted { evidence },
        );

        assert_eq!(
            harness
                .effect
                .reconcile(
                    &awaiting,
                    recovery,
                    transaction(),
                    ReductionContext {
                        current_head: ExpectedHead::Empty,
                        active_fence: recovery.fence,
                    },
                )
                .await
                .expect("exact history proves noncommit"),
            ReconciliationObservation::NotCommitted { evidence }
        );
    }

    fn command(writer_fence: WriterFence) -> FabricCommand {
        FabricCommand {
            identity: CommandIdentity {
                operation_id: operation_id(),
                idempotency_key: IdempotencyKey::from_bytes([0x02; 32]),
            },
            ownership: CommandOwnership {
                workspace_id: workspace_id(),
                principal_id: PrincipalId::from_bytes([0x03; 16]),
                authorization: AuthorizationRef::from_bytes([0x04; 32]),
            },
            expected_head: ExpectedHead::Empty,
            writer_fence,
            pins: CommandPins {
                input_release: InputReleaseRef::from_bytes([0x05; 32]),
                program_release: ProgramReleaseRef::from_bytes([0x06; 32]),
                application_release: crate::fabric::command::ApplicationReleaseRef::from_bytes(
                    [0x06; 32],
                ),
                source_authority: crate::fabric::command::SourceAuthorityRef::from_bytes(
                    [0x06; 32],
                ),
                provider_release: crate::fabric::command::ProviderReleaseRef::from_bytes(
                    [0x06; 32],
                ),
                source_generation: SourceGeneration::new(7),
                provider_set: ProviderSetRef::from_bytes([0x08; 32]),
            },
            resources: ResourceEnvelopeRef::from_bytes([0x09; 32]),
            payload: FabricCommandPayload::ApplyRetention {
                policy: policy(),
                protected: protected(),
            },
        }
    }

    fn executing_record(command: FabricCommand, owner: ExecutionOwner) -> CommandRecord {
        let admitted = CommandReducer::admit(
            None,
            &command,
            AdmissionContext {
                workspace_id: workspace_id(),
                current_head: command.expected_head,
                active_fence: command.writer_fence,
                authorization: AuthorizationDecision::Authorized(command.ownership.authorization),
            },
        )
        .expect("admit retention command");
        let AdmissionOutcome::New(admitted) = admitted else {
            panic!("empty admission creates a record")
        };
        CommandReducer::reduce(&admitted, CommandEvent::Start { owner }, context(owner))
            .expect("start retention command")
            .record
    }

    fn prepared_record(executing: CommandRecord, owner: ExecutionOwner) -> CommandRecord {
        CommandReducer::reduce(
            &executing,
            CommandEvent::PrepareCommit {
                owner,
                transaction: transaction(),
            },
            context(owner),
        )
        .expect("prepare retention transaction")
        .record
    }

    fn awaiting_record(prepared: CommandRecord, owner: ExecutionOwner) -> CommandRecord {
        CommandReducer::reduce(
            &prepared,
            CommandEvent::ReportUnknownCommit {
                owner,
                transaction: transaction(),
                unknown: UnknownCommit {
                    reason: UnknownCommitReason::ProcessInterrupted,
                    diagnostic: diagnostic(61),
                },
            },
            context(owner),
        )
        .expect("record unknown retention commit")
        .record
    }

    fn receipt(writer_generation: WriterGeneration) -> RetentionCommitReceipt {
        RetentionCommitReceipt {
            workspace_id: workspace_id(),
            operation_id: operation_id(),
            transaction: transaction(),
            writer_generation,
            policy: policy(),
            protected: protected(),
            resulting_head: ExpectedHead::Empty,
            operation_selection: selection(),
        }
    }

    fn marker_receipt(writer_generation: WriterGeneration) -> RetentionMarkerReceipt {
        RetentionMarkerReceipt {
            workspace_id: workspace_id(),
            operation_id: operation_id(),
            transaction: transaction(),
            writer_generation,
        }
    }

    fn expected_result() -> CommandResult {
        CommandResult::RetentionApplied {
            protected: protected(),
            resulting_head: ExpectedHead::Empty,
            selection: selection(),
        }
    }

    fn indeterminate_marker() -> RetentionMarkerObservation {
        RetentionMarkerObservation::Indeterminate {
            evidence: ReconciliationEvidenceRef::from_bytes([0x94; 32]),
        }
    }

    fn context(owner: ExecutionOwner) -> ReductionContext {
        ReductionContext {
            current_head: ExpectedHead::Empty,
            active_fence: owner.fence,
        }
    }

    fn owner(actor: u8, lease: u8, generation_value: u64) -> ExecutionOwner {
        ExecutionOwner {
            actor_id: ActorId::from_bytes([actor; 16]),
            fence: WriterFence {
                lease_id: LeaseId::from_bytes([lease; 16]),
                generation: generation(generation_value),
            },
        }
    }

    fn generation(value: u64) -> WriterGeneration {
        WriterGeneration::new(value).expect("nonzero writer generation")
    }

    fn workspace_id() -> WorkspaceId {
        WorkspaceId::from_bytes([0x11; 16])
    }

    fn operation_id() -> OperationId {
        OperationId::from_bytes([0x12; 16])
    }

    fn policy() -> RetentionPolicyRef {
        RetentionPolicyRef::from_bytes([0x13; 32])
    }

    fn protected() -> ProtectedSetRef {
        ProtectedSetRef::from_bytes([0x14; 32])
    }

    fn deletion_plan() -> RetentionDeletionPlanRef {
        RetentionDeletionPlanRef::from_bytes([0x15; 32])
    }

    fn proof() -> ProofReceiptRef {
        ProofReceiptRef::from_bytes([0x16; 32])
    }

    fn transaction() -> TransactionRef {
        TransactionRef::from_bytes([0x17; 32])
    }

    fn selection() -> OperationSelectionRef {
        OperationSelectionRef::from_bytes([0x18; 32])
    }

    fn diagnostic(seed: u8) -> DiagnosticRef {
        DiagnosticRef::from_bytes([seed; 32])
    }
}
