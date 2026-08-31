//! Typed `ApplyModelMigration` command effect and exact model-history reconciliation seam.
//!
//! Model compilation and replay resolve one immutable migration into one target model head before
//! preparation. Resolution is read-only. The commit port is the sole durable model-migration
//! boundary and performs one fenced application transaction without retry, rebase, or latest-state
//! discovery. Recovery requires the exact operation marker plus complete model control history;
//! marker visibility alone never confirms success.

use std::sync::Arc;

use async_trait::async_trait;

use super::command::{
    CommandCancellation, CommandFailure, CommandKind, CommandRecord, CommandResult, DiagnosticRef,
    ExecutionOwner, ExpectedHead, FabricCommand, FabricCommandPayload, ModelHeadRef,
    ModelMigrationRef, OperationId, OperationSelectionRef, ReconciliationEvidenceRef,
    ReconciliationObservation, ReductionContext, TransactionRef, UnknownCommit,
    UnknownCommitReason, WorkspaceId, WriterGeneration,
};
use super::command_actor::{CommandPortError, CommitEffectOutcome, PrepareEffectOutcome};
use super::command_effect_contract::{
    ValidatedCommandAttempt, executing_attempt, prepared_attempt, reconciliation_attempt,
};
use super::command_effect_router::ModelMigrationCommandEffectPort;

/// Exact immutable model-migration attempt supplied to replay/compiler resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelMigrationAttempt {
    validated: ValidatedCommandAttempt,
}

impl ModelMigrationAttempt {
    fn from_validated(validated: ValidatedCommandAttempt) -> Result<Self, CommandPortError> {
        if matches!(
            validated.command().payload,
            FabricCommandPayload::ApplyModelMigration { .. }
        ) {
            Ok(Self { validated })
        } else {
            Err(CommandPortError::CorruptRecord)
        }
    }

    /// Immutable admitted `ApplyModelMigration` command.
    #[must_use]
    pub const fn command(self) -> FabricCommand {
        self.validated.command()
    }

    /// Reducer-owned attempt number.
    #[must_use]
    pub const fn attempt(self) -> u32 {
        self.validated.attempt()
    }

    /// Immutable actor/fence that executes this transaction attempt.
    #[must_use]
    pub const fn execution_owner(self) -> ExecutionOwner {
        self.validated.execution_owner()
    }

    /// Exact immutable migration selected by the admitted command.
    #[must_use]
    pub fn migration(self) -> ModelMigrationRef {
        let FabricCommandPayload::ApplyModelMigration { migration, .. } = self.command().payload
        else {
            unreachable!("model-migration attempt is constructed only after payload checks")
        };
        migration
    }

    /// Exact replayed model head required after applying the migration.
    #[must_use]
    pub fn target_model_head(self) -> ModelHeadRef {
        let FabricCommandPayload::ApplyModelMigration {
            target_model_head, ..
        } = self.command().payload
        else {
            unreachable!("model-migration attempt is constructed only after payload checks")
        };
        target_model_head
    }
}

/// Fully bound result of read-only replay/model-compiler resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedModelMigration {
    attempt: ModelMigrationAttempt,
    transaction: TransactionRef,
}

impl ResolvedModelMigration {
    /// Bind one deterministic transaction to the exact immutable migration attempt.
    #[must_use]
    pub const fn new(attempt: ModelMigrationAttempt, transaction: TransactionRef) -> Self {
        Self {
            attempt,
            transaction,
        }
    }

    /// Exact attempt resolved by replay and the model compiler.
    #[must_use]
    pub const fn attempt(self) -> ModelMigrationAttempt {
        self.attempt
    }

    /// Deterministic application transaction persisted before commit.
    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.transaction
    }
}

/// Read-only replay/compiler resolution result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelMigrationResolution {
    Resolved(ResolvedModelMigration),
    KnownFailure(CommandFailure),
    Cancelled(CommandCancellation),
}

/// Read-only model replay and compiler authority.
///
/// Implementations resolve the exact migration against the admitted predecessor and prove that
/// replay produces the requested target model head. They may derive a deterministic transaction,
/// but must not write model relations, markers, control history, or another durable target.
#[async_trait]
pub trait ModelMigrationResolverPort: Send + Sync {
    async fn resolve(
        &self,
        attempt: ModelMigrationAttempt,
    ) -> Result<ModelMigrationResolution, CommandPortError>;
}

