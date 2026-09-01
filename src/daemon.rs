//! Local daemon lifecycle, closed configuration, singleton lease, and admin IPC.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Write as _};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::oneshot;

use crate::contracts::deployment_profile::DeploymentProfileDocument;
use crate::fabric::forward_cutover::{CutoverAdmission, CutoverPhase, DurableCutoverState};
use crate::fabric::programmatic_query_backend::{
    ProgrammaticSemanticQueryBackend, ProgrammaticSemanticQueryBackendError,
    ProgrammaticSemanticQueryPorts,
};
use crate::fabric::programmatic_workspace::{
    ProgrammaticCommandRecoveryError, ProgrammaticDaemonComposition,
    ProgrammaticDaemonCompositionShutdownError,
};
use crate::fabric::published_arrow_result::PublishedArrowResultRegistry;
use crate::forward_cutover_controller::{
    ProductionCutoverStatus, ProductionForwardCutoverController,
};
use crate::query_service::{
    ProductionQueryService, QueryAuthorization, QueryTransportError, ResultArtifactStore,
    SemanticQueryBackend, serve_query_uds,
};
use crate::rpc::SameUserInterceptor;
use crate::rpc::generated::codefabric::cpgd::v1::{WorkspaceClaim, WorkspaceReadiness};
use crate::workspace_registry::{RelinkProof, RemovalPolicy, WorkspaceRecord};

const CONFIG_MAX_BYTES: u64 = 262_144;
const ADMIN_MESSAGE_MAX_BYTES: usize = 65_536;
const SOCKET_PATH_MAX_BYTES: usize = 103;
const DEPLOYMENT_PROFILE: &[u8] =
    include_bytes!("../contracts/deployment/local-workstation-v1.yaml");

/// Closed restart-required daemon configuration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StaticConfig {
    /// Private durable daemon state root.
    pub state_root: PathBuf,
    /// Private short-path runtime root.
    pub runtime_root: PathBuf,
    /// Private configuration root.
    pub config_root: PathBuf,
    /// Unix-domain administrative endpoint.
    pub socket_endpoint: PathBuf,
    /// Unix-domain authenticated query endpoint.
    pub query_socket_endpoint: PathBuf,
    /// Private capability-token filename, relative to the configuration root.
    pub query_capability_token_file: PathBuf,
    /// Operational database filename, relative to the state root.
    pub operational_database: PathBuf,
    /// Exact sandbox policy from the released deployment profile.
    pub sandbox_policy: String,
    /// Hard-limit profile selected at startup.
    pub hard_limit_profile: String,
    /// Exact supported deployment profile.
    pub supported_platform_profile: String,
}

/// Closed reloadable daemon configuration.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReloadableConfig {
    /// Runtime tracing filter.
    pub log_level: String,
    /// Sampling fraction in the closed inclusive interval zero through one.
    pub telemetry_sampling: f64,
    /// Soft concurrent-query quota.
    pub soft_query_quota: u32,
    /// Stable maintenance schedule code.
    pub maintenance_schedule: String,
}

/// Complete AC-G-62 daemon configuration. Workspace-owned fields are intentionally absent.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonConfig {
    /// Restart-required tier.
    pub static_config: StaticConfig,
    /// Reloadable tier.
    pub reloadable: ReloadableConfig,
}

impl DaemonConfig {
    /// Decode one bounded TOML file and validate it against the released deployment contract.
    ///
    /// # Errors
    ///
    /// Returns a bounded I/O, TOML, tier, path, permission, or profile error.
    pub fn load(path: &Path) -> Result<Self, DaemonError> {
        let metadata = fs::symlink_metadata(path).map_err(|source| DaemonError::Io {
            path: path.to_owned(),
            source,
        })?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.mode() & 0o077 != 0
        {
            return Err(DaemonError::Config(
                "configuration must be a private non-symlink file with mode 0600".into(),
            ));
        }
        if metadata.len() > CONFIG_MAX_BYTES {
            return Err(DaemonError::Config(format!(
                "configuration exceeds {CONFIG_MAX_BYTES} bytes"
            )));
        }
        let bytes = fs::read(path).map_err(|source| DaemonError::Io {
            path: path.to_owned(),
            source,
        })?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|error| DaemonError::Config(format!("configuration is not UTF-8: {error}")))?;
        let config: Self = toml::from_str(text)
            .map_err(|error| DaemonError::Config(format!("invalid closed TOML: {error}")))?;
        config.validate()?;
        Ok(config)
    }

    /// Validate the decoded tiers without touching the filesystem.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-private/unsafe layout or a deployment-profile mismatch.
    pub fn validate(&self) -> Result<(), DaemonError> {
        let profile: DeploymentProfileDocument = serde_yaml_ng::from_slice(DEPLOYMENT_PROFILE)
            .map_err(|error| DaemonError::Config(format!("deployment profile invalid: {error}")))?;
        let static_config = &self.static_config;
        if static_config.supported_platform_profile != profile.profile_id
            || static_config.sandbox_policy != profile.provider_sandbox
        {
            return Err(DaemonError::Config(
                "static config disagrees with the released deployment profile".into(),
            ));
        }
        if !static_config.state_root.is_absolute()
            || !static_config.runtime_root.is_absolute()
            || !static_config.config_root.is_absolute()
            || !static_config.socket_endpoint.is_absolute()
            || !static_config.query_socket_endpoint.is_absolute()
            || !static_config
                .socket_endpoint
                .starts_with(&static_config.runtime_root)
            || !static_config
                .query_socket_endpoint
                .starts_with(&static_config.runtime_root)
            || static_config.query_socket_endpoint == static_config.socket_endpoint
            || static_config.query_capability_token_file.is_absolute()
            || static_config
                .query_capability_token_file
                .components()
                .any(|part| {
                    matches!(
                        part,
                        std::path::Component::ParentDir | std::path::Component::RootDir
                    )
                })
            || static_config.operational_database.is_absolute()
            || static_config.operational_database.components().any(|part| {
                matches!(
                    part,
                    std::path::Component::ParentDir | std::path::Component::RootDir
                )
            })
        {
            return Err(DaemonError::Config(
                "roots and socket must be absolute; database must be a safe relative path".into(),
            ));
        }
        for socket in [
            &static_config.socket_endpoint,
            &static_config.query_socket_endpoint,
        ] {
            let socket_bytes = socket.as_os_str().as_encoded_bytes();
            if socket_bytes.len() <= SOCKET_PATH_MAX_BYTES {
                continue;
            }
            return Err(DaemonError::Config(format!(
                "socket endpoint is {} bytes; maximum is {SOCKET_PATH_MAX_BYTES}",
                socket_bytes.len()
            )));
        }
        if !matches!(
            self.reloadable.log_level.as_str(),
            "error" | "warn" | "info" | "debug"
        ) || !(0.0..=1.0).contains(&self.reloadable.telemetry_sampling)
            || self.reloadable.soft_query_quota == 0
            || self.reloadable.maintenance_schedule.is_empty()
            || static_config.hard_limit_profile.is_empty()
        {
            return Err(DaemonError::Config(
                "reloadable values or hard-limit profile are outside their closed bounds".into(),
            ));
        }
        Ok(())
    }
}

