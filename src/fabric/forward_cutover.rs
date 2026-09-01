//! Durable, append-only authority cutover for the relational-fabric successor.
//!
//! A cutover phase is derived exclusively from immutable events.  Socket files, process-local
//! receipts, operator status, and supervisor availability are observations, never phase
//! authority.  Every event binds the admitted command transaction, active writer fence, exact
//! activation identity where applicable, deployment configuration, and private UDS endpoint.
//! Unknown outcomes append reconciliation evidence and close admission unless exact durable
//! readback either finds the event or proves that it did not commit.

use std::fs::{self, File};
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, OptionalExtension as _, TransactionBehavior, params};
use rustix::fs::{Mode, OFlags, open, openat};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::activation::{
    ActivationChain, ActivationEventId, ActivationReadbackRef, BackendCommitRef, TableVersionSetRef,
};
use super::command::{
    ActorId, CommandRecord, EpochId, ExecutionOwner, LeaseId, OperationId, ProofReceiptRef,
    ReductionContext, TransactionRef, WorkspaceId, WriterFence, WriterGeneration,
};
use super::command_actor::CommandPortError;
use super::command_effect_contract::{ValidatedCommandAttempt, prepared_attempt};

/// Exact schema version of the dedicated append-only cutover journal.
pub const FORWARD_CUTOVER_SCHEMA_VERSION: u32 = 1;

const APPLICATION_ID: u32 = 0x4346_4354; // `CFCT`
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const EVENT_DOMAIN: &[u8] = b"codefabric.forward-cutover.event.v1";
const UDS_DOMAIN: &[u8] = b"codefabric.forward-cutover.uds-endpoint.v1";
const EVENT_TABLE: &str = "forward_cutover_event";
const RECONCILIATION_TABLE: &str = "forward_cutover_reconciliation";
const SCHEMA_V1: &str = "CREATE TABLE forward_cutover_event (
    workspace_id BLOB NOT NULL
        CHECK (typeof(workspace_id) = 'blob' AND length(workspace_id) = 16),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    event_id BLOB NOT NULL UNIQUE
        CHECK (typeof(event_id) = 'blob' AND length(event_id) = 32),
    previous_event_id BLOB
        CHECK (previous_event_id IS NULL OR
               (typeof(previous_event_id) = 'blob' AND length(previous_event_id) = 32)),
    operation_id BLOB NOT NULL
        CHECK (typeof(operation_id) = 'blob' AND length(operation_id) = 16),
    transaction_ref BLOB NOT NULL
        CHECK (typeof(transaction_ref) = 'blob' AND length(transaction_ref) = 32),
    payload BLOB NOT NULL CHECK (typeof(payload) = 'blob' AND length(payload) > 0),
    PRIMARY KEY (workspace_id, sequence),
    UNIQUE (workspace_id, operation_id),
    UNIQUE (workspace_id, transaction_ref)
) WITHOUT ROWID, STRICT;
CREATE TABLE forward_cutover_reconciliation (
    workspace_id BLOB NOT NULL
        CHECK (typeof(workspace_id) = 'blob' AND length(workspace_id) = 16),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    evidence_id BLOB NOT NULL UNIQUE
        CHECK (typeof(evidence_id) = 'blob' AND length(evidence_id) = 32),
    operation_id BLOB NOT NULL
        CHECK (typeof(operation_id) = 'blob' AND length(operation_id) = 16),
    transaction_ref BLOB NOT NULL
        CHECK (typeof(transaction_ref) = 'blob' AND length(transaction_ref) = 32),
    intended_event_id BLOB NOT NULL
        CHECK (typeof(intended_event_id) = 'blob' AND length(intended_event_id) = 32),
    outcome_code INTEGER NOT NULL CHECK (outcome_code BETWEEN 1 AND 4),
    observed_event_id BLOB
        CHECK (observed_event_id IS NULL OR
               (typeof(observed_event_id) = 'blob' AND length(observed_event_id) = 32)),
    command_readback_id BLOB
        CHECK (command_readback_id IS NULL OR
               (typeof(command_readback_id) = 'blob' AND length(command_readback_id) = 32)),
    delta_readback_id BLOB
        CHECK (delta_readback_id IS NULL OR
               (typeof(delta_readback_id) = 'blob' AND length(delta_readback_id) = 32)),
    supervisor_readback_id BLOB
        CHECK (supervisor_readback_id IS NULL OR
               (typeof(supervisor_readback_id) = 'blob' AND length(supervisor_readback_id) = 32)),
    resolution_code INTEGER NOT NULL CHECK (resolution_code BETWEEN 1 AND 4),
    CHECK (
        (outcome_code = 1 AND observed_event_id IS NOT NULL
            AND command_readback_id IS NULL AND delta_readback_id IS NULL
            AND supervisor_readback_id IS NULL)
        OR (outcome_code = 2 AND observed_event_id IS NULL
            AND command_readback_id IS NOT NULL AND delta_readback_id IS NOT NULL
            AND supervisor_readback_id IS NOT NULL)
        OR (outcome_code IN (3, 4) AND observed_event_id IS NULL
            AND command_readback_id IS NULL AND delta_readback_id IS NULL
            AND supervisor_readback_id IS NULL)
    ),
    PRIMARY KEY (workspace_id, sequence)
) WITHOUT ROWID, STRICT;
PRAGMA application_id = 1128678228;
PRAGMA user_version = 1;";

macro_rules! cutover_identity {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        pub struct $name([u8; 32]);

        impl $name {
            /// Construct the typed identity from canonical bytes.
            #[must_use]
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            /// Borrow the canonical identity bytes.
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

cutover_identity!(
    /// Stable identity of one immutable forward-cutover plan.
    CutoverPlanId
);
cutover_identity!(
    /// Exact released daemon/package identity participating in the cutover.
    DaemonReleaseId
);
cutover_identity!(
    /// Content identity of the deployment supervisor configuration.
    SupervisorConfigId
);
cutover_identity!(
    /// Durable observation emitted by the actual deployment supervisor.
    SupervisorObservationId
);
cutover_identity!(
    /// Application identity of the authorized private UDS endpoint.
    UdsEndpointId
);
cutover_identity!(
    /// Immutable identity of one committed cutover event.
    CutoverEventId
);
cutover_identity!(
    /// Immutable reconciliation evidence identity.
    CutoverReconciliationEvidenceId
);
cutover_identity!(
    /// Content identity of one exact command/Delta/supervisor reconciliation readback fact.
    CutoverReadbackFactId
);
cutover_identity!(
    /// Stable identity of the physical host on which the supervisor observation was made.
    HostIdentity
);
cutover_identity!(
    /// Content identity of one supervisor-owned role, revocation, or reboot fact.
    SupervisorFactId
);

impl UdsEndpointId {
    /// Derive an endpoint identity from the exact workspace and absolute authorized path.
    ///
    /// This does not prove that a process owns or serves the socket.  Ownership remains an actual
    /// deployment-supervisor observation.
    pub fn derive(workspace_id: WorkspaceId, path: &Path) -> Result<Self, CutoverEventError> {
        if !path.is_absolute() || path.as_os_str().as_encoded_bytes().is_empty() {
            return Err(CutoverEventError::InvalidUdsEndpoint);
        }
        let mut digest = blake3::Hasher::new();
        digest.update(EVENT_DOMAIN);
        digest.update(&(UDS_DOMAIN.len() as u64).to_be_bytes());
        digest.update(UDS_DOMAIN);
        digest.update(workspace_id.as_bytes());
        let bytes = path.as_os_str().as_encoded_bytes();
        digest.update(&(bytes.len() as u64).to_be_bytes());
        digest.update(bytes);
        Ok(Self(*digest.finalize().as_bytes()))
    }
}

/// Forward-only cutover phases. Physical-zero convergence may close an undeployed predecessor
/// directly; no phase can restore predecessor ownership.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CutoverPhase {
    TargetProved,
    PredecessorFenced,
    TargetServing,
    TargetMutating,
    Complete,
}

/// Which exact released authority is observed to own one exclusive deployment role.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityOwner {
    None,
    Target,
}

impl AuthorityOwner {
    const fn code(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Target => 1,
        }
    }
}

/// Actual package/configuration availability observed by the deployment supervisor.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservedAvailability {
    Absent,
    Present,
}

/// Host boot identity reported by the actual deployment supervisor.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HostBootId([u8; 16]);

impl HostBootId {
    /// Construct a typed boot identity from canonical supervisor bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// Exact supervisor-owned evidence that the frozen predecessor was denied all three roles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PredecessorRevocationReadback {
    pub bind_denial: SupervisorFactId,
    pub serve_denial: SupervisorFactId,
    pub writer_denial: SupervisorFactId,
}

/// Raw production readback emitted by the actual deployment supervisor adapter.
///
/// This is crate-visible so a platform adapter can construct it, but only
/// [`SupervisorObservation::try_from_actual_readback`] can turn it into cutover evidence. The
/// conversion binds the readback to the exact plan releases, host, boot or exact physical-zero
/// census, supervisor configuration, UDS endpoint, and independently identified role
/// observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActualSupervisorReadback {
    pub config_id: SupervisorConfigId,
    pub host_identity: HostIdentity,
    pub previous_boot_id: HostBootId,
    pub current_boot_id: HostBootId,
    pub reboot_observation: SupervisorFactId,
    pub target_release: DaemonReleaseId,
    pub predecessor_release: DaemonReleaseId,
    pub uds_endpoint_id: UdsEndpointId,
    pub uds_owner: AuthorityOwner,
    pub uds_observation: SupervisorFactId,
    pub serving_owner: AuthorityOwner,
    pub serving_observation: SupervisorFactId,
    pub writer_owner: AuthorityOwner,
    pub writer_observation: SupervisorFactId,
    pub activation_head: Option<EpochId>,
    pub programmatic_epoch: Option<EpochId>,
    pub predecessor_revocation: Option<PredecessorRevocationReadback>,
    pub predecessor_package: ObservedAvailability,
    pub temporary_bridge: ObservedAvailability,
}

/// Exact authority census read from a real deployment supervisor/configuration.
///
/// Every value is either a verified identity or a typed observed state. There are no caller-owned
/// success booleans: absence of a bind/serve/write fact is represented by a missing revocation
/// readback and therefore fails closed for every target-authority phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupervisorObservation {
    observation_id: SupervisorObservationId,
    config_id: SupervisorConfigId,
    host_identity: HostIdentity,
    previous_boot_id: HostBootId,
    host_boot_id: HostBootId,
    reboot_observation: SupervisorFactId,
    target_release: DaemonReleaseId,
    predecessor_release: DaemonReleaseId,
    uds_endpoint_id: UdsEndpointId,
    uds_owner: AuthorityOwner,
    uds_observation: SupervisorFactId,
    serving_owner: AuthorityOwner,
    serving_observation: SupervisorFactId,
    writer_owner: AuthorityOwner,
    writer_observation: SupervisorFactId,
    activation_head: Option<EpochId>,
    programmatic_epoch: Option<EpochId>,
    predecessor_revocation: Option<PredecessorRevocationReadback>,
    predecessor_package: ObservedAvailability,
    temporary_bridge: ObservedAvailability,
}

impl SupervisorObservation {
    /// Convert one actual supervisor/configuration readback into application-owned evidence.
    pub fn try_from_actual_readback(
        identity: CutoverChainIdentity,
        readback: ActualSupervisorReadback,
    ) -> Result<Self, CutoverEventError> {
        let physical_predecessor_zero_state = readback.predecessor_package
            == ObservedAvailability::Absent
            && readback.temporary_bridge == ObservedAvailability::Absent;
        if readback.config_id != identity.supervisor_config
            || readback.uds_endpoint_id != identity.uds_endpoint
            || readback.target_release != identity.target_release
            || readback.predecessor_release != identity.predecessor_release
        {
            return Err(CutoverEventError::SupervisorEvidenceMissing);
        }
        if readback.host_identity.as_bytes() == &[0; 32]
            || readback.previous_boot_id.as_bytes() == &[0; 16]
            || readback.current_boot_id.as_bytes() == &[0; 16]
            || (readback.previous_boot_id == readback.current_boot_id
                && !physical_predecessor_zero_state)
            || readback.reboot_observation.as_bytes() == &[0; 32]
            || readback.uds_observation.as_bytes() == &[0; 32]
            || readback.serving_observation.as_bytes() == &[0; 32]
            || readback.writer_observation.as_bytes() == &[0; 32]
            || readback.predecessor_revocation.is_some_and(|revocation| {
                revocation.bind_denial.as_bytes() == &[0; 32]
                    || revocation.serve_denial.as_bytes() == &[0; 32]
                    || revocation.writer_denial.as_bytes() == &[0; 32]
            })
        {
            return Err(CutoverEventError::SupervisorEvidenceMissing);
        }
        let mut observation = Self {
            observation_id: SupervisorObservationId::from_bytes([0; 32]),
            config_id: readback.config_id,
            host_identity: readback.host_identity,
            previous_boot_id: readback.previous_boot_id,
            host_boot_id: readback.current_boot_id,
            reboot_observation: readback.reboot_observation,
            target_release: readback.target_release,
            predecessor_release: readback.predecessor_release,
            uds_endpoint_id: readback.uds_endpoint_id,
            uds_owner: readback.uds_owner,
            uds_observation: readback.uds_observation,
            serving_owner: readback.serving_owner,
            serving_observation: readback.serving_observation,
            writer_owner: readback.writer_owner,
            writer_observation: readback.writer_observation,
            activation_head: readback.activation_head,
            programmatic_epoch: readback.programmatic_epoch,
            predecessor_revocation: readback.predecessor_revocation,
            predecessor_package: readback.predecessor_package,
            temporary_bridge: readback.temporary_bridge,
        };
        observation.observation_id = observation.derived_id();
        Ok(observation)
    }

    fn derived_id(self) -> SupervisorObservationId {
        let mut digest = blake3::Hasher::new();
        digest.update(b"codefabric.forward-cutover.supervisor-observation.v1");
        for bytes in [
            self.config_id.as_bytes().as_slice(),
            self.host_identity.as_bytes().as_slice(),
            self.previous_boot_id.as_bytes().as_slice(),
            self.host_boot_id.as_bytes().as_slice(),
            self.reboot_observation.as_bytes().as_slice(),
            self.target_release.as_bytes().as_slice(),
            self.predecessor_release.as_bytes().as_slice(),
            self.uds_endpoint_id.as_bytes().as_slice(),
            self.uds_observation.as_bytes().as_slice(),
            self.serving_observation.as_bytes().as_slice(),
            self.writer_observation.as_bytes().as_slice(),
        ] {
            digest.update(&(bytes.len() as u64).to_be_bytes());
            digest.update(bytes);
        }
        digest.update(&[
            self.uds_owner.code(),
            self.serving_owner.code(),
            self.writer_owner.code(),
            u8::from(self.predecessor_package == ObservedAvailability::Present),
            u8::from(self.temporary_bridge == ObservedAvailability::Present),
        ]);
        for epoch in [self.activation_head, self.programmatic_epoch] {
            match epoch {
                Some(epoch) => {
                    digest.update(&[1]);
                    digest.update(epoch.as_bytes());
                }
                None => {
                    digest.update(&[0]);
                }
            }
        }
        match self.predecessor_revocation {
            Some(revocation) => {
                digest.update(&[1]);
                for evidence in [
                    revocation.bind_denial,
                    revocation.serve_denial,
                    revocation.writer_denial,
                ] {
                    digest.update(evidence.as_bytes());
                }
            }
            None => {
                digest.update(&[0]);
            }
        }
        SupervisorObservationId::from_bytes(*digest.finalize().as_bytes())
    }

    #[must_use]
    pub const fn observation_id(self) -> SupervisorObservationId {
        self.observation_id
    }

    #[must_use]
    pub const fn config_id(self) -> SupervisorConfigId {
        self.config_id
    }
}

/// Immutable identities shared by every event in one cutover chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CutoverChainIdentity {
    plan_id: CutoverPlanId,
    workspace_id: WorkspaceId,
    deployment_host: HostIdentity,
    target_release: DaemonReleaseId,
    predecessor_release: DaemonReleaseId,
    supervisor_config: SupervisorConfigId,
    uds_endpoint: UdsEndpointId,
}

impl CutoverChainIdentity {
    /// Validate one exact plan/release/deployment identity set.
    pub fn try_new(
        plan_id: CutoverPlanId,
        workspace_id: WorkspaceId,
        deployment_host: HostIdentity,
        target_release: DaemonReleaseId,
        predecessor_release: DaemonReleaseId,
        supervisor_config: SupervisorConfigId,
        uds_endpoint: UdsEndpointId,
    ) -> Result<Self, CutoverEventError> {
        if target_release == predecessor_release {
            return Err(CutoverEventError::ReleaseIdentityCollision);
        }
        if deployment_host.as_bytes() == &[0; 32] {
            return Err(CutoverEventError::InvalidHostIdentity);
        }
        if plan_id.as_bytes() == &[0; 32]
            || workspace_id.as_bytes() == &[0; 16]
            || target_release.as_bytes() == &[0; 32]
            || predecessor_release.as_bytes() == &[0; 32]
            || supervisor_config.as_bytes() == &[0; 32]
            || uds_endpoint.as_bytes() == &[0; 32]
        {
            return Err(CutoverEventError::InvalidChainIdentity);
        }
        Ok(Self {
            plan_id,
            workspace_id,
            deployment_host,
            target_release,
            predecessor_release,
            supervisor_config,
            uds_endpoint,
        })
    }

    /// Workspace governed by this exact cutover chain.
    #[must_use]
    pub const fn workspace_id(self) -> WorkspaceId {
        self.workspace_id
    }

    #[must_use]
    pub const fn plan_id(self) -> CutoverPlanId {
        self.plan_id
    }

    #[must_use]
    pub const fn deployment_host(self) -> HostIdentity {
        self.deployment_host
    }

    #[must_use]
    pub const fn target_release(self) -> DaemonReleaseId {
        self.target_release
    }

    #[must_use]
    pub const fn predecessor_release(self) -> DaemonReleaseId {
        self.predecessor_release
    }

    #[must_use]
    pub const fn supervisor_config(self) -> SupervisorConfigId {
        self.supervisor_config
    }

    #[must_use]
    pub const fn uds_endpoint(self) -> UdsEndpointId {
        self.uds_endpoint
    }
}

/// Exact admitted command transaction and current writer authority for one transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CutoverCommandBinding {
    workspace_id: WorkspaceId,
    operation_id: OperationId,
    transaction: TransactionRef,
    writer_fence: WriterFence,
    actor_id: ActorId,
    attempt: u32,
    command_digest: [u8; 32],
}

