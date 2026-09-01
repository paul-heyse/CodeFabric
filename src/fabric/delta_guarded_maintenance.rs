//! Successor-only, exact-version Delta maintenance guarded by application authority.
//!
//! delta-rs owns physical checkpoint and vacuum planning. Its pinned optimize
//! path is represented but unavailable because it discards the caller's
//! application transaction and retry policy. This module owns the safety
//! decision which delta-rs cannot make: the exact
//! activation head and writer fence, the complete retention closure, proof
//! references, and unresolved commit identities. The authority is re-read for
//! every request; the small cache retained here is diagnostic only.
//!
//! Destructive vacuum is intentionally unavailable. The pinned delta-rs API can
//! produce a native dry-run candidate set, but cannot atomically bind that exact
//! reviewed set and application evidence revision to a later deletion. Returning
//! a typed unavailable outcome is safer than reopening the race with a second
//! independently planned destructive invocation.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use deltalake::checkpoints::create_checkpoint;
use deltalake::operations::vacuum::VacuumMode;
use deltalake::table::config::TablePropertiesExt;
use deltalake::{DeltaTable, DeltaTableBuilder, DeltaTableError};
use thiserror::Error;

use super::command::{OperationId, WriterFence};
use super::delta_exact::{
    DeltaRetainedResource, DeltaRetentionAuthorityKind, DeltaRetentionClosure,
    DeltaRetentionClosureError, ExactDeltaPin, ExactDeltaProviderError, ValidatedDeltaSnapshot,
};

/// Durable application sources required before a maintenance decision is complete.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DeltaMaintenanceEvidenceSource {
    /// Activation-event topology and its exact table pins.
    ActivationEvents,
    /// Current single-writer lease and generation.
    WriterFence,
    /// Union of every retention/lease source, including observed-empty sources.
    RetentionClosure,
    /// Exact proof receipts which retain historical table state.
    ProofReferences,
    /// Writes whose Delta commit outcome has not been reconciled.
    UncertainCommits,
}

impl DeltaMaintenanceEvidenceSource {
    /// Complete set which must be observed for every decision.
    pub const ALL: [Self; 5] = [
        Self::ActivationEvents,
        Self::WriterFence,
        Self::RetentionClosure,
        Self::ProofReferences,
        Self::UncertainCommits,
    ];
}

/// One application-owned activation event carrying its exact Delta state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeltaMaintenanceActivationEvent {
    event_id: [u8; 32],
    parent_event_id: Option<[u8; 32]>,
    table: ExactDeltaPin,
}

impl DeltaMaintenanceActivationEvent {
    /// Construct an activation topology row without deriving identity from a label.
    ///
    /// # Errors
    ///
    /// Rejects zero event or parent identities.
    pub fn try_new(
        event_id: [u8; 32],
        parent_event_id: Option<[u8; 32]>,
        table: ExactDeltaPin,
    ) -> Result<Self, DeltaMaintenanceInputError> {
        if is_zero(&event_id) {
            return Err(DeltaMaintenanceInputError::ZeroActivationEvent);
        }
        if parent_event_id.is_some_and(|parent| is_zero(&parent)) {
            return Err(DeltaMaintenanceInputError::ZeroActivationParent);
        }
        Ok(Self {
            event_id,
            parent_event_id,
            table,
        })
    }

    #[must_use]
    pub const fn event_id(&self) -> &[u8; 32] {
        &self.event_id
    }

    #[must_use]
    pub const fn parent_event_id(&self) -> Option<&[u8; 32]> {
        self.parent_event_id.as_ref()
    }

    #[must_use]
    pub const fn table(&self) -> &ExactDeltaPin {
        &self.table
    }
}

/// One exact proof receipt which keeps a Delta version reachable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeltaMaintenanceProofReference {
    proof_receipt: [u8; 32],
    protected: ExactDeltaPin,
}

impl DeltaMaintenanceProofReference {
    /// Construct a proof reference from canonical receipt bytes and its exact pin.
    ///
    /// # Errors
    ///
    /// Rejects a zero receipt identity.
    pub fn try_new(
        proof_receipt: [u8; 32],
        protected: ExactDeltaPin,
    ) -> Result<Self, DeltaMaintenanceInputError> {
        if is_zero(&proof_receipt) {
            return Err(DeltaMaintenanceInputError::ZeroProofReceipt);
        }
        Ok(Self {
            proof_receipt,
            protected,
        })
    }

    #[must_use]
    pub const fn proof_receipt(&self) -> &[u8; 32] {
        &self.proof_receipt
    }

    #[must_use]
    pub const fn protected(&self) -> &ExactDeltaPin {
        &self.protected
    }
}

/// One unresolved Delta write identity supplied by reconciliation authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeltaMaintenanceUncertainCommit {
    operation_id: OperationId,
    predecessor: ExactDeltaPin,
}

impl DeltaMaintenanceUncertainCommit {
    /// Construct an unresolved commit row.
    ///
    /// # Errors
    ///
    /// Rejects a zero operation identity.
    pub fn try_new(
        operation_id: OperationId,
        predecessor: ExactDeltaPin,
    ) -> Result<Self, DeltaMaintenanceInputError> {
        if is_zero(operation_id.as_bytes()) {
            return Err(DeltaMaintenanceInputError::ZeroOperationIdentity);
        }
        Ok(Self {
            operation_id,
            predecessor,
        })
    }

    #[must_use]
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }

    #[must_use]
    pub const fn predecessor(&self) -> &ExactDeltaPin {
        &self.predecessor
    }
}

/// One exact, revisioned observation read from durable application authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeltaMaintenanceSafetyEvidence {
    revision: NonZeroU64,
    observed_sources: BTreeSet<DeltaMaintenanceEvidenceSource>,
    writer_fence: Option<WriterFence>,
    activation_events: Vec<DeltaMaintenanceActivationEvent>,
    retention_closure: Option<DeltaRetentionClosure>,
    proof_references: Vec<DeltaMaintenanceProofReference>,
    uncertain_commits: Vec<DeltaMaintenanceUncertainCommit>,
}

impl DeltaMaintenanceSafetyEvidence {
    /// Assemble one port observation. Optional fields distinguish missing
    /// material from an explicitly observed empty relation.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        revision: NonZeroU64,
        observed_sources: impl IntoIterator<Item = DeltaMaintenanceEvidenceSource>,
        writer_fence: Option<WriterFence>,
        activation_events: Vec<DeltaMaintenanceActivationEvent>,
        retention_closure: Option<DeltaRetentionClosure>,
        proof_references: Vec<DeltaMaintenanceProofReference>,
        uncertain_commits: Vec<DeltaMaintenanceUncertainCommit>,
    ) -> Self {
        Self {
            revision,
            observed_sources: observed_sources.into_iter().collect(),
            writer_fence,
            activation_events,
            retention_closure,
            proof_references,
            uncertain_commits,
        }
    }

    #[must_use]
    pub const fn revision(&self) -> NonZeroU64 {
        self.revision
    }
}

