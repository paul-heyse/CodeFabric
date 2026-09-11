//! Supervisor-owned daemon lifecycle, closed configuration, and query serving.

use std::fmt;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io;
#[cfg(test)]
use std::io::Write as _;
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::net::UnixStream;
use tokio::sync::watch;
use tokio_stream::StreamExt as _;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;

use crate::cancellation::StructuredCancellationScope;
use crate::contracts::deployment_profile::DeploymentProfileDocument;
use crate::fabric::command::LeaseId;
use crate::fabric::production_kernel::{
    CompiledSemanticRelease, LifecycleAuthority, OperationalWorkspaceRegistry,
    OperationalWorkspaceRegistryError, ProductionLifecycleError, ProductionLifecyclePhase,
    WorkspaceSlotRegistry, WorkspaceSlotRegistryError,
};
use crate::fabric::production_workspace_startup::{
    ProductionWorkspaceStartup, ProductionWorkspaceStartupAssuranceFault,
    WORKSPACE_OPERATION_DRAIN_TIMEOUT, start_production_workspace,
};
use crate::fabric::programmatic_query_backend::ProgrammaticSemanticQueryBackend;
use crate::fabric::query_coordinator::{
    QueryCoordinator, QueryCoordinatorPolicy, SqliteQueryCoordinatorJournal,
};
use crate::fabric::streamed_result_package::{
    ObjectStoreResultSink, StreamedResultPackageBuilder, StreamedResultPackageLimits,
};
use crate::fabric::streamed_result_registry::StreamedResultRegistry;
use crate::fabric::writer_generation_sqlite::{
    SqliteWriterGenerationCloseError, SqliteWriterGenerationOpenError, SqliteWriterGenerationStore,
};
use crate::fabric::writer_lease::{WorkspaceWriterLease, WorkspaceWriterLeaseError};
use crate::operational_store::OperationalStore;
use crate::operational_store::OperationalStoreError;
use crate::owned_unix_socket::{OwnedUnixSocket, OwnedUnixSocketError};
use crate::query_service::ProductionQueryService;
use crate::rpc::AuthorizedUnixStream;
use crate::rpc::generated::codefabric::cpgd::v2::cpg_query_service_server::CpgQueryServiceServer;
use crate::secure_path::read_private_control_artifact;
use crate::session_authority::{LaunchGrantAuthority, SessionAuthorityError};
use crate::supervisor::{
    DaemonControlHeader, DaemonControlHello, DaemonControlReadState, DaemonControlRequest,
    SupervisorError, accept_control_hello, acknowledge_control, read_daemon_control,
};
use crate::workspace_registry::{WorkspaceRegistry, WorkspaceRegistryError};

const CONFIG_MAX_BYTES: u64 = 262_144;
const SOCKET_PATH_MAX_BYTES: usize = 103;
const DEPLOYMENT_PROFILE: &[u8] =
    include_bytes!("../contracts/deployment/local-workstation-v1.yaml");

/// Debug-build-only startup interruption used by the real-process assurance harness.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationStartupAssuranceFault {
    /// Make the durable successor append appear unknown before any local commit readback.
    DurableAppendAcknowledgementLostBeforeReadback,
    /// Exit the real controlled daemon after binding its endpoint but before ready acknowledgement.
    ExitBeforeReadyAcknowledgement,
    /// Pause completed live semantic candidates until a private harness marker is released.
    HoldSemanticUpdatePublication,
}

/// Explicit filesystem observation backend. Polling is a deployment choice, never a fallback.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceWatchProfile {
    /// Native filesystem notifications with pruned directory registration.
    #[default]
    Native,
    /// Two-second metadata polling with independent 150 ms debouncing.
    Poll,
}

