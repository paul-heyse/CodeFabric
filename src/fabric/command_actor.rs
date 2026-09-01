//! Bounded Tokio actor for the durable [`super::command::FabricCommand`] reducer.
//!
//! The actor owns ordering, not semantic current state. Application-owned ports provide durable
//! command-record compare-and-swap, fresh authoritative head/fence reads, and typed effects. The
//! pure reducer remains the only constructor of successor command states.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use super::command::{
    ActorId, AdmissionContext, AdmissionOutcome, AuthorizationDecision, CommandCancellation,
    CommandContractError, CommandEvent, CommandFailure, CommandRecord, CommandRecoveryObligation,
    CommandReducer, CommandResult, DurableCommandState, ExecutionOwner, FabricCommand,
    IdempotencyKey, OperationId, ReconciliationObservation, ReducerTransition, ReductionContext,
    ReductionEffect, TransactionRef, UnknownCommit,
};

/// Bounded command queue configuration. Zero-capacity or implicit unbounded queues are invalid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FabricCommandActorConfig {
    queue_capacity: NonZeroUsize,
}

impl FabricCommandActorConfig {
    #[must_use]
    pub const fn new(queue_capacity: NonZeroUsize) -> Self {
        Self { queue_capacity }
    }

    #[must_use]
    pub const fn queue_capacity(self) -> usize {
        self.queue_capacity.get()
    }
}

impl Default for FabricCommandActorConfig {
    fn default() -> Self {
        Self {
            queue_capacity: NonZeroUsize::new(64).expect("64 is nonzero"),
        }
    }
}

/// Shared fail-closed admission state between the runtime boundary and serial actor.
pub(super) struct FabricCommandIngressGate {
    open: AtomicBool,
}

impl FabricCommandIngressGate {
    pub(super) const fn closed() -> Self {
        Self {
            open: AtomicBool::new(false),
        }
    }

    #[cfg(test)]
    fn open_for_tests() -> Self {
        Self {
            open: AtomicBool::new(true),
        }
    }

    pub(super) fn is_open(&self) -> bool {
        self.open.load(Ordering::Acquire)
    }

    pub(super) fn open(&self) {
        self.open.store(true, Ordering::Release);
    }

    pub(super) fn close(&self) {
        self.open.store(false, Ordering::Release);
    }
}

/// Closed infrastructure failures exposed by application-owned actor ports.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CommandPortError {
    #[error("durable command storage is unavailable")]
    DurableStoreUnavailable,
    #[error("durable command storage contains a contradictory record")]
    CorruptRecord,
    #[error("authoritative command context is unavailable")]
    ContextUnavailable,
    #[error("effect executor is unavailable before producing a typed outcome")]
    EffectUnavailable,
}

/// Atomic insert outcome for revision-zero admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableAdmissionWrite {
    Inserted(CommandRecord),
    Existing(CommandRecord),
}

/// Durable compare-and-swap outcome for one reducer transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableTransitionWrite {
    Stored(CommandRecord),
    RevisionConflict { observed: Option<CommandRecord> },
}

/// Temporal command-record persistence. It stores reducer products but owns no semantic head.
#[async_trait]
pub trait DurableCommandRecordPort: Send + Sync {
    /// Lookup by both unique admission keys in one durable operation.
    async fn lookup_admission(
        &self,
        operation_id: OperationId,
        idempotency_key: IdempotencyKey,
    ) -> Result<Option<CommandRecord>, CommandPortError>;

    /// Lookup one already admitted command by operation identity.
    async fn lookup_operation(
        &self,
        operation_id: OperationId,
    ) -> Result<Option<CommandRecord>, CommandPortError>;

    /// Atomically insert revision zero or return the record that won admission.
    async fn insert_if_absent(
        &self,
        admitted: CommandRecord,
    ) -> Result<DurableAdmissionWrite, CommandPortError>;

    /// Persist exactly one reducer transition against its predecessor revision.
    async fn compare_and_swap(
        &self,
        transition: ReducerTransition,
    ) -> Result<DurableTransitionWrite, CommandPortError>;

    /// Return the first canonical nonterminal record, ordered by operation identity.
    async fn first_nonterminal(&self) -> Result<Option<CommandRecord>, CommandPortError>;
}

/// Authoritative head, policy, lease, and writer-generation reader.
///
/// Implementations reread their source for every call. A cached SQLite pointer is not a valid
/// semantic-current implementation of this port.
#[async_trait]
pub trait CommandReductionContextPort: Send + Sync {
    async fn read_admission_context(
        &self,
        command: &FabricCommand,
    ) -> Result<AdmissionContext, CommandPortError>;

    async fn read_reduction_context(
        &self,
        record: &CommandRecord,
    ) -> Result<ReductionContext, CommandPortError>;
}

/// Typed transaction-preparation result. `Prepared` identifies the exact durable transaction
/// marker that the actor will persist before invoking [`FabricCommandEffectPort::commit`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrepareEffectOutcome {
    Prepared { transaction: TransactionRef },
    KnownFailure { failure: CommandFailure },
    Cancelled { cancellation: CommandCancellation },
}

/// Typed commit result. Unknown is intentionally distinct from a known failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitEffectOutcome {
    Committed { result: CommandResult },
    KnownFailure { failure: CommandFailure },
    Unknown { unknown: UnknownCommit },
}

/// Irreducible command effects.
///
/// `prepare` may validate inputs, reserve bounded in-memory resources, and deterministically
/// derive the transaction identity, but it must not write any durable target. The actor persists
/// `CommitPrepared` before calling `commit`, which is the sole effect method allowed to initiate
/// the command's target transaction. Implementations must fence every durable boundary with the
/// supplied owner/context and may return `KnownFailure` only when no commit occurred.
#[async_trait]
pub trait FabricCommandEffectPort: Send + Sync {
    async fn prepare(
        &self,
        executing: &CommandRecord,
        owner: ExecutionOwner,
        context: ReductionContext,
    ) -> Result<PrepareEffectOutcome, CommandPortError>;

    async fn commit(
        &self,
        prepared: &CommandRecord,
        owner: ExecutionOwner,
        transaction: TransactionRef,
        context: ReductionContext,
    ) -> Result<CommitEffectOutcome, CommandPortError>;

    /// Read the exact application transaction marker and durable control history first.
    ///
    /// This operation must never initiate, retry, rebase, or replace the original target
    /// transaction. [`DurableCommandState::AwaitingReconciliation`] preserves the immutable
    /// transaction `execution_owner` separately from its advancing `recovery_owner`; receipts
    /// bind to the former while every recovery read and convergence action is fenced by the
    /// supplied active `owner`/`context`. After exact marker/history proof, this method may
    /// idempotently converge temporal recovery bookkeeping and already-selected
    /// process/cache/acknowledgement projections. Those actions must remain causally bound to the
    /// proved transaction and active recovery fence. Return `Indeterminate` whenever the backend
    /// cannot prove either the exact committed result or non-commit.
    async fn reconcile(
        &self,
        awaiting: &CommandRecord,
        owner: ExecutionOwner,
        transaction: TransactionRef,
        context: ReductionContext,
    ) -> Result<ReconciliationObservation, CommandPortError>;
}

/// Actor-level failures. Reducer errors remain typed and are never mapped to guessed retries.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum FabricCommandActorError {
    #[error(transparent)]
    Contract(#[from] CommandContractError),
    #[error(transparent)]
    Port(#[from] CommandPortError),
    #[error("durable insert returned a record other than the admitted reducer product")]
    DurableAdmissionMismatch,
    #[error("durable CAS returned a record other than the reducer successor")]
    DurableTransitionMismatch,
    #[error("durable command revision conflict: expected {expected}, observed {observed:?}")]
    RevisionConflict {
        expected: u64,
        observed: Option<u64>,
    },
    #[error("command actor queue is closed")]
    QueueClosed,
    #[error("command actor dropped a response")]
    ResponseDropped,
    #[error("command operation was not found")]
    OperationNotFound,
    #[error("effect execution was requested from an unexpected durable state")]
    EffectStateMismatch,
    #[error("new command admission is closed until durable recovery completes")]
    AdmissionClosedForRecovery,
    #[error("command recovery remains pending for operation {operation_id:?}: {obligation:?}")]
    RecoveryPending {
        operation_id: OperationId,
        obligation: CommandRecoveryObligation,
    },
}

/// Nonblocking admission result for explicit bounded-queue backpressure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum FabricCommandBackpressure {
    #[error("new command admission is closed until durable recovery completes")]
    RecoveryPending,
    #[error("command actor queue is full")]
    Full,
    #[error("command actor queue is closed")]
    Closed,
}

