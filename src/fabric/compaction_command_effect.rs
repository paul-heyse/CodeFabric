//! Typed `CompactRelations` command effect and exact Delta reconciliation seam.
//!
//! Compaction changes physical state only after an exact relation set and equivalence proof have
//! been resolved. Preparation is read-only. The commit port is the sole durable Delta boundary
//! and may execute only controlled zero-retry plan writes. Recovery never retries from a timeout,
//! conflict, or marker alone: it reads the exact application marker and complete control history.

use std::sync::Arc;

use async_trait::async_trait;

use super::command::{
    CommandCancellation, CommandFailure, CommandKind, CommandRecord, CommandResult, DiagnosticRef,
    ExecutionOwner, ExpectedHead, FabricCommand, FabricCommandPayload, OperationId,
    OperationSelectionRef, ProofReceiptRef, ReconciliationEvidenceRef, ReconciliationObservation,
    ReductionContext, RelationSetRef, TransactionRef, UnknownCommit, UnknownCommitReason,
    WorkspaceId, WriterGeneration,
};
use super::command_actor::{CommandPortError, CommitEffectOutcome, PrepareEffectOutcome};
use super::command_effect_contract::{
    ValidatedCommandAttempt, executing_attempt, prepared_attempt, reconciliation_attempt,
};
use super::command_effect_router::CompactionCommandEffectPort;

/// Exact immutable compaction attempt supplied to proof/catalog resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactionAttempt {
    validated: ValidatedCommandAttempt,
}

impl CompactionAttempt {
    fn from_validated(validated: ValidatedCommandAttempt) -> Result<Self, CommandPortError> {
        if matches!(
            validated.command().payload,
            FabricCommandPayload::CompactRelations { .. }
        ) {
            Ok(Self { validated })
        } else {
            Err(CommandPortError::CorruptRecord)
        }
    }

    /// Immutable admitted `CompactRelations` command.
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

    /// Exact relation set selected for physical compaction.
    #[must_use]
    pub fn relations(self) -> RelationSetRef {
        let FabricCommandPayload::CompactRelations { relations, .. } = self.command().payload
        else {
            unreachable!("compaction attempt is constructed only after payload checks")
        };
        relations
    }

    /// Exact proof that the replacement is logically equivalent.
    #[must_use]
    pub fn equivalence_proof(self) -> ProofReceiptRef {
        let FabricCommandPayload::CompactRelations {
            equivalence_proof, ..
        } = self.command().payload
        else {
            unreachable!("compaction attempt is constructed only after payload checks")
        };
        equivalence_proof
    }
}

/// Fully bound result of immutable relation/proof resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedCompaction {
    attempt: CompactionAttempt,
    transaction: TransactionRef,
}

impl ResolvedCompaction {
    /// Bind one deterministic transaction to the exact immutable attempt.
    #[must_use]
    pub const fn new(attempt: CompactionAttempt, transaction: TransactionRef) -> Self {
        Self {
            attempt,
            transaction,
        }
    }

    /// Exact compaction attempt resolved by the catalog/proof authority.
    #[must_use]
    pub const fn attempt(self) -> CompactionAttempt {
        self.attempt
    }

    /// Deterministic application transaction persisted before commit.
    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.transaction
    }
}

/// Immutable compaction-resolution result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompactionResolution {
    Resolved(ResolvedCompaction),
    KnownFailure(CommandFailure),
    Cancelled(CommandCancellation),
}

/// Read-only relation-set and equivalence-proof resolver.
///
/// Implementations may resolve model-selected plans and validate the exact proof. They must not
/// write Delta, operation markers, control history, or another durable target.
#[async_trait]
pub trait CompactionResolverPort: Send + Sync {
    async fn resolve(
        &self,
        attempt: CompactionAttempt,
    ) -> Result<CompactionResolution, CommandPortError>;
}

/// Exact contract presented to the sole durable compaction boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactionCommitRequest {
    attempt: CompactionAttempt,
    transaction: TransactionRef,
}