/// Durable authority queried afresh for every maintenance request.
#[async_trait]
pub trait DeltaMaintenanceSafetyPort: Send + Sync {
    /// Read the evidence relevant to one exact target table.
    async fn observe(
        &self,
        target: &ExactDeltaPin,
    ) -> Result<DeltaMaintenanceSafetyEvidence, DeltaMaintenanceAuthorityError>;
}

/// A port failure is not equivalent to observed-empty safety evidence.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("Delta maintenance safety authority unavailable: {detail}")]
pub struct DeltaMaintenanceAuthorityError {
    detail: Arc<str>,
}

impl DeltaMaintenanceAuthorityError {
    #[must_use]
    pub fn new(detail: impl Into<Arc<str>>) -> Self {
        Self {
            detail: detail.into(),
        }
    }

    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

/// Application transaction marker attached to a native optimize commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeltaMaintenanceApplicationTransaction {
    application_id: Arc<str>,
    application_version: i64,
}

impl DeltaMaintenanceApplicationTransaction {
    /// Construct a non-empty, nonnegative transaction marker.
    ///
    /// # Errors
    ///
    /// Rejects empty application identity or a negative version.
    pub fn try_new(
        application_id: impl Into<Arc<str>>,
        application_version: i64,
    ) -> Result<Self, DeltaMaintenanceInputError> {
        let application_id = application_id.into();
        if application_id.trim().is_empty() {
            return Err(DeltaMaintenanceInputError::EmptyApplicationIdentity);
        }
        if application_version < 0 {
            return Err(DeltaMaintenanceInputError::NegativeApplicationVersion);
        }
        Ok(Self {
            application_id,
            application_version,
        })
    }

    #[must_use]
    pub fn application_id(&self) -> &str {
        &self.application_id
    }

    #[must_use]
    pub const fn application_version(&self) -> i64 {
        self.application_version
    }
}

/// Fully explicit native optimize policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeltaOptimizeCompactSpec {
    target_file_size: NonZeroU64,
    max_concurrent_tasks: NonZeroUsize,
    transaction: DeltaMaintenanceApplicationTransaction,
}

impl DeltaOptimizeCompactSpec {
    #[must_use]
    pub const fn new(
        target_file_size: NonZeroU64,
        max_concurrent_tasks: NonZeroUsize,
        transaction: DeltaMaintenanceApplicationTransaction,
    ) -> Self {
        Self {
            target_file_size,
            max_concurrent_tasks,
            transaction,
        }
    }

    #[must_use]
    pub const fn target_file_size(&self) -> NonZeroU64 {
        self.target_file_size
    }

    #[must_use]
    pub const fn max_concurrent_tasks(&self) -> NonZeroUsize {
        self.max_concurrent_tasks
    }

    #[must_use]
    pub const fn transaction(&self) -> &DeltaMaintenanceApplicationTransaction {
        &self.transaction
    }
}

/// Exact dry-run receipt produced only from a native delta-rs vacuum plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeltaVacuumDryRunReceipt {
    target: ExactDeltaPin,
    evidence_revision: NonZeroU64,
    retention_seconds: u64,
    keep_versions: Arc<[u64]>,
    candidates: Arc<[String]>,
    candidate_digest: [u8; 32],
}

impl DeltaVacuumDryRunReceipt {
    #[must_use]
    pub const fn target(&self) -> &ExactDeltaPin {
        &self.target
    }

    #[must_use]
    pub const fn evidence_revision(&self) -> NonZeroU64 {
        self.evidence_revision
    }

    #[must_use]
    pub const fn retention_seconds(&self) -> u64 {
        self.retention_seconds
    }

    #[must_use]
    pub fn keep_versions(&self) -> &[u64] {
        &self.keep_versions
    }

    #[must_use]
    pub fn candidates(&self) -> &[String] {
        &self.candidates
    }

    #[must_use]
    pub const fn candidate_digest(&self) -> &[u8; 32] {
        &self.candidate_digest
    }
}

/// Closed maintenance intent set. No variant discovers a current/latest version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuardedDeltaMaintenanceIntent {
    /// Inspect the complete application retention closure.
    InspectRetention,
    /// Validate a proposed logical resource-deletion set.
    ValidateRetention {
        proposed_deletions: Arc<[DeltaRetainedResource]>,
    },
    /// Create a replay accelerator for the exact semantic version.
    CreateCheckpoint,
    /// Native delta-rs compact optimization with an exact predecessor.
    OptimizeCompact(DeltaOptimizeCompactSpec),
    /// Produce a native lite-vacuum dry-run receipt under table-configured retention.
    VacuumDryRun { expected_retention_seconds: u64 },
    /// Request destructive execution using a previous exact dry-run receipt.
    VacuumExecute(DeltaVacuumDryRunReceipt),
}

/// Typed request binding the intent to one head, fence, operation, and exact table pin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuardedDeltaMaintenanceRequest {
    target: ExactDeltaPin,
    expected_activation_head: [u8; 32],
    writer_fence: WriterFence,
    operation_id: OperationId,
    intent: GuardedDeltaMaintenanceIntent,
}

impl GuardedDeltaMaintenanceRequest {
    /// Construct a fully typed request.
    ///
    /// # Errors
    ///
    /// Rejects zero activation-head or operation identities.
    pub fn try_new(
        target: ExactDeltaPin,
        expected_activation_head: [u8; 32],
        writer_fence: WriterFence,
        operation_id: OperationId,
        intent: GuardedDeltaMaintenanceIntent,
    ) -> Result<Self, DeltaMaintenanceInputError> {
        if is_zero(&expected_activation_head) {
            return Err(DeltaMaintenanceInputError::ZeroActivationEvent);
        }
        if is_zero(operation_id.as_bytes()) {
            return Err(DeltaMaintenanceInputError::ZeroOperationIdentity);
        }
        Ok(Self {
            target,
            expected_activation_head,
            writer_fence,
            operation_id,
            intent,
        })
    }

    #[must_use]
    pub const fn target(&self) -> &ExactDeltaPin {
        &self.target
    }

    #[must_use]
    pub const fn expected_activation_head(&self) -> &[u8; 32] {
        &self.expected_activation_head
    }

    #[must_use]
    pub const fn writer_fence(&self) -> WriterFence {
        self.writer_fence
    }

    #[must_use]
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }

    #[must_use]
    pub const fn intent(&self) -> &GuardedDeltaMaintenanceIntent {
        &self.intent
    }
}