/// Awaitable response created by [`FabricCommandActorHandle::try_submit`].
pub struct PendingFabricCommand {
    receiver: oneshot::Receiver<Result<CommandRecord, FabricCommandActorError>>,
}

impl PendingFabricCommand {
    /// Wait for the serialized actor result.
    ///
    /// # Errors
    ///
    /// Returns the command result or an explicit dropped-response error.
    pub async fn wait(self) -> Result<CommandRecord, FabricCommandActorError> {
        self.receiver
            .await
            .map_err(|_| FabricCommandActorError::ResponseDropped)?
    }
}

/// Cloneable bounded ingress handle. Clones share one serial actor queue.
#[derive(Clone)]
pub struct FabricCommandActorHandle {
    sender: mpsc::Sender<ActorMessage>,
    ingress_gate: Arc<FabricCommandIngressGate>,
}

impl FabricCommandActorHandle {
    /// Submit with asynchronous bounded backpressure.
    ///
    /// # Errors
    ///
    /// Returns actor/port/reducer failures or `QueueClosed`.
    pub async fn submit(
        &self,
        command: FabricCommand,
    ) -> Result<CommandRecord, FabricCommandActorError> {
        if !self.ingress_gate.is_open() {
            return Err(FabricCommandActorError::AdmissionClosedForRecovery);
        }
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(ActorMessage::Submit { command, response })
            .await
            .map_err(|_| FabricCommandActorError::QueueClosed)?;
        receiver
            .await
            .map_err(|_| FabricCommandActorError::ResponseDropped)?
    }

    /// Submit without waiting for queue capacity.
    ///
    /// # Errors
    ///
    /// Returns explicit full/closed backpressure without losing the command implicitly.
    pub fn try_submit(
        &self,
        command: FabricCommand,
    ) -> Result<PendingFabricCommand, FabricCommandBackpressure> {
        if !self.ingress_gate.is_open() {
            return Err(FabricCommandBackpressure::RecoveryPending);
        }
        let (response, receiver) = oneshot::channel();
        self.sender
            .try_send(ActorMessage::Submit { command, response })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => FabricCommandBackpressure::Full,
                mpsc::error::TrySendError::Closed(_) => FabricCommandBackpressure::Closed,
            })?;
        Ok(PendingFabricCommand { receiver })
    }

    /// Request cancellation through the same serialized durable path.
    ///
    /// The reducer accepts this only in `Admitted` or `Executing`; a prepared commit therefore
    /// cannot be cancelled.
    pub async fn cancel(
        &self,
        operation_id: OperationId,
        cancellation: CommandCancellation,
    ) -> Result<CommandRecord, FabricCommandActorError> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(ActorMessage::Cancel {
                operation_id,
                cancellation,
                response,
            })
            .await
            .map_err(|_| FabricCommandActorError::QueueClosed)?;
        receiver
            .await
            .map_err(|_| FabricCommandActorError::ResponseDropped)?
    }

    /// Request exact operation-marker/control-history readback through the serial actor.
    ///
    /// The actor, not its caller, obtains the typed observation from the effect-owned marker port.
    /// Reconciliation never executes a new target transaction; after exact proof it may converge
    /// idempotent temporal/cache/acknowledgement projections for that already-selected
    /// transaction. A committed observation terminalizes the existing transaction, a
    /// not-committed observation makes the same command retry-eligible, and an indeterminate
    /// observation remains durably awaiting more evidence.
    pub async fn reconcile(
        &self,
        operation_id: OperationId,
    ) -> Result<CommandRecord, FabricCommandActorError> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(ActorMessage::Reconcile {
                operation_id,
                response,
            })
            .await
            .map_err(|_| FabricCommandActorError::QueueClosed)?;
        receiver
            .await
            .map_err(|_| FabricCommandActorError::ResponseDropped)?
    }

    /// Re-enter effects only after the reducer recorded proof that no commit occurred.
    ///
    /// This operation accepts only `RetryReady`; an admitted, executing, prepared, unknown, or
    /// terminal command cannot use it. The actor rereads the authoritative fence before the
    /// retry transition and durably stores the new attempt before invoking an effect.
    pub async fn retry_proved_not_committed(
        &self,
        operation_id: OperationId,
    ) -> Result<CommandRecord, FabricCommandActorError> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(ActorMessage::Retry {
                operation_id,
                response,
            })
            .await
            .map_err(|_| FabricCommandActorError::QueueClosed)?;
        receiver
            .await
            .map_err(|_| FabricCommandActorError::ResponseDropped)?
    }

    /// Resume an admitted or pre-commit executing command under the active fenced writer.
    ///
    /// Recovery is safe at these states because `prepare` is prohibited from writing durable
    /// targets. A newer writer generation is durably adopted before transaction preparation is
    /// invoked again. Prepared or unknown commits must use the reconciliation methods instead.
    ///
    /// # Errors
    ///
    /// Returns an explicit state, context, reducer, or durable-store failure. It never resumes a
    /// prepared/unknown/terminal command.
    pub async fn resume_precommit(
        &self,
        operation_id: OperationId,
    ) -> Result<CommandRecord, FabricCommandActorError> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(ActorMessage::ResumePrecommit {
                operation_id,
                response,
            })
            .await
            .map_err(|_| FabricCommandActorError::QueueClosed)?;
        receiver
            .await
            .map_err(|_| FabricCommandActorError::ResponseDropped)?
    }

    /// Convert a commit prepared by an interrupted writer into durable unknown state.
    ///
    /// This operation executes no target effect. It adopts recovery ownership only under a newer
    /// active writer generation and preserves the exact prepared transaction for marker readback.
    ///
    /// # Errors
    ///
    /// Returns an explicit state, context, reducer, or durable-store failure.
    pub(super) async fn mark_interrupted_commit(
        &self,
        operation_id: OperationId,
        unknown: UnknownCommit,
    ) -> Result<CommandRecord, FabricCommandActorError> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(ActorMessage::MarkInterruptedCommit {
                operation_id,
                unknown,
                response,
            })
            .await
            .map_err(|_| FabricCommandActorError::QueueClosed)?;
        receiver
            .await
            .map_err(|_| FabricCommandActorError::ResponseDropped)?
    }

    /// Atomically prove no nonterminal record remains and reopen submit admission in actor order.
    pub(super) async fn open_admission_after_recovery(
        &self,
    ) -> Result<(), FabricCommandActorError> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(ActorMessage::OpenAdmission { response })
            .await
            .map_err(|_| FabricCommandActorError::QueueClosed)?;
        receiver
            .await
            .map_err(|_| FabricCommandActorError::ResponseDropped)?
    }

    /// Stop the serial actor after all messages ordered before this request have been handled.
    ///
    /// Any cloned ingress handles become closed when the actor acknowledges shutdown. This is a
    /// lifecycle operation only; it never manufactures terminal command records for interrupted
    /// backend work.
    ///
    /// # Errors
    ///
    /// Returns an explicit closed/dropped response when the actor is already unavailable.
    pub(super) async fn shutdown(&self) -> Result<(), FabricCommandActorError> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(ActorMessage::Shutdown { response })
            .await
            .map_err(|_| FabricCommandActorError::QueueClosed)?;
        receiver
            .await
            .map_err(|_| FabricCommandActorError::ResponseDropped)
    }
}

enum ActorMessage {
    Submit {
        command: FabricCommand,
        response: oneshot::Sender<Result<CommandRecord, FabricCommandActorError>>,
    },
    Cancel {
        operation_id: OperationId,
        cancellation: CommandCancellation,
        response: oneshot::Sender<Result<CommandRecord, FabricCommandActorError>>,
    },
    Reconcile {
        operation_id: OperationId,
        response: oneshot::Sender<Result<CommandRecord, FabricCommandActorError>>,
    },
    Retry {
        operation_id: OperationId,
        response: oneshot::Sender<Result<CommandRecord, FabricCommandActorError>>,
    },
    ResumePrecommit {
        operation_id: OperationId,
        response: oneshot::Sender<Result<CommandRecord, FabricCommandActorError>>,
    },
    MarkInterruptedCommit {
        operation_id: OperationId,
        unknown: UnknownCommit,
        response: oneshot::Sender<Result<CommandRecord, FabricCommandActorError>>,
    },
    OpenAdmission {
        response: oneshot::Sender<Result<(), FabricCommandActorError>>,
    },
    Shutdown {
        response: oneshot::Sender<()>,
    },
}