impl CompactionCommitRequest {
    /// Exact command attempt authorized for this commit.
    #[must_use]
    pub const fn attempt(self) -> CompactionAttempt {
        self.attempt
    }

    /// Actor-persisted application transaction identity.
    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.transaction
    }
}

/// Complete readback receipt needed to construct a `RelationsCompacted` result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactionCommitReceipt {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub transaction: TransactionRef,
    pub writer_generation: WriterGeneration,
    pub relations: RelationSetRef,
    pub equivalence_proof: ProofReceiptRef,
    pub resulting_head: ExpectedHead,
    pub operation_selection: OperationSelectionRef,
}

/// Exact marker visibility read after an attempted direct commit.
///
/// Marker visibility does not prove the complete result or equivalence selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactionMarkerReceipt {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub transaction: TransactionRef,
    pub writer_generation: WriterGeneration,
}

/// Exhaustive observation from one zero-retry compaction attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompactionCommitObservation {
    /// Every selected component and control record committed and read back exactly.
    Committed(CompactionCommitReceipt),
    /// The marker is visible, but complete result/control history was not proved.
    MarkerAlreadyCommitted {
        marker: CompactionMarkerReceipt,
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

/// Sole durable compaction authority.
///
/// A production implementation resolves the proved replacement plan from the exact request,
/// binds every [`super::delta_write::ControlledDeltaWriteSpec`] to the operation ID and active
/// writer generation, selects `ControlledDeltaWriteMode::ReplaceAll`, and invokes
/// [`super::delta_write::write_exact_delta_plan`] with the originating DataFusion session. It
/// performs no internal retry, rebase, or latest-state discovery. Optimizing/retrying maintenance
/// builders are not legal implementations.
#[async_trait]
pub trait CompactionDeltaCommitPort: Send + Sync {
    async fn commit(
        &self,
        request: CompactionCommitRequest,
    ) -> Result<CompactionCommitObservation, CommandPortError>;
}

/// Read-only exact marker/control-history lookup key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactionReconciliationRequest {
    attempt: CompactionAttempt,
    active_recovery_owner: ExecutionOwner,
    transaction: TransactionRef,
}

