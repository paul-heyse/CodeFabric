//! Typed `PublishRelations` command effect and exact Delta reconciliation seam.
//!
//! The effect deliberately separates three authorities:
//!
//! - immutable request/catalog resolution derives one deterministic transaction before any
//!   durable target write;
//! - the commit port is the sole durable Delta boundary and owns the exact zero-retry controlled
//!   plan write(s);
//! - the operation-marker port performs a read-only, exact transaction/control-history query.
//!
//! A relation set can span more than one Delta table, while [`super::delta_write`] controls one
//! exact table write at a time. The application-owned commit port therefore owns lossless plan
//! resolution and any component-write sequencing. Its contract requires
//! `ControlledDeltaWriteSpec`, `write_exact_delta_plan`, and `max_retries(0)` semantics for every
//! component. This effect never discovers latest state, retries a conflict, or reduces command
//! state itself.

use std::sync::Arc;

use async_trait::async_trait;

use super::command::{
    CommandCancellation, CommandFailure, CommandKind, CommandRecord, CommandResult, DiagnosticRef,
    ExecutionOwner, ExpectedHead, FabricCommand, FabricCommandPayload, OperationId,
    OperationSelectionRef, ReconciliationEvidenceRef, ReconciliationObservation, ReductionContext,
    RelationPublication, TransactionRef, UnknownCommit, UnknownCommitReason, WorkspaceId,
    WriterGeneration,
};
use super::command_actor::{CommandPortError, CommitEffectOutcome, PrepareEffectOutcome};
use super::command_effect_contract::{
    ValidatedCommandAttempt, executing_attempt, prepared_attempt, reconciliation_attempt,
};
use super::command_effect_router::RelationPublicationCommandEffectPort;

/// Exact immutable attempt contract supplied to request resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationPublicationAttempt {
    validated: ValidatedCommandAttempt,
}

impl RelationPublicationAttempt {
    fn from_validated(validated: ValidatedCommandAttempt) -> Result<Self, CommandPortError> {
        if matches!(
            validated.command().payload,
            FabricCommandPayload::PublishRelations { .. }
        ) {
            Ok(Self { validated })
        } else {
            Err(CommandPortError::CorruptRecord)
        }
    }

    /// Immutable admitted command whose payload is `PublishRelations`.
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

    /// Typed publication fixed by the command payload.
    #[must_use]
    pub fn publication(self) -> RelationPublication {
        let FabricCommandPayload::PublishRelations { publication } = self.command().payload else {
            unreachable!("relation-publication attempt is constructed only after payload checks")
        };
        publication
    }
}

/// Fully bound result of immutable catalog/request resolution.
///
/// All mirrored fields are checked by the effect. A resolver cannot substitute another
/// operation, attempt, publication, or writer generation while returning a plausible transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedRelationPublication {
    attempt: RelationPublicationAttempt,
    transaction: TransactionRef,
}

impl ResolvedRelationPublication {
    /// Bind one deterministic transaction to the exact immutable attempt.
    #[must_use]
    pub const fn new(attempt: RelationPublicationAttempt, transaction: TransactionRef) -> Self {
        Self {
            attempt,
            transaction,
        }
    }

    /// Exact attempt resolved by the catalog.
    #[must_use]
    pub const fn attempt(self) -> RelationPublicationAttempt {
        self.attempt
    }

    /// Deterministic application transaction identity persisted by the command actor.
    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.transaction
    }
}

/// Immutable resolution result. Only `Resolved` can advance to durable preparation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationPublicationResolution {
    Resolved(ResolvedRelationPublication),
    KnownFailure(CommandFailure),
    Cancelled(CommandCancellation),
}

/// Read-only catalog/request resolver.
///
/// Implementations may validate schemas, resolve model-selected relations and plans, and derive
/// an application transaction. They must not write Delta, operation markers, control history, or
/// any other durable target.
#[async_trait]
pub trait RelationPublicationResolverPort: Send + Sync {
    async fn resolve(
        &self,
        attempt: RelationPublicationAttempt,
    ) -> Result<RelationPublicationResolution, CommandPortError>;
}

