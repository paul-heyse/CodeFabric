//! Typed `PublishSourceWave` command effect and exact source-generation reconciliation seam.
//!
//! Source discovery and invalidation remain ordinary relational inputs. This module owns only
//! the static lifecycle boundary that turns one already-selected immutable source-image set into
//! a durable source generation. Resolution is read-only, commit is a single fenced application
//! transaction, and recovery begins by reading the exact operation marker and complete control
//! history. No path discovers "latest", synthesizes an image set, or retries an unknown commit.

use std::sync::Arc;

use async_trait::async_trait;

use super::command::{
    CommandCancellation, CommandFailure, CommandKind, CommandRecord, CommandResult, DiagnosticRef,
    ExecutionOwner, ExpectedHead, FabricCommand, FabricCommandPayload, OperationId,
    OperationSelectionRef, ReconciliationEvidenceRef, ReconciliationObservation, ReductionContext,
    SourceGeneration, SourceImageSetRef, TransactionRef, UnknownCommit, UnknownCommitReason,
    WorkspaceId, WriterGeneration,
};
use super::command_actor::{CommandPortError, CommitEffectOutcome, PrepareEffectOutcome};
use super::command_effect_contract::{
    ValidatedCommandAttempt, executing_attempt, prepared_attempt, reconciliation_attempt,
};
use super::command_effect_router::SourceWaveCommandEffectPort;

/// Exact immutable source-wave attempt supplied to the read-only resolver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceWaveAttempt {
    validated: ValidatedCommandAttempt,
}

impl SourceWaveAttempt {
    fn from_validated(validated: ValidatedCommandAttempt) -> Result<Self, CommandPortError> {
        if matches!(
            validated.command().payload,
            FabricCommandPayload::PublishSourceWave { .. }
        ) {
            Ok(Self { validated })
        } else {
            Err(CommandPortError::CorruptRecord)
        }
    }

    /// Immutable admitted `PublishSourceWave` command.
    #[must_use]
    pub const fn command(self) -> FabricCommand {
        self.validated.command()
    }

    /// Reducer-owned attempt number.
    #[must_use]
    pub const fn attempt(self) -> u32 {
        self.validated.attempt()
    }

    /// Actor and writer fence that owned this exact durable attempt.
    ///
    /// During reconciliation this remains the original commit owner. The newer writer that is
    /// authorized to perform the read-only recovery is carried separately on
    /// [`SourceWaveReconciliationRequest`], so a recovery fence can never be mistaken for the
    /// generation that actually committed.
    #[must_use]
    pub const fn owner(self) -> ExecutionOwner {
        self.validated.execution_owner()
    }

    /// Exact immutable source-image set selected before command admission.
    #[must_use]
    pub fn source_images(self) -> SourceImageSetRef {
        let FabricCommandPayload::PublishSourceWave { source_images, .. } = self.command().payload
        else {
            unreachable!("source-wave attempt is constructed only after payload validation")
        };
        source_images
    }

    /// Source generation selected by the admitted command.
    #[must_use]
    pub const fn prior_generation(self) -> SourceGeneration {
        self.command().pins.source_generation
    }

    /// Exact generation the source-image set will establish.
    #[must_use]
    pub fn target_generation(self) -> SourceGeneration {
        let FabricCommandPayload::PublishSourceWave {
            target_generation, ..
        } = self.command().payload
        else {
            unreachable!("source-wave attempt is constructed only after payload validation")
        };
        target_generation
    }
}

/// Fully bound result of immutable catalog/request resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedSourceWave {
    attempt: SourceWaveAttempt,
    transaction: TransactionRef,
}

impl ResolvedSourceWave {
    /// Bind one deterministic application transaction to the exact attempt.
    #[must_use]
    pub const fn new(attempt: SourceWaveAttempt, transaction: TransactionRef) -> Self {
        Self {
            attempt,
            transaction,
        }
    }

    /// Exact attempt resolved from immutable request/catalog relations.
    #[must_use]
    pub const fn attempt(self) -> SourceWaveAttempt {
        self.attempt
    }

    /// Deterministic transaction persisted before any durable target write.
    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.transaction
    }
}

/// Read-only source-wave resolution result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceWaveResolution {
    Resolved(ResolvedSourceWave),
    KnownFailure(CommandFailure),
    Cancelled(CommandCancellation),
}

