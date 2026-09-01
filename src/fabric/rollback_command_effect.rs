//! Typed `RollbackEpoch` command effect and exact activation-history recovery seam.
//!
//! Rollback is a governed forward selection of one retained epoch. Preparation
//! resolves and validates the exact target, rollback authorization,
//! compatibility, and proof inputs without writing a durable target. Commit
//! has one rollback-event authority and never retries, rebases, or discovers
//! latest state. Recovery cannot append: it reads the exact operation marker
//! and complete activation control history under the current recovery fence.

use std::sync::Arc;

use async_trait::async_trait;

use super::command::{
    CommandCancellation, CommandFailure, CommandKind, CommandRecord, CommandResult, DiagnosticRef,
    EpochId, ExecutionOwner, FabricCommand, FabricCommandPayload, OperationId,
    OperationSelectionRef, ReconciliationEvidenceRef, ReconciliationObservation, ReductionContext,
    RollbackAuthorizationRef, TransactionRef, UnknownCommit, UnknownCommitReason, WorkspaceId,
    WriterGeneration,
};
use super::command_actor::{CommandPortError, CommitEffectOutcome, PrepareEffectOutcome};
use super::command_effect_contract::{
    ValidatedCommandAttempt, executing_attempt, prepared_attempt, reconciliation_attempt,
};
use super::command_effect_router::RollbackCommandEffectPort;

/// Exact immutable rollback attempt supplied to governed request resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RollbackAttempt {
    validated: ValidatedCommandAttempt,
}

impl RollbackAttempt {
    fn from_validated(validated: ValidatedCommandAttempt) -> Result<Self, CommandPortError> {
        if matches!(
            validated.command().payload,
            FabricCommandPayload::RollbackEpoch { .. }
        ) {
            Ok(Self { validated })
        } else {
            Err(CommandPortError::CorruptRecord)
        }
    }

    /// Immutable admitted `RollbackEpoch` command.
    #[must_use]
    pub const fn command(self) -> FabricCommand {
        self.validated.command()
    }

    /// Reducer-owned attempt number.
    #[must_use]
    pub const fn attempt(self) -> u32 {
        self.validated.attempt()
    }

    /// Actor and immutable writer fence that execute this transaction.
    #[must_use]
    pub const fn execution_owner(self) -> ExecutionOwner {
        self.validated.execution_owner()
    }

    /// Exact retained epoch selected by the rollback command.
    #[must_use]
    pub fn target_epoch(self) -> EpochId {
        let FabricCommandPayload::RollbackEpoch { target_epoch, .. } = self.command().payload
        else {
            unreachable!("rollback attempt is constructed only after payload checks")
        };
        target_epoch
    }

    /// Exact governed rollback authorization supplied by the command.
    #[must_use]
    pub fn rollback_authorization(self) -> RollbackAuthorizationRef {
        let FabricCommandPayload::RollbackEpoch { authorization, .. } = self.command().payload
        else {
            unreachable!("rollback attempt is constructed only after payload checks")
        };
        authorization
    }
}

/// Fully bound result of read-only rollback governance resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedRollback {
    attempt: RollbackAttempt,
    transaction: TransactionRef,
}

impl ResolvedRollback {
    /// Bind a deterministic application transaction to the exact validated attempt.
    #[must_use]
    pub const fn new(attempt: RollbackAttempt, transaction: TransactionRef) -> Self {
        Self {
            attempt,
            transaction,
        }
    }

    #[must_use]
    pub const fn attempt(self) -> RollbackAttempt {
        self.attempt
    }

    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.transaction
    }
}

/// Exhaustive result of immutable rollback request validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RollbackResolution {
    Resolved(ResolvedRollback),
    KnownFailure(CommandFailure),
    Cancelled(CommandCancellation),
}