/// Exact contract presented to the sole durable relation-publication boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationPublicationCommitRequest {
    attempt: RelationPublicationAttempt,
    transaction: TransactionRef,
}

impl RelationPublicationCommitRequest {
    /// Exact command attempt authorized for this commit.
    #[must_use]
    pub const fn attempt(self) -> RelationPublicationAttempt {
        self.attempt
    }

    /// Actor-persisted application transaction identity.
    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.transaction
    }
}

/// Complete readback receipt needed to construct a `RelationsPublished` result.
///
/// Direct commit requires the exact active writer generation. Reconciliation may observe a
/// commit made by an older, still-valid generation; operation, transaction, workspace,
/// publication, and head bindings remain exact in both cases.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationPublicationCommitReceipt {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub transaction: TransactionRef,
    pub writer_generation: WriterGeneration,
    pub publication: RelationPublication,
    pub resulting_head: ExpectedHead,
    pub operation_selection: OperationSelectionRef,
}

/// Exact marker visibility read after an attempted direct commit.
///
/// A marker alone is not a complete command result. Even a perfectly bound value therefore maps
/// to unknown and requires the complete reconciliation query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationPublicationMarkerReceipt {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub transaction: TransactionRef,
    pub writer_generation: WriterGeneration,
}

/// Exhaustive observation from one zero-retry relation-publication attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationPublicationCommitObservation {
    /// Every component and control record committed and read back exactly.
    Committed(RelationPublicationCommitReceipt),
    /// The application marker is visible, but complete result/control history was not proved.
    MarkerAlreadyCommitted {
        marker: RelationPublicationMarkerReceipt,
        diagnostic: DiagnosticRef,
    },
    /// Predecessor/application-transaction/commit collision. It is never retried here.
    Conflict { diagnostic: DiagnosticRef },
    /// Commit outcome or readback is not provable.
    Unknown {
        reason: UnknownCommitReason,
        diagnostic: DiagnosticRef,
    },
}

/// Sole durable relation-publication authority.
///
/// A production implementation resolves the model-selected component plans from the exact
/// request, binds every `ControlledDeltaWriteSpec` to `operation_id` and
/// `attempt.owner().fence.generation`, and calls `write_exact_delta_plan` once per selected
/// component. It must not internally retry, rebase, discover latest, or reduce command state.
#[async_trait]
pub trait RelationPublicationDeltaCommitPort: Send + Sync {
    async fn commit(
        &self,
        request: RelationPublicationCommitRequest,
    ) -> Result<RelationPublicationCommitObservation, CommandPortError>;
}

/// Read-only exact marker/control-history lookup key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationPublicationReconciliationRequest {
    attempt: RelationPublicationAttempt,
    active_recovery_owner: ExecutionOwner,
    transaction: TransactionRef,
}

impl RelationPublicationReconciliationRequest {
    /// Original command attempt, including its immutable transaction executor.
    #[must_use]
    pub const fn attempt(self) -> RelationPublicationAttempt {
        self.attempt
    }

    /// Current actor/fence authorized to perform the read-only recovery query.
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

/// Exact result of reading both the application marker and complete control history.
///
/// There is no bare absence. Non-commit and indeterminate outcomes both carry a durable proof
/// reference supplied by the read authority; the effect never manufactures evidence identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationPublicationOperationMarkerObservation {
    Committed {
        receipt: RelationPublicationCommitReceipt,
        evidence: ReconciliationEvidenceRef,
    },
    ProvedNotCommitted {
        evidence: ReconciliationEvidenceRef,
    },
    Indeterminate {
        evidence: ReconciliationEvidenceRef,
    },
}

/// Read-only application-marker and control-history authority.
///
/// Implementations query the exact transaction and [`RelationPublicationAttempt::owner`]
/// execution generation while fencing the read with
/// [`RelationPublicationReconciliationRequest::active_recovery_owner`].
#[async_trait]
pub trait RelationPublicationOperationMarkerPort: Send + Sync {
    async fn read_exact(
        &self,
        request: RelationPublicationReconciliationRequest,
    ) -> Result<RelationPublicationOperationMarkerObservation, CommandPortError>;
}