impl CompactionReconciliationRequest {
    /// Original command attempt, including its immutable transaction executor.
    #[must_use]
    pub const fn attempt(self) -> CompactionAttempt {
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
/// There is no bare absence. Non-commit and indeterminate outcomes carry evidence supplied by
/// the read authority; this effect never manufactures proof identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompactionOperationMarkerObservation {
    Committed {
        receipt: CompactionCommitReceipt,
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
/// Implementations query the exact transaction and [`CompactionAttempt::owner`] execution
/// generation while fencing the read with
/// [`CompactionReconciliationRequest::active_recovery_owner`].
#[async_trait]
pub trait CompactionOperationMarkerPort: Send + Sync {
    async fn read_exact(
        &self,
        request: CompactionReconciliationRequest,
    ) -> Result<CompactionOperationMarkerObservation, CommandPortError>;
}

/// Concrete typed effect for `FabricCommandPayload::CompactRelations`.
pub struct CompactionCommandEffect {
    resolver: Arc<dyn CompactionResolverPort>,
    commits: Arc<dyn CompactionDeltaCommitPort>,
    markers: Arc<dyn CompactionOperationMarkerPort>,
}

impl std::fmt::Debug for CompactionCommandEffect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompactionCommandEffect")
            .field("resolver", &"installed")
            .field("commits", &"installed")
            .field("markers", &"installed")
            .finish()
    }
}

impl CompactionCommandEffect {
    /// Install the three non-overlapping authorities required by compaction.
    #[must_use]
    pub const fn new(
        resolver: Arc<dyn CompactionResolverPort>,
        commits: Arc<dyn CompactionDeltaCommitPort>,
        markers: Arc<dyn CompactionOperationMarkerPort>,
    ) -> Self {
        Self {
            resolver,
            commits,
            markers,
        }
    }
}

#[async_trait]
impl CompactionCommandEffectPort for CompactionCommandEffect {
    async fn prepare(
        &self,
        executing: &CommandRecord,
        owner: ExecutionOwner,
        context: ReductionContext,
    ) -> Result<PrepareEffectOutcome, CommandPortError> {
        let validated =
            executing_attempt(executing, owner, context, CommandKind::CompactRelations)?;
        let attempt = CompactionAttempt::from_validated(validated)?;
        match self.resolver.resolve(attempt).await? {
            CompactionResolution::Resolved(resolved) => {
                if resolved.attempt() != attempt {
                    return Err(CommandPortError::CorruptRecord);
                }
                Ok(PrepareEffectOutcome::Prepared {
                    transaction: resolved.transaction(),
                })
            }
            CompactionResolution::KnownFailure(failure) => {
                Ok(PrepareEffectOutcome::KnownFailure { failure })
            }
            CompactionResolution::Cancelled(cancellation) => {
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
            CommandKind::CompactRelations,
        )?;
        let attempt = CompactionAttempt::from_validated(validated)?;
        let request = CompactionCommitRequest {
            attempt,
            transaction,
        };
        match self.commits.commit(request).await? {
            CompactionCommitObservation::Committed(receipt) => {
                validate_direct_receipt(attempt, transaction, receipt)?;
                Ok(CommitEffectOutcome::Committed {
                    result: result_from_receipt(receipt),
                })
            }
            CompactionCommitObservation::MarkerAlreadyCommitted { marker, diagnostic } => {
                validate_direct_marker(attempt, transaction, marker)?;
                Ok(unknown(
                    UnknownCommitReason::ReadbackUnavailable,
                    diagnostic,
                ))
            }
            CompactionCommitObservation::Conflict { diagnostic } => Ok(unknown(
                UnknownCommitReason::ReadbackUnavailable,
                diagnostic,
            )),
            CompactionCommitObservation::Unknown { reason, diagnostic } => {
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
            CommandKind::CompactRelations,
        )?;
        let attempt = CompactionAttempt::from_validated(recovery.attempt())?;
        let request = CompactionReconciliationRequest {
            attempt,
            active_recovery_owner: recovery.active_recovery_owner(),
            transaction,
        };
        match self.markers.read_exact(request).await? {
            CompactionOperationMarkerObservation::Committed { receipt, evidence } => {
                validate_reconciled_receipt(attempt, transaction, receipt)?;
                Ok(ReconciliationObservation::Committed {
                    evidence,
                    result: result_from_receipt(receipt),
                })
            }
            CompactionOperationMarkerObservation::ProvedNotCommitted { evidence } => {
                Ok(ReconciliationObservation::NotCommitted { evidence })
            }
            CompactionOperationMarkerObservation::Indeterminate { evidence } => {
                Ok(ReconciliationObservation::Indeterminate { evidence })
            }
        }
    }
}

fn validate_direct_receipt(
    attempt: CompactionAttempt,
    transaction: TransactionRef,
    receipt: CompactionCommitReceipt,
) -> Result<(), CommandPortError> {
    validate_receipt_identity(attempt, transaction, receipt)?;
    attempt
        .validated
        .validate_receipt_generation(receipt.writer_generation)
}

fn validate_reconciled_receipt(
    attempt: CompactionAttempt,
    transaction: TransactionRef,
    receipt: CompactionCommitReceipt,
) -> Result<(), CommandPortError> {
    validate_receipt_identity(attempt, transaction, receipt)?;
    attempt
        .validated
        .validate_receipt_generation(receipt.writer_generation)
}

fn validate_receipt_identity(
    attempt: CompactionAttempt,
    transaction: TransactionRef,
    receipt: CompactionCommitReceipt,
) -> Result<(), CommandPortError> {
    let command = attempt.command();
    if receipt.workspace_id != command.ownership.workspace_id
        || receipt.operation_id != command.identity.operation_id
        || receipt.transaction != transaction
        || receipt.relations != attempt.relations()
        || receipt.equivalence_proof != attempt.equivalence_proof()
        || receipt.resulting_head != command.expected_head
    {
        return Err(CommandPortError::CorruptRecord);
    }
    Ok(())
}

fn validate_direct_marker(
    attempt: CompactionAttempt,
    transaction: TransactionRef,
    marker: CompactionMarkerReceipt,
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

const fn result_from_receipt(receipt: CompactionCommitReceipt) -> CommandResult {
    CommandResult::RelationsCompacted {
        relations: receipt.relations,
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
        CompilerReleaseRef, IdempotencyKey, LeaseId, ModelHeadRef, PrincipalId, ProviderSetRef,
        ResourceEnvelopeRef, SourceGeneration, WriterFence,
    };

    struct ResolverProbe {
        transaction: TransactionRef,
        requests: Mutex<Vec<CompactionAttempt>>,
    }

    #[async_trait]
    impl CompactionResolverPort for ResolverProbe {
        async fn resolve(
            &self,
            attempt: CompactionAttempt,
        ) -> Result<CompactionResolution, CommandPortError> {
            self.requests
                .lock()
                .expect("resolver requests lock")
                .push(attempt);
            Ok(CompactionResolution::Resolved(ResolvedCompaction::new(
                attempt,
                self.transaction,
            )))
        }
    }

    struct CommitProbe {
        observation: CompactionCommitObservation,
        requests: Mutex<Vec<CompactionCommitRequest>>,
    }

    #[async_trait]
    impl CompactionDeltaCommitPort for CommitProbe {
        async fn commit(
            &self,
            request: CompactionCommitRequest,
        ) -> Result<CompactionCommitObservation, CommandPortError> {
            self.requests
                .lock()
                .expect("commit requests lock")
                .push(request);
            Ok(self.observation)
        }
    }

    struct MarkerProbe {
        observation: CompactionOperationMarkerObservation,
        requests: Mutex<Vec<CompactionReconciliationRequest>>,
    }

    #[async_trait]
    impl CompactionOperationMarkerPort for MarkerProbe {
        async fn read_exact(
            &self,
            request: CompactionReconciliationRequest,
        ) -> Result<CompactionOperationMarkerObservation, CommandPortError> {
            self.requests
                .lock()
                .expect("marker requests lock")
                .push(request);
            Ok(self.observation)
        }
    }

    struct Harness {
        effect: CompactionCommandEffect,
        resolver: Arc<ResolverProbe>,
        commits: Arc<CommitProbe>,
        markers: Arc<MarkerProbe>,
    }

    impl Harness {
        fn new(
            commit: CompactionCommitObservation,
            marker: CompactionOperationMarkerObservation,
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
            let effect =
                CompactionCommandEffect::new(resolver.clone(), commits.clone(), markers.clone());
            Self {
                effect,
                resolver,
                commits,
                markers,
            }
        }
    }

    #[tokio::test]
    async fn prepare_is_read_only_and_commit_requires_exact_compaction_readback() {
        let owner = owner(1, 1, 1);
        let executing = executing_record(command(owner.fence), owner);
        let harness = Harness::new(
            CompactionCommitObservation::Committed(receipt(owner.fence.generation, transaction())),
            indeterminate_marker(),
        );

        assert_eq!(
            harness
                .effect
                .prepare(&executing, owner, context(owner))
                .await
                .expect("immutable compaction resolution succeeds"),
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

        let prepared = prepare_record(executing, owner, transaction());
        assert_eq!(
            harness
                .effect
                .commit(&prepared, owner, transaction(), context(owner))
                .await
                .expect("exact compaction readback succeeds"),
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
        assert_eq!(resolution_request.relations(), relations());
        assert_eq!(resolution_request.equivalence_proof(), proof());
        let commit_request = harness
            .commits
            .requests
            .lock()
            .expect("commit requests lock")[0];
        assert_eq!(commit_request.attempt().owner(), owner);
        assert_eq!(commit_request.transaction(), transaction());
    }

    #[tokio::test]
    async fn conflict_and_unknown_require_reconciliation_without_retry() {
        let owner = owner(1, 1, 1);
        let observations = [
            CompactionCommitObservation::Conflict {
                diagnostic: diagnostic(41),
            },
            CompactionCommitObservation::Unknown {
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
                    .expect("typed ambiguous compaction outcome"),
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
    async fn marker_visibility_without_complete_history_is_not_compaction_confirmation() {
        let owner = owner(1, 1, 1);
        let diagnostic = diagnostic(51);
        let harness = Harness::new(
            CompactionCommitObservation::MarkerAlreadyCommitted {
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
                .expect("exact compaction marker binding is valid"),
            CommitEffectOutcome::Unknown {
                unknown: UnknownCommit {
                    reason: UnknownCommitReason::ReadbackUnavailable,
                    diagnostic,
                }
            }
        );
    }

    #[tokio::test]
    async fn direct_compaction_rejects_generation_relation_or_proof_drift() {
        let owner = owner(1, 1, 1);
        let wrong_generation = receipt(generation(2), transaction());
        let mut wrong_relations = receipt(generation(1), transaction());
        wrong_relations.relations = RelationSetRef::from_bytes([0x41; 32]);
        let mut wrong_proof = receipt(generation(1), transaction());
        wrong_proof.equivalence_proof = ProofReceiptRef::from_bytes([0x51; 32]);

        for contradictory in [wrong_generation, wrong_relations, wrong_proof] {
            let harness = Harness::new(
                CompactionCommitObservation::Committed(contradictory),
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
    async fn reconciliation_reads_exact_transaction_under_active_recovery_generation() {
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
            CompactionCommitObservation::Unknown {
                reason: UnknownCommitReason::ProcessInterrupted,
                diagnostic: diagnostic(60),
            },
            CompactionOperationMarkerObservation::Committed {
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
                .expect("exact history proves the prior-generation compaction"),
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
        assert_eq!(request.attempt().relations(), relations());
        assert_eq!(request.attempt().equivalence_proof(), proof());
        assert_eq!(request.attempt().owner(), original);
        assert_eq!(
            request.attempt().owner().fence.generation,
            generation(1),
            "receipt validation remains bound to the writer that owned the attempted commit"
        );
        assert_eq!(request.active_recovery_owner(), recovery);
        assert_eq!(
            request.active_recovery_owner().fence.generation,
            generation(2),
            "the read itself is fenced by the active recovery generation"
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
            payload: FabricCommandPayload::CompactRelations {
                relations: relations(),
                equivalence_proof: proof(),
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
        .expect("admit compaction command");
        let AdmissionOutcome::New(admitted) = admitted else {
            panic!("empty admission creates a record")
        };
        CommandReducer::reduce(&admitted, CommandEvent::Start { owner }, context(owner))
            .expect("start compaction command")
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
        .expect("prepare compaction transaction")
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
        .expect("record unknown compaction commit")
        .record
    }

    fn receipt(
        writer_generation: WriterGeneration,
        transaction: TransactionRef,
    ) -> CompactionCommitReceipt {
        CompactionCommitReceipt {
            workspace_id: workspace_id(),
            operation_id: operation_id(),
            transaction,
            writer_generation,
            relations: relations(),
            equivalence_proof: proof(),
            resulting_head: ExpectedHead::Empty,
            operation_selection: selection(),
        }
    }

    fn marker_receipt(writer_generation: WriterGeneration) -> CompactionMarkerReceipt {
        CompactionMarkerReceipt {
            workspace_id: workspace_id(),
            operation_id: operation_id(),
            transaction: transaction(),
            writer_generation,
        }
    }

    fn expected_result() -> CommandResult {
        CommandResult::RelationsCompacted {
            relations: relations(),
            resulting_head: ExpectedHead::Empty,
            selection: selection(),
        }
    }

    fn indeterminate_marker() -> CompactionOperationMarkerObservation {
        CompactionOperationMarkerObservation::Indeterminate {
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

    fn relations() -> RelationSetRef {
        RelationSetRef::from_bytes([0x40; 32])
    }

    fn proof() -> ProofReceiptRef {
        ProofReceiptRef::from_bytes([0x50; 32])
    }

    fn diagnostic(seed: u8) -> DiagnosticRef {
        DiagnosticRef::from_bytes([seed; 32])
    }
}
