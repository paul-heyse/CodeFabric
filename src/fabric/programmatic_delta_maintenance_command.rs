//! Programmatic command adapter for the read-only guarded Delta maintenance subset.
//!
//! The adapter deliberately supports only retention inspection, proposed-deletion validation,
//! and native vacuum dry-run planning. Those actions do not mutate Delta state, so a durable
//! operation-history port can be the single commit point and can prove non-commit from absence.
//! Checkpoint creation, optimize, and destructive vacuum remain typed unavailable: none can be
//! made restart-safe without an operation marker atomically bound to its external side effect.

use std::sync::Arc;

use async_trait::async_trait;

use super::administration_command_effect::{
    AdministrationAttempt, AdministrationCommandEffect, AdministrationCommitObservation,
    AdministrationCommitPort, AdministrationCommitRequest, AdministrationMarkerObservation,
    AdministrationMarkerPort, AdministrationReconciliationRequest, AdministrationResolution,
    AdministrationResolverPort, ResolvedAdministration,
};
use super::command::{
    AdministrationAction, AdministrationRequestRef, CommandFailure, DiagnosticRef, FailureClass,
    FailureCode, OperationSelectionRef, TransactionRef, WorkspaceId,
};
use super::command_actor::CommandPortError;
use super::delta_exact::{DeltaRetainedResource, ExactDeltaPin};
use super::delta_guarded_maintenance::{
    DeltaMaintenanceOutcome, GuardedDeltaMaintenanceIntent, GuardedDeltaMaintenanceRequest,
};
use super::programmatic_delta_runtime::ProgrammaticDeltaRuntime;

/// Exact immutable maintenance request resolved from an administration request relation row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammaticDeltaMaintenanceCommandSelection {
    action: AdministrationAction,
    request: AdministrationRequestRef,
    workspace_id: WorkspaceId,
    relation_id: Arc<str>,
    maintenance: GuardedDeltaMaintenanceRequest,
    history_predecessor: ExactDeltaPin,
    operation_selection: OperationSelectionRef,
}

impl ProgrammaticDeltaMaintenanceCommandSelection {
    /// Construct one resolved command selection.
    ///
    /// The request and operation-selection identities must be nonzero, and the relation identity
    /// must be nonempty. Action/intent and command-attempt binding are validated again by the
    /// adapter against the live command before execution.
    pub fn try_new(
        action: AdministrationAction,
        request: AdministrationRequestRef,
        workspace_id: WorkspaceId,
        relation_id: impl Into<Arc<str>>,
        maintenance: GuardedDeltaMaintenanceRequest,
        history_predecessor: ExactDeltaPin,
        operation_selection: OperationSelectionRef,
    ) -> Result<Self, ProgrammaticDeltaMaintenanceCommandInputError> {
        let relation_id = relation_id.into();
        if relation_id.trim().is_empty() {
            return Err(ProgrammaticDeltaMaintenanceCommandInputError::EmptyRelationId);
        }
        if all_zero(request.as_bytes()) {
            return Err(ProgrammaticDeltaMaintenanceCommandInputError::ZeroRequest);
        }
        if all_zero(operation_selection.as_bytes()) {
            return Err(ProgrammaticDeltaMaintenanceCommandInputError::ZeroOperationSelection);
        }
        Ok(Self {
            action,
            request,
            workspace_id,
            relation_id,
            maintenance,
            history_predecessor,
            operation_selection,
        })
    }

    #[must_use]
    pub const fn action(&self) -> AdministrationAction {
        self.action
    }

    #[must_use]
    pub const fn request(&self) -> AdministrationRequestRef {
        self.request
    }

    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    #[must_use]
    pub fn relation_id(&self) -> &str {
        &self.relation_id
    }

    #[must_use]
    pub const fn maintenance(&self) -> &GuardedDeltaMaintenanceRequest {
        &self.maintenance
    }

    /// Exact operation-history predecessor against which readback must commit.
    #[must_use]
    pub const fn history_predecessor(&self) -> &ExactDeltaPin {
        &self.history_predecessor
    }

    #[must_use]
    pub const fn operation_selection(&self) -> OperationSelectionRef {
        self.operation_selection
    }
}

/// Invalid immutable request-selection input.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProgrammaticDeltaMaintenanceCommandInputError {
    #[error("programmatic Delta maintenance relation identity must be nonempty")]
    EmptyRelationId,
    #[error("programmatic Delta maintenance request identity must be nonzero")]
    ZeroRequest,
    #[error("programmatic Delta maintenance operation selection must be nonzero")]
    ZeroOperationSelection,
    #[error("programmatic Delta maintenance diagnostic identity must be nonzero")]
    ZeroDiagnostic,
}

/// Exact request-relation authority. Resolution is read-only and never discovers latest state.
#[async_trait]
pub trait ProgrammaticDeltaMaintenanceRequestPort: Send + Sync {
    async fn resolve(
        &self,
        action: AdministrationAction,
        request: AdministrationRequestRef,
    ) -> Result<Option<ProgrammaticDeltaMaintenanceCommandSelection>, CommandPortError>;
}

/// Complete successful read-only observation presented to the durable operation-history writer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammaticDeltaMaintenanceCommandCommit {
    attempt: AdministrationAttempt,
    transaction: TransactionRef,
    selection: ProgrammaticDeltaMaintenanceCommandSelection,
    outcome: DeltaMaintenanceOutcome,
}

impl ProgrammaticDeltaMaintenanceCommandCommit {
    #[must_use]
    pub const fn attempt(&self) -> AdministrationAttempt {
        self.attempt
    }

    #[must_use]
    pub const fn transaction(&self) -> TransactionRef {
        self.transaction
    }

    #[must_use]
    pub const fn selection(&self) -> &ProgrammaticDeltaMaintenanceCommandSelection {
        &self.selection
    }

    #[must_use]
    pub const fn outcome(&self) -> &DeltaMaintenanceOutcome {
        &self.outcome
    }
}

