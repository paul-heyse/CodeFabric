//! Durable activation-command inputs and temporal reconciliation state.
//!
//! SQLite owns command-scoped immutable input rows and reconciliation progress only. It never
//! owns semantic current. Candidate and proof authority remain the exact sealed programmatic
//! epoch (whose relation versions are Delta pins) and its computed Arrow proof relations.

use std::fs::{self, File};
use std::num::TryFromIntError;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use async_trait::async_trait;
use rusqlite::{Connection, OpenFlags, OptionalExtension as _, TransactionBehavior};
use rustix::fs::{Mode, OFlags, open, openat};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::oneshot;
use url::Url;

use super::activation::{
    ActivationControlRelationPin, ActivationError, ActivationEventId, CompatibilityClassRef,
    FabricEpochPins, SealedActivationControlBinding, TableVersionSet,
};
use super::activation_transaction::{
    ActivationAdmissionPosture, ActivationAppendUnknownReason, ActivationReadbackViolation,
    ActivationReconciliationReason, ActivationReconciliationTicket, ActivationTransactionStage,
    CandidateProofRequest, DurableSelectionKnowledge,
};
use super::admission::AdmissionError;
use super::command::{
    DiagnosticRef, ExecutionOwner, FabricCommand, OperationSelectionRef, ReconciliationEvidenceRef,
    RetentionPolicyRef, TransactionRef, UnknownCommit, UnknownCommitReason, WorkspaceId,
    WriterFence,
};
use super::command_actor::CommandPortError;
use super::delta_exact::ExactDeltaPin;
use super::programmatic_activation_command_ports::{
    ActivationCandidateProofEvidence, ActivationCandidateProofObservation,
    ActivationCandidateProofRelationsPort, ActivationCommandRequestKey,
    ActivationCommandRequestMaterial, ActivationCommandStateStore,
    ActivationNotSelectedClassification, ActivationNotSelectedClassificationQuery,
    ActivationReconciliationRead, ActivationReconciliationRecord, ActivationReconciliationWrite,
};
use super::programmatic_epoch::{ProgrammaticFabricEpoch, ProgrammaticFabricEpochBuilder};
use super::proof::{ProofRelations, ProofTerminalStatus};

/// Exact schema version for the dedicated temporal activation-command store.
pub const ACTIVATION_COMMAND_STATE_SCHEMA_VERSION: u32 = 2;

const APPLICATION_ID: u32 = 0x4346_4153; // `CFAS`
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_ROW_BYTES: usize = 128 * 1024;
const REQUEST_TABLE: &str = "activation_command_request";
const RECONCILIATION_TABLE: &str = "activation_command_reconciliation";
const SCHEMA_V2: &str = "CREATE TABLE activation_command_request (
    operation_id BLOB NOT NULL PRIMARY KEY
        CHECK (typeof(operation_id) = 'blob' AND length(operation_id) = 16),
    workspace_id BLOB NOT NULL
        CHECK (typeof(workspace_id) = 'blob' AND length(workspace_id) = 16),
    request_jcs BLOB NOT NULL
        CHECK (typeof(request_jcs) = 'blob' AND length(request_jcs) BETWEEN 2 AND 131072)
) WITHOUT ROWID, STRICT;
CREATE TABLE activation_command_reconciliation (
    operation_id BLOB NOT NULL PRIMARY KEY
        CHECK (typeof(operation_id) = 'blob' AND length(operation_id) = 16),
    workspace_id BLOB NOT NULL
        CHECK (typeof(workspace_id) = 'blob' AND length(workspace_id) = 16),
    attempt INTEGER NOT NULL
        CHECK (typeof(attempt) = 'integer' AND attempt BETWEEN 1 AND 4294967295),
    transaction_ref BLOB NOT NULL
        CHECK (typeof(transaction_ref) = 'blob' AND length(transaction_ref) = 32),
    reconciliation_jcs BLOB NOT NULL
        CHECK (typeof(reconciliation_jcs) = 'blob'
               AND length(reconciliation_jcs) BETWEEN 2 AND 131072)
) WITHOUT ROWID, STRICT;
PRAGMA application_id = 1128677715;
PRAGMA user_version = 2;";

/// Failure while binding one sealed candidate and computed proof relation census.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ExactProgrammaticActivationInputError {
    #[error("candidate epoch identity differs from activation pins")]
    CandidateEpochMismatch,
    #[error("candidate exact Delta version-set reference differs from activation pins")]
    CandidateTableVersionMismatch,
    #[error("computed proof relations differ from activation candidate pins")]
    ProofCandidateMismatch,
    #[error("passing proof relations require the exact nonzero proof receipt")]
    PassingProofReceiptMismatch,
    #[error("non-passing proof relations require one explicit diagnostic and no receipt")]
    NonPassingProofPosture,
    #[error("integrity diagnostic uses the all-zero sentinel")]
    ZeroIntegrityDiagnostic,
}

/// One immutable production authority over computed Arrow proof rows for exact candidate pins.
///
/// This is deliberately not a mutable registry. Candidate epoch reconstruction is a separate
/// exact-Delta port so a process restart never depends on retaining an `Arc` from the prior
/// process.
pub struct ExactProgrammaticActivationProofAuthority {
    workspace_id: WorkspaceId,
    pins: FabricEpochPins,
    relations: Arc<ProofRelations>,
    proof_receipt: Option<super::command::ProofReceiptRef>,
    proof_diagnostic: Option<DiagnosticRef>,
    integrity_diagnostic: DiagnosticRef,
}

impl std::fmt::Debug for ExactProgrammaticActivationProofAuthority {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExactProgrammaticActivationProofAuthority")
            .field("workspace_id", &self.workspace_id)
            .field("candidate_epoch", &self.pins.epoch)
            .field("terminal", &self.relations.terminal())
            .finish_non_exhaustive()
    }
}

impl ExactProgrammaticActivationProofAuthority {
    /// Bind the exact candidate pins, computed proof rows, and terminal projection.
    ///
    /// # Errors
    ///
    /// Rejects any candidate/Delta/proof pin drift or an invalid receipt/diagnostic posture.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        workspace_id: WorkspaceId,
        pins: FabricEpochPins,
        relations: Arc<ProofRelations>,
        proof_receipt: Option<super::command::ProofReceiptRef>,
        proof_diagnostic: Option<DiagnosticRef>,
        integrity_diagnostic: DiagnosticRef,
    ) -> Result<Self, ExactProgrammaticActivationInputError> {
        if !proof_pins_match(&relations, pins) {
            return Err(ExactProgrammaticActivationInputError::ProofCandidateMismatch);
        }
        match relations.terminal() {
            ProofTerminalStatus::Pass
                if proof_receipt == Some(pins.proof_receipt) && proof_diagnostic.is_none() => {}
            ProofTerminalStatus::Pass => {
                return Err(ExactProgrammaticActivationInputError::PassingProofReceiptMismatch);
            }
            ProofTerminalStatus::Fail | ProofTerminalStatus::Unknown
                if proof_receipt.is_none() && proof_diagnostic.is_some() => {}
            ProofTerminalStatus::Fail | ProofTerminalStatus::Unknown => {
                return Err(ExactProgrammaticActivationInputError::NonPassingProofPosture);
            }
        }
        if integrity_diagnostic
            .as_bytes()
            .iter()
            .all(|byte| *byte == 0)
        {
            return Err(ExactProgrammaticActivationInputError::ZeroIntegrityDiagnostic);
        }
        Ok(Self {
            workspace_id,
            pins,
            relations,
            proof_receipt,
            proof_diagnostic,
            integrity_diagnostic,
        })
    }

    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    #[must_use]
    pub const fn pins(&self) -> FabricEpochPins {
        self.pins
    }
}

#[async_trait]
impl ActivationCandidateProofRelationsPort for ExactProgrammaticActivationProofAuthority {
    async fn observe_candidate(
        &self,
        request: CandidateProofRequest,
    ) -> ActivationCandidateProofObservation {
        if request.workspace_id != self.workspace_id || request.pins != self.pins {
            return ActivationCandidateProofObservation::Unavailable {
                request,
                diagnostic: self.integrity_diagnostic,
            };
        }
        match ActivationCandidateProofEvidence::try_new(
            request,
            Arc::clone(&self.relations),
            self.proof_receipt,
            self.proof_diagnostic,
        ) {
            Ok(evidence) => ActivationCandidateProofObservation::Evaluated(evidence),
            Err(_) => ActivationCandidateProofObservation::Unavailable {
                request,
                diagnostic: self.integrity_diagnostic,
            },
        }
    }
}

/// Exact durable inputs needed to reconstruct a forward activation candidate after restart.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivationCommandCandidateRebuildRequest {
    pub workspace_id: WorkspaceId,
    pub pins: FabricEpochPins,
    pub table_versions: Arc<TableVersionSet>,
}

/// Candidate reconstruction port used by the SQLite request reader.
#[async_trait]
pub trait ActivationCommandCandidateRebuilderPort: Send + Sync {
    async fn rebuild_candidate(
        &self,
        request: ActivationCommandCandidateRebuildRequest,
    ) -> Result<Arc<ProgrammaticFabricEpoch>, CommandPortError>;
}

/// Exact-Delta candidate rebuilder over the release's explicit programmatic builder recipe.
pub struct ExactDeltaActivationCommandCandidateRebuilder<F> {
    workspace_id: WorkspaceId,
    builder: F,
}

impl<F> ExactDeltaActivationCommandCandidateRebuilder<F> {
    #[must_use]
    pub const fn new(workspace_id: WorkspaceId, builder: F) -> Self {
        Self {
            workspace_id,
            builder,
        }
    }
}

#[async_trait]
impl<F, E> ActivationCommandCandidateRebuilderPort
    for ExactDeltaActivationCommandCandidateRebuilder<F>
where
    F: Fn(super::command::EpochId) -> Result<ProgrammaticFabricEpochBuilder, E> + Send + Sync,
    E: std::fmt::Display + Send,
{
    async fn rebuild_candidate(
        &self,
        request: ActivationCommandCandidateRebuildRequest,
    ) -> Result<Arc<ProgrammaticFabricEpoch>, CommandPortError> {
        if request.workspace_id != self.workspace_id
            || request.table_versions.reference() != request.pins.table_versions
        {
            return Err(CommandPortError::CorruptRecord);
        }
        let builder =
            (self.builder)(request.pins.epoch).map_err(|_| CommandPortError::ContextUnavailable)?;
        let candidate = builder
            .reopen(Arc::clone(&request.table_versions))
            .await
            .map_err(|_| CommandPortError::ContextUnavailable)?;
        if candidate.identity() != &request.pins.epoch
            || candidate.table_version_set_ref() != request.pins.table_versions
        {
            return Err(CommandPortError::CorruptRecord);
        }
        Ok(Arc::new(candidate))
    }
}

fn proof_pins_match(relations: &ProofRelations, activation: FabricEpochPins) -> bool {
    let proof = relations.candidate_pins();
    proof.epoch == activation.epoch
        && proof.input_release == activation.input_release
        && proof.program_release == activation.program_release
        && proof.application_release == activation.application_release
        && proof.source_authority == activation.source_authority
        && proof.source_generation == activation.source_generation
        && proof.provider_release == activation.provider_release
        && proof.provider_set == activation.provider_set
        && proof.table_versions == activation.table_versions
        && proof.overlay_segments == activation.overlay_segments
        && proof.policy_set == activation.policy_set
        && proof.resource_envelope == activation.resource_envelope
}

/// Stable policy for deriving command-scoped temporal diagnostic/evidence identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationReconciliationIdentityPolicy {
    reason: UnknownCommitReason,
    diagnostic_namespace: [u8; 32],
    evidence_namespace: [u8; 32],
}

