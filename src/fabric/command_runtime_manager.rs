//! Per-workspace lifecycle ownership for durable fabric command runtimes.
//!
//! This module is deliberately a composition seam, not a semantic backend. A caller-supplied
//! factory must provide the explicit runtime paths and identities, authoritative semantic reader,
//! complete typed effect closure, and interruption-diagnostic authority for each workspace. The
//! manager owns every resulting [`FabricCommandRuntime`] until joined shutdown and exposes command
//! ingress only after bounded recovery proves that admission is ready.

use std::collections::BTreeMap;
use std::error::Error as StdError;
use std::fmt;
use std::num::NonZeroUsize;
use std::sync::Arc;

use thiserror::Error;

use super::command::{CommandRecoveryObligation, OperationId, WorkspaceId, WriterFence};
use super::command_actor::{FabricCommandActorHandle, FabricCommandEffectPort};
use super::command_effect_router::FabricCommandEffectRouter;
use super::command_record_sqlite::CommandRecoveryPageSize;
use super::command_runtime::{
    CommandSemanticContextPort, FabricCommandRuntime, FabricCommandRuntimeConfig,
    FabricCommandRuntimeShutdownError, FabricCommandRuntimeStartError,
    FabricCommandStartupRecoveryError, FabricCommandStartupRecoveryState,
    InterruptedCommitDiagnosticPort,
};

/// Error returned while composing the explicit dependencies for one workspace runtime.
pub type WorkspaceFabricCommandRuntimeFactoryError = Box<dyn StdError + Send + Sync + 'static>;

/// Complete caller-owned inputs required to start one workspace command runtime.
#[derive(Clone)]
pub struct WorkspaceFabricCommandRuntimeParts {
    config: FabricCommandRuntimeConfig,
    semantics: Arc<dyn CommandSemanticContextPort>,
    effects: WorkspaceFabricCommandRuntimeEffects,
    interruption_diagnostics: Arc<dyn InterruptedCommitDiagnosticPort>,
}

#[derive(Clone)]
enum WorkspaceFabricCommandRuntimeEffects {
    CompleteRouter(Arc<FabricCommandEffectRouter>),
    #[cfg(test)]
    TestProbe(Arc<dyn FabricCommandEffectPort>),
}

impl WorkspaceFabricCommandRuntimeEffects {
    fn into_port(self) -> Arc<dyn FabricCommandEffectPort> {
        match self {
            Self::CompleteRouter(router) => router,
            #[cfg(test)]
            Self::TestProbe(probe) => probe,
        }
    }
}

impl fmt::Debug for WorkspaceFabricCommandRuntimeParts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkspaceFabricCommandRuntimeParts")
            .field("config", &self.config)
            .field("semantics", &"installed")
            .field("effects", &"installed")
            .field("interruption_diagnostics", &"installed")
            .finish()
    }
}

impl WorkspaceFabricCommandRuntimeParts {
    /// Bind one explicit runtime configuration to its complete production port closure.
    #[must_use]
    pub fn new(
        config: FabricCommandRuntimeConfig,
        semantics: Arc<dyn CommandSemanticContextPort>,
        effects: Arc<FabricCommandEffectRouter>,
        interruption_diagnostics: Arc<dyn InterruptedCommitDiagnosticPort>,
    ) -> Self {
        Self {
            config,
            semantics,
            effects: WorkspaceFabricCommandRuntimeEffects::CompleteRouter(effects),
            interruption_diagnostics,
        }
    }

    /// Workspace identity explicitly bound by this complete runtime closure.
    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.config.workspace_id
    }

    #[cfg(test)]
    fn new_for_tests(
        config: FabricCommandRuntimeConfig,
        semantics: Arc<dyn CommandSemanticContextPort>,
        effects: Arc<dyn FabricCommandEffectPort>,
        interruption_diagnostics: Arc<dyn InterruptedCommitDiagnosticPort>,
    ) -> Self {
        Self {
            config,
            semantics,
            effects: WorkspaceFabricCommandRuntimeEffects::TestProbe(effects),
            interruption_diagnostics,
        }
    }
}

