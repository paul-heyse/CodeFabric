//! Typed effect boundary for the closed `Administer` command family.
//!
//! Administrative actions remain static command variants while their request bodies are durable,
//! typed data. Preparation resolves the exact referenced request without mutation. One commit port
//! owns all durable effects, performs one zero-retry attempt, and returns complete readback. A
//! visible marker without matching control history is deliberately an unknown outcome.

use std::sync::Arc;

use async_trait::async_trait;

use super::command::{
    AdministrationAction, AdministrationRequestRef, CommandCancellation, CommandFailure,
    CommandKind, CommandRecord, CommandResult, DiagnosticRef, ExecutionOwner, ExpectedHead,
    FabricCommand, FabricCommandPayload, OperationId, OperationSelectionRef,
    ReconciliationEvidenceRef, ReconciliationObservation, ReductionContext, TransactionRef,
    UnknownCommit, UnknownCommitReason, WorkspaceId, WriterGeneration,
};
use super::command_actor::{CommandPortError, CommitEffectOutcome, PrepareEffectOutcome};
use super::command_effect_contract::{
    ValidatedCommandAttempt, executing_attempt, prepared_attempt, reconciliation_attempt,
};
use super::command_effect_router::AdministrationCommandEffectPort;

/// Immutable administrative transaction attempt proved by the shared reducer contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdministrationAttempt {
    validated: ValidatedCommandAttempt,
}

impl AdministrationAttempt {
    fn from_validated(validated: ValidatedCommandAttempt) -> Result<Self, CommandPortError> {
        if matches!(
            validated.command().payload,
            FabricCommandPayload::Administer { .. }
        ) {
            Ok(Self { validated })
        } else {
            Err(CommandPortError::CorruptRecord)
        }
    }

    /// Immutable admitted command.
    #[must_use]
    pub const fn command(self) -> FabricCommand {
        self.validated.command()
    }

    /// Reducer-owned attempt number.
    #[must_use]
    pub const fn attempt(self) -> u32 {
        self.validated.attempt()
    }

    /// Immutable actor/fence that prepared and attempted this transaction.
    #[must_use]
    pub const fn execution_owner(self) -> ExecutionOwner {
        self.validated.execution_owner()
    }

    /// Closed administrative action selected by the command.
    #[must_use]
    pub fn action(self) -> AdministrationAction {
        let FabricCommandPayload::Administer { action, .. } = self.command().payload else {
            unreachable!("administration attempt is constructed only after payload validation")
        };
        action
    }

    /// Exact typed administrative request record.
    #[must_use]
    pub fn request(self) -> AdministrationRequestRef {
        let FabricCommandPayload::Administer { request, .. } = self.command().payload else {
            unreachable!("administration attempt is constructed only after payload validation")
        };
        request
    }
}

/// Fully bound result of read-only request resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedAdministration {
    attempt: AdministrationAttempt,
    transaction: TransactionRef,
}

impl ResolvedAdministration {
    /// Bind one deterministic transaction identity to the exact resolved request.
    #[must_use]
    pub const fn new(attempt: AdministrationAttempt, transaction: TransactionRef) -> Self {
        Self {
            attempt,
            transaction,
        }
    }

    /// Exact attempt resolved by request authority.
    #[must_use]
    pub const fn attempt(self) -> AdministrationAttempt {
        self.attempt
    }

    /// Deterministic application transaction persisted before commit.
    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.transaction
    }
}

/// Result of immutable request and policy resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdministrationResolution {
    Resolved(ResolvedAdministration),
    KnownFailure(CommandFailure),
    Cancelled(CommandCancellation),
}

/// Read-only typed request resolver.
///
/// Implementations validate the referenced request, policy, and semantic pins. They must not
/// mutate Delta state, operation markers, control history, or temporal caches.
#[async_trait]
pub trait AdministrationResolverPort: Send + Sync {
    async fn resolve(
        &self,
        attempt: AdministrationAttempt,
    ) -> Result<AdministrationResolution, CommandPortError>;
}

