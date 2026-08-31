//! Production lifecycle for the single durable [`super::command::FabricCommand`] actor.
//!
//! The runtime composes the OS-backed workspace lease, monotonic SQLite writer generation,
//! canonical SQLite command journal, lease-bound context reads, bounded actor queue, and caller-
//! supplied typed effect port. Holding this value is proof that the local process still owns the
//! independent OS lease; every reducer context is revalidated against durable generation state.

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;
use tokio::task::JoinHandle;

use super::command::{
    ActorId, AdmissionContext, AuthorizationDecision, CommandRecord, CommandRecoveryObligation,
    DiagnosticRef, ExpectedHead, FabricCommand, LeaseId, OperationId, ReductionContext,
    TransactionRef, UnknownCommit, UnknownCommitReason, WorkspaceId, WriterFence,
};
use super::command_actor::{
    CommandPortError, CommandReductionContextPort, FabricCommandActorConfig,
    FabricCommandActorError, FabricCommandActorHandle, FabricCommandEffectPort,
    FabricCommandIngressGate, spawn_fabric_command_actor_closed,
};
use super::command_record_sqlite::{
    CommandRecoveryPage, CommandRecoveryPageSize, SqliteCommandRecordOpenError,
    SqliteCommandRecordStore,
};
use super::writer_generation_sqlite::{
    SqliteWriterGenerationOpenError, SqliteWriterGenerationStore,
};
use super::writer_lease::{WorkspaceWriterLease, WorkspaceWriterLeaseError, validate_writer_fence};

/// Immutable paths and identities used to start one workspace mutation runtime.
#[derive(Clone, Debug)]
pub struct FabricCommandRuntimeConfig {
    pub admin_root: PathBuf,
    pub writer_generation_database: PathBuf,
    pub command_record_database: PathBuf,
    pub workspace_id: WorkspaceId,
    pub lease_id: LeaseId,
    pub actor_id: ActorId,
    pub actor: FabricCommandActorConfig,
}

impl FabricCommandRuntimeConfig {
    /// Build an explicit runtime configuration without inferring paths from semantic identities.
    #[must_use]
    pub fn new(
        admin_root: impl Into<PathBuf>,
        writer_generation_database: impl Into<PathBuf>,
        command_record_database: impl Into<PathBuf>,
        workspace_id: WorkspaceId,
        lease_id: LeaseId,
        actor_id: ActorId,
        actor: FabricCommandActorConfig,
    ) -> Self {
        Self {
            admin_root: admin_root.into(),
            writer_generation_database: writer_generation_database.into(),
            command_record_database: command_record_database.into(),
            workspace_id,
            lease_id,
            actor_id,
            actor,
        }
    }
}