impl ActivationReconciliationIdentityPolicy {
    /// Construct an explicit release-owned identity policy.
    ///
    /// # Errors
    ///
    /// Rejects either all-zero namespace so a missing configuration cannot silently become a
    /// durable diagnostic identity.
    pub fn try_new(
        reason: UnknownCommitReason,
        diagnostic_namespace: [u8; 32],
        evidence_namespace: [u8; 32],
    ) -> Result<Self, ProgrammaticActivationStateOpenError> {
        if diagnostic_namespace.iter().all(|byte| *byte == 0)
            || evidence_namespace.iter().all(|byte| *byte == 0)
        {
            return Err(ProgrammaticActivationStateOpenError::ZeroIdentityNamespace);
        }
        Ok(Self {
            reason,
            diagnostic_namespace,
            evidence_namespace,
        })
    }

    fn identities(
        self,
        workspace_id: WorkspaceId,
        operation_id: super::command::OperationId,
        transaction: TransactionRef,
        ticket_jcs: &[u8],
    ) -> (UnknownCommit, ReconciliationEvidenceRef) {
        let mut diagnostic = blake3::Hasher::new();
        diagnostic.update(b"codefabric.activation-reconciliation-diagnostic.v1\0");
        diagnostic.update(&self.diagnostic_namespace);
        diagnostic.update(workspace_id.as_bytes());
        diagnostic.update(operation_id.as_bytes());
        diagnostic.update(transaction.as_bytes());
        let unknown = UnknownCommit {
            reason: self.reason,
            diagnostic: DiagnosticRef::from_bytes(*diagnostic.finalize().as_bytes()),
        };

        let mut evidence = blake3::Hasher::new();
        evidence.update(b"codefabric.activation-reconciliation-evidence.v1\0");
        evidence.update(&self.evidence_namespace);
        evidence.update(workspace_id.as_bytes());
        evidence.update(operation_id.as_bytes());
        evidence.update(transaction.as_bytes());
        evidence.update(&(ticket_jcs.len() as u64).to_be_bytes());
        evidence.update(ticket_jcs);
        (
            unknown,
            ReconciliationEvidenceRef::from_bytes(*evidence.finalize().as_bytes()),
        )
    }
}