/// Input-shape failures rejected before authority or delta-rs is invoked.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DeltaMaintenanceInputError {
    #[error("activation event identity must be nonzero")]
    ZeroActivationEvent,
    #[error("activation parent identity must be nonzero")]
    ZeroActivationParent,
    #[error("proof receipt identity must be nonzero")]
    ZeroProofReceipt,
    #[error("operation identity must be nonzero")]
    ZeroOperationIdentity,
    #[error("maintenance application identity must be non-empty")]
    EmptyApplicationIdentity,
    #[error("maintenance application version must be nonnegative")]
    NegativeApplicationVersion,
}

/// Fail-closed safety decision. Absence never maps to admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeltaMaintenanceRejection {
    MissingEvidenceSources {
        missing: Vec<DeltaMaintenanceEvidenceSource>,
    },
    MissingWriterFence,
    MissingRetentionClosure,
    StaleWriterFence {
        requested: WriterFence,
        observed: WriterFence,
    },
    EmptyActivationHistory,
    DuplicateActivationEvent {
        event_id: [u8; 32],
    },
    MissingActivationParent {
        event_id: [u8; 32],
        parent_event_id: [u8; 32],
    },
    SplitActivationHead {
        terminal_heads: Vec<[u8; 32]>,
    },
    AmbiguousActivationRoot {
        roots: Vec<[u8; 32]>,
    },
    CyclicOrDisconnectedActivationHistory,
    ActivationHeadMismatch {
        requested: [u8; 32],
        observed: [u8; 32],
    },
    ActivationTargetMismatch {
        requested: ExactDeltaPin,
        observed: ExactDeltaPin,
    },
    UncertainCommit {
        operations: Vec<OperationId>,
    },
    RetentionBlocked {
        authorities: Vec<DeltaRetentionAuthorityKind>,
    },
    ProofReferenceBlocked {
        receipts: Vec<[u8; 32]>,
    },
    ProtectedRetentionResource(DeltaRetainedResource),
    VacuumReceiptTargetMismatch,
    VacuumReceiptEvidenceChanged {
        reviewed: NonZeroU64,
        current: NonZeroU64,
    },
}

/// Native capability is unavailable without weakening the application contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeltaMaintenanceUnavailable {
    /// Pinned delta-rs cannot bind the reviewed dry-run set atomically to deletion.
    AtomicVacuumApprovalBinding,
    /// Pinned delta-rs optimize drops the application transaction and caller
    /// retry policy, so it cannot be used as a controlled successor write.
    OptimizeCommitIdentityAndRetryControl,
    /// Checkpoint creation failed without changing semantic Delta state.
    CheckpointCreation { detail: Arc<str> },
}

/// Complete outcome. Rejection and unavailability are data, never inferred success.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeltaMaintenanceOutcome {
    RetentionInspected {
        evidence_revision: NonZeroU64,
        closure: DeltaRetentionClosure,
    },
    RetentionValidated {
        evidence_revision: NonZeroU64,
    },
    CheckpointCreated {
        target: ExactDeltaPin,
        evidence_revision: NonZeroU64,
    },
    VacuumDryRun(DeltaVacuumDryRunReceipt),
    Rejected(DeltaMaintenanceRejection),
    Unavailable(DeltaMaintenanceUnavailable),
}