/// Composition authority for one complete runtime dependency set per workspace.
///
/// The manager never infers paths, semantic identities, policy readers, or missing effect
/// families. A production implementation may capture daemon-owned catalogs and port factories,
/// but it must return the complete closure as one value.
pub trait WorkspaceFabricCommandRuntimeFactory: Send + Sync {
    /// Compose explicit dependencies for exactly `workspace_id`.
    ///
    /// # Errors
    ///
    /// Returns the factory's exact source error without starting or retaining a runtime.
    fn build(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceFabricCommandRuntimeParts, WorkspaceFabricCommandRuntimeFactoryError>;
}

/// Immutable production registry of complete workspace runtime closures.
///
/// Registration is deliberately all-or-nothing: a workspace appears in this factory only after
/// its paths, semantic-current reader, exhaustive typed effect router, and interruption
/// diagnostic authority have been composed. The factory is immutable after construction, so a
/// daemon startup observes one closed dependency set rather than a mutable service locator.
#[derive(Clone)]
pub struct RegisteredWorkspaceFabricCommandRuntimeFactory {
    registrations: Arc<BTreeMap<WorkspaceId, WorkspaceFabricCommandRuntimeParts>>,
}

impl fmt::Debug for RegisteredWorkspaceFabricCommandRuntimeFactory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RegisteredWorkspaceFabricCommandRuntimeFactory")
            .field("workspace_count", &self.registrations.len())
            .finish_non_exhaustive()
    }
}

/// Construction and lookup failures for the immutable production runtime registry.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RegisteredWorkspaceFabricCommandRuntimeFactoryError {
    #[error("duplicate fabric command runtime registration for workspace {0:?}")]
    DuplicateWorkspace(WorkspaceId),
    #[error("no complete fabric command runtime registration exists for workspace {0:?}")]
    WorkspaceNotRegistered(WorkspaceId),
}

impl RegisteredWorkspaceFabricCommandRuntimeFactory {
    /// Freeze a complete, duplicate-free set of per-workspace production runtime closures.
    ///
    /// # Errors
    ///
    /// Rejects duplicate workspace identities. An empty registry is valid for a daemon with no
    /// admitted workspaces; any later start request still fails closed as unregistered.
    pub fn try_new(
        registrations: impl IntoIterator<Item = WorkspaceFabricCommandRuntimeParts>,
    ) -> Result<Self, RegisteredWorkspaceFabricCommandRuntimeFactoryError> {
        let mut by_workspace = BTreeMap::new();
        for registration in registrations {
            let workspace_id = registration.workspace_id();
            if by_workspace.insert(workspace_id, registration).is_some() {
                return Err(
                    RegisteredWorkspaceFabricCommandRuntimeFactoryError::DuplicateWorkspace(
                        workspace_id,
                    ),
                );
            }
        }
        Ok(Self {
            registrations: Arc::new(by_workspace),
        })
    }

    /// Number of workspaces whose complete runtime dependency closure was frozen.
    #[must_use]
    pub fn len(&self) -> usize {
        self.registrations.len()
    }

    /// Whether no workspace has a complete frozen runtime dependency closure.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.registrations.is_empty()
    }

    /// Canonical workspace identities admitted by this startup registry.
    pub fn workspace_ids(&self) -> impl ExactSizeIterator<Item = WorkspaceId> + '_ {
        self.registrations.keys().copied()
    }
}

impl WorkspaceFabricCommandRuntimeFactory for RegisteredWorkspaceFabricCommandRuntimeFactory {
    fn build(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceFabricCommandRuntimeParts, WorkspaceFabricCommandRuntimeFactoryError> {
        self.registrations
            .get(&workspace_id)
            .cloned()
            .ok_or_else(|| {
                Box::new(
                    RegisteredWorkspaceFabricCommandRuntimeFactoryError::WorkspaceNotRegistered(
                        workspace_id,
                    ),
                ) as WorkspaceFabricCommandRuntimeFactoryError
            })
    }
}

/// Admission state retained for one managed workspace runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceFabricCommandRuntimeState {
    /// Recovery proved the temporal journal quiescent and actor ingress is open.
    Ready,
    /// Bounded recovery stopped with one exact nonterminal obligation; ingress remains closed.
    Pending {
        operation_id: OperationId,
        obligation: CommandRecoveryObligation,
    },
}

impl From<FabricCommandStartupRecoveryState> for WorkspaceFabricCommandRuntimeState {
    fn from(state: FabricCommandStartupRecoveryState) -> Self {
        match state {
            FabricCommandStartupRecoveryState::Ready => Self::Ready,
            FabricCommandStartupRecoveryState::Pending {
                operation_id,
                obligation,
            } => Self::Pending {
                operation_id,
                obligation,
            },
        }
    }
}

/// Ready-only command ingress plus the exact fence commands must carry.
#[derive(Clone)]
pub struct WorkspaceFabricCommandRuntimeHandle {
    workspace_id: WorkspaceId,
    fence: WriterFence,
    actor: FabricCommandActorHandle,
}

impl fmt::Debug for WorkspaceFabricCommandRuntimeHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkspaceFabricCommandRuntimeHandle")
            .field("workspace_id", &self.workspace_id)
            .field("fence", &self.fence)
            .finish_non_exhaustive()
    }
}