/// Exact contract presented to the sole durable model-migration boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelMigrationCommitRequest {
    attempt: ModelMigrationAttempt,
    transaction: TransactionRef,
}

impl ModelMigrationCommitRequest {
    /// Exact command attempt authorized for this commit.
    #[must_use]
    pub const fn attempt(self) -> ModelMigrationAttempt {
        self.attempt
    }

    /// Actor-persisted application transaction identity.
    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.transaction
    }
}

/// Complete model/control-history readback required for terminal success.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelMigrationCommitReceipt {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub transaction: TransactionRef,
    pub writer_generation: WriterGeneration,
    pub migration: ModelMigrationRef,
    pub target_model_head: ModelHeadRef,
    pub resulting_head: ExpectedHead,
    pub operation_selection: OperationSelectionRef,
}

/// Exact marker visibility after an attempted direct model commit.
///
/// The marker does not prove replay output, model head, or complete control history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelMigrationMarkerReceipt {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub transaction: TransactionRef,
    pub writer_generation: WriterGeneration,
}

/// Exhaustive observation from one no-retry model-migration attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelMigrationCommitObservation {
    /// The exact model transaction and complete control history read back successfully.
    Committed(ModelMigrationCommitReceipt),
    /// The marker is visible, but complete result/control history was not proved.
    MarkerAlreadyCommitted {
        marker: ModelMigrationMarkerReceipt,
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

/// Sole durable model-migration authority.
///
/// A production implementation writes the replayed, schema-validated model relations and exact
/// operation-selection/control records for this request. It performs one fenced application
/// transaction and must not retry, rebase, discover latest, or reduce command state.
#[async_trait]
pub trait ModelMigrationCommitPort: Send + Sync {
    async fn commit(
        &self,
        request: ModelMigrationCommitRequest,
    ) -> Result<ModelMigrationCommitObservation, CommandPortError>;
}

/// Read-only exact marker/control-history lookup key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelMigrationReconciliationRequest {
    attempt: ModelMigrationAttempt,
    active_recovery_owner: ExecutionOwner,
    transaction: TransactionRef,
}

impl ModelMigrationReconciliationRequest {
    /// Original command attempt, including its immutable transaction executor.
    #[must_use]
    pub const fn attempt(self) -> ModelMigrationAttempt {
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

/// Exact result of reading both the operation marker and complete model control history.
///
/// There is no bare absence. Non-commit and indeterminate outcomes carry evidence supplied by
/// the read authority; this effect never manufactures proof identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelMigrationMarkerObservation {
    Committed {
        receipt: ModelMigrationCommitReceipt,
        evidence: ReconciliationEvidenceRef,
    },
    ProvedNotCommitted {
        evidence: ReconciliationEvidenceRef,
    },
    Indeterminate {
        evidence: ReconciliationEvidenceRef,
    },
}

/// Read-only operation-marker and complete model control-history authority.
///
/// Implementations query the exact transaction and
/// [`ModelMigrationAttempt::execution_owner`] generation while fencing the read with
/// [`ModelMigrationReconciliationRequest::active_recovery_owner`].
#[async_trait]
pub trait ModelMigrationMarkerPort: Send + Sync {
    async fn read_exact(
        &self,
        request: ModelMigrationReconciliationRequest,
    ) -> Result<ModelMigrationMarkerObservation, CommandPortError>;
}

/// Concrete typed effect for `FabricCommandPayload::ApplyModelMigration`.
pub struct ModelMigrationCommandEffect {
    resolver: Arc<dyn ModelMigrationResolverPort>,
    commits: Arc<dyn ModelMigrationCommitPort>,
    markers: Arc<dyn ModelMigrationMarkerPort>,
}

impl std::fmt::Debug for ModelMigrationCommandEffect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModelMigrationCommandEffect")
            .field("resolver", &"installed")
            .field("commits", &"installed")
            .field("markers", &"installed")
            .finish()
    }
}

impl ModelMigrationCommandEffect {
    /// Install the three non-overlapping authorities required by model migration.
    #[must_use]
    pub const fn new(
        resolver: Arc<dyn ModelMigrationResolverPort>,
        commits: Arc<dyn ModelMigrationCommitPort>,
        markers: Arc<dyn ModelMigrationMarkerPort>,
    ) -> Self {
        Self {
            resolver,
            commits,
            markers,
        }
    }
}