/// Exact non-secret AC-G-62 discovery document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonDiscovery {
    pub daemon_instance_id: String,
    pub pid: u32,
    pub process_start_token: u128,
    pub socket_endpoint: PathBuf,
    pub query_socket_endpoint: PathBuf,
    pub rpc_minimum_minor: u16,
    pub rpc_maximum_minor: u16,
    pub basic_readiness: bool,
    pub startup_time_unix_ms: u128,
    pub public_bundle_versions: BTreeMap<String, String>,
}

/// One administrative request over the private Unix socket.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdminCommand {
    Status,
    CutoverStatus,
    Stop,
    Drain,
}

/// Closed administrative request envelope.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdminRequest {
    pub command: AdminCommand,
}

/// Closed workspace-administration request family on the private admin socket.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspaceAdminCommand {
    Add {
        root: PathBuf,
    },
    List,
    Show {
        workspace_id: [u8; 16],
    },
    Relink {
        workspace_id: [u8; 16],
        new_root: PathBuf,
        proof: RelinkProof,
    },
    Configure {
        workspace_id: [u8; 16],
        profile_manifest: PathBuf,
    },
    Enable {
        workspace_id: [u8; 16],
    },
    Disable {
        workspace_id: [u8; 16],
    },
    Reconcile {
        workspace_id: [u8; 16],
    },
    Remove {
        workspace_id: [u8; 16],
        policy: RemovalPolicy,
        purge_confirmations: u8,
    },
}

/// Scope-tagged administrative request; query protocols have no such variants.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "scope", content = "request", rename_all = "snake_case")]
pub enum AdminEnvelope {
    Daemon(AdminRequest),
    Workspace(WorkspaceAdminCommand),
}

/// Released non-secret workspace-health response shape.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceHealthStatus {
    pub workspace_id: String,
    pub lifecycle: String,
    pub source_trust: String,
    pub event_stream_health: String,
    pub git_acceleration: String,
    pub source_generation: u64,
    pub inventory_digest: Option<String>,
    pub active_snapshot: Option<String>,
    pub readiness: String,
    pub reconciliation_count: u64,
}

/// Liveness response kept distinct from workspace readiness.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdminResponse {
    pub accepted: bool,
    pub daemon_liveness: String,
    pub workspace_readiness: String,
    pub shutdown_mode: Option<String>,
    pub workspaces: Vec<WorkspaceRecord>,
    pub workspace_health: Vec<WorkspaceHealthStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cutover_status: Option<Vec<CutoverAdminStatus>>,
    pub error_code: Option<String>,
}

/// Read-only projection of the durable cutover journal plus current platform observation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CutoverAdminStatus {
    pub workspace_id: [u8; 16],
    pub durable_phase: String,
    pub durable_event_id: Option<String>,
    pub durable_sequence: Option<u64>,
    pub admission: String,
    pub code: String,
    pub remediation: String,
}

/// Ordered evidence returned after a joined daemon shutdown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DaemonExit {
    pub drained: bool,
    pub shutdown_steps: Vec<&'static str>,
}

/// Stable daemon lifecycle failures.
#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("daemon I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid daemon configuration: {0}")]
    Config(String),
    #[error(
        "production serving requires an explicitly constructed programmatic daemon composition"
    )]
    ProgrammaticCompositionRequired,
    #[error("daemon singleton lease is already held")]
    LeaseHeld,
    #[error("administrative protocol failure: {0}")]
    Admin(String),
    #[error("joined shutdown failed after {completed_steps:?}: {detail}")]
    Shutdown {
        completed_steps: Vec<&'static str>,
        detail: String,
    },
    #[error(transparent)]
    QueryTransport(#[from] QueryTransportError),
    #[error(transparent)]
    ProgrammaticBackend(#[from] ProgrammaticSemanticQueryBackendError),
    #[error(transparent)]
    ProgrammaticCommandRecovery(#[from] ProgrammaticCommandRecoveryError),
    #[error("programmatic daemon composition shutdown failed: {0}")]
    ProgrammaticCompositionShutdown(#[from] ProgrammaticDaemonCompositionShutdownError),
}

fn record_shutdown_step<E: std::fmt::Display>(
    completed_steps: &mut Vec<&'static str>,
    step: &'static str,
    outcome: Result<(), E>,
) -> Result<(), DaemonError> {
    outcome.map_err(|error| DaemonError::Shutdown {
        completed_steps: completed_steps.clone(),
        detail: error.to_string(),
    })?;
    completed_steps.push(step);
    tracing::info!(
        shutdown_step = step,
        "joined daemon shutdown step completed"
    );
    Ok(())
}

fn private_directory(path: &Path) -> Result<(), DaemonError> {
    if !path.exists() {
        fs::create_dir_all(path).map_err(|source| DaemonError::Io {
            path: path.to_owned(),
            source,
        })?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|source| {
            DaemonError::Io {
                path: path.to_owned(),
                source,
            }
        })?;
    }
    let metadata = fs::symlink_metadata(path).map_err(|source| DaemonError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || metadata.mode() & 0o077 != 0 {
        return Err(DaemonError::Config(format!(
            "private root {} must be a non-symlink directory with mode 0700",
            path.display()
        )));
    }
    Ok(())
}

fn private_file(path: &Path, bytes: &[u8]) -> Result<(), DaemonError> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .map_err(|source| DaemonError::Io {
            path: path.to_owned(),
            source,
        })?;
    file.write_all(bytes).map_err(|source| DaemonError::Io {
        path: path.to_owned(),
        source,
    })?;
    file.sync_all().map_err(|source| DaemonError::Io {
        path: path.to_owned(),
        source,
    })
}

fn sync_directory(path: &Path) -> Result<(), DaemonError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| DaemonError::Io {
            path: path.to_owned(),
            source,
        })
}

/// Held singleton lock and atomically published endpoint metadata.
pub struct DaemonLease {
    lock: File,
    discovery_path: PathBuf,
    runtime_root: PathBuf,
}