/// Failure while opening and validating the dedicated activation-command state database.
#[derive(Debug, Error)]
pub enum ProgrammaticActivationStateOpenError {
    #[error("activation-state parent is not a private owned directory: {0}")]
    UnsafeParent(PathBuf),
    #[error("activation-state database is not a private owned regular file: {0}")]
    UnsafeDatabase(PathBuf),
    #[error("activation-state database path has no file name: {0}")]
    InvalidPath(PathBuf),
    #[error("activation-state I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("unsupported activation-state schema version {observed}; supported is {supported}")]
    UnsupportedSchema { observed: u32, supported: u32 },
    #[error("activation-state database schema is not exact: {0}")]
    UnexpectedSchema(String),
    #[error("failed to start the activation-state worker: {0}")]
    Worker(std::io::Error),
    #[error("activation reconciliation identity namespaces must be nonzero")]
    ZeroIdentityNamespace,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct StoredFabricEpochPins {
    epoch: [u8; 16],
    input_release: [u8; 32],
    program_release: [u8; 32],
    application_release: [u8; 32],
    source_authority: [u8; 32],
    source_generation: u64,
    provider_release: [u8; 32],
    provider_set: [u8; 32],
    table_versions: [u8; 32],
    overlay_segments: [u8; 32],
    policy_set: [u8; 32],
    resource_envelope: [u8; 32],
    proof_receipt: [u8; 32],
}

impl From<FabricEpochPins> for StoredFabricEpochPins {
    fn from(value: FabricEpochPins) -> Self {
        Self {
            epoch: *value.epoch.as_bytes(),
            input_release: *value.input_release.as_bytes(),
            program_release: *value.program_release.as_bytes(),
            application_release: *value.application_release.as_bytes(),
            source_authority: *value.source_authority.as_bytes(),
            source_generation: value.source_generation.get(),
            provider_release: *value.provider_release.as_bytes(),
            provider_set: *value.provider_set.as_bytes(),
            table_versions: *value.table_versions.as_bytes(),
            overlay_segments: *value.overlay_segments.as_bytes(),
            policy_set: *value.policy_set.as_bytes(),
            resource_envelope: *value.resource_envelope.as_bytes(),
            proof_receipt: *value.proof_receipt.as_bytes(),
        }
    }
}

impl StoredFabricEpochPins {
    fn decode(&self) -> FabricEpochPins {
        FabricEpochPins {
            epoch: super::command::EpochId::from_bytes(self.epoch),
            input_release: super::command::InputReleaseRef::from_bytes(self.input_release),
            program_release: super::command::ProgramReleaseRef::from_bytes(self.program_release),
            application_release: super::command::ApplicationReleaseRef::from_bytes(
                self.application_release,
            ),
            source_authority: super::command::SourceAuthorityRef::from_bytes(self.source_authority),
            source_generation: super::command::SourceGeneration::new(self.source_generation),
            provider_release: super::command::ProviderReleaseRef::from_bytes(self.provider_release),
            provider_set: super::command::ProviderSetRef::from_bytes(self.provider_set),
            table_versions: super::activation::TableVersionSetRef::from_bytes(self.table_versions),
            overlay_segments: super::activation::OverlaySegmentSetRef::from_bytes(
                self.overlay_segments,
            ),
            policy_set: super::activation::PolicySetRef::from_bytes(self.policy_set),
            resource_envelope: super::command::ResourceEnvelopeRef::from_bytes(
                self.resource_envelope,
            ),
            proof_receipt: super::command::ProofReceiptRef::from_bytes(self.proof_receipt),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct StoredActivationRequest {
    command: FabricCommand,
    attempt: Option<u32>,
    execution_owner: Option<ExecutionOwner>,
    pins: StoredFabricEpochPins,
    table_version_vector: Option<Vec<StoredDeltaComponent>>,
    event_id: [u8; 32],
    compatibility: [u8; 32],
    retention: [u8; 32],
    operation_selection: [u8; 32],
    transaction: [u8; 32],
    control_root: String,
    control_version: u64,
    control_binding_fingerprint: [u8; 32],
    control_relation_fingerprint: [u8; 32],
}

impl StoredActivationRequest {
    fn from_material(material: &ActivationCommandRequestMaterial) -> Self {
        let mut stored = Self::new(
            material.key().command(),
            None,
            None,
            material.pins(),
            material.event_id(),
            material.compatibility(),
            material.retention(),
            material.operation_selection(),
            material.transaction(),
            material.control_relation(),
        );
        stored.table_version_vector = Some(
            material
                .candidate()
                .table_version_set()
                .components()
                .map(|(relation_id, pin)| StoredDeltaComponent {
                    relation_id: relation_id.to_owned(),
                    canonical_root: pin.canonical_root().to_string(),
                    version: pin.version(),
                })
                .collect(),
        );
        stored
    }

    fn from_reconciliation(write: &ActivationReconciliationWrite) -> Self {
        Self::new(
            write.command(),
            Some(write.attempt()),
            Some(write.execution_owner()),
            write.pins(),
            write.event_id(),
            write.compatibility(),
            write.retention(),
            write.operation_selection(),
            write.transaction(),
            write.control_relation(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        command: FabricCommand,
        attempt: Option<u32>,
        execution_owner: Option<ExecutionOwner>,
        pins: FabricEpochPins,
        event_id: ActivationEventId,
        compatibility: CompatibilityClassRef,
        retention: RetentionPolicyRef,
        operation_selection: OperationSelectionRef,
        transaction: TransactionRef,
        control: &ActivationControlRelationPin,
    ) -> Self {
        Self {
            command,
            attempt,
            execution_owner,
            pins: pins.into(),
            table_version_vector: None,
            event_id: *event_id.as_bytes(),
            compatibility: *compatibility.as_bytes(),
            retention: *retention.as_bytes(),
            operation_selection: *operation_selection.as_bytes(),
            transaction: *transaction.as_bytes(),
            control_root: control.table().canonical_root().to_string(),
            control_version: control.table().version(),
            control_binding_fingerprint: *control.binding().fingerprint(),
            control_relation_fingerprint: *control.fingerprint(),
        }
    }

    fn control_relation(
        &self,
        binding: &SealedActivationControlBinding,
    ) -> Result<ActivationControlRelationPin, CommandPortError> {
        if binding.fingerprint() != &self.control_binding_fingerprint {
            return Err(CommandPortError::CorruptRecord);
        }
        let root = Url::parse(&self.control_root).map_err(|_| CommandPortError::CorruptRecord)?;
        let table = ExactDeltaPin::new(&root, self.control_version)
            .map_err(|_| CommandPortError::CorruptRecord)?;
        let relation = ActivationControlRelationPin::new(table, binding.clone());
        if relation.fingerprint() != &self.control_relation_fingerprint {
            return Err(CommandPortError::CorruptRecord);
        }
        Ok(relation)
    }

    async fn material(
        &self,
        workspace_id: WorkspaceId,
        rebuilder: &dyn ActivationCommandCandidateRebuilderPort,
        binding: &SealedActivationControlBinding,
    ) -> Result<ActivationCommandRequestMaterial, CommandPortError> {
        if self.attempt.is_some() || self.execution_owner.is_some() {
            return Err(CommandPortError::CorruptRecord);
        }
        let pins = self.pins.decode();
        if self.command.ownership.workspace_id != workspace_id {
            return Err(CommandPortError::CorruptRecord);
        }
        let table_versions = self
            .table_version_vector
            .as_ref()
            .ok_or(CommandPortError::CorruptRecord)?;
        let table_versions = Arc::new(decode_table_versions(table_versions)?);
        if table_versions.reference() != pins.table_versions {
            return Err(CommandPortError::CorruptRecord);
        }
        let candidate = rebuilder
            .rebuild_candidate(ActivationCommandCandidateRebuildRequest {
                workspace_id,
                pins,
                table_versions,
            })
            .await?;
        Ok(ActivationCommandRequestMaterial::new(
            ActivationCommandRequestKey::new(self.command),
            candidate,
            pins,
            ActivationEventId::from_bytes(self.event_id),
            CompatibilityClassRef::from_bytes(self.compatibility),
            RetentionPolicyRef::from_bytes(self.retention),
            OperationSelectionRef::from_bytes(self.operation_selection),
            TransactionRef::from_bytes(self.transaction),
            self.control_relation(binding)?,
        ))
    }

    fn reconciliation_write(
        &self,
        ticket: ActivationReconciliationTicket,
        binding: &SealedActivationControlBinding,
    ) -> Result<ActivationReconciliationWrite, CommandPortError> {
        let attempt = self.attempt.ok_or(CommandPortError::CorruptRecord)?;
        if self.table_version_vector.is_some() {
            return Err(CommandPortError::CorruptRecord);
        }
        let execution_owner = self
            .execution_owner
            .ok_or(CommandPortError::CorruptRecord)?;
        let write = ActivationReconciliationWrite::from_persisted_primitives(
            self.command,
            attempt,
            execution_owner,
            self.pins.decode(),
            ActivationEventId::from_bytes(self.event_id),
            CompatibilityClassRef::from_bytes(self.compatibility),
            RetentionPolicyRef::from_bytes(self.retention),
            OperationSelectionRef::from_bytes(self.operation_selection),
            TransactionRef::from_bytes(self.transaction),
            self.control_relation(binding)?,
            ticket,
        );
        Ok(write)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct StoredDeltaComponent {
    relation_id: String,
    canonical_root: String,
    version: u64,
}

fn decode_table_versions(
    components: &[StoredDeltaComponent],
) -> Result<TableVersionSet, CommandPortError> {
    TableVersionSet::try_new(
        components
            .iter()
            .map(|component| {
                let root = Url::parse(&component.canonical_root)
                    .map_err(|_| CommandPortError::CorruptRecord)?;
                let pin = ExactDeltaPin::new(&root, component.version)
                    .map_err(|_| CommandPortError::CorruptRecord)?;
                Ok((Arc::<str>::from(component.relation_id.as_str()), pin))
            })
            .collect::<Result<Vec<_>, CommandPortError>>()?,
    )
    .map_err(|_| CommandPortError::CorruptRecord)
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum StoredActivationTransactionStage {
    CandidateProof,
    AdmissionClosure,
    AuthorityRevalidation,
    DurableAppendReadback,
    EpochSwap,
    CacheReconciliation,
    AdmissionReopen,
    Acknowledgement,
}

impl From<ActivationTransactionStage> for StoredActivationTransactionStage {
    fn from(value: ActivationTransactionStage) -> Self {
        match value {
            ActivationTransactionStage::CandidateProof => Self::CandidateProof,
            ActivationTransactionStage::AdmissionClosure => Self::AdmissionClosure,
            ActivationTransactionStage::AuthorityRevalidation => Self::AuthorityRevalidation,
            ActivationTransactionStage::DurableAppendReadback => Self::DurableAppendReadback,
            ActivationTransactionStage::EpochSwap => Self::EpochSwap,
            ActivationTransactionStage::CacheReconciliation => Self::CacheReconciliation,
            ActivationTransactionStage::AdmissionReopen => Self::AdmissionReopen,
            ActivationTransactionStage::Acknowledgement => Self::Acknowledgement,
        }
    }
}

impl From<StoredActivationTransactionStage> for ActivationTransactionStage {
    fn from(value: StoredActivationTransactionStage) -> Self {
        match value {
            StoredActivationTransactionStage::CandidateProof => Self::CandidateProof,
            StoredActivationTransactionStage::AdmissionClosure => Self::AdmissionClosure,
            StoredActivationTransactionStage::AuthorityRevalidation => Self::AuthorityRevalidation,
            StoredActivationTransactionStage::DurableAppendReadback => Self::DurableAppendReadback,
            StoredActivationTransactionStage::EpochSwap => Self::EpochSwap,
            StoredActivationTransactionStage::CacheReconciliation => Self::CacheReconciliation,
            StoredActivationTransactionStage::AdmissionReopen => Self::AdmissionReopen,
            StoredActivationTransactionStage::Acknowledgement => Self::Acknowledgement,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum StoredActivationAppendUnknownReason {
    CommitOutcomeUnknown,
    ReadbackUnavailable,
    CancelledDuringCommit,
}

impl From<ActivationAppendUnknownReason> for StoredActivationAppendUnknownReason {
    fn from(value: ActivationAppendUnknownReason) -> Self {
        match value {
            ActivationAppendUnknownReason::CommitOutcomeUnknown => Self::CommitOutcomeUnknown,
            ActivationAppendUnknownReason::ReadbackUnavailable => Self::ReadbackUnavailable,
            ActivationAppendUnknownReason::CancelledDuringCommit => Self::CancelledDuringCommit,
        }
    }
}

impl From<StoredActivationAppendUnknownReason> for ActivationAppendUnknownReason {
    fn from(value: StoredActivationAppendUnknownReason) -> Self {
        match value {
            StoredActivationAppendUnknownReason::CommitOutcomeUnknown => Self::CommitOutcomeUnknown,
            StoredActivationAppendUnknownReason::ReadbackUnavailable => Self::ReadbackUnavailable,
            StoredActivationAppendUnknownReason::CancelledDuringCommit => {
                Self::CancelledDuringCommit
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
enum StoredActivationError {
    CommandDoesNotSelectEpoch,
    ProofReceiptMismatch,
    SelectedEpochMismatch {
        command: [u8; 16],
        event: [u8; 16],
    },
    CommandPinMismatch,
    WorkspaceMismatch {
        expected: [u8; 16],
        actual: [u8; 16],
    },
    DuplicateEvent {
        event_id: [u8; 32],
    },
    DuplicateOperation {
        operation_id: [u8; 16],
    },
    DuplicateOperationSelection {
        selection: [u8; 32],
    },
    DuplicateTransaction {
        transaction: [u8; 32],
    },
    RootCount {
        count: u64,
    },
    InvalidRoot {
        event_id: [u8; 32],
    },
    MissingPredecessor {
        event: [u8; 32],
        predecessor: [u8; 32],
    },
    Fork {
        event_id: [u8; 32],
    },
    PredecessorEpochMismatch {
        event: [u8; 32],
        predecessor: [u8; 32],
    },
    OrdinalGap {
        event: [u8; 32],
        expected: u64,
        actual: u64,
    },
    WriterGenerationRegression {
        event: [u8; 32],
        predecessor: [u8; 32],
    },
    DisconnectedOrCyclicHistory {
        reachable: u64,
        total: u64,
    },
}

impl TryFrom<ActivationError> for StoredActivationError {
    type Error = TryFromIntError;

    fn try_from(value: ActivationError) -> Result<Self, Self::Error> {
        Ok(match value {
            ActivationError::CommandDoesNotSelectEpoch => Self::CommandDoesNotSelectEpoch,
            ActivationError::ProofReceiptMismatch => Self::ProofReceiptMismatch,
            ActivationError::SelectedEpochMismatch { command, event } => {
                Self::SelectedEpochMismatch {
                    command: *command.as_bytes(),
                    event: *event.as_bytes(),
                }
            }
            ActivationError::CommandPinMismatch => Self::CommandPinMismatch,
            ActivationError::WorkspaceMismatch { expected, actual } => Self::WorkspaceMismatch {
                expected: *expected.as_bytes(),
                actual: *actual.as_bytes(),
            },
            ActivationError::DuplicateEvent(event_id) => Self::DuplicateEvent {
                event_id: *event_id.as_bytes(),
            },
            ActivationError::DuplicateOperation(operation_id) => Self::DuplicateOperation {
                operation_id: *operation_id.as_bytes(),
            },
            ActivationError::DuplicateOperationSelection(selection) => {
                Self::DuplicateOperationSelection {
                    selection: *selection.as_bytes(),
                }
            }
            ActivationError::DuplicateTransaction(transaction) => Self::DuplicateTransaction {
                transaction: *transaction.as_bytes(),
            },
            ActivationError::RootCount(count) => Self::RootCount {
                count: u64::try_from(count)?,
            },
            ActivationError::InvalidRoot(event_id) => Self::InvalidRoot {
                event_id: *event_id.as_bytes(),
            },
            ActivationError::MissingPredecessor { event, predecessor } => {
                Self::MissingPredecessor {
                    event: *event.as_bytes(),
                    predecessor: *predecessor.as_bytes(),
                }
            }
            ActivationError::Fork(event_id) => Self::Fork {
                event_id: *event_id.as_bytes(),
            },
            ActivationError::PredecessorEpochMismatch { event, predecessor } => {
                Self::PredecessorEpochMismatch {
                    event: *event.as_bytes(),
                    predecessor: *predecessor.as_bytes(),
                }
            }
            ActivationError::OrdinalGap {
                event,
                expected,
                actual,
            } => Self::OrdinalGap {
                event: *event.as_bytes(),
                expected,
                actual,
            },
            ActivationError::WriterGenerationRegression { event, predecessor } => {
                Self::WriterGenerationRegression {
                    event: *event.as_bytes(),
                    predecessor: *predecessor.as_bytes(),
                }
            }
            ActivationError::DisconnectedOrCyclicHistory { reachable, total } => {
                Self::DisconnectedOrCyclicHistory {
                    reachable: u64::try_from(reachable)?,
                    total: u64::try_from(total)?,
                }
            }
        })
    }
}

impl TryFrom<StoredActivationError> for ActivationError {
    type Error = TryFromIntError;

    fn try_from(value: StoredActivationError) -> Result<Self, Self::Error> {
        Ok(match value {
            StoredActivationError::CommandDoesNotSelectEpoch => Self::CommandDoesNotSelectEpoch,
            StoredActivationError::ProofReceiptMismatch => Self::ProofReceiptMismatch,
            StoredActivationError::SelectedEpochMismatch { command, event } => {
                Self::SelectedEpochMismatch {
                    command: super::command::EpochId::from_bytes(command),
                    event: super::command::EpochId::from_bytes(event),
                }
            }
            StoredActivationError::CommandPinMismatch => Self::CommandPinMismatch,
            StoredActivationError::WorkspaceMismatch { expected, actual } => {
                Self::WorkspaceMismatch {
                    expected: WorkspaceId::from_bytes(expected),
                    actual: WorkspaceId::from_bytes(actual),
                }
            }
            StoredActivationError::DuplicateEvent { event_id } => {
                Self::DuplicateEvent(ActivationEventId::from_bytes(event_id))
            }
            StoredActivationError::DuplicateOperation { operation_id } => {
                Self::DuplicateOperation(super::command::OperationId::from_bytes(operation_id))
            }
            StoredActivationError::DuplicateOperationSelection { selection } => {
                Self::DuplicateOperationSelection(OperationSelectionRef::from_bytes(selection))
            }
            StoredActivationError::DuplicateTransaction { transaction } => {
                Self::DuplicateTransaction(TransactionRef::from_bytes(transaction))
            }
            StoredActivationError::RootCount { count } => Self::RootCount(usize::try_from(count)?),
            StoredActivationError::InvalidRoot { event_id } => {
                Self::InvalidRoot(ActivationEventId::from_bytes(event_id))
            }
            StoredActivationError::MissingPredecessor { event, predecessor } => {
                Self::MissingPredecessor {
                    event: ActivationEventId::from_bytes(event),
                    predecessor: ActivationEventId::from_bytes(predecessor),
                }
            }
            StoredActivationError::Fork { event_id } => {
                Self::Fork(ActivationEventId::from_bytes(event_id))
            }
            StoredActivationError::PredecessorEpochMismatch { event, predecessor } => {
                Self::PredecessorEpochMismatch {
                    event: ActivationEventId::from_bytes(event),
                    predecessor: ActivationEventId::from_bytes(predecessor),
                }
            }
            StoredActivationError::OrdinalGap {
                event,
                expected,
                actual,
            } => Self::OrdinalGap {
                event: ActivationEventId::from_bytes(event),
                expected,
                actual,
            },
            StoredActivationError::WriterGenerationRegression { event, predecessor } => {
                Self::WriterGenerationRegression {
                    event: ActivationEventId::from_bytes(event),
                    predecessor: ActivationEventId::from_bytes(predecessor),
                }
            }
            StoredActivationError::DisconnectedOrCyclicHistory { reachable, total } => {
                Self::DisconnectedOrCyclicHistory {
                    reachable: usize::try_from(reachable)?,
                    total: usize::try_from(total)?,
                }
            }
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
enum StoredAdmissionError {
    SelectedEpochUnavailable {
        epoch: [u8; 16],
    },
    ResolvedEpochIdentityMismatch {
        selected: [u8; 16],
        resolved: [u8; 16],
    },
    AdmissionClosed,
    NoActiveEpoch,
    AdmissionAlreadyClosed,
    StalePredecessor {
        expected: super::command::ExpectedHead,
        actual: super::command::ExpectedHead,
    },
    ForeignBarrier,
    StaleBarrier,
    AdmissionNotClosed,
    SelectionAlreadyPublished,
    WorkspaceMismatch,
    ProcessCacheChangedWhileClosed,
    MissingSelectedEvent,
    SelectedEventPredecessorMismatch,
    SelectedEventFenceMismatch,
    SelectedCandidateMismatch {
        selected: [u8; 16],
        candidate: [u8; 16],
    },
    SelectionNotPublished,
    ReconciliationMismatch {
        selected: [u8; 16],
        reconciled: super::command::ExpectedHead,
        active: super::command::ExpectedHead,
    },
    CannotAbortAfterSelection,
    RecoveryEventIsNotHead,
    RecoveryHeadMismatch {
        durable: super::command::ExpectedHead,
        active: super::command::ExpectedHead,
    },
    RecoveryPublishedSelectionMismatch,
    RecoveryAdmissionUnexpectedlyOpen,
    ShutdownTransitionInProgress,
    RecoveryFenceNotAuthorized {
        execution: WriterFence,
        active: WriterFence,
    },
    SuccessorQueryAuthorityUnavailable {
        epoch: [u8; 16],
    },
    SuccessorQueryAuthorityMismatch {
        epoch: [u8; 16],
    },
    SuccessorQueryAuthorityInstallFailed {
        epoch: [u8; 16],
    },
    StatePoisoned,
    InternalInvariant {
        code: String,
    },
}

impl From<AdmissionError> for StoredAdmissionError {
    fn from(value: AdmissionError) -> Self {
        match value {
            AdmissionError::SelectedEpochUnavailable(epoch) => Self::SelectedEpochUnavailable {
                epoch: *epoch.as_bytes(),
            },
            AdmissionError::ResolvedEpochIdentityMismatch { selected, resolved } => {
                Self::ResolvedEpochIdentityMismatch {
                    selected: *selected.as_bytes(),
                    resolved: *resolved.as_bytes(),
                }
            }
            AdmissionError::AdmissionClosed => Self::AdmissionClosed,
            AdmissionError::NoActiveEpoch => Self::NoActiveEpoch,
            AdmissionError::AdmissionAlreadyClosed => Self::AdmissionAlreadyClosed,
            AdmissionError::StalePredecessor { expected, actual } => {
                Self::StalePredecessor { expected, actual }
            }
            AdmissionError::ForeignBarrier => Self::ForeignBarrier,
            AdmissionError::StaleBarrier => Self::StaleBarrier,
            AdmissionError::AdmissionNotClosed => Self::AdmissionNotClosed,
            AdmissionError::SelectionAlreadyPublished => Self::SelectionAlreadyPublished,
            AdmissionError::WorkspaceMismatch => Self::WorkspaceMismatch,
            AdmissionError::ProcessCacheChangedWhileClosed => Self::ProcessCacheChangedWhileClosed,
            AdmissionError::MissingSelectedEvent => Self::MissingSelectedEvent,
            AdmissionError::SelectedEventPredecessorMismatch => {
                Self::SelectedEventPredecessorMismatch
            }
            AdmissionError::SelectedEventFenceMismatch => Self::SelectedEventFenceMismatch,
            AdmissionError::SelectedCandidateMismatch {
                selected,
                candidate,
            } => Self::SelectedCandidateMismatch {
                selected: *selected.as_bytes(),
                candidate: *candidate.as_bytes(),
            },
            AdmissionError::SelectionNotPublished => Self::SelectionNotPublished,
            AdmissionError::ReconciliationMismatch {
                selected,
                reconciled,
                active,
            } => Self::ReconciliationMismatch {
                selected: *selected.as_bytes(),
                reconciled,
                active,
            },
            AdmissionError::CannotAbortAfterSelection => Self::CannotAbortAfterSelection,
            AdmissionError::RecoveryEventIsNotHead => Self::RecoveryEventIsNotHead,
            AdmissionError::RecoveryHeadMismatch { durable, active } => {
                Self::RecoveryHeadMismatch { durable, active }
            }
            AdmissionError::RecoveryPublishedSelectionMismatch => {
                Self::RecoveryPublishedSelectionMismatch
            }
            AdmissionError::RecoveryAdmissionUnexpectedlyOpen => {
                Self::RecoveryAdmissionUnexpectedlyOpen
            }
            AdmissionError::ShutdownTransitionInProgress => Self::ShutdownTransitionInProgress,
            AdmissionError::RecoveryFenceNotAuthorized { execution, active } => {
                Self::RecoveryFenceNotAuthorized { execution, active }
            }
            AdmissionError::SuccessorQueryAuthorityUnavailable(epoch) => {
                Self::SuccessorQueryAuthorityUnavailable {
                    epoch: *epoch.as_bytes(),
                }
            }
            AdmissionError::SuccessorQueryAuthorityMismatch(epoch) => {
                Self::SuccessorQueryAuthorityMismatch {
                    epoch: *epoch.as_bytes(),
                }
            }
            AdmissionError::SuccessorQueryAuthorityInstallFailed(epoch) => {
                Self::SuccessorQueryAuthorityInstallFailed {
                    epoch: *epoch.as_bytes(),
                }
            }
            AdmissionError::StatePoisoned => Self::StatePoisoned,
            AdmissionError::InternalInvariant(code) => Self::InternalInvariant {
                code: code.to_owned(),
            },
        }
    }
}

impl StoredAdmissionError {
    fn decode(self) -> Result<AdmissionError, CommandPortError> {
        Ok(match self {
            Self::SelectedEpochUnavailable { epoch } => {
                AdmissionError::SelectedEpochUnavailable(super::command::EpochId::from_bytes(epoch))
            }
            Self::ResolvedEpochIdentityMismatch { selected, resolved } => {
                AdmissionError::ResolvedEpochIdentityMismatch {
                    selected: super::command::EpochId::from_bytes(selected),
                    resolved: super::command::EpochId::from_bytes(resolved),
                }
            }
            Self::AdmissionClosed => AdmissionError::AdmissionClosed,
            Self::NoActiveEpoch => AdmissionError::NoActiveEpoch,
            Self::AdmissionAlreadyClosed => AdmissionError::AdmissionAlreadyClosed,
            Self::StalePredecessor { expected, actual } => {
                AdmissionError::StalePredecessor { expected, actual }
            }
            Self::ForeignBarrier => AdmissionError::ForeignBarrier,
            Self::StaleBarrier => AdmissionError::StaleBarrier,
            Self::AdmissionNotClosed => AdmissionError::AdmissionNotClosed,
            Self::SelectionAlreadyPublished => AdmissionError::SelectionAlreadyPublished,
            Self::WorkspaceMismatch => AdmissionError::WorkspaceMismatch,
            Self::ProcessCacheChangedWhileClosed => AdmissionError::ProcessCacheChangedWhileClosed,
            Self::MissingSelectedEvent => AdmissionError::MissingSelectedEvent,
            Self::SelectedEventPredecessorMismatch => {
                AdmissionError::SelectedEventPredecessorMismatch
            }
            Self::SelectedEventFenceMismatch => AdmissionError::SelectedEventFenceMismatch,
            Self::SelectedCandidateMismatch {
                selected,
                candidate,
            } => AdmissionError::SelectedCandidateMismatch {
                selected: super::command::EpochId::from_bytes(selected),
                candidate: super::command::EpochId::from_bytes(candidate),
            },
            Self::SelectionNotPublished => AdmissionError::SelectionNotPublished,
            Self::ReconciliationMismatch {
                selected,
                reconciled,
                active,
            } => AdmissionError::ReconciliationMismatch {
                selected: super::command::EpochId::from_bytes(selected),
                reconciled,
                active,
            },
            Self::CannotAbortAfterSelection => AdmissionError::CannotAbortAfterSelection,
            Self::RecoveryEventIsNotHead => AdmissionError::RecoveryEventIsNotHead,
            Self::RecoveryHeadMismatch { durable, active } => {
                AdmissionError::RecoveryHeadMismatch { durable, active }
            }
            Self::RecoveryPublishedSelectionMismatch => {
                AdmissionError::RecoveryPublishedSelectionMismatch
            }
            Self::RecoveryAdmissionUnexpectedlyOpen => {
                AdmissionError::RecoveryAdmissionUnexpectedlyOpen
            }
            Self::ShutdownTransitionInProgress => AdmissionError::ShutdownTransitionInProgress,
            Self::RecoveryFenceNotAuthorized { execution, active } => {
                AdmissionError::RecoveryFenceNotAuthorized { execution, active }
            }
            Self::SuccessorQueryAuthorityUnavailable { epoch } => {
                AdmissionError::SuccessorQueryAuthorityUnavailable(
                    super::command::EpochId::from_bytes(epoch),
                )
            }
            Self::SuccessorQueryAuthorityMismatch { epoch } => {
                AdmissionError::SuccessorQueryAuthorityMismatch(
                    super::command::EpochId::from_bytes(epoch),
                )
            }
            Self::SuccessorQueryAuthorityInstallFailed { epoch } => {
                AdmissionError::SuccessorQueryAuthorityInstallFailed(
                    super::command::EpochId::from_bytes(epoch),
                )
            }
            Self::StatePoisoned => AdmissionError::StatePoisoned,
            Self::InternalInvariant { code } => {
                AdmissionError::InternalInvariant(match code.as_str() {
                    "activation barrier sequence exhausted" => {
                        "activation barrier sequence exhausted"
                    }
                    "admission generation exhausted" => "admission generation exhausted",
                    "injected publish fault" => "injected publish fault",
                    "injected reopen fault" => "injected reopen fault",
                    _ => return Err(CommandPortError::CorruptRecord),
                })
            }
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
enum StoredActivationReadbackViolation {
    EventContract { error: StoredActivationError },
    EventDiffersFromContract,
    TableVersionSetMismatch,
    ChainWorkspaceMismatch,
    EventIsNotUniqueHead,
    ChainHeadMismatch,
    UnchangedChainMismatch,
    CacheReceiptMismatch,
    AcknowledgementReceiptMismatch,
    AuthorityMismatch,
    RecoveryTicketMismatch,
    OperationMarkerMismatch,
    AcknowledgementMarkerMismatch,
    RecoveryAttemptMismatch,
    RecoveryFenceNotAuthorized,
}

impl TryFrom<ActivationReadbackViolation> for StoredActivationReadbackViolation {
    type Error = TryFromIntError;

    fn try_from(value: ActivationReadbackViolation) -> Result<Self, Self::Error> {
        Ok(match value {
            ActivationReadbackViolation::EventContract(error) => Self::EventContract {
                error: error.try_into()?,
            },
            ActivationReadbackViolation::EventDiffersFromContract => Self::EventDiffersFromContract,
            ActivationReadbackViolation::TableVersionSetMismatch => Self::TableVersionSetMismatch,
            ActivationReadbackViolation::ChainWorkspaceMismatch => Self::ChainWorkspaceMismatch,
            ActivationReadbackViolation::EventIsNotUniqueHead => Self::EventIsNotUniqueHead,
            ActivationReadbackViolation::ChainHeadMismatch => Self::ChainHeadMismatch,
            ActivationReadbackViolation::UnchangedChainMismatch => Self::UnchangedChainMismatch,
            ActivationReadbackViolation::CacheReceiptMismatch => Self::CacheReceiptMismatch,
            ActivationReadbackViolation::AcknowledgementReceiptMismatch => {
                Self::AcknowledgementReceiptMismatch
            }
            ActivationReadbackViolation::AuthorityMismatch => Self::AuthorityMismatch,
            ActivationReadbackViolation::RecoveryTicketMismatch => Self::RecoveryTicketMismatch,
            ActivationReadbackViolation::OperationMarkerMismatch => Self::OperationMarkerMismatch,
            ActivationReadbackViolation::AcknowledgementMarkerMismatch => {
                Self::AcknowledgementMarkerMismatch
            }
            ActivationReadbackViolation::RecoveryAttemptMismatch => Self::RecoveryAttemptMismatch,
            ActivationReadbackViolation::RecoveryFenceNotAuthorized => {
                Self::RecoveryFenceNotAuthorized
            }
        })
    }
}

impl TryFrom<StoredActivationReadbackViolation> for ActivationReadbackViolation {
    type Error = TryFromIntError;

    fn try_from(value: StoredActivationReadbackViolation) -> Result<Self, Self::Error> {
        Ok(match value {
            StoredActivationReadbackViolation::EventContract { error } => {
                Self::EventContract(error.try_into()?)
            }
            StoredActivationReadbackViolation::EventDiffersFromContract => {
                Self::EventDiffersFromContract
            }
            StoredActivationReadbackViolation::TableVersionSetMismatch => {
                Self::TableVersionSetMismatch
            }
            StoredActivationReadbackViolation::ChainWorkspaceMismatch => {
                Self::ChainWorkspaceMismatch
            }
            StoredActivationReadbackViolation::EventIsNotUniqueHead => Self::EventIsNotUniqueHead,
            StoredActivationReadbackViolation::ChainHeadMismatch => Self::ChainHeadMismatch,
            StoredActivationReadbackViolation::UnchangedChainMismatch => {
                Self::UnchangedChainMismatch
            }
            StoredActivationReadbackViolation::CacheReceiptMismatch => Self::CacheReceiptMismatch,
            StoredActivationReadbackViolation::AcknowledgementReceiptMismatch => {
                Self::AcknowledgementReceiptMismatch
            }
            StoredActivationReadbackViolation::AuthorityMismatch => Self::AuthorityMismatch,
            StoredActivationReadbackViolation::RecoveryTicketMismatch => {
                Self::RecoveryTicketMismatch
            }
            StoredActivationReadbackViolation::OperationMarkerMismatch => {
                Self::OperationMarkerMismatch
            }
            StoredActivationReadbackViolation::AcknowledgementMarkerMismatch => {
                Self::AcknowledgementMarkerMismatch
            }
            StoredActivationReadbackViolation::RecoveryAttemptMismatch => {
                Self::RecoveryAttemptMismatch
            }
            StoredActivationReadbackViolation::RecoveryFenceNotAuthorized => {
                Self::RecoveryFenceNotAuthorized
            }
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
enum StoredActivationReconciliationReason {
    AuthorityStale,
    AuthorityUnknown {
        diagnostic: DiagnosticRef,
    },
    CancelledAfterClosure {
        diagnostic: DiagnosticRef,
    },
    AppendUnknown {
        reason: StoredActivationAppendUnknownReason,
        diagnostic: DiagnosticRef,
    },
    ReadbackViolation {
        violation: StoredActivationReadbackViolation,
    },
    AdmissionFailure {
        error: StoredAdmissionError,
    },
    CacheUnknown {
        diagnostic: DiagnosticRef,
    },
    CacheCancelled {
        diagnostic: DiagnosticRef,
    },
    AcknowledgementUnknown {
        diagnostic: DiagnosticRef,
    },
    AcknowledgementCancelled {
        diagnostic: DiagnosticRef,
    },
    OperationMarkerUnknown {
        diagnostic: DiagnosticRef,
    },
}

impl TryFrom<ActivationReconciliationReason> for StoredActivationReconciliationReason {
    type Error = TryFromIntError;

    fn try_from(value: ActivationReconciliationReason) -> Result<Self, Self::Error> {
        Ok(match value {
            ActivationReconciliationReason::AuthorityStale => Self::AuthorityStale,
            ActivationReconciliationReason::AuthorityUnknown(diagnostic) => {
                Self::AuthorityUnknown { diagnostic }
            }
            ActivationReconciliationReason::CancelledAfterClosure(diagnostic) => {
                Self::CancelledAfterClosure { diagnostic }
            }
            ActivationReconciliationReason::AppendUnknown { reason, diagnostic } => {
                Self::AppendUnknown {
                    reason: reason.into(),
                    diagnostic,
                }
            }
            ActivationReconciliationReason::ReadbackViolation(violation) => {
                Self::ReadbackViolation {
                    violation: violation.try_into()?,
                }
            }
            ActivationReconciliationReason::AdmissionFailure(error) => Self::AdmissionFailure {
                error: error.into(),
            },
            ActivationReconciliationReason::CacheUnknown(diagnostic) => {
                Self::CacheUnknown { diagnostic }
            }
            ActivationReconciliationReason::CacheCancelled(diagnostic) => {
                Self::CacheCancelled { diagnostic }
            }
            ActivationReconciliationReason::AcknowledgementUnknown(diagnostic) => {
                Self::AcknowledgementUnknown { diagnostic }
            }
            ActivationReconciliationReason::AcknowledgementCancelled(diagnostic) => {
                Self::AcknowledgementCancelled { diagnostic }
            }
            ActivationReconciliationReason::OperationMarkerUnknown(diagnostic) => {
                Self::OperationMarkerUnknown { diagnostic }
            }
        })
    }
}

impl StoredActivationReconciliationReason {
    fn decode(self) -> Result<ActivationReconciliationReason, CommandPortError> {
        Ok(match self {
            Self::AuthorityStale => ActivationReconciliationReason::AuthorityStale,
            Self::AuthorityUnknown { diagnostic } => {
                ActivationReconciliationReason::AuthorityUnknown(diagnostic)
            }
            Self::CancelledAfterClosure { diagnostic } => {
                ActivationReconciliationReason::CancelledAfterClosure(diagnostic)
            }
            Self::AppendUnknown { reason, diagnostic } => {
                ActivationReconciliationReason::AppendUnknown {
                    reason: reason.into(),
                    diagnostic,
                }
            }
            Self::ReadbackViolation { violation } => {
                ActivationReconciliationReason::ReadbackViolation(
                    violation
                        .try_into()
                        .map_err(|_| CommandPortError::CorruptRecord)?,
                )
            }
            Self::AdmissionFailure { error } => {
                ActivationReconciliationReason::AdmissionFailure(error.decode()?)
            }
            Self::CacheUnknown { diagnostic } => {
                ActivationReconciliationReason::CacheUnknown(diagnostic)
            }
            Self::CacheCancelled { diagnostic } => {
                ActivationReconciliationReason::CacheCancelled(diagnostic)
            }
            Self::AcknowledgementUnknown { diagnostic } => {
                ActivationReconciliationReason::AcknowledgementUnknown(diagnostic)
            }
            Self::AcknowledgementCancelled { diagnostic } => {
                ActivationReconciliationReason::AcknowledgementCancelled(diagnostic)
            }
            Self::OperationMarkerUnknown { diagnostic } => {
                ActivationReconciliationReason::OperationMarkerUnknown(diagnostic)
            }
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
enum StoredDurableSelectionKnowledge {
    NotAttempted,
    Unknown,
    ReadBack { event_id: [u8; 32] },
}

impl From<DurableSelectionKnowledge> for StoredDurableSelectionKnowledge {
    fn from(value: DurableSelectionKnowledge) -> Self {
        match value {
            DurableSelectionKnowledge::NotAttempted => Self::NotAttempted,
            DurableSelectionKnowledge::Unknown => Self::Unknown,
            DurableSelectionKnowledge::ReadBack { event_id } => Self::ReadBack {
                event_id: *event_id.as_bytes(),
            },
        }
    }
}

impl From<StoredDurableSelectionKnowledge> for DurableSelectionKnowledge {
    fn from(value: StoredDurableSelectionKnowledge) -> Self {
        match value {
            StoredDurableSelectionKnowledge::NotAttempted => Self::NotAttempted,
            StoredDurableSelectionKnowledge::Unknown => Self::Unknown,
            StoredDurableSelectionKnowledge::ReadBack { event_id } => Self::ReadBack {
                event_id: ActivationEventId::from_bytes(event_id),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum StoredActivationAdmissionPosture {
    NeverClosed,
    Closed,
    Swapped,
    Reopened,
}

impl From<ActivationAdmissionPosture> for StoredActivationAdmissionPosture {
    fn from(value: ActivationAdmissionPosture) -> Self {
        match value {
            ActivationAdmissionPosture::NeverClosed => Self::NeverClosed,
            ActivationAdmissionPosture::Closed => Self::Closed,
            ActivationAdmissionPosture::Swapped => Self::Swapped,
            ActivationAdmissionPosture::Reopened => Self::Reopened,
        }
    }
}

impl From<StoredActivationAdmissionPosture> for ActivationAdmissionPosture {
    fn from(value: StoredActivationAdmissionPosture) -> Self {
        match value {
            StoredActivationAdmissionPosture::NeverClosed => Self::NeverClosed,
            StoredActivationAdmissionPosture::Closed => Self::Closed,
            StoredActivationAdmissionPosture::Swapped => Self::Swapped,
            StoredActivationAdmissionPosture::Reopened => Self::Reopened,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct StoredActivationReconciliationTicket {
    stage: StoredActivationTransactionStage,
    reason: StoredActivationReconciliationReason,
    workspace_id: [u8; 16],
    operation_id: [u8; 16],
    candidate_epoch: [u8; 16],
    expected_head: super::command::ExpectedHead,
    execution_fence: WriterFence,
    event_id: [u8; 32],
    transaction: [u8; 32],
    operation_selection: [u8; 32],
    durable_selection: StoredDurableSelectionKnowledge,
    admission_posture: StoredActivationAdmissionPosture,
}

impl TryFrom<ActivationReconciliationTicket> for StoredActivationReconciliationTicket {
    type Error = TryFromIntError;

    fn try_from(value: ActivationReconciliationTicket) -> Result<Self, Self::Error> {
        Ok(Self {
            stage: value.stage.into(),
            reason: value.reason.try_into()?,
            workspace_id: *value.workspace_id.as_bytes(),
            operation_id: *value.operation_id.as_bytes(),
            candidate_epoch: *value.candidate_epoch.as_bytes(),
            expected_head: value.expected_head,
            execution_fence: value.execution_fence,
            event_id: *value.event_id.as_bytes(),
            transaction: *value.transaction.as_bytes(),
            operation_selection: *value.operation_selection.as_bytes(),
            durable_selection: value.durable_selection.into(),
            admission_posture: value.admission_posture.into(),
        })
    }
}

impl StoredActivationReconciliationTicket {
    fn decode(self) -> Result<ActivationReconciliationTicket, CommandPortError> {
        Ok(ActivationReconciliationTicket {
            stage: self.stage.into(),
            reason: self.reason.decode()?,
            workspace_id: WorkspaceId::from_bytes(self.workspace_id),
            operation_id: super::command::OperationId::from_bytes(self.operation_id),
            candidate_epoch: super::command::EpochId::from_bytes(self.candidate_epoch),
            expected_head: self.expected_head,
            execution_fence: self.execution_fence,
            event_id: ActivationEventId::from_bytes(self.event_id),
            transaction: TransactionRef::from_bytes(self.transaction),
            operation_selection: OperationSelectionRef::from_bytes(self.operation_selection),
            durable_selection: self.durable_selection.into(),
            admission_posture: self.admission_posture.into(),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct StoredActivationReconciliationRecord {
    request: StoredActivationRequest,
    ticket: StoredActivationReconciliationTicket,
    unknown: UnknownCommit,
    evidence: ReconciliationEvidenceRef,
}

enum StoreRequest {
    PersistRequest {
        row: StoredActivationRequest,
        response: oneshot::Sender<Result<StoredActivationRequest, CommandPortError>>,
    },
    ReadRequest {
        operation_id: super::command::OperationId,
        response: oneshot::Sender<Result<Option<StoredActivationRequest>, CommandPortError>>,
    },
    PersistReconciliation {
        row: StoredActivationReconciliationRecord,
        response: oneshot::Sender<Result<StoredActivationReconciliationRecord, CommandPortError>>,
    },
    ReadReconciliation {
        operation_id: super::command::OperationId,
        response:
            oneshot::Sender<Result<Option<StoredActivationReconciliationRecord>, CommandPortError>>,
    },
}

/// Dedicated durable request/reconciliation store for one workspace.
///
/// A blocking worker exclusively owns the SQLite connection. Immutable request reads rebuild
/// their sealed candidate from the persisted exact Delta vector through `candidate_rebuilder`;
/// no candidate `Arc` or process-local lookup map is retained here.
pub struct SqliteProgrammaticActivationCommandStateStore {
    database_path: PathBuf,
    workspace_id: WorkspaceId,
    candidate_rebuilder: Arc<dyn ActivationCommandCandidateRebuilderPort>,
    control_binding: SealedActivationControlBinding,
    identity_policy: ActivationReconciliationIdentityPolicy,
    sender: Option<Sender<StoreRequest>>,
    worker: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for SqliteProgrammaticActivationCommandStateStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SqliteProgrammaticActivationCommandStateStore")
            .field("database_path", &self.database_path)
            .field("workspace_id", &self.workspace_id)
            .field("candidate_rebuilder", &"exact-delta")
            .finish_non_exhaustive()
    }
}

impl SqliteProgrammaticActivationCommandStateStore {
    /// Open or initialize one private exact-schema activation command store.
    ///
    /// # Errors
    ///
    /// Rejects unsafe paths/files, incompatible schemas, or worker startup failures.
    pub fn open(
        path: &Path,
        workspace_id: WorkspaceId,
        candidate_rebuilder: Arc<dyn ActivationCommandCandidateRebuilderPort>,
        control_binding: SealedActivationControlBinding,
        identity_policy: ActivationReconciliationIdentityPolicy,
    ) -> Result<Self, ProgrammaticActivationStateOpenError> {
        let prepared_file = prepare_private_database_file(path)?;
        let prepared_metadata = prepared_file.metadata().map_err(|source| {
            ProgrammaticActivationStateOpenError::Io {
                path: path.to_owned(),
                source,
            }
        })?;
        let mut connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        validate_same_private_file(path, &prepared_metadata)?;
        apply_pragmas(&connection)?;
        initialize_or_validate_schema(&mut connection)?;
        drop(prepared_file);

        let (sender, receiver) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("codefabric-activation-command-state".to_owned())
            .spawn(move || run_worker(connection, receiver))
            .map_err(ProgrammaticActivationStateOpenError::Worker)?;
        Ok(Self {
            database_path: path.to_owned(),
            workspace_id,
            candidate_rebuilder,
            control_binding,
            identity_policy,
            sender: Some(sender),
            worker: Some(worker),
        })
    }

    #[must_use]
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    /// Persist one immutable typed request before its durable command may execute.
    ///
    /// Repeating the exact row is idempotent; an operation-ID collision with any changed input
    /// is corrupt rather than a last-writer-wins update.
    pub async fn persist_request(
        &self,
        material: &ActivationCommandRequestMaterial,
    ) -> Result<(), CommandPortError> {
        let command = material.key().command();
        let candidate = material.candidate();
        if command.ownership.workspace_id != self.workspace_id
            || candidate.identity() != &material.pins().epoch
            || candidate.table_version_set_ref() != material.pins().table_versions
            || material.control_relation().binding() != &self.control_binding
        {
            return Err(CommandPortError::CorruptRecord);
        }
        let row = StoredActivationRequest::from_material(material);
        let stored = self
            .send_and_receive(|response| StoreRequest::PersistRequest { row, response })
            .await?;
        if stored != StoredActivationRequest::from_material(material) {
            return Err(CommandPortError::CorruptRecord);
        }
        Ok(())
    }

    async fn send_and_receive<T>(
        &self,
        request: impl FnOnce(oneshot::Sender<Result<T, CommandPortError>>) -> StoreRequest,
    ) -> Result<T, CommandPortError> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .as_ref()
            .ok_or(CommandPortError::DurableStoreUnavailable)?
            .send(request(response))
            .map_err(|_| CommandPortError::DurableStoreUnavailable)?;
        receiver
            .await
            .map_err(|_| CommandPortError::DurableStoreUnavailable)?
    }
}

impl Drop for SqliteProgrammaticActivationCommandStateStore {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[async_trait]
impl ActivationCommandStateStore for SqliteProgrammaticActivationCommandStateStore {
    async fn read_request(
        &self,
        key: ActivationCommandRequestKey,
    ) -> Result<Option<ActivationCommandRequestMaterial>, CommandPortError> {
        let operation_id = key.command().identity.operation_id;
        let Some(stored) = self
            .send_and_receive(|response| StoreRequest::ReadRequest {
                operation_id,
                response,
            })
            .await?
        else {
            return Ok(None);
        };
        let material = stored
            .material(
                self.workspace_id,
                self.candidate_rebuilder.as_ref(),
                &self.control_binding,
            )
            .await?;
        if material.key() != key {
            return Err(CommandPortError::CorruptRecord);
        }
        Ok(Some(material))
    }

    async fn read_not_selected_classification(
        &self,
        _query: ActivationNotSelectedClassificationQuery,
    ) -> Result<Option<ActivationNotSelectedClassification>, CommandPortError> {
        // This store deliberately has no synthesized classification. A release that admits a
        // non-passing candidate must provide the explicit typed classification relation before
        // command execution; absence remains a known ContextUnavailable failure in the adapter.
        Ok(None)
    }

    async fn persist_reconciliation(
        &self,
        write: ActivationReconciliationWrite,
    ) -> Result<ActivationReconciliationRecord, CommandPortError> {
        if write.command().ownership.workspace_id != self.workspace_id {
            return Err(CommandPortError::CorruptRecord);
        }
        let ticket: StoredActivationReconciliationTicket = write
            .ticket()
            .try_into()
            .map_err(|_| CommandPortError::CorruptRecord)?;
        let ticket_jcs = encode_canonical(&ticket)?;
        let (unknown, evidence) = self.identity_policy.identities(
            self.workspace_id,
            write.command().identity.operation_id,
            write.transaction(),
            &ticket_jcs,
        );
        let row = StoredActivationReconciliationRecord {
            request: StoredActivationRequest::from_reconciliation(&write),
            ticket,
            unknown,
            evidence,
        };
        let stored = self
            .send_and_receive(|response| StoreRequest::PersistReconciliation { row, response })
            .await?;
        decode_reconciliation(stored, &self.control_binding)
    }

    async fn read_reconciliation(
        &self,
        query: ActivationReconciliationRead,
    ) -> Result<Option<ActivationReconciliationRecord>, CommandPortError> {
        let operation_id = query.command().identity.operation_id;
        let Some(stored) = self
            .send_and_receive(|response| StoreRequest::ReadReconciliation {
                operation_id,
                response,
            })
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(decode_reconciliation(stored, &self.control_binding)?))
    }
}

fn decode_reconciliation(
    stored: StoredActivationReconciliationRecord,
    binding: &SealedActivationControlBinding,
) -> Result<ActivationReconciliationRecord, CommandPortError> {
    let ticket = stored.ticket.decode()?;
    let write = stored.request.reconciliation_write(ticket, binding)?;
    Ok(ActivationReconciliationRecord::new(
        write,
        stored.unknown,
        stored.evidence,
    ))
}

fn run_worker(mut connection: Connection, receiver: Receiver<StoreRequest>) {
    while let Ok(request) = receiver.recv() {
        match request {
            StoreRequest::PersistRequest { row, response } => {
                let _ = response.send(persist_request_sync(&mut connection, row));
            }
            StoreRequest::ReadRequest {
                operation_id,
                response,
            } => {
                let _ = response.send(read_request_sync(&connection, operation_id));
            }
            StoreRequest::PersistReconciliation { row, response } => {
                let _ = response.send(persist_reconciliation_sync(&mut connection, row));
            }
            StoreRequest::ReadReconciliation {
                operation_id,
                response,
            } => {
                let _ = response.send(read_reconciliation_sync(&connection, operation_id));
            }
        }
    }
}

fn persist_request_sync(
    connection: &mut Connection,
    row: StoredActivationRequest,
) -> Result<StoredActivationRequest, CommandPortError> {
    let operation_id = row.command.identity.operation_id;
    let workspace_id = row.command.ownership.workspace_id;
    let encoded = encode_canonical(&row)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    let affected = transaction
        .execute(
            "INSERT OR IGNORE INTO activation_command_request (
                 operation_id, workspace_id, request_jcs
             ) VALUES (?1, ?2, ?3)",
            rusqlite::params![
                operation_id.as_bytes().as_slice(),
                workspace_id.as_bytes().as_slice(),
                encoded,
            ],
        )
        .map_err(unavailable)?;
    let stored = if affected == 1 {
        row.clone()
    } else {
        read_request_on(&transaction, operation_id)?.ok_or(CommandPortError::CorruptRecord)?
    };
    if stored != row {
        return Err(CommandPortError::CorruptRecord);
    }
    transaction.commit().map_err(unavailable)?;
    Ok(stored)
}

fn read_request_sync(
    connection: &Connection,
    operation_id: super::command::OperationId,
) -> Result<Option<StoredActivationRequest>, CommandPortError> {
    read_request_on(connection, operation_id)
}

fn read_request_on(
    connection: &Connection,
    operation_id: super::command::OperationId,
) -> Result<Option<StoredActivationRequest>, CommandPortError> {
    connection
        .query_row(
            "SELECT operation_id, workspace_id, request_jcs
             FROM activation_command_request WHERE operation_id = ?1",
            [operation_id.as_bytes().as_slice()],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            },
        )
        .optional()
        .map_err(unavailable)?
        .map(|(stored_operation, stored_workspace, bytes)| {
            let decoded: StoredActivationRequest = decode_canonical(&bytes)?;
            if stored_operation.as_slice()
                != decoded.command.identity.operation_id.as_bytes().as_slice()
                || stored_workspace.as_slice()
                    != decoded.command.ownership.workspace_id.as_bytes().as_slice()
            {
                return Err(CommandPortError::CorruptRecord);
            }
            Ok(decoded)
        })
        .transpose()
}

fn persist_reconciliation_sync(
    connection: &mut Connection,
    row: StoredActivationReconciliationRecord,
) -> Result<StoredActivationReconciliationRecord, CommandPortError> {
    let operation_id = row.request.command.identity.operation_id;
    let workspace_id = row.request.command.ownership.workspace_id;
    let attempt = row.request.attempt.ok_or(CommandPortError::CorruptRecord)?;
    if attempt == 0 {
        return Err(CommandPortError::CorruptRecord);
    }
    let transaction_ref = row.request.transaction;
    let encoded = encode_canonical(&row)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    let existing = read_reconciliation_on(&transaction, operation_id)?;
    if let Some(existing) = &existing {
        if existing.request != row.request || existing.unknown != row.unknown {
            return Err(CommandPortError::CorruptRecord);
        }
        let affected = transaction
            .execute(
                "UPDATE activation_command_reconciliation
                 SET reconciliation_jcs = ?2
                 WHERE operation_id = ?1",
                rusqlite::params![operation_id.as_bytes().as_slice(), encoded],
            )
            .map_err(unavailable)?;
        if affected != 1 {
            return Err(CommandPortError::CorruptRecord);
        }
    } else {
        let affected = transaction
            .execute(
                "INSERT INTO activation_command_reconciliation (
                     operation_id, workspace_id, attempt, transaction_ref, reconciliation_jcs
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    operation_id.as_bytes().as_slice(),
                    workspace_id.as_bytes().as_slice(),
                    i64::from(attempt),
                    transaction_ref.as_slice(),
                    encoded,
                ],
            )
            .map_err(unavailable)?;
        if affected != 1 {
            return Err(CommandPortError::CorruptRecord);
        }
    }
    transaction.commit().map_err(unavailable)?;
    Ok(row)
}

fn read_reconciliation_sync(
    connection: &Connection,
    operation_id: super::command::OperationId,
) -> Result<Option<StoredActivationReconciliationRecord>, CommandPortError> {
    read_reconciliation_on(connection, operation_id)
}

fn read_reconciliation_on(
    connection: &Connection,
    operation_id: super::command::OperationId,
) -> Result<Option<StoredActivationReconciliationRecord>, CommandPortError> {
    connection
        .query_row(
            "SELECT operation_id, workspace_id, attempt, transaction_ref, reconciliation_jcs
             FROM activation_command_reconciliation WHERE operation_id = ?1",
            [operation_id.as_bytes().as_slice()],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                ))
            },
        )
        .optional()
        .map_err(unavailable)?
        .map(
            |(stored_operation, stored_workspace, stored_attempt, stored_transaction, bytes)| {
                let decoded: StoredActivationReconciliationRecord = decode_canonical(&bytes)?;
                if stored_attempt <= 0
                    || stored_operation.as_slice()
                        != decoded
                            .request
                            .command
                            .identity
                            .operation_id
                            .as_bytes()
                            .as_slice()
                    || stored_workspace.as_slice()
                        != decoded
                            .request
                            .command
                            .ownership
                            .workspace_id
                            .as_bytes()
                            .as_slice()
                    || u32::try_from(stored_attempt).ok() != decoded.request.attempt
                    || stored_transaction.as_slice() != decoded.request.transaction.as_slice()
                {
                    return Err(CommandPortError::CorruptRecord);
                }
                Ok(decoded)
            },
        )
        .transpose()
}

fn encode_canonical<T: Serialize>(value: &T) -> Result<Vec<u8>, CommandPortError> {
    let bytes =
        serde_json_canonicalizer::to_vec(value).map_err(|_| CommandPortError::CorruptRecord)?;
    if !(2..=MAX_ROW_BYTES).contains(&bytes.len()) {
        return Err(CommandPortError::CorruptRecord);
    }
    Ok(bytes)
}

fn decode_canonical<T>(bytes: &[u8]) -> Result<T, CommandPortError>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    if !(2..=MAX_ROW_BYTES).contains(&bytes.len()) {
        return Err(CommandPortError::CorruptRecord);
    }
    let value = serde_json::from_slice(bytes).map_err(|_| CommandPortError::CorruptRecord)?;
    if encode_canonical(&value)? != bytes {
        return Err(CommandPortError::CorruptRecord);
    }
    Ok(value)
}

fn unavailable(_error: rusqlite::Error) -> CommandPortError {
    CommandPortError::DurableStoreUnavailable
}

fn prepare_private_database_file(
    path: &Path,
) -> Result<File, ProgrammaticActivationStateOpenError> {
    let parent = path
        .parent()
        .filter(|candidate| !candidate.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent_metadata = fs::symlink_metadata(parent)
        .map_err(|_| ProgrammaticActivationStateOpenError::UnsafeParent(parent.to_owned()))?;
    let owner = rustix::process::geteuid().as_raw();
    if !parent_metadata.is_dir()
        || parent_metadata.file_type().is_symlink()
        || parent_metadata.uid() != owner
        || parent_metadata.permissions().mode() & 0o777 != 0o700
    {
        return Err(ProgrammaticActivationStateOpenError::UnsafeParent(
            parent.to_owned(),
        ));
    }
    let file_name = path
        .file_name()
        .ok_or_else(|| ProgrammaticActivationStateOpenError::InvalidPath(path.to_owned()))?;
    let directory = open(
        parent,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY,
        Mode::empty(),
    )
    .map_err(|_| ProgrammaticActivationStateOpenError::UnsafeParent(parent.to_owned()))?;
    let descriptor = openat(
        &directory,
        file_name,
        OFlags::RDWR | OFlags::CREATE | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|_| ProgrammaticActivationStateOpenError::UnsafeDatabase(path.to_owned()))?;
    let file = File::from(descriptor);
    let metadata = file
        .metadata()
        .map_err(|source| ProgrammaticActivationStateOpenError::Io {
            path: path.to_owned(),
            source,
        })?;
    if !private_database_metadata(&metadata, owner) {
        return Err(ProgrammaticActivationStateOpenError::UnsafeDatabase(
            path.to_owned(),
        ));
    }
    file.sync_all()
        .map_err(|source| ProgrammaticActivationStateOpenError::Io {
            path: path.to_owned(),
            source,
        })?;
    Ok(file)
}

fn validate_same_private_file(
    path: &Path,
    prepared: &fs::Metadata,
) -> Result<(), ProgrammaticActivationStateOpenError> {
    let observed = fs::symlink_metadata(path)
        .map_err(|_| ProgrammaticActivationStateOpenError::UnsafeDatabase(path.to_owned()))?;
    if !private_database_metadata(&observed, rustix::process::geteuid().as_raw())
        || observed.dev() != prepared.dev()
        || observed.ino() != prepared.ino()
    {
        return Err(ProgrammaticActivationStateOpenError::UnsafeDatabase(
            path.to_owned(),
        ));
    }
    Ok(())
}

fn private_database_metadata(metadata: &fs::Metadata, owner: u32) -> bool {
    metadata.is_file()
        && !metadata.file_type().is_symlink()
        && metadata.uid() == owner
        && metadata.nlink() == 1
        && metadata.permissions().mode() & 0o777 == 0o600
}

fn apply_pragmas(connection: &Connection) -> Result<(), ProgrammaticActivationStateOpenError> {
    connection.busy_timeout(BUSY_TIMEOUT)?;
    let journal_mode: String =
        connection.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
    if journal_mode != "wal" {
        return Err(ProgrammaticActivationStateOpenError::UnexpectedSchema(
            format!("journal_mode is {journal_mode}, expected wal"),
        ));
    }
    connection.execute_batch(
        "PRAGMA synchronous=FULL;
         PRAGMA foreign_keys=ON;
         PRAGMA trusted_schema=OFF;
         PRAGMA secure_delete=FAST;
         PRAGMA wal_autocheckpoint=1000;",
    )?;
    let synchronous: i64 = connection.query_row("PRAGMA synchronous", [], |row| row.get(0))?;
    if synchronous != 2 {
        return Err(ProgrammaticActivationStateOpenError::UnexpectedSchema(
            format!("synchronous is {synchronous}, expected FULL (2)"),
        ));
    }
    Ok(())
}

fn initialize_or_validate_schema(
    connection: &mut Connection,
) -> Result<(), ProgrammaticActivationStateOpenError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let observed_version: u32 =
        transaction.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let observed_application: u32 =
        transaction.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    match observed_version {
        0 => {
            if observed_application != 0 || !schema_objects(&transaction)?.is_empty() {
                return Err(ProgrammaticActivationStateOpenError::UnexpectedSchema(
                    "unversioned database already contains application identity or objects".into(),
                ));
            }
            transaction.execute_batch(SCHEMA_V2)?;
        }
        ACTIVATION_COMMAND_STATE_SCHEMA_VERSION => {
            if observed_application != APPLICATION_ID {
                return Err(ProgrammaticActivationStateOpenError::UnexpectedSchema(
                    format!("application_id is {observed_application}, expected {APPLICATION_ID}"),
                ));
            }
        }
        observed => {
            return Err(ProgrammaticActivationStateOpenError::UnsupportedSchema {
                observed,
                supported: ACTIVATION_COMMAND_STATE_SCHEMA_VERSION,
            });
        }
    }
    validate_schema(&transaction)?;
    transaction.commit()?;
    Ok(())
}

fn validate_schema(connection: &Connection) -> Result<(), ProgrammaticActivationStateOpenError> {
    let version: u32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let application: u32 = connection.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    if version != ACTIVATION_COMMAND_STATE_SCHEMA_VERSION || application != APPLICATION_ID {
        return Err(ProgrammaticActivationStateOpenError::UnexpectedSchema(
            "version or application identity changed during schema validation".into(),
        ));
    }
    let objects = schema_objects(connection)?;
    let expected = vec![
        ("table".to_owned(), RECONCILIATION_TABLE.to_owned()),
        ("table".to_owned(), REQUEST_TABLE.to_owned()),
    ];
    if objects != expected {
        return Err(ProgrammaticActivationStateOpenError::UnexpectedSchema(
            format!("user object census is {objects:?}"),
        ));
    }
    for (table, required) in [
        (
            REQUEST_TABLE,
            &[
                "length(operation_id) = 16",
                "length(workspace_id) = 16",
                "length(request_jcs) BETWEEN 2 AND 131072",
                "WITHOUT ROWID",
                "STRICT",
            ][..],
        ),
        (
            RECONCILIATION_TABLE,
            &[
                "length(operation_id) = 16",
                "length(workspace_id) = 16",
                "length(transaction_ref) = 32",
                "length(reconciliation_jcs) BETWEEN 2 AND 131072",
                "WITHOUT ROWID",
                "STRICT",
            ][..],
        ),
    ] {
        let table_sql: String = connection.query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )?;
        for constraint in required {
            if !table_sql.contains(constraint) {
                return Err(ProgrammaticActivationStateOpenError::UnexpectedSchema(
                    format!("{table} is missing required constraint {constraint}"),
                ));
            }
        }
    }
    Ok(())
}

fn schema_objects(
    connection: &Connection,
) -> Result<Vec<(String, String)>, ProgrammaticActivationStateOpenError> {
    let mut statement = connection.prepare(
        "SELECT type, name FROM sqlite_schema
         WHERE name NOT LIKE 'sqlite_%'
         ORDER BY type, name",
    )?;
    Ok(statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use tempfile::TempDir;

    use super::*;
    use crate::fabric::activation::{ActivationAttempt, OverlaySegmentSetRef, PolicySetRef};
    use crate::fabric::activation_command_effect::{
        ActivationCommandStatePort, ResolvedActivationRecovery,
    };
    use crate::fabric::activation_transaction::{
        ActivationAdmissionPosture, ActivationReconciliationReason, ActivationRecoveryRequest,
        DurableSelectionKnowledge,
    };
    use crate::fabric::command::{
        ActorId, AdmissionContext, AuthorizationDecision, AuthorizationRef, CommandEvent,
        CommandIdentity, CommandOwnership, CommandPins, CommandRecord, CommandReducer, EpochId,
        ExpectedHead, FabricCommandPayload, IdempotencyKey, InputReleaseRef, LeaseId, OperationId,
        PrincipalId, ProgramReleaseRef, ProviderSetRef, ReductionContext, ResourceEnvelopeRef,
        SourceGeneration, WriterGeneration,
    };
    use crate::fabric::epoch_runtime::FabricEpochRuntimeConfig;
    use crate::fabric::programmatic_activation_command_ports::ExactActivationCommandState;
    use crate::fabric::programmatic_observation_delta::ProgrammaticObservationWriteIdentity;
    use crate::fabric::programmatic_relation_delta::{
        ProgrammaticRelationDeltaLayout, ProgrammaticRelationDeltaPreparation,
    };
    use crate::fabric::programmatic_schema::{
        DEPENDENCY_OBSERVATION_RELATION_ID, FIELD_OBSERVATION_RELATION_ID,
        PROVENANCE_OBSERVATION_RELATION_ID, ProgrammaticRelationId,
        RELATION_OBSERVATION_RELATION_ID, SCHEMA_OBSERVATION_RELATION_ID,
    };

    const fn id16(seed: u8) -> [u8; 16] {
        [seed; 16]
    }

    const fn id32(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn fence(seed: u8, generation: u64) -> WriterFence {
        WriterFence {
            lease_id: LeaseId::from_bytes(id16(seed)),
            generation: WriterGeneration::new(generation).expect("nonzero writer generation"),
        }
    }

    async fn durable_candidate(root: &Path, epoch_id: EpochId) -> Arc<ProgrammaticFabricEpoch> {
        let builder =
            ProgrammaticFabricEpochBuilder::try_new(epoch_id, FabricEpochRuntimeConfig::default())
                .expect("candidate builder");
        let mut roots = BTreeMap::new();
        for relation in [
            RELATION_OBSERVATION_RELATION_ID,
            FIELD_OBSERVATION_RELATION_ID,
            SCHEMA_OBSERVATION_RELATION_ID,
            DEPENDENCY_OBSERVATION_RELATION_ID,
            PROVENANCE_OBSERVATION_RELATION_ID,
        ] {
            let path = root.join(relation.replace('.', "_"));
            fs::create_dir_all(&path).expect("durable test Delta root");
            roots.insert(
                ProgrammaticRelationId::new(relation),
                Url::from_directory_path(path).expect("file URL"),
            );
        }
        let targets = builder
            .provision_observation_histories(roots)
            .await
            .expect("provision exact histories");
        let identity = ProgrammaticObservationWriteIdentity::new(
            epoch_id,
            OperationId::from_bytes(id16(0x31)),
            WriterGeneration::new(1).expect("writer generation"),
            TransactionRef::from_bytes(id32(0x32)),
        );
        Arc::new(
            builder
                .seal(
                    identity,
                    targets,
                    ProgrammaticRelationDeltaPreparation::Genesis(
                        ProgrammaticRelationDeltaLayout::try_new(
                            Url::from_directory_path(root.join("relation-snapshots"))
                                .expect("relation-snapshot file URL"),
                        )
                        .expect("relation-snapshot layout"),
                    ),
                )
                .await
                .expect("seal exact candidate"),
        )
    }

    fn command_and_material(
        workspace_id: WorkspaceId,
        candidate: Arc<ProgrammaticFabricEpoch>,
        binding: SealedActivationControlBinding,
        operation_seed: u8,
    ) -> (FabricCommand, ActivationCommandRequestMaterial) {
        let epoch_id = *candidate.identity();
        let writer_fence = fence(0x41, 1);
        let proof_receipt = super::super::command::ProofReceiptRef::from_bytes(id32(0x42));
        let command = FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes(id16(operation_seed)),
                idempotency_key: IdempotencyKey::from_bytes(id32(operation_seed.wrapping_add(1))),
            },
            ownership: CommandOwnership {
                workspace_id,
                principal_id: PrincipalId::from_bytes(id16(0x43)),
                authorization: AuthorizationRef::from_bytes(id32(0x44)),
            },
            expected_head: ExpectedHead::Empty,
            writer_fence,
            pins: CommandPins {
                input_release: InputReleaseRef::from_bytes(id32(0x45)),
                program_release: ProgramReleaseRef::from_bytes(id32(0x46)),
                application_release: crate::fabric::command::ApplicationReleaseRef::from_bytes(
                    id32(0x46),
                ),
                source_authority: crate::fabric::command::SourceAuthorityRef::from_bytes(id32(
                    0x46,
                )),
                provider_release: crate::fabric::command::ProviderReleaseRef::from_bytes(id32(
                    0x46,
                )),
                source_generation: SourceGeneration::new(7),
                provider_set: ProviderSetRef::from_bytes(id32(0x47)),
            },
            resources: ResourceEnvelopeRef::from_bytes(id32(0x48)),
            payload: FabricCommandPayload::ActivateEpoch {
                candidate_epoch: epoch_id,
                proof_receipt,
            },
        };
        let pins = FabricEpochPins {
            epoch: epoch_id,
            input_release: command.pins.input_release,
            program_release: command.pins.program_release,
            application_release: command.pins.application_release,
            source_authority: command.pins.source_authority,
            provider_release: command.pins.provider_release,
            source_generation: command.pins.source_generation,
            provider_set: command.pins.provider_set,
            table_versions: candidate.table_version_set_ref(),
            overlay_segments: OverlaySegmentSetRef::from_bytes(id32(0x49)),
            policy_set: PolicySetRef::from_bytes(id32(0x4a)),
            resource_envelope: command.resources,
            proof_receipt,
        };
        let control_relation = ActivationControlRelationPin::new(
            ExactDeltaPin::new(
                &Url::parse("memory:///codefabric/activation-command-state").expect("URL"),
                7,
            )
            .expect("control pin"),
            binding,
        );
        let material = ActivationCommandRequestMaterial::new(
            ActivationCommandRequestKey::new(command),
            candidate,
            pins,
            ActivationEventId::from_bytes(id32(0x4b)),
            CompatibilityClassRef::from_bytes(id32(0x4c)),
            RetentionPolicyRef::from_bytes(id32(0x4d)),
            OperationSelectionRef::from_bytes(id32(0x4e)),
            TransactionRef::from_bytes(id32(0x4f)),
            control_relation,
        );
        (command, material)
    }

    fn executing_record(
        command: FabricCommand,
        owner: ExecutionOwner,
        context: ReductionContext,
    ) -> CommandRecord {
        let admitted = CommandReducer::admit(
            None,
            &command,
            AdmissionContext {
                workspace_id: command.ownership.workspace_id,
                current_head: context.current_head,
                active_fence: context.active_fence,
                authorization: AuthorizationDecision::Authorized(command.ownership.authorization),
            },
        )
        .expect("admit activation command")
        .record();
        CommandReducer::reduce(&admitted, CommandEvent::Start { owner }, context)
            .expect("start activation command")
            .record
    }

    fn rebuilder(
        workspace_id: WorkspaceId,
        calls: Arc<AtomicUsize>,
    ) -> Arc<dyn ActivationCommandCandidateRebuilderPort> {
        Arc::new(ExactDeltaActivationCommandCandidateRebuilder::new(
            workspace_id,
            move |epoch_id| {
                calls.fetch_add(1, Ordering::SeqCst);
                ProgrammaticFabricEpochBuilder::try_new(
                    epoch_id,
                    FabricEpochRuntimeConfig::default(),
                )
            },
        ))
    }

    fn identity_policy() -> ActivationReconciliationIdentityPolicy {
        ActivationReconciliationIdentityPolicy::try_new(
            UnknownCommitReason::ReadbackUnavailable,
            id32(0x51),
            id32(0x52),
        )
        .expect("explicit reconciliation identity policy")
    }

    #[tokio::test]
    async fn sqlite_rehydrates_exact_delta_request_and_reconciliation_after_process_reopen() {
        let delta_root = TempDir::new().expect("Delta root");
        let state_root = TempDir::new().expect("state root");
        fs::set_permissions(state_root.path(), fs::Permissions::from_mode(0o700))
            .expect("private state root");
        let database = state_root.path().join("activation-state.sqlite3");
        let workspace_id = WorkspaceId::from_bytes(id16(0x20));
        let epoch_id = EpochId::from_bytes(id16(0x21));
        let candidate = durable_candidate(delta_root.path(), epoch_id).await;
        let binding = SealedActivationControlBinding::for_test(
            "activation-state-session",
            "binding.system.activation-control.delta",
        );
        let (command, material) =
            command_and_material(workspace_id, Arc::clone(&candidate), binding.clone(), 0x22);
        let owner = ExecutionOwner {
            actor_id: ActorId::from_bytes(id16(0x23)),
            fence: command.writer_fence,
        };
        let context = ReductionContext {
            current_head: command.expected_head,
            active_fence: command.writer_fence,
        };
        let attempt = ActivationAttempt::for_test(command, 1, owner);
        let recovery_request = ActivationRecoveryRequest::try_new(
            attempt,
            material.pins(),
            material.event_id(),
            material.compatibility(),
            material.retention(),
            material.operation_selection(),
            material.transaction(),
            material.control_relation().clone(),
        )
        .expect("candidate-free recovery request");
        let resolved =
            ResolvedActivationRecovery::try_new(recovery_request, material.transaction())
                .expect("resolved recovery request");
        let ticket = ActivationReconciliationTicket {
            stage: ActivationTransactionStage::AuthorityRevalidation,
            reason: ActivationReconciliationReason::AuthorityStale,
            workspace_id,
            operation_id: command.identity.operation_id,
            candidate_epoch: epoch_id,
            expected_head: command.expected_head,
            execution_fence: command.writer_fence,
            event_id: material.event_id(),
            transaction: material.transaction(),
            operation_selection: material.operation_selection(),
            durable_selection: DurableSelectionKnowledge::NotAttempted,
            admission_posture: ActivationAdmissionPosture::Closed,
        };

        let first_rebuilds = Arc::new(AtomicUsize::new(0));
        let persisted = {
            let store = Arc::new(
                SqliteProgrammaticActivationCommandStateStore::open(
                    &database,
                    workspace_id,
                    rebuilder(workspace_id, Arc::clone(&first_rebuilds)),
                    binding.clone(),
                    identity_policy(),
                )
                .expect("open first process store"),
            );
            store
                .persist_request(&material)
                .await
                .expect("persist immutable activation input");
            let state = ExactActivationCommandState::new(store);
            state
                .persist_reconciliation(&resolved, ticket)
                .await
                .expect("persist primitive reconciliation row")
        };
        assert_eq!(first_rebuilds.load(Ordering::SeqCst), 0);

        let executing = executing_record(command, owner, context);
        let prepared = CommandReducer::reduce(
            &executing,
            CommandEvent::PrepareCommit {
                owner,
                transaction: material.transaction(),
            },
            context,
        )
        .expect("prepare activation commit")
        .record;
        let awaiting = CommandReducer::reduce(
            &prepared,
            CommandEvent::ReportUnknownCommit {
                owner,
                transaction: material.transaction(),
                unknown: persisted.unknown,
            },
            context,
        )
        .expect("record durable unknown outcome")
        .record;

        let second_rebuilds = Arc::new(AtomicUsize::new(0));
        let store = Arc::new(
            SqliteProgrammaticActivationCommandStateStore::open(
                &database,
                workspace_id,
                rebuilder(workspace_id, Arc::clone(&second_rebuilds)),
                binding,
                identity_policy(),
            )
            .expect("reopen store in a fresh process owner"),
        );
        let rehydrated = store
            .read_request(material.key())
            .await
            .expect("read exact request")
            .expect("persisted request");
        assert_eq!(second_rebuilds.load(Ordering::SeqCst), 1);
        assert!(!Arc::ptr_eq(rehydrated.candidate(), &candidate));
        assert_eq!(
            rehydrated.candidate().table_version_set(),
            candidate.table_version_set()
        );

        let state = ExactActivationCommandState::new(store);
        let loaded = state
            .load_reconciliation(&awaiting, owner, material.transaction(), context)
            .await
            .expect("rehydrate reducer-authorized recovery request");
        assert_eq!(loaded.ticket(), ticket);
        assert_eq!(loaded.resolved().request().pins(), material.pins());
        let recovered_attempt = loaded.resolved().request().attempt();
        assert_eq!(recovered_attempt.command(), attempt.command());
        assert_eq!(recovered_attempt.attempt(), attempt.attempt());
        assert_eq!(
            recovered_attempt.execution_owner(),
            attempt.execution_owner()
        );
        assert_eq!(attempt.prepared_transaction(), None);
        assert_eq!(
            recovered_attempt.prepared_transaction(),
            Some(material.transaction())
        );

        let (missing_command, _) = command_and_material(
            workspace_id,
            Arc::clone(&candidate),
            rehydrated.control_relation().binding().clone(),
            0x72,
        );
        let missing_attempt = ActivationAttempt::for_test(missing_command, 1, owner);
        let missing_record = executing_record(missing_command, owner, context);
        assert!(matches!(
            state
                .resolve_request(&missing_record, missing_attempt, context)
                .await,
            Err(CommandPortError::ContextUnavailable)
        ));
    }

    #[test]
    fn sqlite_rejects_a_non_private_state_parent() {
        let root = TempDir::new().expect("state root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o755))
            .expect("make parent unsafe");
        let workspace_id = WorkspaceId::from_bytes(id16(0x61));
        let calls = Arc::new(AtomicUsize::new(0));
        let binding = SealedActivationControlBinding::for_test(
            "unsafe-parent-session",
            "binding.system.activation-control.delta",
        );
        assert!(matches!(
            SqliteProgrammaticActivationCommandStateStore::open(
                &root.path().join("activation-state.sqlite3"),
                workspace_id,
                rebuilder(workspace_id, calls),
                binding,
                identity_policy(),
            ),
            Err(ProgrammaticActivationStateOpenError::UnsafeParent(_))
        ));
    }
}