struct FabricCommandActor {
    actor_id: ActorId,
    records: Arc<dyn DurableCommandRecordPort>,
    contexts: Arc<dyn CommandReductionContextPort>,
    effects: Arc<dyn FabricCommandEffectPort>,
    ingress_gate: Arc<FabricCommandIngressGate>,
    receiver: mpsc::Receiver<ActorMessage>,
}

impl FabricCommandActor {
    async fn run(mut self) {
        while let Some(message) = self.receiver.recv().await {
            match message {
                ActorMessage::Submit { command, response } => {
                    let result = if self.ingress_gate.is_open() {
                        self.process_submit(command).await
                    } else {
                        Err(FabricCommandActorError::AdmissionClosedForRecovery)
                    };
                    let recovery_required = match &result {
                        Ok(record) => !record.state().is_terminal(),
                        Err(_) => true,
                    };
                    if recovery_required {
                        self.ingress_gate.close();
                    }
                    let _ = response.send(result);
                }
                ActorMessage::Cancel {
                    operation_id,
                    cancellation,
                    response,
                } => {
                    let _ = response.send(self.process_cancel(operation_id, cancellation).await);
                }
                ActorMessage::Reconcile {
                    operation_id,
                    response,
                } => {
                    let _ = response.send(self.process_reconcile(operation_id).await);
                }
                ActorMessage::Retry {
                    operation_id,
                    response,
                } => {
                    let _ = response.send(self.process_retry(operation_id).await);
                }
                ActorMessage::ResumePrecommit {
                    operation_id,
                    response,
                } => {
                    let _ = response.send(self.process_resume_precommit(operation_id).await);
                }
                ActorMessage::MarkInterruptedCommit {
                    operation_id,
                    unknown,
                    response,
                } => {
                    let _ = response.send(
                        self.process_mark_interrupted_commit(operation_id, unknown)
                            .await,
                    );
                }
                ActorMessage::OpenAdmission { response } => {
                    self.ingress_gate.close();
                    let result = match self.records.first_nonterminal().await {
                        Ok(Some(record)) => Err(FabricCommandActorError::RecoveryPending {
                            operation_id: record.command().identity.operation_id,
                            obligation: record.recovery_obligation().expect(
                                "first_nonterminal cannot return a terminal command record",
                            ),
                        }),
                        Ok(None) => {
                            self.ingress_gate.open();
                            Ok(())
                        }
                        Err(error) => Err(error.into()),
                    };
                    let _ = response.send(result);
                }
                ActorMessage::Shutdown { response } => {
                    self.receiver.close();
                    let _ = response.send(());
                    break;
                }
            }
        }
    }

    async fn process_submit(
        &self,
        command: FabricCommand,
    ) -> Result<CommandRecord, FabricCommandActorError> {
        if let Some(existing) = self
            .records
            .lookup_admission(
                command.identity.operation_id,
                command.identity.idempotency_key,
            )
            .await?
        {
            return replay_admission(existing, &command);
        }

        let admission_context = self.contexts.read_admission_context(&command).await?;
        let admitted = match CommandReducer::admit(None, &command, admission_context)? {
            AdmissionOutcome::New(record) => record,
            AdmissionOutcome::Existing(_) => {
                return Err(FabricCommandActorError::DurableAdmissionMismatch);
            }
        };
        let mut record = match self.records.insert_if_absent(admitted).await? {
            DurableAdmissionWrite::Inserted(stored) => {
                if stored != admitted {
                    return Err(FabricCommandActorError::DurableAdmissionMismatch);
                }
                stored
            }
            DurableAdmissionWrite::Existing(existing) => {
                return replay_admission(existing, &command);
            }
        };

        let owner = ExecutionOwner {
            actor_id: self.actor_id,
            fence: command.writer_fence,
        };
        let start = CommandEvent::Start { owner };
        record = self.transition(record, start).await?;

        self.execute_started(record, owner).await
    }

    async fn execute_started(
        &self,
        mut record: CommandRecord,
        owner: ExecutionOwner,
    ) -> Result<CommandRecord, FabricCommandActorError> {
        let start = CommandEvent::Start { owner };

        // Re-read and reducer-validate immediately before invoking an effect. The effect port is
        // additionally obligated to fence its own durable backend boundary with this context.
        let prepare_context = self.validate_idempotent(record, start).await?;
        match self
            .effects
            .prepare(&record, owner, prepare_context)
            .await?
        {
            PrepareEffectOutcome::KnownFailure { failure } => {
                self.transition(record, CommandEvent::ReportKnownFailure { owner, failure })
                    .await
            }
            PrepareEffectOutcome::Cancelled { cancellation } => {
                self.transition(
                    record,
                    CommandEvent::CancelBeforeCommit {
                        owner,
                        cancellation,
                    },
                )
                .await
            }
            PrepareEffectOutcome::Prepared { transaction } => {
                let prepare = CommandEvent::PrepareCommit { owner, transaction };
                record = self.transition(record, prepare).await?;
                // Commit execution is impossible until CommitPrepared is durably CASed.
                let commit_context = self.validate_idempotent(record, prepare).await?;
                let event = match self
                    .effects
                    .commit(&record, owner, transaction, commit_context)
                    .await?
                {
                    CommitEffectOutcome::Committed { result } => CommandEvent::ConfirmCommit {
                        owner,
                        transaction,
                        result,
                    },
                    CommitEffectOutcome::KnownFailure { failure } => {
                        CommandEvent::ReportKnownFailure { owner, failure }
                    }
                    CommitEffectOutcome::Unknown { unknown } => CommandEvent::ReportUnknownCommit {
                        owner,
                        transaction,
                        unknown,
                    },
                };
                self.transition(record, event).await
            }
        }
    }

    async fn process_retry(
        &self,
        operation_id: OperationId,
    ) -> Result<CommandRecord, FabricCommandActorError> {
        let record = self
            .records
            .lookup_operation(operation_id)
            .await?
            .ok_or(FabricCommandActorError::OperationNotFound)?;
        let DurableCommandState::RetryReady { required_fence, .. } = record.state() else {
            return Err(FabricCommandActorError::EffectStateMismatch);
        };
        let context = self.contexts.read_reduction_context(&record).await?;
        let owner = ExecutionOwner {
            actor_id: self.actor_id,
            fence: context.active_fence,
        };
        if owner.fence.generation.get() < required_fence.generation.get() {
            return Err(FabricCommandActorError::Contract(
                CommandContractError::RecoveryFenceNotAdvanced,
            ));
        }
        let started = self
            .transition(record, CommandEvent::Start { owner })
            .await?;
        self.execute_started(started, owner).await
    }

    async fn process_resume_precommit(
        &self,
        operation_id: OperationId,
    ) -> Result<CommandRecord, FabricCommandActorError> {
        let record = self
            .records
            .lookup_operation(operation_id)
            .await?
            .ok_or(FabricCommandActorError::OperationNotFound)?;
        if !matches!(
            record.state(),
            DurableCommandState::Admitted { .. } | DurableCommandState::Executing { .. }
        ) {
            return Err(FabricCommandActorError::EffectStateMismatch);
        }
        let context = self.contexts.read_reduction_context(&record).await?;
        let owner = ExecutionOwner {
            actor_id: self.actor_id,
            fence: context.active_fence,
        };
        let started = self
            .transition(record, CommandEvent::Start { owner })
            .await?;
        self.execute_started(started, owner).await
    }

    async fn process_mark_interrupted_commit(
        &self,
        operation_id: OperationId,
        unknown: UnknownCommit,
    ) -> Result<CommandRecord, FabricCommandActorError> {
        let record = self
            .records
            .lookup_operation(operation_id)
            .await?
            .ok_or(FabricCommandActorError::OperationNotFound)?;
        let DurableCommandState::CommitPrepared { transaction, .. } = record.state() else {
            return Err(FabricCommandActorError::EffectStateMismatch);
        };
        let context = self.contexts.read_reduction_context(&record).await?;
        self.transition(
            record,
            CommandEvent::ReportUnknownCommit {
                owner: ExecutionOwner {
                    actor_id: self.actor_id,
                    fence: context.active_fence,
                },
                transaction,
                unknown,
            },
        )
        .await
    }