/// Failures before a command runtime becomes capable of admitting effects.
#[derive(Debug, Error)]
pub enum FabricCommandRuntimeStartError {
    #[error("a Tokio runtime is required to own the command actor")]
    TokioRuntimeUnavailable,
    #[error(transparent)]
    WriterGeneration(#[from] SqliteWriterGenerationOpenError),
    #[error(transparent)]
    CommandRecord(#[from] SqliteCommandRecordOpenError),
    #[error(transparent)]
    WriterLease(#[from] WorkspaceWriterLeaseError),
}

/// Failures while explicitly stopping an admitted command runtime.
#[derive(Debug, Error)]
pub enum FabricCommandRuntimeShutdownError {
    #[error(transparent)]
    Actor(#[from] FabricCommandActorError),
    #[error("command actor join failed: {0}")]
    Join(#[from] tokio::task::JoinError),
}

/// Failures while reopening new-command admission after startup or an unknown outcome.
#[derive(Debug, Error)]
pub enum FabricCommandAdmissionOpenError {
    #[error(transparent)]
    WriterAuthority(#[from] WorkspaceWriterLeaseError),
    #[error(transparent)]
    CommandStore(#[from] CommandPortError),
    #[error("command recovery remains pending for operation {operation_id:?}: {obligation:?}")]
    RecoveryPending {
        operation_id: OperationId,
        obligation: CommandRecoveryObligation,
    },
    #[error(transparent)]
    Actor(FabricCommandActorError),
}

/// Read-only source for the exact diagnostic attached when recovery discovers a prepared commit.
///
/// The port may resolve an application-owned diagnostic relation or immutable operational event;
/// it must not retry the target transaction or mutate semantic state. The runtime fixes the
/// unknown reason to `ProcessInterrupted`, so a caller cannot misclassify the recovery state.
#[async_trait]
pub trait InterruptedCommitDiagnosticPort: Send + Sync {
    async fn interruption_diagnostic(
        &self,
        prepared: &CommandRecord,
        transaction: TransactionRef,
        active_fence: WriterFence,
    ) -> Result<DiagnosticRef, CommandPortError>;
}

/// Fresh semantic and policy facts read independently of local writer ownership.
///
/// The command runtime, not this value, injects the OS-lease-backed writer fence. Keeping the
/// fence out of this port makes construction order sound: a semantic reader can be installed
/// before [`FabricCommandRuntime::start`] acquires the lease and allocates the next generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticAdmissionContext {
    pub workspace_id: WorkspaceId,
    pub current_head: ExpectedHead,
    pub authorization: AuthorizationDecision,
}

/// Authoritative semantic-current and authorization reader for command execution.
///
/// Implementations read the activation chain/catalog and policy facts afresh. They do not read or
/// infer local lease state and must not use SQLite command records as semantic-current authority.
#[async_trait]
pub trait CommandSemanticContextPort: Send + Sync {
    async fn read_admission_semantics(
        &self,
        command: &FabricCommand,
    ) -> Result<SemanticAdmissionContext, CommandPortError>;

    async fn read_current_head(
        &self,
        record: &CommandRecord,
    ) -> Result<ExpectedHead, CommandPortError>;
}

/// One command advanced by a bounded restart-recovery pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandRecoveryTransition {
    operation_id: OperationId,
    prior_obligation: CommandRecoveryObligation,
    resulting_record: CommandRecord,
}

impl CommandRecoveryTransition {
    /// Stable operation advanced by this pass.
    #[must_use]
    pub const fn operation_id(self) -> OperationId {
        self.operation_id
    }

    /// State-derived action selected before invoking the actor.
    #[must_use]
    pub const fn prior_obligation(self) -> CommandRecoveryObligation {
        self.prior_obligation
    }

    /// Authoritative record returned after the serialized recovery action.
    #[must_use]
    pub const fn resulting_record(self) -> CommandRecord {
        self.resulting_record
    }
}

/// Bounded deterministic restart work plus the exclusive cursor for the next page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandRecoveryPass {
    transitions: Vec<CommandRecoveryTransition>,
    next_after: Option<OperationId>,
}

/// Bounded startup-recovery result. `Pending` keeps command admission closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FabricCommandStartupRecoveryState {
    Ready,
    Pending {
        operation_id: OperationId,
        obligation: CommandRecoveryObligation,
    },
}

/// Work performed while converging the command journal before opening ingress.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FabricCommandStartupRecovery {
    state: FabricCommandStartupRecoveryState,
    sweeps: usize,
    transitions: usize,
}

impl FabricCommandStartupRecovery {
    /// Whether startup proved the journal quiescent or stopped on bounded pending work.
    #[must_use]
    pub const fn state(self) -> FabricCommandStartupRecoveryState {
        self.state
    }

    /// Complete canonical sweeps executed.
    #[must_use]
    pub const fn sweeps(self) -> usize {
        self.sweeps
    }

    /// State-derived recovery actions executed across all sweeps.
    #[must_use]
    pub const fn transitions(self) -> usize {
        self.transitions
    }
}

impl CommandRecoveryPass {
    /// Recovery actions performed in canonical operation-ID order.
    #[must_use]
    pub fn transitions(&self) -> &[CommandRecoveryTransition] {
        &self.transitions
    }

    /// Cursor for more records from this sweep. Start a new sweep at `None` after exhaustion
    /// because an indeterminate or retry-ready result remains deliberately nonterminal.
    #[must_use]
    pub const fn next_after(&self) -> Option<OperationId> {
        self.next_after
    }
}

/// Fail-closed restart-recovery errors.
#[derive(Debug, Error)]
pub enum FabricCommandRecoveryError {
    #[error(transparent)]
    WriterAuthority(#[from] WorkspaceWriterLeaseError),
    #[error("command journal returned a terminal record in its nonterminal projection: {0:?}")]
    TerminalRecord(OperationId),
    #[error("interrupted-commit diagnostic evidence is unavailable: {0}")]
    InterruptionDiagnostic(CommandPortError),
    #[error("command recovery journal read failed: {0}")]
    Journal(CommandPortError),
    #[error(transparent)]
    Actor(#[from] FabricCommandActorError),
}

/// Failures while performing bounded startup convergence and opening ingress.
#[derive(Debug, Error)]
pub enum FabricCommandStartupRecoveryError {
    #[error(transparent)]
    Recovery(#[from] FabricCommandRecoveryError),
    #[error(transparent)]
    Admission(#[from] FabricCommandAdmissionOpenError),
    #[error("startup recovery transition counter overflowed")]
    TransitionCountOverflow,
}

struct LeaseBoundCommandContextPort {
    workspace_id: WorkspaceId,
    fence: WriterFence,
    generations: Arc<SqliteWriterGenerationStore>,
    semantics: Arc<dyn CommandSemanticContextPort>,
}

impl LeaseBoundCommandContextPort {
    fn validate_active_authority(&self) -> Result<(), CommandPortError> {
        validate_writer_fence(self.generations.as_ref(), self.workspace_id, self.fence)
            .map_err(|_| CommandPortError::ContextUnavailable)
    }

    fn validate_admission(&self, command: &FabricCommand) -> Result<(), CommandPortError> {
        if command.ownership.workspace_id != self.workspace_id || command.writer_fence != self.fence
        {
            return Err(CommandPortError::ContextUnavailable);
        }
        self.validate_active_authority()
    }

    fn validate_reduction(&self, command: &FabricCommand) -> Result<(), CommandPortError> {
        if command.ownership.workspace_id != self.workspace_id
            || (command.writer_fence != self.fence
                && self.fence.generation.get() <= command.writer_fence.generation.get())
        {
            return Err(CommandPortError::ContextUnavailable);
        }
        self.validate_active_authority()
    }
}

#[async_trait]
impl CommandReductionContextPort for LeaseBoundCommandContextPort {
    async fn read_admission_context(
        &self,
        command: &FabricCommand,
    ) -> Result<AdmissionContext, CommandPortError> {
        self.validate_admission(command)?;
        let semantics = self.semantics.read_admission_semantics(command).await?;
        if semantics.workspace_id != self.workspace_id {
            return Err(CommandPortError::ContextUnavailable);
        }
        Ok(AdmissionContext {
            workspace_id: semantics.workspace_id,
            current_head: semantics.current_head,
            active_fence: self.fence,
            authorization: semantics.authorization,
        })
    }

    async fn read_reduction_context(
        &self,
        record: &super::command::CommandRecord,
    ) -> Result<ReductionContext, CommandPortError> {
        self.validate_reduction(record.command())?;
        let current_head = self.semantics.read_current_head(record).await?;
        Ok(ReductionContext {
            current_head,
            active_fence: self.fence,
        })
    }
}

/// Live owner of the one local durable mutation actor and its independent fencing substrates.
pub struct FabricCommandRuntime {
    workspace_id: WorkspaceId,
    fence: WriterFence,
    handle: FabricCommandActorHandle,
    ingress_gate: Arc<FabricCommandIngressGate>,
    actor_task: Option<JoinHandle<()>>,
    record_store: Arc<SqliteCommandRecordStore>,
    generation_store: Arc<SqliteWriterGenerationStore>,
    writer_lease: Option<WorkspaceWriterLease>,
}

impl std::fmt::Debug for FabricCommandRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FabricCommandRuntime")
            .field("workspace_id", &self.workspace_id)
            .field("fence", &self.fence)
            .field("record_store", &self.record_store.database_path())
            .field("generation_store", &self.generation_store.database_path())
            .finish_non_exhaustive()
    }
}

impl FabricCommandRuntime {
    /// Acquire the workspace writer authority and start one bounded serial actor.
    ///
    /// The caller supplies the authoritative semantic/policy reader and typed effect executor;
    /// this runtime wraps every context read with its independently held OS lease and durable
    /// generation. No actor starts if either temporal database or the lease cannot be proved.
    ///
    /// # Errors
    ///
    /// Returns exact database/lease failures and refuses construction outside a Tokio runtime.
    pub fn start(
        config: FabricCommandRuntimeConfig,
        semantics: Arc<dyn CommandSemanticContextPort>,
        effects: Arc<dyn FabricCommandEffectPort>,
    ) -> Result<Self, FabricCommandRuntimeStartError> {
        tokio::runtime::Handle::try_current()
            .map_err(|_| FabricCommandRuntimeStartError::TokioRuntimeUnavailable)?;
        let generation_store = Arc::new(SqliteWriterGenerationStore::open(
            &config.writer_generation_database,
        )?);
        let writer_lease = WorkspaceWriterLease::acquire(
            &config.admin_root,
            config.workspace_id,
            config.lease_id,
            generation_store.as_ref(),
        )?;
        let fence = writer_lease.fence();
        let record_store = Arc::new(SqliteCommandRecordStore::open(
            &config.command_record_database,
        )?);
        let fenced_contexts: Arc<dyn CommandReductionContextPort> =
            Arc::new(LeaseBoundCommandContextPort {
                workspace_id: config.workspace_id,
                fence,
                generations: Arc::clone(&generation_store),
                semantics,
            });
        let records = Arc::clone(&record_store);
        let (handle, actor_task, ingress_gate) = spawn_fabric_command_actor_closed(
            config.actor,
            config.actor_id,
            records,
            fenced_contexts,
            effects,
        );
        Ok(Self {
            workspace_id: config.workspace_id,
            fence,
            handle,
            ingress_gate,
            actor_task: Some(actor_task),
            record_store,
            generation_store,
            writer_lease: Some(writer_lease),
        })
    }

    /// Workspace whose mutation authority is held by this runtime.
    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    /// Exact lease/generation pair commands must carry.
    #[must_use]
    pub const fn fence(&self) -> WriterFence {
        self.fence
    }

    /// Cloneable bounded command port to the one serial actor.
    ///
    /// The actor itself rejects new submissions while admission is closed. Recovery operations
    /// remain available through [`Self::recover_nonterminal_page`]; callers reopen admission only
    /// after deterministic sweeps leave no nonterminal record.
    #[must_use]
    pub fn handle(&self) -> FabricCommandActorHandle {
        self.handle.clone()
    }

    /// Reopen new-command admission only when durable recovery has no remaining work.
    ///
    /// The gate closes before authority and journal checks, so concurrent queued submissions
    /// cannot cross the proof boundary. The actor rechecks the same gate when processing each
    /// submit message. Any later submit that finishes nonterminal closes it again.
    ///
    /// # Errors
    ///
    /// Returns stale writer authority, unavailable/corrupt temporal state, or the first exact
    /// recovery obligation. Admission remains closed on every error.
    pub async fn open_command_admission(
        &self,
    ) -> Result<FabricCommandActorHandle, FabricCommandAdmissionOpenError> {
        self.ingress_gate.close();
        self.validate_writer_authority()?;
        self.handle
            .open_admission_after_recovery()
            .await
            .map_err(|error| match error {
                FabricCommandActorError::RecoveryPending {
                    operation_id,
                    obligation,
                } => FabricCommandAdmissionOpenError::RecoveryPending {
                    operation_id,
                    obligation,
                },
                FabricCommandActorError::Port(error) => {
                    FabricCommandAdmissionOpenError::CommandStore(error)
                }
                error => FabricCommandAdmissionOpenError::Actor(error),
            })?;
        Ok(self.handle.clone())
    }

    /// Re-read durable generation authority while the independent OS lease remains held.
    ///
    /// # Errors
    ///
    /// Returns stale/unavailable generation authority instead of treating process ownership as a
    /// sufficient fence.
    pub fn validate_writer_authority(&self) -> Result<(), WorkspaceWriterLeaseError> {
        self.writer_lease
            .as_ref()
            .ok_or(WorkspaceWriterLeaseError::StaleFence)?
            .validate(self.generation_store.as_ref())
    }

    /// Read one bounded, deterministic page of nonterminal commands for recovery.
    ///
    /// The returned records are temporal evidence only. This method deliberately does not retry,
    /// cancel, or otherwise reinterpret an unknown operation outcome. Use
    /// [`Self::recover_nonterminal_page`] to advance them through the actor and its exact
    /// effect-owned marker/control-history reconciler.
    ///
    /// # Errors
    ///
    /// Returns unavailable or noncanonical journal state instead of omitting an undecodable row.
    pub async fn load_nonterminal_commands(
        &self,
        after: Option<super::command::OperationId>,
        page_size: CommandRecoveryPageSize,
    ) -> Result<CommandRecoveryPage, CommandPortError> {
        self.record_store
            .load_nonterminal_page(after, page_size)
            .await
    }

    /// Execute one bounded, deterministic restart-recovery page through the serial actor.
    ///
    /// Each row selects its action solely from [`CommandRecord::recovery_obligation`]. Prepared
    /// commits are first made durably unknown with a port-owned diagnostic; unknown commits use
    /// the effect port's exact marker/control-history reconciliation; only a proved-not-committed
    /// `RetryReady` row can re-enter effects. New-command admission remains closed throughout.
    ///
    /// This method intentionally performs at most one state-derived action per row. Callers sweep
    /// all cursors, restart from `None` while nonterminal rows remain, and invoke
    /// [`Self::open_command_admission`] only after a sweep leaves no recovery work. This bounds
    /// indeterminate marker polling and prevents one operation from starving later rows.
    ///
    /// # Errors
    ///
    /// Returns stale writer authority, corrupt/unavailable journal state, unavailable interruption
    /// evidence, or the exact serialized actor/reducer/effect failure. No error reopens admission.
    pub async fn recover_nonterminal_page(
        &self,
        after: Option<OperationId>,
        page_size: CommandRecoveryPageSize,
        diagnostics: &dyn InterruptedCommitDiagnosticPort,
    ) -> Result<CommandRecoveryPass, FabricCommandRecoveryError> {
        self.ingress_gate.close();
        self.validate_writer_authority()?;
        let page = self
            .load_nonterminal_commands(after, page_size)
            .await
            .map_err(FabricCommandRecoveryError::Journal)?;
        let next_after = page.next_after();
        let mut transitions = Vec::with_capacity(page.records().len());
        for record in page.records() {
            let operation_id = record.command().identity.operation_id;
            let obligation = record
                .recovery_obligation()
                .ok_or(FabricCommandRecoveryError::TerminalRecord(operation_id))?;
            let resulting_record = match obligation {
                CommandRecoveryObligation::ResumePrecommit => {
                    self.handle.resume_precommit(operation_id).await?
                }
                CommandRecoveryObligation::MarkInterruptedCommit { transaction } => {
                    let diagnostic = diagnostics
                        .interruption_diagnostic(record, transaction, self.fence)
                        .await
                        .map_err(FabricCommandRecoveryError::InterruptionDiagnostic)?;
                    self.handle
                        .mark_interrupted_commit(
                            operation_id,
                            UnknownCommit {
                                reason: UnknownCommitReason::ProcessInterrupted,
                                diagnostic,
                            },
                        )
                        .await?
                }
                CommandRecoveryObligation::ReconcileCommit { .. } => {
                    self.handle.reconcile(operation_id).await?
                }
                CommandRecoveryObligation::RetryProvedNotCommitted => {
                    self.handle.retry_proved_not_committed(operation_id).await?
                }
            };
            transitions.push(CommandRecoveryTransition {
                operation_id,
                prior_obligation: obligation,
                resulting_record,
            });
        }
        Ok(CommandRecoveryPass {
            transitions,
            next_after,
        })
    }

    /// Converge a bounded number of complete recovery sweeps, then open admission only after the
    /// journal proves quiescent.
    ///
    /// A sweep visits every nonterminal operation once in canonical operation-ID order. Multiple
    /// sweeps are necessary for crash states that must first become explicitly unknown, then be
    /// reconciled, and only then retry after proved non-commit. The nonzero sweep limit prevents
    /// an indeterminate backend from turning daemon startup into an unbounded polling loop.
    /// `Pending` is a successful fail-closed result: the runtime remains live for later recovery
    /// calls, but its command ingress stays closed.
    ///
    /// # Errors
    ///
    /// Returns exact writer, journal, diagnostic, actor, or admission failures. It never converts
    /// an indeterminate observation into a retry or opens ingress after partial traversal.
    pub async fn recover_and_open_bounded(
        &self,
        page_size: CommandRecoveryPageSize,
        maximum_sweeps: NonZeroUsize,
        diagnostics: &dyn InterruptedCommitDiagnosticPort,
    ) -> Result<FabricCommandStartupRecovery, FabricCommandStartupRecoveryError> {
        let mut transitions = 0_usize;
        for sweep_index in 0..maximum_sweeps.get() {
            let mut after = None;
            loop {
                let pass = self
                    .recover_nonterminal_page(after, page_size, diagnostics)
                    .await?;
                transitions = transitions
                    .checked_add(pass.transitions().len())
                    .ok_or(FabricCommandStartupRecoveryError::TransitionCountOverflow)?;
                let Some(next_after) = pass.next_after() else {
                    break;
                };
                after = Some(next_after);
            }

            match self.open_command_admission().await {
                Ok(_) => {
                    return Ok(FabricCommandStartupRecovery {
                        state: FabricCommandStartupRecoveryState::Ready,
                        sweeps: sweep_index + 1,
                        transitions,
                    });
                }
                Err(FabricCommandAdmissionOpenError::RecoveryPending {
                    operation_id,
                    obligation,
                }) if sweep_index + 1 == maximum_sweeps.get() => {
                    return Ok(FabricCommandStartupRecovery {
                        state: FabricCommandStartupRecoveryState::Pending {
                            operation_id,
                            obligation,
                        },
                        sweeps: sweep_index + 1,
                        transitions,
                    });
                }
                Err(FabricCommandAdmissionOpenError::RecoveryPending { .. }) => {}
                Err(error) => return Err(error.into()),
            }
        }
        unreachable!("a nonzero recovery sweep bound always returns from the loop")
    }

    /// Stop the actor in queue order and wait for its task before releasing the writer lease.
    ///
    /// # Errors
    ///
    /// Returns actor lifecycle or Tokio join failures. It does not rewrite any nonterminal command
    /// record during shutdown.
    pub async fn shutdown(mut self) -> Result<(), FabricCommandRuntimeShutdownError> {
        self.handle.shutdown().await?;
        if let Some(task) = self.actor_task.take() {
            task.await?;
        }
        self.writer_lease.take();
        Ok(())
    }

    /// Exact command journal path, exposed for bounded administrative backup/recovery tooling.
    #[must_use]
    pub fn command_record_database(&self) -> &Path {
        self.record_store.database_path()
    }
}

impl Drop for FabricCommandRuntime {
    fn drop(&mut self) {
        if let Some(task) = self.actor_task.take() {
            task.abort();
            // `abort` is cooperative until Tokio next polls the task. Keep the independent OS
            // writer lock held for the remainder of this process instead of permitting a second
            // writer to overlap an effect whose future has not yet been dropped. Explicit
            // `shutdown` is the only path that releases the lease in-process.
            if let Some(lease) = self.writer_lease.take() {
                std::mem::forget(lease);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;
    use crate::fabric::command::{
        AuthorizationDecision, AuthorizationRef, CommandFailure, CommandIdentity, CommandOwnership,
        CommandPins, CompilerReleaseRef, DurableCommandState, EpochId, ExecutionOwner,
        ExpectedHead, FabricCommandPayload, FailureClass, FailureCode, IdempotencyKey,
        ModelHeadRef, OperationId, PrincipalId, ProofReceiptRef, ProviderSetRef,
        ReconciliationEvidenceRef, ReconciliationObservation, ResourceEnvelopeRef,
        SourceGeneration, TransactionRef,
    };
    use crate::fabric::command_actor::{CommitEffectOutcome, PrepareEffectOutcome};

    struct CurrentContext;

    #[async_trait]
    impl CommandSemanticContextPort for CurrentContext {
        async fn read_admission_semantics(
            &self,
            command: &FabricCommand,
        ) -> Result<SemanticAdmissionContext, CommandPortError> {
            Ok(SemanticAdmissionContext {
                workspace_id: command.ownership.workspace_id,
                current_head: command.expected_head,
                authorization: AuthorizationDecision::Authorized(command.ownership.authorization),
            })
        }

        async fn read_current_head(
            &self,
            record: &CommandRecord,
        ) -> Result<ExpectedHead, CommandPortError> {
            Ok(record.command().expected_head)
        }
    }

    struct UnusedEffects;

    #[async_trait]
    impl FabricCommandEffectPort for UnusedEffects {
        async fn prepare(
            &self,
            _executing: &CommandRecord,
            _owner: ExecutionOwner,
            _context: ReductionContext,
        ) -> Result<PrepareEffectOutcome, CommandPortError> {
            Err(CommandPortError::EffectUnavailable)
        }

        async fn commit(
            &self,
            _prepared: &CommandRecord,
            _owner: ExecutionOwner,
            _transaction: TransactionRef,
            _context: ReductionContext,
        ) -> Result<CommitEffectOutcome, CommandPortError> {
            Err(CommandPortError::EffectUnavailable)
        }

        async fn reconcile(
            &self,
            _awaiting: &CommandRecord,
            _owner: ExecutionOwner,
            _transaction: TransactionRef,
            _context: ReductionContext,
        ) -> Result<super::super::command::ReconciliationObservation, CommandPortError> {
            Err(CommandPortError::EffectUnavailable)
        }
    }

    struct PrepareThenLoseCommitEffects;

    #[async_trait]
    impl FabricCommandEffectPort for PrepareThenLoseCommitEffects {
        async fn prepare(
            &self,
            _executing: &CommandRecord,
            _owner: ExecutionOwner,
            _context: ReductionContext,
        ) -> Result<PrepareEffectOutcome, CommandPortError> {
            Ok(PrepareEffectOutcome::Prepared {
                transaction: transaction(),
            })
        }

        async fn commit(
            &self,
            _prepared: &CommandRecord,
            _owner: ExecutionOwner,
            _transaction: TransactionRef,
            _context: ReductionContext,
        ) -> Result<CommitEffectOutcome, CommandPortError> {
            Err(CommandPortError::EffectUnavailable)
        }

        async fn reconcile(
            &self,
            _awaiting: &CommandRecord,
            _owner: ExecutionOwner,
            _transaction: TransactionRef,
            _context: ReductionContext,
        ) -> Result<ReconciliationObservation, CommandPortError> {
            Err(CommandPortError::EffectUnavailable)
        }
    }

    struct ProveNotCommittedThenFailBeforeRetryEffects;

    #[async_trait]
    impl FabricCommandEffectPort for ProveNotCommittedThenFailBeforeRetryEffects {
        async fn prepare(
            &self,
            _executing: &CommandRecord,
            _owner: ExecutionOwner,
            _context: ReductionContext,
        ) -> Result<PrepareEffectOutcome, CommandPortError> {
            Ok(PrepareEffectOutcome::KnownFailure {
                failure: CommandFailure {
                    code: FailureCode::BackendUnavailable,
                    class: FailureClass::Permanent,
                    diagnostic: DiagnosticRef::from_bytes([0x91; 32]),
                },
            })
        }

        async fn commit(
            &self,
            _prepared: &CommandRecord,
            _owner: ExecutionOwner,
            _transaction: TransactionRef,
            _context: ReductionContext,
        ) -> Result<CommitEffectOutcome, CommandPortError> {
            Err(CommandPortError::EffectUnavailable)
        }

        async fn reconcile(
            &self,
            _awaiting: &CommandRecord,
            _owner: ExecutionOwner,
            observed_transaction: TransactionRef,
            _context: ReductionContext,
        ) -> Result<ReconciliationObservation, CommandPortError> {
            assert_eq!(observed_transaction, transaction());
            Ok(ReconciliationObservation::NotCommitted {
                evidence: ReconciliationEvidenceRef::from_bytes([0x92; 32]),
            })
        }
    }

    struct AlwaysIndeterminateEffects;

    #[async_trait]
    impl FabricCommandEffectPort for AlwaysIndeterminateEffects {
        async fn prepare(
            &self,
            _executing: &CommandRecord,
            _owner: ExecutionOwner,
            _context: ReductionContext,
        ) -> Result<PrepareEffectOutcome, CommandPortError> {
            Err(CommandPortError::EffectUnavailable)
        }

        async fn commit(
            &self,
            _prepared: &CommandRecord,
            _owner: ExecutionOwner,
            _transaction: TransactionRef,
            _context: ReductionContext,
        ) -> Result<CommitEffectOutcome, CommandPortError> {
            Err(CommandPortError::EffectUnavailable)
        }

        async fn reconcile(
            &self,
            _awaiting: &CommandRecord,
            _owner: ExecutionOwner,
            observed_transaction: TransactionRef,
            _context: ReductionContext,
        ) -> Result<ReconciliationObservation, CommandPortError> {
            assert_eq!(observed_transaction, transaction());
            Ok(ReconciliationObservation::Indeterminate {
                evidence: ReconciliationEvidenceRef::from_bytes([0x93; 32]),
            })
        }
    }

    #[derive(Default)]
    struct InterruptedDiagnostic {
        calls: std::sync::atomic::AtomicUsize,
    }

    #[async_trait]
    impl InterruptedCommitDiagnosticPort for InterruptedDiagnostic {
        async fn interruption_diagnostic(
            &self,
            prepared: &CommandRecord,
            observed_transaction: TransactionRef,
            active_fence: WriterFence,
        ) -> Result<DiagnosticRef, CommandPortError> {
            assert!(matches!(
                prepared.state(),
                DurableCommandState::CommitPrepared { .. }
            ));
            assert_eq!(observed_transaction, transaction());
            assert_eq!(active_fence.generation.get(), 2);
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(DiagnosticRef::from_bytes([0x90; 32]))
        }
    }

    fn config(root: &Path, lease_seed: u8) -> FabricCommandRuntimeConfig {
        FabricCommandRuntimeConfig::new(
            root,
            root.join("writer-generations.sqlite3"),
            root.join("commands.sqlite3"),
            WorkspaceId::from_bytes([0x10; 16]),
            LeaseId::from_bytes([lease_seed; 16]),
            ActorId::from_bytes([lease_seed.wrapping_add(1); 16]),
            FabricCommandActorConfig::default(),
        )
    }

    fn fence(lease_seed: u8, generation: u64) -> WriterFence {
        WriterFence {
            lease_id: LeaseId::from_bytes([lease_seed; 16]),
            generation: super::super::command::WriterGeneration::new(generation)
                .expect("test generation is nonzero"),
        }
    }

    fn command(writer_fence: WriterFence) -> FabricCommand {
        FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes([0x20; 16]),
                idempotency_key: IdempotencyKey::from_bytes([0x21; 32]),
            },
            ownership: CommandOwnership {
                workspace_id: WorkspaceId::from_bytes([0x10; 16]),
                principal_id: PrincipalId::from_bytes([0x22; 16]),
                authorization: AuthorizationRef::from_bytes([0x23; 32]),
            },
            expected_head: ExpectedHead::Empty,
            writer_fence,
            pins: CommandPins {
                compiler_release: CompilerReleaseRef::from_bytes([0x24; 32]),
                model_head: ModelHeadRef::from_bytes([0x25; 32]),
                source_generation: SourceGeneration::new(0),
                provider_set: ProviderSetRef::from_bytes([0x26; 32]),
            },
            resources: ResourceEnvelopeRef::from_bytes([0x27; 32]),
            payload: FabricCommandPayload::ActivateEpoch {
                candidate_epoch: EpochId::from_bytes([0x28; 16]),
                proof_receipt: ProofReceiptRef::from_bytes([0x29; 32]),
            },
        }
    }

    fn transaction() -> TransactionRef {
        TransactionRef::from_bytes([0x30; 32])
    }

    #[tokio::test]
    async fn restart_allocates_new_generation_and_closes_the_prior_actor() {
        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let first = FabricCommandRuntime::start(
            config(root.path(), 1),
            Arc::new(CurrentContext),
            Arc::new(UnusedEffects),
        )
        .unwrap();
        assert_eq!(first.fence().generation.get(), 1);
        first.validate_writer_authority().unwrap();
        let recovery = first
            .load_nonterminal_commands(None, CommandRecoveryPageSize::new(1).unwrap())
            .await
            .unwrap();
        assert!(recovery.records().is_empty());
        assert_eq!(recovery.next_after(), None);
        let closed_handle = first.open_command_admission().await.unwrap();
        first.shutdown().await.unwrap();
        assert!(matches!(
            closed_handle.shutdown().await,
            Err(FabricCommandActorError::QueueClosed)
        ));

        let second = FabricCommandRuntime::start(
            config(root.path(), 2),
            Arc::new(CurrentContext),
            Arc::new(UnusedEffects),
        )
        .unwrap();
        assert_eq!(second.fence().generation.get(), 2);
        second.validate_writer_authority().unwrap();
        second.open_command_admission().await.unwrap();
        second.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn restart_adopts_a_precommit_record_only_after_generation_advances() {
        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let first = FabricCommandRuntime::start(
            config(root.path(), 1),
            Arc::new(CurrentContext),
            Arc::new(UnusedEffects),
        )
        .unwrap();
        let command = command(first.fence());
        assert!(matches!(
            first.handle().try_submit(command),
            Err(super::super::command_actor::FabricCommandBackpressure::RecoveryPending)
        ));
        assert!(matches!(
            first.handle().submit(command).await,
            Err(FabricCommandActorError::AdmissionClosedForRecovery)
        ));
        let ingress = first.open_command_admission().await.unwrap();
        assert!(matches!(
            ingress.submit(command).await,
            Err(FabricCommandActorError::Port(
                CommandPortError::EffectUnavailable
            ))
        ));
        let interrupted = first
            .load_nonterminal_commands(None, CommandRecoveryPageSize::new(1).unwrap())
            .await
            .unwrap();
        assert!(matches!(
            interrupted.records()[0].state(),
            DurableCommandState::Executing { owner, .. } if owner.fence == fence(1, 1)
        ));
        first.shutdown().await.unwrap();

        let second = FabricCommandRuntime::start(
            config(root.path(), 2),
            Arc::new(CurrentContext),
            Arc::new(UnusedEffects),
        )
        .unwrap();
        assert!(matches!(
            second.open_command_admission().await,
            Err(FabricCommandAdmissionOpenError::RecoveryPending {
                operation_id,
                obligation: CommandRecoveryObligation::ResumePrecommit,
            }) if operation_id == command.identity.operation_id
        ));
        assert!(matches!(
            second
                .handle()
                .resume_precommit(command.identity.operation_id)
                .await,
            Err(FabricCommandActorError::Port(
                CommandPortError::EffectUnavailable
            ))
        ));
        let adopted = second
            .load_nonterminal_commands(None, CommandRecoveryPageSize::new(1).unwrap())
            .await
            .unwrap();
        assert!(matches!(
            adopted.records()[0].state(),
            DurableCommandState::Executing { owner, .. } if owner.fence == fence(2, 2)
        ));
        assert_eq!(adopted.records()[0].revision(), 2);
        second.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn bounded_recovery_passes_mark_reconcile_and_retry_without_guessing() {
        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let first = FabricCommandRuntime::start(
            config(root.path(), 1),
            Arc::new(CurrentContext),
            Arc::new(PrepareThenLoseCommitEffects),
        )
        .unwrap();
        let command = command(first.fence());
        let ingress = first.open_command_admission().await.unwrap();
        assert!(matches!(
            ingress.submit(command).await,
            Err(FabricCommandActorError::Port(
                CommandPortError::EffectUnavailable
            ))
        ));
        let prepared = first
            .load_nonterminal_commands(None, CommandRecoveryPageSize::new(8).unwrap())
            .await
            .unwrap();
        assert!(matches!(
            prepared.records()[0].state(),
            DurableCommandState::CommitPrepared {
                transaction: observed,
                ..
            } if observed == transaction()
        ));
        first.shutdown().await.unwrap();

        let second = FabricCommandRuntime::start(
            config(root.path(), 2),
            Arc::new(CurrentContext),
            Arc::new(ProveNotCommittedThenFailBeforeRetryEffects),
        )
        .unwrap();
        let diagnostics = InterruptedDiagnostic::default();
        let page_size = CommandRecoveryPageSize::new(8).unwrap();

        let marked = second
            .recover_nonterminal_page(None, page_size, &diagnostics)
            .await
            .unwrap();
        assert_eq!(marked.transitions().len(), 1);
        assert!(matches!(
            marked.transitions()[0].prior_obligation(),
            CommandRecoveryObligation::MarkInterruptedCommit {
                transaction: observed
            } if observed == transaction()
        ));
        assert!(matches!(
            marked.transitions()[0].resulting_record().state(),
            DurableCommandState::AwaitingReconciliation {
                unknown: UnknownCommit {
                    reason: UnknownCommitReason::ProcessInterrupted,
                    ..
                },
                ..
            }
        ));
        assert_eq!(
            diagnostics.calls.load(std::sync::atomic::Ordering::SeqCst),
            1
        );

        let reconciled = second
            .recover_nonterminal_page(None, page_size, &diagnostics)
            .await
            .unwrap();
        assert!(matches!(
            reconciled.transitions()[0].prior_obligation(),
            CommandRecoveryObligation::ReconcileCommit {
                transaction: observed
            } if observed == transaction()
        ));
        assert!(matches!(
            reconciled.transitions()[0].resulting_record().state(),
            DurableCommandState::RetryReady { .. }
        ));

        let retried = second
            .recover_nonterminal_page(None, page_size, &diagnostics)
            .await
            .unwrap();
        assert!(matches!(
            retried.transitions()[0].prior_obligation(),
            CommandRecoveryObligation::RetryProvedNotCommitted
        ));
        assert!(matches!(
            retried.transitions()[0].resulting_record().state(),
            DurableCommandState::Failed { .. }
        ));

        let empty = second
            .recover_nonterminal_page(None, page_size, &diagnostics)
            .await
            .unwrap();
        assert!(empty.transitions().is_empty());
        second.open_command_admission().await.unwrap();
        second.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn bounded_startup_recovery_keeps_ingress_closed_on_indeterminate_history() {
        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let first = FabricCommandRuntime::start(
            config(root.path(), 1),
            Arc::new(CurrentContext),
            Arc::new(PrepareThenLoseCommitEffects),
        )
        .unwrap();
        let command = command(first.fence());
        let ingress = first.open_command_admission().await.unwrap();
        assert!(matches!(
            ingress.submit(command).await,
            Err(FabricCommandActorError::Port(
                CommandPortError::EffectUnavailable
            ))
        ));
        first.shutdown().await.unwrap();

        let second = FabricCommandRuntime::start(
            config(root.path(), 2),
            Arc::new(CurrentContext),
            Arc::new(AlwaysIndeterminateEffects),
        )
        .unwrap();
        let diagnostics = InterruptedDiagnostic::default();
        let recovery = second
            .recover_and_open_bounded(
                CommandRecoveryPageSize::new(1).unwrap(),
                NonZeroUsize::new(2).unwrap(),
                &diagnostics,
            )
            .await
            .unwrap();

        assert_eq!(recovery.sweeps(), 2);
        assert_eq!(recovery.transitions(), 2);
        assert!(matches!(
            recovery.state(),
            FabricCommandStartupRecoveryState::Pending {
                operation_id,
                obligation: CommandRecoveryObligation::ReconcileCommit {
                    transaction: observed
                },
            } if operation_id == command.identity.operation_id && observed == transaction()
        ));
        assert!(matches!(
            second.handle().submit(command).await,
            Err(FabricCommandActorError::AdmissionClosedForRecovery)
        ));
        second.shutdown().await.unwrap();
    }
}