/// Exact contract presented to the sole durable administrative boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdministrationCommitRequest {
    attempt: AdministrationAttempt,
    transaction: TransactionRef,
}

impl AdministrationCommitRequest {
    /// Exact command attempt authorized for the write.
    #[must_use]
    pub const fn attempt(self) -> AdministrationAttempt {
        self.attempt
    }

    /// Actor-persisted application transaction identity.
    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.transaction
    }
}

/// Complete committed readback for an administrative action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdministrationCommitReceipt {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub transaction: TransactionRef,
    pub writer_generation: WriterGeneration,
    pub action: AdministrationAction,
    pub request: AdministrationRequestRef,
    pub resulting_head: ExpectedHead,
    pub operation_selection: OperationSelectionRef,
}

/// Exact application-marker identity visible after an attempted commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdministrationMarkerReceipt {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub transaction: TransactionRef,
    pub writer_generation: WriterGeneration,
}

/// Exhaustive observation from one zero-retry administrative commit attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdministrationCommitObservation {
    /// Marker, control history, selection, and resulting head were read back exactly.
    Committed(AdministrationCommitReceipt),
    /// Marker visibility alone cannot prove which administrative result committed.
    MarkerAlreadyCommitted {
        marker: AdministrationMarkerReceipt,
        diagnostic: DiagnosticRef,
    },
    /// Predecessor or application-transaction collision; never retried inside the port.
    Conflict { diagnostic: DiagnosticRef },
    /// Commit outcome or complete readback is not provable.
    Unknown {
        reason: UnknownCommitReason,
        diagnostic: DiagnosticRef,
    },
}

/// Sole durable authority for administrative effects.
///
/// A production implementation consumes the exact typed request and semantic pins, binds every
/// write to the operation ID and execution generation, and performs no internal retry, rebase, or
/// latest-state discovery. Any cache repair is a rebuildable projection of durable evidence, not
/// an independent source of truth.
#[async_trait]
pub trait AdministrationCommitPort: Send + Sync {
    async fn commit(
        &self,
        request: AdministrationCommitRequest,
    ) -> Result<AdministrationCommitObservation, CommandPortError>;
}

/// Exact marker/control-history lookup key for recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdministrationReconciliationRequest {
    attempt: AdministrationAttempt,
    active_recovery_owner: ExecutionOwner,
    transaction: TransactionRef,
}

impl AdministrationReconciliationRequest {
    /// Original transaction attempt and immutable execution authority.
    #[must_use]
    pub const fn attempt(self) -> AdministrationAttempt {
        self.attempt
    }

    /// Current authority for this read-only recovery query.
    #[must_use]
    pub const fn active_recovery_owner(self) -> ExecutionOwner {
        self.active_recovery_owner
    }

    /// Original actor-persisted transaction identity.
    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.transaction
    }
}

/// Exact result of reading both the application marker and complete control history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdministrationMarkerObservation {
    Committed {
        receipt: AdministrationCommitReceipt,
        evidence: ReconciliationEvidenceRef,
    },
    ProvedNotCommitted {
        evidence: ReconciliationEvidenceRef,
    },
    Indeterminate {
        evidence: ReconciliationEvidenceRef,
    },
}

/// Read-only authority for exact transaction reconciliation.
#[async_trait]
pub trait AdministrationMarkerPort: Send + Sync {
    async fn read_exact(
        &self,
        request: AdministrationReconciliationRequest,
    ) -> Result<AdministrationMarkerObservation, CommandPortError>;
}

/// Complete typed effect for the closed administrative command family.
pub struct AdministrationCommandEffect {
    resolver: Arc<dyn AdministrationResolverPort>,
    commits: Arc<dyn AdministrationCommitPort>,
    markers: Arc<dyn AdministrationMarkerPort>,
}