impl WorkspaceFabricCommandRuntimeHandle {
    /// Workspace whose ready ingress this value addresses.
    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    /// Exact local lease/generation authority commands submitted through this ingress must carry.
    #[must_use]
    pub const fn fence(&self) -> WriterFence {
        self.fence
    }

    /// Clone the bounded actor command port.
    #[must_use]
    pub fn actor(&self) -> FabricCommandActorHandle {
        self.actor.clone()
    }
}

struct ManagedWorkspaceRuntime {
    runtime: FabricCommandRuntime,
    interruption_diagnostics: Arc<dyn InterruptedCommitDiagnosticPort>,
    state: WorkspaceFabricCommandRuntimeState,
}

/// Failure while starting, recovering, or stopping one managed workspace runtime.
#[derive(Debug, Error)]
pub enum WorkspaceFabricCommandRuntimeManagerError {
    #[error("a fabric command runtime is already managed for workspace {0:?}")]
    DuplicateWorkspace(WorkspaceId),
    #[error("no fabric command runtime is managed for workspace {0:?}")]
    WorkspaceNotManaged(WorkspaceId),
    #[error("fabric command runtime factory failed for workspace {workspace_id:?}: {source}")]
    Factory {
        workspace_id: WorkspaceId,
        #[source]
        source: WorkspaceFabricCommandRuntimeFactoryError,
    },
    #[error(
        "fabric command runtime factory returned workspace {configured:?} for requested workspace {requested:?}"
    )]
    FactoryWorkspaceMismatch {
        requested: WorkspaceId,
        configured: WorkspaceId,
    },
    #[error("fabric command runtime failed to start for workspace {workspace_id:?}: {source}")]
    Start {
        workspace_id: WorkspaceId,
        #[source]
        source: FabricCommandRuntimeStartError,
    },
    #[error(
        "fabric command startup recovery failed for workspace {workspace_id:?}; the runtime was joined and stopped: {source}"
    )]
    StartupRecovery {
        workspace_id: WorkspaceId,
        #[source]
        source: FabricCommandStartupRecoveryError,
    },
    #[error(
        "fabric command startup recovery failed for workspace {workspace_id:?}: {recovery}; explicit shutdown also failed: {shutdown}"
    )]
    StartupRecoveryAndShutdown {
        workspace_id: WorkspaceId,
        recovery: FabricCommandStartupRecoveryError,
        shutdown: FabricCommandRuntimeShutdownError,
    },
    #[error("fabric command recovery retry failed for workspace {workspace_id:?}: {source}")]
    Recovery {
        workspace_id: WorkspaceId,
        #[source]
        source: FabricCommandStartupRecoveryError,
    },
    #[error("fabric command runtime shutdown failed for workspace {workspace_id:?}: {source}")]
    Shutdown {
        workspace_id: WorkspaceId,
        #[source]
        source: FabricCommandRuntimeShutdownError,
    },
}

/// One workspace whose runtime shutdown failed during an aggregate stop.
#[derive(Debug)]
pub struct WorkspaceFabricCommandRuntimeShutdownFailure {
    workspace_id: WorkspaceId,
    error: FabricCommandRuntimeShutdownError,
}

impl WorkspaceFabricCommandRuntimeShutdownFailure {
    /// Workspace whose runtime was nevertheless consumed and removed from the manager.
    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    /// Exact joined-shutdown failure.
    #[must_use]
    pub const fn error(&self) -> &FabricCommandRuntimeShutdownError {
        &self.error
    }
}

/// Aggregate proving that shutdown was attempted for every previously managed workspace.
#[derive(Debug)]
pub struct WorkspaceFabricCommandRuntimeShutdownFailures {
    failures: Vec<WorkspaceFabricCommandRuntimeShutdownFailure>,
}

impl WorkspaceFabricCommandRuntimeShutdownFailures {
    /// Failures in canonical workspace-ID order.
    #[must_use]
    pub fn failures(&self) -> &[WorkspaceFabricCommandRuntimeShutdownFailure] {
        &self.failures
    }
}

impl fmt::Display for WorkspaceFabricCommandRuntimeShutdownFailures {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} workspace fabric command runtime shutdown(s) failed",
            self.failures.len()
        )
    }
}

impl StdError for WorkspaceFabricCommandRuntimeShutdownFailures {}

/// Sole lifecycle owner for at most one durable fabric command runtime per workspace.
///
/// This value intentionally exposes neither raw runtimes nor a partial effect router. Explicit
/// [`Self::stop_workspace`] or [`Self::shutdown_all`] is required for joined actor shutdown and
/// in-process lease release; fail-closed [`FabricCommandRuntime`] drop behavior remains the last
/// resort if the manager itself is abandoned.
pub struct WorkspaceFabricCommandRuntimeManager {
    factory: Arc<dyn WorkspaceFabricCommandRuntimeFactory>,
    recovery_page_size: CommandRecoveryPageSize,
    maximum_recovery_sweeps: NonZeroUsize,
    runtimes: BTreeMap<WorkspaceId, ManagedWorkspaceRuntime>,
}