/// Immutable source-image/catalog resolver.
///
/// Implementations validate that the referenced image set is complete for the requested source
/// generation and may use DataFusion relations to derive the selected transaction. They must not
/// mutate source inventory, current generation, operation markers, or any other durable target.
#[async_trait]
pub trait SourceWaveResolverPort: Send + Sync {
    async fn resolve(
        &self,
        attempt: SourceWaveAttempt,
    ) -> Result<SourceWaveResolution, CommandPortError>;
}

/// Exact request presented to the sole durable source-wave boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceWaveCommitRequest {
    attempt: SourceWaveAttempt,
    transaction: TransactionRef,
}

impl SourceWaveCommitRequest {
    /// Exact attempt authorized for this commit.
    #[must_use]
    pub const fn attempt(self) -> SourceWaveAttempt {
        self.attempt
    }

    /// Actor-persisted transaction identity.
    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.transaction
    }
}

/// Complete readback receipt for one published source generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceWaveCommitReceipt {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub transaction: TransactionRef,
    pub writer_generation: WriterGeneration,
    pub source_images: SourceImageSetRef,
    pub prior_generation: SourceGeneration,
    pub target_generation: SourceGeneration,
    pub resulting_head: ExpectedHead,
    pub operation_selection: OperationSelectionRef,
}

/// Exact marker visibility after an attempted direct commit.
///
/// A marker without complete source-generation control history is intentionally insufficient to
/// construct a successful command result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceWaveMarkerReceipt {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub transaction: TransactionRef,
    pub writer_generation: WriterGeneration,
}

/// Exhaustive observation from one no-retry source-wave transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceWaveCommitObservation {
    Committed(SourceWaveCommitReceipt),
    MarkerAlreadyCommitted {
        marker: SourceWaveMarkerReceipt,
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

/// Sole durable source-wave authority.
///
/// The implementation atomically binds the exact image set, prior/target generations,
/// application operation, transaction, and execution writer generation. It performs one
/// controlled attempt and must not rebase, retry, rediscover latest state, or reduce command
/// state.
#[async_trait]
pub trait SourceWaveCommitPort: Send + Sync {
    async fn commit(
        &self,
        request: SourceWaveCommitRequest,
    ) -> Result<SourceWaveCommitObservation, CommandPortError>;
}

/// Exact read-only reconciliation key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceWaveReconciliationRequest {
    attempt: SourceWaveAttempt,
    active_recovery_owner: ExecutionOwner,
    transaction: TransactionRef,
}

impl SourceWaveReconciliationRequest {
    /// Original command attempt and the writer fence that owned its prepared commit.
    #[must_use]
    pub const fn attempt(self) -> SourceWaveAttempt {
        self.attempt
    }

    /// Current actor/fence authorized to read and reconcile the old attempt.
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

/// Exact result of reading the application marker and complete source-wave control history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceWaveMarkerObservation {
    Committed {
        receipt: SourceWaveCommitReceipt,
        evidence: ReconciliationEvidenceRef,
    },
    ProvedNotCommitted {
        evidence: ReconciliationEvidenceRef,
    },
    Indeterminate {
        evidence: ReconciliationEvidenceRef,
    },
}

/// Read-only application-marker and source-generation control-history authority.
///
/// Implementations fence the read with [`SourceWaveReconciliationRequest::active_recovery_owner`]
/// while querying the exact transaction and original
/// [`SourceWaveAttempt::owner`] generation. They must not infer a result from a current/latest
/// generation lookup or write source state while reconciling.
#[async_trait]
pub trait SourceWaveMarkerPort: Send + Sync {
    async fn read_exact(
        &self,
        request: SourceWaveReconciliationRequest,
    ) -> Result<SourceWaveMarkerObservation, CommandPortError>;
}

/// Concrete typed effect for `FabricCommandPayload::PublishSourceWave`.
pub struct SourceWaveCommandEffect {
    resolver: Arc<dyn SourceWaveResolverPort>,
    commits: Arc<dyn SourceWaveCommitPort>,
    markers: Arc<dyn SourceWaveMarkerPort>,
}

impl std::fmt::Debug for SourceWaveCommandEffect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceWaveCommandEffect")
            .field("resolver", &"installed")
            .field("commits", &"installed")
            .field("markers", &"installed")
            .finish()
    }
}