impl std::fmt::Debug for AdministrationCommandEffect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AdministrationCommandEffect")
            .field("resolver", &"installed")
            .field("commits", &"installed")
            .field("markers", &"installed")
            .finish()
    }
}

impl AdministrationCommandEffect {
    /// Install the three non-overlapping authorities required by administrative execution.
    #[must_use]
    pub const fn new(
        resolver: Arc<dyn AdministrationResolverPort>,
        commits: Arc<dyn AdministrationCommitPort>,
        markers: Arc<dyn AdministrationMarkerPort>,
    ) -> Self {
        Self {
            resolver,
            commits,
            markers,
        }
    }
}

#[async_trait]
impl AdministrationCommandEffectPort for AdministrationCommandEffect {
    async fn prepare(
        &self,
        executing: &CommandRecord,
        owner: ExecutionOwner,
        context: ReductionContext,
    ) -> Result<PrepareEffectOutcome, CommandPortError> {
        let validated = executing_attempt(executing, owner, context, CommandKind::Administer)?;
        let attempt = AdministrationAttempt::from_validated(validated)?;
        match self.resolver.resolve(attempt).await? {
            AdministrationResolution::Resolved(resolved) => {
                if resolved.attempt() != attempt {
                    return Err(CommandPortError::CorruptRecord);
                }
                Ok(PrepareEffectOutcome::Prepared {
                    transaction: resolved.transaction(),
                })
            }
            AdministrationResolution::KnownFailure(failure) => {
                Ok(PrepareEffectOutcome::KnownFailure { failure })
            }
            AdministrationResolution::Cancelled(cancellation) => {
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
            CommandKind::Administer,
        )?;
        let attempt = AdministrationAttempt::from_validated(validated)?;
        match self
            .commits
            .commit(AdministrationCommitRequest {
                attempt,
                transaction,
            })
            .await?
        {
            AdministrationCommitObservation::Committed(receipt) => {
                validate_receipt(attempt, transaction, receipt)?;
                Ok(CommitEffectOutcome::Committed {
                    result: result_from_receipt(receipt),
                })
            }
            AdministrationCommitObservation::MarkerAlreadyCommitted { marker, diagnostic } => {
                validate_marker(attempt, transaction, marker)?;
                Ok(unknown(
                    UnknownCommitReason::ReadbackUnavailable,
                    diagnostic,
                ))
            }
            AdministrationCommitObservation::Conflict { diagnostic } => Ok(unknown(
                UnknownCommitReason::ReadbackUnavailable,
                diagnostic,
            )),
            AdministrationCommitObservation::Unknown { reason, diagnostic } => {
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
            CommandKind::Administer,
        )?;
        let attempt = AdministrationAttempt::from_validated(recovery.attempt())?;
        let request = AdministrationReconciliationRequest {
            attempt,
            active_recovery_owner: recovery.active_recovery_owner(),
            transaction,
        };
        match self.markers.read_exact(request).await? {
            AdministrationMarkerObservation::Committed { receipt, evidence } => {
                validate_receipt(attempt, transaction, receipt)?;
                Ok(ReconciliationObservation::Committed {
                    evidence,
                    result: result_from_receipt(receipt),
                })
            }
            AdministrationMarkerObservation::ProvedNotCommitted { evidence } => {
                Ok(ReconciliationObservation::NotCommitted { evidence })
            }
            AdministrationMarkerObservation::Indeterminate { evidence } => {
                Ok(ReconciliationObservation::Indeterminate { evidence })
            }
        }
    }
}

fn validate_receipt(
    attempt: AdministrationAttempt,
    transaction: TransactionRef,
    receipt: AdministrationCommitReceipt,
) -> Result<(), CommandPortError> {
    attempt
        .validated
        .validate_receipt_generation(receipt.writer_generation)?;
    let command = attempt.command();
    if receipt.workspace_id != command.ownership.workspace_id
        || receipt.operation_id != command.identity.operation_id
        || receipt.transaction != transaction
        || receipt.action != attempt.action()
        || receipt.request != attempt.request()
        || receipt.resulting_head != command.expected_head
    {
        return Err(CommandPortError::CorruptRecord);
    }
    Ok(())
}

fn validate_marker(
    attempt: AdministrationAttempt,
    transaction: TransactionRef,
    marker: AdministrationMarkerReceipt,
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

const fn result_from_receipt(receipt: AdministrationCommitReceipt) -> CommandResult {
    CommandResult::AdministrationApplied {
        request: receipt.request,
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
        transaction: TransactionRef,
        requests: Mutex<Vec<AdministrationAttempt>>,
    }

    #[async_trait]
    impl AdministrationResolverPort for ResolverProbe {
        async fn resolve(
            &self,
            attempt: AdministrationAttempt,
        ) -> Result<AdministrationResolution, CommandPortError> {
            self.requests
                .lock()
                .expect("resolver requests lock")
                .push(attempt);
            Ok(AdministrationResolution::Resolved(
                ResolvedAdministration::new(attempt, self.transaction),
            ))
        }
    }

    struct CommitProbe {
        observation: AdministrationCommitObservation,
        requests: Mutex<Vec<AdministrationCommitRequest>>,
    }

    #[async_trait]
    impl AdministrationCommitPort for CommitProbe {
        async fn commit(
            &self,
            request: AdministrationCommitRequest,
        ) -> Result<AdministrationCommitObservation, CommandPortError> {
            self.requests
                .lock()
                .expect("commit requests lock")
                .push(request);
            Ok(self.observation)
        }
    }

    struct MarkerProbe {
        observation: AdministrationMarkerObservation,
        requests: Mutex<Vec<AdministrationReconciliationRequest>>,
    }

    #[async_trait]
    impl AdministrationMarkerPort for MarkerProbe {
        async fn read_exact(
            &self,
            request: AdministrationReconciliationRequest,
        ) -> Result<AdministrationMarkerObservation, CommandPortError> {
            self.requests
                .lock()
                .expect("marker requests lock")
                .push(request);
            Ok(self.observation)
        }
    }

    struct Harness {
        effect: AdministrationCommandEffect,
        resolver: Arc<ResolverProbe>,
        commits: Arc<CommitProbe>,
        markers: Arc<MarkerProbe>,
    }

    impl Harness {
        fn new(
            commit: AdministrationCommitObservation,
            marker: AdministrationMarkerObservation,
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
            let effect = AdministrationCommandEffect::new(
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
    async fn preparation_is_read_only_and_commit_requires_complete_exact_readback() {
        let executor = owner(1, 1, 1);
        let executing = executing_record(command(executor.fence), executor);
        let harness = Harness::new(
            AdministrationCommitObservation::Committed(receipt(executor.fence.generation)),
            indeterminate(),
        );

        assert_eq!(
            harness
                .effect
                .prepare(&executing, executor, context(&executing, executor.fence))
                .await
                .expect("typed request resolves"),
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
            "preparation cannot cross the durable mutation boundary"
        );

        let prepared = prepared_record(executing, executor);
        assert_eq!(
            harness
                .effect
                .commit(
                    &prepared,
                    executor,
                    transaction(),
                    context(&prepared, executor.fence),
                )
                .await
                .expect("exact administrative readback commits"),
            CommitEffectOutcome::Committed {
                result: expected_result()
            }
        );
        let resolved = harness
            .resolver
            .requests
            .lock()
            .expect("resolver requests lock")[0];
        assert_eq!(resolved.action(), AdministrationAction::ReconcileOperation);
        assert_eq!(resolved.request(), request());
        assert_eq!(resolved.execution_owner(), executor);
        let committed = harness
            .commits
            .requests
            .lock()
            .expect("commit requests lock")[0];
        assert_eq!(committed.attempt(), resolved);
        assert_eq!(committed.transaction(), transaction());
    }

    #[tokio::test]
    async fn receipt_identity_and_exact_execution_generation_are_mandatory() {
        let executor = owner(1, 1, 1);
        let prepared = prepared_record(
            executing_record(command(executor.fence), executor),
            executor,
        );
        let mut wrong_action = receipt(executor.fence.generation);
        wrong_action.action = AdministrationAction::RepairTemporalCache;
        let mut wrong_request = receipt(executor.fence.generation);
        wrong_request.request = AdministrationRequestRef::from_bytes([0x91; 32]);
        let wrong_generation = receipt(generation(2));

        for contradictory in [wrong_action, wrong_request, wrong_generation] {
            let harness = Harness::new(
                AdministrationCommitObservation::Committed(contradictory),
                indeterminate(),
            );
            assert_eq!(
                harness
                    .effect
                    .commit(
                        &prepared,
                        executor,
                        transaction(),
                        context(&prepared, executor.fence),
                    )
                    .await,
                Err(CommandPortError::CorruptRecord)
            );
        }
    }

    #[tokio::test]
    async fn marker_visibility_is_unknown_and_never_triggers_an_internal_retry() {
        let executor = owner(1, 1, 1);
        let diagnostic = diagnostic(0x51);
        let harness = Harness::new(
            AdministrationCommitObservation::MarkerAlreadyCommitted {
                marker: marker(executor.fence.generation),
                diagnostic,
            },
            indeterminate(),
        );
        let prepared = prepared_record(
            executing_record(command(executor.fence), executor),
            executor,
        );

        assert_eq!(
            harness
                .effect
                .commit(
                    &prepared,
                    executor,
                    transaction(),
                    context(&prepared, executor.fence),
                )
                .await
                .expect("exact marker is valid but incomplete"),
            CommitEffectOutcome::Unknown {
                unknown: UnknownCommit {
                    reason: UnknownCommitReason::ReadbackUnavailable,
                    diagnostic,
                }
            }
        );
        assert_eq!(
            harness
                .commits
                .requests
                .lock()
                .expect("commit requests lock")
                .len(),
            1,
            "the adapter performs one commit attempt"
        );
    }

    #[tokio::test]
    async fn recovery_advances_read_authority_without_reassigning_transaction_execution() {
        let executor = owner(1, 1, 1);
        let first_recovery = owner(2, 2, 2);
        let active_recovery = owner(3, 3, 3);
        let prepared = prepared_record(
            executing_record(command(executor.fence), executor),
            executor,
        );
        let awaiting = awaiting_record(prepared, executor);
        let probed = CommandReducer::reduce(
            &awaiting,
            CommandEvent::ObserveReconciliation {
                owner: first_recovery,
                transaction: transaction(),
                observation: ReconciliationObservation::Indeterminate {
                    evidence: evidence(0x61),
                },
            },
            context(&awaiting, first_recovery.fence),
        )
        .expect("first recovery read remains indeterminate")
        .record;
        let harness = Harness::new(
            AdministrationCommitObservation::Unknown {
                reason: UnknownCommitReason::ProcessInterrupted,
                diagnostic: diagnostic(0x62),
            },
            AdministrationMarkerObservation::Committed {
                receipt: receipt(executor.fence.generation),
                evidence: evidence(0x63),
            },
        );

        assert_eq!(
            harness
                .effect
                .reconcile(
                    &probed,
                    active_recovery,
                    transaction(),
                    context(&probed, active_recovery.fence),
                )
                .await
                .expect("complete history proves the original commit"),
            ReconciliationObservation::Committed {
                evidence: evidence(0x63),
                result: expected_result(),
            }
        );
        let read = harness
            .markers
            .requests
            .lock()
            .expect("marker requests lock")[0];
        assert_eq!(read.attempt().execution_owner(), executor);
        assert_eq!(read.active_recovery_owner(), active_recovery);
        assert_eq!(read.transaction(), transaction());
    }

    #[tokio::test]
    async fn recovery_rejects_receipt_from_a_plausible_intermediate_generation() {
        let executor = owner(1, 1, 1);
        let recovery = owner(3, 3, 3);
        let prepared = prepared_record(
            executing_record(command(executor.fence), executor),
            executor,
        );
        let awaiting = awaiting_record(prepared, executor);
        let harness = Harness::new(
            AdministrationCommitObservation::Unknown {
                reason: UnknownCommitReason::ProcessInterrupted,
                diagnostic: diagnostic(0x71),
            },
            AdministrationMarkerObservation::Committed {
                receipt: receipt(generation(2)),
                evidence: evidence(0x72),
            },
        );

        assert_eq!(
            harness
                .effect
                .reconcile(
                    &awaiting,
                    recovery,
                    transaction(),
                    context(&awaiting, recovery.fence),
                )
                .await,
            Err(CommandPortError::CorruptRecord)
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
            payload: FabricCommandPayload::Administer {
                action: AdministrationAction::ReconcileOperation,
                request: request(),
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
        .expect("admit administrative command");
        let AdmissionOutcome::New(admitted) = admitted else {
            panic!("fresh command creates one record")
        };
        CommandReducer::reduce(
            &admitted,
            CommandEvent::Start { owner },
            context(&admitted, owner.fence),
        )
        .expect("start administrative command")
        .record
    }

    fn prepared_record(executing: CommandRecord, owner: ExecutionOwner) -> CommandRecord {
        CommandReducer::reduce(
            &executing,
            CommandEvent::PrepareCommit {
                owner,
                transaction: transaction(),
            },
            context(&executing, owner.fence),
        )
        .expect("prepare administrative transaction")
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
                    diagnostic: diagnostic(0x81),
                },
            },
            context(&prepared, owner.fence),
        )
        .expect("mark interrupted administrative commit")
        .record
    }

    fn context(record: &CommandRecord, active_fence: WriterFence) -> ReductionContext {
        ReductionContext {
            current_head: record.command().expected_head,
            active_fence,
        }
    }

    fn receipt(writer_generation: WriterGeneration) -> AdministrationCommitReceipt {
        AdministrationCommitReceipt {
            workspace_id: workspace_id(),
            operation_id: operation_id(),
            transaction: transaction(),
            writer_generation,
            action: AdministrationAction::ReconcileOperation,
            request: request(),
            resulting_head: ExpectedHead::Empty,
            operation_selection: selection(),
        }
    }

    fn marker(writer_generation: WriterGeneration) -> AdministrationMarkerReceipt {
        AdministrationMarkerReceipt {
            workspace_id: workspace_id(),
            operation_id: operation_id(),
            transaction: transaction(),
            writer_generation,
        }
    }

    fn indeterminate() -> AdministrationMarkerObservation {
        AdministrationMarkerObservation::Indeterminate {
            evidence: evidence(0x90),
        }
    }

    fn expected_result() -> CommandResult {
        CommandResult::AdministrationApplied {
            request: request(),
            resulting_head: ExpectedHead::Empty,
            selection: selection(),
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
        WriterGeneration::new(value).expect("test generations are nonzero")
    }

    fn workspace_id() -> WorkspaceId {
        WorkspaceId::from_bytes([0x11; 16])
    }

    fn operation_id() -> OperationId {
        OperationId::from_bytes([0x12; 16])
    }

    fn transaction() -> TransactionRef {
        TransactionRef::from_bytes([0x13; 32])
    }

    fn request() -> AdministrationRequestRef {
        AdministrationRequestRef::from_bytes([0x14; 32])
    }

    fn selection() -> OperationSelectionRef {
        OperationSelectionRef::from_bytes([0x15; 32])
    }

    fn diagnostic(byte: u8) -> DiagnosticRef {
        DiagnosticRef::from_bytes([byte; 32])
    }

    fn evidence(byte: u8) -> ReconciliationEvidenceRef {
        ReconciliationEvidenceRef::from_bytes([byte; 32])
    }
}