    async fn process_cancel(
        &self,
        operation_id: OperationId,
        cancellation: CommandCancellation,
    ) -> Result<CommandRecord, FabricCommandActorError> {
        let record = self
            .records
            .lookup_operation(operation_id)
            .await?
            .ok_or(FabricCommandActorError::OperationNotFound)?;
        let context = self.contexts.read_reduction_context(&record).await?;
        let owner = ExecutionOwner {
            actor_id: self.actor_id,
            fence: context.active_fence,
        };
        let record = if let DurableCommandState::Executing { owner: current, .. } = record.state()
            && current != owner
        {
            self.transition(record, CommandEvent::Start { owner })
                .await?
        } else {
            record
        };
        self.transition(
            record,
            CommandEvent::CancelBeforeCommit {
                owner,
                cancellation,
            },
        )
        .await
    }

    async fn process_reconcile(
        &self,
        operation_id: OperationId,
    ) -> Result<CommandRecord, FabricCommandActorError> {
        let record = self
            .records
            .lookup_operation(operation_id)
            .await?
            .ok_or(FabricCommandActorError::OperationNotFound)?;
        let DurableCommandState::AwaitingReconciliation { transaction, .. } = record.state() else {
            return Err(FabricCommandActorError::EffectStateMismatch);
        };
        let context = self.contexts.read_reduction_context(&record).await?;
        let owner = ExecutionOwner {
            actor_id: self.actor_id,
            fence: context.active_fence,
        };
        let observation = self
            .effects
            .reconcile(&record, owner, transaction, context)
            .await?;
        self.transition(
            record,
            CommandEvent::ObserveReconciliation {
                owner,
                transaction,
                observation,
            },
        )
        .await
    }

    async fn validate_idempotent(
        &self,
        record: CommandRecord,
        event: CommandEvent,
    ) -> Result<ReductionContext, FabricCommandActorError> {
        let context = self.contexts.read_reduction_context(&record).await?;
        let reduction = CommandReducer::reduce(&record, event, context)?;
        if reduction.effect != ReductionEffect::IdempotentReplay || reduction.record != record {
            return Err(FabricCommandActorError::EffectStateMismatch);
        }
        Ok(context)
    }

    async fn transition(
        &self,
        record: CommandRecord,
        event: CommandEvent,
    ) -> Result<CommandRecord, FabricCommandActorError> {
        let context = self.contexts.read_reduction_context(&record).await?;
        let reduction = CommandReducer::reduce(&record, event, context)?;
        match reduction.effect {
            ReductionEffect::IdempotentReplay => Ok(record),
            ReductionEffect::StateChanged | ReductionEffect::ReconciliationStillRequired => {
                let transition = reduction
                    .transition()
                    .ok_or(FabricCommandActorError::EffectStateMismatch)?;
                if transition.predecessor() != record
                    || transition.successor() != reduction.record
                    || transition.effect() != reduction.effect
                {
                    return Err(FabricCommandActorError::EffectStateMismatch);
                }
                match self.records.compare_and_swap(transition).await? {
                    DurableTransitionWrite::Stored(stored) => {
                        if stored != reduction.record {
                            return Err(FabricCommandActorError::DurableTransitionMismatch);
                        }
                        Ok(stored)
                    }
                    DurableTransitionWrite::RevisionConflict { observed } => {
                        Err(FabricCommandActorError::RevisionConflict {
                            expected: record.revision(),
                            observed: observed.map(CommandRecord::revision),
                        })
                    }
                }
            }
        }
    }
}

#[cfg(test)]
fn spawn_fabric_command_actor(
    config: FabricCommandActorConfig,
    actor_id: ActorId,
    records: Arc<dyn DurableCommandRecordPort>,
    contexts: Arc<dyn CommandReductionContextPort>,
    effects: Arc<dyn FabricCommandEffectPort>,
) -> (FabricCommandActorHandle, JoinHandle<()>) {
    let gate = Arc::new(FabricCommandIngressGate::open_for_tests());
    let (handle, task) =
        spawn_fabric_command_actor_with_gate(config, actor_id, records, contexts, effects, gate);
    (handle, task)
}

/// Spawn the production actor with new-command admission closed for restart reconciliation.
pub(super) fn spawn_fabric_command_actor_closed(
    config: FabricCommandActorConfig,
    actor_id: ActorId,
    records: Arc<dyn DurableCommandRecordPort>,
    contexts: Arc<dyn CommandReductionContextPort>,
    effects: Arc<dyn FabricCommandEffectPort>,
) -> (
    FabricCommandActorHandle,
    JoinHandle<()>,
    Arc<FabricCommandIngressGate>,
) {
    let gate = Arc::new(FabricCommandIngressGate::closed());
    let (handle, task) = spawn_fabric_command_actor_with_gate(
        config,
        actor_id,
        records,
        contexts,
        effects,
        Arc::clone(&gate),
    );
    (handle, task, gate)
}

fn spawn_fabric_command_actor_with_gate(
    config: FabricCommandActorConfig,
    actor_id: ActorId,
    records: Arc<dyn DurableCommandRecordPort>,
    contexts: Arc<dyn CommandReductionContextPort>,
    effects: Arc<dyn FabricCommandEffectPort>,
    ingress_gate: Arc<FabricCommandIngressGate>,
) -> (FabricCommandActorHandle, JoinHandle<()>) {
    let (sender, receiver) = mpsc::channel(config.queue_capacity());
    let actor = FabricCommandActor {
        actor_id,
        records,
        contexts,
        effects,
        ingress_gate: Arc::clone(&ingress_gate),
        receiver,
    };
    let task = tokio::spawn(actor.run());
    (
        FabricCommandActorHandle {
            sender,
            ingress_gate,
        },
        task,
    )
}