/// Execution or postcondition failure which is not a safety decision.
#[derive(Debug, Error)]
pub enum DeltaMaintenanceError {
    #[error(transparent)]
    Authority(#[from] DeltaMaintenanceAuthorityError),
    #[error(transparent)]
    Exact(#[from] ExactDeltaProviderError),
    #[error(transparent)]
    Retention(#[from] DeltaRetentionClosureError),
    #[error(transparent)]
    Delta(#[from] DeltaTableError),
    #[error("native maintenance postcondition failed: {0}")]
    Postcondition(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DeltaMaintenanceDerivedCache {
    target: ExactDeltaPin,
    evidence_revision: NonZeroU64,
}

/// Exact-version native maintenance controller.
pub struct GuardedDeltaMaintenance {
    authority: Arc<dyn DeltaMaintenanceSafetyPort>,
    derived_cache: Mutex<Option<DeltaMaintenanceDerivedCache>>,
}

impl fmt::Debug for GuardedDeltaMaintenance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GuardedDeltaMaintenance")
            .field("cached_revision", &self.cached_revision())
            .finish_non_exhaustive()
    }
}

impl GuardedDeltaMaintenance {
    /// Construct with explicit application-owned authority. There is no default
    /// or observed-empty fallback.
    #[must_use]
    pub fn new(authority: Arc<dyn DeltaMaintenanceSafetyPort>) -> Self {
        Self {
            authority,
            derived_cache: Mutex::new(None),
        }
    }

    /// Remove derived observations. The next decision still reads authority.
    pub fn clear_derived_cache(&self) {
        *self
            .derived_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }

    /// Diagnostic revision only; never used for admission.
    #[must_use]
    pub fn cached_revision(&self) -> Option<NonZeroU64> {
        self.derived_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|cache| cache.evidence_revision)
    }

    /// Execute one exact request. `loaded_target` must already be the requested
    /// root and version; this method never updates it to latest before admission.
    pub async fn execute(
        &self,
        request: &GuardedDeltaMaintenanceRequest,
        loaded_target: DeltaTable,
    ) -> Result<DeltaMaintenanceOutcome, DeltaMaintenanceError> {
        ValidatedDeltaSnapshot::try_from_loaded_table(loaded_target.clone(), &request.target)?;

        let evidence = self.authority.observe(&request.target).await?;
        *self
            .derived_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(DeltaMaintenanceDerivedCache {
                target: request.target.clone(),
                evidence_revision: evidence.revision,
            });

        let admitted = match admit(request, &evidence) {
            Ok(admitted) => admitted,
            Err(rejection) => return Ok(DeltaMaintenanceOutcome::Rejected(rejection)),
        };

        match &request.intent {
            GuardedDeltaMaintenanceIntent::InspectRetention => {
                Ok(DeltaMaintenanceOutcome::RetentionInspected {
                    evidence_revision: evidence.revision,
                    closure: admitted.retention.clone(),
                })
            }
            GuardedDeltaMaintenanceIntent::ValidateRetention { proposed_deletions } => {
                match admitted
                    .retention
                    .validate_resource_deletions(proposed_deletions)
                {
                    Ok(()) => Ok(DeltaMaintenanceOutcome::RetentionValidated {
                        evidence_revision: evidence.revision,
                    }),
                    Err(DeltaRetentionClosureError::ProtectedResourceDeletion(resource)) => {
                        Ok(DeltaMaintenanceOutcome::Rejected(
                            DeltaMaintenanceRejection::ProtectedRetentionResource(*resource),
                        ))
                    }
                    Err(error) => Err(error.into()),
                }
            }
            GuardedDeltaMaintenanceIntent::CreateCheckpoint => {
                self.create_exact_checkpoint(&request.target, loaded_target, evidence.revision)
                    .await
            }
            GuardedDeltaMaintenanceIntent::OptimizeCompact(_) => {
                Ok(DeltaMaintenanceOutcome::Unavailable(
                    DeltaMaintenanceUnavailable::OptimizeCommitIdentityAndRetryControl,
                ))
            }
            GuardedDeltaMaintenanceIntent::VacuumDryRun {
                expected_retention_seconds,
            } => {
                self.vacuum_dry_run(
                    &request.target,
                    loaded_target,
                    evidence.revision,
                    admitted.retention,
                    *expected_retention_seconds,
                )
                .await
            }
            GuardedDeltaMaintenanceIntent::VacuumExecute(receipt) => {
                if receipt.target != request.target {
                    return Ok(DeltaMaintenanceOutcome::Rejected(
                        DeltaMaintenanceRejection::VacuumReceiptTargetMismatch,
                    ));
                }
                if receipt.evidence_revision != evidence.revision {
                    return Ok(DeltaMaintenanceOutcome::Rejected(
                        DeltaMaintenanceRejection::VacuumReceiptEvidenceChanged {
                            reviewed: receipt.evidence_revision,
                            current: evidence.revision,
                        },
                    ));
                }
                let mut authorities = admitted
                    .retention
                    .active_claims()
                    .iter()
                    .map(|claim| claim.authority_kind())
                    .collect::<Vec<_>>();
                authorities.sort();
                authorities.dedup();
                if !authorities.is_empty() {
                    return Ok(DeltaMaintenanceOutcome::Rejected(
                        DeltaMaintenanceRejection::RetentionBlocked { authorities },
                    ));
                }
                let mut receipts = evidence
                    .proof_references
                    .iter()
                    .filter(|reference| {
                        reference.protected.canonical_root() == request.target.canonical_root()
                    })
                    .map(|reference| reference.proof_receipt)
                    .collect::<Vec<_>>();
                receipts.sort();
                receipts.dedup();
                if !receipts.is_empty() {
                    return Ok(DeltaMaintenanceOutcome::Rejected(
                        DeltaMaintenanceRejection::ProofReferenceBlocked { receipts },
                    ));
                }
                Ok(DeltaMaintenanceOutcome::Unavailable(
                    DeltaMaintenanceUnavailable::AtomicVacuumApprovalBinding,
                ))
            }
        }
    }

    async fn create_exact_checkpoint(
        &self,
        target: &ExactDeltaPin,
        loaded_target: DeltaTable,
        evidence_revision: NonZeroU64,
    ) -> Result<DeltaMaintenanceOutcome, DeltaMaintenanceError> {
        if let Err(source) = create_checkpoint(&loaded_target, None).await {
            return Ok(DeltaMaintenanceOutcome::Unavailable(
                DeltaMaintenanceUnavailable::CheckpointCreation {
                    detail: Arc::from(source.to_string()),
                },
            ));
        }
        ValidatedDeltaSnapshot::try_from_loaded_table(loaded_target, target)?;
        let reopened = DeltaTableBuilder::from_url(target.canonical_root().clone())?
            .with_version(target.version())
            .load()
            .await?;
        ValidatedDeltaSnapshot::try_from_loaded_table(reopened, target)?;
        Ok(DeltaMaintenanceOutcome::CheckpointCreated {
            target: target.clone(),
            evidence_revision,
        })
    }

    async fn vacuum_dry_run(
        &self,
        target: &ExactDeltaPin,
        loaded_target: DeltaTable,
        evidence_revision: NonZeroU64,
        retention: &DeltaRetentionClosure,
        expected_retention_seconds: u64,
    ) -> Result<DeltaMaintenanceOutcome, DeltaMaintenanceError> {
        let configured_retention = loaded_target
            .snapshot()?
            .table_config()
            .deleted_file_retention_duration()
            .as_secs();
        if configured_retention != expected_retention_seconds {
            return Err(DeltaMaintenanceError::Postcondition(format!(
                "table retention was {configured_retention}s, request pinned {expected_retention_seconds}s"
            )));
        }
        let keep_versions = retention.keep_versions_for(target.canonical_root())?;
        retention.validate_vacuum_dry_run_contract(
            target.canonical_root(),
            true,
            &keep_versions,
        )?;
        let (after, metrics) = loaded_target
            .vacuum()
            .with_keep_versions(&keep_versions)
            .with_mode(VacuumMode::Lite)
            .with_dry_run(true)
            .await?;
        ValidatedDeltaSnapshot::try_from_loaded_table(after, target)?;
        if !metrics.dry_run {
            return Err(DeltaMaintenanceError::Postcondition(
                "delta-rs returned destructive vacuum metrics for a dry run".to_owned(),
            ));
        }
        let mut candidates = metrics.files_deleted;
        candidates.sort();
        if candidates.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DeltaMaintenanceError::Postcondition(
                "native vacuum dry run returned duplicate candidates".to_owned(),
            ));
        }
        let candidate_digest = candidate_digest(&candidates);
        Ok(DeltaMaintenanceOutcome::VacuumDryRun(
            DeltaVacuumDryRunReceipt {
                target: target.clone(),
                evidence_revision,
                retention_seconds: configured_retention,
                keep_versions: keep_versions.into(),
                candidates: candidates.into(),
                candidate_digest,
            },
        ))
    }
}

struct AdmittedMaintenance<'a> {
    retention: &'a DeltaRetentionClosure,
}

fn admit<'a>(
    request: &GuardedDeltaMaintenanceRequest,
    evidence: &'a DeltaMaintenanceSafetyEvidence,
) -> Result<AdmittedMaintenance<'a>, DeltaMaintenanceRejection> {
    let missing = DeltaMaintenanceEvidenceSource::ALL
        .into_iter()
        .filter(|source| !evidence.observed_sources.contains(source))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(DeltaMaintenanceRejection::MissingEvidenceSources { missing });
    }
    let observed_fence = evidence
        .writer_fence
        .ok_or(DeltaMaintenanceRejection::MissingWriterFence)?;
    if observed_fence != request.writer_fence {
        return Err(DeltaMaintenanceRejection::StaleWriterFence {
            requested: request.writer_fence,
            observed: observed_fence,
        });
    }
    let retention = evidence
        .retention_closure
        .as_ref()
        .ok_or(DeltaMaintenanceRejection::MissingRetentionClosure)?;
    let activation_head = resolve_activation_head(&evidence.activation_events)?;
    if activation_head.event_id != request.expected_activation_head {
        return Err(DeltaMaintenanceRejection::ActivationHeadMismatch {
            requested: request.expected_activation_head,
            observed: activation_head.event_id,
        });
    }
    if activation_head.table != request.target {
        return Err(DeltaMaintenanceRejection::ActivationTargetMismatch {
            requested: request.target.clone(),
            observed: activation_head.table.clone(),
        });
    }
    let mut operations = evidence
        .uncertain_commits
        .iter()
        .filter(|commit| commit.predecessor.canonical_root() == request.target.canonical_root())
        .map(|commit| commit.operation_id)
        .collect::<Vec<_>>();
    operations.sort();
    operations.dedup();
    if !operations.is_empty() {
        return Err(DeltaMaintenanceRejection::UncertainCommit { operations });
    }
    Ok(AdmittedMaintenance { retention })
}