/// Read-only retained-target, authorization, compatibility, and proof authority.
///
/// Implementations validate all model-governed rollback inputs and derive the
/// deterministic transaction. They must not append an activation event,
/// operation marker, or control-history row.
#[async_trait]
pub trait RollbackGovernanceResolverPort: Send + Sync {
    async fn resolve(
        &self,
        attempt: RollbackAttempt,
    ) -> Result<RollbackResolution, CommandPortError>;
}

/// Exact contract presented to the sole durable rollback-event boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RollbackCommitRequest {
    attempt: RollbackAttempt,
    transaction: TransactionRef,
}

impl RollbackCommitRequest {
    #[must_use]
    pub const fn attempt(self) -> RollbackAttempt {
        self.attempt
    }

    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.transaction
    }
}

/// Complete rollback-event readback needed to construct one command result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RollbackCommitReceipt {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub transaction: TransactionRef,
    pub writer_generation: WriterGeneration,
    pub target_epoch: EpochId,
    pub rollback_authorization: RollbackAuthorizationRef,
    pub operation_selection: OperationSelectionRef,
    pub selected_epoch: EpochId,
}

/// Exact application-marker visibility after one direct rollback attempt.
///
/// A marker is not complete activation history and therefore cannot confirm
/// rollback success.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RollbackMarkerReceipt {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub transaction: TransactionRef,
    pub writer_generation: WriterGeneration,
}

/// Exhaustive observation from one zero-retry rollback-event attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RollbackCommitObservation {
    Committed(RollbackCommitReceipt),
    MarkerAlreadyCommitted {
        marker: RollbackMarkerReceipt,
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

/// Sole durable rollback-event append/readback authority.
///
/// Implementations execute the already resolved request exactly once. They
/// must not retry, rebase, discover latest state, or convert marker visibility
/// into success.
#[async_trait]
pub trait RollbackEventCommitPort: Send + Sync {
    async fn commit(
        &self,
        request: RollbackCommitRequest,
    ) -> Result<RollbackCommitObservation, CommandPortError>;
}

/// Exact marker/control-history query for an interrupted rollback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RollbackReconciliationRequest {
    attempt: RollbackAttempt,
    active_recovery_owner: ExecutionOwner,
    transaction: TransactionRef,
}

impl RollbackReconciliationRequest {
    /// Original attempt, retaining its immutable transaction executor.
    #[must_use]
    pub const fn attempt(self) -> RollbackAttempt {
        self.attempt
    }

    /// Current actor/fence authorizing the read-only recovery query.
    #[must_use]
    pub const fn active_recovery_owner(self) -> ExecutionOwner {
        self.active_recovery_owner
    }

    /// Original actor-persisted transaction marker.
    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.transaction
    }
}

/// Exact result of reading both the marker and complete activation history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RollbackOperationMarkerObservation {
    Committed {
        receipt: RollbackCommitReceipt,
        evidence: ReconciliationEvidenceRef,
    },
    ProvedNotCommitted {
        evidence: ReconciliationEvidenceRef,
    },
    Indeterminate {
        evidence: ReconciliationEvidenceRef,
    },
}

/// Read-only exact operation-marker and activation-control-history authority.
///
/// The queried transaction and generation come from
/// [`RollbackAttempt::execution_owner`]; the read itself is fenced by
/// [`RollbackReconciliationRequest::active_recovery_owner`]. This port has no
/// append method by construction.
#[async_trait]
pub trait RollbackOperationMarkerPort: Send + Sync {
    async fn read_exact(
        &self,
        request: RollbackReconciliationRequest,
    ) -> Result<RollbackOperationMarkerObservation, CommandPortError>;
}

/// Typed rollback effect installed in the exhaustive command router.
pub struct RollbackCommandEffect {
    resolver: Arc<dyn RollbackGovernanceResolverPort>,
    commits: Arc<dyn RollbackEventCommitPort>,
    markers: Arc<dyn RollbackOperationMarkerPort>,
}

impl std::fmt::Debug for RollbackCommandEffect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RollbackCommandEffect")
            .field("resolver", &"installed")
            .field("commits", &"installed")
            .field("markers", &"installed")
            .finish()
    }
}