impl CutoverCommandBinding {
    /// Bind a cutover transition to a reducer-validated, durably prepared command attempt.
    ///
    /// The transaction is obtained from the unforgeable prepared-attempt token. An executing
    /// token without a validated `CommitPrepared`/reconciliation transaction is rejected.
    pub(crate) fn try_from_validated_attempt(
        attempt: ValidatedCommandAttempt,
    ) -> Result<Self, CutoverEventError> {
        let transaction = attempt
            .prepared_transaction()
            .ok_or(CutoverEventError::CommandNotPrepared)?;
        let command = attempt.command();
        let execution_owner = attempt.execution_owner();
        if attempt.attempt() == 0 {
            return Err(CutoverEventError::CommandAttemptInvalid);
        }
        let value =
            serde_json::to_value(command).map_err(|_| CutoverEventError::Canonicalization)?;
        let bytes = crate::contracts::jcs::canonicalize_value(&value)
            .map_err(|_| CutoverEventError::Canonicalization)?;
        let mut digest = blake3::Hasher::new();
        digest.update(b"codefabric.forward-cutover.validated-command.v1");
        digest.update(&(bytes.len() as u64).to_be_bytes());
        digest.update(&bytes);
        digest.update(&(attempt.attempt() as u64).to_be_bytes());
        digest.update(execution_owner.actor_id.as_bytes());
        digest.update(execution_owner.fence.lease_id.as_bytes());
        digest.update(&execution_owner.fence.generation.get().to_be_bytes());
        digest.update(transaction.as_bytes());
        Ok(Self {
            workspace_id: command.ownership.workspace_id,
            operation_id: command.identity.operation_id,
            transaction,
            writer_fence: execution_owner.fence,
            actor_id: execution_owner.actor_id,
            attempt: attempt.attempt(),
            command_digest: *digest.finalize().as_bytes(),
        })
    }

    #[must_use]
    pub const fn operation_id(self) -> OperationId {
        self.operation_id
    }

    #[must_use]
    pub const fn transaction(self) -> TransactionRef {
        self.transaction
    }

    #[must_use]
    pub const fn writer_fence(self) -> WriterFence {
        self.writer_fence
    }
}

/// Exact activation head derived from the reducer-validated, Delta-readback activation chain.
///
/// There is no raw-byte constructor in production. The cutover journal therefore records a
/// complete activation authority projection without becoming the authority that selected it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CutoverActivationAuthority {
    event_id: ActivationEventId,
    workspace_id: WorkspaceId,
    selected_epoch: EpochId,
    transaction: TransactionRef,
    backend_commit: BackendCommitRef,
    readback: ActivationReadbackRef,
    table_versions: TableVersionSetRef,
    proof_receipt: ProofReceiptRef,
    writer_fence: WriterFence,
}

impl CutoverActivationAuthority {
    /// Derive the sole current activation authority from an already validated activation chain.
    pub fn try_from_chain(chain: &ActivationChain) -> Result<Self, CutoverEventError> {
        let event = chain
            .head_event()
            .copied()
            .ok_or(CutoverEventError::ActivationAuthorityMissing)?;
        let pins = event.pins();
        if chain.current_head() != super::command::ExpectedHead::Epoch(pins.epoch) {
            return Err(CutoverEventError::ActivationIdentityMismatch);
        }
        let commit = event.commit();
        Ok(Self {
            event_id: event.event_id(),
            workspace_id: event.workspace_id(),
            selected_epoch: pins.epoch,
            transaction: commit.transaction,
            backend_commit: commit.backend_commit,
            readback: commit.readback,
            table_versions: pins.table_versions,
            proof_receipt: pins.proof_receipt,
            writer_fence: event.execution_fence(),
        })
    }

    #[must_use]
    pub const fn event_id(self) -> ActivationEventId {
        self.event_id
    }

    #[must_use]
    pub const fn selected_epoch(self) -> EpochId {
        self.selected_epoch
    }

    #[must_use]
    pub const fn writer_fence(self) -> WriterFence {
        self.writer_fence
    }

    #[must_use]
    pub const fn proof_receipt(self) -> ProofReceiptRef {
        self.proof_receipt
    }
}

/// Transition-specific evidence.  No variant can carry a process-local success receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CutoverTransitionEvidence {
    TargetProved {
        proof_receipt: ProofReceiptRef,
    },
    PredecessorFenced {
        supervisor: SupervisorObservation,
    },
    TargetServing {
        activation: CutoverActivationAuthority,
        supervisor: SupervisorObservation,
    },
    TargetMutating {
        activation: CutoverActivationAuthority,
        supervisor: SupervisorObservation,
    },
    Complete {
        activation: CutoverActivationAuthority,
        supervisor: SupervisorObservation,
    },
    /// Target-only convergence when the predecessor package/config/entrypoint is physically
    /// absent and live target UDS/writer/activation authority is independently observed.
    PhysicalZeroConverged {
        activation: CutoverActivationAuthority,
        supervisor: SupervisorObservation,
    },
}

impl CutoverTransitionEvidence {
    const fn phase(self) -> CutoverPhase {
        match self {
            Self::TargetProved { .. } => CutoverPhase::TargetProved,
            Self::PredecessorFenced { .. } => CutoverPhase::PredecessorFenced,
            Self::TargetServing { .. } => CutoverPhase::TargetServing,
            Self::TargetMutating { .. } => CutoverPhase::TargetMutating,
            Self::Complete { .. } | Self::PhysicalZeroConverged { .. } => CutoverPhase::Complete,
        }
    }

    const fn is_physical_zero_convergence(self) -> bool {
        matches!(self, Self::PhysicalZeroConverged { .. })
    }
}

/// One immutable event in the forward cutover chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CutoverEvent {
    event_id: CutoverEventId,
    identity: CutoverChainIdentity,
    sequence: u64,
    previous_event_id: Option<CutoverEventId>,
    phase: CutoverPhase,
    command: CutoverCommandBinding,
    proof_receipt: ProofReceiptRef,
    activation: Option<CutoverActivationAuthority>,
    supervisor: Option<SupervisorObservation>,
    physical_zero_convergence: bool,
}

impl CutoverEvent {
    /// Construct a transition only from the reducer-validated `CommitPrepared` command state.
    pub fn try_next_from_prepared_command(
        previous: Option<&Self>,
        identity: CutoverChainIdentity,
        record: &CommandRecord,
        owner: ExecutionOwner,
        transaction: TransactionRef,
        context: ReductionContext,
        evidence: CutoverTransitionEvidence,
    ) -> Result<Self, PreparedCutoverEventError> {
        let validated =
            prepared_attempt(record, owner, transaction, context, record.command().kind())?;
        let command = CutoverCommandBinding::try_from_validated_attempt(validated)?;
        Self::try_next(previous, identity, command, evidence).map_err(Into::into)
    }

    /// Construct and content-address the next transition after validating the full chain edge.
    pub fn try_next(
        previous: Option<&Self>,
        identity: CutoverChainIdentity,
        command: CutoverCommandBinding,
        evidence: CutoverTransitionEvidence,
    ) -> Result<Self, CutoverEventError> {
        let phase = evidence.phase();
        let physical_zero_convergence = evidence.is_physical_zero_convergence();
        validate_edge(
            previous.map(|event| event.phase),
            phase,
            physical_zero_convergence,
        )?;
        validate_command_binding(identity, command)?;
        if let Some(previous) = previous {
            if previous.identity != identity {
                return Err(CutoverEventError::ChainIdentityMismatch);
            }
            validate_fence_successor(previous.command.writer_fence, command.writer_fence)?;
        }

        let proof_receipt = match evidence {
            CutoverTransitionEvidence::TargetProved { proof_receipt } => proof_receipt,
            CutoverTransitionEvidence::PhysicalZeroConverged { activation, .. } => {
                activation.proof_receipt
            }
            _ => previous
                .map(|event| event.proof_receipt)
                .ok_or(CutoverEventError::MissingTargetProof)?,
        };
        let (activation, supervisor) = match evidence {
            CutoverTransitionEvidence::TargetProved { .. } => (None, None),
            CutoverTransitionEvidence::PredecessorFenced { supervisor } => {
                validate_supervisor(identity, phase, supervisor, None)?;
                (None, Some(supervisor))
            }
            CutoverTransitionEvidence::TargetServing {
                activation,
                supervisor,
            }
            | CutoverTransitionEvidence::TargetMutating {
                activation,
                supervisor,
            }
            | CutoverTransitionEvidence::Complete {
                activation,
                supervisor,
            } => {
                validate_activation_authority(identity, command, activation)?;
                if activation.proof_receipt != proof_receipt {
                    return Err(CutoverEventError::ActivationProofMismatch);
                }
                validate_supervisor(identity, phase, supervisor, Some(activation.selected_epoch))?;
                (Some(activation), Some(supervisor))
            }
            CutoverTransitionEvidence::PhysicalZeroConverged {
                activation,
                supervisor,
            } => {
                validate_activation_authority(identity, command, activation)?;
                if activation.proof_receipt != proof_receipt {
                    return Err(CutoverEventError::ActivationProofMismatch);
                }
                validate_supervisor(identity, phase, supervisor, Some(activation.selected_epoch))?;
                if supervisor.predecessor_package != ObservedAvailability::Absent
                    || supervisor.temporary_bridge != ObservedAvailability::Absent
                {
                    return Err(CutoverEventError::PhysicalZeroConvergenceRequired);
                }
                (Some(activation), Some(supervisor))
            }
        };
        let sequence = match previous {
            Some(event) => event
                .sequence
                .checked_add(1)
                .ok_or(CutoverEventError::SequenceExhausted)?,
            None => 1,
        };
        let mut event = Self {
            event_id: CutoverEventId::from_bytes([0; 32]),
            identity,
            sequence,
            previous_event_id: previous.map(|event| event.event_id),
            phase,
            command,
            proof_receipt,
            activation,
            supervisor,
            physical_zero_convergence,
        };
        event.event_id = event.derived_id()?;
        Ok(event)
    }

    #[must_use]
    pub const fn event_id(&self) -> CutoverEventId {
        self.event_id
    }

    #[must_use]
    pub const fn phase(&self) -> CutoverPhase {
        self.phase
    }

    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    #[must_use]
    pub const fn identity(&self) -> CutoverChainIdentity {
        self.identity
    }

    #[must_use]
    pub const fn command(&self) -> CutoverCommandBinding {
        self.command
    }

    fn derived_id(&self) -> Result<CutoverEventId, CutoverEventError> {
        let bytes = canonical_event_bytes(self)?;
        let mut digest = blake3::Hasher::new();
        digest.update(EVENT_DOMAIN);
        digest.update(&(bytes.len() as u64).to_be_bytes());
        digest.update(&bytes);
        Ok(CutoverEventId::from_bytes(*digest.finalize().as_bytes()))
    }
}

/// Failure to bind one forward-cutover transition to the durable command reducer.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum PreparedCutoverEventError {
    #[error(transparent)]
    Command(#[from] CommandPortError),
    #[error(transparent)]
    Event(#[from] CutoverEventError),
}

fn validate_activation_authority(
    identity: CutoverChainIdentity,
    command: CutoverCommandBinding,
    activation: CutoverActivationAuthority,
) -> Result<(), CutoverEventError> {
    if activation.workspace_id != identity.workspace_id {
        return Err(CutoverEventError::ActivationWorkspaceMismatch);
    }
    if activation.event_id.as_bytes() == &[0; 32]
        || activation.selected_epoch.as_bytes() == &[0; 16]
        || activation.transaction.as_bytes() == &[0; 32]
        || activation.backend_commit.as_bytes() == &[0; 32]
        || activation.readback.as_bytes() == &[0; 32]
        || activation.table_versions.as_bytes() == &[0; 32]
        || activation.proof_receipt.as_bytes() == &[0; 32]
        || activation.writer_fence.lease_id.as_bytes() == &[0; 16]
    {
        return Err(CutoverEventError::ActivationAuthorityMissing);
    }
    validate_fence_successor(activation.writer_fence, command.writer_fence)?;
    Ok(())
}

fn validate_command_binding(
    identity: CutoverChainIdentity,
    command: CutoverCommandBinding,
) -> Result<(), CutoverEventError> {
    if command.workspace_id != identity.workspace_id {
        return Err(CutoverEventError::CommandWorkspaceMismatch);
    }
    if command.operation_id.as_bytes() == &[0; 16]
        || command.transaction.as_bytes() == &[0; 32]
        || command.writer_fence.lease_id.as_bytes() == &[0; 16]
        || command.actor_id.as_bytes() == &[0; 16]
        || command.attempt == 0
        || command.command_digest == [0; 32]
    {
        return Err(CutoverEventError::CommandIdentityMissing);
    }
    Ok(())
}

fn validate_fence_successor(
    previous: WriterFence,
    next: WriterFence,
) -> Result<(), CutoverEventError> {
    if next.generation < previous.generation
        || (next.generation == previous.generation && next.lease_id != previous.lease_id)
    {
        Err(CutoverEventError::StaleWriterFence)
    } else {
        Ok(())
    }
}

fn validate_edge(
    previous: Option<CutoverPhase>,
    next: CutoverPhase,
    physical_zero_convergence: bool,
) -> Result<(), CutoverEventError> {
    if physical_zero_convergence {
        return if next == CutoverPhase::Complete
            && !matches!(previous, Some(CutoverPhase::Complete))
        {
            Ok(())
        } else {
            Err(CutoverEventError::InvalidTransition { previous, next })
        };
    }
    let valid = matches!(
        (previous, next),
        (None, CutoverPhase::TargetProved)
            | (
                Some(CutoverPhase::TargetProved),
                CutoverPhase::PredecessorFenced
            )
            | (
                Some(CutoverPhase::PredecessorFenced),
                CutoverPhase::TargetServing
            )
            | (
                Some(CutoverPhase::TargetServing),
                CutoverPhase::TargetMutating
            )
            | (Some(CutoverPhase::TargetMutating), CutoverPhase::Complete)
    );
    if valid {
        Ok(())
    } else {
        Err(CutoverEventError::InvalidTransition { previous, next })
    }
}

fn validate_supervisor(
    identity: CutoverChainIdentity,
    phase: CutoverPhase,
    observation: SupervisorObservation,
    selected_epoch: Option<EpochId>,
) -> Result<(), CutoverEventError> {
    let physical_predecessor_zero_state = observation.predecessor_package
        == ObservedAvailability::Absent
        && observation.temporary_bridge == ObservedAvailability::Absent;
    if observation.config_id != identity.supervisor_config
        || observation.uds_endpoint_id != identity.uds_endpoint
        || observation.host_identity != identity.deployment_host
        || observation.target_release != identity.target_release
        || observation.predecessor_release != identity.predecessor_release
        || (observation.previous_boot_id == observation.host_boot_id
            && !physical_predecessor_zero_state)
        || observation.observation_id != observation.derived_id()
        || observation.previous_boot_id.as_bytes() == &[0; 16]
        || observation.host_boot_id.as_bytes() == &[0; 16]
        || observation.reboot_observation.as_bytes() == &[0; 32]
        || observation.uds_observation.as_bytes() == &[0; 32]
        || observation.serving_observation.as_bytes() == &[0; 32]
        || observation.writer_observation.as_bytes() == &[0; 32]
        || observation
            .predecessor_revocation
            .is_some_and(|revocation| {
                revocation.bind_denial.as_bytes() == &[0; 32]
                    || revocation.serve_denial.as_bytes() == &[0; 32]
                    || revocation.writer_denial.as_bytes() == &[0; 32]
            })
    {
        return Err(CutoverEventError::SupervisorEvidenceMissing);
    }
    if observation.activation_head != observation.programmatic_epoch {
        return Err(CutoverEventError::ActivationIdentityMismatch);
    }
    let predecessor_denied = observation.predecessor_revocation.is_some();
    match phase {
        CutoverPhase::PredecessorFenced => {
            if !predecessor_denied
                || observation.uds_owner != AuthorityOwner::None
                || observation.serving_owner != AuthorityOwner::None
                || observation.writer_owner != AuthorityOwner::None
            {
                return Err(CutoverEventError::AuthorityCensusMismatch);
            }
        }
        CutoverPhase::TargetServing => {
            if !predecessor_denied
                || observation.uds_owner != AuthorityOwner::Target
                || observation.serving_owner != AuthorityOwner::Target
                || observation.writer_owner != AuthorityOwner::None
            {
                return Err(CutoverEventError::AuthorityCensusMismatch);
            }
        }
        CutoverPhase::TargetMutating | CutoverPhase::Complete => {
            if !predecessor_denied
                || observation.uds_owner != AuthorityOwner::Target
                || observation.serving_owner != AuthorityOwner::Target
                || observation.writer_owner != AuthorityOwner::Target
            {
                return Err(CutoverEventError::AuthorityCensusMismatch);
            }
            if phase == CutoverPhase::Complete
                && (observation.predecessor_package == ObservedAvailability::Present
                    || observation.temporary_bridge == ObservedAvailability::Present)
            {
                return Err(CutoverEventError::TemporaryAuthorityRemains);
            }
        }
        CutoverPhase::TargetProved => return Err(CutoverEventError::UnexpectedSupervisorEvidence),
    }
    if selected_epoch.is_some()
        && (observation.activation_head != selected_epoch
            || observation.programmatic_epoch != selected_epoch)
    {
        return Err(CutoverEventError::ActivationIdentityMismatch);
    }
    Ok(())
}

/// Stable state-machine validation failures.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CutoverEventError {
    #[error("target and predecessor release identities collide")]
    ReleaseIdentityCollision,
    #[error("private UDS endpoint identity requires one absolute authorized path")]
    InvalidUdsEndpoint,
    #[error("cutover deployment host identity is absent")]
    InvalidHostIdentity,
    #[error("cutover plan, workspace, release, supervisor, or UDS identity is absent")]
    InvalidChainIdentity,
    #[error("cutover event chain identity changed")]
    ChainIdentityMismatch,
    #[error("cutover command belongs to another workspace")]
    CommandWorkspaceMismatch,
    #[error("cutover transition requires a reducer-validated prepared transaction")]
    CommandNotPrepared,
    #[error("cutover command attempt is zero")]
    CommandAttemptInvalid,
    #[error("cutover validated command binding contains an absent identity")]
    CommandIdentityMissing,
    #[error("cutover transition is invalid: previous={previous:?} next={next:?}")]
    InvalidTransition {
        previous: Option<CutoverPhase>,
        next: CutoverPhase,
    },
    #[error("target-only convergence requires physical predecessor and bridge zero-state")]
    PhysicalZeroConvergenceRequired,
    #[error("cutover command uses a stale writer fence")]
    StaleWriterFence,
    #[error("cutover transition lacks the initial target proof")]
    MissingTargetProof,
    #[error(
        "actual deployment-supervisor/configuration/boot-or-physical-zero evidence is missing or mismatched"
    )]
    SupervisorEvidenceMissing,
    #[error("supervisor authority census does not prove exactly one permitted owner")]
    AuthorityCensusMismatch,
    #[error("activation head and reconstructed programmatic epoch differ")]
    ActivationIdentityMismatch,
    #[error("cutover activation authority is absent")]
    ActivationAuthorityMissing,
    #[error("cutover activation authority belongs to another workspace")]
    ActivationWorkspaceMismatch,
    #[error("cutover activation proof differs from the target proof")]
    ActivationProofMismatch,
    #[error("completion retains a predecessor package or temporary bridge authority")]
    TemporaryAuthorityRemains,
    #[error("target proof cannot carry supervisor serving authority")]
    UnexpectedSupervisorEvidence,
    #[error("cutover event sequence is exhausted")]
    SequenceExhausted,
    #[error("cutover event canonicalization failed")]
    Canonicalization,
    #[error("cutover event immutable identity differs")]
    IdentityMismatch,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredSupervisorObservation {
    observation_id: [u8; 32],
    config_id: [u8; 32],
    host_identity: [u8; 32],
    previous_boot_id: [u8; 16],
    host_boot_id: [u8; 16],
    reboot_observation: [u8; 32],
    target_release: [u8; 32],
    predecessor_release: [u8; 32],
    uds_endpoint_id: [u8; 32],
    uds_owner: AuthorityOwner,
    uds_observation: [u8; 32],
    serving_owner: AuthorityOwner,
    serving_observation: [u8; 32],
    writer_owner: AuthorityOwner,
    writer_observation: [u8; 32],
    activation_head: Option<[u8; 16]>,
    programmatic_epoch: Option<[u8; 16]>,
    predecessor_bind_denial: Option<[u8; 32]>,
    predecessor_serve_denial: Option<[u8; 32]>,
    predecessor_writer_denial: Option<[u8; 32]>,
    predecessor_package: ObservedAvailability,
    temporary_bridge: ObservedAvailability,
}