/// Durable operation-selection and marker/control-history authority.
///
/// `commit_readback` must append the complete operation-selection row and read it back before
/// returning `Committed`. `read_exact` must query that same durable history by the original
/// operation/transaction/generation; a process-local cache is not a valid implementation.
#[async_trait]
pub trait ProgrammaticDeltaMaintenanceHistoryPort: Send + Sync {
    async fn commit_readback(
        &self,
        commit: ProgrammaticDeltaMaintenanceCommandCommit,
    ) -> Result<AdministrationCommitObservation, CommandPortError>;

    async fn read_exact(
        &self,
        request: AdministrationReconciliationRequest,
    ) -> Result<AdministrationMarkerObservation, CommandPortError>;
}

/// Stable diagnostics for fail-closed request, capability, and evidence-drift outcomes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgrammaticDeltaMaintenanceCommandDiagnostics {
    invalid_request: DiagnosticRef,
    unavailable_action: DiagnosticRef,
    rejected_safety: DiagnosticRef,
    evidence_changed: DiagnosticRef,
}

impl ProgrammaticDeltaMaintenanceCommandDiagnostics {
    pub fn try_new(
        invalid_request: DiagnosticRef,
        unavailable_action: DiagnosticRef,
        rejected_safety: DiagnosticRef,
        evidence_changed: DiagnosticRef,
    ) -> Result<Self, ProgrammaticDeltaMaintenanceCommandInputError> {
        if [
            invalid_request,
            unavailable_action,
            rejected_safety,
            evidence_changed,
        ]
        .iter()
        .any(|diagnostic| all_zero(diagnostic.as_bytes()))
        {
            return Err(ProgrammaticDeltaMaintenanceCommandInputError::ZeroDiagnostic);
        }
        Ok(Self {
            invalid_request,
            unavailable_action,
            rejected_safety,
            evidence_changed,
        })
    }
}

/// Dependencies retained until the live workspace Delta runtime exists.
#[derive(Clone)]
pub struct ProgrammaticDeltaMaintenanceAdministrationPorts {
    requests: Arc<dyn ProgrammaticDeltaMaintenanceRequestPort>,
    history: Arc<dyn ProgrammaticDeltaMaintenanceHistoryPort>,
    diagnostics: ProgrammaticDeltaMaintenanceCommandDiagnostics,
}

impl std::fmt::Debug for ProgrammaticDeltaMaintenanceAdministrationPorts {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProgrammaticDeltaMaintenanceAdministrationPorts")
            .field("requests", &"installed")
            .field("history", &"installed")
            .field("diagnostics", &self.diagnostics)
            .finish()
    }
}

impl ProgrammaticDeltaMaintenanceAdministrationPorts {
    #[must_use]
    pub const fn new(
        requests: Arc<dyn ProgrammaticDeltaMaintenanceRequestPort>,
        history: Arc<dyn ProgrammaticDeltaMaintenanceHistoryPort>,
        diagnostics: ProgrammaticDeltaMaintenanceCommandDiagnostics,
    ) -> Self {
        Self {
            requests,
            history,
            diagnostics,
        }
    }

    pub(crate) fn build(
        &self,
        runtime: Arc<ProgrammaticDeltaRuntime>,
    ) -> Arc<AdministrationCommandEffect> {
        let runtime: Arc<dyn ProgrammaticDeltaMaintenanceRuntimePort> = runtime;
        self.build_with_runtime(runtime)
    }

    fn build_with_runtime(
        &self,
        runtime: Arc<dyn ProgrammaticDeltaMaintenanceRuntimePort>,
    ) -> Arc<AdministrationCommandEffect> {
        let adapter = Arc::new(ProgrammaticDeltaMaintenanceAdministrationAdapter {
            runtime,
            requests: Arc::clone(&self.requests),
            history: Arc::clone(&self.history),
            diagnostics: self.diagnostics,
        });
        Arc::new(AdministrationCommandEffect::new(
            adapter.clone(),
            adapter.clone(),
            adapter,
        ))
    }
}

#[async_trait]
trait ProgrammaticDeltaMaintenanceRuntimePort: Send + Sync {
    async fn maintain(
        &self,
        relation_id: &str,
        request: &GuardedDeltaMaintenanceRequest,
    ) -> Result<DeltaMaintenanceOutcome, ()>;
}

#[async_trait]
impl ProgrammaticDeltaMaintenanceRuntimePort for ProgrammaticDeltaRuntime {
    async fn maintain(
        &self,
        relation_id: &str,
        request: &GuardedDeltaMaintenanceRequest,
    ) -> Result<DeltaMaintenanceOutcome, ()> {
        ProgrammaticDeltaRuntime::maintain(self, relation_id, request)
            .await
            .map_err(|_| ())
    }
}

struct ProgrammaticDeltaMaintenanceAdministrationAdapter {
    runtime: Arc<dyn ProgrammaticDeltaMaintenanceRuntimePort>,
    requests: Arc<dyn ProgrammaticDeltaMaintenanceRequestPort>,
    history: Arc<dyn ProgrammaticDeltaMaintenanceHistoryPort>,
    diagnostics: ProgrammaticDeltaMaintenanceCommandDiagnostics,
}

impl ProgrammaticDeltaMaintenanceAdministrationAdapter {
    async fn resolve_selection(
        &self,
        attempt: AdministrationAttempt,
    ) -> Result<ProgrammaticDeltaMaintenanceCommandSelection, CommandFailure> {
        let Some(selection) = self
            .requests
            .resolve(attempt.action(), attempt.request())
            .await
            .map_err(|_| self.backend_failure())?
        else {
            return Err(self.invalid_failure());
        };
        if !selection_matches_attempt(&selection, attempt) {
            return Err(self.invalid_failure());
        }
        match action_support(selection.action(), selection.maintenance().intent()) {
            ActionSupport::Supported => {}
            ActionSupport::Unavailable => return Err(self.unavailable_failure()),
            ActionSupport::Mismatch => return Err(self.invalid_failure()),
        }
        Ok(selection)
    }

