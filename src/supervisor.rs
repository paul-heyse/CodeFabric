//! One-workspace supervisor, authenticated daemon control, and attach-only MCP launching.
//!
//! The supervisor is the only production parent of `codefabricd`.  Its unnamed socketpair is
//! mapped to daemon stdin, so no pathname, environment variable, or command-line argument can
//! impersonate the control authority.  Agent launchers attach to the private supervisor UDS,
//! request a policy-narrowed single-use grant, wait for daemon registration acknowledgement, and
//! pass the resulting launch envelope to the installed adapter on fd 3.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Read as _, Seek as _, SeekFrom, Write as _};
use std::net::Shutdown;
use std::num::NonZeroUsize;
use std::os::fd::{AsFd as _, OwnedFd};
use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _, OpenOptionsExt as _};
use std::os::unix::net::UnixStream as StdUnixStream;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use command_fds::{CommandFdExt as _, FdMapping};
use rustix::fs::{
    AtFlags, FileType, Mode, OFlags, fstat, fsync, open, openat, renameat, statat, unlinkat,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::UnixStream;
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock, Semaphore, mpsc};

use crate::cancellation::StructuredCancellationScope;
use crate::daemon::DaemonConfig;
use crate::identity::{IdentityDomain, decode_public_id};
use crate::operational_store::OperationalStore;
use crate::owned_unix_socket::{OwnedUnixSocket, OwnedUnixSocketError};
use crate::secure_path::{open_absolute_directory_nofollow, read_private_control_artifact};
use crate::session_authority::{
    LaunchPolicyId, LaunchPolicyRevision, MAXIMUM_CHALLENGE_TTL_SECONDS, RegisteredLaunchGrant,
    RevocationGeneration, SessionOperation,
};
use crate::workspace_registry::WorkspaceRegistry;

const CONTROL_MAX_BYTES: usize = 256 * 1024;
const SUPERVISOR_MAX_BYTES: usize = 256 * 1024;
const ADAPTER_DIAGNOSTIC_MAX_BYTES: usize = 1024 * 1024;
const MAX_DAEMON_RESTARTS: u32 = 3;
const MAX_SUPERVISOR_MONITOR_FAILURES: u32 = 120;
const DAEMON_READY_TIMEOUT: Duration = Duration::from_secs(120);
const POLICY_DIRECTORY: &str = "agent-launch-policies";
const SUPERVISOR_SOCKET: &str = "supervisor.sock";
const SUPERVISOR_DISCOVERY: &str = "supervisor.json";
const SUPERVISOR_LEASE: &str = "supervisor.lock";
const ADAPTER_LAUNCH_FD: i32 = 3;
const SUPERVISOR_LEASE_SLOT_BYTES: usize = 4_096;
const SUPERVISOR_LEASE_SLOT_COUNT: usize = 2;
const SUPERVISOR_LEASE_MAX_BYTES: usize = SUPERVISOR_LEASE_SLOT_BYTES * SUPERVISOR_LEASE_SLOT_COUNT;
const SUPERVISOR_LEASE_SLOT_MAGIC: &[u8; 8] = b"CFSLV2\0\0";
const SUPERVISOR_LEASE_SLOT_HEADER_BYTES: usize = 8 + 4 + 32;
const CONTROL_RECORD_MAXIMUM_LIFETIME_MS: i64 = 30_000;
const CONTROL_RECORD_HISTORY_LIMIT: usize = 64;
const LAUNCH_REQUEST_MAXIMUM_LIFETIME_MS: i64 = 30_000;
const LAUNCH_REQUEST_HISTORY_LIMIT: usize = 1_024;
const ADAPTER_EXECUTABLE_MAX_BYTES: u64 = 512 * 1024 * 1024;
const ADAPTER_IDENTITY_PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const ADAPTER_IDENTITY_OUTPUT_MAX_BYTES: usize = 512;
const ABANDONED_ADAPTER_TERM_GRACE: Duration = Duration::from_secs(2);
const ABANDONED_ADAPTER_KILL_GRACE: Duration = Duration::from_secs(5);
const ABANDONED_ADAPTER_EXIT_POLL: Duration = Duration::from_millis(25);
const DAEMON_CONTROL_IO_TIMEOUT: Duration = Duration::from_secs(2);
const DAEMON_ACCEPTED_WORK_DRAIN_TIMEOUT: Duration = Duration::from_secs(30);
// Source publication joins started native writes after accepted-query drain. Its finite
// 120-second shutdown allowance needs separate headroom from ordinary control I/O.
const DAEMON_WORKSPACE_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(150);
const SUPERVISOR_RENDEZVOUS_IO_TIMEOUT: Duration = Duration::from_secs(2);
const SUPERVISOR_RENDEZVOUS_HANDLE_TIMEOUT: Duration = Duration::from_secs(6);
const SUPERVISOR_RENDEZVOUS_TASK_LIMIT: usize = 32;

/// The exact bounded I/O stage that failed at a supervisor authority boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorIoStage {
    RendezvousRead,
    RendezvousHandle,
    RendezvousWrite,
    DaemonControlWrite,
    DaemonControlRead,
    DaemonControlValidate,
}

impl SupervisorIoStage {
    const fn is_daemon_control(self) -> bool {
        matches!(
            self,
            Self::DaemonControlWrite | Self::DaemonControlRead | Self::DaemonControlValidate
        )
    }
}

/// Operation identity carried independently of a control request's payload.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonControlOperation {
    Hello,
    RegisterLaunchGrant,
    RevokeLaunch,
    RevokePrincipal,
    AdvanceGeneration,
    Drain,
    Shutdown,
}

/// Per-channel secret established by the authenticated hello and never written to durable state.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct DaemonControlKey([u8; 32]);

impl fmt::Debug for DaemonControlKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DaemonControlKey([REDACTED])")
    }
}

/// Typed, bounded authority header for one daemon-control operation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonControlHeader {
    pub workspace_id: [u8; 16],
    pub daemon_generation: u64,
    pub supervisor_generation: u64,
    pub sequence: u64,
    pub operation: DaemonControlOperation,
    pub issued_at_unix_ms: i64,
    pub expires_at_unix_ms: i64,
    pub content_integrity: [u8; 32],
}

/// First record on the unnamed supervisor-to-daemon channel.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonControlHello {
    pub request_id: String,
    pub control_key: DaemonControlKey,
    pub supervisor_generation: u64,
    pub daemon_generation: u64,
    pub supervisor_pid: u32,
    pub supervisor_uid: u32,
}

/// Bounded target-only control protocol carried exclusively by the unnamed socketpair.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DaemonControlRequest {
    Hello(DaemonControlHello),
    RegisterLaunchGrant {
        request_id: String,
        grant: RegisteredLaunchGrant,
    },
    RevokeLaunch {
        request_id: String,
        grant_id: String,
        grant_digest: [u8; 32],
        adapter_pid: u32,
        adapter_start_identity: Option<String>,
    },
    RevokePrincipal {
        request_id: String,
        principal_id: [u8; 16],
        revocation_generation: RevocationGeneration,
    },
    AdvanceGeneration {
        request_id: String,
        daemon_generation: u64,
        supervisor_generation: u64,
    },
    Drain {
        request_id: String,
    },
    Shutdown {
        request_id: String,
    },
}

impl DaemonControlRequest {
    #[must_use]
    pub fn request_id(&self) -> &str {
        match self {
            Self::Hello(value) => &value.request_id,
            Self::RegisterLaunchGrant { request_id, .. }
            | Self::RevokeLaunch { request_id, .. }
            | Self::RevokePrincipal { request_id, .. }
            | Self::AdvanceGeneration { request_id, .. }
            | Self::Drain { request_id }
            | Self::Shutdown { request_id } => request_id,
        }
    }

    #[must_use]
    pub const fn operation(&self) -> DaemonControlOperation {
        match self {
            Self::Hello(_) => DaemonControlOperation::Hello,
            Self::RegisterLaunchGrant { .. } => DaemonControlOperation::RegisterLaunchGrant,
            Self::RevokeLaunch { .. } => DaemonControlOperation::RevokeLaunch,
            Self::RevokePrincipal { .. } => DaemonControlOperation::RevokePrincipal,
            Self::AdvanceGeneration { .. } => DaemonControlOperation::AdvanceGeneration,
            Self::Drain { .. } => DaemonControlOperation::Drain,
            Self::Shutdown { .. } => DaemonControlOperation::Shutdown,
        }
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct DaemonControlUnsignedRecord<'a> {
    workspace_id: [u8; 16],
    daemon_generation: u64,
    supervisor_generation: u64,
    sequence: u64,
    operation: DaemonControlOperation,
    issued_at_unix_ms: i64,
    expires_at_unix_ms: i64,
    request: &'a DaemonControlRequest,
}

/// One integrity-bound record on the inherited daemon-control channel.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonControlRecord {
    pub header: DaemonControlHeader,
    pub request: DaemonControlRequest,
}

impl DaemonControlRecord {
    fn new(
        request: DaemonControlRequest,
        key: &DaemonControlKey,
        workspace_id: [u8; 16],
        daemon_generation: u64,
        supervisor_generation: u64,
        sequence: u64,
    ) -> Result<Self, SupervisorError> {
        let issued_at_unix_ms = unix_millis()?;
        let expires_at_unix_ms = issued_at_unix_ms
            .checked_add(CONTROL_RECORD_MAXIMUM_LIFETIME_MS)
            .ok_or_else(|| SupervisorError::Control("control record expiry overflow".into()))?;
        let operation = request.operation();
        let content_integrity = control_record_integrity(
            key,
            &DaemonControlUnsignedRecord {
                workspace_id,
                daemon_generation,
                supervisor_generation,
                sequence,
                operation,
                issued_at_unix_ms,
                expires_at_unix_ms,
                request: &request,
            },
        )?;
        Ok(Self {
            header: DaemonControlHeader {
                workspace_id,
                daemon_generation,
                supervisor_generation,
                sequence,
                operation,
                issued_at_unix_ms,
                expires_at_unix_ms,
                content_integrity,
            },
            request,
        })
    }