impl From<SupervisorObservation> for StoredSupervisorObservation {
    fn from(value: SupervisorObservation) -> Self {
        Self {
            observation_id: *value.observation_id.as_bytes(),
            config_id: *value.config_id.as_bytes(),
            host_identity: *value.host_identity.as_bytes(),
            previous_boot_id: *value.previous_boot_id.as_bytes(),
            host_boot_id: *value.host_boot_id.as_bytes(),
            reboot_observation: *value.reboot_observation.as_bytes(),
            target_release: *value.target_release.as_bytes(),
            predecessor_release: *value.predecessor_release.as_bytes(),
            uds_endpoint_id: *value.uds_endpoint_id.as_bytes(),
            uds_owner: value.uds_owner,
            uds_observation: *value.uds_observation.as_bytes(),
            serving_owner: value.serving_owner,
            serving_observation: *value.serving_observation.as_bytes(),
            writer_owner: value.writer_owner,
            writer_observation: *value.writer_observation.as_bytes(),
            activation_head: value.activation_head.map(|epoch| *epoch.as_bytes()),
            programmatic_epoch: value.programmatic_epoch.map(|epoch| *epoch.as_bytes()),
            predecessor_bind_denial: value
                .predecessor_revocation
                .map(|evidence| *evidence.bind_denial.as_bytes()),
            predecessor_serve_denial: value
                .predecessor_revocation
                .map(|evidence| *evidence.serve_denial.as_bytes()),
            predecessor_writer_denial: value
                .predecessor_revocation
                .map(|evidence| *evidence.writer_denial.as_bytes()),
            predecessor_package: value.predecessor_package,
            temporary_bridge: value.temporary_bridge,
        }
    }
}

impl From<StoredSupervisorObservation> for SupervisorObservation {
    fn from(value: StoredSupervisorObservation) -> Self {
        Self {
            observation_id: SupervisorObservationId::from_bytes(value.observation_id),
            config_id: SupervisorConfigId::from_bytes(value.config_id),
            host_identity: HostIdentity::from_bytes(value.host_identity),
            previous_boot_id: HostBootId::from_bytes(value.previous_boot_id),
            host_boot_id: HostBootId::from_bytes(value.host_boot_id),
            reboot_observation: SupervisorFactId::from_bytes(value.reboot_observation),
            target_release: DaemonReleaseId::from_bytes(value.target_release),
            predecessor_release: DaemonReleaseId::from_bytes(value.predecessor_release),
            uds_endpoint_id: UdsEndpointId::from_bytes(value.uds_endpoint_id),
            uds_owner: value.uds_owner,
            uds_observation: SupervisorFactId::from_bytes(value.uds_observation),
            serving_owner: value.serving_owner,
            serving_observation: SupervisorFactId::from_bytes(value.serving_observation),
            writer_owner: value.writer_owner,
            writer_observation: SupervisorFactId::from_bytes(value.writer_observation),
            activation_head: value.activation_head.map(EpochId::from_bytes),
            programmatic_epoch: value.programmatic_epoch.map(EpochId::from_bytes),
            predecessor_revocation: match (
                value.predecessor_bind_denial,
                value.predecessor_serve_denial,
                value.predecessor_writer_denial,
            ) {
                (Some(bind_denial), Some(serve_denial), Some(writer_denial)) => {
                    Some(PredecessorRevocationReadback {
                        bind_denial: SupervisorFactId::from_bytes(bind_denial),
                        serve_denial: SupervisorFactId::from_bytes(serve_denial),
                        writer_denial: SupervisorFactId::from_bytes(writer_denial),
                    })
                }
                _ => None,
            },
            predecessor_package: value.predecessor_package,
            temporary_bridge: value.temporary_bridge,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredCutoverActivationAuthority {
    event_id: [u8; 32],
    workspace_id: [u8; 16],
    selected_epoch: [u8; 16],
    transaction: [u8; 32],
    backend_commit: [u8; 32],
    readback: [u8; 32],
    table_versions: [u8; 32],
    proof_receipt: [u8; 32],
    lease_id: [u8; 16],
    writer_generation: u64,
}

impl From<CutoverActivationAuthority> for StoredCutoverActivationAuthority {
    fn from(value: CutoverActivationAuthority) -> Self {
        Self {
            event_id: *value.event_id.as_bytes(),
            workspace_id: *value.workspace_id.as_bytes(),
            selected_epoch: *value.selected_epoch.as_bytes(),
            transaction: *value.transaction.as_bytes(),
            backend_commit: *value.backend_commit.as_bytes(),
            readback: *value.readback.as_bytes(),
            table_versions: *value.table_versions.as_bytes(),
            proof_receipt: *value.proof_receipt.as_bytes(),
            lease_id: *value.writer_fence.lease_id.as_bytes(),
            writer_generation: value.writer_fence.generation.get(),
        }
    }
}

impl TryFrom<StoredCutoverActivationAuthority> for CutoverActivationAuthority {
    type Error = ForwardCutoverJournalError;

    fn try_from(value: StoredCutoverActivationAuthority) -> Result<Self, Self::Error> {
        let generation = WriterGeneration::new(value.writer_generation).ok_or_else(|| {
            ForwardCutoverJournalError::Corrupt(
                "zero activation-authority writer generation".into(),
            )
        })?;
        Ok(Self {
            event_id: ActivationEventId::from_bytes(value.event_id),
            workspace_id: WorkspaceId::from_bytes(value.workspace_id),
            selected_epoch: EpochId::from_bytes(value.selected_epoch),
            transaction: TransactionRef::from_bytes(value.transaction),
            backend_commit: BackendCommitRef::from_bytes(value.backend_commit),
            readback: ActivationReadbackRef::from_bytes(value.readback),
            table_versions: TableVersionSetRef::from_bytes(value.table_versions),
            proof_receipt: ProofReceiptRef::from_bytes(value.proof_receipt),
            writer_fence: WriterFence {
                lease_id: LeaseId::from_bytes(value.lease_id),
                generation,
            },
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredCutoverEvent {
    schema_version: u32,
    plan_id: [u8; 32],
    workspace_id: [u8; 16],
    deployment_host: [u8; 32],
    target_release: [u8; 32],
    predecessor_release: [u8; 32],
    supervisor_config: [u8; 32],
    uds_endpoint: [u8; 32],
    sequence: u64,
    previous_event_id: Option<[u8; 32]>,
    phase: CutoverPhase,
    command_workspace_id: [u8; 16],
    operation_id: [u8; 16],
    transaction_ref: [u8; 32],
    lease_id: [u8; 16],
    writer_generation: u64,
    command_actor_id: [u8; 16],
    command_attempt: u32,
    command_digest: [u8; 32],
    proof_receipt: [u8; 32],
    activation: Option<StoredCutoverActivationAuthority>,
    supervisor: Option<StoredSupervisorObservation>,
    physical_zero_convergence: bool,
}

impl From<&CutoverEvent> for StoredCutoverEvent {
    fn from(value: &CutoverEvent) -> Self {
        Self {
            schema_version: FORWARD_CUTOVER_SCHEMA_VERSION,
            plan_id: *value.identity.plan_id.as_bytes(),
            workspace_id: *value.identity.workspace_id.as_bytes(),
            deployment_host: *value.identity.deployment_host.as_bytes(),
            target_release: *value.identity.target_release.as_bytes(),
            predecessor_release: *value.identity.predecessor_release.as_bytes(),
            supervisor_config: *value.identity.supervisor_config.as_bytes(),
            uds_endpoint: *value.identity.uds_endpoint.as_bytes(),
            sequence: value.sequence,
            previous_event_id: value.previous_event_id.map(|identity| *identity.as_bytes()),
            phase: value.phase,
            command_workspace_id: *value.command.workspace_id.as_bytes(),
            operation_id: *value.command.operation_id.as_bytes(),
            transaction_ref: *value.command.transaction.as_bytes(),
            lease_id: *value.command.writer_fence.lease_id.as_bytes(),
            writer_generation: value.command.writer_fence.generation.get(),
            command_actor_id: *value.command.actor_id.as_bytes(),
            command_attempt: value.command.attempt,
            command_digest: value.command.command_digest,
            proof_receipt: *value.proof_receipt.as_bytes(),
            activation: value.activation.map(Into::into),
            supervisor: value.supervisor.map(Into::into),
            physical_zero_convergence: value.physical_zero_convergence,
        }
    }
}

fn canonical_event_bytes(event: &CutoverEvent) -> Result<Vec<u8>, CutoverEventError> {
    let value = serde_json::to_value(StoredCutoverEvent::from(event))
        .map_err(|_| CutoverEventError::Canonicalization)?;
    crate::contracts::jcs::canonicalize_value(&value)
        .map_err(|_| CutoverEventError::Canonicalization)
}

fn decode_event(
    bytes: &[u8],
    event_id: CutoverEventId,
) -> Result<CutoverEvent, ForwardCutoverJournalError> {
    let stored: StoredCutoverEvent = serde_json::from_slice(bytes)
        .map_err(|error| ForwardCutoverJournalError::Corrupt(error.to_string()))?;
    if stored.schema_version != FORWARD_CUTOVER_SCHEMA_VERSION {
        return Err(ForwardCutoverJournalError::UnsupportedSchema {
            observed: stored.schema_version,
            supported: FORWARD_CUTOVER_SCHEMA_VERSION,
        });
    }
    let generation = WriterGeneration::new(stored.writer_generation)
        .ok_or_else(|| ForwardCutoverJournalError::Corrupt("zero writer generation".into()))?;
    if stored.command_attempt == 0 {
        return Err(CutoverEventError::CommandAttemptInvalid.into());
    }
    let identity = CutoverChainIdentity::try_new(
        CutoverPlanId::from_bytes(stored.plan_id),
        WorkspaceId::from_bytes(stored.workspace_id),
        HostIdentity::from_bytes(stored.deployment_host),
        DaemonReleaseId::from_bytes(stored.target_release),
        DaemonReleaseId::from_bytes(stored.predecessor_release),
        SupervisorConfigId::from_bytes(stored.supervisor_config),
        UdsEndpointId::from_bytes(stored.uds_endpoint),
    )?;
    let event = CutoverEvent {
        event_id,
        identity,
        sequence: stored.sequence,
        previous_event_id: stored.previous_event_id.map(CutoverEventId::from_bytes),
        phase: stored.phase,
        command: CutoverCommandBinding {
            workspace_id: WorkspaceId::from_bytes(stored.command_workspace_id),
            operation_id: OperationId::from_bytes(stored.operation_id),
            transaction: TransactionRef::from_bytes(stored.transaction_ref),
            writer_fence: WriterFence {
                lease_id: LeaseId::from_bytes(stored.lease_id),
                generation,
            },
            actor_id: ActorId::from_bytes(stored.command_actor_id),
            attempt: stored.command_attempt,
            command_digest: stored.command_digest,
        },
        proof_receipt: ProofReceiptRef::from_bytes(stored.proof_receipt),
        activation: stored.activation.map(TryInto::try_into).transpose()?,
        supervisor: stored.supervisor.map(Into::into),
        physical_zero_convergence: stored.physical_zero_convergence,
    };
    if event.derived_id()? != event.event_id {
        return Err(CutoverEventError::IdentityMismatch.into());
    }
    Ok(event)
}

fn validate_loaded_edge(
    previous: Option<&CutoverEvent>,
    event: &CutoverEvent,
) -> Result<(), ForwardCutoverJournalError> {
    validate_edge(
        previous.map(CutoverEvent::phase),
        event.phase,
        event.physical_zero_convergence,
    )?;
    validate_command_binding(event.identity, event.command)?;
    let expected_sequence = match previous {
        Some(prior) => prior.sequence.checked_add(1).ok_or_else(|| {
            ForwardCutoverJournalError::Corrupt("cutover sequence exhausted".into())
        })?,
        None => 1,
    };
    if event.sequence != expected_sequence
        || event.previous_event_id != previous.map(CutoverEvent::event_id)
    {
        return Err(ForwardCutoverJournalError::Corrupt(
            "cutover sequence or previous event identity differs".into(),
        ));
    }
    if let Some(previous) = previous {
        if event.identity != previous.identity || event.proof_receipt != previous.proof_receipt {
            return Err(ForwardCutoverJournalError::Corrupt(
                "cutover chain identity or proof changed".into(),
            ));
        }
        validate_fence_successor(previous.command.writer_fence, event.command.writer_fence)?;
    }
    match event.phase {
        CutoverPhase::TargetProved => {
            if event.supervisor.is_some()
                || event.activation.is_some()
                || event.physical_zero_convergence
            {
                return Err(ForwardCutoverJournalError::Corrupt(
                    "target proof event contains later-phase authority".into(),
                ));
            }
        }
        CutoverPhase::PredecessorFenced => {
            if event.activation.is_some() || event.physical_zero_convergence {
                return Err(ForwardCutoverJournalError::Corrupt(
                    "predecessor fence event contains later target authority".into(),
                ));
            }
            validate_supervisor(
                event.identity,
                event.phase,
                event
                    .supervisor
                    .ok_or(CutoverEventError::SupervisorEvidenceMissing)?,
                None,
            )?;
        }
        CutoverPhase::TargetServing | CutoverPhase::TargetMutating | CutoverPhase::Complete => {
            let activation = event
                .activation
                .ok_or(CutoverEventError::ActivationAuthorityMissing)?;
            validate_activation_authority(event.identity, event.command, activation)?;
            if activation.proof_receipt != event.proof_receipt {
                return Err(CutoverEventError::ActivationProofMismatch.into());
            }
            validate_supervisor(
                event.identity,
                event.phase,
                event
                    .supervisor
                    .ok_or(CutoverEventError::SupervisorEvidenceMissing)?,
                Some(activation.selected_epoch),
            )?;
            if event.physical_zero_convergence {
                let supervisor = event
                    .supervisor
                    .ok_or(CutoverEventError::SupervisorEvidenceMissing)?;
                if event.phase != CutoverPhase::Complete
                    || supervisor.predecessor_package != ObservedAvailability::Absent
                    || supervisor.temporary_bridge != ObservedAvailability::Absent
                {
                    return Err(CutoverEventError::PhysicalZeroConvergenceRequired.into());
                }
            }
        }
    }
    Ok(())
}

/// Current state derived from the immutable event chain, never from a mutable status row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableCutoverState {
    NotStarted,
    At {
        phase: CutoverPhase,
        event_id: CutoverEventId,
        sequence: u64,
    },
}

/// Append outcome.  An exact duplicate converges without creating another event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CutoverAppendOutcome {
    Appended(DurableCutoverState),
    DuplicateConverged(DurableCutoverState),
}

/// Stable failures opening, reading, or appending the dedicated journal.
#[derive(Debug, Error)]
pub enum ForwardCutoverJournalError {
    #[error("forward-cutover journal parent is not a private owned directory: {0}")]
    UnsafeParent(PathBuf),
    #[error("forward-cutover journal is not a private owned regular file: {0}")]
    UnsafeDatabase(PathBuf),
    #[error("forward-cutover journal path has no file name: {0}")]
    InvalidPath(PathBuf),
    #[error("forward-cutover journal I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Event(#[from] CutoverEventError),
    #[error("unsupported forward-cutover schema {observed}; supported schema is {supported}")]
    UnsupportedSchema { observed: u32, supported: u32 },
    #[error("forward-cutover journal is corrupt: {0}")]
    Corrupt(String),
    #[error("forward-cutover command identity conflicts with an existing event")]
    CommandConflict,
    #[error("forward-cutover reconciliation evidence conflicts with durable history")]
    ReconciliationConflict,
    #[error("forward-cutover reconciliation has not durably resolved admission")]
    ReconciliationAdmissionClosed,
    #[error("forward-cutover reconciliation requires an administrative repair command")]
    AdministrativeRepairRequired,
    #[error("forward-cutover journal mutex is unavailable")]
    Unavailable,
}

/// One dedicated append-only SQLite journal.  It contains no mutable current-state table.
pub struct DurableForwardCutoverJournal {
    database_path: PathBuf,
    connection: Mutex<Connection>,
}

impl std::fmt::Debug for DurableForwardCutoverJournal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DurableForwardCutoverJournal")
            .field("database_path", &self.database_path)
            .finish_non_exhaustive()
    }
}

impl DurableForwardCutoverJournal {
    /// Open or initialize one exact-schema private cutover journal.
    pub fn open(path: &Path) -> Result<Self, ForwardCutoverJournalError> {
        let prepared = prepare_private_database_file(path)?;
        let prepared_metadata =
            prepared
                .metadata()
                .map_err(|source| ForwardCutoverJournalError::Io {
                    path: path.to_owned(),
                    source,
                })?;
        let mut connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        validate_same_private_file(path, &prepared_metadata)?;
        apply_pragmas(&connection)?;
        initialize_or_validate_schema(&mut connection)?;
        drop(prepared);
        Ok(Self {
            database_path: path.to_owned(),
            connection: Mutex::new(connection),
        })
    }

    /// Append one event iff it is the exact valid successor of the current durable chain tip.
    pub fn append(
        &self,
        event: &CutoverEvent,
    ) -> Result<CutoverAppendOutcome, ForwardCutoverJournalError> {
        let payload = canonical_event_bytes(event)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| ForwardCutoverJournalError::Unavailable)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let chain = load_chain(&transaction, event.identity.workspace_id)?;
        if let Some(existing) = chain.iter().find(|candidate| {
            candidate.command.operation_id == event.command.operation_id
                || candidate.command.transaction == event.command.transaction
        }) {
            if existing == event {
                let state = state_from_chain(&chain);
                transaction.commit()?;
                return Ok(CutoverAppendOutcome::DuplicateConverged(state));
            }
            return Err(ForwardCutoverJournalError::CommandConflict);
        }
        validate_append_reconciliation_authority(&transaction, event)?;
        validate_loaded_edge(chain.last(), event)?;
        let affected = transaction.execute(
            "INSERT INTO forward_cutover_event (
                 workspace_id, sequence, event_id, previous_event_id,
                 operation_id, transaction_ref, payload
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event.identity.workspace_id.as_bytes().as_slice(),
                i64::try_from(event.sequence)
                    .map_err(|_| ForwardCutoverJournalError::Corrupt("sequence overflow".into()))?,
                event.event_id.as_bytes().as_slice(),
                event
                    .previous_event_id
                    .map(|identity| identity.as_bytes().to_vec()),
                event.command.operation_id.as_bytes().as_slice(),
                event.command.transaction.as_bytes().as_slice(),
                payload,
            ],
        )?;
        if affected != 1 {
            return Err(ForwardCutoverJournalError::Corrupt(
                "event append affected an unexpected row count".into(),
            ));
        }
        transaction.commit()?;
        Ok(CutoverAppendOutcome::Appended(DurableCutoverState::At {
            phase: event.phase,
            event_id: event.event_id,
            sequence: event.sequence,
        }))
    }

    /// Reopen and derive the current state from the complete immutable event chain.
    pub fn current(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<DurableCutoverState, ForwardCutoverJournalError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| ForwardCutoverJournalError::Unavailable)?;
        let chain = load_chain(&connection, workspace_id)?;
        Ok(state_from_chain(&chain))
    }