#[async_trait]
impl ModelMigrationCommandEffectPort for ModelMigrationCommandEffect {
    async fn prepare(
        &self,
        executing: &CommandRecord,
        owner: ExecutionOwner,
        context: ReductionContext,
    ) -> Result<PrepareEffectOutcome, CommandPortError> {
        let validated =
            executing_attempt(executing, owner, context, CommandKind::ApplyModelMigration)?;
        let attempt = ModelMigrationAttempt::from_validated(validated)?;
        match self.resolver.resolve(attempt).await? {
            ModelMigrationResolution::Resolved(resolved) => {
                if resolved.attempt() != attempt {
                    return Err(CommandPortError::CorruptRecord);
                }
                Ok(PrepareEffectOutcome::Prepared {
                    transaction: resolved.transaction(),
                })
            }
            ModelMigrationResolution::KnownFailure(failure) => {
                Ok(PrepareEffectOutcome::KnownFailure { failure })
            }
            ModelMigrationResolution::Cancelled(cancellation) => {
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
            CommandKind::ApplyModelMigration,
        )?;
        let attempt = ModelMigrationAttempt::from_validated(validated)?;
        match self
            .commits
            .commit(ModelMigrationCommitRequest {
                attempt,
                transaction,
            })
            .await?
        {
            ModelMigrationCommitObservation::Committed(receipt) => {
                validate_direct_receipt(attempt, transaction, receipt)?;
                Ok(CommitEffectOutcome::Committed {
                    result: result_from_receipt(receipt),
                })
            }
            ModelMigrationCommitObservation::MarkerAlreadyCommitted { marker, diagnostic } => {
                validate_direct_marker(attempt, transaction, marker)?;
                Ok(unknown(
                    UnknownCommitReason::ReadbackUnavailable,
                    diagnostic,
                ))
            }
            ModelMigrationCommitObservation::Conflict { diagnostic } => Ok(unknown(
                UnknownCommitReason::ReadbackUnavailable,
                diagnostic,
            )),
            ModelMigrationCommitObservation::Unknown { reason, diagnostic } => {
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
            CommandKind::ApplyModelMigration,
        )?;
        let attempt = ModelMigrationAttempt::from_validated(recovery.attempt())?;
        let request = ModelMigrationReconciliationRequest {
            attempt,
            active_recovery_owner: recovery.active_recovery_owner(),
            transaction,
        };
        match self.markers.read_exact(request).await? {
            ModelMigrationMarkerObservation::Committed { receipt, evidence } => {
                validate_reconciled_receipt(attempt, transaction, receipt)?;
                Ok(ReconciliationObservation::Committed {
                    evidence,
                    result: result_from_receipt(receipt),
                })
            }
            ModelMigrationMarkerObservation::ProvedNotCommitted { evidence } => {
                Ok(ReconciliationObservation::NotCommitted { evidence })
            }
            ModelMigrationMarkerObservation::Indeterminate { evidence } => {
                Ok(ReconciliationObservation::Indeterminate { evidence })
            }
        }
    }
}

fn validate_direct_receipt(
    attempt: ModelMigrationAttempt,
    transaction: TransactionRef,
    receipt: ModelMigrationCommitReceipt,
) -> Result<(), CommandPortError> {
    validate_receipt_identity(attempt, transaction, receipt)?;
    validate_receipt_generation(attempt, receipt)
}

fn validate_reconciled_receipt(
    attempt: ModelMigrationAttempt,
    transaction: TransactionRef,
    receipt: ModelMigrationCommitReceipt,
) -> Result<(), CommandPortError> {
    validate_receipt_identity(attempt, transaction, receipt)?;
    validate_receipt_generation(attempt, receipt)
}

fn validate_receipt_identity(
    attempt: ModelMigrationAttempt,
    transaction: TransactionRef,
    receipt: ModelMigrationCommitReceipt,
) -> Result<(), CommandPortError> {
    let command = attempt.command();
    if receipt.workspace_id != command.ownership.workspace_id
        || receipt.operation_id != command.identity.operation_id
        || receipt.transaction != transaction
        || receipt.migration != attempt.migration()
        || receipt.target_model_head != attempt.target_model_head()
        || receipt.resulting_head != command.expected_head
    {
        return Err(CommandPortError::CorruptRecord);
    }
    Ok(())
}

fn validate_receipt_generation(
    attempt: ModelMigrationAttempt,
    receipt: ModelMigrationCommitReceipt,
) -> Result<(), CommandPortError> {
    attempt
        .validated
        .validate_receipt_generation(receipt.writer_generation)
}

fn validate_direct_marker(
    attempt: ModelMigrationAttempt,
    transaction: TransactionRef,
    marker: ModelMigrationMarkerReceipt,
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

const fn result_from_receipt(receipt: ModelMigrationCommitReceipt) -> CommandResult {
    CommandResult::ModelMigrationApplied {
        model_head: receipt.target_model_head,
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
        CompilerReleaseRef, IdempotencyKey, LeaseId, PrincipalId, ProviderSetRef,
        ResourceEnvelopeRef, SourceGeneration, WriterFence,
    };

    struct ResolverProbe {
        transaction: TransactionRef,
        requests: Mutex<Vec<ModelMigrationAttempt>>,
    }

    #[async_trait]
    impl ModelMigrationResolverPort for ResolverProbe {
        async fn resolve(
            &self,
            attempt: ModelMigrationAttempt,
        ) -> Result<ModelMigrationResolution, CommandPortError> {
            self.requests
                .lock()
                .expect("resolver requests lock")
                .push(attempt);
            Ok(ModelMigrationResolution::Resolved(
                ResolvedModelMigration::new(attempt, self.transaction),
            ))
        }
    }

    struct CommitProbe {
        observation: ModelMigrationCommitObservation,
        requests: Mutex<Vec<ModelMigrationCommitRequest>>,
    }

    #[async_trait]
    impl ModelMigrationCommitPort for CommitProbe {
        async fn commit(
            &self,
            request: ModelMigrationCommitRequest,
        ) -> Result<ModelMigrationCommitObservation, CommandPortError> {
            self.requests
                .lock()
                .expect("commit requests lock")
                .push(request);
            Ok(self.observation)
        }
    }

    struct MarkerProbe {
        observation: ModelMigrationMarkerObservation,
        requests: Mutex<Vec<ModelMigrationReconciliationRequest>>,
    }

    #[async_trait]
    impl ModelMigrationMarkerPort for MarkerProbe {
        async fn read_exact(
            &self,
            request: ModelMigrationReconciliationRequest,
        ) -> Result<ModelMigrationMarkerObservation, CommandPortError> {
            self.requests
                .lock()
                .expect("marker requests lock")
                .push(request);
            Ok(self.observation)
        }
    }

    struct Harness {
        effect: ModelMigrationCommandEffect,
        resolver: Arc<ResolverProbe>,
        commits: Arc<CommitProbe>,
        markers: Arc<MarkerProbe>,
    }

    impl Harness {
        fn new(
            commit: ModelMigrationCommitObservation,
            marker: ModelMigrationMarkerObservation,
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
            let effect = ModelMigrationCommandEffect::new(
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
    async fn prepare_is_read_only_and_commit_requires_exact_model_readback() {
        let owner = owner(1, 1, 1);
        let executing = executing_record(command(owner.fence), owner);
        let harness = Harness::new(
            ModelMigrationCommitObservation::Committed(receipt(
                owner.fence.generation,
                transaction(),
            )),
            indeterminate_marker(),
        );

        assert_eq!(
            harness
                .effect
                .prepare(&executing, owner, context(owner))
                .await
                .expect("read-only migration resolution succeeds"),
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
            "prepare must not cross the durable model commit port"
        );

        let prepared = prepare_record(executing, owner, transaction());
        assert_eq!(
            harness
                .effect
                .commit(&prepared, owner, transaction(), context(owner))
                .await
                .expect("exact model history readback succeeds"),
            CommitEffectOutcome::Committed {
                result: expected_result()
            }
        );
        let resolution_request = harness
            .resolver
            .requests
            .lock()
            .expect("resolver requests lock")[0];
        assert_eq!(resolution_request.execution_owner(), owner);
        assert_eq!(resolution_request.migration(), migration());
        assert_eq!(resolution_request.target_model_head(), target_model_head());
        let commit_request = harness
            .commits
            .requests
            .lock()
            .expect("commit requests lock")[0];
        assert_eq!(commit_request.attempt().execution_owner(), owner);
        assert_eq!(commit_request.transaction(), transaction());
    }

    #[tokio::test]
    async fn conflict_and_unknown_require_reconciliation_without_retry() {
        let owner = owner(1, 1, 1);
        let observations = [
            ModelMigrationCommitObservation::Conflict {
                diagnostic: diagnostic(41),
            },
            ModelMigrationCommitObservation::Unknown {
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
                    .expect("typed ambiguous model outcome"),
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
    async fn marker_visibility_without_model_history_is_not_commit_confirmation() {
        let owner = owner(1, 1, 1);
        let diagnostic = diagnostic(51);
        let harness = Harness::new(
            ModelMigrationCommitObservation::MarkerAlreadyCommitted {
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
                .expect("exact model marker binding is valid"),
            CommitEffectOutcome::Unknown {
                unknown: UnknownCommit {
                    reason: UnknownCommitReason::ReadbackUnavailable,
                    diagnostic,
                }
            }
        );
    }

    #[tokio::test]
    async fn direct_migration_rejects_generation_migration_or_model_head_drift() {
        let owner = owner(1, 1, 1);
        let wrong_generation = receipt(generation(2), transaction());
        let mut wrong_migration = receipt(generation(1), transaction());
        wrong_migration.migration = ModelMigrationRef::from_bytes([0x41; 32]);
        let mut wrong_model_head = receipt(generation(1), transaction());
        wrong_model_head.target_model_head = ModelHeadRef::from_bytes([0x51; 32]);

        for contradictory in [wrong_generation, wrong_migration, wrong_model_head] {
            let harness = Harness::new(
                ModelMigrationCommitObservation::Committed(contradictory),
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
    async fn reconciliation_binds_executor_generation_and_active_recovery_fence_separately() {
        let execution_owner = owner(1, 1, 1);
        let recovery_owner = owner(2, 2, 2);
        let prepared = prepare_record(
            executing_record(command(execution_owner.fence), execution_owner),
            execution_owner,
            transaction(),
        );
        let awaiting = awaiting_record(prepared, execution_owner, transaction());
        let evidence = ReconciliationEvidenceRef::from_bytes([0x61; 32]);
        let harness = Harness::new(
            ModelMigrationCommitObservation::Unknown {
                reason: UnknownCommitReason::ProcessInterrupted,
                diagnostic: diagnostic(60),
            },
            ModelMigrationMarkerObservation::Committed {
                receipt: receipt(execution_owner.fence.generation, transaction()),
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
                        current_head: ExpectedHead::Empty,
                        active_fence: recovery_owner.fence,
                    },
                )
                .await
                .expect("exact model history proves the executor-generation commit"),
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
        assert_eq!(request.attempt().execution_owner(), execution_owner);
        assert_eq!(request.active_recovery_owner(), recovery_owner);
        assert_eq!(request.attempt().migration(), migration());
        assert_eq!(request.attempt().target_model_head(), target_model_head());
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
            payload: FabricCommandPayload::ApplyModelMigration {
                migration: migration(),
                target_model_head: target_model_head(),
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
        .expect("admit model-migration command");
        let AdmissionOutcome::New(admitted) = admitted else {
            panic!("empty admission creates a record")
        };
        CommandReducer::reduce(&admitted, CommandEvent::Start { owner }, context(owner))
            .expect("start model-migration command")
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
        .expect("prepare model-migration transaction")
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
        .expect("record unknown model-migration commit")
        .record
    }

    fn receipt(
        writer_generation: WriterGeneration,
        transaction: TransactionRef,
    ) -> ModelMigrationCommitReceipt {
        ModelMigrationCommitReceipt {
            workspace_id: workspace_id(),
            operation_id: operation_id(),
            transaction,
            writer_generation,
            migration: migration(),
            target_model_head: target_model_head(),
            resulting_head: ExpectedHead::Empty,
            operation_selection: selection(),
        }
    }

    fn marker_receipt(writer_generation: WriterGeneration) -> ModelMigrationMarkerReceipt {
        ModelMigrationMarkerReceipt {
            workspace_id: workspace_id(),
            operation_id: operation_id(),
            transaction: transaction(),
            writer_generation,
        }
    }

    fn expected_result() -> CommandResult {
        CommandResult::ModelMigrationApplied {
            model_head: target_model_head(),
            resulting_head: ExpectedHead::Empty,
            selection: selection(),
        }
    }

    fn indeterminate_marker() -> ModelMigrationMarkerObservation {
        ModelMigrationMarkerObservation::Indeterminate {
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

    fn migration() -> ModelMigrationRef {
        ModelMigrationRef::from_bytes([0x40; 32])
    }

    fn target_model_head() -> ModelHeadRef {
        ModelHeadRef::from_bytes([0x50; 32])
    }

    fn diagnostic(seed: u8) -> DiagnosticRef {
        DiagnosticRef::from_bytes([seed; 32])
    }
}