fn resolve_activation_head(
    events: &[DeltaMaintenanceActivationEvent],
) -> Result<&DeltaMaintenanceActivationEvent, DeltaMaintenanceRejection> {
    if events.is_empty() {
        return Err(DeltaMaintenanceRejection::EmptyActivationHistory);
    }
    let mut by_id = BTreeMap::new();
    for (index, event) in events.iter().enumerate() {
        if by_id.insert(event.event_id, index).is_some() {
            return Err(DeltaMaintenanceRejection::DuplicateActivationEvent {
                event_id: event.event_id,
            });
        }
    }
    let mut child_count = vec![0_usize; events.len()];
    let mut children = vec![Vec::new(); events.len()];
    let mut roots = Vec::new();
    for (index, event) in events.iter().enumerate() {
        match event.parent_event_id {
            Some(parent_id) => {
                let parent_index = by_id.get(&parent_id).copied().ok_or(
                    DeltaMaintenanceRejection::MissingActivationParent {
                        event_id: event.event_id,
                        parent_event_id: parent_id,
                    },
                )?;
                child_count[parent_index] += 1;
                children[parent_index].push(index);
            }
            None => roots.push(event.event_id),
        }
    }
    let mut terminal_heads = events
        .iter()
        .zip(&child_count)
        .filter_map(|(event, count)| (*count == 0).then_some(event.event_id))
        .collect::<Vec<_>>();
    terminal_heads.sort();
    if terminal_heads.len() != 1 {
        return Err(if terminal_heads.is_empty() {
            DeltaMaintenanceRejection::CyclicOrDisconnectedActivationHistory
        } else {
            DeltaMaintenanceRejection::SplitActivationHead { terminal_heads }
        });
    }
    roots.sort();
    if roots.len() != 1 {
        return Err(DeltaMaintenanceRejection::AmbiguousActivationRoot { roots });
    }
    let root_index = by_id[&roots[0]];
    let mut stack = vec![root_index];
    let mut visited = BTreeSet::new();
    while let Some(index) = stack.pop() {
        if !visited.insert(index) {
            return Err(DeltaMaintenanceRejection::CyclicOrDisconnectedActivationHistory);
        }
        stack.extend(children[index].iter().copied());
    }
    if visited.len() != events.len() {
        return Err(DeltaMaintenanceRejection::CyclicOrDisconnectedActivationHistory);
    }
    Ok(&events[by_id[&terminal_heads[0]]])
}