    /// Return the exact durable chain for administrative status/reconciliation readers.
    pub fn events(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<CutoverEvent>, ForwardCutoverJournalError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| ForwardCutoverJournalError::Unavailable)?;
        load_chain(&connection, workspace_id)
    }

    /// Derive operator admission from the exact durable chain tip and a current actual-supervisor
    /// observation. Callers cannot substitute a process-local phase projection or epoch.
    pub fn operator_status(
        &self,
        identity: CutoverChainIdentity,
        current_supervisor: Option<SupervisorObservation>,
        current_activation: Option<CutoverActivationAuthority>,
    ) -> Result<CutoverOperatorStatus, ForwardCutoverJournalError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| ForwardCutoverJournalError::Unavailable)?;
        let chain = load_chain(&connection, identity.workspace_id)?;
        let reconciliation = workspace_reconciliation_posture(&connection, identity.workspace_id)?;
        Ok(derive_operator_status_from_tip(
            chain.last(),
            identity,
            current_supervisor,
            current_activation,
            reconciliation,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkspaceReconciliationPosture {
    Clear,
    Pending,
    RepairRequired,
}

fn workspace_reconciliation_posture(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<WorkspaceReconciliationPosture, ForwardCutoverJournalError> {
    let codes = connection
        .prepare(
            "SELECT current.resolution_code
             FROM forward_cutover_reconciliation AS current
             WHERE current.workspace_id = ?1
               AND current.sequence = (
                 SELECT MAX(candidate.sequence)
                 FROM forward_cutover_reconciliation AS candidate
                 WHERE candidate.workspace_id = current.workspace_id
                   AND candidate.operation_id = current.operation_id
                   AND candidate.transaction_ref = current.transaction_ref
                   AND candidate.intended_event_id = current.intended_event_id
               )",
        )?
        .query_map(params![workspace_id.as_bytes().as_slice()], |row| {
            row.get::<_, i64>(0)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if codes.iter().any(|code| *code == 4) {
        Ok(WorkspaceReconciliationPosture::RepairRequired)
    } else if codes.iter().any(|code| matches!(*code, 2 | 3)) {
        Ok(WorkspaceReconciliationPosture::Pending)
    } else if codes.iter().all(|code| *code == 1) {
        Ok(WorkspaceReconciliationPosture::Clear)
    } else {
        Err(ForwardCutoverJournalError::Corrupt(
            "reconciliation resolution census contains an invalid code".into(),
        ))
    }
}

fn validate_append_reconciliation_authority(
    connection: &Connection,
    event: &CutoverEvent,
) -> Result<(), ForwardCutoverJournalError> {
    let unresolved = connection
        .prepare(
            "SELECT current.operation_id, current.transaction_ref,
                    current.intended_event_id, current.resolution_code
             FROM forward_cutover_reconciliation AS current
             WHERE current.workspace_id = ?1
               AND current.sequence = (
                 SELECT MAX(candidate.sequence)
                 FROM forward_cutover_reconciliation AS candidate
                 WHERE candidate.workspace_id = current.workspace_id
                   AND candidate.operation_id = current.operation_id
                   AND candidate.transaction_ref = current.transaction_ref
                   AND candidate.intended_event_id = current.intended_event_id
               )
               AND current.resolution_code != 1",
        )?
        .query_map(
            params![event.identity.workspace_id.as_bytes().as_slice()],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    if unresolved.is_empty() {
        return Ok(());
    }
    if unresolved.iter().any(|row| row.3 == 4) {
        return Err(ForwardCutoverJournalError::AdministrativeRepairRequired);
    }
    if unresolved.as_slice()
        == [(
            event.command.operation_id.as_bytes().to_vec(),
            event.command.transaction.as_bytes().to_vec(),
            event.event_id.as_bytes().to_vec(),
            CutoverReconciliationOutcome::RetryAuthorized.code(),
        )]
    {
        Ok(())
    } else {
        Err(ForwardCutoverJournalError::ReconciliationAdmissionClosed)
    }
}

fn state_from_chain(chain: &[CutoverEvent]) -> DurableCutoverState {
    chain
        .last()
        .map_or(DurableCutoverState::NotStarted, |event| {
            DurableCutoverState::At {
                phase: event.phase,
                event_id: event.event_id,
                sequence: event.sequence,
            }
        })
}

fn load_chain(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<Vec<CutoverEvent>, ForwardCutoverJournalError> {
    let mut statement = connection.prepare(
        "SELECT sequence, event_id, previous_event_id, operation_id, transaction_ref, payload
         FROM forward_cutover_event
         WHERE workspace_id = ?1
         ORDER BY sequence",
    )?;
    let rows = statement
        .query_map(params![workspace_id.as_bytes().as_slice()], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Option<Vec<u8>>>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, Vec<u8>>(4)?,
                row.get::<_, Vec<u8>>(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut chain = Vec::with_capacity(rows.len());
    for (sequence, event_id, previous_event_id, operation_id, transaction_ref, payload) in rows {
        let event_id = CutoverEventId::from_bytes(array::<32>(&event_id, "event identity")?);
        let event = decode_event(&payload, event_id)?;
        if event.identity.workspace_id != workspace_id {
            return Err(ForwardCutoverJournalError::Corrupt(
                "payload workspace differs from indexed workspace".into(),
            ));
        }
        let indexed_sequence = u64::try_from(sequence).map_err(|_| {
            ForwardCutoverJournalError::Corrupt("indexed cutover sequence is invalid".into())
        })?;
        let indexed_previous = previous_event_id
            .as_deref()
            .map(|bytes| {
                array::<32>(bytes, "indexed previous event identity")
                    .map(CutoverEventId::from_bytes)
            })
            .transpose()?;
        if indexed_sequence != event.sequence
            || indexed_previous != event.previous_event_id
            || array::<16>(&operation_id, "indexed operation identity")?
                != *event.command.operation_id.as_bytes()
            || array::<32>(&transaction_ref, "indexed transaction identity")?
                != *event.command.transaction.as_bytes()
        {
            return Err(ForwardCutoverJournalError::Corrupt(
                "indexed cutover columns differ from the immutable payload".into(),
            ));
        }
        validate_loaded_edge(chain.last(), &event)?;
        chain.push(event);
    }
    Ok(chain)
}

/// Exact durable readback after an interrupted cutover append.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CutoverCommitObservation {
    EventCommitted(CutoverEventId),
    ProvedNotCommitted {
        command_absence: CutoverReadbackFactId,
        delta_marker_absence: CutoverReadbackFactId,
        supervisor_unchanged: CutoverReadbackFactId,
    },
    Indeterminate,
    Contradictory,
}

impl CutoverCommitObservation {
    const fn code(self) -> i64 {
        match self {
            Self::EventCommitted(_) => 1,
            Self::ProvedNotCommitted { .. } => 2,
            Self::Indeterminate => 3,
            Self::Contradictory => 4,
        }
    }

    const fn observed_event(self) -> Option<CutoverEventId> {
        match self {
            Self::EventCommitted(event) => Some(event),
            _ => None,
        }
    }

    const fn absence_facts(self) -> Option<[CutoverReadbackFactId; 3]> {
        match self {
            Self::ProvedNotCommitted {
                command_absence,
                delta_marker_absence,
                supervisor_unchanged,
            } => Some([command_absence, delta_marker_absence, supervisor_unchanged]),
            _ => None,
        }
    }
}

/// Immutable evidence binding one exact interrupted command to durable readback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CutoverReconciliationEvidence {
    evidence_id: CutoverReconciliationEvidenceId,
    workspace_id: WorkspaceId,
    operation_id: OperationId,
    transaction: TransactionRef,
    intended_event: CutoverEventId,
    observation: CutoverCommitObservation,
}

impl CutoverReconciliationEvidence {
    /// Construct content-addressed reconciliation evidence.
    ///
    /// `ProvedNotCommitted` is accepted only when command, Delta marker, and supervisor readback
    /// independently prove absence.  A timeout or one missing surface remains indeterminate.
    pub fn try_new(
        workspace_id: WorkspaceId,
        operation_id: OperationId,
        transaction: TransactionRef,
        intended_event: CutoverEventId,
        observation: CutoverCommitObservation,
    ) -> Result<Self, ForwardCutoverJournalError> {
        if matches!(observation, CutoverCommitObservation::ProvedNotCommitted {
            command_absence,
            delta_marker_absence,
            supervisor_unchanged,
        } if command_absence.as_bytes() == &[0; 32]
            || delta_marker_absence.as_bytes() == &[0; 32]
            || supervisor_unchanged.as_bytes() == &[0; 32])
        {
            return Err(ForwardCutoverJournalError::Corrupt(
                "not-committed evidence lacks command, Delta, or supervisor proof".into(),
            ));
        }
        let mut digest = blake3::Hasher::new();
        digest.update(b"codefabric.forward-cutover.reconciliation.v1");
        digest.update(workspace_id.as_bytes());
        digest.update(operation_id.as_bytes());
        digest.update(transaction.as_bytes());
        digest.update(intended_event.as_bytes());
        digest.update(&observation.code().to_be_bytes());
        if let Some(observed) = observation.observed_event() {
            digest.update(observed.as_bytes());
        }
        if let CutoverCommitObservation::ProvedNotCommitted {
            command_absence,
            delta_marker_absence,
            supervisor_unchanged,
        } = observation
        {
            digest.update(command_absence.as_bytes());
            digest.update(delta_marker_absence.as_bytes());
            digest.update(supervisor_unchanged.as_bytes());
        }
        Ok(Self {
            evidence_id: CutoverReconciliationEvidenceId::from_bytes(*digest.finalize().as_bytes()),
            workspace_id,
            operation_id,
            transaction,
            intended_event,
            observation,
        })
    }
}

/// Reconciliation never infers success from a receipt or timeout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CutoverReconciliationOutcome {
    Committed(CutoverEventId),
    RetryAuthorized,
    AdmissionClosed,
    AdministrativeRepairRequired,
}

impl CutoverReconciliationOutcome {
    const fn code(self) -> i64 {
        match self {
            Self::Committed(_) => 1,
            Self::RetryAuthorized => 2,
            Self::AdmissionClosed => 3,
            Self::AdministrativeRepairRequired => 4,
        }
    }
}

impl DurableForwardCutoverJournal {
    /// Append exact reconciliation evidence and derive the only safe next action.
    pub fn reconcile(
        &self,
        evidence: CutoverReconciliationEvidence,
    ) -> Result<CutoverReconciliationOutcome, ForwardCutoverJournalError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| ForwardCutoverJournalError::Unavailable)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing_evidence = transaction
            .query_row(
                "SELECT workspace_id, operation_id, transaction_ref, intended_event_id,
                        outcome_code, observed_event_id, command_readback_id,
                        delta_readback_id, supervisor_readback_id, resolution_code
                 FROM forward_cutover_reconciliation
                 WHERE evidence_id = ?1",
                params![evidence.evidence_id.as_bytes().as_slice()],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, Option<Vec<u8>>>(5)?,
                        row.get::<_, Option<Vec<u8>>>(6)?,
                        row.get::<_, Option<Vec<u8>>>(7)?,
                        row.get::<_, Option<Vec<u8>>>(8)?,
                        row.get::<_, i64>(9)?,
                    ))
                },
            )
            .optional()?;
        if let Some(existing) = existing_evidence {
            let resolution_code = existing.9;
            let absence = evidence.observation.absence_facts();
            if existing.0 != evidence.workspace_id.as_bytes()
                || existing.1 != evidence.operation_id.as_bytes()
                || existing.2 != evidence.transaction.as_bytes()
                || existing.3 != evidence.intended_event.as_bytes()
                || existing.4 != evidence.observation.code()
                || existing.5
                    != evidence
                        .observation
                        .observed_event()
                        .map(|event| event.as_bytes().to_vec())
                || existing.6 != absence.map(|facts| facts[0].as_bytes().to_vec())
                || existing.7 != absence.map(|facts| facts[1].as_bytes().to_vec())
                || existing.8 != absence.map(|facts| facts[2].as_bytes().to_vec())
            {
                return Err(ForwardCutoverJournalError::ReconciliationConflict);
            }
            return reconciliation_outcome_from_code(resolution_code, evidence.intended_event);
        }

        let candidate = derive_reconciliation_outcome(&transaction, evidence)?;
        let outcome = reconcile_progress(&transaction, evidence, candidate)?;
        let absence = evidence.observation.absence_facts();

        let sequence: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1
             FROM forward_cutover_reconciliation
             WHERE workspace_id = ?1",
            params![evidence.workspace_id.as_bytes().as_slice()],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO forward_cutover_reconciliation (
                 workspace_id, sequence, evidence_id, operation_id, transaction_ref,
                 intended_event_id, outcome_code, observed_event_id, command_readback_id,
                 delta_readback_id, supervisor_readback_id, resolution_code
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                evidence.workspace_id.as_bytes().as_slice(),
                sequence,
                evidence.evidence_id.as_bytes().as_slice(),
                evidence.operation_id.as_bytes().as_slice(),
                evidence.transaction.as_bytes().as_slice(),
                evidence.intended_event.as_bytes().as_slice(),
                evidence.observation.code(),
                evidence
                    .observation
                    .observed_event()
                    .map(|event| event.as_bytes().to_vec()),
                absence.map(|facts| facts[0].as_bytes().to_vec()),
                absence.map(|facts| facts[1].as_bytes().to_vec()),
                absence.map(|facts| facts[2].as_bytes().to_vec()),
                outcome.code(),
            ],
        )?;
        transaction.commit()?;
        Ok(outcome)
    }
}

fn derive_reconciliation_outcome(
    connection: &Connection,
    evidence: CutoverReconciliationEvidence,
) -> Result<CutoverReconciliationOutcome, ForwardCutoverJournalError> {
    let mut durable_statement = connection.prepare(
        "SELECT operation_id, transaction_ref, event_id
             FROM forward_cutover_event
             WHERE workspace_id = ?1 AND (operation_id = ?2 OR transaction_ref = ?3)",
    )?;
    let durable_rows = durable_statement
        .query_map(
            params![
                evidence.workspace_id.as_bytes().as_slice(),
                evidence.operation_id.as_bytes().as_slice(),
                evidence.transaction.as_bytes().as_slice(),
            ],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    let durable_event = match durable_rows.as_slice() {
        [] => None,
        [(operation, transaction, event)]
            if operation.as_slice() == evidence.operation_id.as_bytes()
                && transaction.as_slice() == evidence.transaction.as_bytes() =>
        {
            Some(CutoverEventId::from_bytes(array::<32>(
                event,
                "reconciled event identity",
            )?))
        }
        _ => return Ok(CutoverReconciliationOutcome::AdministrativeRepairRequired),
    };
    match (durable_event, evidence.observation) {
        (Some(event), CutoverCommitObservation::EventCommitted(observed))
            if event == evidence.intended_event && observed == event =>
        {
            Ok(CutoverReconciliationOutcome::Committed(event))
        }
        (Some(event), CutoverCommitObservation::Indeterminate)
            if event == evidence.intended_event =>
        {
            Ok(CutoverReconciliationOutcome::Committed(event))
        }
        (Some(_), _) | (None, CutoverCommitObservation::EventCommitted(_)) => {
            Ok(CutoverReconciliationOutcome::AdministrativeRepairRequired)
        }
        (None, CutoverCommitObservation::ProvedNotCommitted { .. }) => {
            Ok(CutoverReconciliationOutcome::RetryAuthorized)
        }
        (None, CutoverCommitObservation::Indeterminate) => {
            Ok(CutoverReconciliationOutcome::AdmissionClosed)
        }
        (None, CutoverCommitObservation::Contradictory) => {
            Ok(CutoverReconciliationOutcome::AdministrativeRepairRequired)
        }
    }
}

fn reconcile_progress(
    connection: &Connection,
    evidence: CutoverReconciliationEvidence,
    candidate: CutoverReconciliationOutcome,
) -> Result<CutoverReconciliationOutcome, ForwardCutoverJournalError> {
    let prior_codes = connection
        .prepare(
            "SELECT resolution_code
             FROM forward_cutover_reconciliation
             WHERE workspace_id = ?1 AND operation_id = ?2 AND transaction_ref = ?3
               AND intended_event_id = ?4
             ORDER BY sequence",
        )?
        .query_map(
            params![
                evidence.workspace_id.as_bytes().as_slice(),
                evidence.operation_id.as_bytes().as_slice(),
                evidence.transaction.as_bytes().as_slice(),
                evidence.intended_event.as_bytes().as_slice(),
            ],
            |row| row.get::<_, i64>(0),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    if prior_codes.contains(&CutoverReconciliationOutcome::AdministrativeRepairRequired.code()) {
        return Ok(CutoverReconciliationOutcome::AdministrativeRepairRequired);
    }
    let Some(previous) = prior_codes.last().copied() else {
        return Ok(candidate);
    };
    let previous = reconciliation_outcome_from_code(previous, evidence.intended_event)?;
    let allowed = matches!(
        (previous, candidate),
        (
            CutoverReconciliationOutcome::AdmissionClosed,
            CutoverReconciliationOutcome::AdmissionClosed
                | CutoverReconciliationOutcome::RetryAuthorized
                | CutoverReconciliationOutcome::Committed(_)
                | CutoverReconciliationOutcome::AdministrativeRepairRequired
        ) | (
            CutoverReconciliationOutcome::RetryAuthorized,
            CutoverReconciliationOutcome::RetryAuthorized
                | CutoverReconciliationOutcome::Committed(_)
                | CutoverReconciliationOutcome::AdministrativeRepairRequired
        ) | (
            CutoverReconciliationOutcome::Committed(_),
            CutoverReconciliationOutcome::Committed(_)
        )
    );
    if allowed {
        Ok(candidate)
    } else {
        Ok(CutoverReconciliationOutcome::AdministrativeRepairRequired)
    }
}

fn reconciliation_outcome_from_code(
    code: i64,
    intended_event: CutoverEventId,
) -> Result<CutoverReconciliationOutcome, ForwardCutoverJournalError> {
    match code {
        1 => Ok(CutoverReconciliationOutcome::Committed(intended_event)),
        2 => Ok(CutoverReconciliationOutcome::RetryAuthorized),
        3 => Ok(CutoverReconciliationOutcome::AdmissionClosed),
        4 => Ok(CutoverReconciliationOutcome::AdministrativeRepairRequired),
        observed => Err(ForwardCutoverJournalError::Corrupt(format!(
            "invalid reconciliation resolution code {observed}"
        ))),
    }
}

/// Admission posture rendered for operators from durable events plus actual observed deployment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CutoverAdmission {
    Closed,
    TargetReadOnly,
    TargetReadWrite,
}

/// Operator-readable status is a projection and cannot author a transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CutoverOperatorStatus {
    pub durable_state: DurableCutoverState,
    pub admission: CutoverAdmission,
    pub code: &'static str,
    pub remediation: &'static str,
}