impl RollbackCommandEffect {
    #[must_use]
    pub const fn new(
        resolver: Arc<dyn RollbackGovernanceResolverPort>,
        commits: Arc<dyn RollbackEventCommitPort>,
        markers: Arc<dyn RollbackOperationMarkerPort>,
    ) -> Self {
        Self {
            resolver,
            commits,
            markers,
        }
    }
}

#[async_trait]
impl RollbackCommandEffectPort for RollbackCommandEffect {
    async fn prepare(
        &self,
        executing: &CommandRecord,
        owner: ExecutionOwner,
        context: ReductionContext,
    ) -> Result<PrepareEffectOutcome, CommandPortError> {
        let validated = executing_attempt(executing, owner, context, CommandKind::RollbackEpoch)?;
        let attempt = RollbackAttempt::from_validated(validated)?;
        match self.resolver.resolve(attempt).await? {
            RollbackResolution::Resolved(resolved) => {
                if resolved.attempt() != attempt {
                    return Err(CommandPortError::CorruptRecord);
                }
                Ok(PrepareEffectOutcome::Prepared {
                    transaction: resolved.transaction(),
                })
            }
            RollbackResolution::KnownFailure(failure) => {
                Ok(PrepareEffectOutcome::KnownFailure { failure })
            }
            RollbackResolution::Cancelled(cancellation) => {
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
            CommandKind::RollbackEpoch,
        )?;
        let attempt = RollbackAttempt::from_validated(validated)?;
        let request = RollbackCommitRequest {
            attempt,
            transaction,
        };
        match self.commits.commit(request).await? {
            RollbackCommitObservation::Committed(receipt) => {
                validate_receipt(attempt, transaction, receipt)?;
                Ok(CommitEffectOutcome::Committed {
                    result: result_from_receipt(receipt),
                })
            }
            RollbackCommitObservation::MarkerAlreadyCommitted { marker, diagnostic } => {
                validate_marker(attempt, transaction, marker)?;
                Ok(unknown(
                    UnknownCommitReason::ReadbackUnavailable,
                    diagnostic,
                ))
            }
            RollbackCommitObservation::Conflict { diagnostic } => Ok(unknown(
                UnknownCommitReason::ReadbackUnavailable,
                diagnostic,
            )),
            RollbackCommitObservation::Unknown { reason, diagnostic } => {
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
            CommandKind::RollbackEpoch,
        )?;
        let attempt = RollbackAttempt::from_validated(recovery.attempt())?;
        let request = RollbackReconciliationRequest {
            attempt,
            active_recovery_owner: recovery.active_recovery_owner(),
            transaction,
        };
        match self.markers.read_exact(request).await? {
            RollbackOperationMarkerObservation::Committed { receipt, evidence } => {
                validate_receipt(attempt, transaction, receipt)?;
                Ok(ReconciliationObservation::Committed {
                    evidence,
                    result: result_from_receipt(receipt),
                })
            }
            RollbackOperationMarkerObservation::ProvedNotCommitted { evidence } => {
                Ok(ReconciliationObservation::NotCommitted { evidence })
            }
            RollbackOperationMarkerObservation::Indeterminate { evidence } => {
                Ok(ReconciliationObservation::Indeterminate { evidence })
            }
        }
    }
}

fn validate_receipt(
    attempt: RollbackAttempt,
    transaction: TransactionRef,
    receipt: RollbackCommitReceipt,
) -> Result<(), CommandPortError> {
    attempt
        .validated
        .validate_receipt_generation(receipt.writer_generation)?;
    let command = attempt.command();
    if receipt.workspace_id != command.ownership.workspace_id
        || receipt.operation_id != command.identity.operation_id
        || receipt.transaction != transaction
        || receipt.target_epoch != attempt.target_epoch()
        || receipt.rollback_authorization != attempt.rollback_authorization()
        || receipt.selected_epoch != attempt.target_epoch()
    {
        return Err(CommandPortError::CorruptRecord);
    }
    Ok(())
}

fn validate_marker(
    attempt: RollbackAttempt,
    transaction: TransactionRef,
    marker: RollbackMarkerReceipt,
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

const fn result_from_receipt(receipt: RollbackCommitReceipt) -> CommandResult {
    CommandResult::EpochRolledBack {
        epoch: receipt.selected_epoch,
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
        CommandEvent, CommandIdentity, CommandOwnership, CommandPins, CommandReducer, ExpectedHead,
        IdempotencyKey, InputReleaseRef, LeaseId, PrincipalId, ProgramReleaseRef, ProviderSetRef,
        ResourceEnvelopeRef, SourceGeneration, WriterFence,
    };

    struct ResolverProbe {
        transaction: TransactionRef,
        substituted_payload: Option<FabricCommandPayload>,
        requests: Mutex<Vec<RollbackAttempt>>,
    }

    #[async_trait]
    impl RollbackGovernanceResolverPort for ResolverProbe {
        async fn resolve(
            &self,
            attempt: RollbackAttempt,
        ) -> Result<RollbackResolution, CommandPortError> {
            self.requests
                .lock()
                .expect("resolver requests lock")
                .push(attempt);
            let resolved_attempt = if let Some(payload) = self.substituted_payload {
                let execution_owner = attempt.execution_owner();
                let command = FabricCommand {
                    payload,
                    ..attempt.command()
                };
                let record = executing_record(command, execution_owner);
                let validated = executing_attempt(
                    &record,
                    execution_owner,
                    ReductionContext {
                        current_head: command.expected_head,
                        active_fence: execution_owner.fence,
                    },
                    CommandKind::RollbackEpoch,
                )?;
                RollbackAttempt::from_validated(validated)?
            } else {
                attempt
            };
            Ok(RollbackResolution::Resolved(ResolvedRollback::new(
                resolved_attempt,
                self.transaction,
            )))
        }
    }

    struct CommitProbe {
        observation: RollbackCommitObservation,
        requests: Mutex<Vec<RollbackCommitRequest>>,
    }

    #[async_trait]
    impl RollbackEventCommitPort for CommitProbe {
        async fn commit(
            &self,
            request: RollbackCommitRequest,
        ) -> Result<RollbackCommitObservation, CommandPortError> {
            self.requests
                .lock()
                .expect("commit requests lock")
                .push(request);
            Ok(self.observation)
        }
    }

    struct MarkerProbe {
        observation: RollbackOperationMarkerObservation,
        requests: Mutex<Vec<RollbackReconciliationRequest>>,
    }

    #[async_trait]
    impl RollbackOperationMarkerPort for MarkerProbe {
        async fn read_exact(
            &self,
            request: RollbackReconciliationRequest,
        ) -> Result<RollbackOperationMarkerObservation, CommandPortError> {
            self.requests
                .lock()
                .expect("marker requests lock")
                .push(request);
            Ok(self.observation)
        }
    }

    struct Harness {
        effect: RollbackCommandEffect,
        resolver: Arc<ResolverProbe>,
        commits: Arc<CommitProbe>,
        markers: Arc<MarkerProbe>,
    }

    impl Harness {
        fn new(
            commit: RollbackCommitObservation,
            marker: RollbackOperationMarkerObservation,
        ) -> Self {
            Self::with_resolver_payload(commit, marker, None)
        }

        fn with_resolver_payload(
            commit: RollbackCommitObservation,
            marker: RollbackOperationMarkerObservation,
            substituted_payload: Option<FabricCommandPayload>,
        ) -> Self {
            let resolver = Arc::new(ResolverProbe {
                transaction: transaction(),
                substituted_payload,
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
                RollbackCommandEffect::new(resolver.clone(), commits.clone(), markers.clone());
            Self {
                effect,
                resolver,
                commits,
                markers,
            }
        }
    }

    #[tokio::test]
    async fn prepare_is_read_only_and_exact_commit_selects_the_governed_target() {
        let owner = owner(1, 1, 1);
        let executing = executing_record(command(owner.fence), owner);
        let harness = Harness::new(
            RollbackCommitObservation::Committed(receipt(owner.fence.generation)),
            indeterminate_marker(),
        );

        assert_eq!(
            harness
                .effect
                .prepare(&executing, owner, context(owner))
                .await
                .expect("governed rollback resolution succeeds"),
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
                .is_empty(),
            "prepare must not cross the durable rollback-event port"
        );
        assert!(
            harness
                .markers
                .requests
                .lock()
                .expect("marker requests lock")
                .is_empty(),
            "prepare must not query recovery state"
        );

        let prepared = prepare_record(executing, owner, transaction());
        assert_eq!(
            harness
                .effect
                .commit(&prepared, owner, transaction(), context(owner))
                .await
                .expect("exact rollback event readback succeeds"),
            CommitEffectOutcome::Committed {
                result: expected_result()
            }
        );
        let resolved = harness
            .resolver
            .requests
            .lock()
            .expect("resolver requests lock")[0];
        assert_eq!(resolved.target_epoch(), target_epoch());
        assert_eq!(resolved.rollback_authorization(), rollback_authorization());
        assert_eq!(resolved.execution_owner(), owner);
        assert_eq!(
            harness
                .commits
                .requests
                .lock()
                .expect("commit requests lock")
                .len(),
            1,
            "the effect performs exactly one durable rollback attempt"
        );
    }

    #[tokio::test]
    async fn prepare_rejects_resolver_target_or_authorization_substitution() {
        let owner = owner(1, 1, 1);
        let substitutions = [
            FabricCommandPayload::RollbackEpoch {
                target_epoch: EpochId::from_bytes([0x41; 16]),
                authorization: rollback_authorization(),
            },
            FabricCommandPayload::RollbackEpoch {
                target_epoch: target_epoch(),
                authorization: RollbackAuthorizationRef::from_bytes([0x42; 32]),
            },
        ];

        for payload in substitutions {
            let harness = Harness::with_resolver_payload(
                RollbackCommitObservation::Committed(receipt(owner.fence.generation)),
                indeterminate_marker(),
                Some(payload),
            );
            let executing = executing_record(command(owner.fence), owner);
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
    }

    #[tokio::test]
    async fn commit_rejects_target_authorization_generation_or_selected_epoch_drift() {
        let owner = owner(1, 1, 1);
        let mut wrong_target = receipt(generation(1));
        wrong_target.target_epoch = EpochId::from_bytes([0x51; 16]);
        let mut wrong_authorization = receipt(generation(1));
        wrong_authorization.rollback_authorization =
            RollbackAuthorizationRef::from_bytes([0x52; 32]);
        let wrong_generation = receipt(generation(2));
        let mut wrong_selected = receipt(generation(1));
        wrong_selected.selected_epoch = EpochId::from_bytes([0x53; 16]);

        for contradictory in [
            wrong_target,
            wrong_authorization,
            wrong_generation,
            wrong_selected,
        ] {
            let harness = Harness::new(
                RollbackCommitObservation::Committed(contradictory),
                indeterminate_marker(),
            );
            let prepared = prepare_record(
                executing_record(command(owner.fence), owner),
                owner,
                transaction(),
            );
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
    async fn marker_visibility_is_unknown_and_marker_generation_is_exact() {
        let owner = owner(1, 1, 1);
        let diagnostic = diagnostic(61);
        let prepared = prepare_record(
            executing_record(command(owner.fence), owner),
            owner,
            transaction(),
        );
        let exact = Harness::new(
            RollbackCommitObservation::MarkerAlreadyCommitted {
                marker: marker_receipt(generation(1)),
                diagnostic,
            },
            indeterminate_marker(),
        );

        assert_eq!(
            exact
                .effect
                .commit(&prepared, owner, transaction(), context(owner))
                .await
                .expect("exact marker identity is valid but incomplete"),
            CommitEffectOutcome::Unknown {
                unknown: UnknownCommit {
                    reason: UnknownCommitReason::ReadbackUnavailable,
                    diagnostic,
                }
            }
        );
        assert!(
            exact
                .markers
                .requests
                .lock()
                .expect("marker requests lock")
                .is_empty(),
            "direct marker visibility must not trigger an implicit recovery query"
        );

        let stale = Harness::new(
            RollbackCommitObservation::MarkerAlreadyCommitted {
                marker: marker_receipt(generation(2)),
                diagnostic,
            },
            indeterminate_marker(),
        );
        assert_eq!(
            stale
                .effect
                .commit(&prepared, owner, transaction(), context(owner))
                .await,
            Err(CommandPortError::CorruptRecord)
        );
    }

    #[tokio::test]
    async fn reconciliation_preserves_execution_owner_and_uses_newer_recovery_owner() {
        let execution_owner = owner(1, 1, 1);
        let recovery_owner = owner(2, 2, 2);
        let prepared = prepare_record(
            executing_record(command(execution_owner.fence), execution_owner),
            execution_owner,
            transaction(),
        );
        let awaiting = awaiting_record(prepared, execution_owner, transaction());
        let evidence = ReconciliationEvidenceRef::from_bytes([0x71; 32]);
        let harness = Harness::new(
            RollbackCommitObservation::Unknown {
                reason: UnknownCommitReason::ProcessInterrupted,
                diagnostic: diagnostic(70),
            },
            RollbackOperationMarkerObservation::Committed {
                receipt: receipt(execution_owner.fence.generation),
                evidence,
            },
        );

        assert_eq!(
            harness
                .effect
                .reconcile(
                    &awaiting,
                    recovery_owner,
                    transaction(),
                    ReductionContext {
                        current_head: ExpectedHead::Epoch(target_epoch()),
                        active_fence: recovery_owner.fence,
                    },
                )
                .await
                .expect("complete history proves the prior-generation rollback"),
            ReconciliationObservation::Committed {
                evidence,
                result: expected_result(),
            }
        );
        assert!(
            harness
                .commits
                .requests
                .lock()
                .expect("commit requests lock")
                .is_empty(),
            "reconciliation has no rollback append path"
        );
        let request = harness
            .markers
            .requests
            .lock()
            .expect("marker requests lock")[0];
        assert_eq!(request.attempt().execution_owner(), execution_owner);
        assert_eq!(request.attempt().target_epoch(), target_epoch());
        assert_eq!(
            request.attempt().rollback_authorization(),
            rollback_authorization()
        );
        assert_eq!(request.transaction(), transaction());
        assert_eq!(request.active_recovery_owner(), recovery_owner);
    }

    #[tokio::test]
    async fn only_complete_history_or_explicit_noncommit_can_finish_reconciliation() {
        let execution_owner = owner(1, 1, 1);
        let recovery_owner = owner(2, 2, 2);
        let prepared = prepare_record(
            executing_record(command(execution_owner.fence), execution_owner),
            execution_owner,
            transaction(),
        );
        let awaiting = awaiting_record(prepared, execution_owner, transaction());
        let evidence = ReconciliationEvidenceRef::from_bytes([0x72; 32]);

        for (observation, expected) in [
            (
                RollbackOperationMarkerObservation::ProvedNotCommitted { evidence },
                ReconciliationObservation::NotCommitted { evidence },
            ),
            (
                RollbackOperationMarkerObservation::Indeterminate { evidence },
                ReconciliationObservation::Indeterminate { evidence },
            ),
        ] {
            let harness = Harness::new(
                RollbackCommitObservation::Unknown {
                    reason: UnknownCommitReason::ProcessInterrupted,
                    diagnostic: diagnostic(71),
                },
                observation,
            );
            assert_eq!(
                harness
                    .effect
                    .reconcile(
                        &awaiting,
                        recovery_owner,
                        transaction(),
                        ReductionContext {
                            current_head: ExpectedHead::Epoch(target_epoch()),
                            active_fence: recovery_owner.fence,
                        },
                    )
                    .await
                    .expect("typed marker/control-history observation"),
                expected
            );
        }
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
            expected_head: ExpectedHead::Epoch(EpochId::from_bytes([0x11; 16])),
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
            payload: FabricCommandPayload::RollbackEpoch {
                target_epoch: target_epoch(),
                authorization: rollback_authorization(),
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
        .expect("admit rollback command");
        let AdmissionOutcome::New(admitted) = admitted else {
            panic!("empty admission creates a record")
        };
        CommandReducer::reduce(&admitted, CommandEvent::Start { owner }, context(owner))
            .expect("start rollback command")
            .record
    }

    fn prepare_record(
        executing: CommandRecord,
        owner: ExecutionOwner,
        transaction: TransactionRef,
    ) -> CommandRecord {
        CommandReducer::reduce(
            &executing,
            CommandEvent::PrepareCommit { owner, transaction },
            context(owner),
        )
        .expect("prepare rollback transaction")
        .record
    }

    fn awaiting_record(
        prepared: CommandRecord,
        owner: ExecutionOwner,
        transaction: TransactionRef,
    ) -> CommandRecord {
        CommandReducer::reduce(
            &prepared,
            CommandEvent::ReportUnknownCommit {
                owner,
                transaction,
                unknown: UnknownCommit {
                    reason: UnknownCommitReason::ProcessInterrupted,
                    diagnostic: diagnostic(80),
                },
            },
            context(owner),
        )
        .expect("record unknown rollback commit")
        .record
    }

    fn receipt(writer_generation: WriterGeneration) -> RollbackCommitReceipt {
        RollbackCommitReceipt {
            workspace_id: workspace_id(),
            operation_id: operation_id(),
            transaction: transaction(),
            writer_generation,
            target_epoch: target_epoch(),
            rollback_authorization: rollback_authorization(),
            operation_selection: selection(),
            selected_epoch: target_epoch(),
        }
    }

    fn marker_receipt(writer_generation: WriterGeneration) -> RollbackMarkerReceipt {
        RollbackMarkerReceipt {
            workspace_id: workspace_id(),
            operation_id: operation_id(),
            transaction: transaction(),
            writer_generation,
        }
    }

    fn expected_result() -> CommandResult {
        CommandResult::EpochRolledBack {
            epoch: target_epoch(),
            selection: selection(),
        }
    }

    fn indeterminate_marker() -> RollbackOperationMarkerObservation {
        RollbackOperationMarkerObservation::Indeterminate {
            evidence: ReconciliationEvidenceRef::from_bytes([0x73; 32]),
        }
    }

    fn context(owner: ExecutionOwner) -> ReductionContext {
        ReductionContext {
            current_head: ExpectedHead::Epoch(EpochId::from_bytes([0x11; 16])),
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
        WriterGeneration::new(value).expect("nonzero test writer generation")
    }

    fn operation_id() -> OperationId {
        OperationId::from_bytes([0x01; 16])
    }

    fn workspace_id() -> WorkspaceId {
        WorkspaceId::from_bytes([0x10; 16])
    }

    fn target_epoch() -> EpochId {
        EpochId::from_bytes([0x12; 16])
    }

    fn rollback_authorization() -> RollbackAuthorizationRef {
        RollbackAuthorizationRef::from_bytes([0x13; 32])
    }

    fn transaction() -> TransactionRef {
        TransactionRef::from_bytes([0x20; 32])
    }

    fn selection() -> OperationSelectionRef {
        OperationSelectionRef::from_bytes([0x30; 32])
    }

    fn diagnostic(seed: u8) -> DiagnosticRef {
        DiagnosticRef::from_bytes([seed; 32])
    }
}