fn replay_admission(
    existing: CommandRecord,
    command: &FabricCommand,
) -> Result<CommandRecord, FabricCommandActorError> {
    let replay_context = AdmissionContext {
        workspace_id: command.ownership.workspace_id,
        current_head: command.expected_head,
        active_fence: command.writer_fence,
        authorization: AuthorizationDecision::Authorized(command.ownership.authorization),
    };
    match CommandReducer::admit(Some(&existing), command, replay_context)? {
        AdmissionOutcome::Existing(record) if record == existing => Ok(record),
        AdmissionOutcome::New(_) | AdmissionOutcome::Existing(_) => {
            Err(FabricCommandActorError::DurableAdmissionMismatch)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, VecDeque};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use super::*;
    use crate::fabric::command::{
        AuthorizationRef, CommandIdentity, CommandOwnership, CommandPins, DiagnosticRef, EpochId,
        ExpectedHead, FabricCommandPayload, InputReleaseRef, LeaseId, OperationSelectionRef,
        PrincipalId, ProgramReleaseRef, ProofReceiptRef, ProviderSetRef, ReconciliationEvidenceRef,
        ResourceEnvelopeRef, RetryBasis, SourceGeneration, UnknownCommitReason, WorkspaceId,
        WriterFence, WriterGeneration,
    };

    #[derive(Default)]
    struct MemoryRecordState {
        by_operation: BTreeMap<OperationId, CommandRecord>,
        by_idempotency: BTreeMap<IdempotencyKey, OperationId>,
    }

    #[derive(Default)]
    struct MemoryRecordPort {
        state: Mutex<MemoryRecordState>,
        conflict_next_cas: AtomicBool,
    }

    impl MemoryRecordPort {
        fn conflict_next_cas(&self) {
            self.conflict_next_cas.store(true, Ordering::SeqCst);
        }

        fn len(&self) -> usize {
            self.state
                .lock()
                .expect("memory record mutex is not poisoned")
                .by_operation
                .len()
        }

        fn record(&self, operation_id: OperationId) -> Option<CommandRecord> {
            self.state
                .lock()
                .expect("memory record mutex is not poisoned")
                .by_operation
                .get(&operation_id)
                .copied()
        }

        fn seed(&self, record: CommandRecord) {
            let mut state = self
                .state
                .lock()
                .expect("memory record mutex is not poisoned");
            state.by_idempotency.insert(
                record.command().identity.idempotency_key,
                record.command().identity.operation_id,
            );
            state
                .by_operation
                .insert(record.command().identity.operation_id, record);
        }

        fn lookup_locked(
            state: &MemoryRecordState,
            operation_id: OperationId,
            idempotency_key: IdempotencyKey,
        ) -> Result<Option<CommandRecord>, CommandPortError> {
            let by_operation = state.by_operation.get(&operation_id).copied();
            let by_idempotency = state
                .by_idempotency
                .get(&idempotency_key)
                .and_then(|operation| state.by_operation.get(operation))
                .copied();
            match (by_operation, by_idempotency) {
                (None, None) => Ok(None),
                (Some(record), None) | (None, Some(record)) => Ok(Some(record)),
                (Some(left), Some(right)) if left == right => Ok(Some(left)),
                (Some(_), Some(_)) => Err(CommandPortError::CorruptRecord),
            }
        }
    }

    #[async_trait]
    impl DurableCommandRecordPort for MemoryRecordPort {
        async fn lookup_admission(
            &self,
            operation_id: OperationId,
            idempotency_key: IdempotencyKey,
        ) -> Result<Option<CommandRecord>, CommandPortError> {
            Self::lookup_locked(
                &self
                    .state
                    .lock()
                    .expect("memory record mutex is not poisoned"),
                operation_id,
                idempotency_key,
            )
        }

        async fn lookup_operation(
            &self,
            operation_id: OperationId,
        ) -> Result<Option<CommandRecord>, CommandPortError> {
            Ok(self
                .state
                .lock()
                .expect("memory record mutex is not poisoned")
                .by_operation
                .get(&operation_id)
                .copied())
        }

        async fn insert_if_absent(
            &self,
            admitted: CommandRecord,
        ) -> Result<DurableAdmissionWrite, CommandPortError> {
            if admitted.revision() != 0 {
                return Err(CommandPortError::CorruptRecord);
            }
            let mut state = self
                .state
                .lock()
                .expect("memory record mutex is not poisoned");
            let command = admitted.command();
            if let Some(existing) = Self::lookup_locked(
                &state,
                command.identity.operation_id,
                command.identity.idempotency_key,
            )? {
                return Ok(DurableAdmissionWrite::Existing(existing));
            }
            state.by_idempotency.insert(
                command.identity.idempotency_key,
                command.identity.operation_id,
            );
            state
                .by_operation
                .insert(command.identity.operation_id, admitted);
            Ok(DurableAdmissionWrite::Inserted(admitted))
        }

        async fn compare_and_swap(
            &self,
            transition: ReducerTransition,
        ) -> Result<DurableTransitionWrite, CommandPortError> {
            let predecessor = transition.predecessor();
            let successor = transition.successor();
            let operation_id = predecessor.command().identity.operation_id;
            let mut state = self
                .state
                .lock()
                .expect("memory record mutex is not poisoned");
            let observed = state.by_operation.get(&operation_id).copied();
            if self.conflict_next_cas.swap(false, Ordering::SeqCst)
                || observed.map(CommandRecord::revision) != Some(predecessor.revision())
            {
                return Ok(DurableTransitionWrite::RevisionConflict { observed });
            }
            if observed != Some(predecessor) {
                return Err(CommandPortError::CorruptRecord);
            }
            if transition.effect() == ReductionEffect::IdempotentReplay
                || successor.command() != predecessor.command()
                || successor.revision() != predecessor.revision().saturating_add(1)
            {
                return Err(CommandPortError::CorruptRecord);
            }
            state.by_operation.insert(operation_id, successor);
            Ok(DurableTransitionWrite::Stored(successor))
        }

        async fn first_nonterminal(&self) -> Result<Option<CommandRecord>, CommandPortError> {
            Ok(self
                .state
                .lock()
                .expect("memory record mutex is not poisoned")
                .by_operation
                .values()
                .copied()
                .find(|record| !record.state().is_terminal()))
        }
    }

    struct MemoryContextPort {
        admission: Mutex<AdmissionContext>,
        reduction: Mutex<ReductionContext>,
        reduction_after_read: Mutex<Option<ReductionContext>>,
    }

    impl MemoryContextPort {
        fn matching(command: &FabricCommand) -> Self {
            Self {
                admission: Mutex::new(AdmissionContext {
                    workspace_id: command.ownership.workspace_id,
                    current_head: command.expected_head,
                    active_fence: command.writer_fence,
                    authorization: AuthorizationDecision::Authorized(
                        command.ownership.authorization,
                    ),
                }),
                reduction: Mutex::new(ReductionContext {
                    current_head: command.expected_head,
                    active_fence: command.writer_fence,
                }),
                reduction_after_read: Mutex::new(None),
            }
        }

        fn set_admission_and_reduction_fence(&self, fence: WriterFence) {
            self.admission
                .lock()
                .expect("memory context mutex is not poisoned")
                .active_fence = fence;
            self.reduction
                .lock()
                .expect("memory context mutex is not poisoned")
                .active_fence = fence;
        }

        fn set_reduction_fence_after_next_read(&self, fence: WriterFence) {
            let current = *self
                .reduction
                .lock()
                .expect("memory context mutex is not poisoned");
            *self
                .reduction_after_read
                .lock()
                .expect("memory context mutex is not poisoned") = Some(ReductionContext {
                current_head: current.current_head,
                active_fence: fence,
            });
        }
    }

    #[async_trait]
    impl CommandReductionContextPort for MemoryContextPort {
        async fn read_admission_context(
            &self,
            _command: &FabricCommand,
        ) -> Result<AdmissionContext, CommandPortError> {
            Ok(*self
                .admission
                .lock()
                .expect("memory context mutex is not poisoned"))
        }

        async fn read_reduction_context(
            &self,
            _record: &CommandRecord,
        ) -> Result<ReductionContext, CommandPortError> {
            let mut reduction = self
                .reduction
                .lock()
                .expect("memory context mutex is not poisoned");
            let observed = *reduction;
            if let Some(next) = self
                .reduction_after_read
                .lock()
                .expect("memory context mutex is not poisoned")
                .take()
            {
                *reduction = next;
            }
            Ok(observed)
        }
    }

    #[derive(Clone, Copy)]
    enum TestCommitMode {
        Committed,
        Unknown(UnknownCommit),
    }

    struct TestEffectPort {
        prepare_outcome: PrepareEffectOutcome,
        commit_mode: TestCommitMode,
        prepare_delay: Duration,
        prepare_calls: AtomicUsize,
        commit_calls: AtomicUsize,
        reconcile_calls: AtomicUsize,
        reconciliation: Mutex<VecDeque<ReconciliationObservation>>,
        in_flight: AtomicUsize,
        max_in_flight: AtomicUsize,
    }

    impl TestEffectPort {
        fn committed() -> Self {
            Self {
                prepare_outcome: PrepareEffectOutcome::Prepared {
                    transaction: transaction(),
                },
                commit_mode: TestCommitMode::Committed,
                prepare_delay: Duration::ZERO,
                prepare_calls: AtomicUsize::new(0),
                commit_calls: AtomicUsize::new(0),
                reconcile_calls: AtomicUsize::new(0),
                reconciliation: Mutex::new(VecDeque::new()),
                in_flight: AtomicUsize::new(0),
                max_in_flight: AtomicUsize::new(0),
            }
        }

        fn unknown(unknown: UnknownCommit) -> Self {
            Self {
                commit_mode: TestCommitMode::Unknown(unknown),
                ..Self::committed()
            }
        }

        fn cancelled(cancellation: CommandCancellation) -> Self {
            Self {
                prepare_outcome: PrepareEffectOutcome::Cancelled { cancellation },
                ..Self::committed()
            }
        }

        fn with_prepare_delay(mut self, delay: Duration) -> Self {
            self.prepare_delay = delay;
            self
        }

        fn prepare_calls(&self) -> usize {
            self.prepare_calls.load(Ordering::SeqCst)
        }

        fn commit_calls(&self) -> usize {
            self.commit_calls.load(Ordering::SeqCst)
        }

        fn reconcile_calls(&self) -> usize {
            self.reconcile_calls.load(Ordering::SeqCst)
        }

        fn queue_reconciliation(&self, observation: ReconciliationObservation) {
            self.reconciliation
                .lock()
                .expect("reconciliation queue mutex is not poisoned")
                .push_back(observation);
        }

        fn max_in_flight(&self) -> usize {
            self.max_in_flight.load(Ordering::SeqCst)
        }

        fn enter_effect(&self) {
            let in_flight = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_in_flight.fetch_max(in_flight, Ordering::SeqCst);
        }

        fn leave_effect(&self) {
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
        }
    }

    #[async_trait]
    impl FabricCommandEffectPort for TestEffectPort {
        async fn prepare(
            &self,
            _executing: &CommandRecord,
            _owner: ExecutionOwner,
            _context: ReductionContext,
        ) -> Result<PrepareEffectOutcome, CommandPortError> {
            self.prepare_calls.fetch_add(1, Ordering::SeqCst);
            self.enter_effect();
            tokio::time::sleep(self.prepare_delay).await;
            self.leave_effect();
            Ok(self.prepare_outcome)
        }

        async fn commit(
            &self,
            prepared: &CommandRecord,
            _owner: ExecutionOwner,
            _transaction: TransactionRef,
            _context: ReductionContext,
        ) -> Result<CommitEffectOutcome, CommandPortError> {
            self.commit_calls.fetch_add(1, Ordering::SeqCst);
            self.enter_effect();
            let outcome = match self.commit_mode {
                TestCommitMode::Committed => {
                    let FabricCommandPayload::ActivateEpoch {
                        candidate_epoch, ..
                    } = prepared.command().payload
                    else {
                        return Err(CommandPortError::EffectUnavailable);
                    };
                    CommitEffectOutcome::Committed {
                        result: CommandResult::EpochActivated {
                            epoch: candidate_epoch,
                            selection: OperationSelectionRef::from_bytes([0xa5; 32]),
                        },
                    }
                }
                TestCommitMode::Unknown(unknown) => CommitEffectOutcome::Unknown { unknown },
            };
            self.leave_effect();
            Ok(outcome)
        }

        async fn reconcile(
            &self,
            _awaiting: &CommandRecord,
            _owner: ExecutionOwner,
            _transaction: TransactionRef,
            _context: ReductionContext,
        ) -> Result<ReconciliationObservation, CommandPortError> {
            self.reconcile_calls.fetch_add(1, Ordering::SeqCst);
            self.reconciliation
                .lock()
                .expect("reconciliation queue mutex is not poisoned")
                .pop_front()
                .ok_or(CommandPortError::EffectUnavailable)
        }
    }

    fn test_command(seed: u8) -> FabricCommand {
        FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes([seed; 16]),
                idempotency_key: IdempotencyKey::from_bytes([seed; 32]),
            },
            ownership: CommandOwnership {
                workspace_id: WorkspaceId::from_bytes([0x10; 16]),
                principal_id: PrincipalId::from_bytes([0x11; 16]),
                authorization: AuthorizationRef::from_bytes([0x12; 32]),
            },
            expected_head: ExpectedHead::Empty,
            writer_fence: fence(1),
            pins: CommandPins {
                input_release: InputReleaseRef::from_bytes([0x20; 32]),
                program_release: ProgramReleaseRef::from_bytes([0x21; 32]),
                application_release: crate::fabric::command::ApplicationReleaseRef::from_bytes(
                    [0x21; 32],
                ),
                source_authority: crate::fabric::command::SourceAuthorityRef::from_bytes(
                    [0x21; 32],
                ),
                provider_release: crate::fabric::command::ProviderReleaseRef::from_bytes(
                    [0x21; 32],
                ),
                source_generation: SourceGeneration::new(0),
                provider_set: ProviderSetRef::from_bytes([0x22; 32]),
            },
            resources: ResourceEnvelopeRef::from_bytes([0x23; 32]),
            payload: FabricCommandPayload::ActivateEpoch {
                candidate_epoch: EpochId::from_bytes([seed.wrapping_add(1); 16]),
                proof_receipt: ProofReceiptRef::from_bytes([seed.wrapping_add(1); 32]),
            },
        }
    }

    fn fence(generation: u64) -> WriterFence {
        WriterFence {
            lease_id: LeaseId::from_bytes([0x30; 16]),
            generation: WriterGeneration::new(generation).expect("test generation is nonzero"),
        }
    }

    fn actor_id() -> ActorId {
        ActorId::from_bytes([0x40; 16])
    }

    fn transaction() -> TransactionRef {
        TransactionRef::from_bytes([0x50; 32])
    }

    fn admitted_record(command: &FabricCommand) -> CommandRecord {
        match CommandReducer::admit(
            None,
            command,
            AdmissionContext {
                workspace_id: command.ownership.workspace_id,
                current_head: command.expected_head,
                active_fence: command.writer_fence,
                authorization: AuthorizationDecision::Authorized(command.ownership.authorization),
            },
        )
        .expect("test command admits")
        {
            AdmissionOutcome::New(record) => record,
            AdmissionOutcome::Existing(_) => panic!("new test command cannot already exist"),
        }
    }

    fn reduce_record(
        record: &CommandRecord,
        event: CommandEvent,
        active_fence: WriterFence,
    ) -> CommandRecord {
        CommandReducer::reduce(
            record,
            event,
            ReductionContext {
                current_head: record.command().expected_head,
                active_fence,
            },
        )
        .expect("test transition succeeds")
        .record
    }

    fn command_result(command: &FabricCommand) -> CommandResult {
        let FabricCommandPayload::ActivateEpoch {
            candidate_epoch, ..
        } = command.payload
        else {
            panic!("test command is activation")
        };
        CommandResult::EpochActivated {
            epoch: candidate_epoch,
            selection: OperationSelectionRef::from_bytes([0xa5; 32]),
        }
    }

    fn config(capacity: usize) -> FabricCommandActorConfig {
        FabricCommandActorConfig::new(
            NonZeroUsize::new(capacity).expect("test queue capacity is nonzero"),
        )
    }

    #[tokio::test]
    async fn one_actor_serializes_effect_execution() {
        let first = test_command(1);
        let second = test_command(2);
        let records = Arc::new(MemoryRecordPort::default());
        let contexts = Arc::new(MemoryContextPort::matching(&first));
        let effects =
            Arc::new(TestEffectPort::committed().with_prepare_delay(Duration::from_millis(20)));
        let (handle, task) =
            spawn_fabric_command_actor(config(4), actor_id(), records, contexts, effects.clone());

        let first_handle = handle.clone();
        let second_handle = handle.clone();
        let (first_result, second_result) =
            tokio::join!(first_handle.submit(first), second_handle.submit(second),);

        assert!(matches!(
            first_result.expect("first command succeeds").state(),
            DurableCommandState::Succeeded { .. }
        ));
        assert!(matches!(
            second_result.expect("second command succeeds").state(),
            DurableCommandState::Succeeded { .. }
        ));
        assert_eq!(effects.max_in_flight(), 1);
        assert_eq!(effects.prepare_calls(), 2);
        assert_eq!(effects.commit_calls(), 2);
        drop(first_handle);
        drop(second_handle);
        drop(handle);
        task.await.expect("actor exits after all handles close");
    }

    #[tokio::test]
    async fn exact_duplicate_returns_existing_record_without_reexecuting() {
        let command = test_command(3);
        let records = Arc::new(MemoryRecordPort::default());
        let contexts = Arc::new(MemoryContextPort::matching(&command));
        let effects = Arc::new(TestEffectPort::committed());
        let (handle, task) = spawn_fabric_command_actor(
            config(2),
            actor_id(),
            records.clone(),
            contexts.clone(),
            effects.clone(),
        );

        let first = handle
            .submit(command)
            .await
            .expect("first command succeeds");
        contexts.set_admission_and_reduction_fence(fence(2));
        let duplicate = handle
            .submit(command)
            .await
            .expect("exact duplicate returns durable record");

        assert_eq!(duplicate, first);
        assert_eq!(records.len(), 1);
        assert_eq!(effects.prepare_calls(), 1);
        assert_eq!(effects.commit_calls(), 1);
        drop(handle);
        task.await.expect("actor exits after handle closes");
    }

    #[tokio::test]
    async fn stale_reduction_fence_rejects_after_admission_before_effects() {
        let command = test_command(4);
        let records = Arc::new(MemoryRecordPort::default());
        let contexts = Arc::new(MemoryContextPort::matching(&command));
        contexts.set_reduction_fence_after_next_read(fence(2));
        let effects = Arc::new(TestEffectPort::committed());
        let (handle, task) = spawn_fabric_command_actor(
            config(1),
            actor_id(),
            records.clone(),
            contexts,
            effects.clone(),
        );

        let error = handle
            .submit(command)
            .await
            .expect_err("stale fence must reject");

        assert!(matches!(
            error,
            FabricCommandActorError::Contract(CommandContractError::StaleWriterFence {
                expected,
                observed,
            }) if expected == fence(1) && observed == fence(2)
        ));
        assert_eq!(effects.prepare_calls(), 0);
        assert_eq!(effects.commit_calls(), 0);
        assert!(matches!(
            records
                .record(command.identity.operation_id)
                .expect("admission and start are durable before effect validation"),
            record if record.revision() == 1
                && matches!(record.state(), DurableCommandState::Executing { attempt: 1, .. })
        ));
        drop(handle);
        task.await.expect("actor exits after handle closes");
    }

    #[tokio::test]
    async fn unknown_commit_remains_awaiting_reconciliation() {
        let command = test_command(5);
        let unknown = UnknownCommit {
            reason: UnknownCommitReason::ConnectionLost,
            diagnostic: DiagnosticRef::from_bytes([0x60; 32]),
        };
        let records = Arc::new(MemoryRecordPort::default());
        let contexts = Arc::new(MemoryContextPort::matching(&command));
        let effects = Arc::new(TestEffectPort::unknown(unknown));
        let (handle, task) = spawn_fabric_command_actor(
            config(1),
            actor_id(),
            records.clone(),
            contexts,
            effects.clone(),
        );

        let result = handle
            .submit(command)
            .await
            .expect("unknown commit is durable state, not actor failure");

        assert_eq!(result.revision(), 3);
        assert!(matches!(
            result.state(),
            DurableCommandState::AwaitingReconciliation {
                unknown: observed,
                probe_count: 0,
                ..
            } if observed == unknown
        ));
        assert_eq!(records.record(command.identity.operation_id), Some(result));
        assert_eq!(effects.prepare_calls(), 1);
        assert_eq!(effects.commit_calls(), 1);
        assert_eq!(
            handle.submit(test_command(6)).await,
            Err(FabricCommandActorError::AdmissionClosedForRecovery)
        );
        drop(handle);
        task.await.expect("actor exits after handle closes");
    }

    #[tokio::test]
    async fn production_gate_reopens_only_after_actor_ordered_terminal_recovery() {
        let command = test_command(7);
        let unknown = UnknownCommit {
            reason: UnknownCommitReason::ReadbackUnavailable,
            diagnostic: DiagnosticRef::from_bytes([0x64; 32]),
        };
        let records = Arc::new(MemoryRecordPort::default());
        let contexts = Arc::new(MemoryContextPort::matching(&command));
        let effects = Arc::new(TestEffectPort::unknown(unknown));
        let (handle, task, _gate) = spawn_fabric_command_actor_closed(
            config(2),
            actor_id(),
            records,
            contexts,
            effects.clone(),
        );
        assert_eq!(
            handle.submit(command).await,
            Err(FabricCommandActorError::AdmissionClosedForRecovery)
        );
        handle
            .open_admission_after_recovery()
            .await
            .expect("empty temporal journal opens admission in actor order");
        let awaiting = handle
            .submit(command)
            .await
            .expect("opened admission accepts the command");
        assert!(matches!(
            awaiting.state(),
            DurableCommandState::AwaitingReconciliation { .. }
        ));
        assert_eq!(
            handle.open_admission_after_recovery().await,
            Err(FabricCommandActorError::RecoveryPending {
                operation_id: command.identity.operation_id,
                obligation: CommandRecoveryObligation::ReconcileCommit {
                    transaction: transaction(),
                },
            })
        );

        effects.queue_reconciliation(ReconciliationObservation::Committed {
            evidence: ReconciliationEvidenceRef::from_bytes([0x65; 32]),
            result: command_result(&command),
        });
        let terminal = handle
            .reconcile(command.identity.operation_id)
            .await
            .expect("exact marker readback terminalizes the command");
        assert!(terminal.state().is_terminal());
        handle
            .open_admission_after_recovery()
            .await
            .expect("terminal journal reopens admission");
        let second = handle
            .submit(test_command(8))
            .await
            .expect("a later command is admitted only after recovery");
        assert!(matches!(
            second.state(),
            DurableCommandState::AwaitingReconciliation { .. }
        ));
        drop(handle);
        task.await.expect("production actor exits");
    }

    #[tokio::test]
    async fn reconciliation_evidence_is_serialized_without_blind_retry() {
        let command = test_command(9);
        let unknown = UnknownCommit {
            reason: UnknownCommitReason::ReadbackUnavailable,
            diagnostic: DiagnosticRef::from_bytes([0x61; 32]),
        };
        let records = Arc::new(MemoryRecordPort::default());
        let contexts = Arc::new(MemoryContextPort::matching(&command));
        let effects = Arc::new(TestEffectPort::unknown(unknown));
        let (handle, task) = spawn_fabric_command_actor(
            config(2),
            actor_id(),
            records.clone(),
            contexts.clone(),
            effects.clone(),
        );
        let awaiting = handle
            .submit(command)
            .await
            .expect("unknown commit becomes durable");
        assert_eq!(awaiting.revision(), 3);

        let first_evidence = ReconciliationEvidenceRef::from_bytes([0x62; 32]);
        effects.queue_reconciliation(ReconciliationObservation::Indeterminate {
            evidence: first_evidence,
        });
        let still_unknown = handle
            .reconcile(command.identity.operation_id)
            .await
            .expect("indeterminate evidence is durably counted");
        assert!(matches!(
            still_unknown.state(),
            DurableCommandState::AwaitingReconciliation {
                probe_count: 1,
                last_evidence: Some(observed),
                ..
            } if observed == first_evidence
        ));

        let no_commit_evidence = ReconciliationEvidenceRef::from_bytes([0x63; 32]);
        effects.queue_reconciliation(ReconciliationObservation::NotCommitted {
            evidence: no_commit_evidence,
        });
        let retry_ready = handle
            .reconcile(command.identity.operation_id)
            .await
            .expect("proved non-commit may authorize a later retry");
        assert!(matches!(
            retry_ready.state(),
            DurableCommandState::RetryReady {
                next_attempt: 2,
                basis: RetryBasis::ReconciledNotCommitted(observed),
                ..
            } if observed == no_commit_evidence
        ));
        assert_eq!(effects.prepare_calls(), 1);
        assert_eq!(effects.commit_calls(), 1);
        assert_eq!(effects.reconcile_calls(), 2);
        assert_eq!(
            records.record(command.identity.operation_id),
            Some(retry_ready)
        );
        drop(handle);
        task.await.expect("actor exits after handle closes");

        let recovery_effects = Arc::new(TestEffectPort::committed());
        let (recovery, recovery_task) = spawn_fabric_command_actor(
            config(1),
            actor_id(),
            records.clone(),
            contexts,
            recovery_effects.clone(),
        );
        let succeeded = recovery
            .retry_proved_not_committed(command.identity.operation_id)
            .await
            .expect("proved-not-committed command executes a second attempt");
        assert!(matches!(
            succeeded.state(),
            DurableCommandState::Succeeded { .. }
        ));
        assert_eq!(recovery_effects.prepare_calls(), 1);
        assert_eq!(recovery_effects.commit_calls(), 1);
        drop(recovery);
        recovery_task
            .await
            .expect("recovery actor exits after handle closes");
    }

    #[tokio::test]
    async fn restart_resumes_only_precommit_states_under_a_new_generation() {
        for (seed, was_executing) in [(10, false), (11, true)] {
            let command = test_command(seed);
            let admitted = admitted_record(&command);
            let record = if was_executing {
                reduce_record(
                    &admitted,
                    CommandEvent::Start {
                        owner: ExecutionOwner {
                            actor_id: actor_id(),
                            fence: command.writer_fence,
                        },
                    },
                    command.writer_fence,
                )
            } else {
                admitted
            };
            let records = Arc::new(MemoryRecordPort::default());
            records.seed(record);
            let contexts = Arc::new(MemoryContextPort::matching(&command));
            contexts.set_admission_and_reduction_fence(fence(2));
            let effects = Arc::new(TestEffectPort::committed());
            let (recovery, task) = spawn_fabric_command_actor(
                config(1),
                ActorId::from_bytes([seed.wrapping_add(0x40); 16]),
                records,
                contexts,
                effects.clone(),
            );

            let succeeded = recovery
                .resume_precommit(command.identity.operation_id)
                .await
                .expect("new fenced writer resumes a pre-commit command");
            assert!(matches!(
                succeeded.state(),
                DurableCommandState::Succeeded { .. }
            ));
            assert_eq!(effects.prepare_calls(), 1);
            assert_eq!(effects.commit_calls(), 1);
            drop(recovery);
            task.await.expect("recovery actor exits");
        }
    }

    #[tokio::test]
    async fn restart_reconciles_prepared_commit_before_retrying_under_a_later_fence() {
        let command = test_command(12);
        let original_owner = ExecutionOwner {
            actor_id: actor_id(),
            fence: command.writer_fence,
        };
        let admitted = admitted_record(&command);
        let executing = reduce_record(
            &admitted,
            CommandEvent::Start {
                owner: original_owner,
            },
            command.writer_fence,
        );
        let prepared = reduce_record(
            &executing,
            CommandEvent::PrepareCommit {
                owner: original_owner,
                transaction: transaction(),
            },
            command.writer_fence,
        );
        let records = Arc::new(MemoryRecordPort::default());
        records.seed(prepared);
        let contexts = Arc::new(MemoryContextPort::matching(&command));
        contexts.set_admission_and_reduction_fence(fence(2));
        let no_effects = Arc::new(TestEffectPort::committed());
        let recovery_actor = ActorId::from_bytes([0x71; 16]);
        let (recovery, recovery_task) = spawn_fabric_command_actor(
            config(2),
            recovery_actor,
            records.clone(),
            contexts.clone(),
            no_effects.clone(),
        );
        let unknown = UnknownCommit {
            reason: UnknownCommitReason::ProcessInterrupted,
            diagnostic: DiagnosticRef::from_bytes([0x72; 32]),
        };
        let awaiting = recovery
            .mark_interrupted_commit(command.identity.operation_id, unknown)
            .await
            .expect("prepared transaction becomes explicit unknown without reexecution");
        assert!(matches!(
            awaiting.state(),
            DurableCommandState::AwaitingReconciliation {
                execution_owner: ExecutionOwner {
                    actor_id: execution_actor,
                    fence: execution_fence,
                },
                recovery_owner: ExecutionOwner {
                    actor_id: recovery_owner,
                    fence: recovery_fence,
                },
                transaction: observed_transaction,
                unknown: observed_unknown,
                ..
            } if execution_actor == actor_id()
                && execution_fence == fence(1)
                && recovery_owner == recovery_actor
                && recovery_fence == fence(2)
                && observed_transaction == transaction()
                && observed_unknown == unknown
        ));
        no_effects.queue_reconciliation(ReconciliationObservation::NotCommitted {
            evidence: ReconciliationEvidenceRef::from_bytes([0x73; 32]),
        });
        let retry_ready = recovery
            .reconcile(command.identity.operation_id)
            .await
            .expect("marker readback, not interruption, authorizes retry");
        assert!(matches!(
            retry_ready.state(),
            DurableCommandState::RetryReady {
                required_fence: observed,
                ..
            } if observed == fence(2)
        ));
        assert_eq!(no_effects.prepare_calls(), 0);
        assert_eq!(no_effects.commit_calls(), 0);
        assert_eq!(no_effects.reconcile_calls(), 1);
        recovery
            .shutdown()
            .await
            .expect("first recovery actor stops");
        recovery_task.await.expect("first recovery task joins");

        contexts.set_admission_and_reduction_fence(fence(3));
        let retry_effects = Arc::new(TestEffectPort::committed());
        let (retry_actor, retry_task) = spawn_fabric_command_actor(
            config(1),
            ActorId::from_bytes([0x74; 16]),
            records,
            contexts,
            retry_effects.clone(),
        );
        let succeeded = retry_actor
            .retry_proved_not_committed(command.identity.operation_id)
            .await
            .expect("later writer retries only after proved non-commit");
        assert_eq!(
            succeeded.state(),
            DurableCommandState::Succeeded {
                transaction: transaction(),
                result: command_result(&command),
                confirmation: super::super::command::CommitConfirmation::Direct,
            }
        );
        assert_eq!(retry_effects.prepare_calls(), 1);
        assert_eq!(retry_effects.commit_calls(), 1);
        drop(retry_actor);
        retry_task.await.expect("retry actor exits");
    }

    #[tokio::test]
    async fn durable_revision_conflict_stops_before_effects() {
        let command = test_command(6);
        let records = Arc::new(MemoryRecordPort::default());
        records.conflict_next_cas();
        let contexts = Arc::new(MemoryContextPort::matching(&command));
        let effects = Arc::new(TestEffectPort::committed());
        let (handle, task) = spawn_fabric_command_actor(
            config(1),
            actor_id(),
            records.clone(),
            contexts,
            effects.clone(),
        );

        let error = handle
            .submit(command)
            .await
            .expect_err("injected CAS conflict must surface");

        assert_eq!(
            error,
            FabricCommandActorError::RevisionConflict {
                expected: 0,
                observed: Some(0),
            }
        );
        assert_eq!(effects.prepare_calls(), 0);
        assert_eq!(effects.commit_calls(), 0);
        let admitted = records
            .record(command.identity.operation_id)
            .expect("admission remains durable");
        assert_eq!(admitted.revision(), 0);
        assert!(matches!(
            admitted.state(),
            DurableCommandState::Admitted { attempt: 1 }
        ));
        drop(handle);
        task.await.expect("actor exits after handle closes");
    }

    #[tokio::test]
    async fn cancellation_is_durable_only_before_commit_preparation() {
        let cancellation = CommandCancellation {
            diagnostic: DiagnosticRef::from_bytes([0x70; 32]),
        };
        let cancellable = test_command(7);
        let records = Arc::new(MemoryRecordPort::default());
        let contexts = Arc::new(MemoryContextPort::matching(&cancellable));
        let effects = Arc::new(TestEffectPort::cancelled(cancellation));
        let (handle, task) =
            spawn_fabric_command_actor(config(1), actor_id(), records, contexts, effects.clone());

        let cancelled = handle
            .submit(cancellable)
            .await
            .expect("preparation may report pre-commit cancellation");
        assert!(matches!(
            cancelled.state(),
            DurableCommandState::Cancelled {
                cancellation: observed,
            } if observed == cancellation
        ));
        assert_eq!(effects.commit_calls(), 0);
        drop(handle);
        task.await.expect("actor exits after handle closes");

        let prepared = test_command(8);
        let unknown = UnknownCommit {
            reason: UnknownCommitReason::ReadbackUnavailable,
            diagnostic: DiagnosticRef::from_bytes([0x71; 32]),
        };
        let records = Arc::new(MemoryRecordPort::default());
        let contexts = Arc::new(MemoryContextPort::matching(&prepared));
        let effects = Arc::new(TestEffectPort::unknown(unknown));
        let (handle, task) =
            spawn_fabric_command_actor(config(1), actor_id(), records, contexts, effects);
        let awaiting = handle
            .submit(prepared)
            .await
            .expect("command reaches reconciliation state");
        assert!(matches!(
            awaiting.state(),
            DurableCommandState::AwaitingReconciliation { .. }
        ));

        let error = handle
            .cancel(prepared.identity.operation_id, cancellation)
            .await
            .expect_err("prepared or unknown commit cannot be cancelled");
        assert!(matches!(
            error,
            FabricCommandActorError::Contract(CommandContractError::IllegalTransition { .. })
        ));
        drop(handle);
        task.await.expect("actor exits after handle closes");
    }

    #[tokio::test]
    async fn ordered_shutdown_closes_every_ingress_clone() {
        let command = test_command(9);
        let records = Arc::new(MemoryRecordPort::default());
        let contexts = Arc::new(MemoryContextPort::matching(&command));
        let effects = Arc::new(TestEffectPort::committed());
        let (handle, task) =
            spawn_fabric_command_actor(config(2), actor_id(), records, contexts, effects);
        let clone = handle.clone();

        handle
            .shutdown()
            .await
            .expect("actor acknowledges shutdown");
        task.await.expect("actor task exits after acknowledgement");
        assert_eq!(
            clone.submit(command).await,
            Err(FabricCommandActorError::QueueClosed)
        );
    }
}