fn candidate_digest(candidates: &[String]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.delta-vacuum-candidates.v1\0");
    hasher.update(&(candidates.len() as u64).to_be_bytes());
    for candidate in candidates {
        let bytes = candidate.as_bytes();
        hasher.update(&(bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
    }
    *hasher.finalize().as_bytes()
}

fn is_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::Duration;

    use arrow_array::{Int64Array, RecordBatch};
    use arrow_schema::{DataType, Field, Schema};
    use deltalake::TableProperty;
    use deltalake::kernel::engine::arrow_conversion::TryIntoKernel as _;
    use deltalake::operations::create::CreateBuilder;
    use deltalake::protocol::SaveMode;
    use rusqlite::{Connection, params};
    use tempfile::TempDir;
    use url::Url;

    use super::*;
    use crate::fabric::command::{LeaseId, WriterGeneration};
    use crate::fabric::delta_exact::DeltaRetentionClaim;

    const HEAD: [u8; 32] = [0x31; 32];

    struct Fixture {
        _temporary: TempDir,
        root: Url,
        table: DeltaTable,
        schema: Arc<Schema>,
    }

    impl Fixture {
        async fn new(retention_seconds: Option<u64>) -> Self {
            let temporary = TempDir::new().expect("temporary maintenance fixture root");
            let table_path = temporary.path().join("table");
            fs::create_dir_all(&table_path).expect("create maintenance fixture directory");
            let root = Url::from_directory_path(&table_path).expect("fixture file URL");
            let schema = Arc::new(Schema::new(vec![Field::new(
                "value",
                DataType::Int64,
                false,
            )]));
            let kernel: deltalake::kernel::StructType = schema
                .as_ref()
                .try_into_kernel()
                .expect("Arrow fixture schema converts to Delta");
            let mut builder = CreateBuilder::new()
                .with_location(root.to_string())
                .with_table_name("guarded_delta_maintenance_fixture")
                .with_save_mode(SaveMode::ErrorIfExists)
                .with_columns(kernel.fields().cloned());
            if let Some(seconds) = retention_seconds {
                builder = builder.with_configuration_property(
                    TableProperty::DeletedFileRetentionDuration,
                    Some(format!("interval {seconds} seconds")),
                );
            }
            let table = builder.await.expect("create maintenance fixture");
            Self {
                _temporary: temporary,
                root,
                table,
                schema,
            }
        }

        fn batch(&self, values: Vec<i64>) -> RecordBatch {
            RecordBatch::try_new(
                Arc::clone(&self.schema),
                vec![Arc::new(Int64Array::from(values))],
            )
            .expect("maintenance fixture batch")
        }

        async fn write(&self, table: DeltaTable, values: Vec<i64>, mode: SaveMode) -> DeltaTable {
            table
                .write([self.batch(values)])
                .with_save_mode(mode)
                .await
                .expect("write maintenance fixture Delta version")
        }

        async fn reopen(&self, version: u64) -> DeltaTable {
            DeltaTableBuilder::from_url(self.root.clone())
                .expect("construct exact maintenance fixture loader")
                .with_version(version)
                .load()
                .await
                .expect("reopen exact maintenance fixture version")
        }
    }

    #[derive(Debug)]
    struct StaticSafetyPort {
        evidence: Mutex<DeltaMaintenanceSafetyEvidence>,
    }

    impl StaticSafetyPort {
        fn new(evidence: DeltaMaintenanceSafetyEvidence) -> Self {
            Self {
                evidence: Mutex::new(evidence),
            }
        }

        fn replace(&self, evidence: DeltaMaintenanceSafetyEvidence) {
            *self.evidence.lock().expect("static safety evidence lock") = evidence;
        }
    }

    #[async_trait]
    impl DeltaMaintenanceSafetyPort for StaticSafetyPort {
        async fn observe(
            &self,
            _target: &ExactDeltaPin,
        ) -> Result<DeltaMaintenanceSafetyEvidence, DeltaMaintenanceAuthorityError> {
            Ok(self
                .evidence
                .lock()
                .expect("static safety evidence lock")
                .clone())
        }
    }

    #[derive(Debug)]
    struct SqliteSafetyPort {
        path: PathBuf,
    }

    #[async_trait]
    impl DeltaMaintenanceSafetyPort for SqliteSafetyPort {
        async fn observe(
            &self,
            target: &ExactDeltaPin,
        ) -> Result<DeltaMaintenanceSafetyEvidence, DeltaMaintenanceAuthorityError> {
            let connection = Connection::open(&self.path)
                .map_err(|error| DeltaMaintenanceAuthorityError::new(error.to_string()))?;
            let row = connection
                .query_row(
                    "SELECT revision, lease_id, generation, head_id FROM safety_authority \
                     WHERE canonical_root = ?1 AND delta_version = ?2",
                    params![target.canonical_root().as_str(), target.version() as i64],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, Vec<u8>>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, Vec<u8>>(3)?,
                        ))
                    },
                )
                .map_err(|error| DeltaMaintenanceAuthorityError::new(error.to_string()))?;
            let revision = u64::try_from(row.0)
                .ok()
                .and_then(NonZeroU64::new)
                .ok_or_else(|| DeltaMaintenanceAuthorityError::new("invalid revision"))?;
            let lease_id: [u8; 16] = row
                .1
                .try_into()
                .map_err(|_| DeltaMaintenanceAuthorityError::new("invalid lease width"))?;
            let generation = u64::try_from(row.2)
                .ok()
                .and_then(WriterGeneration::new)
                .ok_or_else(|| DeltaMaintenanceAuthorityError::new("invalid generation"))?;
            let head_id: [u8; 32] = row
                .3
                .try_into()
                .map_err(|_| DeltaMaintenanceAuthorityError::new("invalid head width"))?;
            let event = DeltaMaintenanceActivationEvent::try_new(head_id, None, target.clone())
                .map_err(|error| DeltaMaintenanceAuthorityError::new(error.to_string()))?;
            Ok(DeltaMaintenanceSafetyEvidence::new(
                revision,
                DeltaMaintenanceEvidenceSource::ALL,
                Some(WriterFence {
                    lease_id: LeaseId::from_bytes(lease_id),
                    generation,
                }),
                vec![event],
                Some(empty_retention()),
                Vec::new(),
                Vec::new(),
            ))
        }
    }

    fn initialize_sqlite_authority(
        path: &Path,
        target: &ExactDeltaPin,
        fence: WriterFence,
        revision: NonZeroU64,
    ) {
        let connection = Connection::open(path).expect("open durable safety fixture");
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = FULL;
                 CREATE TABLE safety_authority (
                    canonical_root TEXT NOT NULL,
                    delta_version INTEGER NOT NULL CHECK(delta_version >= 0),
                    revision INTEGER NOT NULL CHECK(revision > 0),
                    lease_id BLOB NOT NULL CHECK(length(lease_id) = 16),
                    generation INTEGER NOT NULL CHECK(generation > 0),
                    head_id BLOB NOT NULL CHECK(length(head_id) = 32),
                    PRIMARY KEY(canonical_root, delta_version)
                 ) STRICT;",
            )
            .expect("create strict durable safety schema");
        connection
            .execute(
                "INSERT INTO safety_authority \
                 (canonical_root, delta_version, revision, lease_id, generation, head_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    target.canonical_root().as_str(),
                    target.version() as i64,
                    revision.get() as i64,
                    fence.lease_id.as_bytes().as_slice(),
                    fence.generation.get() as i64,
                    HEAD.as_slice(),
                ],
            )
            .expect("persist exact safety authority row");
    }

    fn fence(generation: u64) -> WriterFence {
        WriterFence {
            lease_id: LeaseId::from_bytes([0x21; 16]),
            generation: WriterGeneration::new(generation).expect("nonzero test generation"),
        }
    }

    fn empty_retention() -> DeltaRetentionClosure {
        DeltaRetentionClosure::try_new(100, DeltaRetentionAuthorityKind::ALL, std::iter::empty())
            .expect("complete observed-empty retention closure")
    }

    fn evidence(
        target: &ExactDeltaPin,
        writer_fence: WriterFence,
    ) -> DeltaMaintenanceSafetyEvidence {
        DeltaMaintenanceSafetyEvidence::new(
            NonZeroU64::new(7).expect("nonzero evidence revision"),
            DeltaMaintenanceEvidenceSource::ALL,
            Some(writer_fence),
            vec![
                DeltaMaintenanceActivationEvent::try_new(HEAD, None, target.clone())
                    .expect("valid activation head"),
            ],
            Some(empty_retention()),
            Vec::new(),
            Vec::new(),
        )
    }

    fn request(
        target: ExactDeltaPin,
        writer_fence: WriterFence,
        intent: GuardedDeltaMaintenanceIntent,
    ) -> GuardedDeltaMaintenanceRequest {
        GuardedDeltaMaintenanceRequest::try_new(
            target,
            HEAD,
            writer_fence,
            OperationId::from_bytes([0x41; 16]),
            intent,
        )
        .expect("valid guarded maintenance request")
    }

    #[tokio::test]
    async fn split_head_and_stale_fence_reject_before_native_checkpoint() {
        let fixture = Fixture::new(None).await;
        let pin = ExactDeltaPin::new(&fixture.root, 0).expect("version-zero pin");
        let current_fence = fence(4);
        let port = Arc::new(StaticSafetyPort::new(evidence(&pin, current_fence)));
        let controller = GuardedDeltaMaintenance::new(port.clone());
        let checkpoint_pointer = fixture
            .root
            .to_file_path()
            .expect("local fixture root")
            .join("_delta_log/_last_checkpoint");

        let stale = request(
            pin.clone(),
            fence(3),
            GuardedDeltaMaintenanceIntent::CreateCheckpoint,
        );
        let stale_outcome = controller
            .execute(&stale, fixture.reopen(0).await)
            .await
            .expect("stale-fence decision");
        assert!(matches!(
            stale_outcome,
            DeltaMaintenanceOutcome::Rejected(DeltaMaintenanceRejection::StaleWriterFence { .. })
        ));
        assert!(!checkpoint_pointer.exists());

        let split_events = vec![
            DeltaMaintenanceActivationEvent::try_new([1; 32], None, pin.clone())
                .expect("split root"),
            DeltaMaintenanceActivationEvent::try_new([2; 32], Some([1; 32]), pin.clone())
                .expect("split branch one"),
            DeltaMaintenanceActivationEvent::try_new([3; 32], Some([1; 32]), pin.clone())
                .expect("split branch two"),
        ];
        port.replace(DeltaMaintenanceSafetyEvidence::new(
            NonZeroU64::new(8).expect("nonzero evidence revision"),
            DeltaMaintenanceEvidenceSource::ALL,
            Some(current_fence),
            split_events,
            Some(empty_retention()),
            Vec::new(),
            Vec::new(),
        ));
        let split = request(
            pin,
            current_fence,
            GuardedDeltaMaintenanceIntent::CreateCheckpoint,
        );
        let split_outcome = controller
            .execute(&split, fixture.reopen(0).await)
            .await
            .expect("split-head decision");
        assert!(matches!(
            split_outcome,
            DeltaMaintenanceOutcome::Rejected(
                DeltaMaintenanceRejection::SplitActivationHead { .. }
            )
        ));
        assert!(!checkpoint_pointer.exists());
    }

    #[tokio::test]
    async fn uncertain_commit_and_missing_evidence_never_admit_absence() {
        let fixture = Fixture::new(None).await;
        let pin = ExactDeltaPin::new(&fixture.root, 0).expect("version-zero pin");
        let current_fence = fence(4);
        let uncertain = DeltaMaintenanceUncertainCommit::try_new(
            OperationId::from_bytes([0x51; 16]),
            pin.clone(),
        )
        .expect("valid uncertain operation");
        let mut blocked = evidence(&pin, current_fence);
        blocked.uncertain_commits.push(uncertain);
        let port = Arc::new(StaticSafetyPort::new(blocked));
        let controller = GuardedDeltaMaintenance::new(port.clone());
        let inspect = request(
            pin.clone(),
            current_fence,
            GuardedDeltaMaintenanceIntent::InspectRetention,
        );
        assert!(matches!(
            controller
                .execute(&inspect, fixture.reopen(0).await)
                .await
                .expect("uncertain-commit decision"),
            DeltaMaintenanceOutcome::Rejected(DeltaMaintenanceRejection::UncertainCommit { .. })
        ));

        port.replace(DeltaMaintenanceSafetyEvidence::new(
            NonZeroU64::new(9).expect("nonzero revision"),
            [DeltaMaintenanceEvidenceSource::ActivationEvents],
            None,
            vec![
                DeltaMaintenanceActivationEvent::try_new(HEAD, None, pin)
                    .expect("activation event"),
            ],
            None,
            Vec::new(),
            Vec::new(),
        ));
        assert!(matches!(
            controller
                .execute(&inspect, fixture.reopen(0).await)
                .await
                .expect("missing evidence decision"),
            DeltaMaintenanceOutcome::Rejected(
                DeltaMaintenanceRejection::MissingEvidenceSources { .. }
            )
        ));
    }

    #[tokio::test]
    async fn active_pins_cdf_and_proof_references_refuse_destructive_vacuum() {
        let fixture = Fixture::new(Some(0)).await;
        let pin = ExactDeltaPin::new(&fixture.root, 0).expect("version-zero pin");
        let current_fence = fence(4);
        let claims = vec![
            DeltaRetentionClaim::try_new(
                DeltaRetentionAuthorityKind::ActiveEpoch,
                [1; 32],
                DeltaRetainedResource::DeltaVersion(pin.clone()),
                None,
            )
            .expect("active epoch pin"),
            DeltaRetentionClaim::try_new(
                DeltaRetentionAuthorityKind::QueryLease,
                [2; 32],
                DeltaRetainedResource::DeltaVersion(pin.clone()),
                Some(200),
            )
            .expect("active query pin"),
            DeltaRetentionClaim::try_new(
                DeltaRetentionAuthorityKind::ResultLease,
                [3; 32],
                DeltaRetainedResource::QueryResult([4; 32]),
                Some(200),
            )
            .expect("active result pin"),
            DeltaRetentionClaim::try_new(
                DeltaRetentionAuthorityKind::CdfConsumerCheckpoint,
                [5; 32],
                DeltaRetainedResource::DeltaVersion(pin.clone()),
                None,
            )
            .expect("active CDF consumer pin"),
        ];
        let closure = DeltaRetentionClosure::try_new(100, DeltaRetentionAuthorityKind::ALL, claims)
            .expect("complete active retention closure");
        let proof = DeltaMaintenanceProofReference::try_new([6; 32], pin.clone())
            .expect("valid proof reference");
        let mut safety = evidence(&pin, current_fence);
        safety.retention_closure = Some(closure);
        safety.proof_references = vec![proof];
        let port = Arc::new(StaticSafetyPort::new(safety));
        let controller = GuardedDeltaMaintenance::new(port.clone());
        let receipt = DeltaVacuumDryRunReceipt {
            target: pin.clone(),
            evidence_revision: NonZeroU64::new(7).expect("nonzero revision"),
            retention_seconds: 0,
            keep_versions: Arc::from([0_u64]),
            candidates: Arc::from([]),
            candidate_digest: candidate_digest(&[]),
        };
        let execute = request(
            pin.clone(),
            current_fence,
            GuardedDeltaMaintenanceIntent::VacuumExecute(receipt.clone()),
        );
        let outcome = controller
            .execute(&execute, fixture.reopen(0).await)
            .await
            .expect("retention-blocked vacuum decision");
        let DeltaMaintenanceOutcome::Rejected(DeltaMaintenanceRejection::RetentionBlocked {
            authorities,
        }) = outcome
        else {
            panic!("expected retention rejection, got {outcome:?}");
        };
        assert_eq!(
            authorities,
            vec![
                DeltaRetentionAuthorityKind::ActiveEpoch,
                DeltaRetentionAuthorityKind::QueryLease,
                DeltaRetentionAuthorityKind::ResultLease,
                DeltaRetentionAuthorityKind::CdfConsumerCheckpoint,
            ]
        );

        let mut proof_only = evidence(&pin, current_fence);
        proof_only.proof_references = vec![
            DeltaMaintenanceProofReference::try_new([6; 32], pin.clone())
                .expect("valid proof reference"),
        ];
        port.replace(proof_only);
        assert!(matches!(
            controller
                .execute(&execute, fixture.reopen(0).await)
                .await
                .expect("proof-blocked vacuum decision"),
            DeltaMaintenanceOutcome::Rejected(
                DeltaMaintenanceRejection::ProofReferenceBlocked { .. }
            )
        ));

        port.replace(evidence(&pin, current_fence));
        assert_eq!(
            controller
                .execute(&execute, fixture.reopen(0).await)
                .await
                .expect("native vacuum availability decision"),
            DeltaMaintenanceOutcome::Unavailable(
                DeltaMaintenanceUnavailable::AtomicVacuumApprovalBinding
            )
        );
    }

    #[tokio::test]
    async fn cache_loss_and_process_reopen_reconstruct_durable_authority() {
        let fixture = Fixture::new(None).await;
        let pin = ExactDeltaPin::new(&fixture.root, 0).expect("version-zero pin");
        let current_fence = fence(4);
        let authority_path = fixture
            ._temporary
            .path()
            .join("maintenance-authority.sqlite3");
        let revision = NonZeroU64::new(12).expect("nonzero revision");
        initialize_sqlite_authority(&authority_path, &pin, current_fence, revision);
        let inspect = request(
            pin.clone(),
            current_fence,
            GuardedDeltaMaintenanceIntent::InspectRetention,
        );
        let controller = GuardedDeltaMaintenance::new(Arc::new(SqliteSafetyPort {
            path: authority_path.clone(),
        }));
        let first = controller
            .execute(&inspect, fixture.reopen(0).await)
            .await
            .expect("first durable safety decision");
        assert_eq!(controller.cached_revision(), Some(revision));
        controller.clear_derived_cache();
        assert_eq!(controller.cached_revision(), None);
        let after_cache_loss = controller
            .execute(&inspect, fixture.reopen(0).await)
            .await
            .expect("authority reload after cache loss");
        assert_eq!(first, after_cache_loss);
        drop(controller);

        let reopened_controller = GuardedDeltaMaintenance::new(Arc::new(SqliteSafetyPort {
            path: authority_path,
        }));
        let after_process_reopen = reopened_controller
            .execute(&inspect, fixture.reopen(0).await)
            .await
            .expect("authority reconstruction after process reopen");
        assert_eq!(first, after_process_reopen);
        assert_eq!(reopened_controller.cached_revision(), Some(revision));
    }

    #[tokio::test]
    async fn native_checkpoint_preserves_exact_version_across_process_reopen() {
        let fixture = Fixture::new(None).await;
        let pin = ExactDeltaPin::new(&fixture.root, 0).expect("version-zero pin");
        let current_fence = fence(4);
        let controller = GuardedDeltaMaintenance::new(Arc::new(StaticSafetyPort::new(evidence(
            &pin,
            current_fence,
        ))));
        let checkpoint = request(
            pin.clone(),
            current_fence,
            GuardedDeltaMaintenanceIntent::CreateCheckpoint,
        );
        assert_eq!(
            controller
                .execute(&checkpoint, fixture.reopen(0).await)
                .await
                .expect("native checkpoint execution"),
            DeltaMaintenanceOutcome::CheckpointCreated {
                target: pin.clone(),
                evidence_revision: NonZeroU64::new(7).expect("nonzero revision"),
            }
        );
        assert!(
            fixture
                .root
                .to_file_path()
                .expect("local fixture root")
                .join("_delta_log/_last_checkpoint")
                .exists()
        );
        let reopened = fixture.reopen(0).await;
        assert_eq!(reopened.version(), Some(0));
        ValidatedDeltaSnapshot::try_from_loaded_table(reopened, &pin)
            .expect("checkpoint preserves exact semantic version");
    }

    #[tokio::test]
    async fn native_optimize_is_unavailable_before_mutation_when_commit_contract_is_dropped() {
        let fixture = Fixture::new(None).await;
        let version_one = fixture
            .write(fixture.table.clone(), vec![1, 2], SaveMode::Append)
            .await;
        let version_two = fixture
            .write(version_one, vec![3, 4], SaveMode::Append)
            .await;
        assert_eq!(version_two.version(), Some(2));
        let pin = ExactDeltaPin::new(&fixture.root, 2).expect("version-two pin");
        let current_fence = fence(4);
        let controller = GuardedDeltaMaintenance::new(Arc::new(StaticSafetyPort::new(evidence(
            &pin,
            current_fence,
        ))));
        let specification = DeltaOptimizeCompactSpec::new(
            NonZeroU64::new(1_000_000).expect("nonzero target size"),
            NonZeroUsize::new(2).expect("nonzero concurrency"),
            DeltaMaintenanceApplicationTransaction::try_new("codefabric/test/guarded-optimize", 19)
                .expect("valid optimize transaction"),
        );
        let optimize = request(
            pin.clone(),
            current_fence,
            GuardedDeltaMaintenanceIntent::OptimizeCompact(specification),
        );
        let before_files = parquet_files(&fixture.root);
        let outcome = controller
            .execute(&optimize, version_two)
            .await
            .expect("native optimize capability decision");
        assert_eq!(
            outcome,
            DeltaMaintenanceOutcome::Unavailable(
                DeltaMaintenanceUnavailable::OptimizeCommitIdentityAndRetryControl
            )
        );
        assert_eq!(parquet_files(&fixture.root), before_files);
        assert_eq!(fixture.reopen(2).await.version(), Some(2));
        assert!(
            !fixture
                .root
                .to_file_path()
                .expect("local fixture root")
                .join("_delta_log/00000000000000000003.json")
                .exists(),
            "unavailable optimize must not attempt a native commit"
        );
    }

    #[tokio::test]
    async fn native_vacuum_dry_run_honors_exact_keep_versions_without_deletion() {
        let fixture = Fixture::new(Some(0)).await;
        let version_one = fixture
            .write(fixture.table.clone(), vec![1], SaveMode::Append)
            .await;
        let version_two = fixture
            .write(version_one, vec![2], SaveMode::Overwrite)
            .await;
        tokio::time::sleep(Duration::from_millis(5)).await;
        let target = ExactDeltaPin::new(&fixture.root, 2).expect("version-two pin");
        let protected = ExactDeltaPin::new(&fixture.root, 1).expect("protected version-one pin");
        let closure = DeltaRetentionClosure::try_new(
            100,
            DeltaRetentionAuthorityKind::ALL,
            [DeltaRetentionClaim::try_new(
                DeltaRetentionAuthorityKind::ActiveEpoch,
                [0x61; 32],
                DeltaRetainedResource::DeltaVersion(protected),
                None,
            )
            .expect("protected historical version")],
        )
        .expect("complete retention closure");
        let current_fence = fence(4);
        let mut safety = evidence(&target, current_fence);
        safety.retention_closure = Some(closure);
        let controller = GuardedDeltaMaintenance::new(Arc::new(StaticSafetyPort::new(safety)));
        let dry_run = request(
            target.clone(),
            current_fence,
            GuardedDeltaMaintenanceIntent::VacuumDryRun {
                expected_retention_seconds: 0,
            },
        );
        let before_files = parquet_files(&fixture.root);
        let outcome = controller
            .execute(&dry_run, version_two)
            .await
            .expect("native vacuum dry run");
        let DeltaMaintenanceOutcome::VacuumDryRun(receipt) = outcome else {
            panic!("expected vacuum dry-run receipt, got {outcome:?}");
        };
        assert_eq!(receipt.target(), &target);
        assert_eq!(receipt.keep_versions(), &[1]);
        assert_eq!(
            receipt.candidate_digest(),
            &candidate_digest(receipt.candidates())
        );
        assert_eq!(parquet_files(&fixture.root), before_files);
        assert_eq!(fixture.reopen(2).await.version(), Some(2));
    }

    fn parquet_files(root: &Url) -> Vec<PathBuf> {
        let mut files = fs::read_dir(root.to_file_path().expect("local fixture root"))
            .expect("list fixture root")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "parquet")
            })
            .collect::<Vec<_>>();
        files.sort();
        files
    }
}
