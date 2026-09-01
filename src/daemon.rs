//! Local daemon lifecycle, closed configuration, singleton lease, and admin IPC.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Write as _};
use std::os::unix::fs::{
    FileTypeExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::{UnixListener, UnixStream};

use crate::contracts::deployment_profile::DeploymentProfileDocument;
use crate::fabric::command::LeaseId;
use crate::fabric::production_kernel::{
    CompiledSemanticRelease, LifecycleAuthority, OperationalWorkspaceRegistry,
    OperationalWorkspaceRegistryError, ProductionLifecycleError, ProductionLifecyclePhase,
    WorkspaceSlotRegistry, WorkspaceSlotRegistryError,
};
use crate::fabric::writer_generation_sqlite::{
    SqliteWriterGenerationCloseError, SqliteWriterGenerationOpenError, SqliteWriterGenerationStore,
};
use crate::fabric::writer_lease::{WorkspaceWriterLease, WorkspaceWriterLeaseError};
use crate::operational_store::OperationalStore;
use crate::operational_store::OperationalStoreError;
use crate::rpc::SameUserInterceptor;
use crate::workspace_registry::{RelinkProof, RemovalPolicy, WorkspaceRecord};
use crate::workspace_registry::{WorkspaceRegistry, WorkspaceRegistryError};

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
    #[error("daemon singleton lease is already held")]
    LeaseHeld,
    #[error("administrative protocol failure: {0}")]
    Admin(String),
    #[error("joined shutdown failed after {completed_steps:?}: {detail}")]
    Shutdown {
        completed_steps: Vec<&'static str>,
        detail: String,
    },
    #[error(
        "{primary}; joined startup cleanup also failed after {completed_steps:?}: {cleanup_failures:?}"
    )]
    StartupCleanup {
        primary: Box<DaemonError>,
        completed_steps: Vec<&'static str>,
        cleanup_failures: Vec<String>,
    },
    #[error(transparent)]
    ProductionLifecycle(#[from] ProductionLifecycleError),
    #[error(transparent)]
    OperationalStore(#[from] OperationalStoreError),
    #[error(transparent)]
    WorkspaceRegistry(#[from] WorkspaceRegistryError),
    #[error(transparent)]
    OperationalWorkspaceRegistry(#[from] OperationalWorkspaceRegistryError),
    #[error(transparent)]
    WorkspaceSlotRegistry(#[from] WorkspaceSlotRegistryError),
    #[error(transparent)]
    WriterGeneration(#[from] SqliteWriterGenerationOpenError),
    #[error(transparent)]
    WriterGenerationClose(#[from] SqliteWriterGenerationCloseError),
    #[error(transparent)]
    WorkspaceWriterLease(#[from] WorkspaceWriterLeaseError),
    #[error("production startup has no explicit operational workspace")]
    NoOperationalWorkspace,
    #[error("owned socket identity changed before retirement: {0}")]
    OwnedSocketIdentityChanged(PathBuf),
    #[error(transparent)]
    Identity(#[from] crate::identity::IdentityError),
}

#[cfg(test)]
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

fn record_joined_cleanup<E: std::fmt::Display>(
    completed_steps: &mut Vec<&'static str>,
    cleanup_failures: &mut Vec<String>,
    step: &'static str,
    outcome: Result<(), E>,
) {
    match outcome {
        Ok(()) => {
            completed_steps.push(step);
            tracing::info!(shutdown_step = step, "joined daemon cleanup step completed");
        }
        Err(error) => cleanup_failures.push(format!("{step}: {error}")),
    }
}

fn joined_startup_error(
    primary: DaemonError,
    completed_steps: Vec<&'static str>,
    cleanup_failures: Vec<String>,
) -> DaemonError {
    if cleanup_failures.is_empty() {
        primary
    } else {
        DaemonError::StartupCleanup {
            primary: Box::new(primary),
            completed_steps,
            cleanup_failures,
        }
    }
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
    released: bool,
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
            released: false,
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

    /// Explicitly retire discovery authority and release the singleton lock.
    ///
    /// # Errors
    ///
    /// Returns exact endpoint-metadata, directory-sync, or unlock failures. Drop remains only a
    /// partial-construction safety net and is not successful joined-shutdown evidence.
    pub fn release(mut self) -> Result<(), DaemonError> {
        match fs::remove_file(&self.discovery_path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(DaemonError::Io {
                    path: self.discovery_path.clone(),
                    source,
                });
            }
        }
        sync_directory(&self.runtime_root)?;
        self.lock.unlock().map_err(|source| DaemonError::Io {
            path: self.runtime_root.join("daemon.lock"),
            source,
        })?;
        self.released = true;
        Ok(())
    }
}

impl Drop for DaemonLease {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        let _ = fs::remove_file(&self.discovery_path);
        let _ = sync_directory(&self.runtime_root);
        let _ = self.lock.unlock();
    }
}

/// One bound Unix socket which may unlink only the exact inode it created.
struct OwnedUnixSocket {
    path: PathBuf,
    device: u64,
    inode: u64,
    owner_uid: u32,
    retired: bool,
}

impl OwnedUnixSocket {
    fn bind(path: &Path) -> Result<(UnixListener, Self), DaemonError> {
        match fs::symlink_metadata(path) {
            Ok(_) => {
                return Err(DaemonError::Config(format!(
                    "refusing to replace existing socket path {}",
                    path.display()
                )));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(DaemonError::Io {
                    path: path.to_owned(),
                    source,
                });
            }
        }
        let listener = UnixListener::bind(path).map_err(|source| DaemonError::Io {
            path: path.to_owned(),
            source,
        })?;
        let initial = fs::symlink_metadata(path).map_err(|source| DaemonError::Io {
            path: path.to_owned(),
            source,
        })?;
        let mut owned = Self {
            path: path.to_owned(),
            device: initial.dev(),
            inode: initial.ino(),
            owner_uid: initial.uid(),
            retired: false,
        };
        if !initial.file_type().is_socket() {
            return Err(DaemonError::Config(format!(
                "bound endpoint is not a Unix socket: {}",
                path.display()
            )));
        }
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|source| {
            DaemonError::Io {
                path: path.to_owned(),
                source,
            }
        })?;
        let final_metadata = fs::symlink_metadata(path).map_err(|source| DaemonError::Io {
            path: path.to_owned(),
            source,
        })?;
        if !owned.matches(&final_metadata)
            || !final_metadata.file_type().is_socket()
            || final_metadata.mode() & 0o777 != 0o600
        {
            return Err(DaemonError::OwnedSocketIdentityChanged(path.to_owned()));
        }
        owned.retired = false;
        Ok((listener, owned))
    }

    fn matches(&self, metadata: &fs::Metadata) -> bool {
        metadata.dev() == self.device
            && metadata.ino() == self.inode
            && metadata.uid() == self.owner_uid
    }

    fn retire(&mut self) -> Result<(), DaemonError> {
        if self.retired {
            return Ok(());
        }
        let metadata = fs::symlink_metadata(&self.path).map_err(|source| DaemonError::Io {
            path: self.path.clone(),
            source,
        })?;
        if !metadata.file_type().is_socket() || !self.matches(&metadata) {
            return Err(DaemonError::OwnedSocketIdentityChanged(self.path.clone()));
        }
        fs::remove_file(&self.path).map_err(|source| DaemonError::Io {
            path: self.path.clone(),
            source,
        })?;
        self.retired = true;
        sync_directory(
            self.path
                .parent()
                .ok_or_else(|| DaemonError::Config("socket has no parent".to_owned()))?,
        )
    }
}

impl Drop for OwnedUnixSocket {
    fn drop(&mut self) {
        if self.retired {
            return;
        }
        let Ok(metadata) = fs::symlink_metadata(&self.path) else {
            return;
        };
        if metadata.file_type().is_socket() && self.matches(&metadata) {
            let _ = fs::remove_file(&self.path);
            if let Some(parent) = self.path.parent() {
                let _ = sync_directory(parent);
            }
        }
    }
}

/// Marker for a validated, side-effect-free production startup configuration.
pub struct Configured;

/// Phase aggregate which owns the daemon singleton lease.
pub struct DaemonLeased {
    lease: DaemonLease,
}

/// Phase aggregate which owns every operational writer fence and its durable generation store.
pub struct WriterFenced {
    daemon_lease: DaemonLease,
    registry: OperationalWorkspaceRegistry,
    generation_store: SqliteWriterGenerationStore,
    writer_leases: Vec<WorkspaceWriterLease>,
}

/// Phase-typed startup owner. Semantic phase values cannot be supplied by configuration.
pub struct ProductionStartupCoordinator<Phase> {
    config: DaemonConfig,
    release: CompiledSemanticRelease,
    lifecycle: Arc<LifecycleAuthority>,
    workspace_slots: Arc<WorkspaceSlotRegistry>,
    phase: Phase,
}

fn cleanup_owned_startup_failure(
    lifecycle: &LifecycleAuthority,
    workspace_slots: &WorkspaceSlotRegistry,
    daemon_lease: DaemonLease,
    registry: Option<OperationalWorkspaceRegistry>,
    generation_store: Option<SqliteWriterGenerationStore>,
    mut writer_leases: Vec<WorkspaceWriterLease>,
    primary: DaemonError,
) -> DaemonError {
    let mut completed_steps = Vec::new();
    let mut cleanup_failures = Vec::new();
    record_joined_cleanup(
        &mut completed_steps,
        &mut cleanup_failures,
        "mark-failed-closed",
        lifecycle.fail_closed("PRODUCTION_STARTUP_FAILED").map(drop),
    );
    record_joined_cleanup(
        &mut completed_steps,
        &mut cleanup_failures,
        "mark-draining",
        lifecycle.begin_draining().map(drop),
    );
    if let Some(registry) = registry {
        drop(registry);
        completed_steps.push("close-operational-registry");
    }
    record_joined_cleanup(
        &mut completed_steps,
        &mut cleanup_failures,
        "close-workspace-slots",
        workspace_slots.shutdown().map(drop),
    );
    if let Some(store) = generation_store {
        record_joined_cleanup(
            &mut completed_steps,
            &mut cleanup_failures,
            "close-writer-generation-store",
            store.close(),
        );
    }
    let mut writer_release = Ok(());
    for lease in writer_leases.drain(..) {
        if let Err(error) = lease.release() {
            writer_release = Err(error);
        }
    }
    record_joined_cleanup(
        &mut completed_steps,
        &mut cleanup_failures,
        "release-writer-leases",
        writer_release,
    );
    record_joined_cleanup(
        &mut completed_steps,
        &mut cleanup_failures,
        "release-daemon-lease",
        daemon_lease.release(),
    );
    record_joined_cleanup(
        &mut completed_steps,
        &mut cleanup_failures,
        "mark-stopped",
        lifecycle.finish_stopped().map(drop),
    );
    joined_startup_error(primary, completed_steps, cleanup_failures)
}

impl ProductionStartupCoordinator<Configured> {
    fn new(config: DaemonConfig, release: CompiledSemanticRelease) -> Self {
        Self {
            config,
            release,
            lifecycle: Arc::new(LifecycleAuthority::new()),
            workspace_slots: Arc::new(WorkspaceSlotRegistry::new()),
            phase: Configured,
        }
    }

    fn acquire_daemon_lease(
        self,
    ) -> Result<ProductionStartupCoordinator<DaemonLeased>, DaemonError> {
        let lease = DaemonLease::acquire(&self.config)?;
        if let Err(error) = self.lifecycle.advance(
            ProductionLifecyclePhase::Configured,
            ProductionLifecyclePhase::DaemonLeased,
        ) {
            return Err(cleanup_owned_startup_failure(
                &self.lifecycle,
                &self.workspace_slots,
                lease,
                None,
                None,
                Vec::new(),
                error.into(),
            ));
        }
        Ok(ProductionStartupCoordinator {
            config: self.config,
            release: self.release,
            lifecycle: self.lifecycle,
            workspace_slots: self.workspace_slots,
            phase: DaemonLeased { lease },
        })
    }
}

impl ProductionStartupCoordinator<DaemonLeased> {
    fn acquire_workspace_writers(
        self,
    ) -> Result<ProductionStartupCoordinator<WriterFenced>, DaemonError> {
        let ProductionStartupCoordinator {
            config,
            release,
            lifecycle,
            workspace_slots,
            phase: DaemonLeased {
                lease: daemon_lease,
            },
        } = self;
        let operational_path = config
            .static_config
            .state_root
            .join(&config.static_config.operational_database);
        let mut store = match OperationalStore::open(&operational_path) {
            Ok(store) => store,
            Err(error) => {
                return Err(cleanup_owned_startup_failure(
                    &lifecycle,
                    &workspace_slots,
                    daemon_lease,
                    None,
                    None,
                    Vec::new(),
                    error.into(),
                ));
            }
        };
        let records = WorkspaceRegistry::new(&mut store).list();
        drop(store);
        let records = match records {
            Ok(records) => records,
            Err(error) => {
                return Err(cleanup_owned_startup_failure(
                    &lifecycle,
                    &workspace_slots,
                    daemon_lease,
                    None,
                    None,
                    Vec::new(),
                    error.into(),
                ));
            }
        };
        let registry = match OperationalWorkspaceRegistry::try_from_records(records) {
            Ok(registry) => registry,
            Err(error) => {
                return Err(cleanup_owned_startup_failure(
                    &lifecycle,
                    &workspace_slots,
                    daemon_lease,
                    None,
                    None,
                    Vec::new(),
                    error.into(),
                ));
            }
        };
        if registry.is_empty() {
            return Err(cleanup_owned_startup_failure(
                &lifecycle,
                &workspace_slots,
                daemon_lease,
                Some(registry),
                None,
                Vec::new(),
                DaemonError::NoOperationalWorkspace,
            ));
        }
        if let Err(error) = workspace_slots.close_from_operational_registry(&registry) {
            return Err(cleanup_owned_startup_failure(
                &lifecycle,
                &workspace_slots,
                daemon_lease,
                Some(registry),
                None,
                Vec::new(),
                error.into(),
            ));
        }

        // Validate the explicit credential input before creating any public endpoint. WP34
        // replaces this transitional file with policy-bound launch-grant registration.
        if let Err(error) = query_capability_token(&config.static_config) {
            return Err(cleanup_owned_startup_failure(
                &lifecycle,
                &workspace_slots,
                daemon_lease,
                Some(registry),
                None,
                Vec::new(),
                error,
            ));
        }

        let writer_root = config.static_config.state_root.join("writer-authority");
        if let Err(error) = private_directory(&writer_root) {
            return Err(cleanup_owned_startup_failure(
                &lifecycle,
                &workspace_slots,
                daemon_lease,
                Some(registry),
                None,
                Vec::new(),
                error,
            ));
        }
        let generation_store = SqliteWriterGenerationStore::open(
            &config
                .static_config
                .state_root
                .join("writer-generations.sqlite"),
        );
        let generation_store = match generation_store {
            Ok(store) => store,
            Err(error) => {
                return Err(cleanup_owned_startup_failure(
                    &lifecycle,
                    &workspace_slots,
                    daemon_lease,
                    Some(registry),
                    None,
                    Vec::new(),
                    error.into(),
                ));
            }
        };
        let mut writer_leases = Vec::with_capacity(registry.records().len());
        for record in registry.records() {
            let lease_id = match crate::identity::random_registration_nonce() {
                Ok(nonce) => LeaseId::from_bytes(nonce),
                Err(error) => {
                    return Err(cleanup_owned_startup_failure(
                        &lifecycle,
                        &workspace_slots,
                        daemon_lease,
                        Some(registry),
                        Some(generation_store),
                        writer_leases,
                        error.into(),
                    ));
                }
            };
            let lease = WorkspaceWriterLease::acquire(
                &writer_root,
                crate::fabric::command::WorkspaceId::from_bytes(record.workspace_id),
                lease_id,
                &generation_store,
            );
            match lease {
                Ok(lease) => writer_leases.push(lease),
                Err(error) => {
                    return Err(cleanup_owned_startup_failure(
                        &lifecycle,
                        &workspace_slots,
                        daemon_lease,
                        Some(registry),
                        Some(generation_store),
                        writer_leases,
                        error.into(),
                    ));
                }
            }
        }
        if let Err(error) = lifecycle.advance(
            ProductionLifecyclePhase::DaemonLeased,
            ProductionLifecyclePhase::WriterFenced,
        ) {
            return Err(cleanup_owned_startup_failure(
                &lifecycle,
                &workspace_slots,
                daemon_lease,
                Some(registry),
                Some(generation_store),
                writer_leases,
                error.into(),
            ));
        }
        Ok(ProductionStartupCoordinator {
            config,
            release,
            lifecycle,
            workspace_slots,
            phase: WriterFenced {
                daemon_lease,
                registry,
                generation_store,
                writer_leases,
            },
        })
    }
}

/// Sole factory used by the production `codefabricd` entrypoint.
#[derive(Clone, Copy, Debug)]
pub struct ProductionDaemonFactory {
    release: CompiledSemanticRelease,
}

impl ProductionDaemonFactory {
    #[must_use]
    pub const fn current() -> Self {
        Self {
            release: CompiledSemanticRelease::current(),
        }
    }

    /// Validate operational configuration and construct one side-effect-free kernel.
    ///
    /// # Errors
    ///
    /// Returns closed configuration errors before acquiring leases, opening stores, or binding
    /// endpoints.
    pub fn build(self, config: DaemonConfig) -> Result<DaemonKernel, DaemonError> {
        config.validate()?;
        Ok(DaemonKernel {
            startup: ProductionStartupCoordinator::new(config, self.release),
        })
    }
}

/// Joined production daemon owner. It never accepts a test/default semantic backend.
pub struct DaemonKernel {
    startup: ProductionStartupCoordinator<Configured>,
}

impl fmt::Debug for DaemonKernel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DaemonKernel")
            .field("suite", &self.startup.release.suite().display())
            .field("lifecycle", &self.startup.lifecycle.observe())
            .finish_non_exhaustive()
    }
}

impl DaemonKernel {
    #[must_use]
    pub fn lifecycle(&self) -> Arc<LifecycleAuthority> {
        Arc::clone(&self.startup.lifecycle)
    }

    /// Return the kernel-owned atomic workspace slots. The registry is empty until the
    /// operational census is closed during startup; each slot remains semantically uninstalled
    /// until WP32's exact reconstruction path succeeds.
    #[must_use]
    pub fn workspace_slots(&self) -> Arc<WorkspaceSlotRegistry> {
        Arc::clone(&self.startup.workspace_slots)
    }

    #[must_use]
    pub const fn release(&self) -> CompiledSemanticRelease {
        self.startup.release
    }

    /// Run the currently realizable production startup phases and an admin-only bootstrapping
    /// service. WP32 supplies command recovery/genesis and WP36 supplies the v2 query service; no
    /// semantic endpoint or false Ready projection is exposed in their absence.
    ///
    /// # Errors
    ///
    /// Returns the exact configuration, lease, registry, writer, endpoint, or admin failure after
    /// joined best-effort cleanup. The lifecycle ends `Stopped` on every owned failure path.
    pub async fn run(self) -> Result<DaemonExit, DaemonError> {
        let lifecycle = Arc::clone(&self.startup.lifecycle);
        let result = async {
            let leased = self.startup.acquire_daemon_lease()?;
            let fenced = leased.acquire_workspace_writers()?;
            serve_writer_fenced_bootstrap(fenced).await
        }
        .await;
        if result.is_err() && lifecycle.observe().phase() != ProductionLifecyclePhase::Stopped {
            if lifecycle.observe().phase() != ProductionLifecyclePhase::FailedClosed {
                let _ = lifecycle.fail_closed("PRODUCTION_STARTUP_FAILED");
            }
            let _ = lifecycle.begin_draining();
            let _ = lifecycle.finish_stopped();
        }
        result
    }
}

fn finish_writer_fenced_bootstrap(
    startup: ProductionStartupCoordinator<WriterFenced>,
    listener: Option<UnixListener>,
    mut admin_socket: Option<OwnedUnixSocket>,
    drained: bool,
    primary: Option<DaemonError>,
) -> Result<DaemonExit, DaemonError> {
    let ProductionStartupCoordinator {
        lifecycle,
        workspace_slots,
        phase:
            WriterFenced {
                daemon_lease,
                registry,
                generation_store,
                mut writer_leases,
            },
        ..
    } = startup;
    let mut steps = Vec::new();
    let mut cleanup_failures = Vec::new();
    if primary.is_some() {
        record_joined_cleanup(
            &mut steps,
            &mut cleanup_failures,
            "mark-failed-closed",
            lifecycle
                .fail_closed("PRODUCTION_BOOTSTRAP_SERVICE_FAILED")
                .map(drop),
        );
    }
    record_joined_cleanup(
        &mut steps,
        &mut cleanup_failures,
        "mark-draining",
        lifecycle.begin_draining().map(drop),
    );
    if let Some(listener) = listener {
        drop(listener);
        steps.push("close-admin-ingress");
    }
    if let Some(socket) = admin_socket.as_mut() {
        record_joined_cleanup(
            &mut steps,
            &mut cleanup_failures,
            "retire-admin-endpoint",
            socket.retire(),
        );
    }
    drop(admin_socket);
    drop(registry);
    steps.push("close-operational-registry");
    record_joined_cleanup(
        &mut steps,
        &mut cleanup_failures,
        "close-workspace-slots",
        workspace_slots.shutdown().map(drop),
    );
    record_joined_cleanup(
        &mut steps,
        &mut cleanup_failures,
        "close-writer-generation-store",
        generation_store.close(),
    );
    let mut writer_release = Ok(());
    for lease in writer_leases.drain(..) {
        if let Err(error) = lease.release() {
            writer_release = Err(error);
        }
    }
    record_joined_cleanup(
        &mut steps,
        &mut cleanup_failures,
        "release-writer-leases",
        writer_release,
    );
    record_joined_cleanup(
        &mut steps,
        &mut cleanup_failures,
        "release-daemon-lease",
        daemon_lease.release(),
    );
    record_joined_cleanup(
        &mut steps,
        &mut cleanup_failures,
        "mark-stopped",
        lifecycle.finish_stopped().map(drop),
    );

    if let Some(primary) = primary {
        return Err(joined_startup_error(primary, steps, cleanup_failures));
    }
    if !cleanup_failures.is_empty() {
        return Err(DaemonError::Shutdown {
            completed_steps: steps,
            detail: cleanup_failures.join("; "),
        });
    }
    Ok(DaemonExit {
        drained,
        shutdown_steps: steps,
    })
}

async fn serve_writer_fenced_bootstrap(
    startup: ProductionStartupCoordinator<WriterFenced>,
) -> Result<DaemonExit, DaemonError> {
    if startup.config.static_config.query_socket_endpoint.exists() {
        let error = DaemonError::Config(format!(
            "query endpoint is occupied before v2 service construction: {}",
            startup.config.static_config.query_socket_endpoint.display()
        ));
        return finish_writer_fenced_bootstrap(startup, None, None, false, Some(error));
    }
    let admin_service = match BootstrapAdminService::try_new(
        Arc::clone(&startup.lifecycle),
        &startup.phase.registry,
        &startup.workspace_slots,
    ) {
        Ok(service) => service,
        Err(error) => {
            return finish_writer_fenced_bootstrap(startup, None, None, false, Some(error));
        }
    };
    let public_versions = BTreeMap::from([(
        "codefabric.authoritative-suite".to_owned(),
        startup.release.suite().display(),
    )]);
    let published = match discovery(
        &startup.config,
        public_versions,
        startup.lifecycle.observe().semantic_admission_open(),
    ) {
        Ok(published) => published,
        Err(error) => {
            drop(admin_service);
            return finish_writer_fenced_bootstrap(startup, None, None, false, Some(error));
        }
    };
    let (listener, admin_socket) =
        match OwnedUnixSocket::bind(&startup.config.static_config.socket_endpoint) {
            Ok(owners) => owners,
            Err(error) => {
                drop(admin_service);
                return finish_writer_fenced_bootstrap(startup, None, None, false, Some(error));
            }
        };
    if let Err(error) = startup.lifecycle.advance(
        ProductionLifecyclePhase::WriterFenced,
        ProductionLifecyclePhase::EndpointsBoundBootstrapping,
    ) {
        drop(admin_service);
        return finish_writer_fenced_bootstrap(
            startup,
            Some(listener),
            Some(admin_socket),
            false,
            Some(error.into()),
        );
    }
    if let Err(error) = startup.phase.daemon_lease.publish(&published) {
        drop(admin_service);
        return finish_writer_fenced_bootstrap(
            startup,
            Some(listener),
            Some(admin_socket),
            false,
            Some(error),
        );
    }
    let allowed_uid = match fs::symlink_metadata(&startup.config.static_config.socket_endpoint)
        .map_err(|source| DaemonError::Io {
            path: startup.config.static_config.socket_endpoint.clone(),
            source,
        }) {
        Ok(metadata) => metadata.uid(),
        Err(error) => {
            drop(admin_service);
            return finish_writer_fenced_bootstrap(
                startup,
                Some(listener),
                Some(admin_socket),
                false,
                Some(error),
            );
        }
    };
    tracing::info!(
        lifecycle = ProductionLifecyclePhase::EndpointsBoundBootstrapping.code(),
        "production daemon bootstrapping admin ingress opened"
    );

    let serve_result = loop {
        let (mut stream, _) = match listener.accept().await {
            Ok(stream) => stream,
            Err(source) => break Err(DaemonError::Admin(format!("accept: {source}"))),
        };
        if SameUserInterceptor::new(allowed_uid)
            .authenticate_stream(&stream)
            .is_err()
        {
            continue;
        }
        let request = match read_request(&mut stream).await {
            Ok(request) => request,
            Err(error) => break Err(error),
        };
        let (response, stop) = admin_service.handle(request);
        if let Err(error) = write_response(&mut stream, &response).await {
            break Err(error);
        }
        if let Some(drained) = stop {
            break Ok(drained);
        }
    };
    drop(admin_service);
    match serve_result {
        Ok(drained) => finish_writer_fenced_bootstrap(
            startup,
            Some(listener),
            Some(admin_socket),
            drained,
            None,
        ),
        Err(error) => finish_writer_fenced_bootstrap(
            startup,
            Some(listener),
            Some(admin_socket),
            false,
            Some(error),
        ),
    }
}

/// Fully constructed admin-only bootstrapping service. Binding happens only after this value
/// validates the kernel's operational registry, lifecycle, and atomic slot census.
struct BootstrapAdminService<'a> {
    lifecycle: Arc<LifecycleAuthority>,
    registry: &'a OperationalWorkspaceRegistry,
}

impl<'a> BootstrapAdminService<'a> {
    fn try_new(
        lifecycle: Arc<LifecycleAuthority>,
        registry: &'a OperationalWorkspaceRegistry,
        workspace_slots: &WorkspaceSlotRegistry,
    ) -> Result<Self, DaemonError> {
        if !workspace_slots.is_closed() || workspace_slots.len() != registry.records().len() {
            return Err(DaemonError::Config(
                "production workspace-slot census is not closed over the operational registry"
                    .to_owned(),
            ));
        }
        Ok(Self {
            lifecycle,
            registry,
        })
    }

    fn handle(&self, request: AdminEnvelope) -> (AdminResponse, Option<bool>) {
        let projection = self.lifecycle.observe();
        let readiness = projection.phase().code().to_owned();
        match request {
            AdminEnvelope::Daemon(request) => {
                let stop = match request.command {
                    AdminCommand::Stop => Some(false),
                    AdminCommand::Drain => Some(true),
                    AdminCommand::Status | AdminCommand::CutoverStatus => None,
                };
                let accepted = request.command != AdminCommand::CutoverStatus;
                (
                    AdminResponse {
                        accepted,
                        daemon_liveness: "LIVE".to_owned(),
                        workspace_readiness: readiness,
                        shutdown_mode: stop
                            .map(|drain| if drain { "drain" } else { "stop" }.to_owned()),
                        workspaces: self.registry.records().to_vec(),
                        workspace_health: Vec::new(),
                        cutover_status: None,
                        error_code: Some(
                            if accepted {
                                projection
                                    .failure_code()
                                    .unwrap_or("SEMANTIC_AUTHORITY_BOOTSTRAPPING")
                            } else {
                                "CUTOVER_AUTHORITY_UNAVAILABLE"
                            }
                            .to_owned(),
                        ),
                    },
                    stop,
                )
            }
            AdminEnvelope::Workspace(_) => (
                AdminResponse {
                    accepted: false,
                    daemon_liveness: "LIVE".to_owned(),
                    workspace_readiness: readiness,
                    shutdown_mode: None,
                    workspaces: self.registry.records().to_vec(),
                    workspace_health: Vec::new(),
                    cutover_status: None,
                    error_code: Some("COMMAND_RECOVERY_REQUIRED".to_owned()),
                },
                None,
            ),
        }
    }
}

fn now_millis() -> Result<u128, DaemonError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|error| DaemonError::Admin(format!("system clock before Unix epoch: {error}")))
}