impl DaemonLease {
    /// Acquire the state-root singleton before touching an existing endpoint.
    ///
    /// # Errors
    ///
    /// Returns permission, lock-contention, serialization, or publication errors.
    pub fn acquire(config: &DaemonConfig) -> Result<Self, DaemonError> {
        for root in [
            &config.static_config.state_root,
            &config.static_config.runtime_root,
            &config.static_config.config_root,
        ] {
            private_directory(root)?;
        }
        let lock_path = config.static_config.state_root.join("daemon.lock");
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .mode(0o600)
            .open(&lock_path)
            .map_err(|source| DaemonError::Io {
                path: lock_path.clone(),
                source,
            })?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(DaemonError::LeaseHeld),
            Err(TryLockError::Error(source)) => {
                return Err(DaemonError::Io {
                    path: lock_path,
                    source,
                });
            }
        }
        let discovery_path = config.static_config.runtime_root.join("daemon.json");
        Ok(Self {
            lock,
            discovery_path,
            runtime_root: config.static_config.runtime_root.clone(),
        })
    }

    /// Publish endpoint metadata only after the private socket is bound.
    ///
    /// # Errors
    ///
    /// Returns serialization, tempfile, fsync, or atomic-rename failures.
    pub fn publish(&self, discovery: &DaemonDiscovery) -> Result<(), DaemonError> {
        let temporary_path = self
            .runtime_root
            .join(format!(".daemon.json.{}.tmp", discovery.pid));
        if temporary_path.exists() {
            fs::remove_file(&temporary_path).map_err(|source| DaemonError::Io {
                path: temporary_path.clone(),
                source,
            })?;
        }
        let mut bytes = serde_json::to_vec_pretty(discovery)
            .map_err(|error| DaemonError::Admin(format!("discovery serialization: {error}")))?;
        bytes.push(b'\n');
        private_file(&temporary_path, &bytes)?;
        fs::rename(&temporary_path, &self.discovery_path).map_err(|source| DaemonError::Io {
            path: self.discovery_path.clone(),
            source,
        })?;
        sync_directory(&self.runtime_root)
    }
}

impl Drop for DaemonLease {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.discovery_path);
        let _ = sync_directory(&self.runtime_root);
        let _ = self.lock.unlock();
    }
}

fn now_millis() -> Result<u128, DaemonError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|error| DaemonError::Admin(format!("system clock before Unix epoch: {error}")))
}

fn cutover_admin_statuses(statuses: Vec<ProductionCutoverStatus>) -> Vec<CutoverAdminStatus> {
    statuses
        .into_iter()
        .map(|observed| {
            let (durable_phase, durable_event_id, durable_sequence) =
                match observed.status.durable_state {
                    DurableCutoverState::NotStarted => ("not_started".to_owned(), None, None),
                    DurableCutoverState::At {
                        phase,
                        event_id,
                        sequence,
                    } => {
                        let phase = match phase {
                            CutoverPhase::TargetProved => "target_proved",
                            CutoverPhase::PredecessorFenced => "predecessor_fenced",
                            CutoverPhase::TargetServing => "target_serving",
                            CutoverPhase::TargetMutating => "target_mutating",
                            CutoverPhase::Complete => "complete",
                        };
                        (
                            phase.to_owned(),
                            Some(crate::integrity::frame_digest(*event_id.as_bytes())),
                            Some(sequence),
                        )
                    }
                };
            let admission = match observed.status.admission {
                CutoverAdmission::Closed => "closed",
                CutoverAdmission::TargetReadOnly => "target_read_only",
                CutoverAdmission::TargetReadWrite => "target_read_write",
            };
            CutoverAdminStatus {
                workspace_id: *observed.workspace_id.as_bytes(),
                durable_phase,
                durable_event_id,
                durable_sequence,
                admission: admission.to_owned(),
                code: observed.status.code.to_owned(),
                remediation: observed.status.remediation.to_owned(),
            }
        })
        .collect()
}

fn programmatic_public_bundle_versions(
    composition: &ProgrammaticDaemonComposition,
) -> Result<BTreeMap<String, String>, DaemonError> {
    let mut versions = BTreeMap::new();
    for (_, workspace) in composition.workspaces() {
        let workspace_id = workspace
            .public_workspace_id()
            .map_err(|error| DaemonError::Admin(error.to_string()))?;
        let releases = workspace.startup_observation().releases;
        for (release_kind, release_pin) in [
            ("input", *releases.input_release().as_bytes()),
            ("program", *releases.program_release().as_bytes()),
            ("provider", *releases.provider_release().as_bytes()),
            ("application", *releases.application_release().as_bytes()),
            ("source-authority", *releases.source_authority().as_bytes()),
        ] {
            let key = format!("codefabric.programmatic.{workspace_id}.{release_kind}");
            if versions
                .insert(key, crate::integrity::frame_digest(release_pin))
                .is_some()
            {
                return Err(DaemonError::Admin(
                    "programmatic discovery release identity is duplicated".to_owned(),
                ));
            }
        }
    }
    if versions.is_empty() {
        return Err(DaemonError::Admin(
            "programmatic discovery has no explicit workspace releases".to_owned(),
        ));
    }
    Ok(versions)
}

fn discovery(
    config: &DaemonConfig,
    public_bundle_versions: BTreeMap<String, String>,
) -> Result<DaemonDiscovery, DaemonError> {
    let startup_time_unix_ms = now_millis()?;
    let pid = std::process::id();
    let mut identity = crate::identity::semantic_fingerprint(
        crate::identity::SemanticFingerprintDomain::UnframedId16,
    );
    identity.update(&pid.to_be_bytes());
    identity.update(&startup_time_unix_ms.to_be_bytes());
    identity.update(
        config
            .static_config
            .state_root
            .as_os_str()
            .as_encoded_bytes(),
    );
    let daemon_instance_id = crate::integrity::frame_digest(identity.finalize())[3..35].to_owned();
    Ok(DaemonDiscovery {
        daemon_instance_id,
        pid,
        process_start_token: startup_time_unix_ms,
        socket_endpoint: config.static_config.socket_endpoint.clone(),
        query_socket_endpoint: config.static_config.query_socket_endpoint.clone(),
        rpc_minimum_minor: 0,
        rpc_maximum_minor: 0,
        basic_readiness: false,
        startup_time_unix_ms,
        public_bundle_versions,
    })
}

async fn read_request(stream: &mut UnixStream) -> Result<AdminEnvelope, DaemonError> {
    let mut line = Vec::new();
    let reader = BufReader::new(stream);
    let observed = reader
        .take((ADMIN_MESSAGE_MAX_BYTES + 1) as u64)
        .read_until(b'\n', &mut line)
        .await
        .map_err(|source| DaemonError::Admin(format!("request read: {source}")))?;
    if observed == 0 || observed > ADMIN_MESSAGE_MAX_BYTES {
        return Err(DaemonError::Admin(
            "request exceeds bounded line framing".into(),
        ));
    }
    serde_json::from_slice(&line)
        .map_err(|error| DaemonError::Admin(format!("invalid request: {error}")))
}