impl SourceWaveCommandEffect {
    /// Install all independent source-wave authorities.
    #[must_use]
    pub const fn new(
        resolver: Arc<dyn SourceWaveResolverPort>,
        commits: Arc<dyn SourceWaveCommitPort>,
        markers: Arc<dyn SourceWaveMarkerPort>,
    ) -> Self {
        Self {
            resolver,
            commits,
            markers,
        }
    }
}

#[async_trait]
impl SourceWaveCommandEffectPort for SourceWaveCommandEffect {
    async fn prepare(
        &self,
        executing: &CommandRecord,
        owner: ExecutionOwner,
        context: ReductionContext,
    ) -> Result<PrepareEffectOutcome, CommandPortError> {
        let validated =
            executing_attempt(executing, owner, context, CommandKind::PublishSourceWave)?;
        let attempt = SourceWaveAttempt::from_validated(validated)?;
        match self.resolver.resolve(attempt).await? {
            SourceWaveResolution::Resolved(resolved) => {
                if resolved.attempt() != attempt {
                    return Err(CommandPortError::CorruptRecord);
                }
                Ok(PrepareEffectOutcome::Prepared {
                    transaction: resolved.transaction(),
                })
            }
            SourceWaveResolution::KnownFailure(failure) => {
                Ok(PrepareEffectOutcome::KnownFailure { failure })
            }
            SourceWaveResolution::Cancelled(cancellation) => {
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
            CommandKind::PublishSourceWave,
        )?;
        let attempt = SourceWaveAttempt::from_validated(validated)?;
        match self
            .commits
            .commit(SourceWaveCommitRequest {
                attempt,
                transaction,
            })
            .await?
        {
            SourceWaveCommitObservation::Committed(receipt) => {
                validate_commit_receipt(attempt, transaction, receipt)?;
                Ok(CommitEffectOutcome::Committed {
                    result: result_from_receipt(receipt),
                })
            }
            SourceWaveCommitObservation::MarkerAlreadyCommitted { marker, diagnostic } => {
                validate_direct_marker(attempt, transaction, marker)?;
                Ok(unknown(
                    UnknownCommitReason::ReadbackUnavailable,
                    diagnostic,
                ))
            }
            SourceWaveCommitObservation::Conflict { diagnostic } => Ok(unknown(
                UnknownCommitReason::ReadbackUnavailable,
                diagnostic,
            )),
            SourceWaveCommitObservation::Unknown { reason, diagnostic } => {
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
            CommandKind::PublishSourceWave,
        )?;
        let attempt = SourceWaveAttempt::from_validated(recovery.attempt())?;
        let request = SourceWaveReconciliationRequest {
            attempt,
            active_recovery_owner: recovery.active_recovery_owner(),
            transaction,
        };
        match self.markers.read_exact(request).await? {
            SourceWaveMarkerObservation::Committed { receipt, evidence } => {
                validate_commit_receipt(attempt, transaction, receipt)?;
                Ok(ReconciliationObservation::Committed {
                    evidence,
                    result: result_from_receipt(receipt),
                })
            }
            SourceWaveMarkerObservation::ProvedNotCommitted { evidence } => {
                Ok(ReconciliationObservation::NotCommitted { evidence })
            }
            SourceWaveMarkerObservation::Indeterminate { evidence } => {
                Ok(ReconciliationObservation::Indeterminate { evidence })
            }
        }
    }
}

fn validate_commit_receipt(
    attempt: SourceWaveAttempt,
    transaction: TransactionRef,
    receipt: SourceWaveCommitReceipt,
) -> Result<(), CommandPortError> {
    validate_receipt_identity(attempt, transaction, receipt)?;
    attempt
        .validated
        .validate_receipt_generation(receipt.writer_generation)
}

fn validate_receipt_identity(
    attempt: SourceWaveAttempt,
    transaction: TransactionRef,
    receipt: SourceWaveCommitReceipt,
) -> Result<(), CommandPortError> {
    let command = attempt.command();
    if receipt.workspace_id != command.ownership.workspace_id
        || receipt.operation_id != command.identity.operation_id
        || receipt.transaction != transaction
        || receipt.source_images != attempt.source_images()
        || receipt.prior_generation != attempt.prior_generation()
        || receipt.target_generation != attempt.target_generation()
        || receipt.resulting_head != command.expected_head
    {
        return Err(CommandPortError::CorruptRecord);
    }
    Ok(())
}

fn validate_direct_marker(
    attempt: SourceWaveAttempt,
    transaction: TransactionRef,
    marker: SourceWaveMarkerReceipt,
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

const fn result_from_receipt(receipt: SourceWaveCommitReceipt) -> CommandResult {
    CommandResult::SourceWavePublished {
        source_generation: receipt.target_generation,
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
        ResourceEnvelopeRef, WriterFence,
    };

    struct ResolverProbe {
        transaction: TransactionRef,
        requests: Mutex<Vec<SourceWaveAttempt>>,
        resolved_attempt: Mutex<Option<SourceWaveAttempt>>,
    }

    #[async_trait]
    impl SourceWaveResolverPort for ResolverProbe {
        async fn resolve(
            &self,
            attempt: SourceWaveAttempt,
        ) -> Result<SourceWaveResolution, CommandPortError> {
            self.requests
                .lock()
                .expect("resolver request lock")
                .push(attempt);
            let resolved_attempt = self
                .resolved_attempt
                .lock()
                .expect("resolved-attempt lock")
                .unwrap_or(attempt);
            Ok(SourceWaveResolution::Resolved(ResolvedSourceWave::new(
                resolved_attempt,
                self.transaction,
            )))
        }
    }

    struct CommitProbe {
        observation: SourceWaveCommitObservation,
        requests: Mutex<Vec<SourceWaveCommitRequest>>,
    }

    #[async_trait]
    impl SourceWaveCommitPort for CommitProbe {
        async fn commit(
            &self,
            request: SourceWaveCommitRequest,
        ) -> Result<SourceWaveCommitObservation, CommandPortError> {
            self.requests
                .lock()
                .expect("commit request lock")
                .push(request);
            Ok(self.observation)
        }
    }

    struct MarkerProbe {
        observation: SourceWaveMarkerObservation,
        requests: Mutex<Vec<SourceWaveReconciliationRequest>>,
    }

    #[async_trait]
    impl SourceWaveMarkerPort for MarkerProbe {
        async fn read_exact(
            &self,
            request: SourceWaveReconciliationRequest,
        ) -> Result<SourceWaveMarkerObservation, CommandPortError> {
            self.requests
                .lock()
                .expect("marker request lock")
                .push(request);
            Ok(self.observation)
        }
    }

    struct Harness {
        effect: SourceWaveCommandEffect,
        resolver: Arc<ResolverProbe>,
        commits: Arc<CommitProbe>,
        markers: Arc<MarkerProbe>,
    }

    impl Harness {
        fn new(commit: SourceWaveCommitObservation, marker: SourceWaveMarkerObservation) -> Self {
            let resolver = Arc::new(ResolverProbe {
                transaction: transaction(),
                requests: Mutex::new(Vec::new()),
                resolved_attempt: Mutex::new(None),
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
                SourceWaveCommandEffect::new(resolver.clone(), commits.clone(), markers.clone());
            Self {
                effect,
                resolver,
                commits,
                markers,
            }
        }
    }

    #[tokio::test]
    async fn resolver_cannot_substitute_another_source_wave_binding() {
        let owner = owner(1, 1, 1);
        let executing = executing_record(command(owner.fence), owner);
        let harness = Harness::new(
            SourceWaveCommitObservation::Committed(receipt(
                owner.fence.generation,
                SourceGeneration::new(8),
            )),
            indeterminate_marker(),
        );
        let mut substituted = command(owner.fence);
        substituted.payload = FabricCommandPayload::PublishSourceWave {
            source_images: SourceImageSetRef::from_bytes([0x70; 32]),
            target_generation: SourceGeneration::new(9),
        };
        let substituted_executing = executing_record(substituted, owner);
        let substituted_attempt = SourceWaveAttempt::from_validated(
            executing_attempt(
                &substituted_executing,
                owner,
                context(owner),
                CommandKind::PublishSourceWave,
            )
            .expect("validate substituted source-wave attempt"),
        )
        .expect("typed substituted source-wave attempt");
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
                .expect("commit request lock")
                .is_empty(),
            "a drifted relational resolution cannot reach the durable boundary"
        );
    }

    #[tokio::test]
    async fn prepare_is_read_only_and_exact_commit_publishes_selected_generation() {
        let owner = owner(1, 1, 1);
        let executing = executing_record(command(owner.fence), owner);
        let harness = Harness::new(
            SourceWaveCommitObservation::Committed(receipt(
                owner.fence.generation,
                SourceGeneration::new(8),
            )),
            indeterminate_marker(),
        );

        assert_eq!(
            harness
                .effect
                .prepare(&executing, owner, context(owner))
                .await
                .expect("read-only source-wave resolution"),
            PrepareEffectOutcome::Prepared {
                transaction: transaction()
            }
        );
        assert!(
            harness
                .commits
                .requests
                .lock()
                .expect("commit request lock")
                .is_empty(),
            "prepare cannot cross the durable commit port"
        );

        let prepared = prepared_record(executing, owner);
        assert_eq!(
            harness
                .effect
                .commit(&prepared, owner, transaction(), context(owner))
                .await
                .expect("exact source-wave readback"),
            CommitEffectOutcome::Committed {
                result: expected_result()
            }
        );
        assert_eq!(
            harness
                .resolver
                .requests
                .lock()
                .expect("resolver request lock")[0]
                .source_images(),
            source_images()
        );
        assert_eq!(
            harness
                .commits
                .requests
                .lock()
                .expect("commit request lock")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn mismatched_source_bindings_cannot_confirm_commit() {
        let owner = owner(1, 1, 1);
        let mut wrong_images = receipt(owner.fence.generation, SourceGeneration::new(8));
        wrong_images.source_images = SourceImageSetRef::from_bytes([0x71; 32]);
        let mut wrong_prior = receipt(owner.fence.generation, SourceGeneration::new(8));
        wrong_prior.prior_generation = SourceGeneration::new(6);
        let mut wrong_generation = receipt(owner.fence.generation, SourceGeneration::new(9));
        wrong_generation.source_images = source_images();

        for invalid in [wrong_images, wrong_prior, wrong_generation] {
            let harness = Harness::new(
                SourceWaveCommitObservation::Committed(invalid),
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
            SourceWaveCommitObservation::MarkerAlreadyCommitted {
                marker: marker_receipt(owner.fence.generation),
                diagnostic: diagnostic(61),
            },
            SourceWaveCommitObservation::Conflict {
                diagnostic: diagnostic(62),
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
                    .expect("ambiguous commit maps to unknown"),
                CommitEffectOutcome::Unknown { .. }
            ));
            assert_eq!(
                harness
                    .commits
                    .requests
                    .lock()
                    .expect("commit request lock")
                    .len(),
                1,
                "effect performs no internal retry"
            );
        }
    }

    #[tokio::test]
    async fn recovery_reads_original_transaction_under_new_active_fence() {
        let original = owner(1, 1, 1);
        let recovery = owner(2, 2, 2);
        let awaiting = awaiting_record(
            prepared_record(
                executing_record(command(original.fence), original),
                original,
            ),
            original,
        );
        let evidence = ReconciliationEvidenceRef::from_bytes([0x72; 32]);
        let harness = Harness::new(
            SourceWaveCommitObservation::Unknown {
                reason: UnknownCommitReason::ProcessInterrupted,
                diagnostic: diagnostic(63),
            },
            SourceWaveMarkerObservation::Committed {
                receipt: receipt(original.fence.generation, SourceGeneration::new(8)),
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
                .expect("old-generation commit is exactly proved"),
            ReconciliationObservation::Committed {
                evidence,
                result: expected_result(),
            }
        );
        let request = harness
            .markers
            .requests
            .lock()
            .expect("marker request lock")[0];
        assert_eq!(request.transaction(), transaction());
        assert_eq!(request.attempt().owner(), original);
        assert_eq!(request.active_recovery_owner(), recovery);
        assert_eq!(request.attempt().source_images(), source_images());
    }

    #[tokio::test]
    async fn recovery_rejects_a_generation_that_did_not_own_the_prepared_attempt() {
        let admitted = owner(1, 1, 1);
        let execution = owner(2, 2, 2);
        let unrelated_intermediate = owner(3, 3, 3);
        let recovery = owner(4, 4, 4);
        let awaiting = awaiting_record(
            prepared_record(
                executing_record(command(admitted.fence), execution),
                execution,
            ),
            execution,
        );
        let harness = Harness::new(
            SourceWaveCommitObservation::Unknown {
                reason: UnknownCommitReason::ProcessInterrupted,
                diagnostic: diagnostic(65),
            },
            SourceWaveMarkerObservation::Committed {
                receipt: receipt(
                    unrelated_intermediate.fence.generation,
                    SourceGeneration::new(8),
                ),
                evidence: ReconciliationEvidenceRef::from_bytes([0x75; 32]),
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
            Err(CommandPortError::CorruptRecord),
            "a generation numerically between admission and recovery is not the commit owner"
        );
        let request = harness
            .markers
            .requests
            .lock()
            .expect("marker request lock")[0];
        assert_eq!(request.attempt().owner(), execution);
        assert_eq!(request.active_recovery_owner(), recovery);
    }

    #[tokio::test]
    async fn recovery_requires_the_exact_owner_or_a_strictly_newer_fence() {
        let original = owner(1, 1, 2);
        let equal_generation_intruder = owner(2, 2, 2);
        let awaiting = awaiting_record(
            prepared_record(
                executing_record(command(original.fence), original),
                original,
            ),
            original,
        );
        let harness = Harness::new(
            SourceWaveCommitObservation::Unknown {
                reason: UnknownCommitReason::ProcessInterrupted,
                diagnostic: diagnostic(66),
            },
            indeterminate_marker(),
        );

        assert_eq!(
            harness
                .effect
                .reconcile(
                    &awaiting,
                    equal_generation_intruder,
                    transaction(),
                    ReductionContext {
                        current_head: ExpectedHead::Empty,
                        active_fence: equal_generation_intruder.fence,
                    },
                )
                .await,
            Err(CommandPortError::ContextUnavailable)
        );
        assert!(
            harness
                .markers
                .requests
                .lock()
                .expect("marker request lock")
                .is_empty(),
            "an unauthorized recovery owner must not reach the marker authority"
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
            payload: FabricCommandPayload::PublishSourceWave {
                source_images: source_images(),
                target_generation: SourceGeneration::new(8),
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
        .expect("admit source-wave command");
        let AdmissionOutcome::New(admitted) = admitted else {
            panic!("empty admission creates a record")
        };
        CommandReducer::reduce(&admitted, CommandEvent::Start { owner }, context(owner))
            .expect("start source-wave command")
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
        .expect("prepare source-wave transaction")
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
                    diagnostic: diagnostic(64),
                },
            },
            context(owner),
        )
        .expect("record unknown source-wave commit")
        .record
    }

    fn receipt(
        writer_generation: WriterGeneration,
        target_generation: SourceGeneration,
    ) -> SourceWaveCommitReceipt {
        SourceWaveCommitReceipt {
            workspace_id: workspace_id(),
            operation_id: operation_id(),
            transaction: transaction(),
            writer_generation,
            source_images: source_images(),
            prior_generation: SourceGeneration::new(7),
            target_generation,
            resulting_head: ExpectedHead::Empty,
            operation_selection: selection(),
        }
    }

    fn marker_receipt(writer_generation: WriterGeneration) -> SourceWaveMarkerReceipt {
        SourceWaveMarkerReceipt {
            workspace_id: workspace_id(),
            operation_id: operation_id(),
            transaction: transaction(),
            writer_generation,
        }
    }

    fn indeterminate_marker() -> SourceWaveMarkerObservation {
        SourceWaveMarkerObservation::Indeterminate {
            evidence: ReconciliationEvidenceRef::from_bytes([0x73; 32]),
        }
    }

    fn expected_result() -> CommandResult {
        CommandResult::SourceWavePublished {
            source_generation: SourceGeneration::new(8),
            resulting_head: ExpectedHead::Empty,
            selection: selection(),
        }
    }

    fn owner(actor: u8, lease: u8, generation: u64) -> ExecutionOwner {
        ExecutionOwner {
            actor_id: ActorId::from_bytes([actor; 16]),
            fence: WriterFence {
                lease_id: LeaseId::from_bytes([lease; 16]),
                generation: WriterGeneration::new(generation).expect("nonzero generation"),
            },
        }
    }

    fn context(owner: ExecutionOwner) -> ReductionContext {
        ReductionContext {
            current_head: ExpectedHead::Empty,
            active_fence: owner.fence,
        }
    }

    fn workspace_id() -> WorkspaceId {
        WorkspaceId::from_bytes([0x11; 16])
    }

    fn operation_id() -> OperationId {
        OperationId::from_bytes([0x12; 16])
    }

    fn source_images() -> SourceImageSetRef {
        SourceImageSetRef::from_bytes([0x13; 32])
    }

    fn transaction() -> TransactionRef {
        TransactionRef::from_bytes([0x14; 32])
    }

    fn selection() -> OperationSelectionRef {
        OperationSelectionRef::from_bytes([0x15; 32])
    }

    fn diagnostic(value: u8) -> DiagnosticRef {
        DiagnosticRef::from_bytes([value; 32])
    }
}