    async fn observe_supported(
        &self,
        selection: &ProgrammaticDeltaMaintenanceCommandSelection,
    ) -> Result<DeltaMaintenanceOutcome, CommandFailure> {
        let outcome = self
            .runtime
            .maintain(selection.relation_id(), selection.maintenance())
            .await
            .map_err(|_| self.backend_failure())?;
        match (&selection.action, &outcome) {
            (
                AdministrationAction::InspectDeltaRetention,
                DeltaMaintenanceOutcome::RetentionInspected { .. },
            )
            | (
                AdministrationAction::ValidateDeltaRetention,
                DeltaMaintenanceOutcome::RetentionValidated { .. },
            )
            | (AdministrationAction::PlanDeltaVacuum, DeltaMaintenanceOutcome::VacuumDryRun(_)) => {
                Ok(outcome)
            }
            (_, DeltaMaintenanceOutcome::Rejected(_)) => Err(self.rejected_failure()),
            (_, DeltaMaintenanceOutcome::Unavailable(_)) => Err(self.unavailable_failure()),
            _ => Err(self.invalid_failure()),
        }
    }

    const fn invalid_failure(&self) -> CommandFailure {
        failure(
            FailureCode::InvalidInput,
            FailureClass::Permanent,
            self.diagnostics.invalid_request,
        )
    }

    const fn unavailable_failure(&self) -> CommandFailure {
        failure(
            FailureCode::BackendUnavailable,
            FailureClass::Permanent,
            self.diagnostics.unavailable_action,
        )
    }

    const fn rejected_failure(&self) -> CommandFailure {
        failure(
            FailureCode::InvalidInput,
            FailureClass::RetryableBeforeCommit,
            self.diagnostics.rejected_safety,
        )
    }

    const fn backend_failure(&self) -> CommandFailure {
        failure(
            FailureCode::BackendUnavailable,
            FailureClass::RetryableBeforeCommit,
            self.diagnostics.evidence_changed,
        )
    }
}

#[async_trait]
impl AdministrationResolverPort for ProgrammaticDeltaMaintenanceAdministrationAdapter {
    async fn resolve(
        &self,
        attempt: AdministrationAttempt,
    ) -> Result<AdministrationResolution, CommandPortError> {
        let selection = match self.resolve_selection(attempt).await {
            Ok(selection) => selection,
            Err(failure) => return Ok(AdministrationResolution::KnownFailure(failure)),
        };
        if let Err(failure) = self.observe_supported(&selection).await {
            return Ok(AdministrationResolution::KnownFailure(failure));
        }
        Ok(AdministrationResolution::Resolved(
            ResolvedAdministration::new(attempt, transaction_identity(attempt, &selection)),
        ))
    }
}

#[async_trait]
impl AdministrationCommitPort for ProgrammaticDeltaMaintenanceAdministrationAdapter {
    async fn commit(
        &self,
        request: AdministrationCommitRequest,
    ) -> Result<AdministrationCommitObservation, CommandPortError> {
        let attempt = request.attempt();
        let selection = match self.resolve_selection(attempt).await {
            Ok(selection) => selection,
            Err(_) => {
                return Ok(AdministrationCommitObservation::Conflict {
                    diagnostic: self.diagnostics.evidence_changed,
                });
            }
        };
        if transaction_identity(attempt, &selection) != request.transaction() {
            return Err(CommandPortError::CorruptRecord);
        }
        let outcome = match self.observe_supported(&selection).await {
            Ok(outcome) => outcome,
            Err(_) => {
                return Ok(AdministrationCommitObservation::Conflict {
                    diagnostic: self.diagnostics.evidence_changed,
                });
            }
        };
        self.history
            .commit_readback(ProgrammaticDeltaMaintenanceCommandCommit {
                attempt,
                transaction: request.transaction(),
                selection,
                outcome,
            })
            .await
    }
}