async fn write_response(
    stream: &mut UnixStream,
    response: &AdminResponse,
) -> Result<(), DaemonError> {
    let mut bytes = serde_json::to_vec(response)
        .map_err(|error| DaemonError::Admin(format!("response serialization: {error}")))?;
    bytes.push(b'\n');
    stream
        .write_all(&bytes)
        .await
        .map_err(|source| DaemonError::Admin(format!("response write: {source}")))
}

fn query_capability_token(config: &StaticConfig) -> Result<Vec<u8>, DaemonError> {
    let path = config.config_root.join(&config.query_capability_token_file);
    let metadata = fs::symlink_metadata(&path).map_err(|source| DaemonError::Io {
        path: path.clone(),
        source,
    })?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.mode() & 0o077 != 0
        || !(16..=4_096).contains(&metadata.len())
    {
        return Err(DaemonError::Config(
            "query capability token must be a private bounded non-symlink file".to_owned(),
        ));
    }
    let mut token = fs::read(&path).map_err(|source| DaemonError::Io { path, source })?;
    while matches!(token.last(), Some(b'\n' | b'\r')) {
        token.pop();
    }
    if token.len() < 16 {
        return Err(DaemonError::Config(
            "query capability token is shorter than 16 bytes".to_owned(),
        ));
    }
    Ok(token)
}

/// Run the local administrative service until a joined stop or no-work drain completes.
///
/// # Errors
///
/// Returns startup, permission, socket, protocol, or joined-shutdown failures.
#[allow(clippy::too_many_lines)] // The ordered lifecycle is kept visible as one joined sequence.
pub async fn serve(config: DaemonConfig) -> Result<DaemonExit, DaemonError> {
    config.validate()?;
    Err(DaemonError::ProgrammaticCompositionRequired)
}

/// Run the production daemon over one fully constructed programmatic composition.
///
/// This is the only production serving entry that can open query ingress. The caller must first
/// construct every workspace from exact typed inputs and durable activation authority. The
/// composition remains owned until the daemon has closed its endpoints and joined workers, then
/// its admission and command runtimes are shut down even when serving fails.
///
/// # Errors
///
/// Returns composition, identity, daemon lifecycle, or joined-cleanup failures. No default
/// workspace, bootstrap catalog, or empty-success backend is synthesized.
pub async fn serve_programmatic(
    config: DaemonConfig,
    mut composition: ProgrammaticDaemonComposition,
    ports: ProgrammaticSemanticQueryPorts,
) -> Result<DaemonExit, DaemonError> {
    let serve_result = async {
        let mut claims = Vec::with_capacity(composition.workspaces().len());
        for (_, workspace) in composition.workspaces() {
            let startup = workspace.startup_observation();
            tracing::info!(
                factory_id = startup.factory_id,
                workspace_id = ?startup.workspace_id,
                epoch_id = ?startup.epoch_id,
                activation_event_id = ?startup.activation_event_id,
                active_fence = ?startup.active_fence,
                source_generation = startup.source_generation,
                provider_set_pin = ?startup.provider_set_pin,
                overlay_segment_set_pin = ?startup.overlay_segment_set_pin,
                policy_set_pin = ?startup.policy_set_pin,
                proof_receipt_pin = ?startup.proof_receipt_pin,
                activation_control_root = %startup.activation_control_root,
                activation_control_version = startup.activation_control_version,
                input_release = ?startup.releases.input_release(),
                program_release = ?startup.releases.program_release(),
                provider_release = ?startup.releases.provider_release(),
                application_release = ?startup.releases.application_release(),
                source_authority = ?startup.releases.source_authority(),
                program_catalog_pin = ?startup.program_catalog_pin,
                execution_catalog_pin = ?startup.execution_catalog_pin,
                program_release_pin = ?startup.program_release_pin,
                producer_closure_proof_pin = ?startup.producer_closure_proof_pin,
                request_owned_relation_limits_pin = ?startup.request_owned_relation_limits_pin,
                resource_policy_pin = ?startup.resource_policy_pin,
                runtime_configuration = %startup.runtime_configuration,
                schema_authority = %startup.schema_authority,
                relation_count = startup.relation_count,
                table_versions = ?startup.table_versions,
                "programmatic workspace composition admitted"
            );
            claims.push(WorkspaceClaim {
                workspace_id: workspace
                    .public_workspace_id()
                    .map_err(|error| DaemonError::Admin(error.to_string()))?,
                repository_id: None,
                worktree_id: None,
                workspace_kind: "programmatic".to_owned(),
                readiness: WorkspaceReadiness::Ready as i32,
                permission_claims: vec!["query".to_owned()],
            });
        }
        let cutover_configured = ProductionForwardCutoverController::open_if_configured(&config)
            .map_err(|error| DaemonError::Config(error.to_string()))?
            .is_some();
        let backend = Arc::new(if cutover_configured || !composition.command_runtimes_ready() {
            ProgrammaticSemanticQueryBackend::try_new_staged(&composition, ports)?
        } else {
            ProgrammaticSemanticQueryBackend::try_new(&composition, ports)?
        });
        let published_results = Arc::clone(backend.published_results());
        serve_with_programmatic_query_backend(
            config,
            Arc::clone(&backend),
            claims,
            None,
            published_results,
            &composition,
            Some(backend),
        )
        .await
    }
    .await;
    let shutdown_result = composition.shutdown().await;
    match (serve_result, shutdown_result) {
        (Ok(mut exit), Ok(())) => {
            exit.shutdown_steps.push("close-programmatic-composition");
            Ok(exit)
        }
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error.into()),
        (Err(error), Err(cleanup)) => Err(DaemonError::Shutdown {
            completed_steps: Vec::new(),
            detail: format!("{error}; programmatic composition cleanup also failed: {cleanup}"),
        }),
    }
}

/// Serve an already-composed programmatic fabric without selecting any bootstrap/default path.
///
/// The supplied result registry must be the daemon-wide registry retained by the programmatic
/// workspace factory and relational backend. Legacy workspace/model/ontology administrative
/// ingress is rejected rather than being translated into an unregistered second mutation path.
pub(crate) async fn serve_with_programmatic_query_backend<B: SemanticQueryBackend>(
    config: DaemonConfig,
    query_backend: Arc<B>,
    additional_claims: Vec<WorkspaceClaim>,
    query_allowed_uid_override: Option<u32>,
    published_results: Arc<PublishedArrowResultRegistry>,
    composition: &ProgrammaticDaemonComposition,
    staged_backend: Option<Arc<ProgrammaticSemanticQueryBackend>>,
) -> Result<DaemonExit, DaemonError> {
    let public_bundle_versions = programmatic_public_bundle_versions(composition)?;
    serve_query_transport(
        config,
        query_backend,
        additional_claims,
        query_allowed_uid_override,
        Some(published_results),
        Some(composition),
        public_bundle_versions,
        staged_backend,
    )
    .await
}