fn discovery(
    config: &DaemonConfig,
    public_bundle_versions: BTreeMap<String, String>,
    basic_readiness: bool,
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
        basic_readiness,
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
    ProductionDaemonFactory::current()
        .build(config)?
        .run()
        .await
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

    fn register_operational_workspace(config: &DaemonConfig, root: &Path) -> WorkspaceRecord {
        add_operational_workspace(config, &root.join("workspace"))
    }

    fn add_operational_workspace(config: &DaemonConfig, workspace_root: &Path) -> WorkspaceRecord {
        private_directory(&config.static_config.state_root).unwrap();
        private_directory(workspace_root).unwrap();
        let mut store = OperationalStore::open(
            &config
                .static_config
                .state_root
                .join(&config.static_config.operational_database),
        )
        .unwrap();
        WorkspaceRegistry::new(&mut store)
            .add(
                workspace_root,
                crate::workspace_registry::WorkspaceSourceRegistration::Directory,
            )
            .unwrap()
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

        let discovery = discovery(&config, released_wire_compatibility_versions(), false).unwrap();
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

    #[tokio::test]
    async fn production_factory_rejects_empty_workspace_without_endpoint_or_lease_leaks() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let kernel = ProductionDaemonFactory::current()
            .build(config.clone())
            .unwrap();
        let lifecycle = kernel.lifecycle();
        assert!(matches!(
            kernel.run().await,
            Err(DaemonError::NoOperationalWorkspace)
        ));
        assert_eq!(
            lifecycle.observe().phase(),
            ProductionLifecyclePhase::Stopped
        );
        assert!(!config.static_config.socket_endpoint.exists());
        assert!(!config.static_config.query_socket_endpoint.exists());
        assert!(
            !config
                .static_config
                .runtime_root
                .join("daemon.json")
                .exists()
        );

        let second = DaemonLease::acquire(&config).unwrap();
        drop(second);
    }

    #[tokio::test]
    async fn production_startup_faults_before_endpoint_exposure_and_releases_owners() {
        let missing_token_root = tempfile::tempdir().unwrap();
        let missing_token_config = config(missing_token_root.path());
        register_operational_workspace(&missing_token_config, missing_token_root.path());
        fs::remove_file(
            missing_token_config.static_config.config_root.join(
                &missing_token_config
                    .static_config
                    .query_capability_token_file,
            ),
        )
        .unwrap();
        let missing_token_kernel = ProductionDaemonFactory::current()
            .build(missing_token_config.clone())
            .unwrap();
        let missing_token_lifecycle = missing_token_kernel.lifecycle();
        assert!(missing_token_kernel.run().await.is_err());
        assert_eq!(
            missing_token_lifecycle.observe().phase(),
            ProductionLifecyclePhase::Stopped
        );
        assert!(!missing_token_config.static_config.socket_endpoint.exists());
        assert!(
            !missing_token_config
                .static_config
                .runtime_root
                .join("daemon.json")
                .exists()
        );
        DaemonLease::acquire(&missing_token_config)
            .unwrap()
            .release()
            .unwrap();

        let occupied_query_root = tempfile::tempdir().unwrap();
        let occupied_query_config = config(occupied_query_root.path());
        register_operational_workspace(&occupied_query_config, occupied_query_root.path());
        private_directory(&occupied_query_config.static_config.runtime_root).unwrap();
        private_file(
            &occupied_query_config.static_config.query_socket_endpoint,
            b"foreign-query-owner",
        )
        .unwrap();
        let occupied_query_kernel = ProductionDaemonFactory::current()
            .build(occupied_query_config.clone())
            .unwrap();
        let occupied_query_lifecycle = occupied_query_kernel.lifecycle();
        assert!(matches!(
            occupied_query_kernel.run().await,
            Err(DaemonError::Config(message)) if message.contains("query endpoint is occupied")
        ));
        assert_eq!(
            occupied_query_lifecycle.observe().phase(),
            ProductionLifecyclePhase::Stopped
        );
        assert!(!occupied_query_config.static_config.socket_endpoint.exists());
        assert_eq!(
            fs::read(&occupied_query_config.static_config.query_socket_endpoint).unwrap(),
            b"foreign-query-owner"
        );
        DaemonLease::acquire(&occupied_query_config)
            .unwrap()
            .release()
            .unwrap();

        let writer_contention_root = tempfile::tempdir().unwrap();
        let writer_contention_config = config(writer_contention_root.path());
        let record = register_operational_workspace(
            &writer_contention_config,
            writer_contention_root.path(),
        );
        let writer_root = writer_contention_config
            .static_config
            .state_root
            .join("writer-authority");
        private_directory(&writer_root).unwrap();
        let generation_store = SqliteWriterGenerationStore::open(
            &writer_contention_config
                .static_config
                .state_root
                .join("writer-generations.sqlite"),
        )
        .unwrap();
        let held_writer = WorkspaceWriterLease::acquire(
            &writer_root,
            crate::fabric::command::WorkspaceId::from_bytes(record.workspace_id),
            LeaseId::from_bytes([9; 16]),
            &generation_store,
        )
        .unwrap();
        assert!(matches!(
            ProductionDaemonFactory::current()
                .build(writer_contention_config.clone())
                .unwrap()
                .run()
                .await,
            Err(DaemonError::WorkspaceWriterLease(
                WorkspaceWriterLeaseError::AlreadyHeld
            ))
        ));
        assert!(
            !writer_contention_config
                .static_config
                .socket_endpoint
                .exists()
        );
        held_writer.release().unwrap();
        generation_store.close().unwrap();
        DaemonLease::acquire(&writer_contention_config)
            .unwrap()
            .release()
            .unwrap();
    }

    #[tokio::test]
    async fn production_admin_bind_failure_joins_socket_writer_slot_and_daemon_owners() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let record = register_operational_workspace(&config, root.path());
        private_directory(&config.static_config.runtime_root).unwrap();
        private_file(
            &config.static_config.socket_endpoint,
            b"foreign-admin-owner",
        )
        .unwrap();
        let kernel = ProductionDaemonFactory::current()
            .build(config.clone())
            .unwrap();
        let lifecycle = kernel.lifecycle();
        let workspace_slots = kernel.workspace_slots();

        assert!(matches!(
            kernel.run().await,
            Err(DaemonError::Config(message))
                if message.contains("refusing to replace existing socket path")
        ));
        assert_eq!(
            lifecycle.observe().phase(),
            ProductionLifecyclePhase::Stopped
        );
        assert!(workspace_slots.is_shutdown());
        assert_eq!(
            fs::read(&config.static_config.socket_endpoint).unwrap(),
            b"foreign-admin-owner"
        );
        assert!(
            !config
                .static_config
                .runtime_root
                .join("daemon.json")
                .exists()
        );

        let writer_root = config.static_config.state_root.join("writer-authority");
        let generations = SqliteWriterGenerationStore::open(
            &config
                .static_config
                .state_root
                .join("writer-generations.sqlite"),
        )
        .unwrap();
        WorkspaceWriterLease::acquire(
            &writer_root,
            crate::fabric::command::WorkspaceId::from_bytes(record.workspace_id),
            LeaseId::from_bytes([7; 16]),
            &generations,
        )
        .unwrap()
        .release()
        .unwrap();
        generations.close().unwrap();
        DaemonLease::acquire(&config).unwrap().release().unwrap();
    }

    #[tokio::test]
    async fn production_partial_multi_workspace_fencing_releases_every_earlier_owner() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let first = add_operational_workspace(&config, &root.path().join("workspace-a"));
        let second = add_operational_workspace(&config, &root.path().join("workspace-b"));
        let mut records = [first, second];
        records.sort_by_key(|record| record.workspace_id);

        let writer_root = config.static_config.state_root.join("writer-authority");
        private_directory(&writer_root).unwrap();
        let generations = SqliteWriterGenerationStore::open(
            &config
                .static_config
                .state_root
                .join("writer-generations.sqlite"),
        )
        .unwrap();
        let held_last = WorkspaceWriterLease::acquire(
            &writer_root,
            crate::fabric::command::WorkspaceId::from_bytes(records[1].workspace_id),
            LeaseId::from_bytes([8; 16]),
            &generations,
        )
        .unwrap();
        let kernel = ProductionDaemonFactory::current()
            .build(config.clone())
            .unwrap();
        let lifecycle = kernel.lifecycle();
        let workspace_slots = kernel.workspace_slots();

        assert!(matches!(
            kernel.run().await,
            Err(DaemonError::WorkspaceWriterLease(
                WorkspaceWriterLeaseError::AlreadyHeld
            ))
        ));
        assert_eq!(
            lifecycle.observe().phase(),
            ProductionLifecyclePhase::Stopped
        );
        assert!(workspace_slots.is_shutdown());
        held_last.release().unwrap();

        for (index, record) in records.iter().enumerate() {
            WorkspaceWriterLease::acquire(
                &writer_root,
                crate::fabric::command::WorkspaceId::from_bytes(record.workspace_id),
                LeaseId::from_bytes([10 + index as u8; 16]),
                &generations,
            )
            .unwrap()
            .release()
            .unwrap();
        }
        generations.close().unwrap();
        DaemonLease::acquire(&config).unwrap().release().unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn production_binary_kernel_runs_honest_writer_fenced_bootstrap_and_restarts() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let registered = register_operational_workspace(&config, root.path());
        let discovery_path = config.static_config.runtime_root.join("daemon.json");

        for expected_generation in 1..=2 {
            let kernel = ProductionDaemonFactory::current()
                .build(config.clone())
                .unwrap();
            assert_eq!(
                kernel.release().suite().display(),
                "codefabric-relational-data-fabric@2.2.0"
            );
            let lifecycle = kernel.lifecycle();
            let workspace_slots = kernel.workspace_slots();
            assert!(!workspace_slots.is_closed());
            let task = tokio::spawn(kernel.run());
            wait_for_discovery(&discovery_path, Duration::from_secs(5))
                .await
                .unwrap();

            assert!(workspace_slots.is_closed());
            assert_eq!(workspace_slots.len(), 1);
            let slot = workspace_slots
                .slot(crate::fabric::command::WorkspaceId::from_bytes(
                    registered.workspace_id,
                ))
                .expect("kernel-owned workspace slot");
            assert!(matches!(
                slot.lease(),
                Err(crate::fabric::production_kernel::ActiveWorkspaceError::NotInstalled(_))
            ));

            let response = administer(&discovery_path, AdminCommand::Status)
                .await
                .unwrap();
            assert!(response.accepted);
            assert_eq!(
                response.workspace_readiness,
                "ENDPOINTS_BOUND_BOOTSTRAPPING"
            );
            assert_eq!(
                response.error_code.as_deref(),
                Some("SEMANTIC_AUTHORITY_BOOTSTRAPPING")
            );
            assert_eq!(response.workspaces, [registered.clone()]);
            assert!(!config.static_config.query_socket_endpoint.exists());
            assert_eq!(
                lifecycle.observe().phase(),
                ProductionLifecyclePhase::EndpointsBoundBootstrapping
            );

            administer(&discovery_path, AdminCommand::Stop)
                .await
                .unwrap();
            let exit = task.await.unwrap().unwrap();
            assert!(!exit.drained);
            assert_eq!(
                exit.shutdown_steps,
                [
                    "mark-draining",
                    "close-admin-ingress",
                    "retire-admin-endpoint",
                    "close-operational-registry",
                    "close-workspace-slots",
                    "close-writer-generation-store",
                    "release-writer-leases",
                    "release-daemon-lease",
                    "mark-stopped",
                ]
            );
            assert_eq!(
                lifecycle.observe().phase(),
                ProductionLifecyclePhase::Stopped
            );
            assert!(workspace_slots.is_shutdown());
            assert!(!config.static_config.socket_endpoint.exists());

            let generation_store = SqliteWriterGenerationStore::open(
                &config
                    .static_config
                    .state_root
                    .join("writer-generations.sqlite"),
            )
            .unwrap();
            let observed =
                crate::fabric::writer_lease::DurableWriterGenerationPort::observe_current(
                    &generation_store,
                    crate::fabric::command::WorkspaceId::from_bytes(registered.workspace_id),
                )
                .unwrap()
                .unwrap();
            assert_eq!(observed.generation.get(), expected_generation);
        }
    }
}