    fn expected_integrity(&self, key: &DaemonControlKey) -> Result<[u8; 32], SupervisorError> {
        control_record_integrity(
            key,
            &DaemonControlUnsignedRecord {
                workspace_id: self.header.workspace_id,
                daemon_generation: self.header.daemon_generation,
                supervisor_generation: self.header.supervisor_generation,
                sequence: self.header.sequence,
                operation: self.header.operation,
                issued_at_unix_ms: self.header.issued_at_unix_ms,
                expires_at_unix_ms: self.header.expires_at_unix_ms,
                request: &self.request,
            },
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonControlAcknowledgement {
    pub request_id: String,
    pub workspace_id: [u8; 16],
    pub sequence: u64,
    pub operation: DaemonControlOperation,
    pub request_integrity: [u8; 32],
    pub issued_at_unix_ms: i64,
    pub expires_at_unix_ms: i64,
    pub accepted: bool,
    pub code: String,
    pub daemon_generation: u64,
    pub supervisor_generation: u64,
    pub content_integrity: [u8; 32],
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct DaemonControlUnsignedAcknowledgement<'a> {
    request_id: &'a str,
    workspace_id: [u8; 16],
    sequence: u64,
    operation: DaemonControlOperation,
    request_integrity: [u8; 32],
    issued_at_unix_ms: i64,
    expires_at_unix_ms: i64,
    accepted: bool,
    code: &'a str,
    daemon_generation: u64,
    supervisor_generation: u64,
}

impl DaemonControlAcknowledgement {
    fn new(
        key: &DaemonControlKey,
        request: &DaemonControlHeader,
        request_id: &str,
        accepted: bool,
        code: &str,
    ) -> Result<Self, SupervisorError> {
        let issued_at_unix_ms = unix_millis()?;
        let expires_at_unix_ms = issued_at_unix_ms
            .checked_add(CONTROL_RECORD_MAXIMUM_LIFETIME_MS)
            .ok_or_else(|| {
                SupervisorError::Control("control acknowledgement expiry overflow".into())
            })?;
        let unsigned = DaemonControlUnsignedAcknowledgement {
            request_id,
            workspace_id: request.workspace_id,
            sequence: request.sequence,
            operation: request.operation,
            request_integrity: request.content_integrity,
            issued_at_unix_ms,
            expires_at_unix_ms,
            accepted,
            code,
            daemon_generation: request.daemon_generation,
            supervisor_generation: request.supervisor_generation,
        };
        let content_integrity = control_record_integrity(key, &unsigned)?;
        Ok(Self {
            request_id: request_id.to_owned(),
            workspace_id: request.workspace_id,
            sequence: request.sequence,
            operation: request.operation,
            request_integrity: request.content_integrity,
            issued_at_unix_ms,
            expires_at_unix_ms,
            accepted,
            code: code.to_owned(),
            daemon_generation: request.daemon_generation,
            supervisor_generation: request.supervisor_generation,
            content_integrity,
        })
    }

    fn validate(
        &self,
        key: &DaemonControlKey,
        request: &DaemonControlRecord,
        observed_at_unix_ms: i64,
    ) -> Result<(), SupervisorError> {
        validate_control_interval(
            self.issued_at_unix_ms,
            self.expires_at_unix_ms,
            observed_at_unix_ms,
        )?;
        let expected = control_record_integrity(
            key,
            &DaemonControlUnsignedAcknowledgement {
                request_id: &self.request_id,
                workspace_id: self.workspace_id,
                sequence: self.sequence,
                operation: self.operation,
                request_integrity: self.request_integrity,
                issued_at_unix_ms: self.issued_at_unix_ms,
                expires_at_unix_ms: self.expires_at_unix_ms,
                accepted: self.accepted,
                code: &self.code,
                daemon_generation: self.daemon_generation,
                supervisor_generation: self.supervisor_generation,
            },
        )?;
        if self.content_integrity != expected
            || self.request_id != request.request.request_id()
            || self.workspace_id != request.header.workspace_id
            || self.sequence != request.header.sequence
            || self.operation != request.header.operation
            || self.request_integrity != request.header.content_integrity
            || self.daemon_generation != request.header.daemon_generation
            || self.supervisor_generation != request.header.supervisor_generation
        {
            return Err(SupervisorError::Control(
                "control acknowledgement binding or integrity mismatch".into(),
            ));
        }
        Ok(())
    }
}

/// Monotone receiver state for one inherited daemon-control channel.
#[derive(Debug)]
pub struct DaemonControlReadState {
    workspace_id: [u8; 16],
    daemon_generation: u64,
    supervisor_generation: u64,
    next_sequence: u64,
    key: DaemonControlKey,
    accepted_integrities: VecDeque<(u64, [u8; 32])>,
}

/// One control record accepted against the channel's monotone receiver state.
#[derive(Debug)]
pub struct AcceptedDaemonControlRecord {
    pub header: DaemonControlHeader,
    pub request: DaemonControlRequest,
}

impl DaemonControlReadState {
    fn from_hello(record: &DaemonControlRecord) -> Result<Self, SupervisorError> {
        let DaemonControlRequest::Hello(hello) = &record.request else {
            return Err(SupervisorError::Control(
                "first control record is not hello".into(),
            ));
        };
        if record.header.sequence != 1 {
            return Err(SupervisorError::Control("CONTROL_SEQUENCE_GAP".into()));
        }
        let mut state = Self {
            workspace_id: record.header.workspace_id,
            daemon_generation: record.header.daemon_generation,
            supervisor_generation: record.header.supervisor_generation,
            next_sequence: 1,
            key: hello.control_key.clone(),
            accepted_integrities: VecDeque::new(),
        };
        state.accept(record, unix_millis()?)?;
        Ok(state)
    }

    fn accept(
        &mut self,
        record: &DaemonControlRecord,
        observed_at_unix_ms: i64,
    ) -> Result<(), SupervisorError> {
        validate_control_interval(
            record.header.issued_at_unix_ms,
            record.header.expires_at_unix_ms,
            observed_at_unix_ms,
        )?;
        if record.header.workspace_id != self.workspace_id
            || record.header.daemon_generation != self.daemon_generation
            || record.header.supervisor_generation != self.supervisor_generation
            || record.header.operation != record.request.operation()
        {
            return Err(SupervisorError::Control("CONTROL_BINDING_MISMATCH".into()));
        }
        let expected_integrity = record.expected_integrity(&self.key)?;
        if expected_integrity != record.header.content_integrity {
            return Err(SupervisorError::Control(
                "CONTROL_CONTENT_INTEGRITY_MISMATCH".into(),
            ));
        }
        match record.header.sequence.cmp(&self.next_sequence) {
            std::cmp::Ordering::Greater => {
                return Err(SupervisorError::Control("CONTROL_SEQUENCE_GAP".into()));
            }
            std::cmp::Ordering::Less => {
                let unchanged = self.accepted_integrities.iter().any(|(sequence, digest)| {
                    *sequence == record.header.sequence
                        && *digest == record.header.content_integrity
                });
                let code = if unchanged {
                    "CONTROL_RECORD_REPLAY"
                } else {
                    "CONTROL_CHANGED_DUPLICATE"
                };
                return Err(SupervisorError::Control(code.into()));
            }
            std::cmp::Ordering::Equal => {}
        }
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| SupervisorError::Control("control sequence exhausted".into()))?;
        self.accepted_integrities
            .push_back((record.header.sequence, record.header.content_integrity));
        if self.accepted_integrities.len() > CONTROL_RECORD_HISTORY_LIMIT {
            self.accepted_integrities.pop_front();
        }
        Ok(())
    }

    #[must_use]
    pub const fn workspace_id(&self) -> [u8; 16] {
        self.workspace_id
    }
}

fn control_record_integrity<T: Serialize>(
    key: &DaemonControlKey,
    value: &T,
) -> Result<[u8; 32], SupervisorError> {
    let bytes = serde_json_canonicalizer::to_vec(value)
        .map_err(|error| SupervisorError::Control(error.to_string()))?;
    Ok(*blake3::keyed_hash(&key.0, &bytes).as_bytes())
}

fn validate_control_interval(
    issued_at_unix_ms: i64,
    expires_at_unix_ms: i64,
    observed_at_unix_ms: i64,
) -> Result<(), SupervisorError> {
    if issued_at_unix_ms <= 0
        || expires_at_unix_ms <= issued_at_unix_ms
        || expires_at_unix_ms.saturating_sub(issued_at_unix_ms) > CONTROL_RECORD_MAXIMUM_LIFETIME_MS
        || observed_at_unix_ms < issued_at_unix_ms
        || observed_at_unix_ms >= expires_at_unix_ms
    {
        return Err(SupervisorError::Control(
            "CONTROL_RECORD_EXPIRED_OR_INVALID".into(),
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentLaunchPolicy {
    pub format: String,
    pub policy_id: LaunchPolicyId,
    pub policy_revision: LaunchPolicyRevision,
    pub revocation_generation: RevocationGeneration,
    pub issued_at_unix_ms: i64,
    pub not_before_unix_ms: i64,
    pub expires_at_unix_ms: i64,
    pub adapter_program: PathBuf,
    pub adapter_distribution: String,
    pub adapter_distribution_version: String,
    pub adapter_executable_digest: String,
    #[serde(default)]
    pub adapter_arguments: Vec<String>,
    pub principal_id: String,
    pub workspace_ids: Vec<String>,
    pub operations: BTreeSet<SessionOperation>,
    pub semantic_profiles: BTreeSet<String>,
    pub maximum_resource_chunk_bytes: u64,
    pub maximum_result_bytes: u64,
    pub maximum_result_pages: u64,
    pub maximum_request_state_ttl_seconds: u64,
    pub maximum_session_seconds: u64,
    pub maximum_concurrent_launches: usize,
}

impl AgentLaunchPolicy {
    /// Validate the complete operator-owned policy before any daemon or adapter is started.
    pub fn validate(&self) -> Result<(), SupervisorError> {
        if self.format != "codefabric.agent-launch-policy.v1"
            || !self.policy_id.is_valid()
            || self.policy_revision.get() == 0
            || self.revocation_generation.get() == 0
            || self.issued_at_unix_ms <= 0
            || self.not_before_unix_ms < self.issued_at_unix_ms
            || self.expires_at_unix_ms <= self.not_before_unix_ms
            || !self.adapter_program.is_absolute()
            || !valid_distribution_identity(&self.adapter_distribution)
            || !valid_distribution_identity(&self.adapter_distribution_version)
            || !valid_blake3_digest(&self.adapter_executable_digest)
            || self.adapter_arguments.len() > 64
            || self
                .adapter_arguments
                .iter()
                .any(|argument| argument.len() > 4_096 || argument.contains('\0'))
            || self.workspace_ids.len() != 1
            || self.operations.is_empty()
            || self.semantic_profiles.is_empty()
            || self.maximum_resource_chunk_bytes == 0
            || self.maximum_result_bytes < self.maximum_resource_chunk_bytes
            || self.maximum_result_pages == 0
            || self.maximum_request_state_ttl_seconds == 0
            || self.maximum_request_state_ttl_seconds > MAXIMUM_CHALLENGE_TTL_SECONDS
            || self.maximum_session_seconds == 0
            || self.maximum_session_seconds > 86_400
            || self.maximum_concurrent_launches == 0
            || self.maximum_concurrent_launches > 4_096
            || self
                .semantic_profiles
                .iter()
                .any(|profile| !valid_semantic_profile(profile))
        {
            return Err(SupervisorError::Policy(
                "invalid launch policy header".into(),
            ));
        }
        decode_public_id(IdentityDomain::Owner, None, &self.principal_id)
            .map_err(|error| SupervisorError::Policy(error.to_string()))?;
        let mut workspace_ids = BTreeSet::new();
        for workspace in &self.workspace_ids {
            let workspace_id = decode_public_id(IdentityDomain::Workspace, None, workspace)
                .map_err(|error| SupervisorError::Policy(error.to_string()))?;
            if !workspace_ids.insert(workspace_id) {
                return Err(SupervisorError::Policy(
                    "duplicate workspace launch authority".into(),
                ));
            }
        }
        Ok(())
    }
}

fn valid_semantic_profile(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.is_ascii()
        && !value.chars().any(char::is_whitespace)
}

fn valid_distribution_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.is_ascii()
        && !value.chars().any(char::is_whitespace)
}

fn valid_blake3_digest(value: &str) -> bool {
    value.len() == 67
        && value.starts_with("b3:")
        && value.as_bytes()[3..].iter().all(u8::is_ascii_hexdigit)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupervisorDiscovery {
    pub format: String,
    pub supervisor_socket: PathBuf,
    pub query_socket: PathBuf,
    pub supervisor_generation: u64,
    pub daemon_generation: u64,
    pub daemon_pid: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum SupervisorRequest {
    Launch {
        policy_id: LaunchPolicyId,
        request_id: String,
        issued_at_unix_ms: i64,
        expires_at_unix_ms: i64,
    },
    ActivateLaunch {
        launch_id: String,
        adapter_pid: u32,
    },
    CancelLaunch {
        launch_id: String,
    },
    ReleaseLaunch {
        launch_id: String,
        adapter_pid: u32,
    },
    Status,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SupervisorResponse {
    accepted: bool,
    code: String,
    preparation: Option<AdapterLaunchPreparation>,
    launch: Option<AdapterLaunchEnvelope>,
    daemon_pid: u32,
    daemon_generation: u64,
    supervisor_generation: u64,
}

/// Public, non-secret projection of one supervisor control acknowledgement.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupervisorControlStatus {
    pub accepted: bool,
    pub code: String,
    pub daemon_pid: u32,
    pub daemon_generation: u64,
    pub supervisor_generation: u64,
}

/// Operator control actions accepted by an already-running workspace supervisor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorControlCommand {
    Status,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdapterLaunchPreparation {
    launch_id: String,
    adapter_program: PathBuf,
    adapter_arguments: Vec<String>,
    adapter_distribution: String,
    adapter_distribution_version: String,
    adapter_executable_digest: String,
    daemon_generation: u64,
    supervisor_generation: u64,
}

struct PendingLaunch {
    grant_bytes: [u8; 32],
    grant: RegisteredLaunchGrant,
    query_socket: PathBuf,
    preparation: AdapterLaunchPreparation,
    launcher_uid: u32,
    launcher_pid: Option<u32>,
    launcher_start_identity: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActiveLaunch {
    grant_id: String,
    grant_digest: [u8; 32],
    policy_id: LaunchPolicyId,
    adapter_pid: u32,
    launcher_uid: u32,
    launcher_pid: Option<u32>,
    launcher_start_identity: Option<String>,
    adapter_start_identity: Option<String>,
    expires_at_unix_ms: i64,
    daemon_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AcceptedLaunchRequest {
    request_id: String,
    policy_id: LaunchPolicyId,
    peer_uid: u32,
    peer_pid: Option<u32>,
    peer_start_identity: Option<String>,
    expires_at_unix_ms: i64,
}

#[derive(Default)]
struct SupervisorLaunchRegistry {
    pending: BTreeMap<String, PendingLaunch>,
    activating: BTreeMap<String, ActiveLaunch>,
    active: BTreeMap<String, ActiveLaunch>,
    released: VecDeque<ActiveLaunch>,
    accepted_requests: VecDeque<AcceptedLaunchRequest>,
}

impl SupervisorLaunchRegistry {
    fn occupied_slots(&self, policy_id: &LaunchPolicyId) -> usize {
        self.pending
            .values()
            .filter(|launch| &launch.grant.policy_id == policy_id)
            .count()
            .saturating_add(
                self.activating
                    .values()
                    .filter(|launch| &launch.policy_id == policy_id)
                    .count(),
            )
            .saturating_add(
                self.active
                    .values()
                    .filter(|launch| &launch.policy_id == policy_id)
                    .count(),
            )
    }

    fn prune_expired_pending(&mut self, observed_at_unix_ms: i64) {
        self.pending.retain(|_, launch| {
            launch.grant.issued_at_unix_ms <= observed_at_unix_ms
                && launch.grant.expires_at_unix_ms > observed_at_unix_ms
                && process_identity_matches(
                    launch.launcher_pid,
                    launch.launcher_start_identity.as_deref(),
                )
        });
    }

    fn record_released(&mut self, launch: ActiveLaunch) {
        self.released.push_back(launch);
        while self.released.len() > LAUNCH_REQUEST_HISTORY_LIMIT {
            self.released.pop_front();
        }
    }

    fn reserve(
        &mut self,
        pending: PendingLaunch,
        maximum_concurrent_launches: usize,
        observed_at_unix_ms: i64,
    ) -> Result<AdapterLaunchPreparation, SupervisorError> {
        self.prune_expired_pending(observed_at_unix_ms);
        if self.occupied_slots(&pending.grant.policy_id) >= maximum_concurrent_launches {
            return Err(SupervisorError::Capacity);
        }
        let preparation = pending.preparation.clone();
        if self.pending.contains_key(&preparation.launch_id)
            || self.activating.contains_key(&preparation.launch_id)
            || self.active.contains_key(&preparation.launch_id)
        {
            return Err(SupervisorError::Capacity);
        }
        self.pending.insert(preparation.launch_id.clone(), pending);
        Ok(preparation)
    }

    #[allow(clippy::too_many_arguments)]
    fn accept_request(
        &mut self,
        policy_id: &LaunchPolicyId,
        request_id: &str,
        issued_at_unix_ms: i64,
        expires_at_unix_ms: i64,
        observed_at_unix_ms: i64,
        peer_uid: u32,
        peer_pid: Option<u32>,
        peer_start_identity: Option<String>,
    ) -> Result<(), SupervisorError> {
        validate_launch_request_interval(
            request_id,
            issued_at_unix_ms,
            expires_at_unix_ms,
            observed_at_unix_ms,
        )?;
        self.accepted_requests
            .retain(|request| request.expires_at_unix_ms > observed_at_unix_ms);
        if self
            .accepted_requests
            .iter()
            .any(|request| request.request_id == request_id)
        {
            return Err(SupervisorError::Policy(
                "launch request anti-replay identity was already accepted".into(),
            ));
        }
        if self.accepted_requests.len() >= LAUNCH_REQUEST_HISTORY_LIMIT {
            return Err(SupervisorError::Capacity);
        }
        self.accepted_requests.push_back(AcceptedLaunchRequest {
            request_id: request_id.to_owned(),
            policy_id: policy_id.clone(),
            peer_uid,
            peer_pid,
            peer_start_identity,
            expires_at_unix_ms,
        });
        Ok(())
    }
}

fn validate_launch_request_interval(
    request_id: &str,
    issued_at_unix_ms: i64,
    expires_at_unix_ms: i64,
    observed_at_unix_ms: i64,
) -> Result<(), SupervisorError> {
    if request_id.is_empty()
        || request_id.len() > 192
        || !request_id.is_ascii()
        || request_id.chars().any(char::is_whitespace)
        || issued_at_unix_ms <= 0
        || expires_at_unix_ms <= issued_at_unix_ms
        || expires_at_unix_ms.saturating_sub(issued_at_unix_ms) > LAUNCH_REQUEST_MAXIMUM_LIFETIME_MS
        || observed_at_unix_ms < issued_at_unix_ms
        || observed_at_unix_ms >= expires_at_unix_ms
    {
        return Err(SupervisorError::Policy(
            "launch request anti-replay interval is invalid".into(),
        ));
    }
    Ok(())
}

/// Read-once adapter bootstrap envelope.  The grant is the only secret field.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterLaunchEnvelope {
    pub format: String,
    pub query_socket: PathBuf,
    pub launch_grant_hex: String,
    pub adapter_program: PathBuf,
    pub adapter_arguments: Vec<String>,
    pub daemon_generation: u64,
    pub supervisor_generation: u64,
    pub session_expires_at_unix_ms: i64,
    pub maximum_request_state_ttl_seconds: u64,
}

impl fmt::Debug for AdapterLaunchEnvelope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdapterLaunchEnvelope")
            .field("format", &self.format)
            .field("query_socket", &self.query_socket)
            .field("launch_grant_hex", &"[REDACTED]")
            .field("adapter_program", &self.adapter_program)
            .field("adapter_arguments", &self.adapter_arguments)
            .field("daemon_generation", &self.daemon_generation)
            .field("supervisor_generation", &self.supervisor_generation)
            .field(
                "session_expires_at_unix_ms",
                &self.session_expires_at_unix_ms,
            )
            .field(
                "maximum_request_state_ttl_seconds",
                &self.maximum_request_state_ttl_seconds,
            )
            .finish()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SupervisorLeaseRecord {
    format: String,
    supervisor_pid: u32,
    supervisor_uid: u32,
    supervisor_generation: u64,
    process_start_identity: Option<String>,
    issued_at_unix_ms: i64,
    runtime_device: u64,
    lease_device: u64,
    lease_inode: u64,
}

struct SupervisorLease {
    file: File,
    directory: OwnedFd,
    record: SupervisorLeaseRecord,
}

impl SupervisorLease {
    fn acquire(runtime_root: &Path) -> Result<Self, SupervisorError> {
        Self::acquire_with_clock(runtime_root, || {}, unix_millis)
    }

    fn acquire_with(
        runtime_root: &Path,
        after_open: impl FnOnce(),
    ) -> Result<Self, SupervisorError> {
        Self::acquire_with_clock(runtime_root, after_open, unix_millis)
    }

    fn acquire_with_clock(
        runtime_root: &Path,
        after_open: impl FnOnce(),
        observed_at: impl FnOnce() -> Result<i64, SupervisorError>,
    ) -> Result<Self, SupervisorError> {
        let path = runtime_root.join(SUPERVISOR_LEASE);
        let owner = rustix::process::geteuid().as_raw();
        let directory = open_absolute_directory_nofollow(runtime_root)
            .map_err(|error| SupervisorError::Config(error.to_string()))?;
        let directory_stat = fstat(&directory).map_err(|source| SupervisorError::Io {
            path: runtime_root.to_owned(),
            source: source.into(),
        })?;
        if !FileType::from_raw_mode(directory_stat.st_mode).is_dir()
            || directory_stat.st_uid != owner
            || directory_stat.st_mode & 0o777 != 0o700
        {
            return Err(SupervisorError::Config(
                "supervisor lease root is not an exact private owned directory".into(),
            ));
        }
        let descriptor = openat(
            &directory,
            SUPERVISOR_LEASE,
            OFlags::RDWR | OFlags::CREATE | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|source| SupervisorError::Io {
            path: path.clone(),
            source: source.into(),
        })?;
        let opened = fstat(&descriptor).map_err(|source| SupervisorError::Io {
            path: path.clone(),
            source: source.into(),
        })?;
        after_open();
        let current =
            statat(&directory, SUPERVISOR_LEASE, AtFlags::SYMLINK_NOFOLLOW).map_err(|source| {
                SupervisorError::Io {
                    path: path.clone(),
                    source: source.into(),
                }
            })?;
        if !FileType::from_raw_mode(opened.st_mode).is_file()
            || opened.st_uid != owner
            || opened.st_mode & 0o777 != 0o600
            || opened.st_dev != directory_stat.st_dev
            || current.st_dev != opened.st_dev
            || current.st_ino != opened.st_ino
        {
            return Err(SupervisorError::Config(
                "supervisor lease entry identity or authority is unsafe".into(),
            ));
        }
        let mut file = File::from(descriptor);
        match file.try_lock() {
            Ok(()) => {
                let existing = read_existing_lease_record(&mut file, &opened)?;
                validate_stale_supervisor_artifacts(runtime_root)?;
                if existing
                    .as_ref()
                    .is_some_and(|record| live_lease_record(runtime_root, record))
                {
                    let _ = file.unlock();
                    return Err(SupervisorError::LeaseHeld);
                }
                let supervisor_generation = existing.as_ref().map_or(Ok(1), |record| {
                    record.supervisor_generation.checked_add(1).ok_or_else(|| {
                        SupervisorError::Control("supervisor generation exhausted".into())
                    })
                })?;
                let record = SupervisorLeaseRecord {
                    format: "codefabric.supervisor-lease.v1".to_owned(),
                    supervisor_pid: std::process::id(),
                    supervisor_uid: owner,
                    supervisor_generation,
                    process_start_identity: process_start_identity(std::process::id()),
                    issued_at_unix_ms: observed_at()?,
                    runtime_device: directory_stat.st_dev,
                    lease_device: opened.st_dev,
                    lease_inode: opened.st_ino,
                };
                write_lease_record(&mut file, &record)?;
                fsync(&directory).map_err(|source| SupervisorError::Io {
                    path: runtime_root.to_owned(),
                    source: source.into(),
                })?;
                Ok(Self {
                    file,
                    directory,
                    record,
                })
            }
            Err(TryLockError::WouldBlock) => {
                // The kernel lock is the primary live-owner proof. The bounded record adds the
                // process/start/generation observation without allowing a losing racer to replace
                // or unlink an incomplete winner.
                let _ = read_existing_lease_record(&mut file, &opened).map(|record| {
                    record
                        .as_ref()
                        .is_some_and(|record| live_lease_record(runtime_root, record))
                });
                Err(SupervisorError::LeaseHeld)
            }
            Err(TryLockError::Error(source)) => Err(SupervisorError::Io { path, source }),
        }
    }
}

fn read_existing_lease_record(
    file: &mut File,
    opened: &rustix::fs::Stat,
) -> Result<Option<SupervisorLeaseRecord>, SupervisorError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|source| SupervisorError::Control(source.to_string()))?;
    let mut bytes = Vec::new();
    file.take((SUPERVISOR_LEASE_MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|source| SupervisorError::Control(source.to_string()))?;
    if bytes.len() > SUPERVISOR_LEASE_MAX_BYTES {
        return Err(SupervisorError::Config(
            "supervisor lease record exceeds its bound".into(),
        ));
    }
    if bytes.is_empty() {
        return Ok(None);
    }

    // The lock inode is also the durable generation authority, so replacing it would weaken
    // singleton semantics.  Two integrity-framed slots let a restart overwrite only the older
    // slot: a torn write can invalidate that slot, but cannot erase the last synced generation.
    let mut records = Vec::new();
    for index in 0..SUPERVISOR_LEASE_SLOT_COUNT {
        let start = index * SUPERVISOR_LEASE_SLOT_BYTES;
        if start >= bytes.len() {
            break;
        }
        let end = (start + SUPERVISOR_LEASE_SLOT_BYTES).min(bytes.len());
        if end - start != SUPERVISOR_LEASE_SLOT_BYTES {
            continue;
        }
        if let Ok(Some(record)) = decode_lease_slot(&bytes[start..end], opened)
            && usize::try_from((record.supervisor_generation - 1) % 2).ok() == Some(index)
        {
            records.push(record);
        }
    }
    if records.is_empty() {
        return Err(SupervisorError::Config(
            "supervisor lease has no intact durable generation slot".into(),
        ));
    }
    records.sort_by_key(|record| record.supervisor_generation);
    if records.len() == 2
        && records[1].supervisor_generation != records[0].supervisor_generation.saturating_add(1)
    {
        return Err(SupervisorError::Config(
            "supervisor lease generation slots are not consecutive".into(),
        ));
    }
    Ok(records.pop())
}

fn decode_lease_slot(
    slot: &[u8],
    opened: &rustix::fs::Stat,
) -> Result<Option<SupervisorLeaseRecord>, SupervisorError> {
    if slot.iter().all(|byte| *byte == 0) {
        return Ok(None);
    }
    if slot.len() != SUPERVISOR_LEASE_SLOT_BYTES
        || &slot[..SUPERVISOR_LEASE_SLOT_MAGIC.len()] != SUPERVISOR_LEASE_SLOT_MAGIC
    {
        return Err(SupervisorError::Config(
            "supervisor lease slot framing is invalid".into(),
        ));
    }
    let length = u32::from_be_bytes(
        slot[8..12]
            .try_into()
            .map_err(|_| SupervisorError::Config("invalid supervisor lease length".into()))?,
    ) as usize;
    if length == 0 || length > SUPERVISOR_LEASE_SLOT_BYTES - SUPERVISOR_LEASE_SLOT_HEADER_BYTES {
        return Err(SupervisorError::Config(
            "supervisor lease slot payload exceeds its bound".into(),
        ));
    }
    let payload_end = SUPERVISOR_LEASE_SLOT_HEADER_BYTES + length;
    let payload = &slot[SUPERVISOR_LEASE_SLOT_HEADER_BYTES..payload_end];
    if blake3::hash(payload).as_bytes() != &slot[12..44]
        || slot[payload_end..].iter().any(|byte| *byte != 0)
    {
        return Err(SupervisorError::Config(
            "supervisor lease slot integrity is invalid".into(),
        ));
    }
    let record: SupervisorLeaseRecord = serde_json::from_slice(payload)
        .map_err(|error| SupervisorError::Config(error.to_string()))?;
    if record.format != "codefabric.supervisor-lease.v1"
        || record.supervisor_pid <= 1
        || record.supervisor_uid != rustix::process::geteuid().as_raw()
        || record.supervisor_generation == 0
        || record.issued_at_unix_ms <= 0
        || record.lease_device != opened.st_dev
        || record.lease_inode != opened.st_ino
    {
        return Err(SupervisorError::Config(
            "supervisor lease record binding is invalid".into(),
        ));
    }
    Ok(Some(record))
}

fn encode_lease_slot(record: &SupervisorLeaseRecord) -> Result<Vec<u8>, SupervisorError> {
    let payload = serde_json_canonicalizer::to_vec(record)
        .map_err(|error| SupervisorError::Control(error.to_string()))?;
    if payload.is_empty()
        || payload.len() > SUPERVISOR_LEASE_SLOT_BYTES - SUPERVISOR_LEASE_SLOT_HEADER_BYTES
    {
        return Err(SupervisorError::Config(
            "supervisor lease record exceeds its slot bound".into(),
        ));
    }
    let payload_length = u32::try_from(payload.len())
        .map_err(|_| SupervisorError::Config("supervisor lease payload is too large".into()))?;
    let mut slot = vec![0_u8; SUPERVISOR_LEASE_SLOT_BYTES];
    slot[..8].copy_from_slice(SUPERVISOR_LEASE_SLOT_MAGIC);
    slot[8..12].copy_from_slice(&payload_length.to_be_bytes());
    slot[12..44].copy_from_slice(blake3::hash(&payload).as_bytes());
    slot[SUPERVISOR_LEASE_SLOT_HEADER_BYTES..SUPERVISOR_LEASE_SLOT_HEADER_BYTES + payload.len()]
        .copy_from_slice(&payload);
    Ok(slot)
}

fn write_lease_record(
    file: &mut File,
    record: &SupervisorLeaseRecord,
) -> Result<(), SupervisorError> {
    let slot = encode_lease_slot(record)?;
    let slot_index = usize::try_from((record.supervisor_generation - 1) % 2)
        .map_err(|_| SupervisorError::Control("invalid supervisor lease slot".into()))?;
    let offset = u64::try_from(slot_index * SUPERVISOR_LEASE_SLOT_BYTES)
        .map_err(|_| SupervisorError::Control("supervisor lease offset overflow".into()))?;
    file.seek(SeekFrom::Start(offset))
        .and_then(|_| file.write_all(&slot))
        .and_then(|()| file.sync_all())
        .map_err(|source| SupervisorError::Control(source.to_string()))?;
    let opened = fstat(&*file)
        .map_err(|source| SupervisorError::Control(format!("stat supervisor lease: {source}")))?;
    let readback = read_existing_lease_record(file, &opened)?;
    if readback.as_ref() != Some(record) {
        return Err(SupervisorError::Control(
            "supervisor lease exact durable readback failed".into(),
        ));
    }
    Ok(())
}

fn live_lease_record(runtime_root: &Path, record: &SupervisorLeaseRecord) -> bool {
    let Some(pid) = i32::try_from(record.supervisor_pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
    else {
        return false;
    };
    if rustix::process::test_kill_process(pid).is_err() {
        return false;
    }
    let process_matches = match &record.process_start_identity {
        Some(expected) => process_start_identity(record.supervisor_pid).as_ref() == Some(expected),
        None => true,
    };
    process_matches && live_supervisor_status_probe(runtime_root, record)
}

fn validate_stale_supervisor_artifacts(runtime_root: &Path) -> Result<(), SupervisorError> {
    let discovery_path = runtime_root.join(SUPERVISOR_DISCOVERY);
    match fs::symlink_metadata(&discovery_path) {
        Ok(_) => {
            let bytes = read_private_control_artifact(&discovery_path, SUPERVISOR_MAX_BYTES as u64)
                .map_err(|error| SupervisorError::Config(error.to_string()))?;
            let discovery: SupervisorDiscovery = serde_json::from_slice(&bytes)
                .map_err(|error| SupervisorError::Config(error.to_string()))?;
            if discovery.format != "codefabric.supervisor-discovery.v1"
                || discovery.supervisor_generation == 0
                || discovery.daemon_generation == 0
                || discovery.daemon_pid <= 1
                || discovery.supervisor_socket != runtime_root.join(SUPERVISOR_SOCKET)
                || !discovery.query_socket.starts_with(runtime_root)
            {
                return Err(SupervisorError::Config(
                    "stale supervisor discovery binding is invalid".into(),
                ));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(SupervisorError::Io {
                path: discovery_path,
                source,
            });
        }
    }
    validate_existing_supervisor_socket(runtime_root)
}

fn validate_existing_supervisor_socket(runtime_root: &Path) -> Result<(), SupervisorError> {
    let socket_path = runtime_root.join(SUPERVISOR_SOCKET);
    let root = fs::symlink_metadata(runtime_root).map_err(|source| SupervisorError::Io {
        path: runtime_root.to_owned(),
        source,
    })?;
    match fs::symlink_metadata(&socket_path) {
        Ok(metadata) => {
            if !metadata.file_type().is_socket()
                || metadata.uid() != rustix::process::geteuid().as_raw()
                || metadata.mode() & 0o777 != 0o600
                || metadata.dev() != root.dev()
                || metadata.ino() == 0
            {
                return Err(SupervisorError::Config(
                    "stale supervisor socket identity or authority is unsafe".into(),
                ));
            }
            let current =
                fs::symlink_metadata(&socket_path).map_err(|source| SupervisorError::Io {
                    path: socket_path.clone(),
                    source,
                })?;
            if current.dev() != metadata.dev()
                || current.ino() != metadata.ino()
                || current.ctime() != metadata.ctime()
                || current.ctime_nsec() != metadata.ctime_nsec()
            {
                return Err(SupervisorError::Config(
                    "stale supervisor socket changed during identity validation".into(),
                ));
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(SupervisorError::Io {
            path: socket_path,
            source,
        }),
    }
}

fn live_supervisor_status_probe(runtime_root: &Path, record: &SupervisorLeaseRecord) -> bool {
    (|| -> Option<bool> {
        let discovery_path = runtime_root.join(SUPERVISOR_DISCOVERY);
        let bytes =
            read_private_control_artifact(&discovery_path, SUPERVISOR_MAX_BYTES as u64).ok()?;
        let discovery: SupervisorDiscovery = serde_json::from_slice(&bytes).ok()?;
        if discovery.format != "codefabric.supervisor-discovery.v1"
            || discovery.supervisor_generation != record.supervisor_generation
            || discovery.supervisor_socket != runtime_root.join(SUPERVISOR_SOCKET)
        {
            return Some(false);
        }
        let before = fs::symlink_metadata(&discovery.supervisor_socket).ok()?;
        if !before.file_type().is_socket()
            || before.uid() != record.supervisor_uid
            || before.mode() & 0o777 != 0o600
            || before.dev() != record.runtime_device
        {
            return Some(false);
        }
        let mut stream = StdUnixStream::connect(&discovery.supervisor_socket).ok()?;
        let timeout = Some(Duration::from_millis(250));
        stream.set_read_timeout(timeout).ok()?;
        stream.set_write_timeout(timeout).ok()?;
        let mut request = serde_json::to_vec(&SupervisorRequest::Status).ok()?;
        request.push(b'\n');
        stream.write_all(&request).ok()?;
        let mut response = Vec::new();
        let mut byte = [0_u8; 1];
        while response.len() <= SUPERVISOR_MAX_BYTES {
            stream.read_exact(&mut byte).ok()?;
            response.push(byte[0]);
            if byte[0] == b'\n' {
                break;
            }
        }
        if response.len() > SUPERVISOR_MAX_BYTES || response.last() != Some(&b'\n') {
            return Some(false);
        }
        let response: SupervisorResponse = serde_json::from_slice(&response).ok()?;
        let after = fs::symlink_metadata(&discovery.supervisor_socket).ok()?;
        Some(
            response.accepted
                && response.code == "READY"
                && response.supervisor_generation == record.supervisor_generation
                && response.daemon_generation == discovery.daemon_generation
                && after.dev() == before.dev()
                && after.ino() == before.ino()
                && after.ctime() == before.ctime()
                && after.ctime_nsec() == before.ctime_nsec(),
        )
    })()
    .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn process_start_identity(pid: u32) -> Option<String> {
    // Linux process metadata authenticates a supervised peer; it is not workspace source input.
    // ast-grep-ignore: authoritative-source-read-boundary
    let bytes = fs::read(format!("/proc/{pid}/stat")).ok()?;
    if bytes.len() > 4_096 {
        return None;
    }
    let text = std::str::from_utf8(&bytes).ok()?;
    let close = text.rfind(')')?;
    // Field 22 is the process start tick. After the command name, field 3 is index zero.
    let start_ticks = text.get(close + 2..)?.split_whitespace().nth(19)?;
    Some(format!("linux-proc-start:{start_ticks}"))
}

#[cfg(not(target_os = "linux"))]
fn process_start_identity(_pid: u32) -> Option<String> {
    None
}

fn required_process_start_identity(pid: Option<u32>) -> Result<Option<String>, SupervisorError> {
    #[cfg(target_os = "linux")]
    {
        let pid = pid
            .ok_or_else(|| SupervisorError::Policy("kernel launcher PID is unavailable".into()))?;
        process_start_identity(pid)
            .map(Some)
            .ok_or_else(|| SupervisorError::Policy("process start identity is unavailable".into()))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Ok(pid.and_then(process_start_identity))
    }
}

fn process_identity_matches(pid: Option<u32>, expected_start_identity: Option<&str>) -> bool {
    let Some(pid) = pid else {
        return false;
    };
    #[cfg(target_os = "linux")]
    {
        expected_start_identity.is_some()
            && process_start_identity(pid).as_deref() == expected_start_identity
    }
    #[cfg(not(target_os = "linux"))]
    {
        i32::try_from(pid)
            .ok()
            .and_then(rustix::process::Pid::from_raw)
            .is_some_and(|pid| rustix::process::test_kill_process(pid).is_ok())
    }
}

impl Drop for SupervisorLease {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[derive(Debug)]
struct DaemonControlClientState {
    stream: UnixStream,
    next_sequence: u64,
}

#[derive(Debug)]
struct DaemonControlClient {
    state: Mutex<DaemonControlClientState>,
    workspace_id: [u8; 16],
    daemon_generation: u64,
    supervisor_generation: u64,
    key: DaemonControlKey,
    available: AtomicBool,
    io_timeout: Duration,
}

impl DaemonControlClient {
    async fn drain_accepted_work(&self) -> Result<(), SupervisorError> {
        // Drain acknowledgement follows accepted-query cancellation and joins. It can outlive
        // ordinary control IO, and must fit the supervisor's bounded shutdown allowance.
        let acknowledgement = self
            .transact_with_timeout(
                &DaemonControlRequest::Drain {
                    request_id: request_id("drain")?,
                },
                DAEMON_ACCEPTED_WORK_DRAIN_TIMEOUT,
            )
            .await?;
        if !acknowledgement.accepted || acknowledgement.code != "DRAIN_ACCEPTED" {
            return Err(SupervisorError::Control(acknowledgement.code));
        }
        Ok(())
    }

    fn is_available(&self) -> bool {
        self.available.load(Ordering::Acquire)
    }

    fn unavailable(&self, stage: SupervisorIoStage, detail: impl Into<String>) -> SupervisorError {
        self.available.store(false, Ordering::Release);
        SupervisorError::DaemonControlUnavailable {
            stage,
            detail: detail.into(),
        }
    }

    async fn transact(
        &self,
        request: &DaemonControlRequest,
    ) -> Result<DaemonControlAcknowledgement, SupervisorError> {
        self.transact_with_timeout(request, self.io_timeout).await
    }

    async fn transact_with_timeout(
        &self,
        request: &DaemonControlRequest,
        io_timeout: Duration,
    ) -> Result<DaemonControlAcknowledgement, SupervisorError> {
        if !self.is_available() {
            return Err(SupervisorError::DaemonControlUnavailable {
                stage: SupervisorIoStage::DaemonControlWrite,
                detail: "daemon control generation is already unavailable".into(),
            });
        }
        let mut state = self.state.lock().await;
        if !self.is_available() {
            return Err(SupervisorError::DaemonControlUnavailable {
                stage: SupervisorIoStage::DaemonControlWrite,
                detail: "daemon control generation became unavailable".into(),
            });
        }
        let record = DaemonControlRecord::new(
            request.clone(),
            &self.key,
            self.workspace_id,
            self.daemon_generation,
            self.supervisor_generation,
            state.next_sequence,
        )?;
        match tokio::time::timeout(
            io_timeout,
            write_line(&mut state.stream, &record, CONTROL_MAX_BYTES),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                return Err(
                    self.unavailable(SupervisorIoStage::DaemonControlWrite, error.to_string())
                );
            }
            Err(_) => {
                self.available.store(false, Ordering::Release);
                return Err(SupervisorError::IoDeadlineExceeded {
                    stage: SupervisorIoStage::DaemonControlWrite,
                });
            }
        }
        let acknowledgement: DaemonControlAcknowledgement =
            match tokio::time::timeout(io_timeout, read_line(&mut state.stream, CONTROL_MAX_BYTES))
                .await
            {
                Ok(Ok(acknowledgement)) => acknowledgement,
                Ok(Err(error)) => {
                    return Err(
                        self.unavailable(SupervisorIoStage::DaemonControlRead, error.to_string())
                    );
                }
                Err(_) => {
                    self.available.store(false, Ordering::Release);
                    return Err(SupervisorError::IoDeadlineExceeded {
                        stage: SupervisorIoStage::DaemonControlRead,
                    });
                }
            };
        if let Err(error) = acknowledgement.validate(&self.key, &record, unix_millis()?) {
            return Err(
                self.unavailable(SupervisorIoStage::DaemonControlValidate, error.to_string())
            );
        }
        state.next_sequence = state
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| SupervisorError::Control("control sequence exhausted".into()))?;
        Ok(acknowledgement)
    }
}

struct SupervisedDaemon {
    child: Child,
    control: Arc<DaemonControlClient>,
    generation: u64,
}

#[derive(Clone)]
struct SupervisorServingSnapshot {
    ready: bool,
    daemon_pid: u32,
    daemon_generation: u64,
    control: Arc<DaemonControlClient>,
}

struct SupervisorServingState {
    ready: bool,
    daemon_pid: u32,
    daemon_generation: u64,
    control: Arc<DaemonControlClient>,
}

impl SupervisorServingState {
    fn snapshot(&self) -> SupervisorServingSnapshot {
        SupervisorServingSnapshot {
            ready: self.ready && self.control.is_available(),
            daemon_pid: self.daemon_pid,
            daemon_generation: self.daemon_generation,
            control: self.control.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DaemonControlEscalation {
    daemon_generation: u64,
    stage: SupervisorIoStage,
}

enum SupervisorLoopEvent {
    Continue,
    Shutdown,
    DaemonExited(String),
    DaemonControlFailed(DaemonControlEscalation),
}

async fn close_failed_daemon_generation(
    serving: &RwLock<SupervisorServingState>,
    launches: &Mutex<SupervisorLaunchRegistry>,
    daemon_generation: u64,
) -> bool {
    let mut serving = serving.write().await;
    if serving.daemon_generation != daemon_generation {
        return false;
    }
    serving.ready = false;
    drop(serving);
    *launches.lock().await = SupervisorLaunchRegistry::default();
    true
}

struct AgentLaunchPolicyStore {
    root: PathBuf,
    directory: OwnedFd,
    device: u64,
    inode: u64,
}

impl AgentLaunchPolicyStore {
    fn open(root: &Path) -> Result<Self, SupervisorError> {
        let directory = open_absolute_directory_nofollow(root)
            .map_err(|error| SupervisorError::Config(error.to_string()))?;
        let stat = fstat(&directory).map_err(|source| SupervisorError::Io {
            path: root.to_owned(),
            source: source.into(),
        })?;
        if !FileType::from_raw_mode(stat.st_mode).is_dir()
            || stat.st_uid != rustix::process::geteuid().as_raw()
            || stat.st_mode & 0o777 != 0o700
            || stat.st_ino == 0
        {
            return Err(SupervisorError::Config(
                "launch policy store is not an exact private owned directory".into(),
            ));
        }
        Ok(Self {
            root: root.to_owned(),
            directory,
            device: stat.st_dev,
            inode: stat.st_ino,
        })
    }

    fn load(&self, policy_id: &LaunchPolicyId) -> Result<AgentLaunchPolicy, SupervisorError> {
        self.load_with(policy_id, || {})
    }

    fn load_with<F>(
        &self,
        policy_id: &LaunchPolicyId,
        before_final_path_validation: F,
    ) -> Result<AgentLaunchPolicy, SupervisorError>
    where
        F: FnOnce(),
    {
        if !policy_id.is_valid() {
            return Err(SupervisorError::Policy("invalid launch policy ID".into()));
        }
        let current_root = fstat(&self.directory).map_err(|source| SupervisorError::Io {
            path: self.root.clone(),
            source: source.into(),
        })?;
        if current_root.st_dev != self.device
            || current_root.st_ino != self.inode
            || current_root.st_uid != rustix::process::geteuid().as_raw()
            || current_root.st_mode & 0o777 != 0o700
        {
            return Err(SupervisorError::Policy(
                "launch policy store identity changed".into(),
            ));
        }
        let name = format!("{}.json", policy_id.as_str());
        let path = self.root.join(&name);
        let descriptor = openat(
            &self.directory,
            name.as_str(),
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(|source| SupervisorError::Io {
            path: path.clone(),
            source: source.into(),
        })?;
        let opened = fstat(&descriptor).map_err(|source| SupervisorError::Io {
            path: path.clone(),
            source: source.into(),
        })?;
        if !FileType::from_raw_mode(opened.st_mode).is_file()
            || opened.st_uid != rustix::process::geteuid().as_raw()
            || opened.st_mode & 0o7777 != 0o600
            || opened.st_dev != self.device
            || opened.st_size <= 0
            || u64::try_from(opened.st_size).unwrap_or(u64::MAX) > SUPERVISOR_MAX_BYTES as u64
        {
            return Err(SupervisorError::Policy(
                "launch policy entry authority is unsafe".into(),
            ));
        }
        let mut file = File::from(descriptor);
        let first = read_bounded_policy(&mut file)?;
        let middle = file.metadata().map_err(|source| SupervisorError::Io {
            path: path.clone(),
            source,
        })?;
        file.seek(SeekFrom::Start(0))
            .map_err(|source| SupervisorError::Io {
                path: path.clone(),
                source,
            })?;
        let second = read_bounded_policy(&mut file)?;
        let after = file.metadata().map_err(|source| SupervisorError::Io {
            path: path.clone(),
            source,
        })?;
        before_final_path_validation();
        let current = statat(&self.directory, name.as_str(), AtFlags::SYMLINK_NOFOLLOW).map_err(
            |source| SupervisorError::Io {
                path: path.clone(),
                source: source.into(),
            },
        )?;
        if first != second
            || middle.dev() != opened.st_dev
            || middle.ino() != opened.st_ino
            || after.dev() != opened.st_dev
            || after.ino() != opened.st_ino
            || middle.len() != u64::try_from(opened.st_size).unwrap_or(u64::MAX)
            || after.len() != middle.len()
            || after.mtime() != middle.mtime()
            || after.mtime_nsec() != middle.mtime_nsec()
            || after.ctime() != middle.ctime()
            || after.ctime_nsec() != middle.ctime_nsec()
            || current.st_dev != opened.st_dev
            || current.st_ino != opened.st_ino
            || !FileType::from_raw_mode(current.st_mode).is_file()
            || current.st_uid != opened.st_uid
            || current.st_mode & 0o7777 != 0o600
            || current.st_size != opened.st_size
            || current.st_mtime != opened.st_mtime
            || current.st_mtime_nsec != opened.st_mtime_nsec
            || current.st_ctime != opened.st_ctime
            || current.st_ctime_nsec != opened.st_ctime_nsec
        {
            return Err(SupervisorError::Policy(
                "launch policy entry changed during exact read".into(),
            ));
        }
        let policy: AgentLaunchPolicy = serde_json::from_slice(&first)
            .map_err(|error| SupervisorError::Policy(error.to_string()))?;
        policy.validate()?;
        if &policy.policy_id != policy_id {
            return Err(SupervisorError::Policy(
                "launch policy file identity differs from selected opaque ID".into(),
            ));
        }
        Ok(policy)
    }
}

fn read_bounded_policy(file: &mut File) -> Result<Vec<u8>, SupervisorError> {
    let mut bytes = Vec::new();
    file.take((SUPERVISOR_MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|source| SupervisorError::Control(source.to_string()))?;
    if bytes.is_empty() || bytes.len() > SUPERVISOR_MAX_BYTES {
        return Err(SupervisorError::Policy(
            "launch policy entry exceeds its strict byte bound".into(),
        ));
    }
    Ok(bytes)
}

impl SupervisedDaemon {
    async fn spawn(
        config_path: &Path,
        workspace_id: [u8; 16],
        generation: u64,
        supervisor_generation: u64,
    ) -> Result<Self, SupervisorError> {
        let (parent, child_endpoint) = StdUnixStream::pair().map_err(|source| {
            SupervisorError::Control(format!("daemon control socketpair: {source}"))
        })?;
        parent
            .set_nonblocking(true)
            .map_err(|source| SupervisorError::Control(format!("control nonblocking: {source}")))?;
        let executable = sibling_binary("codefabricd")?;
        let mut command = Command::new(executable);
        command
            .args(["serve", "--config"])
            .arg(config_path)
            .stdin(Stdio::from(OwnedFd::from(child_endpoint)))
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command
            .spawn()
            .map_err(|source| SupervisorError::Child(format!("spawn codefabricd: {source}")))?;
        let stream = UnixStream::from_std(parent)
            .map_err(|source| SupervisorError::Control(format!("control stream: {source}")))?;
        let key = DaemonControlKey(random32()?);
        let control = Arc::new(DaemonControlClient {
            state: Mutex::new(DaemonControlClientState {
                stream,
                next_sequence: 1,
            }),
            workspace_id,
            daemon_generation: generation,
            supervisor_generation,
            key: key.clone(),
            available: AtomicBool::new(true),
            io_timeout: DAEMON_CONTROL_IO_TIMEOUT,
        });
        let hello = DaemonControlRequest::Hello(DaemonControlHello {
            request_id: request_id("hello")?,
            control_key: key,
            supervisor_generation,
            daemon_generation: generation,
            supervisor_pid: std::process::id(),
            supervisor_uid: rustix::process::geteuid().as_raw(),
        });
        let acknowledgement = tokio::select! {
            response = tokio::time::timeout(
                DAEMON_READY_TIMEOUT,
                control.transact_with_timeout(&hello, DAEMON_READY_TIMEOUT),
            ) => {
                response
                    .map_err(|_| SupervisorError::Control("daemon hello deadline exceeded".into()))?
            }
            status = child.wait() => {
                let status = status
                    .map_err(|source| SupervisorError::Child(format!("join codefabricd: {source}")))?;
                return Err(SupervisorError::Child(format!(
                    "codefabricd exited before the authenticated ready acknowledgement: {status}"
                )));
            }
        };
        let acknowledgement = match acknowledgement {
            Ok(acknowledgement) => acknowledgement,
            Err(error) => {
                terminate_and_join_child(&mut child, "failed daemon hello").await?;
                return Err(error);
            }
        };
        if !acknowledgement.accepted {
            terminate_and_join_child(&mut child, "rejected daemon hello").await?;
            return Err(SupervisorError::Control(acknowledgement.code));
        }
        Ok(Self {
            child,
            control,
            generation,
        })
    }
}

struct SupervisorStartupConfiguration {
    daemon: DaemonConfig,
    launch_policies: AgentLaunchPolicyStore,
    workspace_id: [u8; 16],
}

fn load_supervisor_startup_configuration(
    config_path: &Path,
) -> Result<SupervisorStartupConfiguration, SupervisorError> {
    let config = DaemonConfig::load(config_path)
        .map_err(|error| SupervisorError::Config(error.to_string()))?;
    validate_private_directory(&config.static_config.runtime_root)?;
    validate_private_directory(&config.static_config.config_root)?;
    let launch_policies =
        AgentLaunchPolicyStore::open(&config.static_config.config_root.join(POLICY_DIRECTORY))?;
    let operational_path = config
        .static_config
        .state_root
        .join(&config.static_config.operational_database);
    let mut operational = OperationalStore::open(&operational_path)
        .map_err(|error| SupervisorError::Config(error.to_string()))?;
    let workspaces = WorkspaceRegistry::new(&mut operational)
        .list()
        .map_err(|error| SupervisorError::Config(error.to_string()))?;
    if workspaces.len() != 1 {
        return Err(SupervisorError::Config(
            "workspace supervisor requires exactly one operational workspace".into(),
        ));
    }
    Ok(SupervisorStartupConfiguration {
        daemon: config,
        launch_policies,
        workspace_id: workspaces[0].workspace_id,
    })
}

/// Validate all file-backed startup authority needed by the workspace supervisor.
pub fn check_supervisor_config(config_path: &Path) -> Result<(), SupervisorError> {
    load_supervisor_startup_configuration(config_path).map(drop)
}

/// Run the one persistent supervisor for the configured workspace deployment.
pub async fn serve_supervisor(config_path: &Path) -> Result<(), SupervisorError> {
    let startup = load_supervisor_startup_configuration(config_path)?;
    let config = startup.daemon;
    let launch_policies = Arc::new(startup.launch_policies);
    let workspace_id = startup.workspace_id;
    let lease = SupervisorLease::acquire(&config.static_config.runtime_root)?;
    let supervisor_generation = lease.record.supervisor_generation;
    let mut daemon =
        SupervisedDaemon::spawn(config_path, workspace_id, 1, supervisor_generation).await?;
    let daemon_pid = daemon
        .child
        .id()
        .ok_or_else(|| SupervisorError::Child("daemon PID unavailable".into()))?;
    let socket_path = config.static_config.runtime_root.join(SUPERVISOR_SOCKET);
    let (listener, mut socket) = OwnedUnixSocket::bind_with_root_descriptor(
        &config.static_config.runtime_root,
        &socket_path,
        supervisor_generation,
        &lease.directory,
    )?;
    let discovery_path = config.static_config.runtime_root.join(SUPERVISOR_DISCOVERY);
    let discovery = SupervisorDiscovery {
        format: "codefabric.supervisor-discovery.v1".to_owned(),
        supervisor_socket: socket_path,
        query_socket: config.static_config.query_socket_endpoint.clone(),
        supervisor_generation,
        daemon_generation: daemon.generation,
        daemon_pid,
    };
    write_private_json_at(
        &lease.directory,
        SUPERVISOR_DISCOVERY,
        &discovery,
        &discovery_path,
    )?;
    let allowed_uid = rustix::process::geteuid().as_raw();
    let query_socket = config.static_config.query_socket_endpoint.clone();
    let launches = Arc::new(Mutex::new(SupervisorLaunchRegistry::default()));
    let serving = Arc::new(RwLock::new(SupervisorServingState {
        ready: true,
        daemon_pid,
        daemon_generation: daemon.generation,
        control: daemon.control.clone(),
    }));
    let connection_slots = Arc::new(Semaphore::new(SUPERVISOR_RENDEZVOUS_TASK_LIMIT));
    let connection_tasks = StructuredCancellationScope::try_root(
        "supervisor",
        NonZeroUsize::new(SUPERVISOR_RENDEZVOUS_TASK_LIMIT + 8)
            .expect("supervisor task capacity is nonzero"),
    )
    .and_then(|root| root.child("rendezvous"))
    .map_err(|error| SupervisorError::Control(format!("supervisor task hierarchy: {error}")))?;
    let mut connection_sequence = 0_u64;
    let (escalation_sender, mut escalation_receiver) = mpsc::unbounded_channel();
    let mut launch_reaper = tokio::time::interval(Duration::from_millis(250));
    launch_reaper.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut terminate_signal = tokio::signal::unix::signal(
        tokio::signal::unix::SignalKind::terminate(),
    )
    .map_err(|source| SupervisorError::Control(format!("install SIGTERM handler: {source}")))?;
    let mut interrupt_signal = tokio::signal::unix::signal(
        tokio::signal::unix::SignalKind::interrupt(),
    )
    .map_err(|source| SupervisorError::Control(format!("install SIGINT handler: {source}")))?;
    let mut restart_count = 0_u32;
    loop {
        let event = tokio::select! {
            accepted = listener.accept() => {
                let (stream, _) = accepted.map_err(|source| SupervisorError::Io {
                    path: discovery.supervisor_socket.clone(),
                    source,
                })?;
                let credentials = match stream.peer_cred() {
                    Ok(credentials) => credentials,
                    Err(_) => continue,
                };
                if credentials.uid() != allowed_uid {
                    continue;
                }
                let Ok(permit) = connection_slots.clone().try_acquire_owned() else {
                    continue;
                };
                let launch_policies = launch_policies.clone();
                let launches = launches.clone();
                let query_socket = query_socket.clone();
                let serving = serving.clone();
                let escalation_sender = escalation_sender.clone();
                let peer_uid = credentials.uid();
                let peer_pid = credentials.pid().and_then(|value| u32::try_from(value).ok());
                connection_sequence = connection_sequence.checked_add(1).ok_or_else(|| {
                    SupervisorError::Control("supervisor connection sequence exhausted".into())
                })?;
                let cancellation = connection_tasks.clone();
                connection_tasks
                    .spawn(&format!("connection:{connection_sequence}"), async move {
                        let _permit = permit;
                        tokio::select! {
                            () = cancellation.cancelled() => {}
                            _ = serve_supervisor_connection(
                                stream,
                                &launch_policies,
                                &launches,
                                &query_socket,
                                &serving,
                                supervisor_generation,
                                workspace_id,
                                peer_uid,
                                peer_pid,
                                &escalation_sender,
                            ) => {}
                        }
                    })
                    .await
                    .map_err(|error| {
                        SupervisorError::Control(format!(
                            "supervisor connection ownership: {error}"
                        ))
                    })?;
                SupervisorLoopEvent::Continue
            }
            status = daemon.child.wait() => {
                let status = status.map_err(|source| SupervisorError::Child(source.to_string()))?;
                SupervisorLoopEvent::DaemonExited(status.to_string())
            }
            _ = launch_reaper.tick() => {
                // A failed exact revocation retains the slot and session fail-closed. Daemon
                // control loss is handled by the owned-child restart branch.
                match reap_abandoned_launches(&launches, &daemon.control).await {
                    Err(error) if error.daemon_control_stage().is_some() => {
                        SupervisorLoopEvent::DaemonControlFailed(DaemonControlEscalation {
                            daemon_generation: daemon.generation,
                            stage: error.daemon_control_stage().expect("matched daemon stage"),
                        })
                    }
                    _ => SupervisorLoopEvent::Continue,
                }
            }
            escalation = escalation_receiver.recv() => match escalation {
                Some(escalation) => SupervisorLoopEvent::DaemonControlFailed(escalation),
                None => SupervisorLoopEvent::Continue,
            },
            _ = terminate_signal.recv() => SupervisorLoopEvent::Shutdown,
            _ = interrupt_signal.recv() => SupervisorLoopEvent::Shutdown,
        };
        match event {
            SupervisorLoopEvent::Continue => continue,
            SupervisorLoopEvent::Shutdown => break,
            SupervisorLoopEvent::DaemonExited(status) => {
                close_failed_daemon_generation(&serving, &launches, daemon.generation).await;
                restart_count = restart_count.saturating_add(1);
                if restart_count > MAX_DAEMON_RESTARTS {
                    remove_owned_file_at(&lease.directory, SUPERVISOR_DISCOVERY, &discovery_path)?;
                    socket.retire()?;
                    return Err(SupervisorError::Child(format!(
                        "codefabricd exhausted the bounded restart budget after {status}"
                    )));
                }
            }
            SupervisorLoopEvent::DaemonControlFailed(escalation) => {
                if escalation.daemon_generation != daemon.generation {
                    continue;
                }
                close_failed_daemon_generation(&serving, &launches, escalation.daemon_generation)
                    .await;
                restart_count = restart_count.saturating_add(1);
                if restart_count > MAX_DAEMON_RESTARTS {
                    remove_owned_file_at(&lease.directory, SUPERVISOR_DISCOVERY, &discovery_path)?;
                    socket.retire()?;
                    return Err(SupervisorError::Child(format!(
                        "codefabricd exhausted the bounded restart budget after daemon control failure at {:?}",
                        escalation.stage
                    )));
                }
                terminate_and_join_child(
                    &mut daemon.child,
                    "unavailable authenticated daemon control generation",
                )
                .await?;
            }
        }
        let next_generation = daemon
            .generation
            .checked_add(1)
            .ok_or_else(|| SupervisorError::Control("daemon generation exhausted".into()))?;
        daemon = SupervisedDaemon::spawn(
            config_path,
            workspace_id,
            next_generation,
            supervisor_generation,
        )
        .await?;
        let next_pid = daemon
            .child
            .id()
            .ok_or_else(|| SupervisorError::Child("restarted daemon PID unavailable".into()))?;
        let next_discovery = SupervisorDiscovery {
            format: discovery.format.clone(),
            supervisor_socket: discovery.supervisor_socket.clone(),
            query_socket: discovery.query_socket.clone(),
            supervisor_generation,
            daemon_generation: next_generation,
            daemon_pid: next_pid,
        };
        write_private_json_at(
            &lease.directory,
            SUPERVISOR_DISCOVERY,
            &next_discovery,
            &discovery_path,
        )?;
        let mut serving = serving.write().await;
        serving.ready = true;
        serving.daemon_pid = next_pid;
        serving.daemon_generation = next_generation;
        serving.control = daemon.control.clone();
    }
    serving.write().await.ready = false;
    let connection_shutdown = connection_tasks
        .cancel_and_join(SUPERVISOR_RENDEZVOUS_HANDLE_TIMEOUT)
        .await
        .map_err(|error| SupervisorError::Control(format!("supervisor connection drain: {error}")));
    let shutdown_result = drain_shutdown_and_join_daemon(&mut daemon, &launches).await;
    if shutdown_result.is_err() {
        let _ = terminate_and_join_child(&mut daemon.child, "failed ordered daemon drain").await;
    }
    let discovery_cleanup =
        remove_owned_file_at(&lease.directory, SUPERVISOR_DISCOVERY, &discovery_path);
    let socket_cleanup = socket.retire().map_err(SupervisorError::from);
    connection_shutdown?;
    shutdown_result?;
    discovery_cleanup?;
    socket_cleanup?;
    Ok(())
}

async fn drain_shutdown_and_join_daemon(
    daemon: &mut SupervisedDaemon,
    launches: &Mutex<SupervisorLaunchRegistry>,
) -> Result<(), SupervisorError> {
    retire_all_launches_for_shutdown(launches, &daemon.control).await?;
    daemon.control.drain_accepted_work().await?;
    // Shutdown is queued only after the authenticated drain acknowledgement. The daemon closes
    // query admission and drains workspace publication before reading and acknowledging this
    // second record; both stages have the explicit finite shutdown allowance.
    let shutdown = DaemonControlRequest::Shutdown {
        request_id: request_id("shutdown")?,
    };
    let acknowledgement = daemon
        .control
        .transact_with_timeout(&shutdown, DAEMON_WORKSPACE_SHUTDOWN_TIMEOUT)
        .await?;
    if !acknowledgement.accepted || acknowledgement.code != "SHUTDOWN_ACCEPTED" {
        return Err(SupervisorError::Control(acknowledgement.code));
    }
    let status = tokio::time::timeout(DAEMON_WORKSPACE_SHUTDOWN_TIMEOUT, daemon.child.wait())
        .await
        .map_err(|_| SupervisorError::Child("daemon joined-shutdown deadline exceeded".into()))?
        .map_err(|source| SupervisorError::Child(source.to_string()))?;
    if !status.success() {
        return Err(SupervisorError::Child(format!(
            "daemon shutdown failed: {status}"
        )));
    }
    Ok(())
}

async fn retire_all_launches_for_shutdown(
    launches: &Mutex<SupervisorLaunchRegistry>,
    control: &DaemonControlClient,
) -> Result<(), SupervisorError> {
    let candidates = {
        let mut registry = launches.lock().await;
        registry.pending.clear();
        registry
            .active
            .values()
            .chain(registry.activating.values())
            .cloned()
            .map(|launch| (launch.grant_id.clone(), launch))
            .collect::<BTreeMap<_, _>>()
            .into_values()
            .collect::<Vec<_>>()
    };
    for active in &candidates {
        terminate_exact_adapter_group(active).await?;
        let request = DaemonControlRequest::RevokeLaunch {
            request_id: request_id("shutdown-revoke")?,
            grant_id: active.grant_id.clone(),
            grant_digest: active.grant_digest,
            adapter_pid: active.adapter_pid,
            adapter_start_identity: active.adapter_start_identity.clone(),
        };
        let acknowledgement = control.transact(&request).await?;
        if !acknowledgement.accepted {
            return Err(SupervisorError::Control(acknowledgement.code));
        }
    }
    let mut registry = launches.lock().await;
    for active in candidates {
        registry.active.remove(&active.grant_id);
        registry.activating.remove(&active.grant_id);
        registry.record_released(active);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn serve_supervisor_connection(
    mut stream: UnixStream,
    launch_policies: &AgentLaunchPolicyStore,
    launches: &Mutex<SupervisorLaunchRegistry>,
    query_socket: &Path,
    serving: &RwLock<SupervisorServingState>,
    supervisor_generation: u64,
    workspace_id: [u8; 16],
    peer_uid: u32,
    peer_pid: Option<u32>,
    escalation_sender: &mpsc::UnboundedSender<DaemonControlEscalation>,
) -> Result<(), SupervisorError> {
    let request =
        read_supervisor_request_with_timeout(&mut stream, SUPERVISOR_RENDEZVOUS_IO_TIMEOUT).await?;
    let snapshot = serving.read().await.snapshot();
    let (response, daemon_control_stage) = tokio::time::timeout(
        SUPERVISOR_RENDEZVOUS_HANDLE_TIMEOUT,
        handle_supervisor_request(
            request,
            launch_policies,
            &snapshot.control,
            launches,
            query_socket,
            snapshot.daemon_pid,
            snapshot.daemon_generation,
            supervisor_generation,
            workspace_id,
            peer_uid,
            peer_pid,
            snapshot.ready,
        ),
    )
    .await
    .map_err(|_| SupervisorError::IoDeadlineExceeded {
        stage: SupervisorIoStage::RendezvousHandle,
    })?;
    if let Some(stage) = daemon_control_stage {
        let _ = escalation_sender.send(DaemonControlEscalation {
            daemon_generation: snapshot.daemon_generation,
            stage,
        });
    }
    write_supervisor_response_with_timeout(
        &mut stream,
        &response,
        SUPERVISOR_RENDEZVOUS_IO_TIMEOUT,
    )
    .await?;
    Ok(())
}

async fn read_supervisor_request_with_timeout(
    stream: &mut UnixStream,
    timeout: Duration,
) -> Result<SupervisorRequest, SupervisorError> {
    read_rendezvous_line_with_timeout(stream, timeout).await
}

async fn read_rendezvous_line_with_timeout<T: for<'de> Deserialize<'de>>(
    stream: &mut UnixStream,
    timeout: Duration,
) -> Result<T, SupervisorError> {
    tokio::time::timeout(timeout, read_line(stream, SUPERVISOR_MAX_BYTES))
        .await
        .map_err(|_| SupervisorError::IoDeadlineExceeded {
            stage: SupervisorIoStage::RendezvousRead,
        })?
}

async fn write_supervisor_response_with_timeout(
    stream: &mut UnixStream,
    response: &SupervisorResponse,
    timeout: Duration,
) -> Result<(), SupervisorError> {
    write_rendezvous_line_with_timeout(stream, response, timeout).await
}

async fn write_rendezvous_line_with_timeout<T: Serialize>(
    stream: &mut UnixStream,
    value: &T,
    timeout: Duration,
) -> Result<(), SupervisorError> {
    tokio::time::timeout(timeout, write_line(stream, value, SUPERVISOR_MAX_BYTES))
        .await
        .map_err(|_| SupervisorError::IoDeadlineExceeded {
            stage: SupervisorIoStage::RendezvousWrite,
        })?
}

#[allow(clippy::too_many_arguments)]
async fn handle_supervisor_request(
    request: SupervisorRequest,
    launch_policies: &AgentLaunchPolicyStore,
    control: &DaemonControlClient,
    launches: &Mutex<SupervisorLaunchRegistry>,
    query_socket: &Path,
    daemon_pid: u32,
    daemon_generation: u64,
    supervisor_generation: u64,
    workspace_id: [u8; 16],
    peer_uid: u32,
    peer_pid: Option<u32>,
    admission_ready: bool,
) -> (SupervisorResponse, Option<SupervisorIoStage>) {
    let rejected = |code: &str| SupervisorResponse {
        accepted: false,
        code: code.to_owned(),
        preparation: None,
        launch: None,
        daemon_pid,
        daemon_generation,
        supervisor_generation,
    };
    if !admission_ready || !control.is_available() {
        return (rejected("SUPERVISOR_NOT_READY"), None);
    }
    match request {
        SupervisorRequest::Status => (
            SupervisorResponse {
                accepted: true,
                code: "READY".to_owned(),
                preparation: None,
                launch: None,
                daemon_pid,
                daemon_generation,
                supervisor_generation,
            },
            None,
        ),
        SupervisorRequest::ActivateLaunch {
            launch_id,
            adapter_pid,
        } => {
            let result = activate_launch(
                launches,
                control,
                &launch_id,
                adapter_pid,
                peer_uid,
                peer_pid,
            )
            .await;
            match result {
                Ok(launch) => (
                    SupervisorResponse {
                        accepted: true,
                        code: "LAUNCH_REGISTERED".to_owned(),
                        preparation: None,
                        launch: Some(launch),
                        daemon_pid,
                        daemon_generation,
                        supervisor_generation,
                    },
                    None,
                ),
                Err(error) => {
                    let stage = error.daemon_control_stage();
                    (rejected(error.public_code()), stage)
                }
            }
        }
        SupervisorRequest::CancelLaunch { launch_id } => {
            let result = cancel_launch(launches, &launch_id, peer_uid, peer_pid).await;
            match result {
                Ok(()) => (
                    SupervisorResponse {
                        accepted: true,
                        code: "LAUNCH_CANCELLED".to_owned(),
                        preparation: None,
                        launch: None,
                        daemon_pid,
                        daemon_generation,
                        supervisor_generation,
                    },
                    None,
                ),
                Err(error) => (rejected(error.public_code()), None),
            }
        }
        SupervisorRequest::ReleaseLaunch {
            launch_id,
            adapter_pid,
        } => {
            let result = release_launch(
                launches,
                control,
                &launch_id,
                adapter_pid,
                peer_uid,
                peer_pid,
            )
            .await;
            match result {
                Ok(()) => (
                    SupervisorResponse {
                        accepted: true,
                        code: "LAUNCH_RELEASED".to_owned(),
                        preparation: None,
                        launch: None,
                        daemon_pid,
                        daemon_generation,
                        supervisor_generation,
                    },
                    None,
                ),
                Err(error) => {
                    let stage = error.daemon_control_stage();
                    (rejected(error.public_code()), stage)
                }
            }
        }
        SupervisorRequest::Launch {
            policy_id,
            request_id,
            issued_at_unix_ms,
            expires_at_unix_ms,
        } => {
            let policy = match launch_policies.load(&policy_id) {
                Ok(policy) => policy,
                Err(error) => return (rejected(error.public_code()), None),
            };
            let observed_at = match unix_millis() {
                Ok(observed_at) => observed_at,
                Err(error) => return (rejected(error.public_code()), None),
            };
            let launcher_start_identity = match required_process_start_identity(peer_pid) {
                Ok(identity) => identity,
                Err(error) => return (rejected(error.public_code()), None),
            };
            if let Err(error) = launches.lock().await.accept_request(
                &policy_id,
                &request_id,
                issued_at_unix_ms,
                expires_at_unix_ms,
                observed_at,
                peer_uid,
                peer_pid,
                launcher_start_identity.clone(),
            ) {
                return (rejected(error.public_code()), None);
            }
            let result = prepare_launch(
                &policy,
                query_socket,
                &policy_id,
                workspace_id,
                peer_uid,
                peer_pid,
                launcher_start_identity,
                daemon_generation,
                supervisor_generation,
            );
            match result {
                Ok(pending) => {
                    let result = launches.lock().await.reserve(
                        pending,
                        policy.maximum_concurrent_launches,
                        observed_at,
                    );
                    match result {
                        Ok(preparation) => (
                            SupervisorResponse {
                                accepted: true,
                                code: "LAUNCH_PREPARED".to_owned(),
                                preparation: Some(preparation),
                                launch: None,
                                daemon_pid,
                                daemon_generation,
                                supervisor_generation,
                            },
                            None,
                        ),
                        Err(error) => (rejected(error.public_code()), None),
                    }
                }
                Err(error) => (rejected(error.public_code()), None),
            }
        }
    }
}

fn prepare_launch(
    policy: &AgentLaunchPolicy,
    query_socket: &Path,
    policy_id: &LaunchPolicyId,
    expected_workspace_id: [u8; 16],
    peer_uid: u32,
    peer_pid: Option<u32>,
    launcher_start_identity: Option<String>,
    daemon_generation: u64,
    supervisor_generation: u64,
) -> Result<PendingLaunch, SupervisorError> {
    if policy_id != &policy.policy_id {
        return Err(SupervisorError::Policy("unknown launch policy".into()));
    }
    let workspaces = policy
        .workspace_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let now = unix_millis()?;
    if now < policy.not_before_unix_ms || now >= policy.expires_at_unix_ms {
        return Err(SupervisorError::Policy(
            "launch policy is outside its immutable validity interval".into(),
        ));
    }
    let expires = now
        .checked_add(
            i64::try_from(policy.maximum_session_seconds.saturating_mul(1_000)).map_err(|_| {
                SupervisorError::Policy("session expiry exceeds the supported clock range".into())
            })?,
        )
        .ok_or_else(|| SupervisorError::Policy("session expiry overflow".into()))?
        .min(policy.expires_at_unix_ms);
    let grant_bytes = random32()?;
    let grant_digest = *blake3::hash(&grant_bytes).as_bytes();
    let workspace_ids = workspaces
        .iter()
        .map(|value| decode_public_id(IdentityDomain::Workspace, None, value))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SupervisorError::Policy(error.to_string()))?;
    if workspace_ids.as_slice() != [expected_workspace_id] {
        return Err(SupervisorError::Policy(
            "selected policy does not authorize this supervisor workspace".into(),
        ));
    }
    let principal_id = decode_public_id(IdentityDomain::Owner, None, &policy.principal_id)
        .map_err(|error| SupervisorError::Policy(error.to_string()))?;
    let launch_id = request_id("launch")?;
    let grant = RegisteredLaunchGrant {
        grant_id: launch_id.clone(),
        grant_digest,
        policy_id: policy.policy_id.clone(),
        policy_revision: policy.policy_revision,
        revocation_generation: policy.revocation_generation,
        principal_id,
        workspace_ids,
        operations: policy.operations.clone(),
        semantic_profiles: policy.semantic_profiles.clone(),
        maximum_resource_chunk_bytes: policy.maximum_resource_chunk_bytes,
        maximum_result_bytes: policy.maximum_result_bytes,
        maximum_result_pages: policy.maximum_result_pages,
        maximum_request_state_ttl_seconds: policy.maximum_request_state_ttl_seconds,
        issued_at_unix_ms: now,
        expires_at_unix_ms: expires,
        daemon_generation,
        supervisor_generation,
        peer_uid,
        peer_pid: None,
        peer_start_identity: None,
    };
    Ok(PendingLaunch {
        grant_bytes,
        grant,
        query_socket: query_socket.to_owned(),
        preparation: AdapterLaunchPreparation {
            launch_id,
            adapter_program: policy.adapter_program.clone(),
            adapter_arguments: policy.adapter_arguments.clone(),
            adapter_distribution: policy.adapter_distribution.clone(),
            adapter_distribution_version: policy.adapter_distribution_version.clone(),
            adapter_executable_digest: policy.adapter_executable_digest.clone(),
            daemon_generation,
            supervisor_generation,
        },
        launcher_uid: peer_uid,
        launcher_pid: peer_pid,
        launcher_start_identity,
    })
}

async fn activate_launch(
    launches: &Mutex<SupervisorLaunchRegistry>,
    control: &DaemonControlClient,
    launch_id: &str,
    adapter_pid: u32,
    peer_uid: u32,
    peer_pid: Option<u32>,
) -> Result<AdapterLaunchEnvelope, SupervisorError> {
    if adapter_pid <= 1 {
        return Err(SupervisorError::Policy("invalid adapter PID".into()));
    }
    let observed_at = unix_millis()?;
    let adapter_start_identity = required_process_start_identity(Some(adapter_pid))?;
    let (pending, active) = {
        let mut registry = launches.lock().await;
        registry.prune_expired_pending(observed_at);
        let pending = registry
            .pending
            .get(launch_id)
            .ok_or_else(|| SupervisorError::Policy("unknown or consumed launch".into()))?;
        if pending.launcher_uid != peer_uid
            || pending.launcher_pid != peer_pid
            || pending.launcher_start_identity != required_process_start_identity(peer_pid)?
        {
            return Err(SupervisorError::Policy("launcher identity changed".into()));
        }
        let mut pending = registry
            .pending
            .remove(launch_id)
            .expect("validated pending launch remains reserved");
        pending.grant.peer_pid = Some(adapter_pid);
        pending.grant.peer_start_identity = adapter_start_identity.clone();
        let active = ActiveLaunch {
            grant_id: pending.grant.grant_id.clone(),
            grant_digest: pending.grant.grant_digest,
            policy_id: pending.grant.policy_id.clone(),
            adapter_pid,
            launcher_uid: pending.launcher_uid,
            launcher_pid: pending.launcher_pid,
            launcher_start_identity: pending.launcher_start_identity.clone(),
            adapter_start_identity,
            expires_at_unix_ms: pending.grant.expires_at_unix_ms,
            daemon_generation: pending.grant.daemon_generation,
        };
        if registry
            .activating
            .insert(launch_id.to_owned(), active.clone())
            .is_some()
        {
            return Err(SupervisorError::Control(
                "activating launch identity collision".into(),
            ));
        }
        (pending, active)
    };
    let session_expires_at_unix_ms = pending.grant.expires_at_unix_ms;
    let maximum_request_state_ttl_seconds = pending.grant.maximum_request_state_ttl_seconds;
    let request = DaemonControlRequest::RegisterLaunchGrant {
        request_id: match request_id("register") {
            Ok(request_id) => request_id,
            Err(error) => {
                launches.lock().await.activating.remove(launch_id);
                return Err(error);
            }
        },
        grant: pending.grant.clone(),
    };
    let acknowledgement = match control.transact(&request).await {
        Ok(acknowledgement) => acknowledgement,
        Err(error) => {
            // An absent acknowledgement is an uncertain daemon outcome. Retaining the activating
            // slot is the fail-closed capacity posture until the daemon generation is replaced.
            return Err(error);
        }
    };
    if !acknowledgement.accepted {
        launches.lock().await.activating.remove(launch_id);
        return Err(SupervisorError::Control(acknowledgement.code));
    }
    let mut registry = launches.lock().await;
    if registry.activating.get(launch_id) != Some(&active)
        || registry.active.contains_key(launch_id)
    {
        return Err(SupervisorError::Control(
            "activating launch changed before daemon registration acknowledgement".into(),
        ));
    }
    registry.activating.remove(launch_id);
    registry.active.insert(launch_id.to_owned(), active);
    drop(registry);
    Ok(AdapterLaunchEnvelope {
        format: "codefabric.adapter-launch.v1".to_owned(),
        query_socket: pending.query_socket,
        launch_grant_hex: hex(&pending.grant_bytes),
        adapter_program: pending.preparation.adapter_program,
        adapter_arguments: pending.preparation.adapter_arguments,
        daemon_generation: pending.preparation.daemon_generation,
        supervisor_generation: pending.preparation.supervisor_generation,
        session_expires_at_unix_ms,
        maximum_request_state_ttl_seconds,
    })
}

async fn release_launch(
    launches: &Mutex<SupervisorLaunchRegistry>,
    control: &DaemonControlClient,
    launch_id: &str,
    adapter_pid: u32,
    peer_uid: u32,
    peer_pid: Option<u32>,
) -> Result<(), SupervisorError> {
    if adapter_pid <= 1 {
        return Err(SupervisorError::Policy("invalid adapter PID".into()));
    }
    let observed_launcher_start = required_process_start_identity(peer_pid)?;
    let active = {
        let registry = launches.lock().await;
        let active = registry
            .active
            .get(launch_id)
            .or_else(|| registry.activating.get(launch_id))
            .cloned();
        if active.is_none()
            && registry.released.iter().any(|released| {
                released.grant_id == launch_id
                    && released.adapter_pid == adapter_pid
                    && released.launcher_uid == peer_uid
                    && released.launcher_pid == peer_pid
                    && released.launcher_start_identity == observed_launcher_start
            })
        {
            return Ok(());
        }
        active.ok_or_else(|| SupervisorError::Policy("unknown or released launch".into()))?
    };
    if active.grant_id != launch_id
        || active.adapter_pid != adapter_pid
        || active.launcher_uid != peer_uid
        || active.launcher_pid != peer_pid
        || active.launcher_start_identity != observed_launcher_start
        || active.daemon_generation != control.daemon_generation
    {
        return Err(SupervisorError::Policy(
            "active launch authority binding changed".into(),
        ));
    }
    let request = DaemonControlRequest::RevokeLaunch {
        request_id: request_id("release")?,
        grant_id: active.grant_id.clone(),
        grant_digest: active.grant_digest,
        adapter_pid: active.adapter_pid,
        adapter_start_identity: active.adapter_start_identity.clone(),
    };
    let acknowledgement = control.transact(&request).await?;
    if !acknowledgement.accepted {
        return Err(SupervisorError::Control(acknowledgement.code));
    }
    let mut registry = launches.lock().await;
    if registry.active.get(launch_id) != Some(&active)
        && registry.activating.get(launch_id) != Some(&active)
    {
        return Err(SupervisorError::Control(
            "active launch changed during exact daemon revocation".into(),
        ));
    }
    registry.active.remove(launch_id);
    registry.activating.remove(launch_id);
    registry.record_released(active);
    Ok(())
}

async fn cancel_launch(
    launches: &Mutex<SupervisorLaunchRegistry>,
    launch_id: &str,
    peer_uid: u32,
    peer_pid: Option<u32>,
) -> Result<(), SupervisorError> {
    let mut registry = launches.lock().await;
    let pending = registry
        .pending
        .get(launch_id)
        .ok_or_else(|| SupervisorError::Policy("unknown or consumed launch".into()))?;
    if pending.launcher_uid != peer_uid
        || pending.launcher_pid != peer_pid
        || pending.launcher_start_identity != required_process_start_identity(peer_pid)?
    {
        return Err(SupervisorError::Policy(
            "pending launch authority binding changed".into(),
        ));
    }
    registry.pending.remove(launch_id);
    Ok(())
}

async fn reap_abandoned_launches(
    launches: &Mutex<SupervisorLaunchRegistry>,
    control: &DaemonControlClient,
) -> Result<(), SupervisorError> {
    let observed_at = unix_millis()?;
    let candidates = {
        let mut registry = launches.lock().await;
        registry.prune_expired_pending(observed_at);
        registry
            .active
            .values()
            .filter(|active| {
                observed_at >= active.expires_at_unix_ms
                    || !process_identity_matches(
                        active.launcher_pid,
                        active.launcher_start_identity.as_deref(),
                    )
                    || !process_identity_matches(
                        Some(active.adapter_pid),
                        active.adapter_start_identity.as_deref(),
                    )
            })
            .cloned()
            .collect::<Vec<_>>()
    };
    for active in candidates {
        let adapter_is_live = process_identity_matches(
            Some(active.adapter_pid),
            active.adapter_start_identity.as_deref(),
        );
        if adapter_is_live {
            terminate_exact_adapter_group(&active).await?;
        }
        let request = DaemonControlRequest::RevokeLaunch {
            request_id: request_id("reap")?,
            grant_id: active.grant_id.clone(),
            grant_digest: active.grant_digest,
            adapter_pid: active.adapter_pid,
            adapter_start_identity: active.adapter_start_identity.clone(),
        };
        let acknowledgement = control.transact(&request).await?;
        if !acknowledgement.accepted {
            return Err(SupervisorError::Control(acknowledgement.code));
        }
        let mut registry = launches.lock().await;
        if registry.active.get(&active.grant_id) == Some(&active) {
            registry.active.remove(&active.grant_id);
            registry.record_released(active);
        }
    }
    Ok(())
}

async fn terminate_exact_adapter_group(active: &ActiveLaunch) -> Result<(), SupervisorError> {
    if !process_identity_matches(
        Some(active.adapter_pid),
        active.adapter_start_identity.as_deref(),
    ) {
        return Ok(());
    }
    let pid = i32::try_from(active.adapter_pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
        .ok_or_else(|| SupervisorError::Child("invalid active adapter process group".into()))?;
    match rustix::process::kill_process_group(pid, rustix::process::Signal::TERM) {
        Ok(()) | Err(rustix::io::Errno::SRCH) => {}
        Err(error) => {
            return Err(SupervisorError::Child(format!(
                "terminate abandoned adapter process group: {error}"
            )));
        }
    }
    if wait_for_exact_process_exit(active, ABANDONED_ADAPTER_TERM_GRACE).await {
        return Ok(());
    }
    // Re-observe the exact PID/start tuple immediately before escalation.  A recycled PID must
    // never receive the kill intended for the abandoned adapter.
    if !process_identity_matches(
        Some(active.adapter_pid),
        active.adapter_start_identity.as_deref(),
    ) {
        return Ok(());
    }
    match rustix::process::kill_process_group(pid, rustix::process::Signal::KILL) {
        Ok(()) | Err(rustix::io::Errno::SRCH) => {}
        Err(error) => {
            return Err(SupervisorError::Child(format!(
                "kill abandoned adapter process group: {error}"
            )));
        }
    }
    if wait_for_exact_process_exit(active, ABANDONED_ADAPTER_KILL_GRACE).await {
        Ok(())
    } else {
        Err(SupervisorError::Child(
            "abandoned adapter retained its exact PID/start identity after TERM and KILL".into(),
        ))
    }
}

async fn wait_for_exact_process_exit(active: &ActiveLaunch, maximum_wait: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + maximum_wait;
    loop {
        if !process_identity_matches(
            Some(active.adapter_pid),
            active.adapter_start_identity.as_deref(),
        ) {
            return true;
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return false;
        }
        tokio::time::sleep(ABANDONED_ADAPTER_EXIT_POLL.min(deadline - now)).await;
    }
}

/// Attach to an existing supervisor and launch the policy-selected installed FastMCP adapter.
pub async fn serve_mcp_launcher(
    discovery_path: &Path,
    policy_id: &LaunchPolicyId,
) -> Result<i32, SupervisorError> {
    let discovery: SupervisorDiscovery = read_private_json(discovery_path)?;
    let preparation = request_launch_preparation(&discovery.supervisor_socket, policy_id).await?;
    let launch_id = preparation.launch_id.clone();
    match launch_adapter(preparation, &discovery.supervisor_socket, policy_id).await {
        Ok(status) => Ok(status),
        Err(error) => {
            let _ = cancel_prepared_launch(&discovery.supervisor_socket, &launch_id).await;
            Err(error)
        }
    }
}

/// Send one bounded operator control request to an existing supervisor.
///
/// This route never starts a daemon and never exposes launch preparation or capability material.
pub async fn administer_supervisor(
    discovery_path: &Path,
    command: SupervisorControlCommand,
) -> Result<SupervisorControlStatus, SupervisorError> {
    let discovery: SupervisorDiscovery = read_private_json(discovery_path)?;
    let mut stream = UnixStream::connect(&discovery.supervisor_socket)
        .await
        .map_err(|source| SupervisorError::Io {
            path: discovery.supervisor_socket.clone(),
            source,
        })?;
    let request = match command {
        SupervisorControlCommand::Status => SupervisorRequest::Status,
    };
    write_rendezvous_line_with_timeout(&mut stream, &request, SUPERVISOR_RENDEZVOUS_IO_TIMEOUT)
        .await?;
    let response: SupervisorResponse =
        read_rendezvous_line_with_timeout(&mut stream, SUPERVISOR_RENDEZVOUS_IO_TIMEOUT).await?;
    Ok(SupervisorControlStatus {
        accepted: response.accepted,
        code: response.code,
        daemon_pid: response.daemon_pid,
        daemon_generation: response.daemon_generation,
        supervisor_generation: response.supervisor_generation,
    })
}

async fn request_launch_preparation(
    supervisor_socket: &Path,
    policy_id: &LaunchPolicyId,
) -> Result<AdapterLaunchPreparation, SupervisorError> {
    let mut stream = UnixStream::connect(supervisor_socket)
        .await
        .map_err(|source| SupervisorError::Io {
            path: supervisor_socket.to_owned(),
            source,
        })?;
    let issued_at_unix_ms = unix_millis()?;
    let expires_at_unix_ms = issued_at_unix_ms
        .checked_add(LAUNCH_REQUEST_MAXIMUM_LIFETIME_MS)
        .ok_or_else(|| SupervisorError::Control("launch request expiry overflow".into()))?;
    let request = SupervisorRequest::Launch {
        policy_id: policy_id.clone(),
        request_id: request_id("attach")?,
        issued_at_unix_ms,
        expires_at_unix_ms,
    };
    write_rendezvous_line_with_timeout(&mut stream, &request, SUPERVISOR_RENDEZVOUS_IO_TIMEOUT)
        .await?;
    let response: SupervisorResponse =
        read_rendezvous_line_with_timeout(&mut stream, SUPERVISOR_RENDEZVOUS_IO_TIMEOUT).await?;
    if !response.accepted {
        return Err(SupervisorError::Policy(response.code));
    }
    let preparation = response.preparation.ok_or_else(|| {
        SupervisorError::Control("launch preparation acknowledgement omitted payload".into())
    })?;
    Ok(preparation)
}

async fn verify_adapter_identity(
    preparation: &AdapterLaunchPreparation,
) -> Result<AdapterProgramIdentity, SupervisorError> {
    let program = preparation.adapter_program.clone();
    let first = tokio::task::spawn_blocking(move || identify_adapter_program(&program))
        .await
        .map_err(|error| SupervisorError::Child(format!("adapter digest task: {error}")))??;
    if first.executable_digest != preparation.adapter_executable_digest {
        return Err(SupervisorError::Policy(
            "adapter executable identity does not match operator policy".into(),
        ));
    }
    let (observed_version, observed_identity) = observe_adapter_distribution(
        &preparation.adapter_program,
        &preparation.adapter_distribution,
        &preparation.adapter_executable_digest,
    )
    .await?;
    if observed_version != preparation.adapter_distribution_version {
        return Err(SupervisorError::Policy(
            "adapter distribution version does not match operator policy".into(),
        ));
    }
    let program = preparation.adapter_program.clone();
    let second = tokio::task::spawn_blocking(move || identify_adapter_program(&program))
        .await
        .map_err(|error| SupervisorError::Child(format!("adapter digest task: {error}")))??;
    if observed_identity != first || second != first {
        return Err(SupervisorError::Policy(
            "adapter program path or executable changed during distribution identity observation"
                .into(),
        ));
    }
    Ok(first)
}

async fn observe_adapter_distribution(
    program: &Path,
    distribution: &str,
    expected_digest: &str,
) -> Result<(String, AdapterProgramIdentity), SupervisorError> {
    let identity_program = program.to_owned();
    let before = tokio::task::spawn_blocking(move || identify_adapter_program(&identity_program))
        .await
        .map_err(|error| SupervisorError::Child(format!("adapter identity task: {error}")))??;
    if before.executable_digest != expected_digest {
        return Err(SupervisorError::Policy(
            "adapter executable identity does not match operator policy".into(),
        ));
    }
    let mut command = Command::new(program);
    command
        .args([
            "-I",
            "-c",
            "import importlib.metadata as m,sys; print(m.version(sys.argv[1]))",
            distribution,
        ])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command
        .spawn()
        .map_err(|source| SupervisorError::Child(format!("adapter identity probe: {source}")))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| SupervisorError::Child("adapter identity stdout unavailable".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| SupervisorError::Child("adapter identity stderr unavailable".into()))?;
    let stdout_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        BufReader::new(stdout)
            .take((ADAPTER_IDENTITY_OUTPUT_MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .await
            .map(|_| bytes)
    });
    let stderr_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        BufReader::new(stderr)
            .take((ADAPTER_IDENTITY_OUTPUT_MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .await
            .map(|_| bytes)
    });
    let status = if let Ok(status) =
        tokio::time::timeout(ADAPTER_IDENTITY_PROBE_TIMEOUT, child.wait()).await
    {
        status
            .map_err(|source| SupervisorError::Child(format!("adapter identity join: {source}")))?
    } else {
        terminate_and_join_child(&mut child, "adapter identity probe timeout").await?;
        return Err(SupervisorError::Policy(
            "adapter distribution identity observation timed out".into(),
        ));
    };
    let stdout = stdout_task
        .await
        .map_err(|error| SupervisorError::Child(format!("adapter identity stdout: {error}")))?
        .map_err(|source| SupervisorError::Child(format!("adapter identity stdout: {source}")))?;
    let stderr = stderr_task
        .await
        .map_err(|error| SupervisorError::Child(format!("adapter identity stderr: {error}")))?
        .map_err(|source| SupervisorError::Child(format!("adapter identity stderr: {source}")))?;
    if !status.success()
        || stdout.len() > ADAPTER_IDENTITY_OUTPUT_MAX_BYTES
        || stderr.len() > ADAPTER_IDENTITY_OUTPUT_MAX_BYTES
        || !stderr.is_empty()
    {
        return Err(SupervisorError::Policy(
            "adapter distribution identity observation failed".into(),
        ));
    }
    let version = std::str::from_utf8(&stdout)
        .map_err(|_| SupervisorError::Policy("adapter distribution version is not UTF-8".into()))?
        .trim();
    if !valid_distribution_identity(version) {
        return Err(SupervisorError::Policy(
            "adapter distribution version observation is invalid".into(),
        ));
    }
    let identity_program = program.to_owned();
    let after = tokio::task::spawn_blocking(move || identify_adapter_program(&identity_program))
        .await
        .map_err(|error| SupervisorError::Child(format!("adapter identity task: {error}")))??;
    if after != before {
        return Err(SupervisorError::Policy(
            "adapter program identity changed while observing the installed distribution".into(),
        ));
    }
    Ok((version.to_owned(), before))
}

fn adapter_executable_digest(path: &Path) -> Result<String, SupervisorError> {
    Ok(identify_adapter_program(path)?.executable_digest)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AdapterProgramIdentity {
    path_device: u64,
    path_inode: u64,
    path_uid: u32,
    path_gid: u32,
    path_mode: u32,
    path_size: u64,
    path_mtime: i64,
    path_mtime_nsec: i64,
    path_ctime: i64,
    path_ctime_nsec: i64,
    symlink_target: Option<PathBuf>,
    executable_device: u64,
    executable_inode: u64,
    executable_uid: u32,
    executable_gid: u32,
    executable_mode: u32,
    executable_size: u64,
    executable_mtime: i64,
    executable_mtime_nsec: u64,
    executable_ctime: i64,
    executable_ctime_nsec: u64,
    executable_digest: String,
}

fn identify_adapter_program(path: &Path) -> Result<AdapterProgramIdentity, SupervisorError> {
    let path_before = fs::symlink_metadata(path).map_err(|source| SupervisorError::Io {
        path: path.to_owned(),
        source,
    })?;
    if (!path_before.is_file() && !path_before.file_type().is_symlink())
        || (path_before.uid() != rustix::process::geteuid().as_raw() && path_before.uid() != 0)
    {
        return Err(SupervisorError::Policy(
            "adapter program path is not an owned regular file or symbolic link".into(),
        ));
    }
    let symlink_target_before = if path_before.file_type().is_symlink() {
        Some(fs::read_link(path).map_err(|source| SupervisorError::Io {
            path: path.to_owned(),
            source,
        })?)
    } else {
        None
    };
    let descriptor =
        open(path, OFlags::RDONLY | OFlags::CLOEXEC, Mode::empty()).map_err(|source| {
            SupervisorError::Io {
                path: path.to_owned(),
                source: source.into(),
            }
        })?;
    let opened = fstat(&descriptor).map_err(|source| SupervisorError::Io {
        path: path.to_owned(),
        source: source.into(),
    })?;
    if !FileType::from_raw_mode(opened.st_mode).is_file()
        || (opened.st_uid != rustix::process::geteuid().as_raw() && opened.st_uid != 0)
        || opened.st_size <= 0
        || u64::try_from(opened.st_size).unwrap_or(u64::MAX) > ADAPTER_EXECUTABLE_MAX_BYTES
        || opened.st_mode & 0o111 == 0
        || opened.st_mode & 0o7002 != 0
    {
        return Err(SupervisorError::Policy(
            "adapter executable is not a bounded owned regular file".into(),
        ));
    }
    let mut file = File::from(descriptor);
    let first_digest = hash_open_adapter_executable(&mut file, path)?;
    let middle = file.metadata().map_err(|source| SupervisorError::Io {
        path: path.to_owned(),
        source,
    })?;
    file.seek(SeekFrom::Start(0))
        .map_err(|source| SupervisorError::Io {
            path: path.to_owned(),
            source,
        })?;
    let second_digest = hash_open_adapter_executable(&mut file, path)?;
    let after = file.metadata().map_err(|source| SupervisorError::Io {
        path: path.to_owned(),
        source,
    })?;
    let path_after = fs::symlink_metadata(path).map_err(|source| SupervisorError::Io {
        path: path.to_owned(),
        source,
    })?;
    let symlink_target_after = if path_after.file_type().is_symlink() {
        Some(fs::read_link(path).map_err(|source| SupervisorError::Io {
            path: path.to_owned(),
            source,
        })?)
    } else {
        None
    };
    let current =
        statat(rustix::fs::CWD, path, AtFlags::empty()).map_err(|source| SupervisorError::Io {
            path: path.to_owned(),
            source: source.into(),
        })?;
    if first_digest != second_digest
        || path_after.dev() != path_before.dev()
        || path_after.ino() != path_before.ino()
        || path_after.uid() != path_before.uid()
        || path_after.gid() != path_before.gid()
        || path_after.mode() != path_before.mode()
        || path_after.len() != path_before.len()
        || path_after.mtime() != path_before.mtime()
        || path_after.mtime_nsec() != path_before.mtime_nsec()
        || path_after.ctime() != path_before.ctime()
        || path_after.ctime_nsec() != path_before.ctime_nsec()
        || symlink_target_after != symlink_target_before
        || middle.dev() != opened.st_dev
        || middle.ino() != opened.st_ino
        || after.dev() != opened.st_dev
        || after.ino() != opened.st_ino
        || after.len() != u64::try_from(opened.st_size).unwrap_or(u64::MAX)
        || after.mtime() != middle.mtime()
        || after.mtime_nsec() != middle.mtime_nsec()
        || after.ctime() != middle.ctime()
        || after.ctime_nsec() != middle.ctime_nsec()
        || current.st_dev != opened.st_dev
        || current.st_ino != opened.st_ino
        || !FileType::from_raw_mode(current.st_mode).is_file()
        || current.st_uid != opened.st_uid
        || current.st_gid != opened.st_gid
        || current.st_mode & 0o7777 != opened.st_mode & 0o7777
        || current.st_size != opened.st_size
        || current.st_mtime != opened.st_mtime
        || current.st_mtime_nsec != opened.st_mtime_nsec
        || current.st_ctime != opened.st_ctime
        || current.st_ctime_nsec != opened.st_ctime_nsec
    {
        return Err(SupervisorError::Policy(
            "adapter executable changed during exact identity observation".into(),
        ));
    }
    Ok(AdapterProgramIdentity {
        path_device: path_before.dev(),
        path_inode: path_before.ino(),
        path_uid: path_before.uid(),
        path_gid: path_before.gid(),
        path_mode: path_before.mode(),
        path_size: path_before.len(),
        path_mtime: path_before.mtime(),
        path_mtime_nsec: path_before.mtime_nsec(),
        path_ctime: path_before.ctime(),
        path_ctime_nsec: path_before.ctime_nsec(),
        symlink_target: symlink_target_before,
        executable_device: opened.st_dev,
        executable_inode: opened.st_ino,
        executable_uid: opened.st_uid,
        executable_gid: opened.st_gid,
        executable_mode: opened.st_mode,
        executable_size: u64::try_from(opened.st_size).unwrap_or(u64::MAX),
        executable_mtime: opened.st_mtime,
        executable_mtime_nsec: opened.st_mtime_nsec,
        executable_ctime: opened.st_ctime,
        executable_ctime_nsec: opened.st_ctime_nsec,
        executable_digest: first_digest,
    })
}

fn hash_open_adapter_executable(file: &mut File, path: &Path) -> Result<String, SupervisorError> {
    let mut hasher = blake3::Hasher::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|source| SupervisorError::Io {
                path: path.to_owned(),
                source,
            })?;
        if count == 0 {
            break;
        }
        total = total.saturating_add(count as u64);
        if total > ADAPTER_EXECUTABLE_MAX_BYTES {
            return Err(SupervisorError::Policy(
                "adapter executable exceeds its identity bound".into(),
            ));
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("b3:{}", hex(hasher.finalize().as_bytes())))
}

async fn verify_running_adapter_identity(
    preparation: &AdapterLaunchPreparation,
    adapter_pid: u32,
) -> Result<(), SupervisorError> {
    #[cfg(target_os = "linux")]
    let executable = PathBuf::from(format!("/proc/{adapter_pid}/exe"));
    #[cfg(not(target_os = "linux"))]
    let executable = preparation.adapter_program.clone();
    let observed =
        tokio::task::spawn_blocking(move || running_adapter_executable_digest(&executable))
            .await
            .map_err(|error| {
                SupervisorError::Child(format!("running adapter digest task: {error}"))
            })??;
    if observed != preparation.adapter_executable_digest {
        return Err(SupervisorError::Policy(
            "spawned adapter executable identity does not match operator policy".into(),
        ));
    }
    Ok(())
}

async fn validate_generation_refresh_authority(
    original: &AdapterLaunchPreparation,
    replacement: &AdapterLaunchPreparation,
    observed_daemon_generation: u64,
    observed_supervisor_generation: u64,
    adapter_pid: u32,
) -> Result<(), SupervisorError> {
    if replacement.adapter_program != original.adapter_program
        || replacement.adapter_arguments != original.adapter_arguments
        || replacement.adapter_distribution != original.adapter_distribution
        || replacement.adapter_distribution_version != original.adapter_distribution_version
        || replacement.adapter_executable_digest != original.adapter_executable_digest
        || replacement.supervisor_generation != original.supervisor_generation
        || replacement.supervisor_generation != observed_supervisor_generation
        || replacement.daemon_generation != observed_daemon_generation
    {
        return Err(SupervisorError::Control(
            "replacement launch authority changed the running adapter contract".into(),
        ));
    }
    verify_running_adapter_identity(replacement, adapter_pid).await
}

fn running_adapter_executable_digest(path: &Path) -> Result<String, SupervisorError> {
    // The already-running executable is operator policy material, not workspace source input.
    // ast-grep-ignore: authoritative-source-read-boundary
    let mut file = File::open(path).map_err(|source| SupervisorError::Io {
        path: path.to_owned(),
        source,
    })?;
    let before = file.metadata().map_err(|source| SupervisorError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !before.is_file()
        || before.len() == 0
        || before.len() > ADAPTER_EXECUTABLE_MAX_BYTES
        || before.mode() & 0o111 == 0
    {
        return Err(SupervisorError::Policy(
            "running adapter is not a bounded executable regular file".into(),
        ));
    }
    let digest = hash_open_adapter_executable(&mut file, path)?;
    let after = file.metadata().map_err(|source| SupervisorError::Io {
        path: path.to_owned(),
        source,
    })?;
    if after.dev() != before.dev()
        || after.ino() != before.ino()
        || after.len() != before.len()
        || after.mtime() != before.mtime()
        || after.mtime_nsec() != before.mtime_nsec()
        || after.ctime() != before.ctime()
        || after.ctime_nsec() != before.ctime_nsec()
    {
        return Err(SupervisorError::Policy(
            "running adapter identity changed during observation".into(),
        ));
    }
    Ok(digest)
}

fn directional_adapter_launch_channel() -> Result<(StdUnixStream, OwnedFd), SupervisorError> {
    let (parent, child) = StdUnixStream::pair()
        .map_err(|source| SupervisorError::Child(format!("launch socketpair: {source}")))?;
    parent
        .shutdown(Shutdown::Read)
        .map_err(|source| SupervisorError::Child(format!("close launcher read half: {source}")))?;
    child
        .shutdown(Shutdown::Write)
        .map_err(|source| SupervisorError::Child(format!("close adapter write half: {source}")))?;
    Ok((parent, child.into()))
}

async fn launch_adapter(
    preparation: AdapterLaunchPreparation,
    supervisor_socket: &Path,
    policy_id: &LaunchPolicyId,
) -> Result<i32, SupervisorError> {
    let expected_program_identity = verify_adapter_identity(&preparation).await?;
    let (parent, child_fd) = directional_adapter_launch_channel()?;
    let mut command = Command::new(&preparation.adapter_program);
    command
        .args(&preparation.adapter_arguments)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    command
        .fd_mappings(vec![FdMapping {
            parent_fd: child_fd,
            child_fd: ADAPTER_LAUNCH_FD,
        }])
        .map_err(|error| SupervisorError::Child(error.to_string()))?;
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command
        .spawn()
        .map_err(|source| SupervisorError::Child(format!("spawn adapter: {source}")))?;
    let adapter_pid = child
        .id()
        .ok_or_else(|| SupervisorError::Child("adapter PID unavailable".into()))?;
    if let Err(error) = verify_running_adapter_identity(&preparation, adapter_pid).await {
        terminate_and_join_child(&mut child, "adapter identity rejection").await?;
        return Err(error);
    }
    let post_spawn_distribution = observe_adapter_distribution(
        &preparation.adapter_program,
        &preparation.adapter_distribution,
        &preparation.adapter_executable_digest,
    )
    .await;
    let (post_spawn_version, post_spawn_program_identity) = match post_spawn_distribution {
        Ok(observation) => observation,
        Err(error) => {
            terminate_and_join_child(&mut child, "adapter distribution rejection").await?;
            return Err(error);
        }
    };
    if post_spawn_version != preparation.adapter_distribution_version
        || post_spawn_program_identity != expected_program_identity
    {
        terminate_and_join_child(&mut child, "adapter identity substitution").await?;
        return Err(SupervisorError::Policy(
            "spawned adapter distribution or program identity changed before grant delivery".into(),
        ));
    }
    let launch = match activate_prepared_launch(
        supervisor_socket,
        &preparation.launch_id,
        adapter_pid,
    )
    .await
    {
        Ok(launch) => launch,
        Err(error) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            let _ = release_prepared_launch(supervisor_socket, &preparation.launch_id, adapter_pid)
                .await;
            return Err(error);
        }
    };
    let mut active_launch_id = preparation.launch_id.clone();
    let result = async {
        let mut parent = UnixStream::from_std({
            parent
                .set_nonblocking(true)
                .map_err(|source| SupervisorError::Child(source.to_string()))?;
            parent
        })
        .map_err(|source| SupervisorError::Child(source.to_string()))?;
        write_launch_envelope(&mut parent, &launch).await?;
        let mut daemon_generation = launch.daemon_generation;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| SupervisorError::Child("adapter stderr unavailable".into()))?;
        let diagnostic = tokio::spawn(async move {
            let mut bytes = Vec::new();
            BufReader::new(stderr)
                .take((ADAPTER_DIAGNOSTIC_MAX_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .await
                .map(|_| bytes)
        });
        let mut monitor = tokio::time::interval(Duration::from_millis(250));
        monitor.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut monitor_failures = 0_u32;
        let status = loop {
            monitor.tick().await;
            if let Some(status) = child
                .try_wait()
                .map_err(|source| SupervisorError::Child(source.to_string()))?
            {
                break status;
            }
            let status = match request_supervisor_status(supervisor_socket).await {
                Ok(status) => {
                    monitor_failures = 0;
                    status
                }
                Err(error) => {
                    monitor_failures = monitor_failures.saturating_add(1);
                    if monitor_failures < MAX_SUPERVISOR_MONITOR_FAILURES {
                        continue;
                    }
                    return Err(SupervisorError::Control(format!(
                        "adapter launch monitor exhausted its retry budget: {error}"
                    )));
                }
            };
            if status.daemon_generation == daemon_generation {
                continue;
            }
            if status.daemon_generation < daemon_generation
                || status.supervisor_generation != preparation.supervisor_generation
            {
                return Err(SupervisorError::Control(
                    "supervisor generation regressed during adapter execution".into(),
                ));
            }
            let replacement = request_launch_preparation(supervisor_socket, policy_id).await?;
            if let Err(error) = validate_generation_refresh_authority(
                &preparation,
                &replacement,
                status.daemon_generation,
                status.supervisor_generation,
                adapter_pid,
            )
            .await
            {
                let _ = cancel_prepared_launch(supervisor_socket, &replacement.launch_id).await;
                return Err(error);
            }
            let replacement_launch =
                activate_prepared_launch(supervisor_socket, &replacement.launch_id, adapter_pid)
                    .await?;
            active_launch_id.clone_from(&replacement.launch_id);
            write_launch_envelope(&mut parent, &replacement_launch).await?;
            daemon_generation = replacement_launch.daemon_generation;
        };
        parent
            .shutdown()
            .await
            .map_err(|source| SupervisorError::Child(source.to_string()))?;
        let diagnostics = diagnostic
            .await
            .map_err(|error| SupervisorError::Child(error.to_string()))?
            .map_err(|source| SupervisorError::Child(source.to_string()))?;
        if diagnostics.len() > ADAPTER_DIAGNOSTIC_MAX_BYTES {
            return Err(SupervisorError::Child(
                "adapter diagnostics exceeded bound".into(),
            ));
        }
        if !diagnostics.is_empty() {
            tokio::io::stderr()
                .write_all(&diagnostics)
                .await
                .map_err(|source| SupervisorError::Child(source.to_string()))?;
        }
        Ok(status.code().unwrap_or(1))
    }
    .await;
    if result.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    let release = release_prepared_launch(supervisor_socket, &active_launch_id, adapter_pid).await;
    match (result, release) {
        (Ok(status), Ok(())) => Ok(status),
        (Ok(_), Err(error)) | (Err(error), _) => Err(error),
    }
}

async fn request_supervisor_status(
    supervisor_socket: &Path,
) -> Result<SupervisorResponse, SupervisorError> {
    let mut stream = UnixStream::connect(supervisor_socket)
        .await
        .map_err(|source| SupervisorError::Io {
            path: supervisor_socket.to_owned(),
            source,
        })?;
    write_rendezvous_line_with_timeout(
        &mut stream,
        &SupervisorRequest::Status,
        SUPERVISOR_RENDEZVOUS_IO_TIMEOUT,
    )
    .await?;
    let response: SupervisorResponse =
        read_rendezvous_line_with_timeout(&mut stream, SUPERVISOR_RENDEZVOUS_IO_TIMEOUT).await?;
    if !response.accepted {
        return Err(SupervisorError::Control(response.code));
    }
    Ok(response)
}

async fn write_launch_envelope(
    stream: &mut UnixStream,
    launch: &AdapterLaunchEnvelope,
) -> Result<(), SupervisorError> {
    let mut bytes =
        serde_json::to_vec(launch).map_err(|error| SupervisorError::Control(error.to_string()))?;
    bytes.push(b'\n');
    if bytes.len() > SUPERVISOR_MAX_BYTES {
        return Err(SupervisorError::Control(
            "adapter launch envelope exceeds bound".into(),
        ));
    }
    stream
        .write_all(&bytes)
        .await
        .map_err(|source| SupervisorError::Child(source.to_string()))
}

async fn activate_prepared_launch(
    supervisor_socket: &Path,
    launch_id: &str,
    adapter_pid: u32,
) -> Result<AdapterLaunchEnvelope, SupervisorError> {
    let mut stream = UnixStream::connect(supervisor_socket)
        .await
        .map_err(|source| SupervisorError::Io {
            path: supervisor_socket.to_owned(),
            source,
        })?;
    write_rendezvous_line_with_timeout(
        &mut stream,
        &SupervisorRequest::ActivateLaunch {
            launch_id: launch_id.to_owned(),
            adapter_pid,
        },
        SUPERVISOR_RENDEZVOUS_IO_TIMEOUT,
    )
    .await?;
    let response: SupervisorResponse =
        read_rendezvous_line_with_timeout(&mut stream, SUPERVISOR_RENDEZVOUS_IO_TIMEOUT).await?;
    if !response.accepted {
        return Err(SupervisorError::Policy(response.code));
    }
    response.launch.ok_or_else(|| {
        SupervisorError::Control("registered launch omitted the adapter envelope".into())
    })
}

async fn release_prepared_launch(
    supervisor_socket: &Path,
    launch_id: &str,
    adapter_pid: u32,
) -> Result<(), SupervisorError> {
    let mut stream = UnixStream::connect(supervisor_socket)
        .await
        .map_err(|source| SupervisorError::Io {
            path: supervisor_socket.to_owned(),
            source,
        })?;
    write_rendezvous_line_with_timeout(
        &mut stream,
        &SupervisorRequest::ReleaseLaunch {
            launch_id: launch_id.to_owned(),
            adapter_pid,
        },
        SUPERVISOR_RENDEZVOUS_IO_TIMEOUT,
    )
    .await?;
    let response: SupervisorResponse =
        read_rendezvous_line_with_timeout(&mut stream, SUPERVISOR_RENDEZVOUS_IO_TIMEOUT).await?;
    if !response.accepted || response.code != "LAUNCH_RELEASED" {
        return Err(SupervisorError::Policy(response.code));
    }
    Ok(())
}

async fn cancel_prepared_launch(
    supervisor_socket: &Path,
    launch_id: &str,
) -> Result<(), SupervisorError> {
    let mut stream = UnixStream::connect(supervisor_socket)
        .await
        .map_err(|source| SupervisorError::Io {
            path: supervisor_socket.to_owned(),
            source,
        })?;
    write_rendezvous_line_with_timeout(
        &mut stream,
        &SupervisorRequest::CancelLaunch {
            launch_id: launch_id.to_owned(),
        },
        SUPERVISOR_RENDEZVOUS_IO_TIMEOUT,
    )
    .await?;
    let response: SupervisorResponse =
        read_rendezvous_line_with_timeout(&mut stream, SUPERVISOR_RENDEZVOUS_IO_TIMEOUT).await?;
    if !response.accepted || response.code != "LAUNCH_CANCELLED" {
        return Err(SupervisorError::Policy(response.code));
    }
    Ok(())
}

/// Convert the daemon's inherited stdin socket endpoint into a Tokio stream.
pub fn control_stream_from_stdin() -> Result<UnixStream, SupervisorError> {
    let owned = std::io::stdin()
        .as_fd()
        .try_clone_to_owned()
        .map_err(|error| SupervisorError::Control(format!("duplicate control stdin: {error}")))?;
    let stream = StdUnixStream::from(owned);
    stream
        .set_nonblocking(true)
        .map_err(|source| SupervisorError::Control(source.to_string()))?;
    UnixStream::from_std(stream).map_err(|source| SupervisorError::Control(source.to_string()))
}

/// Read and authenticate the mandatory first control record.
pub async fn accept_control_hello(
    stream: &mut UnixStream,
) -> Result<
    (
        DaemonControlHello,
        DaemonControlHeader,
        DaemonControlReadState,
    ),
    SupervisorError,
> {
    let credentials = stream
        .peer_cred()
        .map_err(|source| SupervisorError::Control(source.to_string()))?;
    let record: DaemonControlRecord = read_line(stream, CONTROL_MAX_BYTES).await?;
    let DaemonControlRequest::Hello(hello) = &record.request else {
        return Err(SupervisorError::Control(
            "first control record is not hello".into(),
        ));
    };
    let state = DaemonControlReadState::from_hello(&record)?;
    if hello.request_id.is_empty()
        || hello.daemon_generation == 0
        || hello.supervisor_generation == 0
        || hello.daemon_generation != record.header.daemon_generation
        || hello.supervisor_generation != record.header.supervisor_generation
        || hello.supervisor_uid != credentials.uid()
        || hello.supervisor_uid != rustix::process::geteuid().as_raw()
        || credentials
            .pid()
            .and_then(|value| u32::try_from(value).ok())
            .is_some_and(|pid| pid != hello.supervisor_pid)
    {
        return Err(SupervisorError::Control(
            "control hello identity mismatch".into(),
        ));
    }
    Ok((hello.clone(), record.header, state))
}

pub async fn acknowledge_control(
    stream: &mut UnixStream,
    state: &DaemonControlReadState,
    request: &DaemonControlHeader,
    request_id: &str,
    accepted: bool,
    code: &str,
) -> Result<(), SupervisorError> {
    if request.workspace_id != state.workspace_id
        || request.daemon_generation != state.daemon_generation
        || request.supervisor_generation != state.supervisor_generation
    {
        return Err(SupervisorError::Control(
            "control acknowledgement authority mismatch".into(),
        ));
    }
    write_line(
        stream,
        &DaemonControlAcknowledgement::new(&state.key, request, request_id, accepted, code)?,
        CONTROL_MAX_BYTES,
    )
    .await
}

pub async fn read_daemon_control(
    stream: &mut UnixStream,
    state: &mut DaemonControlReadState,
) -> Result<AcceptedDaemonControlRecord, SupervisorError> {
    let record: DaemonControlRecord = read_line(stream, CONTROL_MAX_BYTES).await?;
    state.accept(&record, unix_millis()?)?;
    Ok(AcceptedDaemonControlRecord {
        header: record.header,
        request: record.request,
    })
}

fn load_policy(path: &Path) -> Result<AgentLaunchPolicy, SupervisorError> {
    let policy: AgentLaunchPolicy = read_private_json(path)?;
    policy.validate()?;
    Ok(policy)
}

fn read_private_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, SupervisorError> {
    let bytes = read_private_control_artifact(path, SUPERVISOR_MAX_BYTES as u64)
        .map_err(|error| SupervisorError::Policy(error.to_string()))?;
    serde_json::from_slice(&bytes).map_err(|error| SupervisorError::Policy(error.to_string()))
}

fn write_private_json<T: Serialize>(path: &Path, value: &T) -> Result<(), SupervisorError> {
    let bytes = serde_json_canonicalizer::to_vec(value)
        .map_err(|error| SupervisorError::Control(error.to_string()))?;
    if bytes.len() > SUPERVISOR_MAX_BYTES {
        return Err(SupervisorError::Control(
            "private JSON exceeds bound".into(),
        ));
    }
    let temporary = path.with_extension(format!("tmp.{}", hex(&random32()?[..8])));
    // This is a private control-state writer; secure_path exclusively owns source reads.
    // ast-grep-ignore: authoritative-source-read-boundary
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|source| SupervisorError::Io {
            path: temporary.clone(),
            source,
        })?;
    use std::io::Write as _;
    if let Err(source) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(&temporary);
        return Err(SupervisorError::Io {
            path: temporary,
            source,
        });
    }
    if let Err(source) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(SupervisorError::Io {
            path: path.to_owned(),
            source,
        });
    }
    if let Some(parent) = path.parent() {
        // Directory fsync makes the private control rename durable; it does not read source.
        // ast-grep-ignore: authoritative-source-read-boundary
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|source| SupervisorError::Io {
                path: parent.to_owned(),
                source,
            })?;
    }
    Ok(())
}

fn write_private_json_at<T: Serialize>(
    directory: &OwnedFd,
    name: &str,
    value: &T,
    display_path: &Path,
) -> Result<(), SupervisorError> {
    let bytes = serde_json_canonicalizer::to_vec(value)
        .map_err(|error| SupervisorError::Control(error.to_string()))?;
    if bytes.len() > SUPERVISOR_MAX_BYTES {
        return Err(SupervisorError::Control(
            "private JSON exceeds bound".into(),
        ));
    }
    match statat(directory, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(current)
            if FileType::from_raw_mode(current.st_mode).is_file()
                && current.st_uid == rustix::process::geteuid().as_raw()
                && current.st_mode & 0o7777 == 0o600 => {}
        Ok(_) => {
            return Err(SupervisorError::Config(
                "private JSON replacement target is unsafe".into(),
            ));
        }
        Err(rustix::io::Errno::NOENT) => {}
        Err(source) => {
            return Err(SupervisorError::Io {
                path: display_path.to_owned(),
                source: source.into(),
            });
        }
    }
    let temporary = format!("{name}.tmp.{}", hex(&random32()?[..8]));
    let descriptor = openat(
        directory,
        temporary.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|source| SupervisorError::Io {
        path: display_path.to_owned(),
        source: source.into(),
    })?;
    let opened = fstat(&descriptor).map_err(|source| SupervisorError::Io {
        path: display_path.to_owned(),
        source: source.into(),
    })?;
    let mut file = File::from(descriptor);
    if let Err(source) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
        let _ = unlinkat(directory, temporary.as_str(), AtFlags::empty());
        return Err(SupervisorError::Io {
            path: display_path.to_owned(),
            source,
        });
    }
    let written = fstat(&file).map_err(|source| SupervisorError::Io {
        path: display_path.to_owned(),
        source: source.into(),
    })?;
    if let Err(source) = renameat(directory, temporary.as_str(), directory, name) {
        let _ = unlinkat(directory, temporary.as_str(), AtFlags::empty());
        return Err(SupervisorError::Io {
            path: display_path.to_owned(),
            source: source.into(),
        });
    }
    let current = statat(directory, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|source| {
        SupervisorError::Io {
            path: display_path.to_owned(),
            source: source.into(),
        }
    })?;
    if current.st_dev != opened.st_dev
        || current.st_ino != opened.st_ino
        || current.st_uid != opened.st_uid
        || current.st_mode & 0o7777 != 0o600
        || current.st_size != written.st_size
        || current.st_mtime != written.st_mtime
        || current.st_mtime_nsec != written.st_mtime_nsec
    {
        return Err(SupervisorError::Config(
            "private JSON durable readback identity changed".into(),
        ));
    }
    fsync(directory).map_err(|source| SupervisorError::Io {
        path: display_path.to_owned(),
        source: source.into(),
    })?;
    Ok(())
}

async fn read_line<T: for<'de> Deserialize<'de>>(
    stream: &mut UnixStream,
    maximum: usize,
) -> Result<T, SupervisorError> {
    let mut bytes = Vec::new();
    BufReader::new(stream)
        .take((maximum + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .await
        .map_err(|source| SupervisorError::Control(source.to_string()))?;
    if bytes.is_empty() || bytes.len() > maximum || bytes.last() != Some(&b'\n') {
        return Err(SupervisorError::Control(
            "bounded line framing rejected".into(),
        ));
    }
    serde_json::from_slice(&bytes).map_err(|error| SupervisorError::Control(error.to_string()))
}

async fn write_line<T: Serialize>(
    stream: &mut UnixStream,
    value: &T,
    maximum: usize,
) -> Result<(), SupervisorError> {
    let mut bytes =
        serde_json::to_vec(value).map_err(|error| SupervisorError::Control(error.to_string()))?;
    bytes.push(b'\n');
    if bytes.len() > maximum {
        return Err(SupervisorError::Control(
            "bounded line framing rejected".into(),
        ));
    }
    stream
        .write_all(&bytes)
        .await
        .map_err(|source| SupervisorError::Control(source.to_string()))
}

fn validate_private_directory(path: &Path) -> Result<(), SupervisorError> {
    let descriptor = open_absolute_directory_nofollow(path)
        .map_err(|error| SupervisorError::Config(error.to_string()))?;
    let metadata = fstat(&descriptor).map_err(|source| SupervisorError::Io {
        path: path.to_owned(),
        source: source.into(),
    })?;
    if !FileType::from_raw_mode(metadata.st_mode).is_dir()
        || metadata.st_uid != rustix::process::geteuid().as_raw()
        || metadata.st_mode & 0o777 != 0o700
    {
        return Err(SupervisorError::Config(format!(
            "private directory is unsafe: {}",
            path.display()
        )));
    }
    Ok(())
}

fn sibling_binary(name: &str) -> Result<PathBuf, SupervisorError> {
    let current = std::env::current_exe()
        .map_err(|source| SupervisorError::Child(format!("current executable: {source}")))?;
    let sibling = current.with_file_name(name);
    if !sibling.is_file() {
        return Err(SupervisorError::Child(format!(
            "required sibling binary is absent: {}",
            sibling.display()
        )));
    }
    Ok(sibling)
}

fn unix_millis() -> Result<i64, SupervisorError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| SupervisorError::Control(error.to_string()))?
        .as_millis();
    i64::try_from(millis).map_err(|_| SupervisorError::Control("clock overflow".into()))
}

fn random32() -> Result<[u8; 32], SupervisorError> {
    let first = crate::identity::random_registration_nonce()
        .map_err(|error| SupervisorError::Control(error.to_string()))?;
    let second = crate::identity::random_registration_nonce()
        .map_err(|error| SupervisorError::Control(error.to_string()))?;
    let mut value = [0_u8; 32];
    value[..16].copy_from_slice(&first);
    value[16..].copy_from_slice(&second);
    Ok(value)
}

fn request_id(kind: &str) -> Result<String, SupervisorError> {
    Ok(format!("{kind}:{}", hex(&random32()?[..16])))
}

fn hex(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(ALPHABET[usize::from(byte >> 4)]));
        encoded.push(char::from(ALPHABET[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn remove_owned_file_at(
    directory: &OwnedFd,
    name: &str,
    display_path: &Path,
) -> Result<(), SupervisorError> {
    match statat(directory, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(current)
            if FileType::from_raw_mode(current.st_mode).is_file()
                && current.st_uid == rustix::process::geteuid().as_raw()
                && current.st_mode & 0o7777 == 0o600 => {}
        Ok(_) => {
            return Err(SupervisorError::Config(
                "owned private control artifact was replaced".into(),
            ));
        }
        Err(rustix::io::Errno::NOENT) => return Ok(()),
        Err(source) => {
            return Err(SupervisorError::Io {
                path: display_path.to_owned(),
                source: source.into(),
            });
        }
    }
    unlinkat(directory, name, AtFlags::empty()).map_err(|source| SupervisorError::Io {
        path: display_path.to_owned(),
        source: source.into(),
    })?;
    fsync(directory).map_err(|source| SupervisorError::Io {
        path: display_path.to_owned(),
        source: source.into(),
    })?;
    Ok(())
}

async fn terminate_and_join_child(child: &mut Child, context: &str) -> Result<(), SupervisorError> {
    let Some(raw_pid) = child.id() else {
        child
            .wait()
            .await
            .map_err(|source| SupervisorError::Child(format!("join {context}: {source}")))?;
        return Ok(());
    };
    let pid = i32::try_from(raw_pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
        .ok_or_else(|| SupervisorError::Child(format!("invalid process group for {context}")))?;
    match rustix::process::kill_process_group(pid, rustix::process::Signal::TERM) {
        Ok(()) | Err(rustix::io::Errno::SRCH) => {}
        Err(error) => {
            return Err(SupervisorError::Child(format!(
                "terminate process group for {context}: {error}"
            )));
        }
    }
    if tokio::time::timeout(Duration::from_secs(2), child.wait())
        .await
        .is_ok()
    {
        return Ok(());
    }
    match rustix::process::kill_process_group(pid, rustix::process::Signal::KILL) {
        Ok(()) | Err(rustix::io::Errno::SRCH) => {}
        Err(error) => {
            return Err(SupervisorError::Child(format!(
                "kill process group for {context}: {error}"
            )));
        }
    }
    tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .map_err(|_| SupervisorError::Child(format!("join process group for {context} timed out")))?
        .map_err(|source| SupervisorError::Child(format!("join {context}: {source}")))?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("supervisor I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("supervisor singleton lease is already held")]
    LeaseHeld,
    #[error("invalid supervisor configuration: {0}")]
    Config(String),
    #[error("agent launch policy rejected the request: {0}")]
    Policy(String),
    #[error("agent launch policy has no available supervised process slot")]
    Capacity,
    #[error("daemon control protocol failed: {0}")]
    Control(String),
    #[error("supervisor I/O deadline exceeded at {stage:?}")]
    IoDeadlineExceeded { stage: SupervisorIoStage },
    #[error("daemon control became unavailable at {stage:?}: {detail}")]
    DaemonControlUnavailable {
        stage: SupervisorIoStage,
        detail: String,
    },
    #[error("supervised child failed: {0}")]
    Child(String),
    #[error(transparent)]
    OwnedSocket(#[from] OwnedUnixSocketError),
}

impl SupervisorError {
    fn public_code(&self) -> &str {
        match self {
            Self::LeaseHeld => "SUPERVISOR_ALREADY_RUNNING",
            Self::Capacity => "LAUNCH_CAPACITY_EXCEEDED",
            Self::Policy(_) => "LAUNCH_POLICY_DENIED",
            Self::Control(_)
            | Self::IoDeadlineExceeded { .. }
            | Self::DaemonControlUnavailable { .. } => "DAEMON_CONTROL_UNAVAILABLE",
            Self::Child(_) => "ADAPTER_LAUNCH_FAILED",
            Self::Io { .. } | Self::Config(_) | Self::OwnedSocket(_) => {
                "SUPERVISOR_CONFIGURATION_INVALID"
            }
        }
    }

    fn daemon_control_stage(&self) -> Option<SupervisorIoStage> {
        match self {
            Self::IoDeadlineExceeded { stage } if stage.is_daemon_control() => Some(*stage),
            Self::DaemonControlUnavailable { stage, .. } => Some(*stage),
            _ => None,
        }
    }
}

const _: u32 = MAX_DAEMON_RESTARTS;

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    fn policy() -> AgentLaunchPolicy {
        let now = unix_millis().expect("test clock");
        AgentLaunchPolicy {
            format: "codefabric.agent-launch-policy.v1".to_owned(),
            policy_id: LaunchPolicyId::try_new("policy-one").unwrap(),
            policy_revision: LaunchPolicyRevision::new(4).unwrap(),
            revocation_generation: RevocationGeneration::new(7).unwrap(),
            issued_at_unix_ms: now - 1_000,
            not_before_unix_ms: now - 1_000,
            expires_at_unix_ms: now + 60_000,
            adapter_program: PathBuf::from("/opt/codefabric/adapter"),
            adapter_arguments: vec!["-m".to_owned(), "codefabric_cpg_mcp".to_owned()],
            adapter_distribution: "codefabric-cpg-mcp".to_owned(),
            adapter_distribution_version: "0.1.0".to_owned(),
            adapter_executable_digest: format!("b3:{}", "ab".repeat(32)),
            principal_id: format!("owner:{}", "11".repeat(16)),
            workspace_ids: vec![format!("workspace:{}", "22".repeat(16))],
            operations: BTreeSet::from([SessionOperation::Status, SessionOperation::Start]),
            semantic_profiles: BTreeSet::from(["codefabric.semantic-query.v2".to_owned()]),
            maximum_resource_chunk_bytes: 1_024,
            maximum_result_bytes: 8_192,
            maximum_result_pages: 8,
            maximum_request_state_ttl_seconds: 30,
            maximum_session_seconds: 60,
            maximum_concurrent_launches: 4,
        }
    }

    #[test]
    fn wp44_int_policy_and_launch_defaults_are_strict_library_contracts() {
        let policy = policy();
        policy.validate().expect("valid typed policy");
        let pending = prepare_launch(
            &policy,
            Path::new("/private/query.sock"),
            &policy.policy_id,
            [0x22; 16],
            rustix::process::geteuid().as_raw(),
            Some(41),
            Some("test-start:41".to_owned()),
            7,
            9,
        )
        .expect("policy-owned defaults");
        assert_eq!(pending.grant.workspace_ids.len(), 1);
        assert_eq!(pending.grant.policy_id, policy.policy_id);
        assert_eq!(pending.grant.policy_revision, policy.policy_revision);
        assert_eq!(
            pending.grant.revocation_generation,
            policy.revocation_generation
        );
        assert_eq!(pending.grant.operations, policy.operations);
        assert_eq!(pending.grant.semantic_profiles, policy.semantic_profiles);
        assert_eq!(pending.grant.maximum_resource_chunk_bytes, 1_024);
        assert_eq!(pending.grant.maximum_result_bytes, 8_192);
        assert_eq!(pending.grant.maximum_result_pages, 8);
        assert_eq!(pending.grant.daemon_generation, 7);
        assert_eq!(pending.grant.supervisor_generation, 9);
        assert_eq!(pending.launcher_pid, Some(41));
        assert_eq!(pending.grant.peer_pid, None);
    }

    #[test]
    fn wp44_neg_policy_expansion_and_malformed_authority_are_rejected() {
        let mut malformed = policy();
        malformed.semantic_profiles = BTreeSet::from(["semantic profile with spaces".to_owned()]);
        assert!(malformed.validate().is_err());

        let result = prepare_launch(
            &policy(),
            Path::new("/private/query.sock"),
            &LaunchPolicyId::try_new("substituted-policy").unwrap(),
            [0x22; 16],
            rustix::process::geteuid().as_raw(),
            None,
            None,
            1,
            1,
        );
        assert!(matches!(result, Err(SupervisorError::Policy(_))));

        let now = unix_millis().unwrap();
        let mut not_yet_valid = policy();
        not_yet_valid.issued_at_unix_ms = now;
        not_yet_valid.not_before_unix_ms = now + 60_000;
        not_yet_valid.expires_at_unix_ms = now + 120_000;
        assert!(matches!(
            prepare_launch(
                &not_yet_valid,
                Path::new("/private/query.sock"),
                &not_yet_valid.policy_id,
                [0x22; 16],
                rustix::process::geteuid().as_raw(),
                None,
                None,
                1,
                1,
            ),
            Err(SupervisorError::Policy(_))
        ));

        let mut expired = policy();
        expired.issued_at_unix_ms = now - 120_000;
        expired.not_before_unix_ms = now - 120_000;
        expired.expires_at_unix_ms = now - 1;
        assert!(matches!(
            prepare_launch(
                &expired,
                Path::new("/private/query.sock"),
                &expired.policy_id,
                [0x22; 16],
                rustix::process::geteuid().as_raw(),
                None,
                None,
                1,
                1,
            ),
            Err(SupervisorError::Policy(_))
        ));

        let request = serde_json::to_value(SupervisorRequest::Launch {
            policy_id: policy().policy_id,
            request_id: "attach:test".to_owned(),
            issued_at_unix_ms: now,
            expires_at_unix_ms: now + 1_000,
        })
        .unwrap();
        let fields = request
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            fields,
            BTreeSet::from([
                "expires_at_unix_ms".to_owned(),
                "issued_at_unix_ms".to_owned(),
                "kind".to_owned(),
                "policy_id".to_owned(),
                "request_id".to_owned(),
            ])
        );
    }

    #[test]
    fn wp44_int_singleton_lease_records_exact_identity_and_generation() {
        let root = tempfile::tempdir().expect("supervisor runtime root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("private runtime root");
        let first = SupervisorLease::acquire(root.path()).expect("first lease");
        assert_eq!(first.record.supervisor_pid, std::process::id());
        assert_eq!(first.record.supervisor_generation, 1);
        assert_eq!(first.record.lease_device, first.record.runtime_device);
        assert_ne!(first.record.lease_inode, 0);
        assert!(matches!(
            SupervisorLease::acquire(root.path()),
            Err(SupervisorError::LeaseHeld)
        ));
        drop(first);
        let successor = SupervisorLease::acquire(root.path()).expect("joined owner releases lease");
        assert_eq!(successor.record.supervisor_generation, 2);
    }

    #[test]
    fn wp44_neg_supervisor_generation_is_durable_monotone_across_clock_faults() {
        let root = tempfile::tempdir().expect("supervisor runtime root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let first = SupervisorLease::acquire_with_clock(root.path(), || {}, || Ok(10_000))
            .expect("first persisted generation");
        assert_eq!(first.record.supervisor_generation, 1);
        assert_eq!(first.record.issued_at_unix_ms, 10_000);
        drop(first);

        let same_clock = SupervisorLease::acquire_with_clock(root.path(), || {}, || Ok(10_000))
            .expect("same-clock restart advances");
        assert_eq!(same_clock.record.supervisor_generation, 2);
        drop(same_clock);

        let regressed_clock = SupervisorLease::acquire_with_clock(root.path(), || {}, || Ok(9_000))
            .expect("clock-regressed restart advances");
        assert_eq!(regressed_clock.record.supervisor_generation, 3);
        assert_eq!(regressed_clock.record.issued_at_unix_ms, 9_000);
        drop(regressed_clock);

        let lease_path = root.path().join(SUPERVISOR_LEASE);
        let descriptor = open(
            &lease_path,
            OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .unwrap();
        let opened = fstat(&descriptor).unwrap();
        let mut file = File::from(descriptor);
        let persisted = read_existing_lease_record(&mut file, &opened)
            .unwrap()
            .unwrap();
        assert_eq!(persisted.supervisor_generation, 3);
    }

    #[test]
    fn wp44_neg_supervisor_generation_survives_interrupted_inactive_slot_write() {
        let root = tempfile::tempdir().expect("supervisor runtime root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let first = SupervisorLease::acquire_with_clock(root.path(), || {}, || Ok(10_000))
            .expect("first persisted generation");
        drop(first);
        let second = SupervisorLease::acquire_with_clock(root.path(), || {}, || Ok(10_001))
            .expect("second persisted generation");
        let mut interrupted = second.record.clone();
        interrupted.supervisor_generation = 3;
        interrupted.issued_at_unix_ms = 10_002;
        drop(second);

        // Generation three targets slot zero.  Simulate interruption after only part of that
        // inactive slot reached the locked inode; generation two in slot one must remain the
        // sole durable authority and the next acquisition must advance from it.
        let slot = encode_lease_slot(&interrupted).unwrap();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(root.path().join(SUPERVISOR_LEASE))
            .unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(&slot[..SUPERVISOR_LEASE_SLOT_HEADER_BYTES + 11])
            .unwrap();
        file.sync_all().unwrap();
        drop(file);

        let recovered = SupervisorLease::acquire_with_clock(root.path(), || {}, || Ok(10_003))
            .expect("intact prior slot advances after interrupted write");
        assert_eq!(recovered.record.supervisor_generation, 3);
        assert_eq!(recovered.record.issued_at_unix_ms, 10_003);
    }

    #[tokio::test]
    async fn wp44_neg_agent_rendezvous_rejects_shutdown_and_bounds_silent_peers() {
        assert!(serde_json::from_slice::<SupervisorRequest>(br#"{"kind":"drain"}"#).is_err());
        assert!(serde_json::from_slice::<SupervisorRequest>(br#"{"kind":"shutdown"}"#).is_err());

        let slots = Arc::new(Semaphore::new(SUPERVISOR_RENDEZVOUS_TASK_LIMIT));
        let mut permits = Vec::new();
        for _ in 0..SUPERVISOR_RENDEZVOUS_TASK_LIMIT {
            permits.push(slots.clone().try_acquire_owned().unwrap());
        }
        assert!(slots.clone().try_acquire_owned().is_err());
        drop(permits);

        let (silent, reader) = StdUnixStream::pair().unwrap();
        silent.set_nonblocking(true).unwrap();
        reader.set_nonblocking(true).unwrap();
        let mut reader = UnixStream::from_std(reader).unwrap();
        let error = read_supervisor_request_with_timeout(&mut reader, Duration::from_millis(10))
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            SupervisorError::IoDeadlineExceeded {
                stage: SupervisorIoStage::RendezvousRead
            }
        ));

        let (mut blocked_writer, unread) = StdUnixStream::pair().unwrap();
        blocked_writer.set_nonblocking(true).unwrap();
        unread.set_nonblocking(true).unwrap();
        let fill = [0_u8; 16 * 1024];
        loop {
            match blocked_writer.write(&fill) {
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => panic!("fill rendezvous send buffer: {error}"),
            }
        }
        let mut blocked_writer = UnixStream::from_std(blocked_writer).unwrap();
        let response = SupervisorResponse {
            accepted: false,
            code: "BOUNDED".to_owned(),
            preparation: None,
            launch: None,
            daemon_pid: 41,
            daemon_generation: 7,
            supervisor_generation: 9,
        };
        let error = write_supervisor_response_with_timeout(
            &mut blocked_writer,
            &response,
            Duration::from_millis(10),
        )
        .await
        .unwrap_err();
        assert!(matches!(
            error,
            SupervisorError::IoDeadlineExceeded {
                stage: SupervisorIoStage::RendezvousWrite
            }
        ));
        drop(unread);
    }

    #[test]
    fn wp44_neg_singleton_lease_rejects_symlink_and_replaced_inode() {
        use std::os::unix::fs::symlink;

        let symlink_root = tempfile::tempdir().expect("symlink runtime root");
        fs::set_permissions(symlink_root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let target = symlink_root.path().join("target");
        fs::write(&target, b"untrusted").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        symlink(&target, symlink_root.path().join(SUPERVISOR_LEASE)).unwrap();
        assert!(SupervisorLease::acquire(symlink_root.path()).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"untrusted");

        let replaced_root = tempfile::tempdir().expect("replacement runtime root");
        fs::set_permissions(replaced_root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let replacement = replaced_root.path().join("replacement");
        fs::write(&replacement, b"replacement").unwrap();
        fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(matches!(
            SupervisorLease::acquire_with(replaced_root.path(), || {
                fs::rename(&replacement, replaced_root.path().join(SUPERVISOR_LEASE)).unwrap();
            }),
            Err(SupervisorError::Config(_))
        ));
    }

    #[tokio::test]
    async fn wp44_neg_control_hello_binds_kernel_peer_and_generations() {
        let (writer, reader) = StdUnixStream::pair().expect("control socketpair");
        writer.set_nonblocking(true).expect("writer nonblocking");
        reader.set_nonblocking(true).expect("reader nonblocking");
        let mut writer = UnixStream::from_std(writer).expect("Tokio writer");
        let mut reader = UnixStream::from_std(reader).expect("Tokio reader");
        let key = DaemonControlKey([0x51; 32]);
        let hello = DaemonControlRequest::Hello(DaemonControlHello {
            request_id: "hello:test".to_owned(),
            control_key: key.clone(),
            supervisor_generation: 3,
            daemon_generation: 4,
            supervisor_pid: std::process::id(),
            supervisor_uid: rustix::process::geteuid().as_raw(),
        });
        let record = DaemonControlRecord::new(hello, &key, [0x22; 16], 4, 3, 1).unwrap();
        write_line(&mut writer, &record, CONTROL_MAX_BYTES)
            .await
            .expect("write hello");
        let accepted = accept_control_hello(&mut reader)
            .await
            .expect("accept hello");
        assert_eq!(accepted.0.daemon_generation, 4);
        assert_eq!(accepted.0.supervisor_generation, 3);
        assert_eq!(accepted.1.workspace_id, [0x22; 16]);

        let (writer, reader) = StdUnixStream::pair().expect("fault socketpair");
        writer.set_nonblocking(true).expect("writer nonblocking");
        reader.set_nonblocking(true).expect("reader nonblocking");
        let mut writer = UnixStream::from_std(writer).expect("Tokio writer");
        let mut reader = UnixStream::from_std(reader).expect("Tokio reader");
        let fault_key = DaemonControlKey([0x52; 32]);
        let wrong_uid = DaemonControlRequest::Hello(DaemonControlHello {
            request_id: "hello:fault".to_owned(),
            control_key: fault_key.clone(),
            supervisor_generation: 3,
            daemon_generation: 4,
            supervisor_pid: std::process::id(),
            supervisor_uid: rustix::process::geteuid().as_raw().wrapping_add(1),
        });
        let wrong_uid =
            DaemonControlRecord::new(wrong_uid, &fault_key, [0x22; 16], 4, 3, 1).unwrap();
        write_line(&mut writer, &wrong_uid, CONTROL_MAX_BYTES)
            .await
            .expect("write fault hello");
        assert!(accept_control_hello(&mut reader).await.is_err());
    }

    #[test]
    fn wp44_neg_daemon_control_rejects_replay_gap_changed_duplicate_and_expiry() {
        let key = DaemonControlKey([0x61; 32]);
        let workspace_id = [0x22; 16];
        let hello = DaemonControlRequest::Hello(DaemonControlHello {
            request_id: "hello:state".to_owned(),
            control_key: key.clone(),
            supervisor_generation: 3,
            daemon_generation: 4,
            supervisor_pid: std::process::id(),
            supervisor_uid: rustix::process::geteuid().as_raw(),
        });
        let hello = DaemonControlRecord::new(hello, &key, workspace_id, 4, 3, 1).unwrap();
        let mut state = DaemonControlReadState::from_hello(&hello).unwrap();
        let second = DaemonControlRecord::new(
            DaemonControlRequest::Drain {
                request_id: "drain:one".to_owned(),
            },
            &key,
            workspace_id,
            4,
            3,
            2,
        )
        .unwrap();
        state.accept(&second, unix_millis().unwrap()).unwrap();
        assert!(matches!(
            state.accept(&second, unix_millis().unwrap()),
            Err(SupervisorError::Control(code)) if code == "CONTROL_RECORD_REPLAY"
        ));

        let changed = DaemonControlRecord::new(
            DaemonControlRequest::Shutdown {
                request_id: "shutdown:changed".to_owned(),
            },
            &key,
            workspace_id,
            4,
            3,
            2,
        )
        .unwrap();
        assert!(matches!(
            state.accept(&changed, unix_millis().unwrap()),
            Err(SupervisorError::Control(code)) if code == "CONTROL_CHANGED_DUPLICATE"
        ));

        let mut integrity_tamper = DaemonControlRecord::new(
            DaemonControlRequest::Drain {
                request_id: "drain:integrity-original".to_owned(),
            },
            &key,
            workspace_id,
            4,
            3,
            3,
        )
        .unwrap();
        let DaemonControlRequest::Drain { request_id } = &mut integrity_tamper.request else {
            unreachable!("constructed a drain record")
        };
        *request_id = "drain:integrity-substituted".to_owned();
        assert!(matches!(
            state.accept(&integrity_tamper, unix_millis().unwrap()),
            Err(SupervisorError::Control(code)) if code == "CONTROL_CONTENT_INTEGRITY_MISMATCH"
        ));

        let gap = DaemonControlRecord::new(
            DaemonControlRequest::Drain {
                request_id: "drain:gap".to_owned(),
            },
            &key,
            workspace_id,
            4,
            3,
            4,
        )
        .unwrap();
        assert!(matches!(
            state.accept(&gap, unix_millis().unwrap()),
            Err(SupervisorError::Control(code)) if code == "CONTROL_SEQUENCE_GAP"
        ));

        let mut expired = DaemonControlRecord::new(
            DaemonControlRequest::Drain {
                request_id: "drain:expired".to_owned(),
            },
            &key,
            workspace_id,
            4,
            3,
            3,
        )
        .unwrap();
        expired.header.issued_at_unix_ms = 1;
        expired.header.expires_at_unix_ms = 2;
        expired.header.content_integrity = expired.expected_integrity(&key).unwrap();
        assert!(matches!(
            state.accept(&expired, unix_millis().unwrap()),
            Err(SupervisorError::Control(code)) if code == "CONTROL_RECORD_EXPIRED_OR_INVALID"
        ));
    }

    #[test]
    fn wp44_int_named_policy_store_resolves_two_independent_principals_exactly() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("policy store parent");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let store_path = root.path().join(POLICY_DIRECTORY);
        fs::create_dir(&store_path).unwrap();
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o700)).unwrap();

        let first = policy();
        let mut second = policy();
        second.policy_id = LaunchPolicyId::try_new("policy-two").unwrap();
        second.principal_id = format!("owner:{}", "33".repeat(16));
        let first_path = store_path.join("policy-one.json");
        let second_path = store_path.join("policy-two.json");
        write_private_json(&first_path, &first).unwrap();
        write_private_json(&second_path, &second).unwrap();

        let store = AgentLaunchPolicyStore::open(&store_path).expect("exact policy store");
        let loaded_first = store.load(&first.policy_id).expect("first policy");
        let loaded_second = store.load(&second.policy_id).expect("second policy");
        assert_eq!(loaded_first.policy_id.as_str(), "policy-one");
        assert_eq!(loaded_second.policy_id.as_str(), "policy-two");
        assert_ne!(loaded_first.principal_id, loaded_second.principal_id);

        fs::set_permissions(&first_path, fs::Permissions::from_mode(0o640)).unwrap();
        assert!(matches!(
            store.load(&first.policy_id),
            Err(SupervisorError::Policy(_))
        ));
        fs::set_permissions(&first_path, fs::Permissions::from_mode(0o600)).unwrap();
        let before_inode = fs::metadata(&first_path).unwrap().ino();
        assert!(matches!(
            store.load_with(&first.policy_id, || {
                fs::set_permissions(&first_path, fs::Permissions::from_mode(0o400)).unwrap();
            }),
            Err(SupervisorError::Policy(_))
        ));
        assert_eq!(fs::metadata(&first_path).unwrap().ino(), before_inode);

        let linked_id = LaunchPolicyId::try_new("policy-linked").unwrap();
        symlink(&second_path, store_path.join("policy-linked.json")).unwrap();
        assert!(store.load(&linked_id).is_err());

        let mut substituted = second;
        substituted.policy_id = first.policy_id.clone();
        write_private_json(&second_path, &substituted).unwrap();
        assert!(matches!(
            store.load(&LaunchPolicyId::try_new("policy-two").unwrap()),
            Err(SupervisorError::Policy(_))
        ));
    }

    #[test]
    fn wp44_neg_policy_root_ancestor_symlink_and_replacement_cannot_redirect_load() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("policy authority parent");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let config_root = root.path().join("config-real");
        let store_path = config_root.join(POLICY_DIRECTORY);
        fs::create_dir(&config_root).unwrap();
        fs::create_dir(&store_path).unwrap();
        fs::set_permissions(&config_root, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o700)).unwrap();
        let original = policy();
        write_private_json(&store_path.join("policy-one.json"), &original).unwrap();

        let config_alias = root.path().join("config-alias");
        symlink(&config_root, &config_alias).unwrap();
        assert!(matches!(
            AgentLaunchPolicyStore::open(&config_alias.join(POLICY_DIRECTORY)),
            Err(SupervisorError::Config(_))
        ));

        let store = AgentLaunchPolicyStore::open(&store_path).expect("descriptor-owned store");
        let renamed_store = config_root.join("agent-launch-policies-original");
        fs::rename(&store_path, &renamed_store).unwrap();
        fs::create_dir(&store_path).unwrap();
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o700)).unwrap();
        let mut substitute = policy();
        substitute.principal_id = format!("owner:{}", "99".repeat(16));
        write_private_json(&store_path.join("policy-one.json"), &substitute).unwrap();

        let loaded = store
            .load(&original.policy_id)
            .expect("load remains relative to retained original store");
        assert_eq!(loaded.principal_id, original.principal_id);
        assert_ne!(loaded.principal_id, substitute.principal_id);
    }

    #[test]
    fn wp44_neg_runtime_discovery_root_replacement_cannot_redirect_write_or_cleanup() {
        let parent = tempfile::tempdir().expect("runtime authority parent");
        fs::set_permissions(parent.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let runtime = parent.path().join("runtime");
        fs::create_dir(&runtime).unwrap();
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
        let directory = open_absolute_directory_nofollow(&runtime).unwrap();
        let display_path = runtime.join(SUPERVISOR_DISCOVERY);
        let mut discovery = SupervisorDiscovery {
            format: "codefabric.supervisor-discovery.v1".to_owned(),
            supervisor_socket: runtime.join(SUPERVISOR_SOCKET),
            query_socket: runtime.join("query.sock"),
            supervisor_generation: 1,
            daemon_generation: 1,
            daemon_pid: std::process::id(),
        };
        write_private_json_at(&directory, SUPERVISOR_DISCOVERY, &discovery, &display_path).unwrap();

        let original = parent.path().join("runtime-original");
        fs::rename(&runtime, &original).unwrap();
        fs::create_dir(&runtime).unwrap();
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(&display_path, b"substitute discovery").unwrap();
        fs::set_permissions(&display_path, fs::Permissions::from_mode(0o600)).unwrap();

        discovery.daemon_generation = 2;
        write_private_json_at(&directory, SUPERVISOR_DISCOVERY, &discovery, &display_path)
            .expect("update remains relative to retained original runtime root");
        let original_bytes =
            fs::read(original.join(SUPERVISOR_DISCOVERY)).expect("original discovery update");
        let observed: SupervisorDiscovery = serde_json::from_slice(&original_bytes).unwrap();
        assert_eq!(observed.daemon_generation, 2);
        assert_eq!(fs::read(&display_path).unwrap(), b"substitute discovery");

        remove_owned_file_at(&directory, SUPERVISOR_DISCOVERY, &display_path)
            .expect("cleanup remains relative to retained original runtime root");
        assert!(!original.join(SUPERVISOR_DISCOVERY).exists());
        assert_eq!(fs::read(&display_path).unwrap(), b"substitute discovery");
    }

    #[test]
    fn wp44_neg_launch_request_replay_and_process_start_substitution_fail_closed() {
        let now = unix_millis().unwrap();
        let policy_id = LaunchPolicyId::try_new("policy-one").unwrap();
        let peer_pid = Some(std::process::id());
        let peer_start_identity = required_process_start_identity(peer_pid).unwrap();
        let mut registry = SupervisorLaunchRegistry::default();
        registry
            .accept_request(
                &policy_id,
                "attach:anti-replay",
                now,
                now + 1_000,
                now,
                rustix::process::geteuid().as_raw(),
                peer_pid,
                peer_start_identity.clone(),
            )
            .expect("first exact request");
        assert!(matches!(
            registry.accept_request(
                &policy_id,
                "attach:anti-replay",
                now,
                now + 1_000,
                now,
                rustix::process::geteuid().as_raw(),
                peer_pid,
                peer_start_identity,
            ),
            Err(SupervisorError::Policy(_))
        ));
        assert!(!process_identity_matches(
            peer_pid,
            Some("linux-proc-start:substituted")
        ));
    }

    #[tokio::test]
    async fn wp44_neg_activating_launch_reserves_capacity_across_daemon_acknowledgement() {
        let (client_stream, server_stream) = StdUnixStream::pair().expect("control socketpair");
        client_stream.set_nonblocking(true).unwrap();
        server_stream.set_nonblocking(true).unwrap();
        let key = DaemonControlKey([0x73; 32]);
        let control = Arc::new(DaemonControlClient {
            state: Mutex::new(DaemonControlClientState {
                stream: UnixStream::from_std(client_stream).unwrap(),
                next_sequence: 1,
            }),
            workspace_id: [0x22; 16],
            daemon_generation: 7,
            supervisor_generation: 9,
            key: key.clone(),
            available: AtomicBool::new(true),
            io_timeout: Duration::from_millis(100),
        });
        let received = Arc::new(tokio::sync::Barrier::new(2));
        let release = Arc::new(tokio::sync::Barrier::new(2));
        let server = {
            let received = received.clone();
            let release = release.clone();
            tokio::spawn(async move {
                let mut stream = UnixStream::from_std(server_stream).unwrap();
                let record: DaemonControlRecord =
                    read_line(&mut stream, CONTROL_MAX_BYTES).await.unwrap();
                received.wait().await;
                release.wait().await;
                let acknowledgement = DaemonControlAcknowledgement::new(
                    &key,
                    &record.header,
                    record.request.request_id(),
                    true,
                    "LAUNCH_GRANT_REGISTERED",
                )
                .unwrap();
                write_line(&mut stream, &acknowledgement, CONTROL_MAX_BYTES)
                    .await
                    .unwrap();
            })
        };

        let mut policy = policy();
        policy.maximum_concurrent_launches = 1;
        let peer_uid = rustix::process::geteuid().as_raw();
        let peer_pid = Some(std::process::id());
        let peer_start_identity = required_process_start_identity(peer_pid).unwrap();
        let first = prepare_launch(
            &policy,
            Path::new("/private/query.sock"),
            &policy.policy_id,
            [0x22; 16],
            peer_uid,
            peer_pid,
            peer_start_identity.clone(),
            7,
            9,
        )
        .unwrap();
        let first_id = first.preparation.launch_id.clone();
        let launches = Arc::new(Mutex::new(SupervisorLaunchRegistry::default()));
        launches
            .lock()
            .await
            .reserve(first, 1, unix_millis().unwrap())
            .unwrap();
        let activation = {
            let launches = launches.clone();
            let control = control.clone();
            tokio::spawn(async move {
                activate_launch(
                    &launches,
                    &control,
                    &first_id,
                    std::process::id(),
                    peer_uid,
                    peer_pid,
                )
                .await
            })
        };
        received.wait().await;
        let second = prepare_launch(
            &policy,
            Path::new("/private/query.sock"),
            &policy.policy_id,
            [0x22; 16],
            peer_uid,
            peer_pid,
            peer_start_identity,
            7,
            9,
        )
        .unwrap();
        let mut registry = launches.lock().await;
        assert_eq!(registry.activating.len(), 1);
        assert!(matches!(
            registry.reserve(second, 1, unix_millis().unwrap()),
            Err(SupervisorError::Capacity)
        ));
        drop(registry);
        release.wait().await;
        let envelope = activation.await.unwrap().expect("accepted activation");
        assert!(envelope.session_expires_at_unix_ms > unix_millis().unwrap());
        assert_eq!(launches.lock().await.active.len(), 1);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn accepted_work_drain_waits_beyond_control_io_without_retiring_the_generation() {
        let (client, mut server) = UnixStream::pair().unwrap();
        let key = DaemonControlKey([0x79; 32]);
        let control = Arc::new(DaemonControlClient {
            state: Mutex::new(DaemonControlClientState {
                stream: client,
                next_sequence: 1,
            }),
            workspace_id: [0x22; 16],
            daemon_generation: 7,
            supervisor_generation: 9,
            key: key.clone(),
            available: AtomicBool::new(true),
            io_timeout: Duration::from_millis(10),
        });
        let task = {
            let control = Arc::clone(&control);
            tokio::spawn(async move { control.drain_accepted_work().await })
        };
        let record: DaemonControlRecord = read_line(&mut server, CONTROL_MAX_BYTES).await.unwrap();
        assert!(matches!(record.request, DaemonControlRequest::Drain { .. }));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            !task.is_finished(),
            "accepted-work cleanup must outlive the ordinary IO allowance"
        );
        let acknowledgement = DaemonControlAcknowledgement::new(
            &key,
            &record.header,
            record.request.request_id(),
            true,
            "DRAIN_ACCEPTED",
        )
        .unwrap();
        write_line(&mut server, &acknowledgement, CONTROL_MAX_BYTES)
            .await
            .unwrap();
        task.await.unwrap().unwrap();
        assert!(control.is_available());
    }

    #[tokio::test]
    async fn wp44_neg_daemon_control_timeout_closes_admission_and_invalidates_generation() {
        let (client_stream, silent_server) = StdUnixStream::pair().unwrap();
        client_stream.set_nonblocking(true).unwrap();
        silent_server.set_nonblocking(true).unwrap();
        let control = Arc::new(DaemonControlClient {
            state: Mutex::new(DaemonControlClientState {
                stream: UnixStream::from_std(client_stream).unwrap(),
                next_sequence: 1,
            }),
            workspace_id: [0x22; 16],
            daemon_generation: 7,
            supervisor_generation: 9,
            key: DaemonControlKey([0x74; 32]),
            available: AtomicBool::new(true),
            io_timeout: Duration::from_millis(10),
        });
        let error = control
            .transact(&DaemonControlRequest::Shutdown {
                request_id: "shutdown:timeout".to_owned(),
            })
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            SupervisorError::IoDeadlineExceeded {
                stage: SupervisorIoStage::DaemonControlRead
            }
        ));
        assert!(!control.is_available());

        let serving = RwLock::new(SupervisorServingState {
            ready: true,
            daemon_pid: 41,
            daemon_generation: 7,
            control,
        });
        let launches = Mutex::new(SupervisorLaunchRegistry {
            accepted_requests: VecDeque::from([AcceptedLaunchRequest {
                request_id: "attach:retired-generation".to_owned(),
                policy_id: LaunchPolicyId::try_new("policy-one").unwrap(),
                peer_uid: rustix::process::geteuid().as_raw(),
                peer_pid: Some(std::process::id()),
                peer_start_identity: process_start_identity(std::process::id()),
                expires_at_unix_ms: unix_millis().unwrap() + 1_000,
            }]),
            ..SupervisorLaunchRegistry::default()
        });
        assert!(close_failed_daemon_generation(&serving, &launches, 7).await);
        assert!(!serving.read().await.snapshot().ready);
        assert!(launches.lock().await.accepted_requests.is_empty());
        drop(silent_server);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn wp44_neg_stubborn_abandoned_adapter_is_gone_before_capacity_release() {
        let mut child = Command::new("/bin/sh")
            .args([
                "-c",
                "trap '' TERM; printf ready; while :; do sleep 1; done",
            ])
            .stdout(Stdio::piped())
            .process_group(0)
            .spawn()
            .expect("spawn TERM-resistant adapter group");
        let pid = child.id().expect("adapter PID");
        let mut ready = [0_u8; 5];
        child
            .stdout
            .as_mut()
            .expect("adapter stdout")
            .read_exact(&mut ready)
            .await
            .expect("adapter readiness");
        assert_eq!(&ready, b"ready");
        let active = ActiveLaunch {
            grant_id: "launch:stubborn-adapter".to_owned(),
            grant_digest: [0x51; 32],
            policy_id: LaunchPolicyId::try_new("policy-one").unwrap(),
            adapter_pid: pid,
            launcher_uid: rustix::process::geteuid().as_raw(),
            launcher_pid: Some(std::process::id()),
            launcher_start_identity: process_start_identity(std::process::id()),
            adapter_start_identity: process_start_identity(pid),
            expires_at_unix_ms: unix_millis().unwrap() + 60_000,
            daemon_generation: 7,
        };
        let adapter_start_identity = active.adapter_start_identity.clone();
        let join = tokio::spawn(async move { child.wait().await });

        terminate_exact_adapter_group(&active)
            .await
            .expect("TERM-to-KILL escalation proves exact exit");
        let status = join.await.unwrap().unwrap();
        assert!(!status.success());
        assert!(!process_identity_matches(
            Some(pid),
            adapter_start_identity.as_deref()
        ));
    }

    #[tokio::test]
    async fn wp44_neg_adapter_executable_substitution_fails_declared_digest() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("adapter executable root");
        let program = root.path().join("adapter");
        fs::write(&program, b"#!/bin/sh\nprintf '0.1.0\\n'\n").unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
        let declared = adapter_executable_digest(&program).unwrap();
        let linked_program = root.path().join("linked-adapter");
        symlink(&program, &linked_program).unwrap();
        assert_eq!(
            adapter_executable_digest(&linked_program).unwrap(),
            declared
        );
        let preparation = AdapterLaunchPreparation {
            launch_id: "launch:adapter-substitution".to_owned(),
            adapter_program: program.clone(),
            adapter_arguments: Vec::new(),
            adapter_distribution: "test-adapter".to_owned(),
            adapter_distribution_version: "0.1.0".to_owned(),
            adapter_executable_digest: declared,
            daemon_generation: 7,
            supervisor_generation: 9,
        };
        verify_adapter_identity(&preparation)
            .await
            .expect("descriptor-bound distribution identity");
        fs::write(&program, b"#!/bin/sh\nprintf '9.9.9\\n'\n").unwrap();
        assert!(matches!(
            verify_adapter_identity(&preparation).await,
            Err(SupervisorError::Policy(_))
        ));
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn wp44_neg_generation_refresh_revalidates_complete_adapter_authority() {
        let executable = fs::canonicalize("/bin/sleep").unwrap();
        let digest = running_adapter_executable_digest(&executable).unwrap();
        let mut child = Command::new(&executable).arg("30").spawn().unwrap();
        let adapter_pid = child.id().unwrap();
        let original = AdapterLaunchPreparation {
            launch_id: "launch:original".to_owned(),
            adapter_program: executable,
            adapter_arguments: vec!["--authority-bound".to_owned()],
            adapter_distribution: "codefabric-cpg-mcp".to_owned(),
            adapter_distribution_version: "0.1.0".to_owned(),
            adapter_executable_digest: digest,
            daemon_generation: 7,
            supervisor_generation: 9,
        };
        let mut replacement = original.clone();
        replacement.launch_id = "launch:replacement".to_owned();
        replacement.daemon_generation = 8;
        validate_generation_refresh_authority(&original, &replacement, 8, 9, adapter_pid)
            .await
            .expect("unchanged authority and live executable");

        for substituted in [
            (
                "other-distribution",
                "0.1.0",
                original.adapter_executable_digest.as_str(),
            ),
            (
                "codefabric-cpg-mcp",
                "9.9.9",
                original.adapter_executable_digest.as_str(),
            ),
            (
                "codefabric-cpg-mcp",
                "0.1.0",
                "b3:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            ),
        ] {
            let mut candidate = replacement.clone();
            candidate.adapter_distribution = substituted.0.to_owned();
            candidate.adapter_distribution_version = substituted.1.to_owned();
            candidate.adapter_executable_digest = substituted.2.to_owned();
            assert!(
                validate_generation_refresh_authority(&original, &candidate, 8, 9, adapter_pid,)
                    .await
                    .is_err()
            );
        }

        let mut wrong_live_original = original.clone();
        wrong_live_original.adapter_executable_digest =
            "b3:0000000000000000000000000000000000000000000000000000000000000000".to_owned();
        let mut wrong_live_replacement = replacement;
        wrong_live_replacement.adapter_executable_digest =
            wrong_live_original.adapter_executable_digest.clone();
        assert!(matches!(
            validate_generation_refresh_authority(
                &wrong_live_original,
                &wrong_live_replacement,
                8,
                9,
                adapter_pid,
            )
            .await,
            Err(SupervisorError::Policy(_))
        ));
        child.kill().await.unwrap();
        child.wait().await.unwrap();
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn wp44_beh_fd3_is_real_unidirectional_bounded_frame_with_terminal_eof() {
        let (mut parent, child_fd) = directional_adapter_launch_channel().unwrap();
        let script = r#"
import os, sys
try:
    os.write(3, b"reverse")
except OSError:
    pass
else:
    raise SystemExit(91)
data = b""
while True:
    chunk = os.read(3, 4096)
    if not chunk:
        break
    data += chunk
    if len(data) > 64:
        raise SystemExit(92)
raise SystemExit(0 if data == b'{"format":"probe"}\n' else 93)
"#;
        let mut command = Command::new("/usr/bin/python3");
        command
            .args(["-I", "-c", script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .fd_mappings(vec![FdMapping {
                parent_fd: child_fd,
                child_fd: ADAPTER_LAUNCH_FD,
            }])
            .unwrap();
        let child = command.spawn().expect("real fd3 probe child");
        let mut reverse = [0_u8; 1];
        assert_eq!(parent.read(&mut reverse).unwrap(), 0);
        parent.write_all(b"{\"format\":\"probe\"}\n").unwrap();
        parent.shutdown(Shutdown::Write).unwrap();
        let output = tokio::time::timeout(Duration::from_secs(5), child.wait_with_output())
            .await
            .expect("fd3 probe deadline")
            .expect("fd3 probe join");
        assert!(
            output.status.success(),
            "fd3 probe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn wp44_neg_adapter_launch_debug_redacts_the_sole_grant_secret() {
        let envelope = AdapterLaunchEnvelope {
            format: "codefabric.adapter-launch.v1".to_owned(),
            query_socket: PathBuf::from("/private/query.sock"),
            launch_grant_hex: "sole-secret-grant".to_owned(),
            adapter_program: PathBuf::from("/opt/codefabric/adapter"),
            adapter_arguments: Vec::new(),
            daemon_generation: 7,
            supervisor_generation: 9,
            session_expires_at_unix_ms: unix_millis().unwrap() + 1_000,
            maximum_request_state_ttl_seconds: 30,
        };
        let debug = format!("{envelope:?}");
        assert!(!debug.contains("sole-secret-grant"));
        assert!(debug.contains("[REDACTED]"));
    }
}