#[allow(clippy::too_many_lines)] // The ordered lifecycle is kept visible as one joined sequence.
async fn serve_query_transport<B: SemanticQueryBackend>(
    config: DaemonConfig,
    query_backend: Arc<B>,
    additional_claims: Vec<WorkspaceClaim>,
    query_allowed_uid_override: Option<u32>,
    published_results: Option<Arc<PublishedArrowResultRegistry>>,
    programmatic_composition: Option<&ProgrammaticDaemonComposition>,
    public_bundle_versions: BTreeMap<String, String>,
    staged_backend: Option<Arc<ProgrammaticSemanticQueryBackend>>,
) -> Result<DaemonExit, DaemonError> {
    config.validate()?;
    for root in [
        &config.static_config.state_root,
        &config.static_config.runtime_root,
        &config.static_config.config_root,
    ] {
        private_directory(root)?;
    }
    if public_bundle_versions.is_empty()
        || public_bundle_versions
            .iter()
            .any(|(bundle, version)| bundle.trim().is_empty() || version.trim().is_empty())
    {
        return Err(DaemonError::Config(
            "released discovery identities must be explicit and non-empty".to_owned(),
        ));
    }
    let cutover_controller = ProductionForwardCutoverController::open_if_configured(&config)
        .map_err(|error| DaemonError::Config(error.to_string()))?;
    let lease = DaemonLease::acquire(&config)?;
    let mut claims = BTreeMap::new();
    for claim in additional_claims {
        let workspace_id = claim.workspace_id.clone();
        if claims.insert(workspace_id.clone(), claim).is_some() {
            return Err(DaemonError::Config(format!(
                "workspace claim {workspace_id} is duplicated"
            )));
        }
    }
    let admitted_workspace_count = claims.len();
    if config.static_config.socket_endpoint.exists() {
        fs::remove_file(&config.static_config.socket_endpoint).map_err(|source| {
            DaemonError::Io {
                path: config.static_config.socket_endpoint.clone(),
                source,
            }
        })?;
    }
    let listener = UnixListener::bind(&config.static_config.socket_endpoint).map_err(|source| {
        DaemonError::Io {
            path: config.static_config.socket_endpoint.clone(),
            source,
        }
    })?;
    fs::set_permissions(
        &config.static_config.socket_endpoint,
        fs::Permissions::from_mode(0o600),
    )
    .map_err(|source| DaemonError::Io {
        path: config.static_config.socket_endpoint.clone(),
        source,
    })?;
    let allowed_uid = fs::metadata(&config.static_config.socket_endpoint)
        .map_err(|source| DaemonError::Io {
            path: config.static_config.socket_endpoint.clone(),
            source,
        })?
        .uid();
    if config.static_config.query_socket_endpoint.exists() {
        fs::remove_file(&config.static_config.query_socket_endpoint).map_err(|source| {
            DaemonError::Io {
                path: config.static_config.query_socket_endpoint.clone(),
                source,
            }
        })?;
    }
    let query_token = query_capability_token(&config.static_config)?;
    let query_authorization = QueryAuthorization::new(&query_token, claims.into_values().collect())
        .map_err(|status| DaemonError::Config(status.to_string()))?;
    let result_root = config.static_config.state_root.join("query-results");
    let query_service = ProductionQueryService::new(
        query_backend,
        ResultArtifactStore::new(result_root.clone()).map_err(|source| DaemonError::Io {
            path: result_root,
            source,
        })?,
        query_authorization,
        crate::freshness::FreshnessBarrier::default(),
        Duration::from_secs(2),
    );
    let query_service = match published_results {
        Some(published_results) => query_service.with_published_results(published_results),
        None => query_service,
    };
    let (query_shutdown, query_shutdown_receiver) = oneshot::channel();
    let query_socket = config.static_config.query_socket_endpoint.clone();
    let query_allowed_uid = query_allowed_uid_override.unwrap_or(allowed_uid);
    let query_task = tokio::spawn(async move {
        serve_query_uds(
            &query_socket,
            query_allowed_uid,
            query_service,
            async move {
                let _ = query_shutdown_receiver.await;
            },
        )
        .await
    });
    for _ in 0..1_000 {
        if config.static_config.query_socket_endpoint.exists() || query_task.is_finished() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    if !config.static_config.query_socket_endpoint.exists() {
        return Err(DaemonError::Admin(
            "query service did not bind its configured endpoint".to_owned(),
        ));
    }
    let mut published_discovery = discovery(&config, public_bundle_versions)?;
    lease.publish(&published_discovery)?;
    let startup_admission = async {
        if let Some(composition) = programmatic_composition
            && !composition.command_runtimes_ready()
            && !composition.retry_pending_command_recovery().await?
        {
            return Err(DaemonError::Admin(
                "programmatic command recovery remained pending after live endpoint readback"
                    .to_owned(),
            ));
        }
        if let Some(controller) = &cutover_controller {
            let composition = programmatic_composition.ok_or_else(|| {
                DaemonError::Config(
                    "configured cutover requires the production programmatic composition"
                        .to_owned(),
                )
            })?;
            controller
                .require_target_read_write(&config, composition)
                .await
                .map_err(|error| {
                    DaemonError::Config(format!(
                        "forward-cutover production admission failed closed: {error}"
                    ))
                })?;
        }
        if let Some(backend) = &staged_backend {
            let composition = programmatic_composition.ok_or_else(|| {
                DaemonError::Config(
                    "staged query backend has no programmatic composition authority".to_owned(),
                )
            })?;
            if !composition.command_runtimes_ready() {
                return Err(DaemonError::Admin(
                    "staged query backend cannot open before command recovery".to_owned(),
                ));
            }
            backend.open_after_startup_authority();
        }
        published_discovery.basic_readiness = true;
        lease.publish(&published_discovery)?;
        Ok(())
    }
    .await;
    if let Err(error) = startup_admission {
        let _ = query_shutdown.send(());
        let _ = query_task.await;
        let _ = fs::remove_file(&config.static_config.socket_endpoint);
        if config.static_config.query_socket_endpoint.exists() {
            let _ = fs::remove_file(&config.static_config.query_socket_endpoint);
        }
        return Err(error);
    }
    tracing::info!(lifecycle = "serve", "daemon administrative ingress opened");

    let drained = loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|source| DaemonError::Admin(format!("accept: {source}")))?;
        if SameUserInterceptor::new(allowed_uid)
            .authenticate_stream(&stream)
            .is_err()
        {
            continue;
        }
        let request = read_request(&mut stream).await?;
        let (response, shutdown_mode) = match request {
            AdminEnvelope::Daemon(request) => {
                let shutdown_mode = match request.command {
                    AdminCommand::Status => None,
                    AdminCommand::CutoverStatus => None,
                    AdminCommand::Stop => Some("stop".to_owned()),
                    AdminCommand::Drain => Some("drain".to_owned()),
                };
                let workspace_readiness = if admitted_workspace_count == 0 {
                    "NO_WORKSPACES_READY"
                } else {
                    "WORKSPACES_READY"
                }
                .to_owned();
                let (accepted, workspace_readiness, cutover_status, error_code) = if request.command
                    == AdminCommand::CutoverStatus
                {
                    match (&cutover_controller, programmatic_composition) {
                        (Some(controller), Some(composition)) => {
                            match controller.operator_statuses(&config, composition).await {
                                Ok(statuses) => {
                                    let accepted = statuses.iter().all(|status| {
                                        status.status.admission == CutoverAdmission::TargetReadWrite
                                    });
                                    let error_code =
                                        (!accepted).then(|| "CUTOVER_ADMISSION_CLOSED".to_owned());
                                    (
                                        accepted,
                                        if accepted {
                                            "CUTOVER_TARGET_READ_WRITE"
                                        } else {
                                            "CUTOVER_ADMISSION_CLOSED"
                                        }
                                        .to_owned(),
                                        Some(cutover_admin_statuses(statuses)),
                                        error_code,
                                    )
                                }
                                Err(error) => {
                                    tracing::warn!(%error, "cutover status observation failed closed");
                                    (
                                        false,
                                        "CUTOVER_OBSERVATION_FAILED".to_owned(),
                                        None,
                                        Some("CUTOVER_OBSERVATION_FAILED".to_owned()),
                                    )
                                }
                            }
                        }
                        _ => (
                            false,
                            "CUTOVER_DEPLOYMENT_NOT_CONFIGURED".to_owned(),
                            None,
                            Some("CUTOVER_DEPLOYMENT_NOT_CONFIGURED".to_owned()),
                        ),
                    }
                } else {
                    (true, workspace_readiness, None, None)
                };
                (
                    AdminResponse {
                        accepted,
                        daemon_liveness: "LIVE".to_owned(),
                        workspace_readiness,
                        shutdown_mode: shutdown_mode.clone(),
                        workspaces: Vec::new(),
                        workspace_health: Vec::new(),
                        cutover_status,
                        error_code,
                    },
                    shutdown_mode,
                )
            }
            AdminEnvelope::Workspace(_) => (
                AdminResponse {
                    accepted: false,
                    daemon_liveness: "LIVE".to_owned(),
                    workspace_readiness: "UNCHANGED".to_owned(),
                    shutdown_mode: None,
                    workspaces: Vec::new(),
                    workspace_health: Vec::new(),
                    cutover_status: None,
                    error_code: Some("PROGRAMMATIC_COMMAND_INGRESS_REQUIRED".to_owned()),
                },
                None,
            ),
        };
        write_response(&mut stream, &response).await?;
        if let Some(mode) = shutdown_mode {
            break mode == "drain";
        }
    };

    let mut steps = Vec::new();
    record_shutdown_step(
        &mut steps,
        "mark-stopping",
        Ok::<(), std::convert::Infallible>(()),
    )?;
    drop(listener);
    let initial_programmatic_admission_result =
        programmatic_composition.map_or(Ok(()), |composition| {
            composition
                .close_query_admission()
                .map_err(|error| error.to_string())
        });
    let programmatic_command_result = match programmatic_composition {
        Some(composition) => {
            let result = composition
                .shutdown_commands()
                .await
                .map_err(|error| error.to_string());
            steps.push("drain-programmatic-commands");
            result
        }
        None => Ok(()),
    };
    let _ = query_shutdown.send(());
    record_shutdown_step(
        &mut steps,
        "close-ingress",
        Ok::<(), std::convert::Infallible>(()),
    )?;
    let query_result = query_task
        .await
        .map_err(|error| error.to_string())
        .and_then(|result| result.map_err(|error| error.to_string()));
    // A command may already own the activation barrier when shutdown begins. Joining the command
    // actor before retrying admission closure lets its commit/readback path settle the exact head.
    let programmatic_admission_result = match initial_programmatic_admission_result {
        Ok(()) => Ok(()),
        Err(initial) => programmatic_composition.map_or(Err(initial.clone()), |composition| {
            composition.close_query_admission().map_err(|retry| {
                format!("initial admission close failed: {initial}; retry failed: {retry}")
            })
        }),
    };
    record_shutdown_step(
        &mut steps,
        "await-workers",
        programmatic_admission_result
            .and(programmatic_command_result)
            .and(query_result),
    )?;
    record_shutdown_step(
        &mut steps,
        "close-durable-stores",
        Ok::<(), std::convert::Infallible>(()),
    )?;
    let retire_endpoints = fs::remove_file(&config.static_config.socket_endpoint).and_then(|()| {
        if config.static_config.query_socket_endpoint.exists() {
            fs::remove_file(&config.static_config.query_socket_endpoint)
        } else {
            Ok(())
        }
    });
    record_shutdown_step(&mut steps, "retire-endpoint-metadata", retire_endpoints)?;
    drop(lease);
    record_shutdown_step(
        &mut steps,
        "release-singleton-lease",
        Ok::<(), std::convert::Infallible>(()),
    )?;
    Ok(DaemonExit {
        drained,
        shutdown_steps: steps,
    })
}