impl SourceWatchProfile {
    // Serde's skip_serializing_if callback receives a reference.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    const fn is_native(&self) -> bool {
        matches!(self, Self::Native)
    }
}

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
    /// Unix-domain authenticated query endpoint.
    pub query_socket_endpoint: PathBuf,
    /// Operational database filename, relative to the state root.
    pub operational_database: PathBuf,
    /// Exact sandbox policy from the released deployment profile.
    pub sandbox_policy: String,
    /// Hard-limit profile selected at startup.
    pub hard_limit_profile: String,
    /// Exact supported deployment profile.
    pub supported_platform_profile: String,
    /// Selected filesystem observer; omission preserves the native deployment default.
    #[serde(default, skip_serializing_if = "SourceWatchProfile::is_native")]
    pub source_watch_profile: SourceWatchProfile,
    /// Optional debug-build-only real-process activation interruption.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activation_startup_assurance_fault: Option<ActivationStartupAssuranceFault>,
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
        let bytes = read_private_control_artifact(path, CONFIG_MAX_BYTES)
            .map_err(|error| DaemonError::Config(error.to_string()))?;
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
        if static_config.activation_startup_assurance_fault.is_some() && !cfg!(debug_assertions) {
            return Err(DaemonError::Config(
                "activation startup assurance faults are unavailable in release builds".into(),
            ));
        }
        if !static_config.state_root.is_absolute()
            || !static_config.runtime_root.is_absolute()
            || !static_config.config_root.is_absolute()
            || !static_config.query_socket_endpoint.is_absolute()
            || !static_config
                .query_socket_endpoint
                .starts_with(&static_config.runtime_root)
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
        for socket in [&static_config.query_socket_endpoint] {
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
    #[error("daemon serving failure: {0}")]
    Serving(String),
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
    #[error(transparent)]
    OwnedUnixSocket(#[from] OwnedUnixSocketError),
    #[error(transparent)]
    Supervisor(#[from] SupervisorError),
    #[error("production startup has no explicit operational workspace")]
    NoOperationalWorkspace,
    #[error(transparent)]
    Identity(#[from] crate::identity::IdentityError),
    #[error(transparent)]
    SemanticRelease(#[from] crate::semantic_release::SemanticReleaseError),
    #[error(transparent)]
    QueryService(#[from] crate::query_service::QueryServiceCompositionError),
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

#[cfg(test)]
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

/// Held process-local safety lock beneath the supervisor-owned singleton.
pub struct DaemonLease {
    lock: File,
    lock_path: PathBuf,
    released: bool,
}

impl DaemonLease {
    /// Acquire the state-root singleton before touching an existing endpoint.
    ///
    /// # Errors
    ///
    /// Returns permission or lock-contention errors.
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
        Ok(Self {
            lock,
            lock_path,
            released: false,
        })
    }

    /// Explicitly release the daemon's subordinate process lock.
    ///
    /// # Errors
    ///
    /// Returns an unlock failure. Drop remains only a partial-construction safety net and is not
    /// successful joined-shutdown evidence.
    pub fn release(mut self) -> Result<(), DaemonError> {
        self.lock.unlock().map_err(|source| DaemonError::Io {
            path: self.lock_path.clone(),
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
        let _ = self.lock.unlock();
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
    generation_store: Arc<SqliteWriterGenerationStore>,
    writer_leases: Vec<WorkspaceWriterLease>,
}

/// Phase-typed startup owner. Semantic phase values cannot be supplied by configuration.
pub struct ProductionStartupCoordinator<Phase> {
    config: DaemonConfig,
    release: Arc<CompiledSemanticRelease>,
    lifecycle: Arc<LifecycleAuthority>,
    workspace_slots: Arc<WorkspaceSlotRegistry>,
    phase: Phase,
}

fn close_writer_generation_store(
    store: Arc<SqliteWriterGenerationStore>,
) -> Result<(), DaemonError> {
    Arc::try_unwrap(store)
        .map_err(|_| DaemonError::Shutdown {
            completed_steps: Vec::new(),
            detail: "writer-generation authority is still retained during joined shutdown"
                .to_owned(),
        })?
        .close()
        .map_err(DaemonError::from)
}

fn cleanup_owned_startup_failure(
    lifecycle: &LifecycleAuthority,
    workspace_slots: &WorkspaceSlotRegistry,
    daemon_lease: DaemonLease,
    registry: Option<OperationalWorkspaceRegistry>,
    generation_store: Option<Arc<SqliteWriterGenerationStore>>,
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
            close_writer_generation_store(store),
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
    fn new(config: DaemonConfig, release: Arc<CompiledSemanticRelease>) -> Self {
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
            Ok(store) => Arc::new(store),
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
                generation_store.as_ref(),
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
#[derive(Clone, Debug)]
pub struct ProductionDaemonFactory {
    release: Arc<CompiledSemanticRelease>,
}

impl ProductionDaemonFactory {
    /// Compile the one closed semantic release before any workspace can become ready.
    ///
    /// # Errors
    ///
    /// Returns a release-definition or closed-program validation failure. There is no fallback
    /// release, runtime selector, or partially initialized daemon kernel.
    pub fn compile() -> Result<Self, DaemonError> {
        let definition =
            crate::production_provider_recipe::current_v23_provider_program_definition()?;
        let release = crate::semantic_release::compile_current_v23_release(definition)?;
        Ok(Self {
            release: Arc::new(release),
        })
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
            .field("suite", &self.startup.release.suite().as_str())
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
    pub const fn release(&self) -> &Arc<CompiledSemanticRelease> {
        &self.startup.release
    }

    /// Run the target-only daemon after an authenticated supervisor hello has been consumed.
    pub async fn run_controlled(
        self,
        control: UnixStream,
        hello: DaemonControlHello,
        hello_header: DaemonControlHeader,
        control_state: DaemonControlReadState,
    ) -> Result<DaemonExit, DaemonError> {
        let lifecycle = Arc::clone(&self.startup.lifecycle);
        let result = async {
            let leased = self.startup.acquire_daemon_lease()?;
            let fenced = leased.acquire_workspace_writers()?;
            serve_writer_fenced_v2(fenced, control, hello, hello_header, control_state).await
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

fn finish_writer_fenced(
    startup: ProductionStartupCoordinator<WriterFenced>,
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
        close_writer_generation_store(generation_store),
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

async fn finish_writer_fenced_v2(
    startup: ProductionStartupCoordinator<WriterFenced>,
    workspace: ProductionWorkspaceStartup,
    drained: bool,
    primary: Option<DaemonError>,
) -> Result<DaemonExit, DaemonError> {
    let workspace_shutdown = workspace.shutdown().await;
    let primary = match (primary, workspace_shutdown) {
        (Some(primary), _) => Some(primary),
        (None, Ok(())) => None,
        (None, Err(error)) => Some(DaemonError::Shutdown {
            completed_steps: Vec::new(),
            detail: error.to_string(),
        }),
    };
    finish_writer_fenced(startup, drained, primary)
}

fn production_query_coordinator_policy() -> Result<QueryCoordinatorPolicy, DaemonError> {
    QueryCoordinatorPolicy::try_new(
        16,
        8,
        4,
        128,
        256,
        1_024,
        4 * 1024 * 1024,
        4 * 1024 * 1024 * 1024,
        65_536,
        10_000,
    )
    .map_err(|error| DaemonError::Config(format!("query coordinator policy: {error}")))
}

#[allow(clippy::too_many_lines)]
async fn serve_writer_fenced_v2(
    mut startup: ProductionStartupCoordinator<WriterFenced>,
    mut control: UnixStream,
    hello: DaemonControlHello,
    hello_header: DaemonControlHeader,
    mut control_state: DaemonControlReadState,
) -> Result<DaemonExit, DaemonError> {
    if startup.phase.registry.records().len() != 1 || startup.phase.writer_leases.len() != 1 {
        let error = DaemonError::Config(
            "one supervisor-launched daemon must own exactly one operational workspace".into(),
        );
        return finish_writer_fenced(startup, false, Some(error));
    }
    let record = startup.phase.registry.records()[0].clone();
    if hello_header.workspace_id != record.workspace_id
        || control_state.workspace_id() != record.workspace_id
    {
        let error = DaemonError::Config(
            "daemon control workspace binding differs from the operational workspace".into(),
        );
        return finish_writer_fenced(startup, false, Some(error));
    }
    let workspace_id = crate::fabric::command::WorkspaceId::from_bytes(record.workspace_id);
    let policy = match production_query_coordinator_policy() {
        Ok(policy) => policy,
        Err(error) => return finish_writer_fenced(startup, false, Some(error)),
    };
    let process_id = match daemon_random32() {
        Ok(value) => value[..16]
            .try_into()
            .expect("sixteen-byte process identity"),
        Err(error) => return finish_writer_fenced(startup, false, Some(error)),
    };
    let workspace_resources = match crate::resource_budget::ResourceBudget::try_process(
        process_id,
        crate::fabric::workspace_resources::local_resource_policy(),
    )
    .map_err(|error| error.to_string())
    .and_then(|process| {
        crate::fabric::workspace_resources::ProductionWorkspaceResources::try_new(
            &process,
            workspace_id,
            &startup.config.static_config.state_root,
        )
    }) {
        Ok(resources) => resources,
        Err(error) => {
            return finish_writer_fenced(startup, false, Some(DaemonError::Config(error)));
        }
    };
    let daemon_task_scope = match StructuredCancellationScope::try_root_with_control_reserve(
        "daemon",
        policy.structured_task_capacity(),
        std::num::NonZeroUsize::new(16).expect("control task reserve is positive"),
    ) {
        Ok(scope) => scope,
        Err(error) => {
            return finish_writer_fenced(
                startup,
                false,
                Some(DaemonError::Config(error.to_string())),
            );
        }
    };
    let Some(slot) = startup.workspace_slots.slot(workspace_id) else {
        let error =
            DaemonError::Config("operational workspace has no closed authority slot".into());
        return finish_writer_fenced(startup, false, Some(error));
    };
    let writer_lease = startup.phase.writer_leases.swap_remove(0);
    let operational_database = startup
        .config
        .static_config
        .state_root
        .join(&startup.config.static_config.operational_database);
    let workspace = match start_production_workspace(
        &startup.config.static_config.state_root,
        &operational_database,
        &record,
        Arc::clone(&startup.release),
        slot,
        Arc::clone(&startup.phase.generation_store),
        writer_lease,
        startup
            .config
            .static_config
            .activation_startup_assurance_fault
            .and_then(|fault| match fault {
                ActivationStartupAssuranceFault::DurableAppendAcknowledgementLostBeforeReadback => {
                    Some(ProductionWorkspaceStartupAssuranceFault::DurableAppendAcknowledgementLostBeforeReadback)
                }
                ActivationStartupAssuranceFault::ExitBeforeReadyAcknowledgement => None,
                ActivationStartupAssuranceFault::HoldSemanticUpdatePublication => Some(ProductionWorkspaceStartupAssuranceFault::HoldSemanticUpdatePublication),
            }),
        workspace_resources,
        startup.config.static_config.source_watch_profile,
        daemon_task_scope.clone(),
    )
    .await
    {
        Ok(workspace) => workspace,
        Err(error) => {
            let joined = daemon_task_scope
                .cancel_and_join(WORKSPACE_OPERATION_DRAIN_TIMEOUT)
                .await;
            let detail = match joined {
                Ok(()) => error.to_string(),
                Err(join) => format!("{error}; startup operation join: {join}"),
            };
            return finish_writer_fenced(
                startup,
                false,
                Some(DaemonError::Config(detail)),
            );
        }
    };
    if let Err(error) = startup.lifecycle.advance(
        ProductionLifecyclePhase::WriterFenced,
        ProductionLifecyclePhase::CommandRecovered,
    ) {
        return finish_writer_fenced_v2(startup, workspace, false, Some(error.into())).await;
    }
    let recovered = if workspace.fresh_activation() {
        ProductionLifecyclePhase::GenesisRequired
    } else {
        ProductionLifecyclePhase::SelectedEpochRecovered
    };
    if let Err(error) = startup
        .lifecycle
        .advance(ProductionLifecyclePhase::CommandRecovered, recovered)
        .and_then(|_| {
            startup
                .lifecycle
                .advance(recovered, ProductionLifecyclePhase::EpochBuiltAndProved)
        })
        .and_then(|_| {
            startup.lifecycle.advance(
                ProductionLifecyclePhase::EpochBuiltAndProved,
                ProductionLifecyclePhase::WorkspaceInstalledClosed,
            )
        })
    {
        return finish_writer_fenced_v2(startup, workspace, false, Some(error.into())).await;
    }
    let result_root = startup
        .config
        .static_config
        .state_root
        .join("query-results");
    if let Err(error) = private_directory(&result_root) {
        return finish_writer_fenced_v2(startup, workspace, false, Some(error)).await;
    }
    let object_store = match object_store::local::LocalFileSystem::new_with_prefix(&result_root) {
        Ok(store) => Arc::new(store) as Arc<dyn object_store::ObjectStore>,
        Err(error) => {
            return finish_writer_fenced_v2(
                startup,
                workspace,
                false,
                Some(DaemonError::Config(format!("result object store: {error}"))),
            )
            .await;
        }
    };
    let package_limits = match StreamedResultPackageLimits::try_new(
        64,
        4_096,
        8_192,
        crate::rpc::MAX_PAYLOAD_CHUNK_BYTES,
        10_000_000,
        512 * 1024 * 1024,
        4 * 1024 * 1024,
        4_096,
        4 * 1024 * 1024,
    ) {
        Ok(limits) => limits,
        Err(error) => {
            return finish_writer_fenced_v2(
                startup,
                workspace,
                false,
                Some(DaemonError::Config(format!(
                    "result package limits: {error}"
                ))),
            )
            .await;
        }
    };
    let package_builder = StreamedResultPackageBuilder::new(
        Arc::new(ObjectStoreResultSink::new(object_store)),
        package_limits,
        workspace.resources().budget().clone(),
    );
    let journal = match SqliteQueryCoordinatorJournal::open(
        &startup
            .config
            .static_config
            .state_root
            .join("query-coordinator.sqlite3"),
    ) {
        Ok(journal) => Arc::new(journal),
        Err(error) => {
            return finish_writer_fenced_v2(
                startup,
                workspace,
                false,
                Some(DaemonError::Config(format!(
                    "query coordinator journal: {error}"
                ))),
            )
            .await;
        }
    };
    let cursor_secret = match daemon_random32() {
        Ok(secret) => secret,
        Err(error) => {
            return finish_writer_fenced_v2(startup, workspace, false, Some(error)).await;
        }
    };
    let observed_at = match now_millis().and_then(|value| {
        i64::try_from(value).map_err(|_| DaemonError::Config("system clock exceeds i64".into()))
    }) {
        Ok(value) => value,
        Err(error) => {
            return finish_writer_fenced_v2(startup, workspace, false, Some(error)).await;
        }
    };
    let query_task_scope = match daemon_task_scope
        .child("workspace")
        .and_then(|scope| scope.child("primary"))
        .and_then(|scope| scope.child("query-runtime"))
    {
        Ok(scope) => scope,
        Err(error) => {
            return finish_writer_fenced_v2(
                startup,
                workspace,
                false,
                Some(DaemonError::Config(format!(
                    "workspace task hierarchy: {error}"
                ))),
            )
            .await;
        }
    };
    let coordinator = match QueryCoordinator::try_new_in_scope(
        policy,
        hello.daemon_generation,
        cursor_secret,
        journal,
        observed_at,
        query_task_scope,
        workspace.resources().budget().clone(),
    ) {
        Ok(coordinator) => Arc::new(coordinator),
        Err(error) => {
            return finish_writer_fenced_v2(
                startup,
                workspace,
                false,
                Some(DaemonError::Config(format!(
                    "query coordinator recovery: {error}"
                ))),
            )
            .await;
        }
    };
    let sessions = match LaunchGrantAuthority::try_new(
        hello.daemon_generation,
        hello.supervisor_generation,
        4_096,
        4_096,
    ) {
        Ok(authority) => Arc::new(authority),
        Err(error) => {
            return finish_writer_fenced_v2(
                startup,
                workspace,
                false,
                Some(DaemonError::Config(format!("session authority: {error}"))),
            )
            .await;
        }
    };
    let results = match StreamedResultRegistry::try_new(
        crate::rpc::MAX_PAYLOAD_CHUNK_BYTES,
        workspace.resources().budget().clone(),
    ) {
        Ok(registry) => Arc::new(registry),
        Err(error) => {
            return finish_writer_fenced_v2(
                startup,
                workspace,
                false,
                Some(DaemonError::Config(format!(
                    "streamed result registry: {error}"
                ))),
            )
            .await;
        }
    };
    match crate::operational_store::OperationalReaderFactory::for_database(&operational_database) {
        Ok(reader) => results.install_source_disclosure_reader(reader),
        Err(error) => {
            return finish_writer_fenced_v2(
                startup,
                workspace,
                false,
                Some(DaemonError::Config(format!(
                    "source disclosure reader: {error}"
                ))),
            )
            .await;
        }
    }
    results.install_processing_reader(crate::fabric::processing_status::ProcessingPageReader::new(
        workspace.resources().clone(),
        daemon_task_scope.clone(),
    ));
    let backend = Arc::new(ProgrammaticSemanticQueryBackend::new(
        Arc::clone(&startup.release),
        Arc::clone(&startup.workspace_slots),
        Arc::clone(&startup.lifecycle),
        package_builder,
    ));
    let daemon_instance_id = match daemon_instance_id(&startup.config) {
        Ok(value) => value,
        Err(error) => {
            return finish_writer_fenced_v2(startup, workspace, false, Some(error)).await;
        }
    };
    let query_service = ProductionQueryService::try_new(
        Arc::clone(&startup.release),
        backend,
        Arc::clone(&startup.lifecycle),
        Arc::clone(&startup.workspace_slots),
        Arc::clone(&coordinator),
        Arc::clone(&sessions),
        Arc::clone(&results),
        daemon_instance_id,
    )?;
    let (query_listener, mut query_socket) = match OwnedUnixSocket::bind(
        &startup.config.static_config.runtime_root,
        &startup.config.static_config.query_socket_endpoint,
        hello.daemon_generation,
    ) {
        Ok(value) => value,
        Err(error) => {
            return finish_writer_fenced_v2(startup, workspace, false, Some(error.into())).await;
        }
    };
    if let Err(error) = startup.lifecycle.advance(
        ProductionLifecyclePhase::WorkspaceInstalledClosed,
        ProductionLifecyclePhase::EndpointsBoundBootstrapping,
    ) {
        let _ = query_socket.retire();
        return finish_writer_fenced_v2(startup, workspace, false, Some(error.into())).await;
    }
    let exact_workspace = startup
        .workspace_slots
        .slot(workspace_id)
        .and_then(|slot| slot.lease().ok());
    if exact_workspace
        .as_ref()
        .map(|lease| lease.workspace().selection().epoch_id())
        != Some(workspace.selected_epoch())
    {
        let _ = query_socket.retire();
        return finish_writer_fenced_v2(
            startup,
            workspace,
            false,
            Some(DaemonError::Config(
                "installed workspace differs from command-selected epoch".into(),
            )),
        )
        .await;
    }
    drop(exact_workspace);
    if let Err(error) = startup
        .lifecycle
        .advance(
            ProductionLifecyclePhase::EndpointsBoundBootstrapping,
            ProductionLifecyclePhase::SoleTargetAuthorityObserved,
        )
        .and_then(|_| {
            startup.lifecycle.advance(
                ProductionLifecyclePhase::SoleTargetAuthorityObserved,
                ProductionLifecyclePhase::SoleTargetAuthorityCommitted,
            )
        })
        .and_then(|_| {
            startup.lifecycle.advance(
                ProductionLifecyclePhase::SoleTargetAuthorityCommitted,
                ProductionLifecyclePhase::Ready,
            )
        })
    {
        let _ = query_socket.retire();
        return finish_writer_fenced_v2(startup, workspace, false, Some(error.into())).await;
    }
    let allowed_uid = rustix::process::geteuid().as_raw();
    let incoming =
        UnixListenerStream::new(query_listener).filter_map(move |accepted| match accepted {
            Ok(stream) => AuthorizedUnixStream::authenticate(stream, allowed_uid)
                .ok()
                .map(Ok),
            Err(error) => Some(Err(error)),
        });
    let query_server = CpgQueryServiceServer::new(query_service)
        .max_decoding_message_size(crate::rpc::MAX_CONTROL_MESSAGE_BYTES)
        .max_encoding_message_size(crate::rpc::MAX_CONTROL_MESSAGE_BYTES);
    let (health_reporter, health_server) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<CpgQueryServiceServer<ProductionQueryService<ProgrammaticSemanticQueryBackend>>>()
        .await;
    let (query_shutdown_tx, mut query_shutdown_rx) = watch::channel(false);
    let query_future = async move {
        Server::builder()
            .max_concurrent_streams(Some(crate::rpc::MAX_QUERY_TRANSPORT_STREAMS))
            .add_service(health_server)
            .add_service(query_server)
            .serve_with_incoming_shutdown(incoming, async move {
                while !*query_shutdown_rx.borrow() {
                    if query_shutdown_rx.changed().await.is_err() {
                        break;
                    }
                }
            })
            .await
    };
    let query_owner = async {
        let reservation = workspace
            .resources()
            .budget()
            .try_reserve(
                crate::resource_budget::ResourceClass::Control,
                crate::resource_budget::ResourceAmounts {
                    running_jobs: 1,
                    memory_bytes: 64 * 1024,
                    ..crate::resource_budget::ResourceAmounts::default()
                },
            )
            .map_err(|error| DaemonError::Config(error.to_string()))?;
        daemon_task_scope
            .child_control("query-server")
            .map_err(|error| DaemonError::Config(error.to_string()))?
            .spawn_async_owned(
                "transport",
                crate::cancellation::TaskCancellationMode::AbortableAsync,
                reservation,
                query_future,
            )
            .await
            .map_err(|error| DaemonError::Config(error.to_string()))
    }
    .await;
    let query_owner = match query_owner {
        Ok(owner) => owner,
        Err(error) => {
            let _ = query_socket.retire();
            return finish_writer_fenced_v2(startup, workspace, false, Some(error)).await;
        }
    };
    let mut query_task = Box::pin(query_owner.wait());
    if matches!(
        startup
            .config
            .static_config
            .activation_startup_assurance_fault,
        Some(ActivationStartupAssuranceFault::ExitBeforeReadyAcknowledgement)
    ) {
        let _ = query_shutdown_tx.send(true);
        let _ = query_task.await;
        let _ = query_socket.retire();
        return finish_writer_fenced_v2(
            startup,
            workspace,
            false,
            Some(DaemonError::Serving(
                "injected pre-ready child exit before authenticated acknowledgement".into(),
            )),
        )
        .await;
    }
    if let Err(error) = acknowledge_control(
        &mut control,
        &control_state,
        &hello_header,
        &hello.request_id,
        true,
        "DAEMON_CONTROL_READY",
    )
    .await
    {
        let _ = query_shutdown_tx.send(true);
        let _ = query_task.await;
        let _ = query_socket.retire();
        return finish_writer_fenced_v2(startup, workspace, false, Some(error.into())).await;
    }
    let (serve_result, query_already_joined) = {
        let control_future = serve_daemon_control(
            &mut control,
            &mut control_state,
            Arc::clone(&sessions),
            Arc::clone(&startup.lifecycle),
            Arc::clone(&coordinator),
        );
        tokio::pin!(control_future);
        tokio::select! {
            result = &mut control_future => (result, false),
            result = &mut query_task => {
                let result = result
                    .map_err(|error| DaemonError::Serving(format!("query server join: {error}")))
                    .and_then(|result| {
                        result.map_err(|error| DaemonError::Serving(format!("query server: {error}")))
                    });
                (result.map(|()| false), true)
            }
        }
    };
    let _ = query_shutdown_tx.send(true);
    let cleanup = async {
        let query_join = if query_already_joined {
            Ok(())
        } else {
            query_task
                .await
                .map_err(|error| DaemonError::Serving(format!("query server join: {error}")))
                .and_then(|result| {
                    result.map_err(|error| DaemonError::Serving(format!("query server: {error}")))
                })
        };
        let query_retire = query_socket.retire().map_err(DaemonError::from);
        // Source cancellation may still need an activation append/readback. Drain those
        // producers while the sibling native control scopes remain admitted, then retire
        // the daemon lifetime. A failed producer join retains its native owners and fence.
        workspace
            .drain_operations()
            .await
            .map_err(|error| DaemonError::Serving(format!("workspace drain: {error}")))?;
        let task_drain = daemon_task_scope
            .cancel_and_join(WORKSPACE_OPERATION_DRAIN_TIMEOUT)
            .await
            .map_err(|error| DaemonError::Serving(format!("daemon task drain: {error}")));
        query_join.and(query_retire).and(task_drain)
    };
    let primary = match serve_result {
        Ok(true) => await_shutdown_after_drain(&mut control, &mut control_state, cleanup)
            .await
            .map(|()| true),
        Ok(false) => cleanup.await.and(Err(DaemonError::Serving(
            "controlled daemon exited without an authenticated drain".into(),
        ))),
        Err(primary) => match cleanup.await {
            Ok(()) => Err(primary),
            Err(cleanup) => Err(DaemonError::Serving(format!(
                "{primary}; controlled cleanup also failed: {cleanup}"
            ))),
        },
    };
    match primary {
        Ok(drained) => finish_writer_fenced_v2(startup, workspace, drained, None).await,
        Err(error) => finish_writer_fenced_v2(startup, workspace, false, Some(error)).await,
    }
}

async fn serve_daemon_control(
    stream: &mut UnixStream,
    state: &mut DaemonControlReadState,
    sessions: Arc<LaunchGrantAuthority>,
    lifecycle: Arc<LifecycleAuthority>,
    coordinator: Arc<QueryCoordinator>,
) -> Result<bool, DaemonError> {
    loop {
        let accepted = read_daemon_control(stream, state).await?;
        let header = accepted.header;
        let request = accepted.request;
        let request_id = request.request_id().to_owned();
        let outcome = match request {
            DaemonControlRequest::Hello(_) => Err("CONTROL_HELLO_REPLAY".to_owned()),
            DaemonControlRequest::RegisterLaunchGrant { grant, .. } => sessions
                .register(grant)
                .await
                .map_err(|error| format!("GRANT_REGISTRATION_REJECTED:{error}")),
            DaemonControlRequest::RevokeLaunch {
                grant_id,
                grant_digest,
                adapter_pid,
                adapter_start_identity,
                ..
            } => match sessions
                .revoke_launch(
                    &grant_id,
                    grant_digest,
                    adapter_pid,
                    adapter_start_identity.as_deref(),
                )
                .await
            {
                Ok(()) | Err(SessionAuthorityError::UnknownGrant) => Ok(()),
                Err(error) => Err(format!("LAUNCH_REVOCATION_REJECTED:{error}")),
            },
            DaemonControlRequest::RevokePrincipal {
                principal_id,
                revocation_generation,
                ..
            } => sessions
                .revoke_principal(
                    crate::fabric::command::PrincipalId::from_bytes(principal_id),
                    revocation_generation,
                )
                .await
                .map_err(|error| format!("PRINCIPAL_REVOCATION_REJECTED:{error}")),
            DaemonControlRequest::AdvanceGeneration {
                daemon_generation,
                supervisor_generation,
                ..
            } => sessions
                .advance_generation(daemon_generation, supervisor_generation)
                .await
                .map_err(|error| format!("GENERATION_ADVANCE_REJECTED:{error}")),
            DaemonControlRequest::Drain { .. } => {
                lifecycle.begin_draining()?;
                coordinator
                    .drain_owned_tasks(Duration::from_secs(10))
                    .await
                    .map_err(|error| {
                        DaemonError::Serving(format!("accepted query drain: {error}"))
                    })?;
                acknowledge_control(stream, state, &header, &request_id, true, "DRAIN_ACCEPTED")
                    .await?;
                return Ok(true);
            }
            DaemonControlRequest::Shutdown { .. } => {
                acknowledge_control(stream, state, &header, &request_id, false, "DRAIN_REQUIRED")
                    .await?;
                continue;
            }
        };
        let (accepted, code) = match outcome {
            Ok(()) => (true, "CONTROL_APPLIED".to_owned()),
            Err(code) => (false, code),
        };
        acknowledge_control(stream, state, &header, &request_id, accepted, &code).await?;
    }
}

/// Authenticate the next shutdown command while cleanup advances, then acknowledge its join.
pub(crate) async fn await_shutdown_after_drain(
    stream: &mut UnixStream,
    state: &mut DaemonControlReadState,
    cleanup: impl std::future::Future<Output = Result<(), DaemonError>>,
) -> Result<(), DaemonError> {
    let receive = async {
        loop {
            let accepted = read_daemon_control(stream, state).await?;
            if matches!(accepted.request, DaemonControlRequest::Shutdown { .. }) {
                return Ok::<_, DaemonError>(accepted);
            }
            acknowledge_control(
                stream,
                state,
                &accepted.header,
                accepted.request.request_id(),
                false,
                "DAEMON_DRAINING",
            )
            .await?;
        }
    };
    // The record's short authentication lifetime governs admission, not the duration of
    // accepted workspace cleanup. join! also keeps cleanup owned when receive rejects a record.
    let (received, cleaned) = tokio::join!(receive, cleanup);
    let accepted = match (received, cleaned) {
        (Ok(accepted), Ok(())) => accepted,
        (Err(control), Err(cleanup)) => {
            return Err(DaemonError::Serving(format!(
                "{control}; controlled cleanup also failed: {cleanup}"
            )));
        }
        (Err(error), Ok(())) | (Ok(_), Err(error)) => return Err(error),
    };
    acknowledge_control(
        stream,
        state,
        &accepted.header,
        accepted.request.request_id(),
        true,
        "SHUTDOWN_ACCEPTED",
    )
    .await?;
    Ok(())
}

fn daemon_random32() -> Result<[u8; 32], DaemonError> {
    let first = crate::identity::random_registration_nonce()?;
    let second = crate::identity::random_registration_nonce()?;
    let mut value = [0_u8; 32];
    value[..16].copy_from_slice(&first);
    value[16..].copy_from_slice(&second);
    Ok(value)
}

fn now_millis() -> Result<u128, DaemonError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|error| DaemonError::Serving(format!("system clock before Unix epoch: {error}")))
}

fn daemon_instance_id(config: &DaemonConfig) -> Result<String, DaemonError> {
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
    Ok(crate::integrity::frame_digest(identity.finalize())[3..35].to_owned())
}

/// Run the sole production daemon route over its inherited supervisor control endpoint.
pub async fn serve_controlled(
    config: DaemonConfig,
    mut control: UnixStream,
) -> Result<DaemonExit, DaemonError> {
    let (hello, hello_header, control_state) = accept_control_hello(&mut control).await?;
    ProductionDaemonFactory::compile()?
        .build(config)?
        .run_controlled(control, hello, hello_header, control_state)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct AcceptPublicationIntent;

    #[async_trait::async_trait]
    impl crate::fabric::streamed_result_package::ResultPublicationIntentRecorder
        for AcceptPublicationIntent
    {
        async fn record_publication_intent(
            &self,
            _intent: crate::fabric::streamed_result_package::PendingResultObjectSet,
        ) -> Result<(), crate::fabric::streamed_result_package::ResultPublicationIntentError>
        {
            Ok(())
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

    #[test]
    fn source_watch_configuration_preserves_native_default_and_requires_explicit_polling() {
        let directory = tempfile::tempdir().unwrap();
        let mut selected = config(directory.path());
        let native = serde_json::to_string(&selected).unwrap();
        assert!(!native.contains("source_watch_profile"));
        assert_eq!(
            serde_json::from_str::<DaemonConfig>(&native).unwrap(),
            selected
        );
        selected.static_config.source_watch_profile = SourceWatchProfile::Poll;
        let poll = serde_json::to_string(&selected).unwrap();
        assert_eq!(
            serde_json::from_str::<DaemonConfig>(&poll).unwrap(),
            selected
        );
        assert!(
            serde_json::from_str::<DaemonConfig>(&poll.replace("\"poll\"", "\"automatic\""))
                .is_err()
        );
    }

    fn config(root: &Path) -> DaemonConfig {
        private_directory(&root.join("config")).unwrap();
        DaemonConfig {
            static_config: StaticConfig {
                state_root: root.join("state"),
                runtime_root: root.join("runtime"),
                config_root: root.join("config"),
                query_socket_endpoint: root.join("runtime/query.sock"),
                operational_database: PathBuf::from("operational.sqlite3"),
                sandbox_policy: "required-for-untrusted".to_owned(),
                hard_limit_profile: "daemon-default-v1".to_owned(),
                supported_platform_profile: "local-workstation-v1".to_owned(),
                source_watch_profile: SourceWatchProfile::Native,
                activation_startup_assurance_fault: None,
            },
            reloadable: ReloadableConfig {
                log_level: "info".to_owned(),
                telemetry_sampling: 0.1,
                soft_query_quota: 4,
                maintenance_schedule: "daily-idle".to_owned(),
            },
        }
    }

    fn add_operational_workspace(
        config: &DaemonConfig,
        workspace_root: &Path,
    ) -> crate::workspace_registry::WorkspaceRecord {
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

    fn write_config(root: &Path, extra: &str) -> PathBuf {
        private_directory(&root.join("config")).unwrap();
        let path = root.join("config/codefabric.toml");
        let source = format!(
            r#"
[static_config]
state_root = {state:?}
runtime_root = {runtime:?}
config_root = {config:?}
query_socket_endpoint = {query_socket:?}
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

        let instance_id = daemon_instance_id(&config).unwrap();
        assert_eq!(instance_id.len(), 32);
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
        let startup = ProductionStartupCoordinator::new(
            config.clone(),
            Arc::new(crate::fabric::production_kernel::compile_test_semantic_release()),
        );
        let lifecycle = Arc::clone(&startup.lifecycle);
        let workspace_slots = Arc::clone(&startup.workspace_slots);

        assert!(matches!(
            startup
                .acquire_daemon_lease()
                .unwrap()
                .acquire_workspace_writers(),
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

    #[tokio::test]
    async fn wp47_fresh_production_workspace_prepares_exact_and_guarded_semantic_requests() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        private_directory(&config.static_config.runtime_root).unwrap();
        let workspace_root = root.path().join("workspace");
        let record = add_operational_workspace(&config, &workspace_root);
        fs::write(
            workspace_root.join("sample.py"),
            b"def answer(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

        let mut startup = ProductionStartupCoordinator::new(
            config.clone(),
            Arc::new(crate::fabric::production_kernel::compile_test_semantic_release()),
        )
        .acquire_daemon_lease()
        .unwrap()
        .acquire_workspace_writers()
        .unwrap();
        let workspace_id = crate::fabric::command::WorkspaceId::from_bytes(record.workspace_id);
        let slot = startup.workspace_slots.slot(workspace_id).unwrap();
        let writer_lease = startup.phase.writer_leases.swap_remove(0);
        let operational_database = config
            .static_config
            .state_root
            .join(&config.static_config.operational_database);
        let workspace = start_production_workspace(
            &config.static_config.state_root,
            &operational_database,
            &record,
            Arc::clone(&startup.release),
            Arc::clone(&slot),
            Arc::clone(&startup.phase.generation_store),
            writer_lease,
            None,
            crate::fabric::workspace_resources::ProductionWorkspaceResources::try_new(
                &crate::resource_budget::ResourceBudget::try_process(
                    [91; 16],
                    crate::fabric::workspace_resources::local_resource_policy(),
                )
                .unwrap(),
                workspace_id,
                &config.static_config.state_root,
            )
            .unwrap(),
            SourceWatchProfile::Native,
            StructuredCancellationScope::try_root_with_control_reserve(
                "daemon",
                std::num::NonZeroUsize::new(64).unwrap(),
                std::num::NonZeroUsize::new(8).unwrap(),
            )
            .unwrap(),
        )
        .await
        .unwrap();

        let workspace_lease = slot.lease().unwrap();
        let active = workspace_lease.workspace();
        let runtime = active.runtime();
        let authority = runtime.query_authority();
        let ports = active.query_ports();
        assert_eq!(
            *authority.activation_pins().application_release.as_bytes(),
            ports.application_release(),
            "fresh workspace query ports differ from activation release"
        );
        assert_eq!(
            authority.epoch_id(),
            active.selection().epoch_id(),
            "fresh workspace query authority differs from durable selection"
        );

        let request = |identity: &str, looking_for: &str| {
            crate::semantic_query_contract::parse_request(
                &serde_json::to_vec(&serde_json::json!({
                    "specification": "composable semantic CPG fact query",
                    "version": "2.0",
                    "semantic_request_id": identity,
                    "scope": {"workspace_id": record.public_id()},
                    "freshness": {"policy": "require_semantic_current", "deadline_ms": 180_000},
                    "queries": [{
                        "request": "find code entities",
                        "query_id": "q1",
                        "looking_for": looking_for,
                        "within": [],
                        "where": [],
                        "return": {"limit": {"maximum_results": 32}}
                    }]
                }))
                .unwrap(),
            )
            .unwrap()
        };
        let exact = request("request:wp47-direct-exact", "Python function declarations");
        let guarded = request("request:wp47-direct-guarded", "functions");
        drop(workspace_lease);

        let recovered = if workspace.fresh_activation() {
            ProductionLifecyclePhase::GenesisRequired
        } else {
            ProductionLifecyclePhase::SelectedEpochRecovered
        };
        startup
            .lifecycle
            .advance(
                ProductionLifecyclePhase::WriterFenced,
                ProductionLifecyclePhase::CommandRecovered,
            )
            .and_then(|_| {
                startup
                    .lifecycle
                    .advance(ProductionLifecyclePhase::CommandRecovered, recovered)
            })
            .and_then(|_| {
                startup
                    .lifecycle
                    .advance(recovered, ProductionLifecyclePhase::EpochBuiltAndProved)
            })
            .and_then(|_| {
                startup.lifecycle.advance(
                    ProductionLifecyclePhase::EpochBuiltAndProved,
                    ProductionLifecyclePhase::WorkspaceInstalledClosed,
                )
            })
            .and_then(|_| {
                startup.lifecycle.advance(
                    ProductionLifecyclePhase::WorkspaceInstalledClosed,
                    ProductionLifecyclePhase::EndpointsBoundBootstrapping,
                )
            })
            .and_then(|_| {
                startup.lifecycle.advance(
                    ProductionLifecyclePhase::EndpointsBoundBootstrapping,
                    ProductionLifecyclePhase::SoleTargetAuthorityObserved,
                )
            })
            .and_then(|_| {
                startup.lifecycle.advance(
                    ProductionLifecyclePhase::SoleTargetAuthorityObserved,
                    ProductionLifecyclePhase::SoleTargetAuthorityCommitted,
                )
            })
            .and_then(|_| {
                startup.lifecycle.advance(
                    ProductionLifecyclePhase::SoleTargetAuthorityCommitted,
                    ProductionLifecyclePhase::Ready,
                )
            })
            .unwrap();

        let result_root = config.static_config.state_root.join("query-results");
        private_directory(&result_root).unwrap();
        let object_store =
            Arc::new(object_store::local::LocalFileSystem::new_with_prefix(&result_root).unwrap())
                as Arc<dyn object_store::ObjectStore>;
        let package_builder = StreamedResultPackageBuilder::new(
            Arc::new(ObjectStoreResultSink::new(object_store)),
            StreamedResultPackageLimits::try_new(
                64,
                4_096,
                8_192,
                crate::rpc::MAX_PAYLOAD_CHUNK_BYTES,
                10_000_000,
                512 * 1024 * 1024,
                4 * 1024 * 1024,
                4_096,
                4 * 1024 * 1024,
            )
            .unwrap(),
            workspace.resources().budget().clone(),
        );
        let backend = ProgrammaticSemanticQueryBackend::new(
            Arc::clone(&startup.release),
            Arc::clone(&startup.workspace_slots),
            Arc::clone(&startup.lifecycle),
            package_builder,
        );
        let exact_preparation =
            crate::query_backend::SemanticQueryBackend::prepare_execution_request(
                &backend,
                &exact,
                &[],
            )
            .expect("fresh production backend prepares an exact catalog selection");
        let crate::query_backend::SemanticExecutionPreparation::Ready(exact_resolved) =
            exact_preparation
        else {
            panic!("exact catalog selection unexpectedly requires guarded input");
        };
        let guarded_preparation =
            crate::query_backend::SemanticQueryBackend::prepare_execution_request(
                &backend,
                &guarded,
                &[],
            )
            .expect("fresh production backend prepares a guarded catalog selection");
        assert!(matches!(
            guarded_preparation,
            crate::query_backend::SemanticExecutionPreparation::InputRequired(ref requirements)
                if requirements.len() == 1
        ));

        let prepared = crate::query_backend::SemanticQueryBackend::admit_fresh_execution_request(
            &backend,
            exact_resolved,
        )
        .await
        .expect("fresh production backend admits the exact request after semantic convergence");
        assert_eq!(
            prepared.snapshot().freshness_state,
            crate::freshness::FreshnessState::Current
        );
        let principal_id = crate::fabric::command::PrincipalId::from_bytes([0x47; 16]);
        let execution = crate::fabric::QueryExecutionContext {
            execution_id: "query:wp47-direct-exact".to_owned(),
            semantic_request_id: exact.request.semantic_request_id.clone(),
            request_correlation_id: "correlation:wp47-direct-exact".to_owned(),
        };
        let context = crate::query_backend::SemanticBackendExecutionContext::new(
            execution.clone(),
            "47".repeat(16),
            record.public_id(),
            crate::fabric::published_arrow_result::PublishedResultOwner::new(
                workspace_id,
                principal_id,
            ),
            crate::fabric::arrow_result_resource::QueryExecutionPin::from_bytes([0x47; 32]),
            LeaseId::from_bytes([0x48; 16]),
            crate::fabric::published_arrow_result::OpaqueResultLeaseToken::try_from_bytes(
                [0x49; 32],
            )
            .unwrap(),
            Arc::new(AcceptPublicationIntent),
            std::time::Instant::now() + std::time::Duration::from_secs(60),
        );
        let artifacts = crate::fabric::QueryExecutionArtifactAccumulator::new(execution);
        match crate::query_backend::SemanticQueryBackend::execute(
            &backend,
            prepared,
            crate::cancellation::Cancellation::with_check_interval(1),
            context,
            artifacts,
        )
        .await
        {
            crate::query_backend::SemanticBackendOutcome::PublishedArrow(success) => {
                assert!(success.publication().package().manifest().total_rows > 0);
            }
            crate::query_backend::SemanticBackendOutcome::Failed { error, evidence }
            | crate::query_backend::SemanticBackendOutcome::Cancelled { error, evidence } => {
                panic!(
                    "fresh production execution failed at {:?}: {error}; evidence={evidence:?}",
                    evidence.failing_stage
                );
            }
        }

        workspace.shutdown().await.unwrap();
        finish_writer_fenced(startup, false, None).unwrap();
    }
}