/// Derive operator status from an already-validated durable event tip and a current supervisor
/// observation. Production callers enter through [`DurableForwardCutoverJournal::operator_status`].
fn derive_operator_status_from_tip(
    tip: Option<&CutoverEvent>,
    identity: CutoverChainIdentity,
    current_supervisor: Option<SupervisorObservation>,
    current_activation: Option<CutoverActivationAuthority>,
    reconciliation: WorkspaceReconciliationPosture,
) -> CutoverOperatorStatus {
    let state = tip.map_or(DurableCutoverState::NotStarted, |event| {
        DurableCutoverState::At {
            phase: event.phase,
            event_id: event.event_id,
            sequence: event.sequence,
        }
    });
    if tip.is_some_and(|event| event.identity != identity) {
        return CutoverOperatorStatus {
            durable_state: state,
            admission: CutoverAdmission::Closed,
            code: "CUTOVER_DURABLE_CHAIN_IDENTITY_MISMATCH",
            remediation: "open the journal with the exact workspace, plan, release, supervisor, and UDS identities",
        };
    }
    match reconciliation {
        WorkspaceReconciliationPosture::RepairRequired => {
            return CutoverOperatorStatus {
                durable_state: state,
                admission: CutoverAdmission::Closed,
                code: "CUTOVER_RECONCILIATION_REPAIR_REQUIRED",
                remediation: "execute an authorized administrative repair after reconciling command, Delta activation, and supervisor history",
            };
        }
        WorkspaceReconciliationPosture::Pending => {
            return CutoverOperatorStatus {
                durable_state: state,
                admission: CutoverAdmission::Closed,
                code: "CUTOVER_RECONCILIATION_PENDING",
                remediation: "append exact durable command, Delta activation, and supervisor readback before reopening admission",
            };
        }
        WorkspaceReconciliationPosture::Clear => {}
    }
    let Some(observation) = current_supervisor else {
        return CutoverOperatorStatus {
            durable_state: state,
            admission: CutoverAdmission::Closed,
            code: "CUTOVER_DEPLOYMENT_SUPERVISOR_OBSERVATION_REQUIRED",
            remediation: "install and observe the actual deployment supervisor/configuration, including boot or exact physical-zero bind/serve/write evidence",
        };
    };
    if observation.config_id != identity.supervisor_config
        || observation.uds_endpoint_id != identity.uds_endpoint
        || observation.host_identity != identity.deployment_host
        || observation.target_release != identity.target_release
        || observation.predecessor_release != identity.predecessor_release
        || observation.observation_id != observation.derived_id()
    {
        return CutoverOperatorStatus {
            durable_state: state,
            admission: CutoverAdmission::Closed,
            code: "CUTOVER_DEPLOYMENT_SUPERVISOR_OBSERVATION_MISMATCH",
            remediation: "reconcile the exact supervisor configuration, UDS endpoint, and current host-boot observation",
        };
    }
    match tip {
        Some(event)
            if event.phase == CutoverPhase::TargetServing
                && event.activation == current_activation
                && event.activation.is_some_and(|activation| {
                    validate_supervisor(
                        identity,
                        event.phase,
                        observation,
                        Some(activation.selected_epoch),
                    )
                    .is_ok()
                }) =>
        {
            CutoverOperatorStatus {
                durable_state: state,
                admission: CutoverAdmission::TargetReadOnly,
                code: "CUTOVER_TARGET_SERVING_READ_ONLY",
                remediation: "execute the exact target-mutation command before admitting writes",
            }
        }
        Some(event)
            if matches!(
                event.phase,
                CutoverPhase::TargetMutating | CutoverPhase::Complete
            ) && event.activation == current_activation
                && event.activation.is_some_and(|activation| {
                    validate_supervisor(
                        identity,
                        event.phase,
                        observation,
                        Some(activation.selected_epoch),
                    )
                    .is_ok()
                }) =>
        {
            CutoverOperatorStatus {
                durable_state: state,
                admission: CutoverAdmission::TargetReadWrite,
                code: "CUTOVER_TARGET_AUTHORITY_OBSERVED",
                remediation: "none",
            }
        }
        _ => CutoverOperatorStatus {
            durable_state: state,
            admission: CutoverAdmission::Closed,
            code: "CUTOVER_DURABLE_AND_DEPLOYMENT_STATE_DISAGREE",
            remediation: "run the administrative reconciliation command; never infer authority from a socket or process receipt",
        },
    }
}

fn array<const N: usize>(
    bytes: &[u8],
    context: &str,
) -> Result<[u8; N], ForwardCutoverJournalError> {
    bytes.try_into().map_err(|_| {
        ForwardCutoverJournalError::Corrupt(format!(
            "{context} has width {}, expected {N}",
            bytes.len()
        ))
    })
}