/// Send one bounded administrative command through a discovery document.
///
/// # Errors
///
/// Returns discovery, connection, framing, or response-validation failures.
pub async fn administer(
    discovery_path: &Path,
    command: AdminCommand,
) -> Result<AdminResponse, DaemonError> {
    administer_envelope(
        discovery_path,
        &AdminEnvelope::Daemon(AdminRequest { command }),
    )
    .await
}

/// Send one closed workspace command through the private admin discovery endpoint.
///
/// # Errors
///
/// Returns discovery, connection, framing, or response-validation failures.
pub async fn administer_workspace(
    discovery_path: &Path,
    command: WorkspaceAdminCommand,
) -> Result<AdminResponse, DaemonError> {
    administer_envelope(discovery_path, &AdminEnvelope::Workspace(command)).await
}

async fn administer_envelope(
    discovery_path: &Path,
    request: &AdminEnvelope,
) -> Result<AdminResponse, DaemonError> {
    let bytes = fs::read(discovery_path).map_err(|source| DaemonError::Io {
        path: discovery_path.to_owned(),
        source,
    })?;
    if bytes.len() > ADMIN_MESSAGE_MAX_BYTES {
        return Err(DaemonError::Admin("discovery file exceeds limit".into()));
    }
    let discovery: DaemonDiscovery = serde_json::from_slice(&bytes)
        .map_err(|error| DaemonError::Admin(format!("invalid discovery document: {error}")))?;
    let mut stream = UnixStream::connect(&discovery.socket_endpoint)
        .await
        .map_err(|source| DaemonError::Admin(format!("connect: {source}")))?;
    let mut request = serde_json::to_vec(request)
        .map_err(|error| DaemonError::Admin(format!("request serialization: {error}")))?;
    request.push(b'\n');
    stream
        .write_all(&request)
        .await
        .map_err(|source| DaemonError::Admin(format!("request write: {source}")))?;
    let mut response = Vec::new();
    BufReader::new(stream)
        .take((ADMIN_MESSAGE_MAX_BYTES + 1) as u64)
        .read_until(b'\n', &mut response)
        .await
        .map_err(|source| DaemonError::Admin(format!("response read: {source}")))?;
    if response.len() > ADMIN_MESSAGE_MAX_BYTES {
        return Err(DaemonError::Admin("response exceeds limit".into()));
    }
    serde_json::from_slice(&response)
        .map_err(|error| DaemonError::Admin(format!("invalid response: {error}")))
}

