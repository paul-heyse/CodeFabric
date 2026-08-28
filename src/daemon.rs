//! Local daemon lifecycle, closed configuration, singleton lease, and admin IPC.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Write as _};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::OptionalExtension as _;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, oneshot};

use crate::contracts::index::model_artifact_index;
use crate::contracts::models::DeploymentProfileDocument;
use crate::coordinator::{
    CoordinatorError, WorkspaceCoordinatorManager, WorkspaceHealthStatus,
    persisted_workspace_health,
};
use crate::fabric::{CommonRepositoryRecord, FabricError, bootstrap_workspace_with_repository};
use crate::golden_corpus::{CorpusError, core_source_v1_coverage, current_released_corpus_root};
use crate::identity::{IdentityDomain, encode_public_id};
use crate::operational_store::{OperationalReaderFactory, OperationalStore, OperationalStoreError};
use crate::query_service::{
    ProductionQueryService, QueryAuthorization, QueryTransportError, ResultArtifactStore,
    WorkspaceQueryBackend, serve_query_uds,
};
use crate::registries::WorkspaceRegistryLifecycle;
use crate::rpc::SameUserInterceptor;
use crate::rpc::generated::codefabric::cpgd::v1::{WorkspaceClaim, WorkspaceReadiness};
use crate::workspace_registry::{
    RelinkProof, RemovalPolicy, WorkspaceRecord, WorkspaceRegistry, WorkspaceRegistryError,
    WorkspaceSourceRegistration,
};

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
    /// Packaged contract bundle/index location.
    pub bundle_index: PathBuf,
    /// Closed toolchain identity location.
    pub toolchain_identity: PathBuf,
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
    ActivateCandidate {
        workspace_id: [u8; 16],
        candidate_identity: String,
        decision_identity: String,
        request_key: String,
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
    pub error_code: Option<String>,
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
    #[error(transparent)]
    OperationalStore(#[from] OperationalStoreError),
    #[error(transparent)]
    Coordinator(#[from] CoordinatorError),
    #[error(transparent)]
    WorkspaceRegistry(#[from] WorkspaceRegistryError),
    #[error(transparent)]
    Fabric(#[from] FabricError),
    #[error(transparent)]
    QueryTransport(#[from] QueryTransportError),
    #[error(transparent)]
    Corpus(#[from] CorpusError),
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

fn open_operational_store_with_provider_recovery(
    path: &Path,
) -> Result<OperationalStore, DaemonError> {
    let mut store = OperationalStore::open(path)?;
    store.validate_ontology_activation_recovery()?;
    let recovered_at = now_millis()?.to_string();
    let recovered_provider_runs = store.recover_incomplete_provider_runs(&recovered_at)?;
    tracing::info!(
        recovered_provider_runs,
        lifecycle = "provider-recovery",
        "reconciled non-terminal provider runs before admission"
    );
    Ok(store)
}

fn discovery(config: &DaemonConfig) -> Result<DaemonDiscovery, DaemonError> {
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
    let public_bundle_versions = model_artifact_index()
        .map_err(|error| DaemonError::Admin(format!("artifact index: {error}")))?
        .artifacts
        .iter()
        .filter(|artifact| artifact.artifact_kind == "bundle-manifest")
        .map(|artifact| (artifact.artifact_id.clone(), artifact.version.clone()))
        .collect();
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

fn workspace_readiness(health: &[WorkspaceHealthStatus]) -> String {
    if health.is_empty() {
        "NO_WORKSPACES_READY"
    } else if health
        .iter()
        .any(|workspace| workspace.readiness == "READY")
    {
        "WORKSPACES_READY"
    } else {
        "WORKSPACE_BOOTSTRAPPING"
    }
    .to_owned()
}

async fn execute_workspace_command(
    store: &Arc<Mutex<OperationalStore>>,
    coordinators: &mut WorkspaceCoordinatorManager,
    state_root: &Path,
    command: WorkspaceAdminCommand,
) -> AdminResponse {
    let coordinator_bootstrap = match &command {
        WorkspaceAdminCommand::Enable { workspace_id }
        | WorkspaceAdminCommand::Reconcile { workspace_id } => Some(*workspace_id),
        _ => None,
    };
    let stop_workspace = match &command {
        WorkspaceAdminCommand::Disable { workspace_id }
        | WorkspaceAdminCommand::Remove { workspace_id, .. } => Some(*workspace_id),
        _ => None,
    };
    let result: Result<Vec<WorkspaceRecord>, String> = {
        let mut store = store.lock().await;
        match command {
            WorkspaceAdminCommand::ActivateCandidate {
                workspace_id,
                candidate_identity,
                decision_identity,
                request_key,
            } => {
                let requested_at = now_millis()
                    .ok()
                    .and_then(|value| i64::try_from(value).ok())
                    .unwrap_or(i64::MAX);
                store
                    .resolve_ontology_activation_request(
                        workspace_id,
                        &candidate_identity,
                        &decision_identity,
                        &request_key,
                        requested_at,
                    )
                    .and_then(|request| store.activate_ontology_candidate(&request))
                    .map(|_| Vec::new())
                    .map_err(|error| ontology_activation_error_code(&error).to_owned())
            }
            other => execute_workspace_command_inner(&mut store, other)
                .map_err(|error| workspace_error_code(&error).to_owned()),
        }
    };
    match result {
        Ok(workspaces) => {
            if let Some(workspace_id) = stop_workspace
                && coordinators.stop(workspace_id).await.is_err()
            {
                return internal_admin_response();
            }
            if let Some(workspace_id) = coordinator_bootstrap {
                let handle = match coordinators.handle(workspace_id) {
                    Some(handle) => handle,
                    None => match coordinators.spawn(workspace_id).await {
                        Ok(handle) => handle,
                        Err(_) => return internal_admin_response(),
                    },
                };
                if let Err(error) = handle.bootstrap().await {
                    return coordinator_admin_response(&error);
                }
            }
            if bootstrap_fabrics(state_root, store, &workspaces)
                .await
                .is_err()
            {
                return internal_admin_response();
            }
            match health_response(store).await {
                Ok((workspace_readiness, workspace_health)) => AdminResponse {
                    accepted: true,
                    daemon_liveness: "LIVE".to_owned(),
                    workspace_readiness,
                    shutdown_mode: None,
                    workspaces,
                    workspace_health,
                    error_code: None,
                },
                Err(_) => internal_admin_response(),
            }
        }
        Err(error) => AdminResponse {
            accepted: false,
            daemon_liveness: "LIVE".to_owned(),
            workspace_readiness: "UNCHANGED".to_owned(),
            shutdown_mode: None,
            workspaces: Vec::new(),
            workspace_health: Vec::new(),
            error_code: Some(error),
        },
    }
}

fn common_repository_record(
    readers: &OperationalReaderFactory,
    repository_id: Option<[u8; 16]>,
) -> Result<Option<CommonRepositoryRecord>, DaemonError> {
    let Some(repository_id) = repository_id else {
        return Ok(None);
    };
    let reader = readers.open()?;
    let row = reader
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT common_dir_path_bytes, common_dir_path_display, object_format_code, trust_policy_fingerprint, updated_at FROM common_repository_state WHERE repository_id=?1",
                    [repository_id.as_slice()],
                    |row| {
                        Ok((
                            row.get::<_, Vec<u8>>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, Vec<u8>>(3)?,
                            row.get::<_, String>(4)?,
                        ))
                    },
                )
                .optional()
        })
        .map_err(OperationalStoreError::from)?;
    let Some((path_bytes, path_display, object_format, fingerprint, updated_at)) = row else {
        return Ok(None);
    };
    let trust_policy_fingerprint = fingerprint.try_into().map_err(|_| {
        DaemonError::Admin("persisted repository trust fingerprint has invalid width".into())
    })?;
    let object_format_code = i16::try_from(object_format).map_err(|_| {
        DaemonError::Admin("persisted repository object format is out of range".into())
    })?;
    Ok(Some(CommonRepositoryRecord {
        repository_id,
        common_dir_path_bytes: path_bytes,
        common_dir_path_display: path_display,
        object_format_code,
        trust_policy_fingerprint,
        updated_at,
    }))
}

async fn bootstrap_fabrics(
    state_root: &Path,
    store: &Arc<Mutex<OperationalStore>>,
    workspaces: &[WorkspaceRecord],
) -> Result<(), DaemonError> {
    let readers = store.lock().await.reader_factory();
    for workspace in workspaces {
        if workspace.status == WorkspaceRegistryLifecycle::Removed {
            continue;
        }
        let repository = common_repository_record(&readers, workspace.repository_id)?;
        bootstrap_workspace_with_repository(state_root, workspace, repository.as_ref()).await?;
    }
    Ok(())
}

async fn health_response(
    store: &Arc<Mutex<OperationalStore>>,
) -> Result<(String, Vec<WorkspaceHealthStatus>), DaemonError> {
    let health = {
        let store = store.lock().await;
        persisted_workspace_health(&store)?
    };
    Ok((workspace_readiness(&health), health))
}

fn coordinator_admin_response(error: &CoordinatorError) -> AdminResponse {
    AdminResponse {
        accepted: false,
        daemon_liveness: "LIVE".to_owned(),
        workspace_readiness: "WORKSPACE_BOOTSTRAPPING".to_owned(),
        shutdown_mode: None,
        workspaces: Vec::new(),
        workspace_health: Vec::new(),
        error_code: Some(
            if matches!(error, CoordinatorError::SourceChanged) {
                "WORKSPACE_BOOTSTRAPPING"
            } else {
                "INTERNAL"
            }
            .to_owned(),
        ),
    }
}

fn internal_admin_response() -> AdminResponse {
    AdminResponse {
        accepted: false,
        daemon_liveness: "LIVE".to_owned(),
        workspace_readiness: "UNKNOWN".to_owned(),
        shutdown_mode: None,
        workspaces: Vec::new(),
        workspace_health: Vec::new(),
        error_code: Some("INTERNAL".to_owned()),
    }
}

fn execute_workspace_command_inner(
    store: &mut OperationalStore,
    command: WorkspaceAdminCommand,
) -> Result<Vec<WorkspaceRecord>, WorkspaceRegistryError> {
    let mut registry = WorkspaceRegistry::new(store);
    Ok(match command {
        WorkspaceAdminCommand::Add { root } => {
            vec![registry.add(&root, WorkspaceSourceRegistration::Directory)?]
        }
        WorkspaceAdminCommand::List => registry.list()?,
        WorkspaceAdminCommand::Show { workspace_id } => vec![registry.show(workspace_id)?],
        WorkspaceAdminCommand::Relink {
            workspace_id,
            new_root,
            proof,
        } => vec![registry.relink(workspace_id, &new_root, &proof)?],
        WorkspaceAdminCommand::Configure {
            workspace_id,
            profile_manifest,
        } => {
            let metadata = fs::symlink_metadata(&profile_manifest).map_err(|error| {
                WorkspaceRegistryError::Root(format!("profile manifest: {error}"))
            })?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.len() > 1_048_576
            {
                return Err(WorkspaceRegistryError::Root(
                    "profile manifest must be a bounded non-symlink file".into(),
                ));
            }
            let bytes = fs::read(&profile_manifest).map_err(|error| {
                WorkspaceRegistryError::Root(format!("profile manifest: {error}"))
            })?;
            vec![registry.configure(workspace_id, crate::integrity::digest_bytes(&bytes))?]
        }
        WorkspaceAdminCommand::Enable { workspace_id } => vec![registry.enable(workspace_id)?],
        WorkspaceAdminCommand::Disable { workspace_id } => vec![registry.disable(workspace_id)?],
        WorkspaceAdminCommand::Reconcile { workspace_id } => {
            vec![registry.reconcile(workspace_id)?]
        }
        WorkspaceAdminCommand::ActivateCandidate { .. } => unreachable!(
            "ontology activation is handled by the sole durable owner route before registry dispatch"
        ),
        WorkspaceAdminCommand::Remove {
            workspace_id,
            policy,
            purge_confirmations,
        } => vec![registry.remove(workspace_id, policy, purge_confirmations)?],
    })
}

const fn ontology_activation_error_code(error: &OperationalStoreError) -> &'static str {
    match error {
        OperationalStoreError::OntologyActivation(_)
        | OperationalStoreError::OntologyActivationOutcomeUnknown { .. } => {
            "ONTOLOGY_ACTIVATION_TRANSACTION_INVALID"
        }
        _ => "INTERNAL",
    }
}

const fn workspace_error_code(error: &WorkspaceRegistryError) -> &'static str {
    match error {
        WorkspaceRegistryError::StateTransitionViolation { .. }
        | WorkspaceRegistryError::DuplicateAdministrativeKey
        | WorkspaceRegistryError::ActiveLease => "STATE_TRANSITION_VIOLATION",
        WorkspaceRegistryError::Root(_)
        | WorkspaceRegistryError::NotFound(_)
        | WorkspaceRegistryError::RelinkProof => "WORKSPACE_NOT_AUTHORIZED",
        WorkspaceRegistryError::PurgeConfirmation => "INVALID_REQUEST_SCHEMA",
        WorkspaceRegistryError::Store(_)
        | WorkspaceRegistryError::Identity(_)
        | WorkspaceRegistryError::AnalysisContext(_)
        | WorkspaceRegistryError::Sqlite(_)
        | WorkspaceRegistryError::Persisted(_) => "INTERNAL",
    }
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

fn workspace_claim(record: &WorkspaceRecord) -> Result<WorkspaceClaim, DaemonError> {
    Ok(WorkspaceClaim {
        workspace_id: encode_public_id(IdentityDomain::Workspace, None, record.workspace_id)
            .map_err(|error| DaemonError::Admin(error.to_string()))?,
        repository_id: record
            .repository_id
            .map(|id| encode_public_id(IdentityDomain::Repository, None, id))
            .transpose()
            .map_err(|error| DaemonError::Admin(error.to_string()))?,
        worktree_id: record
            .worktree_id
            .map(|id| encode_public_id(IdentityDomain::Worktree, None, id))
            .transpose()
            .map_err(|error| DaemonError::Admin(error.to_string()))?,
        workspace_kind: if record.repository_id.is_some() {
            "git-worktree"
        } else {
            "non-git-root"
        }
        .to_owned(),
        readiness: if record.status == WorkspaceRegistryLifecycle::Ready {
            WorkspaceReadiness::Ready
        } else {
            WorkspaceReadiness::Bootstrapping
        } as i32,
        permission_claims: vec!["query".to_owned()],
    })
}

/// Run the local administrative service until a joined stop or no-work drain completes.
///
/// # Errors
///
/// Returns startup, permission, socket, protocol, or joined-shutdown failures.
#[allow(clippy::too_many_lines)] // The ordered lifecycle is kept visible as one joined sequence.
pub async fn serve(config: DaemonConfig) -> Result<DaemonExit, DaemonError> {
    serve_with_query_backend(
        config,
        Arc::new(WorkspaceQueryBackend::default()),
        Vec::new(),
        None,
    )
    .await
}

#[allow(clippy::too_many_lines)] // The ordered lifecycle is kept visible as one joined sequence.
pub(crate) async fn serve_with_query_backend(
    config: DaemonConfig,
    query_backend: Arc<WorkspaceQueryBackend>,
    additional_claims: Vec<WorkspaceClaim>,
    query_allowed_uid_override: Option<u32>,
) -> Result<DaemonExit, DaemonError> {
    config.validate()?;
    for root in [
        &config.static_config.state_root,
        &config.static_config.runtime_root,
        &config.static_config.config_root,
    ] {
        private_directory(root)?;
    }
    let lease = DaemonLease::acquire(&config)?;
    let operational_database = config
        .static_config
        .state_root
        .join(&config.static_config.operational_database);
    let operational_store = Arc::new(Mutex::new(open_operational_store_with_provider_recovery(
        &operational_database,
    )?));
    let mut coordinators = WorkspaceCoordinatorManager::new(
        Arc::clone(&operational_store),
        config.static_config.state_root.clone(),
    )?;
    coordinators.restore_and_bootstrap().await?;
    let workspaces = {
        let mut store = operational_store.lock().await;
        WorkspaceRegistry::new(&mut store).list()?
    };
    let mut claims = workspaces
        .iter()
        .filter(|workspace| workspace.status != WorkspaceRegistryLifecycle::Removed)
        .map(|workspace| {
            workspace_claim(workspace).map(|claim| (claim.workspace_id.clone(), claim))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    for claim in additional_claims {
        claims.insert(claim.workspace_id.clone(), claim);
    }
    bootstrap_fabrics(
        &config.static_config.state_root,
        &operational_store,
        &workspaces,
    )
    .await?;
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
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let coverage = core_source_v1_coverage(&current_released_corpus_root(repository_root)?)?;
    let result_root = config.static_config.state_root.join("query-results");
    let query_service = ProductionQueryService::new(
        query_backend,
        ResultArtifactStore::new(result_root.clone()).map_err(|source| DaemonError::Io {
            path: result_root,
            source,
        })?,
        query_authorization,
        crate::lifecycle::FreshnessBarrier::default(),
        Duration::from_secs(2),
    )
    .with_core_source_coverage(coverage);
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
    lease.publish(&discovery(&config)?)?;
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
                    AdminCommand::Stop => Some("stop".to_owned()),
                    AdminCommand::Drain => Some("drain".to_owned()),
                };
                let (workspace_readiness, workspace_health) =
                    health_response(&operational_store).await?;
                (
                    AdminResponse {
                        accepted: true,
                        daemon_liveness: "LIVE".to_owned(),
                        workspace_readiness,
                        shutdown_mode: shutdown_mode.clone(),
                        workspaces: Vec::new(),
                        workspace_health,
                        error_code: None,
                    },
                    shutdown_mode,
                )
            }
            AdminEnvelope::Workspace(command) => (
                execute_workspace_command(
                    &operational_store,
                    &mut coordinators,
                    &config.static_config.state_root,
                    command,
                )
                .await,
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
    let coordinator_result = coordinators
        .shutdown_all()
        .await
        .map_err(|error| error.to_string());
    record_shutdown_step(
        &mut steps,
        "await-workers",
        query_result.and(coordinator_result),
    )?;
    let durable_close = if drained {
        operational_store
            .lock()
            .await
            .checkpoint()
            .map_err(|error| error.to_string())
    } else {
        Ok(())
    };
    record_shutdown_step(&mut steps, "close-durable-stores", durable_close)?;
    drop(operational_store);
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

    #[test]
    fn provider_run_daemon_startup_recovery() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("operational.sqlite");
        let mut store = OperationalStore::open(&path).unwrap();
        for (byte, state) in [
            (0x31, crate::registries::ProviderRunState::Queued),
            (0x32, crate::registries::ProviderRunState::Running),
        ] {
            store
                .record_provider_run(&crate::operational_store::ProviderRunRecord {
                    provider_run_id: vec![byte; 16],
                    workspace_id: vec![0x33; 16],
                    analysis_context_id: vec![0x34; 16],
                    wave_id: vec![0x35; 16],
                    provider_code: 40,
                    owner_id: None,
                    build_unit_id: None,
                    source_generation: 1,
                    input_fingerprint: vec![byte; 32],
                    output_fingerprint: None,
                    sandbox_profile_digest: Some(format!("b3:{}", "36".repeat(32))),
                    state_code: i64::from(state as u16),
                    accepted_at: "1".into(),
                    terminal_at: None,
                    diagnostic_id: None,
                })
                .unwrap();
        }
        drop(store);

        let recovered = open_operational_store_with_provider_recovery(&path).unwrap();
        let non_terminal = recovered
            .reader_factory()
            .open()
            .unwrap()
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT COUNT(*) FROM provider_run WHERE terminal_at IS NULL",
                    [],
                    |row| row.get::<_, i64>(0),
                )
            })
            .unwrap();
        assert_eq!(non_terminal, 0);
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
                bundle_index: PathBuf::from(
                    "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_artifact_index.json",
                ),
                toolchain_identity: PathBuf::from("contracts/toolchain/toolchain-identity.json"),
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
bundle_index = "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_artifact_index.json"
toolchain_identity = "contracts/toolchain/toolchain-identity.json"
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

        let discovery = discovery(&config).unwrap();
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
        let task = tokio::spawn(serve(config.clone()));
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
        let task = tokio::spawn(serve(config));
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
}