impl fmt::Debug for WorkspaceFabricCommandRuntimeManager {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkspaceFabricCommandRuntimeManager")
            .field("factory", &"installed")
            .field("recovery_page_size", &self.recovery_page_size)
            .field("maximum_recovery_sweeps", &self.maximum_recovery_sweeps)
            .field("workspace_count", &self.runtimes.len())
            .finish()
    }
}

impl WorkspaceFabricCommandRuntimeManager {
    /// Construct an empty manager with explicit bounded startup-recovery policy.
    #[must_use]
    pub fn new(
        factory: Arc<dyn WorkspaceFabricCommandRuntimeFactory>,
        recovery_page_size: CommandRecoveryPageSize,
        maximum_recovery_sweeps: NonZeroUsize,
    ) -> Self {
        Self {
            factory,
            recovery_page_size,
            maximum_recovery_sweeps,
            runtimes: BTreeMap::new(),
        }
    }

    /// Start, recover, and retain one workspace runtime inside the active Tokio executor.
    ///
    /// A bounded `Pending` result is retained fail-closed for a later
    /// [`Self::retry_recovery`]. Any actual recovery error triggers explicit joined shutdown before
    /// this method returns. If that shutdown also fails, both errors are returned together.
    ///
    /// # Errors
    ///
    /// Rejects duplicate ownership, factory/config drift, runtime start failure, or recovery and
    /// cleanup failure.
    pub async fn start_workspace(
        &mut self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceFabricCommandRuntimeState, WorkspaceFabricCommandRuntimeManagerError> {
        if self.runtimes.contains_key(&workspace_id) {
            return Err(
                WorkspaceFabricCommandRuntimeManagerError::DuplicateWorkspace(workspace_id),
            );
        }
        let parts = self.factory.build(workspace_id).map_err(|source| {
            WorkspaceFabricCommandRuntimeManagerError::Factory {
                workspace_id,
                source,
            }
        })?;
        let WorkspaceFabricCommandRuntimeParts {
            config,
            semantics,
            effects,
            interruption_diagnostics,
        } = parts;
        if config.workspace_id != workspace_id {
            return Err(
                WorkspaceFabricCommandRuntimeManagerError::FactoryWorkspaceMismatch {
                    requested: workspace_id,
                    configured: config.workspace_id,
                },
            );
        }
        let runtime = FabricCommandRuntime::start(config, semantics, effects.into_port()).map_err(
            |source| WorkspaceFabricCommandRuntimeManagerError::Start {
                workspace_id,
                source,
            },
        )?;
        let recovery = runtime
            .recover_and_open_bounded(
                self.recovery_page_size,
                self.maximum_recovery_sweeps,
                interruption_diagnostics.as_ref(),
            )
            .await;
        let recovery = match recovery {
            Ok(recovery) => recovery,
            Err(recovery) => {
                return match runtime.shutdown().await {
                    Ok(()) => Err(WorkspaceFabricCommandRuntimeManagerError::StartupRecovery {
                        workspace_id,
                        source: recovery,
                    }),
                    Err(shutdown) => Err(
                        WorkspaceFabricCommandRuntimeManagerError::StartupRecoveryAndShutdown {
                            workspace_id,
                            recovery,
                            shutdown,
                        },
                    ),
                };
            }
        };
        let state = recovery.state().into();
        let prior = self.runtimes.insert(
            workspace_id,
            ManagedWorkspaceRuntime {
                runtime,
                interruption_diagnostics,
                state,
            },
        );
        debug_assert!(
            prior.is_none(),
            "duplicate ownership was checked before start"
        );
        Ok(state)
    }

    /// Retry bounded recovery for one retained `Pending` runtime.
    ///
    /// Calling this for a ready runtime is an idempotent observation. On error the runtime remains
    /// retained with ingress closed; a later retry re-derives work from the durable journal.
    ///
    /// # Errors
    ///
    /// Returns an unknown workspace or the exact bounded-recovery failure.
    pub async fn retry_recovery(
        &mut self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceFabricCommandRuntimeState, WorkspaceFabricCommandRuntimeManagerError> {
        let page_size = self.recovery_page_size;
        let maximum_sweeps = self.maximum_recovery_sweeps;
        let managed = self
            .runtimes
            .get_mut(&workspace_id)
            .ok_or(WorkspaceFabricCommandRuntimeManagerError::WorkspaceNotManaged(workspace_id))?;
        if managed.state == WorkspaceFabricCommandRuntimeState::Ready {
            return Ok(WorkspaceFabricCommandRuntimeState::Ready);
        }
        let recovery = managed
            .runtime
            .recover_and_open_bounded(
                page_size,
                maximum_sweeps,
                managed.interruption_diagnostics.as_ref(),
            )
            .await
            .map_err(
                |source| WorkspaceFabricCommandRuntimeManagerError::Recovery {
                    workspace_id,
                    source,
                },
            )?;
        let state = recovery.state().into();
        managed.state = state;
        Ok(state)
    }

    /// Return the last proved startup/recovery state for a managed workspace.
    #[must_use]
    pub fn state(&self, workspace_id: WorkspaceId) -> Option<WorkspaceFabricCommandRuntimeState> {
        self.runtimes
            .get(&workspace_id)
            .map(|managed| managed.state)
    }

    /// Return command ingress only when bounded recovery proved the runtime ready.
    #[must_use]
    pub fn handle(&self, workspace_id: WorkspaceId) -> Option<WorkspaceFabricCommandRuntimeHandle> {
        let managed = self.runtimes.get(&workspace_id)?;
        (managed.state == WorkspaceFabricCommandRuntimeState::Ready).then(|| {
            WorkspaceFabricCommandRuntimeHandle {
                workspace_id,
                fence: managed.runtime.fence(),
                actor: managed.runtime.handle(),
            }
        })
    }

    /// Number of retained workspace runtimes, including fail-closed pending runtimes.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.runtimes.len()
    }

    /// Remove one runtime and await its actor before releasing writer authority.
    ///
    /// Returns `false` when no runtime was present, making lifecycle cleanup idempotent.
    ///
    /// # Errors
    ///
    /// Returns the exact actor/join shutdown failure after consuming the runtime.
    pub async fn stop_workspace(
        &mut self,
        workspace_id: WorkspaceId,
    ) -> Result<bool, WorkspaceFabricCommandRuntimeManagerError> {
        let Some(managed) = self.runtimes.remove(&workspace_id) else {
            return Ok(false);
        };
        managed.runtime.shutdown().await.map_err(|source| {
            WorkspaceFabricCommandRuntimeManagerError::Shutdown {
                workspace_id,
                source,
            }
        })?;
        Ok(true)
    }

    /// Consume and join every retained runtime, collecting failures without short-circuiting.
    ///
    /// The manager is empty on return, including when one or more runtimes fail shutdown.
    ///
    /// # Errors
    ///
    /// Returns every workspace/error pair in canonical workspace-ID order.
    pub async fn shutdown_all(
        &mut self,
    ) -> Result<(), WorkspaceFabricCommandRuntimeShutdownFailures> {
        let runtimes = std::mem::take(&mut self.runtimes);
        let mut failures = Vec::new();
        for (workspace_id, managed) in runtimes {
            if let Err(error) = managed.runtime.shutdown().await {
                failures.push(WorkspaceFabricCommandRuntimeShutdownFailure {
                    workspace_id,
                    error,
                });
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(WorkspaceFabricCommandRuntimeShutdownFailures { failures })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

    use async_trait::async_trait;

    use super::*;
    use crate::fabric::command::{
        ActorId, AuthorizationDecision, AuthorizationRef, CommandFailure, CommandIdentity,
        CommandOwnership, CommandPins, CommandRecord, CompilerReleaseRef, DiagnosticRef, EpochId,
        ExecutionOwner, ExpectedHead, FabricCommand, FabricCommandPayload, FailureClass,
        FailureCode, IdempotencyKey, LeaseId, ModelHeadRef, PrincipalId, ProofReceiptRef,
        ProviderSetRef, ReconciliationEvidenceRef, ReconciliationObservation, ReductionContext,
        ResourceEnvelopeRef, SourceGeneration, TransactionRef,
    };
    use crate::fabric::command_actor::{
        CommandPortError, CommitEffectOutcome, FabricCommandActorConfig, FabricCommandActorError,
        PrepareEffectOutcome,
    };
    use crate::fabric::command_runtime::SemanticAdmissionContext;

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

    struct UnavailableEffects;

    #[async_trait]
    impl FabricCommandEffectPort for UnavailableEffects {
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
        ) -> Result<ReconciliationObservation, CommandPortError> {
            Err(CommandPortError::EffectUnavailable)
        }
    }

    struct PrepareThenLoseCommit;

    #[async_trait]
    impl FabricCommandEffectPort for PrepareThenLoseCommit {
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

    struct SwitchingRecoveryEffects {
        prove_not_committed: AtomicBool,
    }

    #[async_trait]
    impl FabricCommandEffectPort for SwitchingRecoveryEffects {
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
            if self.prove_not_committed.load(Ordering::SeqCst) {
                Ok(ReconciliationObservation::NotCommitted {
                    evidence: ReconciliationEvidenceRef::from_bytes([0x92; 32]),
                })
            } else {
                Ok(ReconciliationObservation::Indeterminate {
                    evidence: ReconciliationEvidenceRef::from_bytes([0x93; 32]),
                })
            }
        }
    }

    struct PanickingPrepare;

    #[async_trait]
    impl FabricCommandEffectPort for PanickingPrepare {
        async fn prepare(
            &self,
            _executing: &CommandRecord,
            _owner: ExecutionOwner,
            _context: ReductionContext,
        ) -> Result<PrepareEffectOutcome, CommandPortError> {
            panic!("injected actor failure during recovery")
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

    struct Diagnostic;

    #[async_trait]
    impl InterruptedCommitDiagnosticPort for Diagnostic {
        async fn interruption_diagnostic(
            &self,
            _prepared: &CommandRecord,
            observed_transaction: TransactionRef,
            _active_fence: WriterFence,
        ) -> Result<DiagnosticRef, CommandPortError> {
            assert_eq!(observed_transaction, transaction());
            Ok(DiagnosticRef::from_bytes([0x90; 32]))
        }
    }

    struct TestFactory {
        root: PathBuf,
        effects: Arc<dyn FabricCommandEffectPort>,
        next_lease_seed: AtomicU8,
    }

    impl TestFactory {
        fn new(
            root: &Path,
            effects: Arc<dyn FabricCommandEffectPort>,
            next_lease_seed: u8,
        ) -> Self {
            Self {
                root: root.to_owned(),
                effects,
                next_lease_seed: AtomicU8::new(next_lease_seed),
            }
        }
    }

    impl WorkspaceFabricCommandRuntimeFactory for TestFactory {
        fn build(
            &self,
            workspace_id: WorkspaceId,
        ) -> Result<WorkspaceFabricCommandRuntimeParts, WorkspaceFabricCommandRuntimeFactoryError>
        {
            let root = workspace_root(&self.root, workspace_id);
            fs::create_dir_all(&root)?;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
            let lease_seed = self.next_lease_seed.fetch_add(1, Ordering::SeqCst);
            Ok(WorkspaceFabricCommandRuntimeParts::new_for_tests(
                runtime_config(&root, workspace_id, lease_seed),
                Arc::new(CurrentContext),
                Arc::clone(&self.effects),
                Arc::new(Diagnostic),
            ))
        }
    }

    fn manager(
        root: &Path,
        effects: Arc<dyn FabricCommandEffectPort>,
        next_lease_seed: u8,
        maximum_sweeps: usize,
    ) -> WorkspaceFabricCommandRuntimeManager {
        WorkspaceFabricCommandRuntimeManager::new(
            Arc::new(TestFactory::new(root, effects, next_lease_seed)),
            CommandRecoveryPageSize::new(8).unwrap(),
            NonZeroUsize::new(maximum_sweeps).unwrap(),
        )
    }

    fn workspace(seed: u8) -> WorkspaceId {
        WorkspaceId::from_bytes([seed; 16])
    }

    fn workspace_root(root: &Path, workspace_id: WorkspaceId) -> PathBuf {
        root.join(format!("workspace-{:02x}", workspace_id.as_bytes()[0]))
    }

    fn runtime_config(
        root: &Path,
        workspace_id: WorkspaceId,
        lease_seed: u8,
    ) -> FabricCommandRuntimeConfig {
        FabricCommandRuntimeConfig::new(
            root,
            root.join("writer-generations.sqlite3"),
            root.join("commands.sqlite3"),
            workspace_id,
            LeaseId::from_bytes([lease_seed; 16]),
            ActorId::from_bytes([lease_seed.wrapping_add(0x40); 16]),
            FabricCommandActorConfig::default(),
        )
    }

    fn command(
        workspace_id: WorkspaceId,
        writer_fence: WriterFence,
        operation_seed: u8,
    ) -> FabricCommand {
        FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes([operation_seed; 16]),
                idempotency_key: IdempotencyKey::from_bytes([operation_seed.wrapping_add(1); 32]),
            },
            ownership: CommandOwnership {
                workspace_id,
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

    async fn seed_nonterminal(
        root: &Path,
        workspace_id: WorkspaceId,
        effects: Arc<dyn FabricCommandEffectPort>,
    ) {
        fs::create_dir_all(root).unwrap();
        fs::set_permissions(root, fs::Permissions::from_mode(0o700)).unwrap();
        let runtime = FabricCommandRuntime::start(
            runtime_config(root, workspace_id, 1),
            Arc::new(CurrentContext),
            effects,
        )
        .unwrap();
        let command = command(workspace_id, runtime.fence(), 0x20);
        let ingress = runtime.open_command_admission().await.unwrap();
        assert!(matches!(
            ingress.submit(command).await,
            Err(FabricCommandActorError::Port(
                CommandPortError::EffectUnavailable
            ))
        ));
        runtime.shutdown().await.unwrap();
    }

    #[test]
    fn registered_factory_freezes_complete_closures_and_fails_closed() {
        let root = tempfile::tempdir().unwrap();
        let first = workspace(0x10);
        let second = workspace(0x20);
        let registration = |workspace_id, lease_seed| {
            WorkspaceFabricCommandRuntimeParts::new_for_tests(
                runtime_config(root.path(), workspace_id, lease_seed),
                Arc::new(CurrentContext),
                Arc::new(UnavailableEffects),
                Arc::new(Diagnostic),
            )
        };
        let factory = RegisteredWorkspaceFabricCommandRuntimeFactory::try_new([
            registration(second, 2),
            registration(first, 1),
        ])
        .unwrap();

        assert_eq!(factory.len(), 2);
        assert!(!factory.is_empty());
        assert_eq!(
            factory.workspace_ids().collect::<Vec<_>>(),
            vec![first, second]
        );
        assert_eq!(factory.build(first).unwrap().workspace_id(), first);
        assert!(
            factory
                .build(workspace(0x30))
                .unwrap_err()
                .to_string()
                .contains("no complete fabric command runtime registration")
        );

        let duplicate = RegisteredWorkspaceFabricCommandRuntimeFactory::try_new([
            registration(first, 3),
            registration(first, 4),
        ]);
        assert!(matches!(
            duplicate,
            Err(RegisteredWorkspaceFabricCommandRuntimeFactoryError::DuplicateWorkspace(
                observed
            )) if observed == first
        ));
    }

    #[tokio::test]
    async fn ready_runtime_is_unique_and_joined_stop_releases_its_actor() {
        let root = tempfile::tempdir().unwrap();
        let workspace_id = workspace(0x10);
        let mut manager = manager(root.path(), Arc::new(UnavailableEffects), 1, 1);

        assert_eq!(
            manager.start_workspace(workspace_id).await.unwrap(),
            WorkspaceFabricCommandRuntimeState::Ready
        );
        let ready = manager.handle(workspace_id).expect("ready handle");
        assert_eq!(ready.workspace_id(), workspace_id);
        assert_eq!(ready.fence().generation.get(), 1);
        assert!(matches!(
            manager.start_workspace(workspace_id).await,
            Err(WorkspaceFabricCommandRuntimeManagerError::DuplicateWorkspace(
                observed
            )) if observed == workspace_id
        ));

        assert!(manager.stop_workspace(workspace_id).await.unwrap());
        assert!(!manager.stop_workspace(workspace_id).await.unwrap());
        assert!(manager.handle(workspace_id).is_none());
        assert!(matches!(
            ready.actor().shutdown().await,
            Err(FabricCommandActorError::QueueClosed)
        ));
    }

    #[tokio::test]
    async fn pending_runtime_is_retained_without_a_handle_and_retry_requires_noncommit_proof() {
        let root = tempfile::tempdir().unwrap();
        let workspace_id = workspace(0x11);
        let workspace_root = workspace_root(root.path(), workspace_id);
        seed_nonterminal(
            &workspace_root,
            workspace_id,
            Arc::new(PrepareThenLoseCommit),
        )
        .await;
        let effects = Arc::new(SwitchingRecoveryEffects {
            prove_not_committed: AtomicBool::new(false),
        });
        let installed_effects: Arc<dyn FabricCommandEffectPort> = effects.clone();
        let mut manager = manager(root.path(), installed_effects, 2, 2);

        let pending = manager.start_workspace(workspace_id).await.unwrap();
        assert!(matches!(
            pending,
            WorkspaceFabricCommandRuntimeState::Pending {
                obligation: CommandRecoveryObligation::ReconcileCommit { transaction: observed },
                ..
            } if observed == transaction()
        ));
        assert_eq!(manager.state(workspace_id), Some(pending));
        assert_eq!(manager.active_count(), 1);
        assert!(manager.handle(workspace_id).is_none());

        effects.prove_not_committed.store(true, Ordering::SeqCst);
        assert_eq!(
            manager.retry_recovery(workspace_id).await.unwrap(),
            WorkspaceFabricCommandRuntimeState::Ready
        );
        assert!(manager.handle(workspace_id).is_some());
        manager.shutdown_all().await.unwrap();
    }

    #[tokio::test]
    async fn startup_recovery_error_explicitly_stops_the_unretained_runtime() {
        let root = tempfile::tempdir().unwrap();
        let workspace_id = workspace(0x12);
        let workspace_root = workspace_root(root.path(), workspace_id);
        seed_nonterminal(&workspace_root, workspace_id, Arc::new(UnavailableEffects)).await;
        let mut manager = manager(root.path(), Arc::new(UnavailableEffects), 2, 1);

        assert!(matches!(
            manager.start_workspace(workspace_id).await,
            Err(WorkspaceFabricCommandRuntimeManagerError::StartupRecovery {
                workspace_id: observed,
                ..
            }) if observed == workspace_id
        ));
        assert_eq!(manager.active_count(), 0);

        let successor = FabricCommandRuntime::start(
            runtime_config(&workspace_root, workspace_id, 3),
            Arc::new(CurrentContext),
            Arc::new(UnavailableEffects),
        )
        .expect("explicit cleanup released the workspace OS lease");
        successor.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn startup_recovery_and_shutdown_failures_are_preserved_together() {
        let root = tempfile::tempdir().unwrap();
        let workspace_id = workspace(0x13);
        let workspace_root = workspace_root(root.path(), workspace_id);
        seed_nonterminal(&workspace_root, workspace_id, Arc::new(UnavailableEffects)).await;
        let mut manager = manager(root.path(), Arc::new(PanickingPrepare), 2, 1);

        assert!(matches!(
            manager.start_workspace(workspace_id).await,
            Err(
                WorkspaceFabricCommandRuntimeManagerError::StartupRecoveryAndShutdown {
                    workspace_id: observed,
                    recovery: _,
                    shutdown: _,
                }
            ) if observed == workspace_id
        ));
        assert_eq!(manager.active_count(), 0);
    }

    #[tokio::test]
    async fn shutdown_all_attempts_every_runtime_and_aggregates_in_workspace_order() {
        let root = tempfile::tempdir().unwrap();
        let first = workspace(0x21);
        let second = workspace(0x22);
        let healthy = workspace(0x23);
        let mut manager = manager(root.path(), Arc::new(UnavailableEffects), 1, 1);
        for workspace_id in [healthy, second, first] {
            assert_eq!(
                manager.start_workspace(workspace_id).await.unwrap(),
                WorkspaceFabricCommandRuntimeState::Ready
            );
        }
        let first_actor = manager.handle(first).unwrap().actor();
        let second_actor = manager.handle(second).unwrap().actor();
        let healthy_actor = manager.handle(healthy).unwrap().actor();
        first_actor.shutdown().await.unwrap();
        second_actor.shutdown().await.unwrap();

        let failures = manager.shutdown_all().await.unwrap_err();
        assert_eq!(manager.active_count(), 0);
        assert_eq!(failures.failures().len(), 2);
        assert_eq!(failures.failures()[0].workspace_id(), first);
        assert_eq!(failures.failures()[1].workspace_id(), second);
        assert!(matches!(
            healthy_actor.shutdown().await,
            Err(FabricCommandActorError::QueueClosed)
        ));
    }

    #[tokio::test]
    async fn factory_workspace_drift_is_rejected_before_any_runtime_starts() {
        struct MismatchedFactory {
            root: PathBuf,
        }

        impl WorkspaceFabricCommandRuntimeFactory for MismatchedFactory {
            fn build(
                &self,
                _workspace_id: WorkspaceId,
            ) -> Result<WorkspaceFabricCommandRuntimeParts, WorkspaceFabricCommandRuntimeFactoryError>
            {
                let configured = workspace(0x32);
                Ok(WorkspaceFabricCommandRuntimeParts::new_for_tests(
                    runtime_config(&self.root, configured, 1),
                    Arc::new(CurrentContext),
                    Arc::new(UnavailableEffects),
                    Arc::new(Diagnostic),
                ))
            }
        }

        let root = tempfile::tempdir().unwrap();
        let requested = workspace(0x31);
        let mut manager = WorkspaceFabricCommandRuntimeManager::new(
            Arc::new(MismatchedFactory {
                root: root.path().to_owned(),
            }),
            CommandRecoveryPageSize::new(1).unwrap(),
            NonZeroUsize::new(1).unwrap(),
        );
        assert!(matches!(
            manager.start_workspace(requested).await,
            Err(
                WorkspaceFabricCommandRuntimeManagerError::FactoryWorkspaceMismatch {
                    requested: observed,
                    configured,
                }
            ) if observed == requested && configured == workspace(0x32)
        ));
        assert_eq!(manager.active_count(), 0);
    }
}