/// Poll until an atomically published discovery document appears.
///
/// # Errors
///
/// Returns a timeout when the daemon does not finish startup within the deadline.
pub async fn wait_for_discovery(path: &Path, deadline: Duration) -> Result<(), DaemonError> {
    tokio::time::timeout(deadline, async {
        while !path.is_file() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| DaemonError::Admin("daemon discovery deadline exceeded".into()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::registries::FreshnessState;

    struct TestQueryBackend;

    #[async_trait::async_trait]
    impl SemanticQueryBackend for TestQueryBackend {
        fn validate_execution_request(
            &self,
            _request: &crate::semantic_query_contract::ParsedSemanticRequest,
        ) -> Result<(), crate::semantic_query_contract::SemanticQueryError> {
            Ok(())
        }

        async fn execute(
            &self,
            _request: crate::semantic_query_contract::ParsedSemanticRequest,
            _freshness: FreshnessState,
            _cancellation: crate::cancellation::Cancellation,
            _context: crate::query_service::SemanticBackendExecutionContext,
            artifacts: crate::fabric::QueryExecutionArtifactAccumulator,
        ) -> crate::query_service::SemanticBackendOutcome {
            artifacts.set_failure("test_backend");
            crate::query_service::SemanticBackendOutcome::Failed {
                error: crate::semantic_query_contract::SemanticQueryError::Invalid(
                    "test backend has no workspace".to_owned(),
                ),
                evidence: artifacts.snapshot(),
            }
        }

        async fn public_snapshot(
            &self,
            _workspace_id: &str,
        ) -> Result<
            crate::semantic_query_contract::SemanticSnapshotResponse,
            crate::semantic_query_contract::SemanticQueryError,
        > {
            Err(crate::semantic_query_contract::SemanticQueryError::Invalid(
                "test backend has no workspace".to_owned(),
            ))
        }
    }

    #[test]
    fn wp61_operational_acceptance() {
        let mut completed = Vec::new();
        record_shutdown_step(&mut completed, "stop-admission", Ok::<_, &str>(())).unwrap();
        record_shutdown_step(&mut completed, "drain-queries", Ok::<_, &str>(())).unwrap();
        let error = record_shutdown_step(
            &mut completed,
            "flush-publications",
            Err::<(), _>("injected flush failure"),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            DaemonError::Shutdown {
                completed_steps,
                detail,
            } if completed_steps == ["stop-admission", "drain-queries"]
                && detail == "injected flush failure"
        ));
        assert_eq!(completed, ["stop-admission", "drain-queries"]);
    }

    fn config(root: &Path) -> DaemonConfig {
        private_directory(&root.join("config")).unwrap();
        let token = root.join("config/query.capability");
        if !token.exists() {
            private_file(&token, b"test-query-capability-token").unwrap();
        }
        DaemonConfig {
            static_config: StaticConfig {
                state_root: root.join("state"),
                runtime_root: root.join("runtime"),
                config_root: root.join("config"),
                socket_endpoint: root.join("runtime/admin.sock"),
                query_socket_endpoint: root.join("runtime/query.sock"),
                query_capability_token_file: PathBuf::from("query.capability"),
                operational_database: PathBuf::from("operational.sqlite3"),
                sandbox_policy: "required-for-untrusted".to_owned(),
                hard_limit_profile: "daemon-default-v1".to_owned(),
                supported_platform_profile: "local-workstation-v1".to_owned(),
            },
            reloadable: ReloadableConfig {
                log_level: "info".to_owned(),
                telemetry_sampling: 0.1,
                soft_query_quota: 4,
                maintenance_schedule: "daily-idle".to_owned(),
            },
        }
    }

    fn released_wire_compatibility_versions() -> BTreeMap<String, String> {
        BTreeMap::from([(
            "codefabric.released-wire-compatibility".to_owned(),
            "1.3".to_owned(),
        )])
    }

    fn write_config(root: &Path, extra: &str) -> PathBuf {
        private_directory(&root.join("config")).unwrap();
        let token = root.join("config/query.capability");
        if !token.exists() {
            private_file(&token, b"test-query-capability-token").unwrap();
        }
        let path = root.join("config/codefabric.toml");
        let source = format!(
            r#"
[static_config]
state_root = {state:?}
runtime_root = {runtime:?}
config_root = {config:?}
socket_endpoint = {socket:?}
query_socket_endpoint = {query_socket:?}
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
{extra}
"#,
            state = root.join("state").display().to_string(),
            runtime = root.join("runtime").display().to_string(),
            config = root.join("config").display().to_string(),
            socket = root.join("runtime/admin.sock").display().to_string(),
            query_socket = root.join("runtime/query.sock").display().to_string(),
        );
        private_file(&path, source.as_bytes()).unwrap();
        path
    }

    #[test]
    fn wp12_structural_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let path = write_config(root.path(), "");
        let actual = DaemonConfig::load(&path).unwrap();
        assert_eq!(actual, config(root.path()));
        let profile: DeploymentProfileDocument =
            serde_yaml_ng::from_slice(DEPLOYMENT_PROFILE).unwrap();
        assert_eq!(
            actual.static_config.supported_platform_profile,
            profile.profile_id
        );
        assert_eq!(
            actual.static_config.sandbox_policy,
            profile.provider_sandbox
        );
    }

    #[test]
    fn wp67_structural_acceptance() {
        let schema: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../contracts/rpc/admin-line-protocol.schema.json"
        ))
        .unwrap();
        assert_eq!(
            schema["$id"],
            "https://codefabric.dev/contracts/rpc/admin-line-protocol.schema.json"
        );
        assert_eq!(schema["x-codefabric-framing"]["delimiter"], "LF");
        assert_eq!(
            schema["x-codefabric-framing"]["maximum_encoded_line_bytes"],
            ADMIN_MESSAGE_MAX_BYTES
        );

        let examples: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../contracts/fixtures/admin-line-protocol-examples.json"
        ))
        .unwrap();
        for value in examples["valid"].as_array().unwrap() {
            let accepted = if value.get("scope").is_some() {
                serde_json::from_value::<AdminEnvelope>(value.clone()).is_ok()
            } else {
                serde_json::from_value::<AdminResponse>(value.clone()).is_ok()
            };
            assert!(accepted, "valid schema example differs from Rust: {value}");
        }
        for value in examples["invalid"].as_array().unwrap() {
            assert!(
                serde_json::from_value::<AdminEnvelope>(value.clone()).is_err()
                    && serde_json::from_value::<AdminResponse>(value.clone()).is_err(),
                "invalid schema example was accepted by Rust: {value}"
            );
        }
    }

    #[test]
    fn wp12_negative_zero_state() {
        let root = tempfile::tempdir().unwrap();
        let path = write_config(root.path(), "\n[workspace]\nroots = [\"/secret\"]\n");
        assert!(matches!(
            DaemonConfig::load(&path),
            Err(DaemonError::Config(_))
        ));

        let config = config(root.path());
        private_directory(&config.static_config.state_root).unwrap();
        private_directory(&config.static_config.runtime_root).unwrap();
        private_directory(&config.static_config.config_root).unwrap();
        fs::set_permissions(
            &config.static_config.state_root,
            fs::Permissions::from_mode(0o777),
        )
        .unwrap();
        assert!(matches!(
            DaemonLease::acquire(&config),
            Err(DaemonError::Config(_))
        ));

        let discovery = discovery(&config, released_wire_compatibility_versions()).unwrap();
        let value = serde_json::to_value(&discovery).unwrap();
        let keys = value
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            keys,
            BTreeSet::from([
                "basic_readiness".to_owned(),
                "daemon_instance_id".to_owned(),
                "pid".to_owned(),
                "process_start_token".to_owned(),
                "public_bundle_versions".to_owned(),
                "query_socket_endpoint".to_owned(),
                "rpc_maximum_minor".to_owned(),
                "rpc_minimum_minor".to_owned(),
                "socket_endpoint".to_owned(),
                "startup_time_unix_ms".to_owned(),
            ])
        );
        let encoded = serde_json::to_string(&discovery).unwrap();
        for forbidden in [
            "credential",
            "auth_token",
            "access_token",
            "workspace_root",
            "source_path",
            "secret",
        ] {
            assert!(!encoded.contains(forbidden));
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wp12_behavioral_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let discovery_path = config.static_config.runtime_root.join("daemon.json");
        let task = tokio::spawn(serve_query_transport(
            config.clone(),
            Arc::new(TestQueryBackend),
            Vec::new(),
            None,
            None,
            None,
            released_wire_compatibility_versions(),
        ));
        wait_for_discovery(&discovery_path, Duration::from_secs(5))
            .await
            .unwrap();

        assert!(matches!(
            DaemonLease::acquire(&config),
            Err(DaemonError::LeaseHeld)
        ));
        let status = administer(&discovery_path, AdminCommand::Status)
            .await
            .unwrap();
        assert_eq!(status.daemon_liveness, "LIVE");
        assert_eq!(status.workspace_readiness, "NO_WORKSPACES_READY");
        assert!(status.accepted);
        assert!(status.shutdown_mode.is_none());
        let legacy_mutation = administer_workspace(
            &discovery_path,
            WorkspaceAdminCommand::Add {
                root: root.path().join("legacy-workspace"),
            },
        )
        .await
        .unwrap();
        assert!(!legacy_mutation.accepted);
        assert_eq!(
            legacy_mutation.error_code.as_deref(),
            Some("PROGRAMMATIC_COMMAND_INGRESS_REQUIRED")
        );

        let drained = administer(&discovery_path, AdminCommand::Drain)
            .await
            .unwrap();
        assert_eq!(drained.shutdown_mode.as_deref(), Some("drain"));
        let exit = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(exit.drained);
        assert!(!discovery_path.exists());
        assert!(!config.static_config.socket_endpoint.exists());
        assert!(!config.static_config.query_socket_endpoint.exists());

        let lease = DaemonLease::acquire(&config).unwrap();
        drop(lease);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wp12_operational_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let discovery_path = config.static_config.runtime_root.join("daemon.json");
        let task = tokio::spawn(serve_query_transport(
            config,
            Arc::new(TestQueryBackend),
            Vec::new(),
            None,
            None,
            None,
            released_wire_compatibility_versions(),
        ));
        wait_for_discovery(&discovery_path, Duration::from_secs(5))
            .await
            .unwrap();
        administer(&discovery_path, AdminCommand::Stop)
            .await
            .unwrap();
        let exit = task.await.unwrap().unwrap();
        assert!(!exit.drained);
        assert_eq!(
            exit.shutdown_steps,
            [
                "mark-stopping",
                "close-ingress",
                "await-workers",
                "close-durable-stores",
                "retire-endpoint-metadata",
                "release-singleton-lease",
            ]
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wp41_prod_neg_cutover_status_admin_path_fails_closed_without_deployment() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let discovery_path = config.static_config.runtime_root.join("daemon.json");
        let task = tokio::spawn(serve_query_transport(
            config,
            Arc::new(TestQueryBackend),
            Vec::new(),
            None,
            None,
            None,
            released_wire_compatibility_versions(),
        ));
        wait_for_discovery(&discovery_path, Duration::from_secs(5))
            .await
            .unwrap();
        let status = administer(&discovery_path, AdminCommand::CutoverStatus)
            .await
            .unwrap();
        assert!(!status.accepted);
        assert_eq!(
            status.error_code.as_deref(),
            Some("CUTOVER_DEPLOYMENT_NOT_CONFIGURED")
        );
        assert!(status.cutover_status.is_none());
        administer(&discovery_path, AdminCommand::Stop)
            .await
            .unwrap();
        task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn production_serve_requires_programmatic_composition() {
        let root = tempfile::tempdir().unwrap();
        assert!(matches!(
            serve(config(root.path())).await,
            Err(DaemonError::ProgrammaticCompositionRequired)
        ));
    }
}