#[async_trait]
impl AdministrationMarkerPort for ProgrammaticDeltaMaintenanceAdministrationAdapter {
    async fn read_exact(
        &self,
        request: AdministrationReconciliationRequest,
    ) -> Result<AdministrationMarkerObservation, CommandPortError> {
        self.history.read_exact(request).await
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActionSupport {
    Supported,
    Unavailable,
    Mismatch,
}

const fn action_support(
    action: AdministrationAction,
    intent: &GuardedDeltaMaintenanceIntent,
) -> ActionSupport {
    match (action, intent) {
        (
            AdministrationAction::InspectDeltaRetention,
            GuardedDeltaMaintenanceIntent::InspectRetention,
        )
        | (
            AdministrationAction::ValidateDeltaRetention,
            GuardedDeltaMaintenanceIntent::ValidateRetention { .. },
        )
        | (
            AdministrationAction::PlanDeltaVacuum,
            GuardedDeltaMaintenanceIntent::VacuumDryRun { .. },
        ) => ActionSupport::Supported,
        (
            AdministrationAction::CreateDeltaCheckpoint,
            GuardedDeltaMaintenanceIntent::CreateCheckpoint,
        )
        | (AdministrationAction::CompactDelta, GuardedDeltaMaintenanceIntent::OptimizeCompact(_))
        | (
            AdministrationAction::ExecuteDeltaVacuum,
            GuardedDeltaMaintenanceIntent::VacuumExecute(_),
        ) => ActionSupport::Unavailable,
        _ => ActionSupport::Mismatch,
    }
}

fn selection_matches_attempt(
    selection: &ProgrammaticDeltaMaintenanceCommandSelection,
    attempt: AdministrationAttempt,
) -> bool {
    let command = attempt.command();
    selection.action == attempt.action()
        && selection.request == attempt.request()
        && selection.workspace_id == command.ownership.workspace_id
        && selection.maintenance.operation_id() == command.identity.operation_id
        && selection.maintenance.writer_fence() == attempt.execution_owner().fence
}

fn transaction_identity(
    attempt: AdministrationAttempt,
    selection: &ProgrammaticDeltaMaintenanceCommandSelection,
) -> TransactionRef {
    let command = attempt.command();
    let mut digest = blake3::Hasher::new();
    digest.update(b"codefabric.delta-maintenance-administration-transaction.v2");
    digest.update(command.ownership.workspace_id.as_bytes());
    digest.update(command.identity.operation_id.as_bytes());
    digest.update(command.identity.idempotency_key.as_bytes());
    digest.update(&attempt.attempt().to_be_bytes());
    digest.update(&[administration_action_tag(selection.action)]);
    digest.update(selection.request.as_bytes());
    frame(&mut digest, selection.relation_id.as_bytes());
    frame(
        &mut digest,
        selection
            .maintenance
            .target()
            .canonical_root()
            .as_str()
            .as_bytes(),
    );
    digest.update(&selection.maintenance.target().version().to_be_bytes());
    frame(
        &mut digest,
        selection
            .history_predecessor
            .canonical_root()
            .as_str()
            .as_bytes(),
    );
    digest.update(&selection.history_predecessor.version().to_be_bytes());
    digest.update(selection.maintenance.expected_activation_head());
    digest.update(selection.maintenance.writer_fence().lease_id.as_bytes());
    digest.update(
        &selection
            .maintenance
            .writer_fence()
            .generation
            .get()
            .to_be_bytes(),
    );
    digest.update(selection.operation_selection.as_bytes());
    let intent_identity = supported_maintenance_intent_identity(selection.maintenance().intent())
        .unwrap_or_else(|| {
            unreachable!("unsupported maintenance cannot reach transaction selection")
        });
    digest.update(&intent_identity);
    TransactionRef::from_bytes(*digest.finalize().as_bytes())
}

/// Canonical identity of the supported read-only guarded maintenance intent.
///
/// The operation-selection row is an independent provenance identity; it does not replace binding
/// the executable intent itself. In particular, `ValidateRetention` binds the ordered, reversible
/// proposed-deletion child relation rather than only its parent request identity. Mutating and
/// checkpoint intents return `None`; they remain typed unavailable until their side effects can be
/// atomically bound to a durable operation marker.
fn supported_maintenance_intent_identity(
    intent: &GuardedDeltaMaintenanceIntent,
) -> Option<[u8; 32]> {
    let mut digest = blake3::Hasher::new();
    digest.update(b"codefabric.delta-maintenance-intent.v1");
    match intent {
        GuardedDeltaMaintenanceIntent::InspectRetention => {
            digest.update(&[0]);
        }
        GuardedDeltaMaintenanceIntent::ValidateRetention { proposed_deletions } => {
            digest.update(&[1]);
            digest.update(&proposed_deletion_set_identity(proposed_deletions));
        }
        GuardedDeltaMaintenanceIntent::VacuumDryRun {
            expected_retention_seconds,
        } => {
            digest.update(&[2]);
            digest.update(&expected_retention_seconds.to_be_bytes());
        }
        GuardedDeltaMaintenanceIntent::CreateCheckpoint
        | GuardedDeltaMaintenanceIntent::OptimizeCompact(_)
        | GuardedDeltaMaintenanceIntent::VacuumExecute(_) => return None,
    }
    Some(*digest.finalize().as_bytes())
}

/// Canonical identity of an ordered proposed-deletion child relation.
///
/// Ordinal is semantic because validation reports the first protected resource in input order.
/// Every resource variant is encoded reversibly; no display/debug representation participates.
pub(super) fn proposed_deletion_set_identity(resources: &[DeltaRetainedResource]) -> [u8; 32] {
    let mut digest = blake3::Hasher::new();
    digest.update(b"codefabric.delta-maintenance-proposed-deletion-set.v1");
    digest.update(
        &u64::try_from(resources.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    for (ordinal, resource) in resources.iter().enumerate() {
        digest.update(&u64::try_from(ordinal).unwrap_or(u64::MAX).to_be_bytes());
        match resource {
            DeltaRetainedResource::DeltaVersion(pin) => {
                digest.update(&[0]);
                frame(&mut digest, pin.canonical_root().as_str().as_bytes());
                digest.update(&pin.version().to_be_bytes());
            }
            DeltaRetainedResource::ImmutableSegment(identity) => {
                digest.update(&[1]);
                digest.update(identity);
            }
            DeltaRetainedResource::ProgramRelease(identity) => {
                digest.update(&[2]);
                digest.update(identity);
            }
            DeltaRetainedResource::Expectation(identity) => {
                digest.update(&[3]);
                digest.update(identity);
            }
            DeltaRetainedResource::QueryResult(identity) => {
                digest.update(&[4]);
                digest.update(identity);
            }
            DeltaRetainedResource::RollbackPoint(identity) => {
                digest.update(&[5]);
                digest.update(identity);
            }
        }
    }
    *digest.finalize().as_bytes()
}

const fn administration_action_tag(action: AdministrationAction) -> u8 {
    match action {
        AdministrationAction::RebuildCandidate => 0,
        AdministrationAction::RepairTemporalCache => 1,
        AdministrationAction::ReconcileOperation => 2,
        AdministrationAction::InspectDeltaRetention => 3,
        AdministrationAction::ValidateDeltaRetention => 4,
        AdministrationAction::PlanDeltaVacuum => 5,
        AdministrationAction::CreateDeltaCheckpoint => 6,
        AdministrationAction::CompactDelta => 7,
        AdministrationAction::ExecuteDeltaVacuum => 8,
    }
}

fn frame(digest: &mut blake3::Hasher, value: &[u8]) {
    digest.update(&u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    digest.update(value);
}

const fn failure(
    code: FailureCode,
    class: FailureClass,
    diagnostic: DiagnosticRef,
) -> CommandFailure {
    CommandFailure {
        code,
        class,
        diagnostic,
    }
}

fn all_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::{fs, sync::Arc};

    use super::*;
    use crate::fabric::administration_command_effect::AdministrationCommitReceipt;
    use crate::fabric::command::{
        ActorId, AdmissionContext, AdmissionOutcome, AuthorizationDecision, AuthorizationRef,
        CommandEvent, CommandIdentity, CommandOwnership, CommandPins, CommandReducer, ExpectedHead,
        FabricCommand, FabricCommandPayload, IdempotencyKey, InputReleaseRef, LeaseId, OperationId,
        PrincipalId, ProgramReleaseRef, ProviderSetRef, ReconciliationEvidenceRef,
        ReconciliationObservation, ReductionContext, ResourceEnvelopeRef, SourceGeneration,
        TransactionRef, UnknownCommit, UnknownCommitReason, WorkspaceId, WriterFence,
        WriterGeneration,
    };
    use crate::fabric::command_actor::{CommitEffectOutcome, PrepareEffectOutcome};
    use crate::fabric::command_effect_router::AdministrationCommandEffectPort;
    use crate::fabric::delta_exact::{
        DeltaRetainedResource, DeltaRetentionAuthorityKind, DeltaRetentionClosure, ExactDeltaPin,
    };
    use crate::fabric::delta_guarded_maintenance::{
        DeltaMaintenanceRejection, GuardedDeltaMaintenanceIntent,
    };
    use crate::fabric::programmatic_delta_maintenance_relation::DeltaProgrammaticMaintenanceCommandRelation;
    use datafusion::execution::SessionStateBuilder;
    use datafusion::prelude::SessionConfig;
    use deltalake::DeltaTableBuilder;
    use deltalake::delta_datafusion::planner::DeltaPlanner;
    use deltalake::kernel::engine::arrow_conversion::TryIntoKernel as _;
    use deltalake::operations::create::CreateBuilder;
    use deltalake::protocol::SaveMode;
    use tempfile::TempDir;
    use url::Url;

    struct StaticRequests {
        selection: ProgrammaticDeltaMaintenanceCommandSelection,
    }

    #[async_trait]
    impl ProgrammaticDeltaMaintenanceRequestPort for StaticRequests {
        async fn resolve(
            &self,
            action: AdministrationAction,
            request: AdministrationRequestRef,
        ) -> Result<Option<ProgrammaticDeltaMaintenanceCommandSelection>, CommandPortError>
        {
            Ok(
                (self.selection.action == action && self.selection.request == request)
                    .then(|| self.selection.clone()),
            )
        }
    }

    struct ScriptedRuntime {
        outcomes: Mutex<Vec<DeltaMaintenanceOutcome>>,
        calls: AtomicUsize,
    }

    #[async_trait]
    impl ProgrammaticDeltaMaintenanceRuntimePort for ScriptedRuntime {
        async fn maintain(
            &self,
            _relation_id: &str,
            _request: &GuardedDeltaMaintenanceRequest,
        ) -> Result<DeltaMaintenanceOutcome, ()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let mut outcomes = self.outcomes.lock().expect("runtime outcomes lock");
            if outcomes.is_empty() {
                return Err(());
            }
            Ok(outcomes.remove(0))
        }
    }

    #[derive(Default)]
    struct DurableHistoryProbe {
        receipt: Mutex<Option<AdministrationCommitReceipt>>,
    }

    #[async_trait]
    impl ProgrammaticDeltaMaintenanceHistoryPort for DurableHistoryProbe {
        async fn commit_readback(
            &self,
            commit: ProgrammaticDeltaMaintenanceCommandCommit,
        ) -> Result<AdministrationCommitObservation, CommandPortError> {
            let command = commit.attempt.command();
            let receipt = AdministrationCommitReceipt {
                workspace_id: command.ownership.workspace_id,
                operation_id: command.identity.operation_id,
                transaction: commit.transaction,
                writer_generation: commit.attempt.execution_owner().fence.generation,
                action: commit.selection.action,
                request: commit.selection.request,
                resulting_head: command.expected_head,
                operation_selection: commit.selection.operation_selection,
            };
            *self.receipt.lock().expect("history receipt lock") = Some(receipt);
            Ok(AdministrationCommitObservation::Committed(receipt))
        }

        async fn read_exact(
            &self,
            _request: AdministrationReconciliationRequest,
        ) -> Result<AdministrationMarkerObservation, CommandPortError> {
            Ok(self.receipt.lock().expect("history receipt lock").map_or(
                AdministrationMarkerObservation::ProvedNotCommitted {
                    evidence: ReconciliationEvidenceRef::from_bytes([0x71; 32]),
                },
                |receipt| AdministrationMarkerObservation::Committed {
                    receipt,
                    evidence: ReconciliationEvidenceRef::from_bytes([0x72; 32]),
                },
            ))
        }
    }

    #[tokio::test]
    async fn supported_retention_inspection_commits_complete_readback() {
        let executor = owner(1, 1, 1);
        let selection = selection(
            AdministrationAction::InspectDeltaRetention,
            GuardedDeltaMaintenanceIntent::InspectRetention,
            executor.fence,
        );
        let outcome = inspected_outcome();
        let runtime = Arc::new(ScriptedRuntime {
            outcomes: Mutex::new(vec![outcome.clone(), outcome]),
            calls: AtomicUsize::new(0),
        });
        let history = Arc::new(DurableHistoryProbe::default());
        let effect = ports(selection, Arc::clone(&history)).build_with_runtime(runtime.clone());
        let executing = executing_record(
            command(AdministrationAction::InspectDeltaRetention, executor.fence),
            executor,
        );

        let PrepareEffectOutcome::Prepared { transaction } = effect
            .prepare(&executing, executor, context(&executing, executor.fence))
            .await
            .expect("prepare supported maintenance")
        else {
            panic!("supported maintenance prepares one transaction")
        };
        let prepared = prepared_record(executing, executor, transaction);
        assert!(matches!(
            effect
                .commit(
                    &prepared,
                    executor,
                    transaction,
                    context(&prepared, executor.fence),
                )
                .await
                .expect("commit supported maintenance"),
            CommitEffectOutcome::Committed {
                result: crate::fabric::command::CommandResult::AdministrationApplied {
                    request: observed_request,
                    selection: observed_selection,
                    ..
                }
            } if observed_request == request() && observed_selection == operation_selection()
        ));
        assert_eq!(runtime.calls.load(Ordering::SeqCst), 2);
        assert!(
            history
                .receipt
                .lock()
                .expect("history receipt lock")
                .is_some()
        );
    }

    #[tokio::test]
    async fn unsupported_checkpoint_is_denied_before_runtime_execution() {
        let executor = owner(1, 1, 1);
        let selection = selection(
            AdministrationAction::CreateDeltaCheckpoint,
            GuardedDeltaMaintenanceIntent::CreateCheckpoint,
            executor.fence,
        );
        let runtime = Arc::new(ScriptedRuntime {
            outcomes: Mutex::new(Vec::new()),
            calls: AtomicUsize::new(0),
        });
        let effect = ports(selection, Arc::new(DurableHistoryProbe::default()))
            .build_with_runtime(runtime.clone());
        let executing = executing_record(
            command(AdministrationAction::CreateDeltaCheckpoint, executor.fence),
            executor,
        );

        assert!(matches!(
            effect
                .prepare(&executing, executor, context(&executing, executor.fence))
                .await
                .expect("unsupported action is a typed decision"),
            PrepareEffectOutcome::KnownFailure {
                failure: CommandFailure {
                    code: FailureCode::BackendUnavailable,
                    class: FailureClass::Permanent,
                    ..
                }
            }
        ));
        assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn restart_reconciles_from_durable_history_without_reexecuting_maintenance() {
        let executor = owner(1, 1, 1);
        let recovery = owner(2, 2, 2);
        let selection = selection(
            AdministrationAction::InspectDeltaRetention,
            GuardedDeltaMaintenanceIntent::InspectRetention,
            executor.fence,
        );
        let outcome = inspected_outcome();
        let history = Arc::new(DurableHistoryProbe::default());
        let first_runtime = Arc::new(ScriptedRuntime {
            outcomes: Mutex::new(vec![outcome.clone(), outcome]),
            calls: AtomicUsize::new(0),
        });
        let first =
            ports(selection.clone(), Arc::clone(&history)).build_with_runtime(first_runtime);
        let executing = executing_record(
            command(AdministrationAction::InspectDeltaRetention, executor.fence),
            executor,
        );
        let PrepareEffectOutcome::Prepared { transaction } = first
            .prepare(&executing, executor, context(&executing, executor.fence))
            .await
            .expect("prepare maintenance")
        else {
            panic!("maintenance prepared")
        };
        let prepared = prepared_record(executing, executor, transaction);
        first
            .commit(
                &prepared,
                executor,
                transaction,
                context(&prepared, executor.fence),
            )
            .await
            .expect("persist maintenance readback");
        let awaiting = awaiting_record(prepared, executor, transaction);

        let restarted_runtime = Arc::new(ScriptedRuntime {
            outcomes: Mutex::new(Vec::new()),
            calls: AtomicUsize::new(0),
        });
        let restarted = ports(selection, history).build_with_runtime(restarted_runtime.clone());
        assert!(matches!(
            restarted
                .reconcile(
                    &awaiting,
                    recovery,
                    transaction,
                    context(&awaiting, recovery.fence),
                )
                .await
                .expect("reconcile durable maintenance history"),
            ReconciliationObservation::Committed { .. }
        ));
        assert_eq!(restarted_runtime.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn safety_rejection_is_known_before_commit() {
        let executor = owner(1, 1, 1);
        let selection = selection(
            AdministrationAction::InspectDeltaRetention,
            GuardedDeltaMaintenanceIntent::InspectRetention,
            executor.fence,
        );
        let runtime = Arc::new(ScriptedRuntime {
            outcomes: Mutex::new(vec![DeltaMaintenanceOutcome::Rejected(
                DeltaMaintenanceRejection::MissingWriterFence,
            )]),
            calls: AtomicUsize::new(0),
        });
        let effect =
            ports(selection, Arc::new(DurableHistoryProbe::default())).build_with_runtime(runtime);
        let executing = executing_record(
            command(AdministrationAction::InspectDeltaRetention, executor.fence),
            executor,
        );
        assert!(matches!(
            effect
                .prepare(&executing, executor, context(&executing, executor.fence))
                .await
                .expect("safety rejection is typed"),
            PrepareEffectOutcome::KnownFailure {
                failure: CommandFailure {
                    code: FailureCode::InvalidInput,
                    class: FailureClass::RetryableBeforeCommit,
                    ..
                }
            }
        ));
    }

    #[test]
    fn validation_intent_identity_binds_every_child_and_its_ordinal() {
        let delta = DeltaRetainedResource::DeltaVersion(
            ExactDeltaPin::new(&Url::parse("memory:///retained/table").unwrap(), 11).unwrap(),
        );
        let result = DeltaRetainedResource::QueryResult([0x52; 32]);
        let ordered = GuardedDeltaMaintenanceIntent::ValidateRetention {
            proposed_deletions: vec![delta.clone(), result.clone()].into(),
        };
        let reordered = GuardedDeltaMaintenanceIntent::ValidateRetention {
            proposed_deletions: vec![result, delta.clone()].into(),
        };
        let changed_version = GuardedDeltaMaintenanceIntent::ValidateRetention {
            proposed_deletions: vec![
                DeltaRetainedResource::DeltaVersion(
                    ExactDeltaPin::new(&Url::parse("memory:///retained/table").unwrap(), 12)
                        .unwrap(),
                ),
                DeltaRetainedResource::QueryResult([0x52; 32]),
            ]
            .into(),
        };

        assert_eq!(
            supported_maintenance_intent_identity(&ordered),
            supported_maintenance_intent_identity(&ordered.clone())
        );
        assert_ne!(
            supported_maintenance_intent_identity(&ordered),
            supported_maintenance_intent_identity(&reordered)
        );
        assert_ne!(
            supported_maintenance_intent_identity(&ordered),
            supported_maintenance_intent_identity(&changed_version)
        );
        assert_eq!(
            supported_maintenance_intent_identity(&GuardedDeltaMaintenanceIntent::CreateCheckpoint),
            None
        );
    }

    #[tokio::test]
    async fn exact_delta_retention_validation_commits_child_set_and_reconciles_after_restart() {
        let temporary = TempDir::new().expect("maintenance relation fixture");
        let table_path = temporary.path().join("maintenance_history");
        fs::create_dir_all(&table_path).expect("create maintenance history directory");
        let root = Url::from_directory_path(&table_path).expect("maintenance history URL");
        let schema = DeltaProgrammaticMaintenanceCommandRelation::schema();
        let kernel: deltalake::kernel::StructType = schema
            .as_ref()
            .try_into_kernel()
            .expect("convert maintenance relation schema");
        let table = DeltaProgrammaticMaintenanceCommandRelation::creation_properties()
            .apply_to(CreateBuilder::new())
            .with_location(root.to_string())
            .with_table_name("programmatic_delta_maintenance_history")
            .with_save_mode(SaveMode::ErrorIfExists)
            .with_columns(kernel.fields().cloned())
            .await
            .expect("create maintenance history table");
        let executor = owner(1, 1, 1);
        let history_predecessor = ExactDeltaPin::new(&root, 1).unwrap();
        let proposed_deletions: Arc<[DeltaRetainedResource]> = vec![
            DeltaRetainedResource::DeltaVersion(
                ExactDeltaPin::new(&Url::parse("memory:///retained/table").unwrap(), 11).unwrap(),
            ),
            DeltaRetainedResource::QueryResult([0x52; 32]),
        ]
        .into();
        let selection = ProgrammaticDeltaMaintenanceCommandSelection::try_new(
            AdministrationAction::ValidateDeltaRetention,
            request(),
            workspace_id(),
            "system.programmatic_relation_observation",
            GuardedDeltaMaintenanceRequest::try_new(
                ExactDeltaPin::new(&Url::parse("memory:///maintenance").unwrap(), 7).unwrap(),
                [0x31; 32],
                executor.fence,
                operation_id(),
                GuardedDeltaMaintenanceIntent::ValidateRetention { proposed_deletions },
            )
            .unwrap(),
            history_predecessor.clone(),
            operation_selection(),
        )
        .unwrap();
        let request_batch =
            DeltaProgrammaticMaintenanceCommandRelation::encode_request(&selection).unwrap();
        assert_eq!(request_batch.num_rows(), 3, "parent plus two child rows");
        let table = table
            .write(vec![request_batch])
            .with_save_mode(SaveMode::Append)
            .await
            .expect("append exact maintenance request row");
        assert_eq!(table.version(), Some(1));
        let session = Arc::new(
            SessionStateBuilder::new()
                .with_default_features()
                .with_config(
                    SessionConfig::new()
                        .set_bool("datafusion.execution.parquet.pushdown_filters", false),
                )
                .with_query_planner(DeltaPlanner::new())
                .build(),
        );
        let relation = Arc::new(
            DeltaProgrammaticMaintenanceCommandRelation::try_from_loaded_table(
                Arc::clone(&session),
                history_predecessor,
                table,
            )
            .unwrap(),
        );
        let diagnostics = ProgrammaticDeltaMaintenanceCommandDiagnostics::try_new(
            DiagnosticRef::from_bytes([0xa1; 32]),
            DiagnosticRef::from_bytes([0xa2; 32]),
            DiagnosticRef::from_bytes([0xa3; 32]),
            DiagnosticRef::from_bytes([0xa4; 32]),
        )
        .unwrap();
        let outcome = validated_outcome();
        let runtime = Arc::new(ScriptedRuntime {
            outcomes: Mutex::new(vec![outcome.clone(), outcome]),
            calls: AtomicUsize::new(0),
        });
        let effect = relation
            .administration_ports(diagnostics)
            .build_with_runtime(runtime);
        let executing = executing_record(
            command(AdministrationAction::ValidateDeltaRetention, executor.fence),
            executor,
        );
        let prepare = effect
            .prepare(&executing, executor, context(&executing, executor.fence))
            .await
            .expect("prepare Delta-backed maintenance command");
        let PrepareEffectOutcome::Prepared { transaction } = prepare else {
            panic!("Delta-backed request prepares: {prepare:?}")
        };
        let prepared = prepared_record(executing, executor, transaction);
        assert!(matches!(
            effect
                .commit(
                    &prepared,
                    executor,
                    transaction,
                    context(&prepared, executor.fence),
                )
                .await
                .expect("commit Delta-backed maintenance history"),
            CommitEffectOutcome::Committed { .. }
        ));
        let awaiting = awaiting_record(prepared, executor, transaction);

        let committed_pin = ExactDeltaPin::new(&root, 2).unwrap();
        let committed_table = DeltaTableBuilder::from_url(root)
            .unwrap()
            .with_version(2)
            .load()
            .await
            .expect("reopen exact maintenance commit version");
        let reconstructed = Arc::new(
            DeltaProgrammaticMaintenanceCommandRelation::try_from_loaded_table(
                session,
                committed_pin,
                committed_table,
            )
            .unwrap(),
        );
        let restarted_runtime = Arc::new(ScriptedRuntime {
            outcomes: Mutex::new(Vec::new()),
            calls: AtomicUsize::new(0),
        });
        let restarted = reconstructed
            .administration_ports(diagnostics)
            .build_with_runtime(restarted_runtime.clone());
        let recovery = owner(2, 2, 2);
        assert!(matches!(
            restarted
                .reconcile(
                    &awaiting,
                    recovery,
                    transaction,
                    context(&awaiting, recovery.fence),
                )
                .await
                .expect("reconcile exact Delta maintenance history"),
            ReconciliationObservation::Committed { .. }
        ));
        assert_eq!(restarted_runtime.calls.load(Ordering::SeqCst), 0);
    }

    fn ports(
        selection: ProgrammaticDeltaMaintenanceCommandSelection,
        history: Arc<DurableHistoryProbe>,
    ) -> ProgrammaticDeltaMaintenanceAdministrationPorts {
        ProgrammaticDeltaMaintenanceAdministrationPorts::new(
            Arc::new(StaticRequests { selection }),
            history,
            ProgrammaticDeltaMaintenanceCommandDiagnostics::try_new(
                DiagnosticRef::from_bytes([0x81; 32]),
                DiagnosticRef::from_bytes([0x82; 32]),
                DiagnosticRef::from_bytes([0x83; 32]),
                DiagnosticRef::from_bytes([0x84; 32]),
            )
            .expect("nonzero diagnostics"),
        )
    }

    fn selection(
        action: AdministrationAction,
        intent: GuardedDeltaMaintenanceIntent,
        writer_fence: WriterFence,
    ) -> ProgrammaticDeltaMaintenanceCommandSelection {
        ProgrammaticDeltaMaintenanceCommandSelection::try_new(
            action,
            request(),
            workspace_id(),
            "system.programmatic_relation_observation",
            GuardedDeltaMaintenanceRequest::try_new(
                ExactDeltaPin::new(&Url::parse("memory:///maintenance").unwrap(), 7).unwrap(),
                [0x31; 32],
                writer_fence,
                operation_id(),
                intent,
            )
            .unwrap(),
            ExactDeltaPin::new(&Url::parse("memory:///maintenance-history").unwrap(), 3).unwrap(),
            operation_selection(),
        )
        .unwrap()
    }

    fn inspected_outcome() -> DeltaMaintenanceOutcome {
        DeltaMaintenanceOutcome::RetentionInspected {
            evidence_revision: NonZeroU64::new(1).unwrap(),
            closure: DeltaRetentionClosure::try_new(0, DeltaRetentionAuthorityKind::ALL, [])
                .unwrap(),
        }
    }

    fn validated_outcome() -> DeltaMaintenanceOutcome {
        DeltaMaintenanceOutcome::RetentionValidated {
            evidence_revision: NonZeroU64::new(1).unwrap(),
        }
    }

    fn command(action: AdministrationAction, writer_fence: WriterFence) -> FabricCommand {
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
                    [0x07; 32],
                ),
                source_authority: crate::fabric::command::SourceAuthorityRef::from_bytes(
                    [0x08; 32],
                ),
                provider_release: crate::fabric::command::ProviderReleaseRef::from_bytes(
                    [0x09; 32],
                ),
                source_generation: SourceGeneration::new(7),
                provider_set: ProviderSetRef::from_bytes([0x0a; 32]),
            },
            resources: ResourceEnvelopeRef::from_bytes([0x0b; 32]),
            payload: FabricCommandPayload::Administer {
                action,
                request: request(),
            },
        }
    }

    fn executing_record(
        command: FabricCommand,
        owner: crate::fabric::command::ExecutionOwner,
    ) -> crate::fabric::command::CommandRecord {
        let AdmissionOutcome::New(admitted) = CommandReducer::admit(
            None,
            &command,
            AdmissionContext {
                workspace_id: workspace_id(),
                current_head: command.expected_head,
                active_fence: command.writer_fence,
                authorization: AuthorizationDecision::Authorized(command.ownership.authorization),
            },
        )
        .unwrap() else {
            panic!("fresh maintenance command is newly admitted")
        };
        CommandReducer::reduce(
            &admitted,
            CommandEvent::Start { owner },
            context(&admitted, owner.fence),
        )
        .unwrap()
        .record
    }

    fn prepared_record(
        executing: crate::fabric::command::CommandRecord,
        owner: crate::fabric::command::ExecutionOwner,
        transaction: TransactionRef,
    ) -> crate::fabric::command::CommandRecord {
        CommandReducer::reduce(
            &executing,
            CommandEvent::PrepareCommit { owner, transaction },
            context(&executing, owner.fence),
        )
        .unwrap()
        .record
    }

    fn awaiting_record(
        prepared: crate::fabric::command::CommandRecord,
        owner: crate::fabric::command::ExecutionOwner,
        transaction: TransactionRef,
    ) -> crate::fabric::command::CommandRecord {
        CommandReducer::reduce(
            &prepared,
            CommandEvent::ReportUnknownCommit {
                owner,
                transaction,
                unknown: UnknownCommit {
                    reason: UnknownCommitReason::ProcessInterrupted,
                    diagnostic: DiagnosticRef::from_bytes([0x91; 32]),
                },
            },
            context(&prepared, owner.fence),
        )
        .unwrap()
        .record
    }

    fn context(
        record: &crate::fabric::command::CommandRecord,
        active_fence: WriterFence,
    ) -> ReductionContext {
        ReductionContext {
            current_head: record.command().expected_head,
            active_fence,
        }
    }

    fn owner(actor: u8, lease: u8, generation: u64) -> crate::fabric::command::ExecutionOwner {
        crate::fabric::command::ExecutionOwner {
            actor_id: ActorId::from_bytes([actor; 16]),
            fence: WriterFence {
                lease_id: LeaseId::from_bytes([lease; 16]),
                generation: WriterGeneration::new(generation).unwrap(),
            },
        }
    }

    fn workspace_id() -> WorkspaceId {
        WorkspaceId::from_bytes([0x11; 16])
    }

    fn operation_id() -> OperationId {
        OperationId::from_bytes([0x12; 16])
    }

    fn request() -> AdministrationRequestRef {
        AdministrationRequestRef::from_bytes([0x13; 32])
    }

    fn operation_selection() -> OperationSelectionRef {
        OperationSelectionRef::from_bytes([0x14; 32])
    }
}