/// Concrete typed effect for `FabricCommandPayload::PublishRelations`.
pub struct RelationPublicationCommandEffect {
    resolver: Arc<dyn RelationPublicationResolverPort>,
    commits: Arc<dyn RelationPublicationDeltaCommitPort>,
    markers: Arc<dyn RelationPublicationOperationMarkerPort>,
}

impl std::fmt::Debug for RelationPublicationCommandEffect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RelationPublicationCommandEffect")
            .field("resolver", &"installed")
            .field("commits", &"installed")
            .field("markers", &"installed")
            .finish()
    }
}

impl RelationPublicationCommandEffect {
    /// Install the three non-overlapping authorities required by the effect.
    #[must_use]
    pub const fn new(
        resolver: Arc<dyn RelationPublicationResolverPort>,
        commits: Arc<dyn RelationPublicationDeltaCommitPort>,
        markers: Arc<dyn RelationPublicationOperationMarkerPort>,
    ) -> Self {
        Self {
            resolver,
            commits,
            markers,
        }
    }
}

#[async_trait]
impl RelationPublicationCommandEffectPort for RelationPublicationCommandEffect {
    async fn prepare(
        &self,
        executing: &CommandRecord,
        owner: ExecutionOwner,
        context: ReductionContext,
    ) -> Result<PrepareEffectOutcome, CommandPortError> {
        let validated =
            executing_attempt(executing, owner, context, CommandKind::PublishRelations)?;
        let attempt = RelationPublicationAttempt::from_validated(validated)?;
        match self.resolver.resolve(attempt).await? {
            RelationPublicationResolution::Resolved(resolved) => {
                if resolved.attempt() != attempt {
                    return Err(CommandPortError::CorruptRecord);
                }
                Ok(PrepareEffectOutcome::Prepared {
                    transaction: resolved.transaction(),
                })
            }
            RelationPublicationResolution::KnownFailure(failure) => {
                Ok(PrepareEffectOutcome::KnownFailure { failure })
            }
            RelationPublicationResolution::Cancelled(cancellation) => {
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
            CommandKind::PublishRelations,
        )?;
        let attempt = RelationPublicationAttempt::from_validated(validated)?;
        let request = RelationPublicationCommitRequest {
            attempt,
            transaction,
        };
        match self.commits.commit(request).await? {
            RelationPublicationCommitObservation::Committed(receipt) => {
                validate_direct_receipt(attempt, transaction, receipt)?;
                Ok(CommitEffectOutcome::Committed {
                    result: result_from_receipt(receipt),
                })
            }
            RelationPublicationCommitObservation::MarkerAlreadyCommitted { marker, diagnostic } => {
                validate_direct_marker(attempt, transaction, marker)?;
                Ok(unknown(
                    UnknownCommitReason::ReadbackUnavailable,
                    diagnostic,
                ))
            }
            RelationPublicationCommitObservation::Conflict { diagnostic } => Ok(unknown(
                UnknownCommitReason::ReadbackUnavailable,
                diagnostic,
            )),
            RelationPublicationCommitObservation::Unknown { reason, diagnostic } => {
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
            CommandKind::PublishRelations,
        )?;
        let attempt = RelationPublicationAttempt::from_validated(recovery.attempt())?;
        let request = RelationPublicationReconciliationRequest {
            attempt,
            active_recovery_owner: recovery.active_recovery_owner(),
            transaction,
        };
        match self.markers.read_exact(request).await? {
            RelationPublicationOperationMarkerObservation::Committed { receipt, evidence } => {
                validate_reconciled_receipt(attempt, transaction, receipt)?;
                Ok(ReconciliationObservation::Committed {
                    evidence,
                    result: result_from_receipt(receipt),
                })
            }
            RelationPublicationOperationMarkerObservation::ProvedNotCommitted { evidence } => {
                Ok(ReconciliationObservation::NotCommitted { evidence })
            }
            RelationPublicationOperationMarkerObservation::Indeterminate { evidence } => {
                Ok(ReconciliationObservation::Indeterminate { evidence })
            }
        }
    }
}

fn validate_direct_receipt(
    attempt: RelationPublicationAttempt,
    transaction: TransactionRef,
    receipt: RelationPublicationCommitReceipt,
) -> Result<(), CommandPortError> {
    validate_receipt_identity(attempt, transaction, receipt)?;
    attempt
        .validated
        .validate_receipt_generation(receipt.writer_generation)
}

fn validate_reconciled_receipt(
    attempt: RelationPublicationAttempt,
    transaction: TransactionRef,
    receipt: RelationPublicationCommitReceipt,
) -> Result<(), CommandPortError> {
    validate_receipt_identity(attempt, transaction, receipt)?;
    attempt
        .validated
        .validate_receipt_generation(receipt.writer_generation)
}

fn validate_receipt_identity(
    attempt: RelationPublicationAttempt,
    transaction: TransactionRef,
    receipt: RelationPublicationCommitReceipt,
) -> Result<(), CommandPortError> {
    let command = attempt.command();
    if receipt.workspace_id != command.ownership.workspace_id
        || receipt.operation_id != command.identity.operation_id
        || receipt.transaction != transaction
        || receipt.publication != attempt.publication()
        || receipt.resulting_head != command.expected_head
    {
        return Err(CommandPortError::CorruptRecord);
    }
    Ok(())
}

fn validate_direct_marker(
    attempt: RelationPublicationAttempt,
    transaction: TransactionRef,
    marker: RelationPublicationMarkerReceipt,
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

fn result_from_receipt(receipt: RelationPublicationCommitReceipt) -> CommandResult {
    CommandResult::RelationsPublished {
        class: receipt.publication.class(),
        relations: match receipt.publication {
            RelationPublication::ProviderNative { relations, .. }
            | RelationPublication::Canonical { relations, .. }
            | RelationPublication::Derived { relations, .. }
            | RelationPublication::OperationalFact { relations, .. } => relations,
        },
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
        ActorId, AdmissionContext, AdmissionOutcome, AnalysisRunRef, AuthorizationDecision,
        AuthorizationRef, CommandEvent, CommandIdentity, CommandOwnership, CommandPins,
        CommandReducer, CompilerReleaseRef, IdempotencyKey, LeaseId, ModelHeadRef, OwnerSetRef,
        PrincipalId, ProviderSetRef, RelationSetRef, ResourceEnvelopeRef, SourceGeneration,
        WorkspaceId, WriterFence,
    };

    struct ResolverProbe {
        transaction: TransactionRef,
        requests: Mutex<Vec<RelationPublicationAttempt>>,
    }

    #[async_trait]
    impl RelationPublicationResolverPort for ResolverProbe {
        async fn resolve(
            &self,
            attempt: RelationPublicationAttempt,
        ) -> Result<RelationPublicationResolution, CommandPortError> {
            self.requests
                .lock()
                .expect("resolver requests lock")
                .push(attempt);
            Ok(RelationPublicationResolution::Resolved(
                ResolvedRelationPublication::new(attempt, self.transaction),
            ))
        }
    }

    struct CommitProbe {
        observation: RelationPublicationCommitObservation,
        requests: Mutex<Vec<RelationPublicationCommitRequest>>,
    }

    #[async_trait]
    impl RelationPublicationDeltaCommitPort for CommitProbe {
        async fn commit(
            &self,
            request: RelationPublicationCommitRequest,
        ) -> Result<RelationPublicationCommitObservation, CommandPortError> {
            self.requests
                .lock()
                .expect("commit requests lock")
                .push(request);
            Ok(self.observation)
        }
    }

    struct MarkerProbe {
        observation: RelationPublicationOperationMarkerObservation,
        requests: Mutex<Vec<RelationPublicationReconciliationRequest>>,
    }

    #[async_trait]
    impl RelationPublicationOperationMarkerPort for MarkerProbe {
        async fn read_exact(
            &self,
            request: RelationPublicationReconciliationRequest,
        ) -> Result<RelationPublicationOperationMarkerObservation, CommandPortError> {
            self.requests
                .lock()
                .expect("marker requests lock")
                .push(request);
            Ok(self.observation)
        }
    }

    struct Harness {
        effect: RelationPublicationCommandEffect,
        resolver: Arc<ResolverProbe>,
        commits: Arc<CommitProbe>,
        markers: Arc<MarkerProbe>,
    }

    impl Harness {
        fn new(
            commit: RelationPublicationCommitObservation,
            marker: RelationPublicationOperationMarkerObservation,
        ) -> Self {
            let resolver = Arc::new(ResolverProbe {
                transaction: transaction(),
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
            let effect = RelationPublicationCommandEffect::new(
                resolver.clone(),
                commits.clone(),
                markers.clone(),
            );
            Self {
                effect,
                resolver,
                commits,
                markers,
            }
        }
    }

    #[tokio::test]
    async fn prepare_is_read_only_and_commit_returns_only_exact_readback() {
        let owner = owner(1, 1, 1);
        let executing = executing_record(command(owner.fence), owner);
        let receipt = receipt(owner.fence.generation, transaction());
        let harness = Harness::new(
            RelationPublicationCommitObservation::Committed(receipt),
            indeterminate_marker(),
        );

        let prepared = harness
            .effect
            .prepare(&executing, owner, context(owner))
            .await
            .expect("immutable resolution succeeds");
        assert_eq!(
            prepared,
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
            "prepare must not cross the durable commit port"
        );

        let prepared_record = prepare_record(executing, owner, transaction());
        let committed = harness
            .effect
            .commit(&prepared_record, owner, transaction(), context(owner))
            .await
            .expect("exact commit readback succeeds");
        assert_eq!(
            committed,
            CommitEffectOutcome::Committed {
                result: expected_result()
            }
        );
        let resolution_request = harness
            .resolver
            .requests
            .lock()
            .expect("resolver requests lock")[0];
        assert_eq!(resolution_request.owner(), owner);
        let commit_request = harness
            .commits
            .requests
            .lock()
            .expect("commit requests lock")[0];
        assert_eq!(commit_request.attempt().owner(), owner);
        assert_eq!(commit_request.transaction(), transaction());
    }

    #[tokio::test]
    async fn conflict_and_unknown_both_require_reconciliation_without_retry() {
        let owner = owner(1, 1, 1);
        let observations = [
            RelationPublicationCommitObservation::Conflict {
                diagnostic: diagnostic(41),
            },
            RelationPublicationCommitObservation::Unknown {
                reason: UnknownCommitReason::ConnectionLost,
                diagnostic: diagnostic(42),
            },
        ];
        let expected = [
            UnknownCommit {
                reason: UnknownCommitReason::ReadbackUnavailable,
                diagnostic: diagnostic(41),
            },
            UnknownCommit {
                reason: UnknownCommitReason::ConnectionLost,
                diagnostic: diagnostic(42),
            },
        ];

        for (observation, expected) in observations.into_iter().zip(expected) {
            let harness = Harness::new(observation, indeterminate_marker());
            let prepared = prepare_record(
                executing_record(command(owner.fence), owner),
                owner,
                transaction(),
            );
            assert_eq!(
                harness
                    .effect
                    .commit(&prepared, owner, transaction(), context(owner))
                    .await
                    .expect("typed ambiguous outcome"),
                CommitEffectOutcome::Unknown { unknown: expected }
            );
            assert_eq!(
                harness
                    .commits
                    .requests
                    .lock()
                    .expect("commit requests lock")
                    .len(),
                1,
                "effect performs exactly one attempt and never retries"
            );
        }
    }

    #[tokio::test]
    async fn marker_visibility_without_complete_history_is_not_commit_confirmation() {
        let owner = owner(1, 1, 1);
        let diagnostic = diagnostic(51);
        let harness = Harness::new(
            RelationPublicationCommitObservation::MarkerAlreadyCommitted {
                marker: marker_receipt(owner.fence.generation),
                diagnostic,
            },
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
                .await
                .expect("exact marker binding is valid"),
            CommitEffectOutcome::Unknown {
                unknown: UnknownCommit {
                    reason: UnknownCommitReason::ReadbackUnavailable,
                    diagnostic,
                }
            }
        );
    }

    #[tokio::test]
    async fn direct_commit_rejects_a_receipt_from_another_writer_generation() {
        let owner = owner(1, 1, 1);
        let harness = Harness::new(
            RelationPublicationCommitObservation::Committed(receipt(generation(2), transaction())),
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

    #[tokio::test]
    async fn reconciliation_reads_exact_original_transaction_under_active_recovery_generation() {
        let original = owner(1, 1, 1);
        let recovery = owner(2, 2, 2);
        let prepared = prepare_record(
            executing_record(command(original.fence), original),
            original,
            transaction(),
        );
        let awaiting = awaiting_record(prepared, original, transaction());
        let evidence = ReconciliationEvidenceRef::from_bytes([0x61; 32]);
        let harness = Harness::new(
            RelationPublicationCommitObservation::Unknown {
                reason: UnknownCommitReason::ProcessInterrupted,
                diagnostic: diagnostic(60),
            },
            RelationPublicationOperationMarkerObservation::Committed {
                receipt: receipt(original.fence.generation, transaction()),
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
                .expect("exact marker and history prove the prior-generation commit"),
            ReconciliationObservation::Committed {
                evidence,
                result: expected_result(),
            }
        );
        let request = harness
            .markers
            .requests
            .lock()
            .expect("marker requests lock")[0];
        assert_eq!(request.transaction(), transaction());
        assert_eq!(
            request.attempt().command().identity.operation_id,
            operation_id()
        );
        assert_eq!(request.attempt().owner(), original);
        assert_eq!(
            request.attempt().owner().fence.generation,
            generation(1),
            "committed readback remains bound to the immutable transaction executor"
        );
        assert_eq!(request.active_recovery_owner(), recovery);
        assert_eq!(
            request.active_recovery_owner().fence.generation,
            generation(2),
            "marker read is fenced by the active recovery generation"
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
                compiler_release: CompilerReleaseRef::from_bytes([0x05; 32]),
                model_head: ModelHeadRef::from_bytes([0x06; 32]),
                source_generation: SourceGeneration::new(7),
                provider_set: ProviderSetRef::from_bytes([0x08; 32]),
            },
            resources: ResourceEnvelopeRef::from_bytes([0x09; 32]),
            payload: FabricCommandPayload::PublishRelations {
                publication: publication(),
            },
        }
    }

    fn publication() -> RelationPublication {
        RelationPublication::Derived {
            analysis_run: AnalysisRunRef::from_bytes([0x0a; 32]),
            owners: OwnerSetRef::from_bytes([0x0b; 32]),
            relations: RelationSetRef::from_bytes([0x0c; 32]),
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
        .expect("admit publication command");
        let AdmissionOutcome::New(admitted) = admitted else {
            panic!("empty admission creates a record")
        };
        CommandReducer::reduce(&admitted, CommandEvent::Start { owner }, context(owner))
            .expect("start publication command")
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
        .expect("prepare publication transaction")
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
                    diagnostic: diagnostic(70),
                },
            },
            context(owner),
        )
        .expect("record unknown publication commit")
        .record
    }

    fn receipt(
        writer_generation: WriterGeneration,
        transaction: TransactionRef,
    ) -> RelationPublicationCommitReceipt {
        RelationPublicationCommitReceipt {
            workspace_id: workspace_id(),
            operation_id: operation_id(),
            transaction,
            writer_generation,
            publication: publication(),
            resulting_head: ExpectedHead::Empty,
            operation_selection: selection(),
        }
    }

    fn marker_receipt(writer_generation: WriterGeneration) -> RelationPublicationMarkerReceipt {
        RelationPublicationMarkerReceipt {
            workspace_id: workspace_id(),
            operation_id: operation_id(),
            transaction: transaction(),
            writer_generation,
        }
    }

    fn expected_result() -> CommandResult {
        CommandResult::RelationsPublished {
            class: publication().class(),
            relations: RelationSetRef::from_bytes([0x0c; 32]),
            resulting_head: ExpectedHead::Empty,
            selection: selection(),
        }
    }

    fn indeterminate_marker() -> RelationPublicationOperationMarkerObservation {
        RelationPublicationOperationMarkerObservation::Indeterminate {
            evidence: ReconciliationEvidenceRef::from_bytes([0x71; 32]),
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
        WriterGeneration::new(value).expect("nonzero test writer generation")
    }

    fn operation_id() -> OperationId {
        OperationId::from_bytes([0x01; 16])
    }

    fn workspace_id() -> WorkspaceId {
        WorkspaceId::from_bytes([0x10; 16])
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