fn prepare_private_database_file(path: &Path) -> Result<File, ForwardCutoverJournalError> {
    let parent = path
        .parent()
        .filter(|candidate| !candidate.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let metadata = fs::symlink_metadata(parent)
        .map_err(|_| ForwardCutoverJournalError::UnsafeParent(parent.to_owned()))?;
    let owner = rustix::process::geteuid().as_raw();
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != owner
        || metadata.permissions().mode() & 0o777 != 0o700
    {
        return Err(ForwardCutoverJournalError::UnsafeParent(parent.to_owned()));
    }
    let file_name = path
        .file_name()
        .ok_or_else(|| ForwardCutoverJournalError::InvalidPath(path.to_owned()))?;
    let directory = open(
        parent,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY,
        Mode::empty(),
    )
    .map_err(|_| ForwardCutoverJournalError::UnsafeParent(parent.to_owned()))?;
    let descriptor = openat(
        &directory,
        file_name,
        OFlags::RDWR | OFlags::CREATE | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|_| ForwardCutoverJournalError::UnsafeDatabase(path.to_owned()))?;
    let file = File::from(descriptor);
    let metadata = file
        .metadata()
        .map_err(|source| ForwardCutoverJournalError::Io {
            path: path.to_owned(),
            source,
        })?;
    if !private_database_metadata(&metadata, owner) {
        return Err(ForwardCutoverJournalError::UnsafeDatabase(path.to_owned()));
    }
    file.sync_all()
        .map_err(|source| ForwardCutoverJournalError::Io {
            path: path.to_owned(),
            source,
        })?;
    Ok(file)
}

fn validate_same_private_file(
    path: &Path,
    prepared: &fs::Metadata,
) -> Result<(), ForwardCutoverJournalError> {
    let observed = fs::symlink_metadata(path)
        .map_err(|_| ForwardCutoverJournalError::UnsafeDatabase(path.to_owned()))?;
    if !private_database_metadata(&observed, rustix::process::geteuid().as_raw())
        || observed.dev() != prepared.dev()
        || observed.ino() != prepared.ino()
    {
        return Err(ForwardCutoverJournalError::UnsafeDatabase(path.to_owned()));
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

fn apply_pragmas(connection: &Connection) -> Result<(), ForwardCutoverJournalError> {
    connection.busy_timeout(BUSY_TIMEOUT)?;
    let journal_mode: String =
        connection.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
    if journal_mode != "wal" {
        return Err(ForwardCutoverJournalError::Corrupt(format!(
            "journal_mode is {journal_mode}, expected wal"
        )));
    }
    connection.execute_batch(
        "PRAGMA synchronous=FULL;
         PRAGMA foreign_keys=ON;
         PRAGMA trusted_schema=OFF;
         PRAGMA secure_delete=FAST;
         PRAGMA wal_autocheckpoint=1000;",
    )?;
    Ok(())
}

fn initialize_or_validate_schema(
    connection: &mut Connection,
) -> Result<(), ForwardCutoverJournalError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: u32 = transaction.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let application: u32 = transaction.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    match version {
        0 => {
            if application != 0 || !user_schema_objects(&transaction)?.is_empty() {
                return Err(ForwardCutoverJournalError::Corrupt(
                    "unversioned journal already contains application objects".into(),
                ));
            }
            transaction.execute_batch(SCHEMA_V1)?;
        }
        FORWARD_CUTOVER_SCHEMA_VERSION if application == APPLICATION_ID => {}
        FORWARD_CUTOVER_SCHEMA_VERSION => {
            return Err(ForwardCutoverJournalError::Corrupt(format!(
                "application identity is {application}, expected {APPLICATION_ID}"
            )));
        }
        observed => {
            return Err(ForwardCutoverJournalError::UnsupportedSchema {
                observed,
                supported: FORWARD_CUTOVER_SCHEMA_VERSION,
            });
        }
    }
    validate_schema(&transaction)?;
    transaction.commit()?;
    Ok(())
}

fn validate_schema(connection: &Connection) -> Result<(), ForwardCutoverJournalError> {
    let version: u32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let application: u32 = connection.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    if version != FORWARD_CUTOVER_SCHEMA_VERSION || application != APPLICATION_ID {
        return Err(ForwardCutoverJournalError::Corrupt(
            "version or application identity changed during schema validation".into(),
        ));
    }
    let objects = user_schema_objects(connection)?;
    let expected = vec![
        ("table".to_owned(), EVENT_TABLE.to_owned()),
        ("table".to_owned(), RECONCILIATION_TABLE.to_owned()),
    ];
    if objects != expected {
        return Err(ForwardCutoverJournalError::Corrupt(format!(
            "journal object census differs: {objects:?}"
        )));
    }

    validate_table_schema(
        connection,
        EVENT_TABLE,
        &[
            (0, "workspace_id", "BLOB", 1, 1),
            (1, "sequence", "INTEGER", 1, 2),
            (2, "event_id", "BLOB", 1, 0),
            (3, "previous_event_id", "BLOB", 0, 0),
            (4, "operation_id", "BLOB", 1, 0),
            (5, "transaction_ref", "BLOB", 1, 0),
            (6, "payload", "BLOB", 1, 0),
        ],
        &[
            "length(workspace_id) = 16",
            "sequence > 0",
            "length(event_id) = 32",
            "length(previous_event_id) = 32",
            "length(operation_id) = 16",
            "length(transaction_ref) = 32",
            "length(payload) > 0",
            "UNIQUE (workspace_id, operation_id)",
            "UNIQUE (workspace_id, transaction_ref)",
            "WITHOUT ROWID",
            "STRICT",
        ],
    )?;
    validate_table_schema(
        connection,
        RECONCILIATION_TABLE,
        &[
            (0, "workspace_id", "BLOB", 1, 1),
            (1, "sequence", "INTEGER", 1, 2),
            (2, "evidence_id", "BLOB", 1, 0),
            (3, "operation_id", "BLOB", 1, 0),
            (4, "transaction_ref", "BLOB", 1, 0),
            (5, "intended_event_id", "BLOB", 1, 0),
            (6, "outcome_code", "INTEGER", 1, 0),
            (7, "observed_event_id", "BLOB", 0, 0),
            (8, "command_readback_id", "BLOB", 0, 0),
            (9, "delta_readback_id", "BLOB", 0, 0),
            (10, "supervisor_readback_id", "BLOB", 0, 0),
            (11, "resolution_code", "INTEGER", 1, 0),
        ],
        &[
            "length(workspace_id) = 16",
            "sequence > 0",
            "length(evidence_id) = 32",
            "length(operation_id) = 16",
            "length(transaction_ref) = 32",
            "length(intended_event_id) = 32",
            "outcome_code BETWEEN 1 AND 4",
            "length(observed_event_id) = 32",
            "length(command_readback_id) = 32",
            "length(delta_readback_id) = 32",
            "length(supervisor_readback_id) = 32",
            "resolution_code BETWEEN 1 AND 4",
            "outcome_code = 2 AND observed_event_id IS NULL",
            "WITHOUT ROWID",
            "STRICT",
        ],
    )?;
    Ok(())
}

fn validate_table_schema(
    connection: &Connection,
    table: &'static str,
    expected_columns: &[(i64, &'static str, &'static str, i64, i64)],
    required_sql: &[&'static str],
) -> Result<(), ForwardCutoverJournalError> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let expected = expected_columns
        .iter()
        .map(|(cid, name, data_type, not_null, primary_key)| {
            (
                *cid,
                (*name).to_owned(),
                (*data_type).to_owned(),
                *not_null,
                *primary_key,
            )
        })
        .collect::<Vec<_>>();
    if columns != expected {
        return Err(ForwardCutoverJournalError::Corrupt(format!(
            "{table} column census differs: {columns:?}"
        )));
    }
    let table_sql: String = connection.query_row(
        "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
        [table],
        |row| row.get(0),
    )?;
    for required in required_sql {
        if !table_sql.contains(required) {
            return Err(ForwardCutoverJournalError::Corrupt(format!(
                "{table} is missing required constraint {required}"
            )));
        }
    }
    Ok(())
}

fn user_schema_objects(connection: &Connection) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let mut statement = connection.prepare(
        "SELECT type, name
         FROM sqlite_schema
         WHERE name NOT LIKE 'sqlite_%'
         ORDER BY type, name",
    )?;
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    use std::os::unix::net::UnixListener;

    use tempfile::TempDir;

    use super::*;
    use crate::fabric::activation::{
        ActivationAttempt, ActivationCommit, ActivationEvent, ActivationOrdinal,
        CompatibilityClassRef, FabricEpochPins, OverlaySegmentSetRef, PolicySetRef,
    };
    use crate::fabric::command::{
        AdministrationAction, AdministrationRequestRef, AdmissionContext, AdmissionOutcome,
        ApplicationReleaseRef, AuthorizationDecision, AuthorizationRef, CommandEvent,
        CommandIdentity, CommandOwnership, CommandPins, CommandReducer, ExpectedHead,
        FabricCommand, FabricCommandPayload, IdempotencyKey, InputReleaseRef, PrincipalId,
        ProgramReleaseRef, ProviderReleaseRef, ProviderSetRef, ResourceEnvelopeRef,
        RetentionPolicyRef, SourceAuthorityRef, SourceGeneration,
    };
    use crate::forward_cutover_controller::{
        FORWARD_CUTOVER_DEPLOYMENT_FILE, FORWARD_CUTOVER_JOURNAL_FILE,
        ProductionCutoverAdvanceOutcome, ProductionCutoverEvidence,
        ProductionForwardCutoverController,
    };

    fn bytes16(marker: u8) -> [u8; 16] {
        [marker; 16]
    }

    fn bytes32(marker: u8) -> [u8; 32] {
        [marker; 32]
    }

    fn readback_fact(marker: u8) -> CutoverReadbackFactId {
        CutoverReadbackFactId::from_bytes(bytes32(marker))
    }

    fn workspace(marker: u8) -> WorkspaceId {
        WorkspaceId::from_bytes(bytes16(marker))
    }

    fn identity(marker: u8) -> CutoverChainIdentity {
        let workspace_id = workspace(marker);
        CutoverChainIdentity::try_new(
            CutoverPlanId::from_bytes(bytes32(marker)),
            workspace_id,
            HostIdentity::from_bytes(bytes32(0x31)),
            DaemonReleaseId::from_bytes(bytes32(0x91)),
            DaemonReleaseId::from_bytes(bytes32(0x41)),
            SupervisorConfigId::from_bytes(bytes32(0x51)),
            UdsEndpointId::derive(
                workspace_id,
                Path::new("/run/user/1000/codefabric/query.sock"),
            )
            .expect("absolute UDS identity"),
        )
        .expect("distinct release identities")
    }

    fn command_for_workspace(
        workspace_id: WorkspaceId,
        marker: u8,
        generation: u64,
    ) -> CutoverCommandBinding {
        let writer_fence = WriterFence {
            lease_id: LeaseId::from_bytes(bytes16(0x61)),
            generation: WriterGeneration::new(generation).expect("nonzero generation"),
        };
        let transaction = TransactionRef::from_bytes(bytes32(marker));
        let command = FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes(bytes16(marker)),
                idempotency_key: IdempotencyKey::from_bytes(bytes32(marker.wrapping_add(0x40))),
            },
            ownership: CommandOwnership {
                workspace_id,
                principal_id: PrincipalId::from_bytes(bytes16(0x21)),
                authorization: AuthorizationRef::from_bytes(bytes32(0x22)),
            },
            expected_head: ExpectedHead::Empty,
            writer_fence,
            pins: command_pins(),
            resources: ResourceEnvelopeRef::from_bytes(bytes32(0x2a)),
            payload: FabricCommandPayload::Administer {
                action: AdministrationAction::ReconcileOperation,
                request: AdministrationRequestRef::from_bytes(bytes32(marker)),
            },
        };
        let validated = ValidatedCommandAttempt::for_test_prepared(
            command,
            1,
            ExecutionOwner {
                actor_id: ActorId::from_bytes(bytes16(0x23)),
                fence: writer_fence,
            },
            transaction,
        );
        CutoverCommandBinding::try_from_validated_attempt(validated)
            .expect("prepared command binding")
    }

    fn command_pins() -> CommandPins {
        CommandPins {
            input_release: InputReleaseRef::from_bytes(bytes32(0x24)),
            program_release: ProgramReleaseRef::from_bytes(bytes32(0x25)),
            application_release: ApplicationReleaseRef::from_bytes(bytes32(0x26)),
            source_authority: SourceAuthorityRef::from_bytes(bytes32(0x27)),
            source_generation: SourceGeneration::new(7),
            provider_release: ProviderReleaseRef::from_bytes(bytes32(0x28)),
            provider_set: ProviderSetRef::from_bytes(bytes32(0x29)),
        }
    }

    fn prepared_record(
        workspace_id: WorkspaceId,
        marker: u8,
        generation: u64,
    ) -> (
        CommandRecord,
        ExecutionOwner,
        TransactionRef,
        ReductionContext,
    ) {
        let writer_fence = WriterFence {
            lease_id: LeaseId::from_bytes(bytes16(0x61)),
            generation: WriterGeneration::new(generation).unwrap(),
        };
        let transaction = TransactionRef::from_bytes(bytes32(marker));
        let command = FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes(bytes16(marker)),
                idempotency_key: IdempotencyKey::from_bytes(bytes32(marker.wrapping_add(0x40))),
            },
            ownership: CommandOwnership {
                workspace_id,
                principal_id: PrincipalId::from_bytes(bytes16(0x21)),
                authorization: AuthorizationRef::from_bytes(bytes32(0x22)),
            },
            expected_head: ExpectedHead::Empty,
            writer_fence,
            pins: command_pins(),
            resources: ResourceEnvelopeRef::from_bytes(bytes32(0x2a)),
            payload: FabricCommandPayload::Administer {
                action: AdministrationAction::ReconcileOperation,
                request: AdministrationRequestRef::from_bytes(bytes32(marker)),
            },
        };
        let owner = ExecutionOwner {
            actor_id: ActorId::from_bytes(bytes16(0x23)),
            fence: writer_fence,
        };
        let admission = CommandReducer::admit(
            None,
            &command,
            AdmissionContext {
                workspace_id,
                current_head: command.expected_head,
                active_fence: writer_fence,
                authorization: AuthorizationDecision::Authorized(command.ownership.authorization),
            },
        )
        .expect("admit cutover command");
        let AdmissionOutcome::New(admitted) = admission else {
            panic!("fresh cutover command must be new")
        };
        let context = ReductionContext {
            current_head: command.expected_head,
            active_fence: writer_fence,
        };
        let executing = CommandReducer::reduce(&admitted, CommandEvent::Start { owner }, context)
            .expect("start cutover command")
            .record;
        let prepared = CommandReducer::reduce(
            &executing,
            CommandEvent::PrepareCommit { owner, transaction },
            context,
        )
        .expect("prepare cutover command")
        .record;
        (prepared, owner, transaction, context)
    }

    fn prepared_physical_zero_command(
        evidence: &ProductionCutoverEvidence,
        marker: u8,
    ) -> (
        CommandRecord,
        ExecutionOwner,
        TransactionRef,
        ReductionContext,
    ) {
        let writer_fence = evidence.current_writer_fence;
        let expected_head = ExpectedHead::Epoch(evidence.activation.selected_epoch());
        let transaction = TransactionRef::from_bytes(bytes32(marker));
        let command = FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes(bytes16(marker)),
                idempotency_key: IdempotencyKey::from_bytes(bytes32(marker.wrapping_add(0x40))),
            },
            ownership: CommandOwnership {
                workspace_id: evidence.identity.workspace_id(),
                principal_id: PrincipalId::from_bytes(bytes16(0x21)),
                authorization: AuthorizationRef::from_bytes(bytes32(0x22)),
            },
            expected_head,
            writer_fence,
            pins: command_pins(),
            resources: ResourceEnvelopeRef::from_bytes(bytes32(0x2a)),
            payload: FabricCommandPayload::Administer {
                action: AdministrationAction::ReconcileOperation,
                request: AdministrationRequestRef::from_bytes(
                    *evidence.identity.plan_id().as_bytes(),
                ),
            },
        };
        let owner = ExecutionOwner {
            actor_id: ActorId::from_bytes(bytes16(0x23)),
            fence: writer_fence,
        };
        let admission = CommandReducer::admit(
            None,
            &command,
            AdmissionContext {
                workspace_id: evidence.identity.workspace_id(),
                current_head: expected_head,
                active_fence: writer_fence,
                authorization: AuthorizationDecision::Authorized(command.ownership.authorization),
            },
        )
        .expect("admit physical-zero convergence command");
        let AdmissionOutcome::New(admitted) = admission else {
            panic!("fresh convergence command must be new")
        };
        let context = ReductionContext {
            current_head: expected_head,
            active_fence: writer_fence,
        };
        let executing = CommandReducer::reduce(&admitted, CommandEvent::Start { owner }, context)
            .expect("start convergence command")
            .record;
        let prepared = CommandReducer::reduce(
            &executing,
            CommandEvent::PrepareCommit { owner, transaction },
            context,
        )
        .expect("prepare convergence command")
        .record;
        (prepared, owner, transaction, context)
    }

    fn supervisor(
        identity: CutoverChainIdentity,
        phase: CutoverPhase,
        epoch: Option<EpochId>,
    ) -> SupervisorObservation {
        supervisor_on_boot(identity, phase, epoch, 0x70, 0x71)
    }

    fn supervisor_on_boot(
        identity: CutoverChainIdentity,
        phase: CutoverPhase,
        epoch: Option<EpochId>,
        previous_boot: u8,
        current_boot: u8,
    ) -> SupervisorObservation {
        let (uds_owner, serving_owner, writer_owner, predecessor_denied, package, bridge) =
            match phase {
                CutoverPhase::PredecessorFenced => (
                    AuthorityOwner::None,
                    AuthorityOwner::None,
                    AuthorityOwner::None,
                    true,
                    ObservedAvailability::Present,
                    ObservedAvailability::Present,
                ),
                CutoverPhase::TargetServing => (
                    AuthorityOwner::Target,
                    AuthorityOwner::Target,
                    AuthorityOwner::None,
                    true,
                    ObservedAvailability::Present,
                    ObservedAvailability::Present,
                ),
                CutoverPhase::TargetMutating => (
                    AuthorityOwner::Target,
                    AuthorityOwner::Target,
                    AuthorityOwner::Target,
                    true,
                    ObservedAvailability::Present,
                    ObservedAvailability::Present,
                ),
                CutoverPhase::Complete => (
                    AuthorityOwner::Target,
                    AuthorityOwner::Target,
                    AuthorityOwner::Target,
                    true,
                    ObservedAvailability::Absent,
                    ObservedAvailability::Absent,
                ),
                CutoverPhase::TargetProved => panic!("target proof has no supervisor observation"),
            };
        SupervisorObservation::try_from_actual_readback(
            identity,
            ActualSupervisorReadback {
                config_id: identity.supervisor_config,
                host_identity: identity.deployment_host,
                previous_boot_id: HostBootId::from_bytes(bytes16(previous_boot)),
                current_boot_id: HostBootId::from_bytes(bytes16(current_boot)),
                reboot_observation: SupervisorFactId::from_bytes(bytes32(0x72)),
                target_release: identity.target_release,
                predecessor_release: identity.predecessor_release,
                uds_endpoint_id: identity.uds_endpoint,
                uds_owner,
                uds_observation: SupervisorFactId::from_bytes(bytes32(0x73)),
                serving_owner,
                serving_observation: SupervisorFactId::from_bytes(bytes32(0x74)),
                writer_owner,
                writer_observation: SupervisorFactId::from_bytes(bytes32(0x75)),
                activation_head: epoch,
                programmatic_epoch: epoch,
                predecessor_revocation: predecessor_denied.then_some(
                    PredecessorRevocationReadback {
                        bind_denial: SupervisorFactId::from_bytes(bytes32(0x76)),
                        serve_denial: SupervisorFactId::from_bytes(bytes32(0x77)),
                        writer_denial: SupervisorFactId::from_bytes(bytes32(0x78)),
                    },
                ),
                predecessor_package: package,
                temporary_bridge: bridge,
            },
        )
        .expect("actual supervisor readback")
    }

    fn activation_authority(
        identity: CutoverChainIdentity,
        epoch: EpochId,
        event_marker: u8,
    ) -> CutoverActivationAuthority {
        activation_authority_with_fence(
            identity,
            epoch,
            event_marker,
            WriterFence {
                lease_id: LeaseId::from_bytes(bytes16(0x61)),
                generation: WriterGeneration::new(1).unwrap(),
            },
        )
    }

    fn activation_authority_with_fence(
        identity: CutoverChainIdentity,
        epoch: EpochId,
        event_marker: u8,
        writer_fence: WriterFence,
    ) -> CutoverActivationAuthority {
        let pins = FabricEpochPins {
            epoch,
            input_release: command_pins().input_release,
            program_release: command_pins().program_release,
            application_release: command_pins().application_release,
            source_authority: command_pins().source_authority,
            source_generation: command_pins().source_generation,
            provider_release: command_pins().provider_release,
            provider_set: command_pins().provider_set,
            table_versions: TableVersionSetRef::from_bytes(bytes32(0x31)),
            overlay_segments: OverlaySegmentSetRef::from_bytes(bytes32(0x32)),
            policy_set: PolicySetRef::from_bytes(bytes32(0x33)),
            resource_envelope: ResourceEnvelopeRef::from_bytes(bytes32(0x2a)),
            proof_receipt: ProofReceiptRef::from_bytes(bytes32(0x81)),
        };
        let command = FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes(bytes16(event_marker)),
                idempotency_key: IdempotencyKey::from_bytes(bytes32(event_marker)),
            },
            ownership: CommandOwnership {
                workspace_id: identity.workspace_id,
                principal_id: PrincipalId::from_bytes(bytes16(0x21)),
                authorization: AuthorizationRef::from_bytes(bytes32(0x22)),
            },
            expected_head: ExpectedHead::Empty,
            writer_fence,
            pins: command_pins(),
            resources: ResourceEnvelopeRef::from_bytes(bytes32(0x2a)),
            payload: FabricCommandPayload::ActivateEpoch {
                candidate_epoch: epoch,
                proof_receipt: pins.proof_receipt,
            },
        };
        let attempt = ActivationAttempt::for_test(
            command,
            1,
            ExecutionOwner {
                actor_id: ActorId::from_bytes(bytes16(0x23)),
                fence: writer_fence,
            },
        );
        let event = ActivationEvent::try_from_attempt(
            ActivationEventId::from_bytes(bytes32(event_marker)),
            attempt,
            None,
            ActivationOrdinal::new(1).unwrap(),
            pins,
            CompatibilityClassRef::from_bytes(bytes32(0x34)),
            RetentionPolicyRef::from_bytes(bytes32(0x35)),
            ActivationCommit {
                operation_selection: super::super::command::OperationSelectionRef::from_bytes(
                    bytes32(0x36),
                ),
                transaction: TransactionRef::from_bytes(bytes32(0x37)),
                backend_commit: BackendCommitRef::from_bytes(bytes32(0x38)),
                readback: ActivationReadbackRef::from_bytes(bytes32(0x39)),
            },
        )
        .expect("validated activation event");
        let chain = ActivationChain::derive(identity.workspace_id, [event])
            .expect("validated activation chain");
        CutoverActivationAuthority::try_from_chain(&chain).expect("exact activation authority")
    }

    fn target_proved(identity: CutoverChainIdentity) -> CutoverEvent {
        CutoverEvent::try_next(
            None,
            identity,
            command_for_workspace(identity.workspace_id, 1, 1),
            CutoverTransitionEvidence::TargetProved {
                proof_receipt: ProofReceiptRef::from_bytes(bytes32(0x81)),
            },
        )
        .expect("target proof event")
    }

    fn predecessor_fenced(previous: &CutoverEvent) -> CutoverEvent {
        let identity = previous.identity();
        CutoverEvent::try_next(
            Some(previous),
            identity,
            command_for_workspace(identity.workspace_id, 2, 1),
            CutoverTransitionEvidence::PredecessorFenced {
                supervisor: supervisor(identity, CutoverPhase::PredecessorFenced, None),
            },
        )
        .expect("predecessor fence event")
    }

    fn target_serving(previous: &CutoverEvent) -> CutoverEvent {
        let identity = previous.identity();
        let epoch = EpochId::from_bytes(bytes16(0x82));
        let activation = activation_authority(identity, epoch, 0x83);
        CutoverEvent::try_next(
            Some(previous),
            identity,
            command_for_workspace(identity.workspace_id, 3, 1),
            CutoverTransitionEvidence::TargetServing {
                activation,
                supervisor: supervisor(identity, CutoverPhase::TargetServing, Some(epoch)),
            },
        )
        .expect("target serving event")
    }

    fn target_mutating(previous: &CutoverEvent) -> CutoverEvent {
        let identity = previous.identity();
        let activation = previous.activation.expect("serving activation authority");
        CutoverEvent::try_next(
            Some(previous),
            identity,
            command_for_workspace(identity.workspace_id, 4, 2),
            CutoverTransitionEvidence::TargetMutating {
                activation,
                supervisor: supervisor(
                    identity,
                    CutoverPhase::TargetMutating,
                    Some(activation.selected_epoch),
                ),
            },
        )
        .expect("target mutation event")
    }

    fn complete(previous: &CutoverEvent) -> CutoverEvent {
        let identity = previous.identity();
        let activation = previous.activation.expect("mutating activation authority");
        CutoverEvent::try_next(
            Some(previous),
            identity,
            command_for_workspace(identity.workspace_id, 5, 2),
            CutoverTransitionEvidence::Complete {
                activation,
                supervisor: supervisor(
                    identity,
                    CutoverPhase::Complete,
                    Some(activation.selected_epoch),
                ),
            },
        )
        .expect("complete event")
    }

    fn journal_root(label: &str) -> (TempDir, PathBuf) {
        let root = TempDir::with_prefix(label).expect("temporary journal root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("private journal root");
        let path = root.path().join("cutover.sqlite3");
        (root, path)
    }

    fn full_chain(identity: CutoverChainIdentity) -> Vec<CutoverEvent> {
        let proved = target_proved(identity);
        let fenced = predecessor_fenced(&proved);
        let serving = target_serving(&fenced);
        let mutating = target_mutating(&serving);
        let done = complete(&mutating);
        vec![proved, fenced, serving, mutating, done]
    }

    struct ProductionCutoverFixture {
        _root: TempDir,
        config: crate::daemon::DaemonConfig,
        _admin_listener: UnixListener,
        _query_listener: UnixListener,
        controller: ProductionForwardCutoverController,
        workspace_id: WorkspaceId,
        activation: CutoverActivationAuthority,
        writer_fence: WriterFence,
    }

    fn private_test_file(path: &Path, bytes: &[u8]) {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .expect("create private test file");
        file.write_all(bytes).expect("write private test file");
        file.sync_all().expect("sync private test file");
    }

    fn production_cutover_fixture() -> ProductionCutoverFixture {
        let executable = std::env::current_exe().expect("current test executable");
        let root = tempfile::Builder::new()
            .prefix("cf41-")
            .tempdir_in(executable.parent().expect("test executable parent"))
            .expect("same-filesystem deployment root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("private fixture root");
        for name in ["package", "config", "state", "runtime"] {
            let path = root.path().join(name);
            fs::create_dir(&path).expect("create deployment directory");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .expect("private deployment directory");
        }
        let package_root = root.path().join("package");
        fs::hard_link(&executable, package_root.join("codefabric-target"))
            .expect("target entrypoint hard link");
        let config_root = root.path().join("config");
        private_test_file(
            &config_root.join("query.capability"),
            b"production-query-capability",
        );
        let config = crate::daemon::DaemonConfig {
            static_config: crate::daemon::StaticConfig {
                state_root: root.path().join("state"),
                runtime_root: root.path().join("runtime"),
                config_root: config_root.clone(),
                socket_endpoint: root.path().join("runtime/admin.sock"),
                query_socket_endpoint: root.path().join("runtime/query.sock"),
                query_capability_token_file: PathBuf::from("query.capability"),
                operational_database: PathBuf::from("operational.sqlite3"),
                sandbox_policy: "required-for-untrusted".to_owned(),
                hard_limit_profile: "daemon-default-v1".to_owned(),
                supported_platform_profile: "local-workstation-v1".to_owned(),
            },
            reloadable: crate::daemon::ReloadableConfig {
                log_level: "info".to_owned(),
                telemetry_sampling: 0.1,
                soft_query_quota: 4,
                maintenance_schedule: "daily-idle".to_owned(),
            },
        };
        let config_source = format!(
            r#"[static_config]
state_root = {state:?}
runtime_root = {runtime:?}
config_root = {config_root:?}
socket_endpoint = {admin:?}
query_socket_endpoint = {query:?}
query_capability_token_file = "query.capability"
operational_database = "operational.sqlite3"
sandbox_policy = "required-for-untrusted"
hard_limit_profile = "daemon-default-v1"
supported_platform_profile = "local-workstation-v1"

[reloadable]
log_level = "info"
telemetry_sampling = 0.1
soft_query_quota = 4
maintenance_schedule = "daily-idle"
"#,
            state = config.static_config.state_root.display().to_string(),
            runtime = config.static_config.runtime_root.display().to_string(),
            config_root = config.static_config.config_root.display().to_string(),
            admin = config.static_config.socket_endpoint.display().to_string(),
            query = config
                .static_config
                .query_socket_endpoint
                .display()
                .to_string(),
        );
        private_test_file(
            &config_root.join("codefabric.toml"),
            config_source.as_bytes(),
        );
        let workspace_id = workspace(0x55);
        let manifest = serde_json::json!({
            "schema": "codefabric.forward-cutover.deployment.v1",
            "package_root": package_root,
            "target_entrypoint": "codefabric-target",
            "daemon_config_file": "codefabric.toml",
            "workspaces": [{
                "plan_id": bytes32(0x55),
                "workspace_id": bytes16(0x55),
                "predecessor_release": bytes32(0x44)
            }]
        });
        private_test_file(
            &config_root.join(FORWARD_CUTOVER_DEPLOYMENT_FILE),
            &serde_json::to_vec_pretty(&manifest).expect("deployment JSON"),
        );
        let admin_listener = UnixListener::bind(&config.static_config.socket_endpoint)
            .expect("bind actual admin UDS");
        let query_listener = UnixListener::bind(&config.static_config.query_socket_endpoint)
            .expect("bind actual query UDS");
        for path in [
            &config.static_config.socket_endpoint,
            &config.static_config.query_socket_endpoint,
        ] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                .expect("private actual UDS");
        }
        let discovery = crate::daemon::DaemonDiscovery {
            daemon_instance_id: "production-cutover-test-instance".to_owned(),
            pid: std::process::id(),
            process_start_token: 41,
            socket_endpoint: config.static_config.socket_endpoint.clone(),
            query_socket_endpoint: config.static_config.query_socket_endpoint.clone(),
            rpc_minimum_minor: 0,
            rpc_maximum_minor: 0,
            basic_readiness: false,
            startup_time_unix_ms: 41,
            public_bundle_versions: BTreeMap::from([(
                "codefabric.programmatic.test".to_owned(),
                "v1".to_owned(),
            )]),
        };
        private_test_file(
            &config.static_config.runtime_root.join("daemon.json"),
            &serde_json::to_vec_pretty(&discovery).expect("discovery JSON"),
        );
        let controller = ProductionForwardCutoverController::open_if_configured(&config)
            .expect("validate deployment")
            .expect("configured controller");
        let activation = activation_authority_with_fence(
            identity(0x55),
            EpochId::from_bytes(bytes16(0x82)),
            0x83,
            WriterFence {
                lease_id: LeaseId::from_bytes(bytes16(0x61)),
                generation: WriterGeneration::new(2).unwrap(),
            },
        );
        let writer_fence = WriterFence {
            lease_id: LeaseId::from_bytes(bytes16(0x62)),
            generation: WriterGeneration::new(3).unwrap(),
        };
        ProductionCutoverFixture {
            _root: root,
            config,
            _admin_listener: admin_listener,
            _query_listener: query_listener,
            controller,
            workspace_id,
            activation,
            writer_fence,
        }
    }

    fn append_complete_production_chain(
        fixture: &ProductionCutoverFixture,
    ) -> ProductionCutoverEvidence {
        let evidence = fixture
            .controller
            .current_evidence_for_test(
                &fixture.config,
                fixture.workspace_id,
                fixture.activation,
                fixture.writer_fence,
            )
            .expect("actual production evidence");
        let (record, owner, transaction, context) = prepared_physical_zero_command(&evidence, 0xc1);
        let advanced = fixture
            .controller
            .advance_from_prepared_command_for_test(
                &fixture.config,
                fixture.workspace_id,
                fixture.activation,
                fixture.writer_fence,
                &record,
                owner,
                transaction,
                context,
            )
            .expect("production controller advances exact prepared command");
        assert!(matches!(
            advanced,
            ProductionCutoverAdvanceOutcome::Advanced { .. }
        ));
        let converged = fixture
            .controller
            .advance_from_prepared_command_for_test(
                &fixture.config,
                fixture.workspace_id,
                fixture.activation,
                fixture.writer_fence,
                &record,
                owner,
                transaction,
                context,
            )
            .expect("duplicate controller advance converges");
        assert!(matches!(
            converged,
            ProductionCutoverAdvanceOutcome::AlreadyConverged(_)
        ));
        evidence
    }

    #[test]
    fn wp41_prod_int_actual_package_config_process_uds_and_writer_evidence_is_bound() {
        let fixture = production_cutover_fixture();
        let evidence = fixture
            .controller
            .current_evidence_for_test(
                &fixture.config,
                fixture.workspace_id,
                fixture.activation,
                fixture.writer_fence,
            )
            .expect("actual production evidence");
        assert_eq!(evidence.identity.workspace_id(), fixture.workspace_id);
        assert_eq!(evidence.current_writer_fence, fixture.writer_fence);
        assert!(
            evidence.current_writer_fence.generation
                > evidence.activation.writer_fence().generation
        );
        assert_ne!(evidence.activation.writer_fence(), fixture.writer_fence);
        assert_eq!(
            evidence.activation.selected_epoch(),
            EpochId::from_bytes(bytes16(0x82))
        );
        let intents = fixture
            .controller
            .command_intents_for_test(
                &fixture.config,
                fixture.workspace_id,
                fixture.activation,
                fixture.writer_fence,
            )
            .expect("exact live command intent");
        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0].workspace_id, fixture.workspace_id);
        assert_eq!(intents[0].plan_id, evidence.identity.plan_id());
        assert_eq!(
            intents[0].expected_epoch,
            evidence.activation.selected_epoch()
        );
        assert_eq!(intents[0].current_writer_fence, fixture.writer_fence);
    }

    #[test]
    fn wp41_prod_beh_complete_journal_reopen_admits_actual_target_read_write() {
        let fixture = production_cutover_fixture();
        append_complete_production_chain(&fixture);
        let first = fixture
            .controller
            .operator_statuses_for_test(
                &fixture.config,
                fixture.workspace_id,
                fixture.activation,
                fixture.writer_fence,
            )
            .expect("first reopened journal status");
        let reopened = ProductionForwardCutoverController::open_if_configured(&fixture.config)
            .expect("revalidate production deployment")
            .expect("configured controller")
            .operator_statuses_for_test(
                &fixture.config,
                fixture.workspace_id,
                fixture.activation,
                fixture.writer_fence,
            )
            .expect("second reopened journal status");
        for statuses in [first, reopened] {
            assert_eq!(statuses.len(), 1);
            assert_eq!(
                statuses[0].status.admission,
                CutoverAdmission::TargetReadWrite
            );
            assert_eq!(statuses[0].status.code, "CUTOVER_TARGET_AUTHORITY_OBSERVED");
        }
    }

    #[test]
    fn wp41_prod_neg_extra_predecessor_config_and_stale_writer_fail_closed() {
        let fixture = production_cutover_fixture();
        let stale = WriterFence {
            lease_id: LeaseId::from_bytes(bytes16(0xee)),
            generation: WriterGeneration::new(1).unwrap(),
        };
        let equal = fixture.activation.writer_fence();
        for invalid in [stale, equal] {
            assert!(
                fixture
                    .controller
                    .operator_statuses_for_test(
                        &fixture.config,
                        fixture.workspace_id,
                        fixture.activation,
                        invalid,
                    )
                    .is_err(),
                "stale or equal activation generation cannot become the current writer"
            );
            assert!(
                fixture
                    .controller
                    .command_intents_for_test(
                        &fixture.config,
                        fixture.workspace_id,
                        fixture.activation,
                        invalid,
                    )
                    .is_err(),
                "command-intent census must reject a stale or equal writer generation"
            );
        }
        let evidence = append_complete_production_chain(&fixture);
        let (later_record, later_owner, later_transaction, later_context) =
            prepared_physical_zero_command(&evidence, 0xc2);
        assert!(
            fixture
                .controller
                .advance_from_prepared_command_for_test(
                    &fixture.config,
                    fixture.workspace_id,
                    fixture.activation,
                    fixture.writer_fence,
                    &later_record,
                    later_owner,
                    later_transaction,
                    later_context,
                )
                .is_err(),
            "another plan command cannot claim an earlier command's durable completion"
        );
        private_test_file(
            &fixture
                .config
                .static_config
                .config_root
                .join("predecessor-service.toml"),
            b"retired = true\n",
        );
        assert!(ProductionForwardCutoverController::open_if_configured(&fixture.config).is_err());
    }

    #[test]
    fn wp41_prod_ops_every_admin_read_reopens_and_revalidates_the_durable_journal() {
        let fixture = production_cutover_fixture();
        append_complete_production_chain(&fixture);
        let status = fixture
            .controller
            .operator_statuses_for_test(
                &fixture.config,
                fixture.workspace_id,
                fixture.activation,
                fixture.writer_fence,
            )
            .expect("healthy durable status");
        assert_eq!(
            status[0].status.admission,
            CutoverAdmission::TargetReadWrite
        );
        let path = fixture
            .config
            .static_config
            .state_root
            .join(FORWARD_CUTOVER_JOURNAL_FILE);
        let connection = Connection::open(&path).expect("open journal for corruption injection");
        connection
            .execute(
                "UPDATE forward_cutover_event SET payload = X'7B7D' WHERE sequence = 1",
                [],
            )
            .expect("inject durable tip corruption");
        drop(connection);
        assert!(
            fixture
                .controller
                .operator_statuses_for_test(
                    &fixture.config,
                    fixture.workspace_id,
                    fixture.activation,
                    fixture.writer_fence,
                )
                .is_err()
        );
    }

    #[test]
    fn wp41_int_event_contract_is_content_addressed_and_corruption_fails_closed() {
        let (_root, path) = journal_root("wp41-int-corruption");
        let identity = identity(1);
        let event = target_proved(identity);
        let journal = DurableForwardCutoverJournal::open(&path).expect("open journal");
        assert_eq!(
            journal.append(&event).expect("append target proof"),
            CutoverAppendOutcome::Appended(DurableCutoverState::At {
                phase: CutoverPhase::TargetProved,
                event_id: event.event_id(),
                sequence: 1,
            })
        );
        {
            let connection = journal.connection.lock().expect("journal mutex");
            connection
                .execute(
                    "UPDATE forward_cutover_event SET payload = X'7B7D' WHERE sequence = 1",
                    [],
                )
                .expect("inject payload corruption");
        }
        assert!(matches!(
            journal.current(identity.workspace_id),
            Err(ForwardCutoverJournalError::Corrupt(_))
        ));
    }

    #[test]
    fn wp41_int_transition_is_bound_to_the_reducer_validated_prepared_command() {
        let identity = identity(0x30);
        let (record, owner, transaction, context) = prepared_record(identity.workspace_id, 1, 1);
        let event = CutoverEvent::try_next_from_prepared_command(
            None,
            identity,
            &record,
            owner,
            transaction,
            context,
            CutoverTransitionEvidence::TargetProved {
                proof_receipt: ProofReceiptRef::from_bytes(bytes32(0x81)),
            },
        )
        .expect("prepared command constructs a cutover event");
        assert_eq!(
            event.command().operation_id(),
            record.command().identity.operation_id
        );
        assert_eq!(event.command().transaction(), transaction);

        assert_eq!(
            CutoverEvent::try_next_from_prepared_command(
                None,
                identity,
                &record,
                owner,
                TransactionRef::from_bytes(bytes32(0xee)),
                context,
                CutoverTransitionEvidence::TargetProved {
                    proof_receipt: ProofReceiptRef::from_bytes(bytes32(0x81)),
                },
            ),
            Err(PreparedCutoverEventError::Command(
                CommandPortError::ContextUnavailable
            )),
            "a caller-minted transaction cannot become cutover authority"
        );
    }

    #[test]
    fn wp41_int_unknown_payload_fields_and_schema_drift_fail_closed() {
        let (_root, path) = journal_root("wp41-int-closed-schema");
        let identity = identity(0x31);
        let event = target_proved(identity);
        let journal = DurableForwardCutoverJournal::open(&path).expect("open journal");
        journal.append(&event).expect("append target proof");
        {
            let connection = journal.connection.lock().expect("journal mutex");
            let payload: Vec<u8> = connection
                .query_row(
                    "SELECT payload FROM forward_cutover_event WHERE sequence = 1",
                    [],
                    |row| row.get(0),
                )
                .expect("load event payload");
            let mut value: serde_json::Value =
                serde_json::from_slice(&payload).expect("decode trusted event payload");
            value.as_object_mut().expect("event object").insert(
                "unregistered_authority".into(),
                serde_json::Value::Bool(true),
            );
            let corrupted = crate::contracts::jcs::canonicalize_value(&value)
                .expect("canonical corrupt payload");
            connection
                .execute(
                    "UPDATE forward_cutover_event SET payload = ?1 WHERE sequence = 1",
                    [corrupted],
                )
                .expect("inject unknown field");
        }
        assert!(matches!(
            journal.current(identity.workspace_id),
            Err(ForwardCutoverJournalError::Corrupt(_))
        ));
        {
            let connection = journal.connection.lock().expect("journal mutex");
            connection
                .execute(
                    "ALTER TABLE forward_cutover_reconciliation ADD COLUMN mutable_status TEXT",
                    [],
                )
                .expect("inject schema drift");
        }
        drop(journal);
        assert!(matches!(
            DurableForwardCutoverJournal::open(&path),
            Err(ForwardCutoverJournalError::Corrupt(_))
        ));
    }

    #[test]
    fn wp41_int_sequence_overflow_is_rejected_without_saturation() {
        let identity = identity(0x32);
        let mut previous = target_proved(identity);
        previous.sequence = u64::MAX;
        assert_eq!(
            CutoverEvent::try_next(
                Some(&previous),
                identity,
                command_for_workspace(identity.workspace_id, 2, 1),
                CutoverTransitionEvidence::PredecessorFenced {
                    supervisor: supervisor(identity, CutoverPhase::PredecessorFenced, None),
                },
            ),
            Err(CutoverEventError::SequenceExhausted)
        );
    }

    #[test]
    fn wp41_int_transition_identity_fence_and_phase_substitution_fail_closed() {
        let first_identity = identity(2);
        let proved = target_proved(first_identity);
        assert!(matches!(
            CutoverEvent::try_next(
                Some(&proved),
                identity(3),
                command_for_workspace(identity(3).workspace_id, 2, 1),
                CutoverTransitionEvidence::PredecessorFenced {
                    supervisor: supervisor(identity(3), CutoverPhase::PredecessorFenced, None,),
                },
            ),
            Err(CutoverEventError::ChainIdentityMismatch)
        ));
        assert!(matches!(
            CutoverEvent::try_next(
                Some(&proved),
                first_identity,
                command_for_workspace(first_identity.workspace_id, 4, 2),
                CutoverTransitionEvidence::TargetMutating {
                    activation: activation_authority(
                        first_identity,
                        EpochId::from_bytes(bytes16(0x82)),
                        0x84,
                    ),
                    supervisor: supervisor(
                        first_identity,
                        CutoverPhase::TargetMutating,
                        Some(EpochId::from_bytes(bytes16(0x82))),
                    ),
                },
            ),
            Err(CutoverEventError::InvalidTransition { .. })
        ));

        let mut substituted_fence = command_for_workspace(first_identity.workspace_id, 2, 1);
        substituted_fence.writer_fence.lease_id = LeaseId::from_bytes(bytes16(0x62));
        assert_eq!(
            CutoverEvent::try_next(
                Some(&proved),
                first_identity,
                substituted_fence,
                CutoverTransitionEvidence::PredecessorFenced {
                    supervisor: supervisor(first_identity, CutoverPhase::PredecessorFenced, None,),
                },
            ),
            Err(CutoverEventError::StaleWriterFence),
            "one durable generation cannot be rebound to a different lease"
        );
    }

    #[test]
    fn wp41_int_indexed_columns_and_command_aliases_fail_closed() {
        let (_root, path) = journal_root("wp41-int-index-and-command-alias");
        let identity = identity(0x34);
        let event = target_proved(identity);
        let journal = DurableForwardCutoverJournal::open(&path).expect("open journal");
        journal.append(&event).expect("append target proof");

        let mut operation_alias = command_for_workspace(identity.workspace_id, 0x35, 1);
        operation_alias.operation_id = event.command().operation_id;
        let operation_alias = CutoverEvent::try_next(
            None,
            identity,
            operation_alias,
            CutoverTransitionEvidence::TargetProved {
                proof_receipt: ProofReceiptRef::from_bytes(bytes32(0x81)),
            },
        )
        .expect("construct operation alias");
        assert!(matches!(
            journal.append(&operation_alias),
            Err(ForwardCutoverJournalError::CommandConflict)
        ));

        let mut transaction_alias = command_for_workspace(identity.workspace_id, 0x36, 1);
        transaction_alias.transaction = event.command().transaction;
        let transaction_alias = CutoverEvent::try_next(
            None,
            identity,
            transaction_alias,
            CutoverTransitionEvidence::TargetProved {
                proof_receipt: ProofReceiptRef::from_bytes(bytes32(0x81)),
            },
        )
        .expect("construct transaction alias");
        assert!(matches!(
            journal.append(&transaction_alias),
            Err(ForwardCutoverJournalError::CommandConflict)
        ));

        {
            let connection = journal.connection.lock().expect("journal mutex");
            connection
                .execute(
                    "UPDATE forward_cutover_event SET operation_id = ?1 WHERE sequence = 1",
                    [bytes16(0xee).as_slice()],
                )
                .expect("inject indexed-column drift");
        }
        assert!(matches!(
            journal.current(identity.workspace_id()),
            Err(ForwardCutoverJournalError::Corrupt(_))
        ));
    }

    #[test]
    fn wp41_int_phase_payload_shape_is_closed_on_durable_readback() {
        let identity = identity(0x36);
        let proved = target_proved(identity);
        let mut fenced = predecessor_fenced(&proved);
        fenced.activation = Some(activation_authority(
            identity,
            EpochId::from_bytes(bytes16(0xee)),
            0xee,
        ));
        fenced.event_id = fenced.derived_id().expect("readdress corrupt fenced event");
        assert!(matches!(
            validate_loaded_edge(Some(&proved), &fenced),
            Err(ForwardCutoverJournalError::Corrupt(_))
        ));

        let fenced = predecessor_fenced(&proved);
        let mut serving = target_serving(&fenced);
        serving.physical_zero_convergence = true;
        serving.event_id = serving
            .derived_id()
            .expect("readdress corrupt serving event");
        assert!(matches!(
            validate_loaded_edge(Some(&fenced), &serving),
            Err(ForwardCutoverJournalError::Event(
                CutoverEventError::InvalidTransition { .. }
            ))
        ));
    }

    #[test]
    fn wp41_int_uds_identity_requires_an_absolute_authorized_path() {
        assert_eq!(
            UdsEndpointId::derive(workspace(4), Path::new("relative.sock")),
            Err(CutoverEventError::InvalidUdsEndpoint)
        );
        assert_ne!(
            UdsEndpointId::derive(workspace(4), Path::new("/run/a.sock")).unwrap(),
            UdsEndpointId::derive(workspace(4), Path::new("/run/b.sock")).unwrap()
        );
    }

    #[test]
    fn wp41_beh_forward_chain_is_idempotent_and_survives_process_reopen() {
        let (_root, path) = journal_root("wp41-beh-forward");
        let identity = identity(5);
        let chain = full_chain(identity);
        {
            let journal = DurableForwardCutoverJournal::open(&path).expect("open journal");
            for event in &chain {
                assert!(matches!(
                    journal.append(event),
                    Ok(CutoverAppendOutcome::Appended(_))
                ));
            }
            assert!(matches!(
                journal.append(chain.last().unwrap()),
                Ok(CutoverAppendOutcome::DuplicateConverged(_))
            ));
        }
        let reopened = DurableForwardCutoverJournal::open(&path).expect("reopen journal");
        assert_eq!(
            reopened.current(identity.workspace_id).unwrap(),
            DurableCutoverState::At {
                phase: CutoverPhase::Complete,
                event_id: chain.last().unwrap().event_id(),
                sequence: 5,
            }
        );
        assert_eq!(reopened.events(identity.workspace_id).unwrap(), chain);
    }

    #[test]
    fn wp41_beh_predecessor_reenable_route_is_absent_and_target_repair_is_forward_only() {
        let identity = identity(6);
        let activation = activation_authority(identity, EpochId::from_bytes(bytes16(0x82)), 0x83);
        let converged = CutoverEvent::try_next(
            None,
            identity,
            command_for_workspace(identity.workspace_id, 9, 1),
            CutoverTransitionEvidence::PhysicalZeroConverged {
                activation,
                supervisor: supervisor(
                    identity,
                    CutoverPhase::Complete,
                    Some(activation.selected_epoch),
                ),
            },
        )
        .expect("physical-zero target convergence");
        assert_eq!(converged.phase(), CutoverPhase::Complete);
        assert!(matches!(
            CutoverEvent::try_next(
                Some(&converged),
                identity,
                command_for_workspace(identity.workspace_id, 11, 2),
                CutoverTransitionEvidence::PhysicalZeroConverged {
                    activation,
                    supervisor: supervisor(
                        identity,
                        CutoverPhase::Complete,
                        Some(activation.selected_epoch)
                    ),
                },
            ),
            Err(CutoverEventError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn wp41_beh_operator_status_is_derived_and_missing_supervisor_closes_admission() {
        let (_root, path) = journal_root("wp41-beh-operator-status");
        let identity = identity(7);
        let proved = target_proved(identity);
        let fenced = predecessor_fenced(&proved);
        let serving = target_serving(&fenced);
        let journal = DurableForwardCutoverJournal::open(&path).expect("open journal");
        for event in [&proved, &fenced, &serving] {
            journal.append(event).expect("append cutover event");
        }
        let missing = journal
            .operator_status(identity, None, serving.activation)
            .expect("missing-supervisor status");
        assert_eq!(missing.admission, CutoverAdmission::Closed);
        assert_eq!(
            missing.code,
            "CUTOVER_DEPLOYMENT_SUPERVISOR_OBSERVATION_REQUIRED"
        );
        let observed = journal
            .operator_status(identity, serving.supervisor, serving.activation)
            .expect("observed status");
        assert_eq!(observed.admission, CutoverAdmission::TargetReadOnly);

        let wrong_epoch = supervisor(
            identity,
            CutoverPhase::TargetServing,
            Some(EpochId::from_bytes(bytes16(0x99))),
        );
        let mismatched = journal
            .operator_status(identity, Some(wrong_epoch), serving.activation)
            .expect("mismatched status");
        assert_eq!(mismatched.admission, CutoverAdmission::Closed);
        assert_eq!(
            mismatched.code,
            "CUTOVER_DURABLE_AND_DEPLOYMENT_STATE_DISAGREE"
        );

        let mut predecessor_revived = serving.supervisor.expect("serving observation");
        predecessor_revived.predecessor_revocation = None;
        predecessor_revived.observation_id = predecessor_revived.derived_id();
        let revived = journal
            .operator_status(identity, Some(predecessor_revived), serving.activation)
            .expect("revived predecessor status");
        assert_eq!(revived.admission, CutoverAdmission::Closed);
    }

    #[test]
    fn wp41_neg_reboot_readback_revalidates_host_release_and_predecessor_revocation() {
        let (_root, path) = journal_root("wp41-neg-reboot-readback");
        let identity = identity(0x37);
        let proved = target_proved(identity);
        let fenced = predecessor_fenced(&proved);
        let serving = target_serving(&fenced);
        let journal = DurableForwardCutoverJournal::open(&path).expect("open journal");
        for event in [&proved, &fenced, &serving] {
            journal.append(event).expect("append cutover event");
        }
        let epoch = serving.activation.expect("activation").selected_epoch;
        let after_reboot = supervisor_on_boot(
            identity,
            CutoverPhase::TargetServing,
            Some(epoch),
            0x71,
            0x72,
        );
        let status = journal
            .operator_status(identity, Some(after_reboot), serving.activation)
            .expect("derive rebooted status");
        assert_eq!(status.admission, CutoverAdmission::TargetReadOnly);

        let mut substituted_release = after_reboot;
        substituted_release.predecessor_release = DaemonReleaseId::from_bytes(bytes32(0xee));
        substituted_release.observation_id = substituted_release.derived_id();
        let rejected = journal
            .operator_status(identity, Some(substituted_release), serving.activation)
            .expect("derive substituted release status");
        assert_eq!(rejected.admission, CutoverAdmission::Closed);
        assert_eq!(
            rejected.code,
            "CUTOVER_DEPLOYMENT_SUPERVISOR_OBSERVATION_MISMATCH"
        );
    }

    #[test]
    fn wp41_neg_missing_actual_bind_serve_write_or_reboot_proof_blocks_fence() {
        let identity = identity(8);
        let proved = target_proved(identity);
        let mut missing_reboot = supervisor(identity, CutoverPhase::PredecessorFenced, None);
        missing_reboot.previous_boot_id = missing_reboot.host_boot_id;
        missing_reboot.observation_id = missing_reboot.derived_id();
        assert_eq!(
            CutoverEvent::try_next(
                Some(&proved),
                identity,
                command_for_workspace(identity.workspace_id, 2, 1),
                CutoverTransitionEvidence::PredecessorFenced {
                    supervisor: missing_reboot,
                },
            ),
            Err(CutoverEventError::SupervisorEvidenceMissing)
        );
        let mut missing_write_denial = supervisor(identity, CutoverPhase::PredecessorFenced, None);
        missing_write_denial.predecessor_revocation = None;
        missing_write_denial.observation_id = missing_write_denial.derived_id();
        assert_eq!(
            CutoverEvent::try_next(
                Some(&proved),
                identity,
                command_for_workspace(identity.workspace_id, 2, 1),
                CutoverTransitionEvidence::PredecessorFenced {
                    supervisor: missing_write_denial,
                },
            ),
            Err(CutoverEventError::AuthorityCensusMismatch)
        );
    }

    #[test]
    fn wp41_neg_predecessor_revocation_and_temporary_authority_are_fail_closed() {
        let identity = identity(9);
        let proved = target_proved(identity);
        let fenced = predecessor_fenced(&proved);
        let epoch = EpochId::from_bytes(bytes16(0x82));
        let activation = activation_authority(identity, epoch, 0x83);
        let mut revived = supervisor(identity, CutoverPhase::TargetServing, Some(epoch));
        revived.uds_owner = AuthorityOwner::None;
        revived.observation_id = revived.derived_id();
        assert_eq!(
            CutoverEvent::try_next(
                Some(&fenced),
                identity,
                command_for_workspace(identity.workspace_id, 3, 1),
                CutoverTransitionEvidence::TargetServing {
                    activation,
                    supervisor: revived,
                },
            ),
            Err(CutoverEventError::AuthorityCensusMismatch)
        );
        let serving = target_serving(&fenced);
        let mutating = target_mutating(&serving);
        let mut incomplete = supervisor(identity, CutoverPhase::Complete, Some(epoch));
        incomplete.predecessor_package = ObservedAvailability::Present;
        incomplete.observation_id = incomplete.derived_id();
        assert_eq!(
            CutoverEvent::try_next(
                Some(&mutating),
                identity,
                command_for_workspace(identity.workspace_id, 5, 2),
                CutoverTransitionEvidence::Complete {
                    activation: mutating.activation.expect("mutating activation"),
                    supervisor: incomplete,
                },
            ),
            Err(CutoverEventError::TemporaryAuthorityRemains)
        );
    }

    #[test]
    fn wp41_neg_supervisor_config_and_activation_substitution_are_rejected() {
        let identity = identity(10);
        let proved = target_proved(identity);
        let mut observation = supervisor(identity, CutoverPhase::PredecessorFenced, None);
        observation.config_id = SupervisorConfigId::from_bytes(bytes32(0xff));
        assert_eq!(
            CutoverEvent::try_next(
                Some(&proved),
                identity,
                command_for_workspace(identity.workspace_id, 2, 1),
                CutoverTransitionEvidence::PredecessorFenced {
                    supervisor: observation,
                },
            ),
            Err(CutoverEventError::SupervisorEvidenceMissing)
        );
        let fenced = predecessor_fenced(&proved);
        let selected = EpochId::from_bytes(bytes16(0x82));
        let wrong = EpochId::from_bytes(bytes16(0x99));
        let activation = activation_authority(identity, selected, 0x83);
        assert_eq!(
            CutoverEvent::try_next(
                Some(&fenced),
                identity,
                command_for_workspace(identity.workspace_id, 3, 1),
                CutoverTransitionEvidence::TargetServing {
                    activation,
                    supervisor: supervisor(identity, CutoverPhase::TargetServing, Some(wrong),),
                },
            ),
            Err(CutoverEventError::ActivationIdentityMismatch)
        );
    }

    #[test]
    fn wp41_ops_unknown_then_exact_absence_resolves_to_retry_authority() {
        let (_root, path) = journal_root("wp41-ops-unknown");
        let identity = identity(11);
        let intended = target_proved(identity);
        let journal = DurableForwardCutoverJournal::open(&path).expect("open journal");
        let unknown = CutoverReconciliationEvidence::try_new(
            identity.workspace_id,
            intended.command().operation_id,
            intended.command().transaction,
            intended.event_id(),
            CutoverCommitObservation::Indeterminate,
        )
        .unwrap();
        assert_eq!(
            journal.reconcile(unknown).unwrap(),
            CutoverReconciliationOutcome::AdmissionClosed
        );
        let proved_absent = CutoverReconciliationEvidence::try_new(
            identity.workspace_id,
            intended.command().operation_id,
            intended.command().transaction,
            intended.event_id(),
            CutoverCommitObservation::ProvedNotCommitted {
                command_absence: readback_fact(0xa1),
                delta_marker_absence: readback_fact(0xa2),
                supervisor_unchanged: readback_fact(0xa3),
            },
        )
        .unwrap();
        assert_eq!(
            journal.reconcile(proved_absent).unwrap(),
            CutoverReconciliationOutcome::RetryAuthorized,
            "an indeterminate observation may be resolved by later complete absence evidence"
        );
    }

    #[test]
    fn wp41_ops_workspace_unknown_closes_unrelated_append_until_exact_retry() {
        let (_root, path) = journal_root("wp41-ops-workspace-admission");
        let identity = identity(0x40);
        let intended = target_proved(identity);
        let unrelated = CutoverEvent::try_next(
            None,
            identity,
            command_for_workspace(identity.workspace_id, 0x41, 1),
            CutoverTransitionEvidence::TargetProved {
                proof_receipt: ProofReceiptRef::from_bytes(bytes32(0x81)),
            },
        )
        .expect("construct unrelated initial event");
        let journal = DurableForwardCutoverJournal::open(&path).expect("open journal");
        let unknown = CutoverReconciliationEvidence::try_new(
            identity.workspace_id,
            intended.command().operation_id,
            intended.command().transaction,
            intended.event_id(),
            CutoverCommitObservation::Indeterminate,
        )
        .expect("construct unknown evidence");
        assert_eq!(
            journal.reconcile(unknown).unwrap(),
            CutoverReconciliationOutcome::AdmissionClosed
        );
        assert!(matches!(
            journal.append(&unrelated),
            Err(ForwardCutoverJournalError::ReconciliationAdmissionClosed)
        ));

        let absent = CutoverReconciliationEvidence::try_new(
            identity.workspace_id,
            intended.command().operation_id,
            intended.command().transaction,
            intended.event_id(),
            CutoverCommitObservation::ProvedNotCommitted {
                command_absence: readback_fact(0xe1),
                delta_marker_absence: readback_fact(0xe2),
                supervisor_unchanged: readback_fact(0xe3),
            },
        )
        .expect("construct complete absence evidence");
        assert_eq!(
            journal.reconcile(absent).unwrap(),
            CutoverReconciliationOutcome::RetryAuthorized
        );
        assert!(matches!(
            journal.append(&unrelated),
            Err(ForwardCutoverJournalError::ReconciliationAdmissionClosed)
        ));
        assert!(matches!(
            journal.append(&intended),
            Ok(CutoverAppendOutcome::Appended(_))
        ));
    }

    #[test]
    fn wp41_ops_partial_command_identity_match_requires_administrative_repair() {
        let (_root, path) = journal_root("wp41-ops-command-alias");
        let identity = identity(0x35);
        let intended = target_proved(identity);
        let journal = DurableForwardCutoverJournal::open(&path).expect("open journal");
        journal.append(&intended).expect("append intended event");

        let aliased = CutoverReconciliationEvidence::try_new(
            identity.workspace_id(),
            intended.command().operation_id,
            TransactionRef::from_bytes(bytes32(0xee)),
            intended.event_id(),
            CutoverCommitObservation::ProvedNotCommitted {
                command_absence: readback_fact(0xb1),
                delta_marker_absence: readback_fact(0xb2),
                supervisor_unchanged: readback_fact(0xb3),
            },
        )
        .expect("construct aliased reconciliation evidence");
        assert_eq!(
            journal.reconcile(aliased).unwrap(),
            CutoverReconciliationOutcome::AdministrativeRepairRequired,
            "an operation or transaction alias cannot authorize retry"
        );
    }

    #[test]
    fn wp41_ops_exact_not_committed_readback_retries_and_duplicate_execution_converges() {
        let (_root, path) = journal_root("wp41-ops-retry");
        let identity = identity(12);
        let intended = target_proved(identity);
        let journal = DurableForwardCutoverJournal::open(&path).expect("open journal");
        let absent = CutoverReconciliationEvidence::try_new(
            identity.workspace_id,
            intended.command().operation_id,
            intended.command().transaction,
            intended.event_id(),
            CutoverCommitObservation::ProvedNotCommitted {
                command_absence: readback_fact(0xc1),
                delta_marker_absence: readback_fact(0xc2),
                supervisor_unchanged: readback_fact(0xc3),
            },
        )
        .unwrap();
        assert_eq!(
            journal.reconcile(absent).unwrap(),
            CutoverReconciliationOutcome::RetryAuthorized
        );
        let stored_resolution = journal
            .connection
            .lock()
            .expect("journal mutex")
            .query_row(
                "SELECT resolution_code, length(command_readback_id),
                        length(delta_readback_id), length(supervisor_readback_id)
                 FROM forward_cutover_reconciliation
                 WHERE evidence_id = ?1",
                [absent.evidence_id.as_bytes().as_slice()],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .expect("read exact persisted reconciliation authority");
        assert_eq!(stored_resolution, (2, 32, 32, 32));
        journal.append(&intended).expect("retry append");
        assert_eq!(
            workspace_reconciliation_posture(
                &journal.connection.lock().expect("journal mutex"),
                identity.workspace_id,
            )
            .unwrap(),
            WorkspaceReconciliationPosture::Pending,
            "retry authority is not a committed readback and keeps admission closed"
        );
        assert!(matches!(
            journal.append(&intended),
            Ok(CutoverAppendOutcome::DuplicateConverged(_))
        ));
        let committed = CutoverReconciliationEvidence::try_new(
            identity.workspace_id,
            intended.command().operation_id,
            intended.command().transaction,
            intended.event_id(),
            CutoverCommitObservation::EventCommitted(intended.event_id()),
        )
        .unwrap();
        assert_eq!(
            journal.reconcile(committed).unwrap(),
            CutoverReconciliationOutcome::Committed(intended.event_id()),
            "proved absence authorizes one retry whose exact committed event resolves admission"
        );
        assert_eq!(
            workspace_reconciliation_posture(
                &journal.connection.lock().expect("journal mutex"),
                identity.workspace_id,
            )
            .unwrap(),
            WorkspaceReconciliationPosture::Clear
        );
    }

    #[test]
    fn wp41_ops_conflicting_committed_identity_cannot_be_hidden_by_the_same_outcome_code() {
        let (_root, path) = journal_root("wp41-ops-conflicting-commit-identity");
        let identity = identity(0x33);
        let intended = target_proved(identity);
        let journal = DurableForwardCutoverJournal::open(&path).expect("open journal");
        journal.append(&intended).expect("append intended event");
        let wrong = CutoverReconciliationEvidence::try_new(
            identity.workspace_id,
            intended.command().operation_id,
            intended.command().transaction,
            intended.event_id(),
            CutoverCommitObservation::EventCommitted(CutoverEventId::from_bytes(bytes32(0xee))),
        )
        .expect("wrong committed observation");
        assert_eq!(
            journal.reconcile(wrong).unwrap(),
            CutoverReconciliationOutcome::AdministrativeRepairRequired
        );
        let correct = CutoverReconciliationEvidence::try_new(
            identity.workspace_id,
            intended.command().operation_id,
            intended.command().transaction,
            intended.event_id(),
            CutoverCommitObservation::EventCommitted(intended.event_id()),
        )
        .expect("correct committed observation");
        assert_eq!(
            journal.reconcile(correct).unwrap(),
            CutoverReconciliationOutcome::AdministrativeRepairRequired,
            "a prior contradictory commit identity remains durable evidence"
        );
    }

    #[test]
    fn wp41_ops_crash_after_every_committed_edge_recovers_from_the_event_chain() {
        let chain = full_chain(identity(13));
        for (index, intended) in chain.iter().enumerate() {
            let (_root, path) = journal_root(&format!("wp41-ops-edge-{index}"));
            let journal = DurableForwardCutoverJournal::open(&path).expect("open journal");
            for predecessor in chain.iter().take(index) {
                journal.append(predecessor).expect("append predecessor");
            }
            journal
                .append(intended)
                .expect("append intended before crash");
            drop(journal);
            let reopened = DurableForwardCutoverJournal::open(&path).expect("reopen after crash");
            let unknown = CutoverReconciliationEvidence::try_new(
                intended.identity().workspace_id,
                intended.command().operation_id,
                intended.command().transaction,
                intended.event_id(),
                CutoverCommitObservation::Indeterminate,
            )
            .unwrap();
            assert_eq!(
                reopened.reconcile(unknown).unwrap(),
                CutoverReconciliationOutcome::Committed(intended.event_id())
            );
        }
    }

    #[test]
    fn wp41_ops_unknown_before_every_edge_retries_then_commits_forward() {
        let chain = full_chain(identity(0x42));
        for (index, intended) in chain.iter().enumerate() {
            let (_root, path) = journal_root(&format!("wp41-ops-precommit-edge-{index}"));
            {
                let journal = DurableForwardCutoverJournal::open(&path).expect("open journal");
                for predecessor in chain.iter().take(index) {
                    journal.append(predecessor).expect("append predecessor");
                }
                let unknown = CutoverReconciliationEvidence::try_new(
                    intended.identity().workspace_id,
                    intended.command().operation_id,
                    intended.command().transaction,
                    intended.event_id(),
                    CutoverCommitObservation::Indeterminate,
                )
                .expect("construct unknown observation");
                assert_eq!(
                    journal.reconcile(unknown).unwrap(),
                    CutoverReconciliationOutcome::AdmissionClosed
                );
                let absent = CutoverReconciliationEvidence::try_new(
                    intended.identity().workspace_id,
                    intended.command().operation_id,
                    intended.command().transaction,
                    intended.event_id(),
                    CutoverCommitObservation::ProvedNotCommitted {
                        command_absence: readback_fact(0xf1),
                        delta_marker_absence: readback_fact(0xf2),
                        supervisor_unchanged: readback_fact(0xf3),
                    },
                )
                .expect("construct exact absence observation");
                assert_eq!(
                    journal.reconcile(absent).unwrap(),
                    CutoverReconciliationOutcome::RetryAuthorized
                );
            }
            let reopened = DurableForwardCutoverJournal::open(&path).expect("reopen journal");
            reopened.append(intended).expect("append exact retry");
            let committed = CutoverReconciliationEvidence::try_new(
                intended.identity().workspace_id,
                intended.command().operation_id,
                intended.command().transaction,
                intended.event_id(),
                CutoverCommitObservation::EventCommitted(intended.event_id()),
            )
            .expect("construct exact committed observation");
            assert_eq!(
                reopened.reconcile(committed).unwrap(),
                CutoverReconciliationOutcome::Committed(intended.event_id())
            );
        }
    }

    #[test]
    fn wp41_ops_partial_absence_can_never_authorize_retry() {
        let identity = identity(14);
        let intended = target_proved(identity);
        let failure = CutoverReconciliationEvidence::try_new(
            identity.workspace_id,
            intended.command().operation_id,
            intended.command().transaction,
            intended.event_id(),
            CutoverCommitObservation::ProvedNotCommitted {
                command_absence: readback_fact(0xd1),
                delta_marker_absence: CutoverReadbackFactId::from_bytes([0; 32]),
                supervisor_unchanged: readback_fact(0xd3),
            },
        );
        assert!(matches!(
            failure,
            Err(ForwardCutoverJournalError::Corrupt(_))
        ));
    }
}
