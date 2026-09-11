//! Stable-daemon adapter for the isolated Pyrefly sidecar.
//!
//! Only application-owned Arrow batches and correlation DTOs cross this module. No Pyrefly
//! library type is linked into or exposed by the stable daemon.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use arrow_array::{Array as _, FixedSizeBinaryArray, RecordBatch, StringArray, UInt64Array};
use hyper_util::rt::TokioIo;
use prost::Message as _;
use serde::{Deserialize, Serialize};
use tokio::net::UnixStream;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::{Channel, Endpoint};
use tower::service_fn;

use crate::provider_contracts::{
    ProviderContractError, ProviderCoverage, ProviderCoverageState, ProviderGap, ProviderJob,
    ProviderLane, ProviderRelationOutput, ProviderRunEvidenceSpec, ProviderRunResult,
    ProviderTerminalStatus, ProviderTrustOutcome, ProviderTrustPosture, ProviderUnknownCause,
};
use crate::provider_sandbox::{
    ProviderProcessGroupChild, ProviderTrustProfile, SandboxCapabilityMatrix, SandboxMechanism,
};
use crate::relation_ipc::{
    FlowControlAck, FrameHeader, RelationIpcAssembler, RelationIpcFrame, RelationIpcLimits,
    StreamId,
};
use crate::relation_ipc_wire::{
    decode_relation_frame, encode_relation_frame, relation_stream_contract,
};
use crate::rpc::generated::codefabric::provider::v1::{
    BlobReference, CancelAcknowledgementState, ProviderRunState as WireProviderRunState,
    SourceSnapshotLease,
};
use crate::rpc::generated::codefabric::pyrefly::v1::analyze_command::Command;
use crate::rpc::generated::codefabric::pyrefly::v1::analyze_event::Event;
use crate::rpc::generated::codefabric::pyrefly::v1::pyrefly_sidecar_client::PyreflySidecarClient as WireClient;
use crate::rpc::generated::codefabric::pyrefly::v1::{
    AnalyzeCommand, AnalyzeEventHeader, AnalyzeInventoryChunk, AnalyzeInventoryEnd,
    AnalyzeModulesRequest, CancelRunRequest, Hello, ModuleRequest, OpenContextRequest,
};

#[path = "pyrefly_relation_schema.rs"]
mod relation_schema;

#[path = "pyrefly_inventory_stream.rs"]
pub(crate) mod inventory_stream;
use inventory_stream::{
    MAX_MODULES_PER_RUN, MAX_SOURCE_BYTES_PER_MODULE, MAX_SOURCE_BYTES_PER_RUN, MODULES_PER_CHUNK,
};

pub use relation_schema::PyreflyRelation;
pub(crate) use relation_schema::schema_bundle_digest;
use relation_schema::schema_digests;

const PYREFLY_SOURCE_DIGEST: &str =
    "b3:1b9e72144644d1b3df0bdca564496566238543dfb7f576980a8408714327fc3e";
const REQUIRED_FEATURE_BITS: u64 = (1_u64 << 17) | (1_u64 << 32) | (1_u64 << 34);
const OPTIONAL_FEATURE_BITS: u64 = 1_u64 << 33;
const MAX_UNACKNOWLEDGED_BYTES: u64 = 16 * 1024 * 1024;
const MAX_RELATION_ROWS: u64 = 1_000_000;
const MAX_TOTAL_RELATION_BYTES: usize = 256 * 1024 * 1024;
const RESOURCE_PROFILE_ID: &str = "sidecar-semantic-standard";
const TRUST_PROFILE: &str = "UNTRUSTED_SANDBOXED";

/// One immutable Python module admitted to the sidecar.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PyreflyModuleInput {
    pub module_id: String,
    pub module_name: String,
    pub file_id: String,
    pub source_blob_path: PathBuf,
    /// Absolute location of the same digest-checked blob inside the provider's mount view.
    pub provider_source_blob_path: PathBuf,
    pub content_digest: String,
}

/// Operational immutable inputs for one release-prepared Pyrefly provider job.
///
/// Categorical provider, protocol, build, source, context, run, schema, policy, resource, and
/// cancellation authority remains in [`ProviderJob`]. This value carries only workspace source
/// material that cannot be represented by a single provider-contract source binding.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PyreflyWorkspaceInput {
    pub workspace_id: String,
    pub canonical_workspace_id: [u8; 16],
    pub context_manifest: Vec<u8>,
    pub source_snapshot_lease_id: String,
    pub modules: Vec<PyreflyModuleInput>,
    /// Scheduling hints selected from the job's changed paths; `modules` remains complete.
    pub changed_module_ids: Vec<String>,
}

/// Transport-local request derived exclusively from a provider job plus operational source input.
#[derive(Clone, Debug)]
struct PyreflyRunRequest {
    resource_budget: crate::resource_budget::ResourceBudget,
    max_input_bytes: u64,
    max_output_bytes: u64,
    provider_run_id: String,
    workspace_id: String,
    analysis_context_id: String,
    canonical_workspace_id: [u8; 16],
    canonical_analysis_context_id: [u8; 16],
    source_generation: u64,
    context_manifest: Vec<u8>,
    source_snapshot_lease_id: String,
    source_manifest_digest: String,
    modules: Vec<PyreflyModuleInput>,
    changed_module_ids: Vec<String>,
    expected_removed_module_ids: Vec<String>,
    requested_capability_codes: Vec<u32>,
    deadline_unix_ms: i64,
    sandbox_profile_digest: String,
    output_schema_bundle_digest: String,
}

/// One fully verified application-owned module observation.
#[derive(Clone, Debug)]
pub struct AcceptedPyreflyModule {
    pub module_id: String,
    pub module_name: String,
    pub canonical_file_id: [u8; 16],
    pub source_bytes: crate::resource_budget::ChargedSlice<u8>,
    pub module_digest: String,
    /// Complete target-route application-owned relation set.
    pub relations: crate::resource_budget::ChargedSlice<AcceptedPyreflyRelation>,
}

/// One independently schema-validated relation-scoped Arrow stream.
#[derive(Clone, Debug)]
pub struct AcceptedPyreflyRelation {
    pub relation: PyreflyRelation,
    pub arrow_ipc: crate::resource_budget::ChargedSlice<u8>,
    pub batch: RecordBatch,
    pub schema_digest: String,
    pub arrow_ipc_digest: String,
    pub row_count: u64,
}

struct PendingPyreflyModule {
    module_id: String,
    module_name: String,
    canonical_file_id: [u8; 16],
    source_bytes: crate::resource_budget::ChargedSlice<u8>,
    relations: BTreeMap<PyreflyRelation, AcceptedPyreflyRelation>,
    relation_by_stream: BTreeMap<StreamId, PyreflyRelation>,
    next_ack_sequence: BTreeMap<StreamId, u64>,
    assembler: RelationIpcAssembler,
}

/// Terminal sidecar stream admitted for reconciliation.
#[derive(Clone, Debug)]
pub struct AcceptedPyreflyRun {
    pub provider_run_id: String,
    pub workspace_id: String,
    pub analysis_context_id: String,
    pub canonical_workspace_id: [u8; 16],
    pub canonical_analysis_context_id: [u8; 16],
    pub source_generation: u64,
    pub modules: crate::resource_budget::ChargedSlice<AcceptedPyreflyModule>,
    pub capability_codes: crate::resource_budget::ChargedSlice<u32>,
    pub overall_digest: String,
    pub rechecked_module_ids: crate::resource_budget::ChargedSlice<String>,
    pub removed_module_ids: crate::resource_budget::ChargedSlice<String>,
    pub sandbox_profile_digest: String,
    pub trust_profile: String,
}

/// Closed sidecar/transport/Arrow validation failures.
#[derive(Debug, thiserror::Error)]
pub enum PyreflyServiceError {
    #[error("Pyrefly request is invalid: {0}")]
    Invalid(String),
    #[error("Pyrefly input exceeds its configured {0} bound")]
    InputLimit(&'static str),
    #[error("Pyrefly sidecar transport failed: {0}")]
    Transport(String),
    #[error("Pyrefly sidecar protocol failed: {0}")]
    Protocol(String),
    #[error("Pyrefly observation Arrow IPC failed: {0}")]
    Arrow(String),
    #[error("Pyrefly run was cooperatively cancelled")]
    Cancelled,
    #[error("Pyrefly run exceeded its release-owned deadline")]
    TimedOut,
    #[error("Pyrefly workspace sidecar process could not be drained and joined: {0}")]
    ProcessTermination(String),
    #[error("the exact untrusted Pyrefly containment profile is unavailable")]
    TrustUnavailable,
    #[error("selected Pyrefly context inputs lack proved bundle or root authority")]
    PreparationUnavailable,
    #[error("Pyrefly source read failed at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Pyrefly application provider contract failed: {0}")]
    Contract(#[from] ProviderContractError),
}

/// Exact incomplete provider outcome returned instead of an accepted Pyrefly batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PyreflyRunGap {
    ResourceLimit,
    Cancelled,
    TimedOut,
    ProcessFailure,
    TrustUnavailable,
    PreparationUnavailable,
}

impl PyreflyServiceError {
    /// Translate process lifecycle failures into the exact provider-lane gap category.
    #[must_use]
    pub const fn run_gap(&self) -> Option<PyreflyRunGap> {
        match self {
            Self::InputLimit(_) => Some(PyreflyRunGap::ResourceLimit),
            Self::Cancelled => Some(PyreflyRunGap::Cancelled),
            Self::TimedOut => Some(PyreflyRunGap::TimedOut),
            Self::ProcessTermination(_) | Self::Transport(_) => Some(PyreflyRunGap::ProcessFailure),
            Self::TrustUnavailable => Some(PyreflyRunGap::TrustUnavailable),
            Self::PreparationUnavailable => Some(PyreflyRunGap::PreparationUnavailable),
            Self::Invalid(_)
            | Self::Protocol(_)
            | Self::Arrow(_)
            | Self::Io { .. }
            | Self::Contract(_) => None,
        }
    }
}

fn context_open_error(status: tonic::Status) -> PyreflyServiceError {
    if status.code() == tonic::Code::FailedPrecondition
        && status.details() == b"codefabric.pyrefly.preparation-unavailable.v1"
    {
        PyreflyServiceError::PreparationUnavailable
    } else {
        PyreflyServiceError::Protocol(status.to_string())
    }
}

/// One Pyrefly execution expressed through the shared application-owned provider-result contract.
#[derive(Clone, Debug)]
pub struct PyreflyProviderRunResult {
    accepted: Option<AcceptedPyreflyRun>,
    result: ProviderRunResult,
}

impl PyreflyProviderRunResult {
    fn try_new(
        job: &ProviderJob,
        accepted: AcceptedPyreflyRun,
    ) -> Result<Self, PyreflyServiceError> {
        let crate::provider_contracts::ProviderSourceSelection::Inventory(inventory) =
            job.source().selection()
        else {
            return Err(PyreflyServiceError::Invalid(
                "Pyrefly result requires a complete selected inventory".to_owned(),
            ));
        };
        let expected_files = inventory.selected_files().collect::<BTreeMap<_, _>>();
        let actual_files = accepted
            .modules
            .iter()
            .map(|module| module.canonical_file_id)
            .collect::<std::collections::BTreeSet<_>>();
        if accepted.provider_run_id != job.run().identity().as_str()
            || accepted.canonical_workspace_id != job.source().workspace_id()
            || accepted.canonical_analysis_context_id != job.context().analysis_context_id()
            || accepted.analysis_context_id != job.context().identity().as_str()
            || accepted.source_generation != job.source().generation()
            || actual_files.len() != accepted.modules.len()
            || actual_files.iter().ne(expected_files.keys())
            || accepted.modules.iter().any(|module| {
                expected_files
                    .get(&module.canonical_file_id)
                    .is_none_or(|digest| blake3::hash(&module.source_bytes).as_bytes() != digest)
                    || job
                        .context()
                        .module_for_file(module.canonical_file_id)
                        .is_none_or(|binding| binding.qualified_name != module.module_name)
            })
        {
            return Err(PyreflyServiceError::Invalid(
                "Pyrefly accepted source/context/run pins differ from the exact job".to_owned(),
            ));
        }
        let module_count = u64::try_from(accepted.modules.len()).unwrap_or(u64::MAX);
        let requested_modules = accepted
            .modules
            .iter()
            .map(|module| module.module_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let rechecked_modules = accepted
            .rechecked_module_ids
            .iter()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        if rechecked_modules.len() != accepted.rechecked_module_ids.len()
            || !rechecked_modules.is_subset(&requested_modules)
        {
            return Err(PyreflyServiceError::Protocol(
                "Pyrefly affected scope is duplicate or outside the requested modules".to_owned(),
            ));
        }

        let mut relations = Vec::with_capacity(job.requests().len());
        let mut coverage = Vec::with_capacity(job.requests().len());
        for request in job.requests() {
            if request.requested_units() != module_count {
                return Err(PyreflyServiceError::Invalid(
                    "Pyrefly requested-unit census differs from the immutable module set"
                        .to_owned(),
                ));
            }
            let relation = pyrefly_relation_for_job(request.relation().as_str())?;
            let mut batches = accepted
                .modules
                .iter()
                .map(|module| {
                    module
                        .relations
                        .iter()
                        .find(|output| output.relation == relation)
                        .map(|output| output.batch.clone())
                        .ok_or_else(|| {
                            PyreflyServiceError::Protocol(format!(
                                "Pyrefly module omitted requested relation {}",
                                relation.relation_id()
                            ))
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if batches.is_empty() {
                // A proved empty selected universe still returns the requested Arrow schema.
                batches.push(RecordBatch::new_empty(Arc::clone(request.schema())));
            }
            relations.push(ProviderRelationOutput::try_new(
                request.relation().clone(),
                request.schema_identity().clone(),
                Arc::clone(request.schema()),
                batches,
                job.resource_budget(),
            )?);
            coverage.push(ProviderCoverage::new(
                request.family().clone(),
                ProviderCoverageState::Complete {
                    completed_units: module_count,
                },
            ));
        }
        let result = ProviderRunResult::try_from_job(
            job,
            ProviderRunEvidenceSpec {
                support: crate::provider_contracts::ProviderRunSupport::conservative(job),
                relations,
                coverage,
                gaps: Vec::new(),
                diagnostics: Vec::new(),
                trust: ProviderTrustOutcome::Trusted,
                terminal: ProviderTerminalStatus::Complete,
            },
        )?;
        Ok(Self {
            accepted: Some(accepted),
            result,
        })
    }

    fn gap(
        job: &ProviderJob,
        cause: ProviderUnknownCause,
        detail: &'static str,
    ) -> Result<Self, PyreflyServiceError> {
        let coverage = job
            .requests()
            .iter()
            .map(|request| {
                ProviderCoverage::new(
                    request.family().clone(),
                    ProviderCoverageState::Unknown {
                        completed_units: 0,
                        cause,
                    },
                )
            })
            .collect();
        let gaps = job
            .requests()
            .iter()
            .map(|request| ProviderGap::try_new(request.family().clone(), cause, detail))
            .collect::<Result<Vec<_>, _>>()?;
        let terminal = match cause {
            ProviderUnknownCause::MissingOutput | ProviderUnknownCause::Unsupported => {
                ProviderTerminalStatus::Unknown
            }
            ProviderUnknownCause::Timeout => ProviderTerminalStatus::TimedOut,
            ProviderUnknownCause::Cancelled => ProviderTerminalStatus::Cancelled,
            ProviderUnknownCause::Corruption => ProviderTerminalStatus::Corrupt,
            ProviderUnknownCause::Oversized => ProviderTerminalStatus::Oversized,
            ProviderUnknownCause::ProviderFailure | ProviderUnknownCause::TrustLoss => {
                ProviderTerminalStatus::Failed
            }
        };
        let result = ProviderRunResult::try_from_job(
            job,
            ProviderRunEvidenceSpec {
                support: crate::provider_contracts::ProviderRunSupport::conservative(job),
                relations: Vec::new(),
                coverage,
                gaps,
                diagnostics: Vec::new(),
                trust: ProviderTrustOutcome::Degraded {
                    detail: Arc::from(detail),
                },
                terminal,
            },
        )?;
        Ok(Self {
            accepted: None,
            result,
        })
    }

    #[must_use]
    pub const fn accepted(&self) -> Option<&AcceptedPyreflyRun> {
        self.accepted.as_ref()
    }

    #[must_use]
    pub const fn result(&self) -> &ProviderRunResult {
        &self.result
    }
}

/// Exact compatibility identity for one long-lived native Pyrefly context.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PyreflyContextCompatibility {
    workspace_id: String,
    analysis_context_id: String,
    semantic_environment_id: [u8; 32],
    manifest_digest: String,
    provider_build: String,
    protocol: String,
    sandbox_profile_digest: String,
}

/// Provider-local checker state is not the durable CPG replacement predecessor.
fn checker_removals(
    previous: &std::collections::BTreeSet<String>,
    current: &[PyreflyModuleInput],
) -> Vec<String> {
    let current = current
        .iter()
        .map(|module| module.module_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    previous
        .iter()
        .filter(|module| !current.contains(module.as_str()))
        .cloned()
        .collect()
}

/// One contained, supervisor-owned Pyrefly process for a compatible workspace/context.
///
/// The process-group child has no public constructor and therefore cannot be substituted with an
/// arbitrary PID. Successful generations and cooperatively cancelled runs retain the same channel
/// and native `Query` state. Context drift closes the prior context before reconstruction. Crash,
/// corruption, trust loss, or unresponsive cancellation consumes the child and requires a clean
/// replacement from immutable inputs.
pub(crate) struct SupervisedPyreflyWorkspace {
    process: Arc<PyreflyProcessState>,
    socket: PathBuf,
    cancellation_grace: Duration,
    maximum_wall_time: Duration,
    analysis_started: Arc<AtomicBool>,
    sandbox_profile_digest: String,
    client: Option<WireClient<Channel>>,
    compatibility: Option<PyreflyContextCompatibility>,
    context_handle: Option<String>,
    completed_generations: u64,
    // None means a failed/cancelled run may have changed private checker state.
    local_module_inventory: Option<std::collections::BTreeSet<String>>,
    cleanup_tasks: crate::cancellation::StructuredCancellationScope,
}

#[derive(Default)]
struct PyreflyProcessState {
    alive: AtomicBool,
    drain_requested: AtomicBool,
    terminal: std::sync::Mutex<Option<Result<(), String>>>,
    kernel_usage: std::sync::Mutex<Option<crate::provider_sandbox::ProviderKernelUsage>>,
}

// Constructor cancellation must close the already admitted worker even when readiness was sent
// immediately before the observing future disappeared. The worker, never this guard, owns the PID.
struct CancelPyreflyConstruction(Option<crate::cancellation::StructuredCancellationScope>);

impl Drop for CancelPyreflyConstruction {
    fn drop(&mut self) {
        if let Some(scope) = self.0.take() {
            scope.cancel();
        }
    }
}

impl SupervisedPyreflyWorkspace {
    /// Admit the process owner before invoking the launcher. The supplied control task scope
    /// reserves cleanup execution capacity; the process residency reservation remains Data.
    /// The owned output descriptor survives cancelled construction and stays pinned through join.
    pub(crate) async fn try_new<F>(
        job: &ProviderJob,
        output: std::os::fd::OwnedFd,
        cleanup_tasks: crate::cancellation::StructuredCancellationScope,
        launch: F,
    ) -> Result<Self, PyreflyServiceError>
    where
        F: FnOnce() -> Result<ProviderProcessGroupChild, crate::provider_sandbox::SandboxError>
            + Send
            + 'static,
    {
        use std::os::fd::AsRawFd as _;
        validate_pyrefly_job(job)?;
        let socket = PathBuf::from(format!("/proc/self/fd/{}/pyrefly.sock", output.as_raw_fd()));
        if job.ceilings().max_workers() == 0 {
            return Err(PyreflyServiceError::Invalid(
                "release-prepared Pyrefly job or private socket is invalid".to_owned(),
            ));
        }
        let native_envelope = crate::provider_contracts::allocation::reserve_native_state(job)?;
        let process = Arc::new(PyreflyProcessState::default());
        let mut construction = CancelPyreflyConstruction(Some(cleanup_tasks.clone()));
        let (ready, readiness) = tokio::sync::oneshot::channel();
        let worker_state = Arc::clone(&process);
        let worker_socket = socket.clone();
        let grace = Duration::from_millis(job.ceilings().cancellation_ack_millis());
        cleanup_tasks
            .spawn_blocking_owned("process-owner", (native_envelope, output), move |cancel| {
                run_owned_pyrefly_process(
                    launch,
                    &cancel,
                    &worker_socket,
                    grace,
                    &worker_state,
                    ready,
                );
            })
            .await
            .map_err(|error| PyreflyServiceError::ProcessTermination(error.to_string()))?;
        let sandbox_profile_digest = tokio::time::timeout(
            Duration::from_millis(job.ceilings().max_wall_millis()),
            readiness,
        )
        .await
        .map_err(|_| PyreflyServiceError::TimedOut)?
        .map_err(|error| PyreflyServiceError::ProcessTermination(error.to_string()))??;
        construction.0 = None;
        Ok(Self {
            process,
            socket,
            cancellation_grace: Duration::from_millis(job.ceilings().cancellation_ack_millis()),
            maximum_wall_time: Duration::from_millis(job.ceilings().max_wall_millis()),
            analysis_started: Arc::new(AtomicBool::new(false)),
            sandbox_profile_digest,
            client: None,
            compatibility: None,
            context_handle: None,
            completed_generations: 0,
            local_module_inventory: Some(std::collections::BTreeSet::new()),
            cleanup_tasks,
        })
    }

    /// Clone the real protocol acceptance signal for supervision and cancellation evidence.
    #[must_use]
    pub(crate) fn analysis_started_signal(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.analysis_started)
    }

    #[must_use]
    pub(crate) const fn completed_generations(&self) -> u64 {
        self.completed_generations
    }

    /// Latest aggregate cgroup sample from the process owner, including native allocations.
    pub(crate) fn kernel_usage(&self) -> Option<crate::provider_sandbox::ProviderKernelUsage> {
        *self
            .process
            .kernel_usage
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[must_use]
    pub(crate) fn is_healthy(&mut self) -> bool {
        !self.cleanup_tasks.is_cancelled() && self.process.alive.load(Ordering::Acquire)
    }

    async fn connect_and_validate(
        &mut self,
        job: &ProviderJob,
    ) -> Result<WireClient<Channel>, PyreflyServiceError> {
        if let Some(client) = &self.client {
            return Ok(client.clone());
        }
        validate_pyrefly_job(job)?;
        // Process ownership is established before the provider binds its socket. The caller's
        // job deadline/cancellation bounds this readiness wait, including a failed handshake.
        let channel = loop {
            if !self.process.alive.load(Ordering::Acquire) || self.cleanup_tasks.is_cancelled() {
                return Err(PyreflyServiceError::Transport(
                    "sidecar exited before connection".into(),
                ));
            }
            let socket = self.socket.clone();
            match Endpoint::from_static("http://[::]:50051")
                .connect_with_connector(service_fn(move |_| {
                    let socket = socket.clone();
                    async move { UnixStream::connect(socket).await.map(TokioIo::new) }
                }))
                .await
            {
                Ok(channel) => break channel,
                Err(_) => tokio::time::sleep(Duration::from_millis(10)).await,
            }
        };
        let mut client = WireClient::new(channel)
            .max_decoding_message_size(4 * 1024 * 1024)
            .max_encoding_message_size(4 * 1024 * 1024);
        validate_handshake(&mut client, &self.sandbox_profile_digest).await?;
        self.client = Some(client.clone());
        Ok(client)
    }

    async fn close_incompatible_context(
        &mut self,
        client: &mut WireClient<Channel>,
        compatibility: &PyreflyContextCompatibility,
    ) -> Result<(), PyreflyServiceError> {
        if self.compatibility.as_ref() == Some(compatibility)
            && self.local_module_inventory.is_some()
        {
            return Ok(());
        }
        if let Some(context_handle) = self.context_handle.take() {
            let closed = client
                .close_context(
                    crate::rpc::generated::codefabric::pyrefly::v1::CloseContextRequest {
                        context_handle,
                    },
                )
                .await
                .map_err(|error| PyreflyServiceError::Protocol(error.to_string()))?
                .into_inner();
            if !closed.closed {
                return Err(PyreflyServiceError::Protocol(
                    "incompatible Pyrefly context was not closed".to_owned(),
                ));
            }
        }
        self.compatibility = None;
        self.local_module_inventory = Some(std::collections::BTreeSet::new());
        Ok(())
    }

    async fn force_join(&mut self) -> Result<(), PyreflyServiceError> {
        self.join_process(false).await
    }

    async fn join_process(&mut self, drain_first: bool) -> Result<(), PyreflyServiceError> {
        self.process
            .drain_requested
            .store(drain_first, Ordering::Release);
        self.cleanup_tasks
            .cancel_and_join(self.cancellation_grace.saturating_mul(4))
            .await
            .map_err(|error| PyreflyServiceError::ProcessTermination(error.to_string()))?;
        self.process
            .terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .ok_or_else(|| {
                PyreflyServiceError::ProcessTermination(
                    "process owner has no observed terminal outcome".to_owned(),
                )
            })?
            .map_err(PyreflyServiceError::ProcessTermination)
    }

    async fn invalidate_and_join(&mut self) -> Result<(), PyreflyServiceError> {
        self.client = None;
        self.compatibility = None;
        self.context_handle = None;
        self.local_module_inventory = None;
        self.force_join().await
    }

    /// Gracefully close native context state, stop the serving loop, and join the complete group.
    pub(crate) async fn drain_and_join(&mut self) -> Result<(), PyreflyServiceError> {
        if let Some(mut client) = self.client.take() {
            if let Some(context_handle) = self.context_handle.take() {
                let closed = tokio::time::timeout(
                    self.cancellation_grace,
                    client.close_context(
                        crate::rpc::generated::codefabric::pyrefly::v1::CloseContextRequest {
                            context_handle,
                        },
                    ),
                )
                .await;
                if !matches!(closed, Ok(Ok(ref response)) if response.get_ref().closed) {
                    return self.force_join().await;
                }
            }
            let shutdown = tokio::time::timeout(
                self.cancellation_grace,
                client.shutdown(
                    crate::rpc::generated::codefabric::pyrefly::v1::ShutdownRequest {
                        reason: "daemon-workspace-drain".to_owned(),
                    },
                ),
            )
            .await;
            if !matches!(shutdown, Ok(Ok(ref response)) if response.get_ref().accepted) {
                return self.force_join().await;
            }
        }
        self.compatibility = None;
        self.join_process(true).await
    }
}

fn run_owned_pyrefly_process<F>(
    launch: F,
    cancel: &crate::cancellation::Cancellation,
    socket: &std::path::Path,
    grace: Duration,
    state: &PyreflyProcessState,
    ready: tokio::sync::oneshot::Sender<Result<String, PyreflyServiceError>>,
) where
    F: FnOnce() -> Result<ProviderProcessGroupChild, crate::provider_sandbox::SandboxError>,
{
    let terminal = if cancel.is_cancelled() {
        let _ = ready.send(Err(PyreflyServiceError::Cancelled));
        Ok(())
    } else {
        match launch() {
            Err(error) => {
                let detail = error.to_string();
                let _ = ready.send(Err(PyreflyServiceError::ProcessTermination(detail.clone())));
                Err(detail)
            }
            Ok(mut child) => {
                let digest = child.sandbox_profile_digest().to_owned();
                let trusted = child.trust_profile() == ProviderTrustProfile::UntrustedSandboxed
                    && valid_sandbox_profile_digest(&digest);
                if trusted {
                    state.alive.store(true, Ordering::Release);
                    let observed = ready.send(Ok(digest)).is_ok();
                    let mut next_sample = std::time::Instant::now();
                    while observed && !cancel.is_cancelled() {
                        if !matches!(child.try_wait(), Ok(None)) {
                            break;
                        }
                        if std::time::Instant::now() >= next_sample {
                            if let Ok(usage) = child.kernel_usage() {
                                *state
                                    .kernel_usage
                                    .lock()
                                    .unwrap_or_else(std::sync::PoisonError::into_inner) = usage;
                            }
                            next_sample = std::time::Instant::now() + Duration::from_millis(250);
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    state.alive.store(false, Ordering::Release);
                    finish_pyrefly_process_group(
                        child,
                        socket,
                        grace,
                        state.drain_requested.load(Ordering::Acquire),
                    )
                    .map_err(|error| error.to_string())
                } else {
                    let joined = finish_pyrefly_process_group(child, socket, grace, false)
                        .map_err(|error| error.to_string());
                    let _ = ready.send(Err(PyreflyServiceError::TrustUnavailable));
                    joined
                }
            }
        }
    };
    *state
        .terminal
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(terminal);
}

// This owned worker may outlive an observation timeout. Resource/task ownership is deliberately
// retained until kernel observation proves the complete group empty; retry failure is not death.
fn finish_pyrefly_process_group(
    mut child: ProviderProcessGroupChild,
    socket: &std::path::Path,
    grace: Duration,
    drain_first: bool,
) -> std::io::Result<()> {
    let mut empty = drain_first && child.wait_group_empty(grace).unwrap_or(false);
    while !empty {
        let _ = child.terminate_group();
        empty = child.wait_group_empty(grace).unwrap_or(false);
        if !empty {
            let _ = child.kill_group();
            empty = child.wait_group_empty(grace).unwrap_or(false);
        }
        if !empty {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    child.wait()?;
    match std::fs::remove_file(socket) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

impl Drop for SupervisedPyreflyWorkspace {
    fn drop(&mut self) {
        // No blocking work or fresh task admission at Drop. The pre-admitted worker owns the
        // child and its Data residency charge through actual process-group termination.
        self.cleanup_tasks.cancel();
    }
}

fn require_exact_pyrefly_containment(
    job: &ProviderJob,
    matrix: &SandboxCapabilityMatrix,
) -> Result<SandboxMechanism, PyreflyServiceError> {
    validate_pyrefly_job(job)?;
    let row = matrix
        .row(ProviderTrustProfile::UntrustedSandboxed)
        .ok_or(PyreflyServiceError::TrustUnavailable)?;
    if !row.available || row.mechanism == SandboxMechanism::None {
        return Err(PyreflyServiceError::TrustUnavailable);
    }
    Ok(row.mechanism)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PyreflyStopCause {
    Cancelled,
    TimedOut,
}

fn now_unix_millis() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(i64::MAX)
}

async fn wait_for_pyrefly_stop(job: &ProviderJob) -> PyreflyStopCause {
    loop {
        if job.cancellation().is_cancelled() {
            return PyreflyStopCause::Cancelled;
        }
        if job.remaining().is_none() {
            return PyreflyStopCause::TimedOut;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

async fn until_job_stop<T>(
    job: &ProviderJob,
    work: impl std::future::Future<Output = Result<T, PyreflyServiceError>>,
) -> Result<T, PyreflyServiceError> {
    tokio::select! {
        biased;
        cause = wait_for_pyrefly_stop(job) => Err(match cause {
            PyreflyStopCause::Cancelled => PyreflyServiceError::Cancelled,
            PyreflyStopCause::TimedOut => PyreflyServiceError::TimedOut,
        }),
        result = work => result,
    }
}

fn b3(bytes: &[u8]) -> String {
    format!("b3:{}", blake3::hash(bytes).to_hex())
}

fn encoded_digest(bytes: [u8; 32]) -> String {
    let mut encoded = String::with_capacity(67);
    encoded.push_str("b3:");
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn valid_digest(value: &str) -> bool {
    value.len() == 67
        && value.starts_with("b3:")
        && value[3..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_sandbox_profile_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn pyrefly_relation_for_job(value: &str) -> Result<PyreflyRelation, PyreflyServiceError> {
    PyreflyRelation::ALL
        .into_iter()
        .find(|relation| relation.relation_id() == value)
        .ok_or_else(|| {
            PyreflyServiceError::Invalid(format!(
                "provider job requested non-Pyrefly relation {value}"
            ))
        })
}

fn validate_pyrefly_job(job: &ProviderJob) -> Result<(), PyreflyServiceError> {
    if job.lane() != ProviderLane::Pyrefly
        || job.provider().as_str() != "pyrefly-python"
        || job.protocol().as_str() != "codefabric.pyrefly.provider.v1"
        || job.trust() != ProviderTrustPosture::LocalSidecarConstrained
        || job.provenance().provider_build().as_str() != "pyrefly-sidecar-pinned-source"
        || job.requests().len() != PyreflyRelation::ALL.len()
        || job.remaining().is_none()
    {
        return Err(PyreflyServiceError::Invalid(
            "provider job is not the exact release-prepared Pyrefly lane".to_owned(),
        ));
    }
    let requested = job
        .requests()
        .iter()
        .map(|request| pyrefly_relation_for_job(request.relation().as_str()))
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
    if requested != PyreflyRelation::ALL.into_iter().collect() {
        return Err(PyreflyServiceError::Invalid(
            "provider job does not request the exact Pyrefly relation closure".to_owned(),
        ));
    }
    Ok(())
}

fn request_from_job(
    job: &ProviderJob,
    input: &PyreflyWorkspaceInput,
    sandbox_profile_digest: &str,
    maximum_wall_time: Duration,
) -> Result<PyreflyRunRequest, PyreflyServiceError> {
    validate_pyrefly_job(job)?;
    if input.modules.len() > MAX_MODULES_PER_RUN
        || input.changed_module_ids.len() > MAX_MODULES_PER_RUN
        || input.context_manifest.len() as u64 > job.ceilings().max_input_bytes()
    {
        return Err(PyreflyServiceError::InputLimit(
            "module count or immutable input bytes",
        ));
    }
    let context_manifest_digest = b3(&input.context_manifest);
    let expected_context_digest = encoded_digest(job.context().semantic_environment_id());
    let module_count = u64::try_from(input.modules.len()).unwrap_or(u64::MAX);
    let changed_module_ids = validate_complete_workspace_input(job, input)?;
    if input.workspace_id.is_empty()
        || input.canonical_workspace_id == [0; 16]
        || input.canonical_workspace_id != job.source().workspace_id()
        || input.context_manifest.is_empty()
        || context_manifest_digest != expected_context_digest
        || input.source_snapshot_lease_id.is_empty()
        || input.modules.len() > MAX_MODULES_PER_RUN
        || job
            .requests()
            .iter()
            .any(|request| request.requested_units() != module_count)
    {
        return Err(PyreflyServiceError::Invalid(
            "Pyrefly workspace input differs from the exact provider job bindings".to_owned(),
        ));
    }
    let remaining = job.remaining().ok_or_else(|| {
        PyreflyServiceError::Invalid("Pyrefly provider job expired before dispatch".to_owned())
    })?;
    let effective_remaining = remaining.min(maximum_wall_time);
    let deadline_unix_ms = now_unix_millis()
        .saturating_add(i64::try_from(effective_remaining.as_millis()).unwrap_or(i64::MAX));
    let canonical_analysis_context_id = job.context().analysis_context_id();
    let requested_capability_codes = job
        .requests()
        .iter()
        .map(|request| {
            pyrefly_relation_for_job(request.relation().as_str()).map(PyreflyRelation::family_code)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PyreflyRunRequest {
        resource_budget: job.resource_budget().clone(),
        max_input_bytes: job.ceilings().max_input_bytes(),
        max_output_bytes: job.ceilings().max_bytes(),
        provider_run_id: job.run().identity().as_str().to_owned(),
        workspace_id: input.workspace_id.clone(),
        analysis_context_id: job.context().identity().as_str().to_owned(),
        canonical_workspace_id: input.canonical_workspace_id,
        canonical_analysis_context_id,
        source_generation: job.source().generation(),
        context_manifest: input.context_manifest.clone(),
        source_snapshot_lease_id: input.source_snapshot_lease_id.clone(),
        source_manifest_digest: encoded_digest(job.source().content_digest()),
        modules: input.modules.clone(),
        changed_module_ids,
        // Filled from the exact retained checker census at dispatch, never CPG withdrawals.
        expected_removed_module_ids: Vec::new(),
        requested_capability_codes,
        deadline_unix_ms,
        sandbox_profile_digest: sandbox_profile_digest.to_owned(),
        output_schema_bundle_digest: schema_bundle_digest(),
    })
}

fn validate_complete_workspace_input(
    job: &ProviderJob,
    input: &PyreflyWorkspaceInput,
) -> Result<Vec<String>, PyreflyServiceError> {
    use crate::identity::{IdentityDomain, decode_public_id, encode_public_id};
    use crate::provider_contracts::{ProviderInputDisposition, ProviderSourceSelection};
    let invalid = || {
        PyreflyServiceError::Invalid(
            "Pyrefly modules or changed work do not close the exact selected inventory".to_owned(),
        )
    };
    let ProviderSourceSelection::Inventory(inventory) = job.source().selection() else {
        return Err(invalid());
    };
    let expected = inventory
        .members()
        .iter()
        .filter_map(|member| match member.disposition {
            ProviderInputDisposition::Captured {
                file_id, digest, ..
            } if member.selected_for_provider => {
                Some((file_id, (digest, member.relative_path.as_slice())))
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut actual = BTreeMap::new();
    for module in &input.modules {
        let file_id = decode_public_id(IdentityDomain::SourceFile, None, &module.file_id)
            .map_err(|_| invalid())?;
        let binding = job.context().module_for_file(file_id).ok_or_else(invalid)?;
        // This opaque transport module handle is the source-file ID, not a semantic module ID.
        if module.module_id != module.file_id
            || module.module_name != binding.qualified_name
            || actual.insert(file_id, &module.content_digest).is_some()
            || expected.get(&file_id).is_none_or(|(digest, path)| {
                module.content_digest != encoded_digest(*digest) || binding.relative_path != *path
            })
        {
            return Err(invalid());
        }
    }
    if actual.keys().ne(expected.keys()) {
        return Err(invalid());
    }
    let changed = inventory
        .members()
        .iter()
        .filter(|member| {
            member.selected_for_provider
                && inventory
                    .changed_paths()
                    .binary_search(&member.relative_path)
                    .is_ok()
        })
        .filter_map(|member| match member.disposition {
            ProviderInputDisposition::Captured { file_id, .. } => Some(file_id),
            _ => None,
        })
        .map(|file_id| {
            encode_public_id(IdentityDomain::SourceFile, None, file_id).map_err(|_| invalid())
        })
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
    let supplied = input
        .changed_module_ids
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    if supplied.len() != input.changed_module_ids.len() || changed != supplied {
        return Err(invalid());
    }
    Ok(changed.into_iter().collect())
}

fn context_compatibility(
    job: &ProviderJob,
    request: &PyreflyRunRequest,
) -> PyreflyContextCompatibility {
    PyreflyContextCompatibility {
        workspace_id: request.workspace_id.clone(),
        analysis_context_id: request.analysis_context_id.clone(),
        semantic_environment_id: job.context().semantic_environment_id(),
        manifest_digest: b3(&request.context_manifest),
        provider_build: job.provenance().provider_build().as_str().to_owned(),
        protocol: job.protocol().as_str().to_owned(),
        sandbox_profile_digest: request.sandbox_profile_digest.clone(),
    }
}

fn expected_context_handle(request: &PyreflyRunRequest) -> String {
    let manifest_digest = b3(&request.context_manifest);
    format!(
        "pyrefly-context:{}",
        &b3(&[
            request.workspace_id.as_bytes(),
            request.analysis_context_id.as_bytes(),
            manifest_digest.as_bytes(),
        ]
        .concat())[3..35]
    )
}

async fn send_inventory(
    sender: &tokio::sync::mpsc::Sender<AnalyzeCommand>,
    modules: &[ModuleRequest],
    changed: &[String],
    chunked: bool,
) -> Result<(), PyreflyServiceError> {
    if !chunked {
        return Ok(());
    }
    let count = modules.len().max(changed.len()).div_ceil(MODULES_PER_CHUNK);
    for sequence in 0..count {
        let start = sequence * MODULES_PER_CHUNK;
        let end = start + MODULES_PER_CHUNK;
        sender
            .send(AnalyzeCommand {
                command: Some(Command::InventoryChunk(AnalyzeInventoryChunk {
                    sequence: u32::try_from(sequence).expect("bounded chunks"),
                    modules: modules[start.min(modules.len())..end.min(modules.len())].to_vec(),
                    changed_module_ids: changed[start.min(changed.len())..end.min(changed.len())]
                        .to_vec(),
                })),
            })
            .await
            .map_err(|_| {
                PyreflyServiceError::Protocol("inventory command stream closed".to_owned())
            })?;
    }
    sender
        .send(AnalyzeCommand {
            command: Some(Command::InventoryEnd(AnalyzeInventoryEnd {
                chunk_count: u32::try_from(count).expect("bounded chunks"),
                module_count: u32::try_from(modules.len()).expect("bounded modules"),
                changed_module_count: u32::try_from(changed.len())
                    .expect("bounded changed modules"),
            })),
        })
        .await
        .map_err(|_| PyreflyServiceError::Protocol("inventory command stream closed".to_owned()))
}

async fn validate_handshake(
    client: &mut WireClient<Channel>,
    sandbox_profile_digest: &str,
) -> Result<(), PyreflyServiceError> {
    let relation_schema_digests = schema_digests();
    let acknowledgement = client
        .handshake(Hello {
            protocol_major: 1,
            protocol_minor: 0,
            required_feature_bits: REQUIRED_FEATURE_BITS,
            optional_feature_bits: OPTIONAL_FEATURE_BITS,
            daemon_build: "codefabricd 0.1.0".to_owned(),
            supported_python_versions: vec!["3.14".to_owned()],
            observation_schema_digests: relation_schema_digests.clone(),
            maximum_frame_bytes: 4 * 1024 * 1024,
            maximum_arrow_ipc_bytes: 64 * 1024 * 1024,
            sandbox_profile_digest: sandbox_profile_digest.to_owned(),
        })
        .await
        .map_err(|error| PyreflyServiceError::Protocol(error.to_string()))?
        .into_inner();
    if acknowledgement.protocol_major != 1
        || acknowledgement.protocol_minor != 0
        || acknowledgement.negotiated_feature_bits != REQUIRED_FEATURE_BITS | OPTIONAL_FEATURE_BITS
        || acknowledgement.sidecar_build != "codefabric-pyrefly-sidecar 0.1.0+configured-context-v1"
        || acknowledgement.pyrefly_source_digest != PYREFLY_SOURCE_DIGEST
        || acknowledgement.observation_schema_digests != relation_schema_digests
        || acknowledgement.maximum_frame_bytes != 4 * 1024 * 1024
        || acknowledgement.maximum_arrow_ipc_bytes != 64 * 1024 * 1024
        || acknowledgement.sandbox_profile_digest != sandbox_profile_digest
    {
        return Err(PyreflyServiceError::Protocol(
            "handshake acknowledgement identity differs".to_owned(),
        ));
    }
    Ok(())
}

fn parse_digest(value: &str) -> Result<[u8; 32], PyreflyServiceError> {
    let encoded = value
        .strip_prefix("b3:")
        .filter(|encoded| encoded.len() == 64)
        .ok_or_else(|| PyreflyServiceError::Protocol("digest is not b3-32".to_owned()))?;
    let mut result = [0_u8; 32];
    for (index, chunk) in encoded.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let high = hex_nibble(chunk[0])
            .ok_or_else(|| PyreflyServiceError::Protocol("digest is not hexadecimal".to_owned()))?;
        let low = hex_nibble(chunk[1])
            .ok_or_else(|| PyreflyServiceError::Protocol("digest is not hexadecimal".to_owned()))?;
        result[index] = (high << 4) | low;
    }
    Ok(result)
}

const fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn validate_relation_pins(
    batch: &RecordBatch,
    request: &PyreflyRunRequest,
    module: &PyreflyModuleInput,
) -> Result<(), PyreflyServiceError> {
    if batch.num_rows() == 0 {
        return Ok(());
    }
    let strings_match = |column: &str, expected: &str| {
        batch
            .column_by_name(column)
            .and_then(|array| array.as_any().downcast_ref::<StringArray>())
            .is_some_and(|values| values.iter().all(|value| value == Some(expected)))
    };
    let binary_matches = |column: &str, expected: &[u8; 32]| {
        batch
            .column_by_name(column)
            .and_then(|array| array.as_any().downcast_ref::<FixedSizeBinaryArray>())
            .is_some_and(|values| {
                values
                    .iter()
                    .all(|value| value.is_some_and(|value| value == expected.as_slice()))
            })
    };
    let generations_match = batch
        .column_by_name("source_generation")
        .and_then(|array| array.as_any().downcast_ref::<UInt64Array>())
        .is_some_and(|values| {
            values
                .iter()
                .all(|value| value == Some(request.source_generation))
        });
    if !strings_match("provider_run_id", &request.provider_run_id)
        || !strings_match("analysis_context_id", &request.analysis_context_id)
        || !strings_match("module_id", &module.module_id)
        || !strings_match("file_id", &module.file_id)
        || !binary_matches("content_digest", &parse_digest(&module.content_digest)?)
        || !binary_matches(
            "semantic_environment_id",
            &parse_digest(&b3(&request.context_manifest))?,
        )
        || !generations_match
    {
        return Err(PyreflyServiceError::Arrow(
            "relation source, context, run, or generation pins differ".to_owned(),
        ));
    }
    Ok(())
}

fn expected_module_digest(
    module: &PyreflyModuleInput,
    relations: &[AcceptedPyreflyRelation],
) -> String {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(module.module_id.as_bytes());
    bytes.extend_from_slice(module.module_name.as_bytes());
    bytes.extend_from_slice(module.file_id.as_bytes());
    bytes.extend_from_slice(module.content_digest.as_bytes());
    for relation in relations {
        bytes.extend_from_slice(&relation.relation.family_code().to_be_bytes());
        bytes.extend_from_slice(relation.schema_digest.as_bytes());
        bytes.extend_from_slice(relation.arrow_ipc_digest.as_bytes());
        bytes.extend_from_slice(&relation.row_count.to_be_bytes());
    }
    b3(&bytes)
}

fn validate_call_definition_pins(
    batch: &RecordBatch,
    sources: &BTreeMap<&str, ([u8; 32], crate::source_encoding::DecodedSource<'_>)>,
) -> Result<(), PyreflyServiceError> {
    let invalid =
        || PyreflyServiceError::Arrow("call definition differs from captured input".to_owned());
    let files = batch
        .column_by_name("target_file_id")
        .and_then(|array| array.as_any().downcast_ref::<StringArray>())
        .ok_or_else(invalid)?;
    let digests = batch
        .column_by_name("target_content_digest")
        .and_then(|array| array.as_any().downcast_ref::<FixedSizeBinaryArray>())
        .ok_or_else(invalid)?;
    let starts = batch
        .column_by_name("target_start_byte")
        .and_then(|array| array.as_any().downcast_ref::<UInt64Array>())
        .ok_or_else(invalid)?;
    let ends = batch
        .column_by_name("target_end_byte")
        .and_then(|array| array.as_any().downcast_ref::<UInt64Array>())
        .ok_or_else(invalid)?;
    let mappings = batch
        .column_by_name("target_source_mapping")
        .and_then(|array| array.as_any().downcast_ref::<StringArray>())
        .ok_or_else(invalid)?;
    for row in 0..batch.num_rows() {
        if files.is_null(row) {
            if !digests.is_null(row)
                || !starts.is_null(row)
                || !ends.is_null(row)
                || !matches!(
                    mappings.value(row),
                    "definition_unavailable" | "definition_outside_inventory"
                )
            {
                return Err(invalid());
            }
            continue;
        }
        let (digest, decoded) = sources.get(files.value(row)).ok_or_else(invalid)?;
        let start = usize::try_from(starts.value(row)).map_err(|_| invalid())?;
        let end = usize::try_from(ends.value(row)).map_err(|_| invalid())?;
        let boundary = |position: usize| match decoded {
            crate::source_encoding::DecodedSource::Utf8 {
                text,
                original_start,
            } => position
                .checked_sub(*original_start)
                .is_some_and(|offset| text.is_char_boundary(offset)),
            crate::source_encoding::DecodedSource::Latin1(bytes) => position <= bytes.len(),
        };
        if digests.is_null(row)
            || starts.is_null(row)
            || ends.is_null(row)
            || digests.value(row) != digest
            || start >= end
            || end > decoded.original_len()
            || !boundary(start)
            || !boundary(end)
            || mappings.value(row) != "exact_checker_definition"
        {
            return Err(invalid());
        }
    }
    Ok(())
}

fn header_matches(header: &AnalyzeEventHeader, request: &PyreflyRunRequest, sequence: u64) -> bool {
    header.provider_run_id == request.provider_run_id
        && header.workspace_id == request.workspace_id
        && header.analysis_context_id == request.analysis_context_id
        && header.source_generation == request.source_generation
        && header.sequence == sequence
        && header.context_manifest_digest == b3(&request.context_manifest)
        && header.source_manifest_digest == request.source_manifest_digest
        && header.sandbox_profile_digest == request.sandbox_profile_digest
}

struct AdmittedImmutableBlob {
    reference: BlobReference,
    bytes: crate::resource_budget::ChargedSlice<u8>,
}

fn read_immutable_blob(
    input: &PyreflyModuleInput,
    allocation: &mut crate::provider_contracts::allocation::ProviderAllocation,
) -> Result<AdmittedImmutableBlob, PyreflyServiceError> {
    if !input.provider_source_blob_path.is_absolute()
        || input.provider_source_blob_path.as_os_str().len() > 4096
        || input
            .provider_source_blob_path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(PyreflyServiceError::Invalid(
            "provider blob path must be a bounded absolute path".into(),
        ));
    }
    let bytes =
        crate::secure_path::read_pinned_blob(&input.source_blob_path, MAX_SOURCE_BYTES_PER_MODULE)
            .map_err(|error| {
                PyreflyServiceError::Invalid(format!("immutable source blob read: {error}"))
            })?;
    if b3(&bytes) != input.content_digest {
        return Err(PyreflyServiceError::Invalid(
            "immutable source blob digest differs".to_owned(),
        ));
    }
    Ok(AdmittedImmutableBlob {
        reference: BlobReference {
            blob_id: format!("blob:{}", &b3(&bytes)[3..35]),
            content_digest: b3(&bytes),
            byte_length: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            read_only_uri: url::Url::from_file_path(&input.provider_source_blob_path)
                .map_err(|()| PyreflyServiceError::Invalid("provider blob file URI".into()))?
                .into(),
        },
        bytes: allocation.retain_measured_vec(bytes, |_| 0)?,
    })
}

/// Execute one release-prepared generation while retaining a healthy compatible sidecar.
///
/// # Errors
///
/// Rejects malformed daemon inputs or an unjoinable process. Provider cancellation, timeout,
/// corruption, trust loss, and process failure become explicit [`ProviderRunResult`] gaps.
#[allow(clippy::too_many_lines)] // Lifecycle, cooperative cancellation, and gap classification remain one atomic owner transition.
pub(crate) async fn analyze_pyrefly_uds(
    process: &mut SupervisedPyreflyWorkspace,
    job: &ProviderJob,
    input: &PyreflyWorkspaceInput,
) -> Result<PyreflyProviderRunResult, PyreflyServiceError> {
    let _request_work = crate::provider_contracts::allocation::ProviderAllocation::try_new(
        job.resource_budget(),
        job.ceilings().max_bytes(),
    )?;
    let mut request = request_from_job(
        job,
        input,
        &process.sandbox_profile_digest,
        process.maximum_wall_time,
    )?;
    if job.cancellation().is_cancelled() {
        return PyreflyProviderRunResult::gap(
            job,
            ProviderUnknownCause::Cancelled,
            "Pyrefly provider job was cancelled before dispatch",
        );
    }
    if !process.is_healthy() {
        process.invalidate_and_join().await?;
        return PyreflyProviderRunResult::gap(
            job,
            ProviderUnknownCause::ProviderFailure,
            "Pyrefly workspace sidecar exited before provider dispatch",
        );
    }

    let mut client = match until_job_stop(job, process.connect_and_validate(job)).await {
        Ok(client) => client,
        Err(PyreflyServiceError::Protocol(_)) => {
            process.invalidate_and_join().await?;
            return PyreflyProviderRunResult::gap(
                job,
                ProviderUnknownCause::Corruption,
                "Pyrefly sidecar handshake did not match the released protocol",
            );
        }
        Err(PyreflyServiceError::Transport(_)) => {
            process.invalidate_and_join().await?;
            return PyreflyProviderRunResult::gap(
                job,
                ProviderUnknownCause::ProviderFailure,
                "Pyrefly workspace sidecar transport was unavailable",
            );
        }
        Err(error @ (PyreflyServiceError::Cancelled | PyreflyServiceError::TimedOut)) => {
            process.invalidate_and_join().await?;
            let cause = if matches!(error, PyreflyServiceError::Cancelled) {
                ProviderUnknownCause::Cancelled
            } else {
                ProviderUnknownCause::Timeout
            };
            return PyreflyProviderRunResult::gap(
                job,
                cause,
                "Pyrefly handshake stopped before dispatch",
            );
        }
        Err(error) => return Err(error),
    };
    let compatibility = context_compatibility(job, &request);
    if let Err(error) = until_job_stop(
        job,
        process.close_incompatible_context(&mut client, &compatibility),
    )
    .await
    {
        process.invalidate_and_join().await?;
        return match error {
            PyreflyServiceError::Protocol(_) => PyreflyProviderRunResult::gap(
                job,
                ProviderUnknownCause::Corruption,
                "Pyrefly sidecar did not close an incompatible native context",
            ),
            PyreflyServiceError::Cancelled => PyreflyProviderRunResult::gap(
                job,
                ProviderUnknownCause::Cancelled,
                "Pyrefly context close was cancelled",
            ),
            PyreflyServiceError::TimedOut => PyreflyProviderRunResult::gap(
                job,
                ProviderUnknownCause::Timeout,
                "Pyrefly context close exceeded the job deadline",
            ),
            other => Err(other),
        };
    }
    let context_handle = expected_context_handle(&request);
    request.expected_removed_module_ids = checker_removals(
        process.local_module_inventory.as_ref().ok_or_else(|| {
            PyreflyServiceError::Protocol(
                "checker census was not reset after incompatible context closure".to_owned(),
            )
        })?,
        &request.modules,
    );
    // A cancelled or failed run cannot leave guessed provider-local reuse authority.
    process.local_module_inventory = None;
    process.compatibility = Some(compatibility);
    process.context_handle = Some(context_handle.clone());
    process.analysis_started.store(false, Ordering::Release);

    let cancellation_requested = Arc::new(AtomicBool::new(false));
    let analysis = analyze_pyrefly_uds_inner(
        client.clone(),
        &request,
        Arc::clone(&cancellation_requested),
        Some(Arc::clone(&process.analysis_started)),
    );
    let stop = wait_for_pyrefly_stop(job);
    tokio::pin!(analysis);
    tokio::pin!(stop);
    let outcome = tokio::select! {
        biased;
        result = &mut analysis => Ok(result),
        cause = &mut stop => Err(cause),
    };
    let accepted = match outcome {
        Ok(result) => result,
        Err(cause) => {
            cancellation_requested.store(true, Ordering::Release);
            let reason = match cause {
                PyreflyStopCause::Cancelled => "provider-job-cancelled",
                PyreflyStopCause::TimedOut => "provider-job-deadline",
            };
            let cancellation = CancelRunRequest {
                provider_run_id: request.provider_run_id.clone(),
                reason: reason.to_owned(),
            };
            let acknowledgement =
                tokio::time::timeout(process.cancellation_grace, client.cancel_run(cancellation))
                    .await;
            let acknowledged = matches!(
                acknowledgement,
                Ok(Ok(ref response)) if {
                    let response = response.get_ref();
                    response.provider_run_id == request.provider_run_id
                        && matches!(
                            CancelAcknowledgementState::try_from(response.state),
                            Ok(CancelAcknowledgementState::CancellationRequested
                                | CancelAcknowledgementState::AlreadyTerminal)
                        )
                        && !response.forced_termination
                }
            );
            if acknowledged {
                match tokio::time::timeout(process.cancellation_grace, &mut analysis).await {
                    Ok(Err(PyreflyServiceError::Cancelled)) => {
                        let (cause, detail) = match cause {
                            PyreflyStopCause::Cancelled => (
                                ProviderUnknownCause::Cancelled,
                                "Pyrefly provider run was cooperatively cancelled",
                            ),
                            PyreflyStopCause::TimedOut => (
                                ProviderUnknownCause::Timeout,
                                "Pyrefly provider run reached its release-owned deadline",
                            ),
                        };
                        return PyreflyProviderRunResult::gap(job, cause, detail);
                    }
                    Ok(result) => result,
                    Err(_) => {
                        process.invalidate_and_join().await?;
                        return PyreflyProviderRunResult::gap(
                            job,
                            ProviderUnknownCause::ProviderFailure,
                            "Pyrefly cancellation acknowledgement did not lead to a bounded terminal",
                        );
                    }
                }
            } else {
                process.invalidate_and_join().await?;
                return PyreflyProviderRunResult::gap(
                    job,
                    ProviderUnknownCause::ProviderFailure,
                    "Pyrefly sidecar did not acknowledge cooperative cancellation",
                );
            }
        }
    };

    match accepted {
        Ok(accepted) => {
            if process.context_handle.as_deref() != Some(context_handle.as_str()) {
                process.invalidate_and_join().await?;
                return PyreflyProviderRunResult::gap(
                    job,
                    ProviderUnknownCause::Corruption,
                    "Pyrefly sidecar returned a different native context handle",
                );
            }
            let result = PyreflyProviderRunResult::try_new(job, accepted)?;
            process.local_module_inventory = Some(
                request
                    .modules
                    .iter()
                    .map(|module| module.module_id.clone())
                    .collect(),
            );
            process.completed_generations = process.completed_generations.saturating_add(1);
            Ok(result)
        }
        Err(PyreflyServiceError::InputLimit(_)) => PyreflyProviderRunResult::gap(
            job,
            ProviderUnknownCause::Oversized,
            "Pyrefly inventory exceeds the admitted input limit",
        ),
        Err(PyreflyServiceError::Cancelled) => {
            process.invalidate_and_join().await?;
            PyreflyProviderRunResult::gap(
                job,
                ProviderUnknownCause::Corruption,
                "Pyrefly sidecar cancelled a run without an application request",
            )
        }
        Err(PyreflyServiceError::Protocol(_) | PyreflyServiceError::Arrow(_)) => {
            process.invalidate_and_join().await?;
            PyreflyProviderRunResult::gap(
                job,
                ProviderUnknownCause::Corruption,
                "Pyrefly sidecar emitted corrupt transport or Arrow evidence",
            )
        }
        Err(PyreflyServiceError::Transport(_) | PyreflyServiceError::ProcessTermination(_)) => {
            process.invalidate_and_join().await?;
            PyreflyProviderRunResult::gap(
                job,
                ProviderUnknownCause::ProviderFailure,
                "Pyrefly workspace sidecar failed before a valid terminal",
            )
        }
        Err(PyreflyServiceError::PreparationUnavailable) => {
            process.invalidate_and_join().await?;
            PyreflyProviderRunResult::gap(
                job,
                ProviderUnknownCause::Unsupported,
                "Selected Pyrefly context lacks proved bundle or root authority",
            )
        }
        Err(PyreflyServiceError::TrustUnavailable) => {
            process.invalidate_and_join().await?;
            PyreflyProviderRunResult::gap(
                job,
                ProviderUnknownCause::TrustLoss,
                "Pyrefly workspace sidecar lost its exact containment trust",
            )
        }
        Err(PyreflyServiceError::TimedOut) => {
            process.invalidate_and_join().await?;
            PyreflyProviderRunResult::gap(
                job,
                ProviderUnknownCause::Timeout,
                "Pyrefly provider run reached its release-owned deadline",
            )
        }
        Err(
            error @ (PyreflyServiceError::Invalid(_)
            | PyreflyServiceError::Io { .. }
            | PyreflyServiceError::Contract(_)),
        ) => Err(error),
    }
}

#[allow(clippy::too_many_lines)] // One sidecar stream validator keeps every ordered correlation and terminal check adjacent.
async fn analyze_pyrefly_uds_inner(
    mut client: WireClient<Channel>,
    request: &PyreflyRunRequest,
    cancellation_requested: Arc<AtomicBool>,
    analysis_started: Option<Arc<AtomicBool>>,
) -> Result<AcceptedPyreflyRun, PyreflyServiceError> {
    let maximum = request
        .max_input_bytes
        .checked_add(request.max_output_bytes)
        .ok_or(crate::provider_contracts::ProviderContractError::ResourceOverflow)?;
    let mut allocation = crate::provider_contracts::allocation::ProviderAllocation::try_new(
        &request.resource_budget,
        maximum,
    )?;
    // IPC assembly and decoding are opaque working storage, released on every terminal path.
    let _decode_scratch = crate::provider_contracts::allocation::ProviderAllocation::try_new(
        &request.resource_budget,
        request.max_output_bytes,
    )?;
    if request.modules.len() > MAX_MODULES_PER_RUN
        || request.provider_run_id.is_empty()
        || request.workspace_id.is_empty()
        || request.analysis_context_id.is_empty()
        || request.canonical_workspace_id == [0; 16]
        || request.canonical_analysis_context_id == [0; 16]
        || !valid_digest(&request.source_manifest_digest)
        || !valid_sandbox_profile_digest(&request.sandbox_profile_digest)
        || request.output_schema_bundle_digest != schema_bundle_digest()
    {
        return Err(PyreflyServiceError::Invalid(
            "run identity, modules, or digests are incomplete".to_owned(),
        ));
    }
    let admitted_blobs = request
        .modules
        .iter()
        .map(|module| read_immutable_blob(module, &mut allocation))
        .collect::<Result<Vec<_>, _>>()?;
    let admitted_source_total = admitted_blobs.iter().try_fold(0_u64, |total, blob| {
        total.checked_add(blob.reference.byte_length)
    });
    if admitted_source_total.is_none_or(|total| total > MAX_SOURCE_BYTES_PER_RUN) {
        return Err(PyreflyServiceError::InputLimit("source bytes per run"));
    }
    let blobs = admitted_blobs
        .iter()
        .map(|blob| blob.reference.clone())
        .collect::<Vec<_>>();
    let admitted_source_bytes = request
        .modules
        .iter()
        .zip(&admitted_blobs)
        .map(|(module, blob)| (module.module_id.clone(), blob.bytes.clone()))
        .collect::<BTreeMap<_, _>>();
    let definition_sources = request
        .modules
        .iter()
        .zip(&admitted_blobs)
        .map(|(module, blob)| {
            Ok((
                module.file_id.as_str(),
                (
                    parse_digest(&module.content_digest)?,
                    crate::source_encoding::DecodedSource::select(&blob.bytes, true).map_err(
                        |_| {
                            PyreflyServiceError::Protocol(
                                "unsupported captured source encoding".to_owned(),
                            )
                        },
                    )?,
                ),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, PyreflyServiceError>>()?;
    let context_digest = b3(&request.context_manifest);
    let lease = SourceSnapshotLease {
        lease_id: request.source_snapshot_lease_id.clone(),
        workspace_id: request.workspace_id.clone(),
        source_generation: request.source_generation,
        source_manifest_digest: request.source_manifest_digest.clone(),
        expires_at_unix_ms: request.deadline_unix_ms,
        blobs: blobs.clone(),
    };
    let open = OpenContextRequest {
        workspace_id: request.workspace_id.clone(),
        analysis_context_id: request.analysis_context_id.clone(),
        immutable_context_manifest: request.context_manifest.clone(),
        context_manifest_digest: context_digest.clone(),
        source_snapshot_lease: Some(lease),
        resource_profile_id: RESOURCE_PROFILE_ID.to_owned(),
        maximum_contexts: 4,
        maximum_memory_mib: 16_384,
        sandbox_profile_digest: request.sandbox_profile_digest.clone(),
    };
    if open.encoded_len() > 4 * 1024 * 1024 {
        return Err(PyreflyServiceError::InputLimit("context metadata frame"));
    }
    let opened = client
        .open_context(open)
        .await
        .map_err(context_open_error)?
        .into_inner();
    if opened.context_handle.is_empty()
        || opened.context_manifest_digest != context_digest
        || opened.sandbox_profile_digest != request.sandbox_profile_digest
    {
        return Err(PyreflyServiceError::Protocol(
            "opened context identity differs".to_owned(),
        ));
    }
    let mut modules = request
        .modules
        .iter()
        .zip(blobs)
        .map(|(module, blob)| ModuleRequest {
            module_id: module.module_id.clone(),
            module_name: module.module_name.clone(),
            file_id: module.file_id.clone(),
            source_digest: blob.content_digest.clone(),
            source_blob: Some(blob),
            dependency_generation: request.source_generation,
            module_resolution_generation: request.source_generation,
        })
        .collect::<Vec<_>>();
    let chunked =
        modules.len() > MODULES_PER_CHUNK || request.changed_module_ids.len() > MODULES_PER_CHUNK;
    let mut changed_module_ids = request.changed_module_ids.clone();
    let start = AnalyzeCommand {
        command: Some(Command::Start(AnalyzeModulesRequest {
            complete_inventory: true,
            expected_module_count: chunked
                .then(|| u32::try_from(modules.len()).expect("bounded module count")),
            changed_module_ids: if chunked {
                Vec::new()
            } else {
                std::mem::take(&mut changed_module_ids)
            },
            provider_run_id: request.provider_run_id.clone(),
            workspace_id: request.workspace_id.clone(),
            analysis_context_id: request.analysis_context_id.clone(),
            context_handle: opened.context_handle,
            context_manifest_digest: context_digest.clone(),
            source_generation: request.source_generation,
            source_snapshot_lease_id: request.source_snapshot_lease_id.clone(),
            modules: if chunked {
                Vec::new()
            } else {
                std::mem::take(&mut modules)
            },
            requested_capability_codes: request.requested_capability_codes.clone(),
            deadline_unix_ms: request.deadline_unix_ms,
            output_schema_bundle_digest: request.output_schema_bundle_digest.clone(),
            initial_frame_credits: 4,
            initial_credit_bytes: MAX_UNACKNOWLEDGED_BYTES,
            sandbox_profile_digest: request.sandbox_profile_digest.clone(),
            trust_profile: TRUST_PROFILE.to_owned(),
            resource_profile_id: RESOURCE_PROFILE_ID.to_owned(),
        })),
    };
    let (command_sender, command_receiver) = tokio::sync::mpsc::channel(8);
    command_sender
        .send(start)
        .await
        .map_err(|_| PyreflyServiceError::Protocol("analysis command stream closed".to_owned()))?;
    // Upload and RPC establishment must advance together: the bounded sender may fill before
    // the server returns response headers. No independent task survives a failed RPC.
    let ((), response) = tokio::try_join!(
        send_inventory(&command_sender, &modules, &changed_module_ids, chunked),
        async {
            client
                .analyze_modules(ReceiverStream::new(command_receiver))
                .await
                .map_err(|error| PyreflyServiceError::Protocol(error.to_string()))
        },
    )?;
    let mut stream = response.into_inner();
    let mut sequence = 0_u64;
    let mut analysis_progress_seen = false;
    let mut accepted = Vec::new();
    let mut open_module: Option<String> = None;
    let mut pending: Option<PendingPyreflyModule> = None;
    let mut terminal = None;
    let mut cancelled_terminal = false;
    let mut total_relation_bytes = 0_usize;
    while let Some(event) = stream
        .message()
        .await
        .map_err(|error| PyreflyServiceError::Protocol(error.to_string()))?
    {
        match event
            .event
            .ok_or_else(|| PyreflyServiceError::Protocol("stream event is empty".to_owned()))?
        {
            Event::RunAccepted(event) => {
                let header = event.header.ok_or_else(|| {
                    PyreflyServiceError::Protocol("accepted header is absent".to_owned())
                })?;
                if sequence != 0
                    || !header_matches(&header, request, sequence)
                    || event.granted_frame_credits != 4
                    || event.granted_credit_bytes != MAX_UNACKNOWLEDGED_BYTES
                {
                    return Err(PyreflyServiceError::Protocol(
                        "accepted correlation or sequence differs".to_owned(),
                    ));
                }
            }
            Event::ModuleBegin(event) => {
                sequence += 1;
                let header = event.header.ok_or_else(|| {
                    PyreflyServiceError::Protocol("module-begin header is absent".to_owned())
                })?;
                if !analysis_progress_seen
                    || open_module.is_some()
                    || pending.is_some()
                    || !header_matches(&header, request, sequence)
                {
                    return Err(PyreflyServiceError::Protocol(
                        "module-begin order or correlation differs".to_owned(),
                    ));
                }
                let requested_module = request
                    .modules
                    .iter()
                    .find(|module| module.module_id == event.module_id)
                    .ok_or_else(|| {
                        PyreflyServiceError::Protocol(
                            "provider returned an unrequested module".to_owned(),
                        )
                    })?;
                let canonical_file_id = crate::identity::decode_public_id(
                    crate::identity::IdentityDomain::SourceFile,
                    None,
                    &requested_module.file_id,
                )
                .map_err(|_| {
                    PyreflyServiceError::Invalid(
                        "module file_id is not a canonical file identity".to_owned(),
                    )
                })?;
                let source_bytes = admitted_source_bytes
                    .get(&event.module_id)
                    .ok_or_else(|| {
                        PyreflyServiceError::Protocol(
                            "provider returned a module without admitted source bytes".to_owned(),
                        )
                    })?
                    .clone();
                let limits = RelationIpcLimits {
                    max_registered_streams: PyreflyRelation::ALL.len(),
                    max_frames_per_stream: 64,
                    max_payload_bytes_per_frame:
                        crate::relation_ipc_contract::RELATION_IPC_FRAGMENT_BYTES,
                    max_payload_bytes_per_stream: 16 * 1024 * 1024,
                    max_total_payload_bytes: MAX_TOTAL_RELATION_BYTES,
                    initial_credit_bytes: usize::try_from(MAX_UNACKNOWLEDGED_BYTES)
                        .unwrap_or(usize::MAX),
                    max_credit_bytes: usize::try_from(MAX_UNACKNOWLEDGED_BYTES)
                        .unwrap_or(usize::MAX),
                    max_batches_per_stream: 1,
                    max_rows_per_stream: usize::try_from(MAX_RELATION_ROWS).unwrap_or(usize::MAX),
                    max_remainders_per_stream: 64,
                };
                let mut assembler = RelationIpcAssembler::new(limits).map_err(|error| {
                    PyreflyServiceError::Protocol(format!(
                        "relation assembler limits are invalid: {error}"
                    ))
                })?;
                let mut relation_by_stream = BTreeMap::new();
                let mut next_ack_sequence = BTreeMap::new();
                for relation in PyreflyRelation::ALL {
                    let contract = relation_stream_contract(
                        relation.relation_id(),
                        relation.schema(),
                        &request.provider_run_id,
                        &event.module_id,
                        &request.source_manifest_digest,
                        &context_digest,
                        1,
                    )
                    .map_err(PyreflyServiceError::Protocol)?;
                    relation_by_stream.insert(contract.identity.stream_id, relation);
                    next_ack_sequence.insert(contract.identity.stream_id, 0);
                    assembler.register_contract(contract).map_err(|error| {
                        PyreflyServiceError::Protocol(format!(
                            "relation contract registration failed: {error}"
                        ))
                    })?;
                }
                open_module = Some(event.module_id.clone());
                pending = Some(PendingPyreflyModule {
                    module_id: event.module_id,
                    module_name: requested_module.module_name.clone(),
                    canonical_file_id,
                    source_bytes,
                    relations: BTreeMap::new(),
                    relation_by_stream,
                    next_ack_sequence,
                    assembler,
                });
            }
            Event::RelationIpcFrame(event) => {
                sequence += 1;
                let header = event.header.ok_or_else(|| {
                    PyreflyServiceError::Protocol("relation-frame header is absent".to_owned())
                })?;
                let relation = PyreflyRelation::from_family_code(event.observation_family_code);
                if open_module.as_deref() != Some(event.module_id.as_str())
                    || pending.is_none()
                    || !header_matches(&header, request, sequence)
                    || relation.is_none()
                {
                    return Err(PyreflyServiceError::Protocol(
                        "relation-frame identity, module, or family differs".to_owned(),
                    ));
                }
                let relation = relation.expect("checked above");
                let wire_frame = event.frame.ok_or_else(|| {
                    PyreflyServiceError::Protocol("relation frame is absent".to_owned())
                })?;
                let frame = decode_relation_frame(wire_frame).map_err(|error| {
                    PyreflyServiceError::Protocol(format!(
                        "relation protobuf envelope is invalid: {error}"
                    ))
                })?;
                if matches!(frame, RelationIpcFrame::FlowControlAck(_)) {
                    return Err(PyreflyServiceError::Protocol(
                        "provider sent a receiver-direction acknowledgement frame".to_owned(),
                    ));
                }
                let frame_header = frame.header();
                let stream_id = frame_header.identity.stream_id;
                let payload = match &frame {
                    RelationIpcFrame::Payload(payload) => Some((
                        payload.header.identity,
                        payload.header.sequence,
                        payload.payload.len(),
                    )),
                    _ => None,
                };
                if pending
                    .as_ref()
                    .and_then(|module| module.relation_by_stream.get(&stream_id))
                    != Some(&relation)
                {
                    return Err(PyreflyServiceError::Protocol(
                        "relation frame does not match its application-owned stream contract"
                            .to_owned(),
                    ));
                }
                if let Some((_, _, bytes)) = payload {
                    total_relation_bytes =
                        total_relation_bytes.checked_add(bytes).ok_or_else(|| {
                            PyreflyServiceError::Protocol(
                                "relation stream byte accounting overflowed".to_owned(),
                            )
                        })?;
                    if total_relation_bytes > MAX_TOTAL_RELATION_BYTES {
                        return Err(PyreflyServiceError::Protocol(
                            "relation streams exceed the per-run byte budget".to_owned(),
                        ));
                    }
                }
                let assembled = {
                    let module = pending.as_mut().expect("checked above");
                    module.assembler.push(frame)
                };
                let assembled = match assembled {
                    Ok(assembled) => assembled,
                    Err(error) => {
                        let ack_sequence = pending
                            .as_ref()
                            .and_then(|module| module.next_ack_sequence.get(&stream_id))
                            .copied()
                            .unwrap_or_default();
                        let cancellation = RelationIpcFrame::FlowControlAck(FlowControlAck {
                            header: FrameHeader::current(frame_header.identity, ack_sequence),
                            acknowledged_sequence: None,
                            released_bytes: 0,
                            cancelled: true,
                        });
                        if let Ok(frame) = encode_relation_frame(&cancellation) {
                            let _ = command_sender
                                .send(AnalyzeCommand {
                                    command: Some(Command::RelationIpcAck(frame)),
                                })
                                .await;
                        }
                        return Err(PyreflyServiceError::Protocol(format!(
                            "relation stream failed closed: {error}"
                        )));
                    }
                };
                if let Some((identity, acknowledged_sequence, bytes)) = payload {
                    let ack_sequence = pending
                        .as_ref()
                        .and_then(|module| module.next_ack_sequence.get(&stream_id))
                        .copied()
                        .ok_or_else(|| {
                            PyreflyServiceError::Protocol(
                                "relation acknowledgement state is absent".to_owned(),
                            )
                        })?;
                    let acknowledgement = RelationIpcFrame::FlowControlAck(FlowControlAck {
                        header: FrameHeader::current(identity, ack_sequence),
                        acknowledged_sequence: Some(acknowledged_sequence),
                        released_bytes: u64::try_from(bytes).unwrap_or(u64::MAX),
                        cancelled: false,
                    });
                    {
                        let module = pending.as_mut().expect("checked above");
                        module
                            .assembler
                            .push(acknowledgement.clone())
                            .map_err(|error| {
                                PyreflyServiceError::Protocol(format!(
                                    "local credit proof failed: {error}"
                                ))
                            })?;
                        *module
                            .next_ack_sequence
                            .get_mut(&stream_id)
                            .expect("registered stream has acknowledgement state") += 1;
                    }
                    command_sender
                        .send(AnalyzeCommand {
                            command: Some(Command::RelationIpcAck(
                                encode_relation_frame(&acknowledgement)
                                    .map_err(PyreflyServiceError::Protocol)?,
                            )),
                        })
                        .await
                        .map_err(|_| {
                            PyreflyServiceError::Protocol(
                                "relation acknowledgement stream closed".to_owned(),
                            )
                        })?;
                }
                if let Some(assembled) = assembled {
                    if assembled.trailer.status != crate::relation_ipc::TerminalStatus::Complete
                        || assembled.batches.len() != 1
                    {
                        return Err(PyreflyServiceError::Protocol(
                            "successful Pyrefly relation is not complete or single-batch"
                                .to_owned(),
                        ));
                    }
                    let batch = assembled
                        .batches
                        .into_iter()
                        .next()
                        .expect("single batch checked above");
                    let requested_module = request
                        .modules
                        .iter()
                        .find(|module| module.module_id == event.module_id)
                        .ok_or_else(|| {
                            PyreflyServiceError::Protocol(
                                "provider returned an unrequested module".to_owned(),
                            )
                        })?;
                    validate_relation_pins(&batch, request, requested_module)?;
                    if relation == PyreflyRelation::CallTarget {
                        validate_call_definition_pins(&batch, &definition_sources)?;
                    }
                    let row_count = u64::try_from(batch.num_rows()).unwrap_or(u64::MAX);
                    let arrow_ipc = allocation.retain_measured_vec(assembled.ipc_bytes, |_| 0)?;
                    let accepted = AcceptedPyreflyRelation {
                        relation,
                        schema_digest: relation.schema_digest(),
                        arrow_ipc_digest: b3(&arrow_ipc),
                        arrow_ipc,
                        batch,
                        row_count,
                    };
                    if pending
                        .as_mut()
                        .expect("checked above")
                        .relations
                        .insert(relation, accepted)
                        .is_some()
                    {
                        return Err(PyreflyServiceError::Protocol(
                            "relation terminal is duplicated".to_owned(),
                        ));
                    }
                }
            }
            Event::ModuleEnd(event) => {
                sequence += 1;
                let header = event.header.ok_or_else(|| {
                    PyreflyServiceError::Protocol("module-end header is absent".to_owned())
                })?;
                let module = pending.take().ok_or_else(|| {
                    PyreflyServiceError::Protocol(
                        "module ended without a complete relation stream".to_owned(),
                    )
                })?;
                module.assembler.finish().map_err(|error| {
                    PyreflyServiceError::Protocol(format!(
                        "module ended before every relation terminal: {error}"
                    ))
                })?;
                let requested_module = request
                    .modules
                    .iter()
                    .find(|candidate| candidate.module_id == event.module_id)
                    .ok_or_else(|| {
                        PyreflyServiceError::Protocol(
                            "module terminal names an unrequested module".to_owned(),
                        )
                    })?;
                let relation_census_matches = PyreflyRelation::ALL.into_iter().all(|relation| {
                    module.relations.get(&relation).is_some_and(|accepted| {
                        event.family_counts.get(&relation.family_code())
                            == Some(&accepted.row_count)
                    })
                }) && event.family_counts.len()
                    == PyreflyRelation::ALL.len();
                let relations = module.relations.into_values().collect::<Vec<_>>();
                if open_module.take().as_deref() != Some(event.module_id.as_str())
                    || module.module_id != event.module_id
                    || !relation_census_matches
                    || !header_matches(&header, request, sequence)
                    || !valid_digest(&event.module_digest)
                    || event.module_digest != expected_module_digest(requested_module, &relations)
                {
                    return Err(PyreflyServiceError::Protocol(
                        "module terminal identity or counts differ".to_owned(),
                    ));
                }
                let context = relations
                    .iter()
                    .find(|relation| relation.relation == PyreflyRelation::ModuleContext)
                    .expect("relation census checked above");
                let module_name = context
                    .batch
                    .column_by_name("module_name")
                    .and_then(|array| array.as_any().downcast_ref::<StringArray>())
                    .filter(|array| array.len() == 1)
                    .map(|array| array.value(0).to_owned())
                    .ok_or_else(|| {
                        PyreflyServiceError::Arrow(
                            "module context does not carry one module name".to_owned(),
                        )
                    })?;
                if module_name != module.module_name {
                    return Err(PyreflyServiceError::Arrow(
                        "module context name differs from the admitted request".to_owned(),
                    ));
                }
                accepted.push(AcceptedPyreflyModule {
                    module_id: module.module_id,
                    module_name,
                    canonical_file_id: module.canonical_file_id,
                    source_bytes: module.source_bytes,
                    module_digest: event.module_digest,
                    relations: allocation.retain_measured_vec(relations, |relation| {
                        relation
                            .schema_digest
                            .capacity()
                            .saturating_add(relation.arrow_ipc_digest.capacity())
                    })?,
                });
            }
            Event::RunTerminal(event) => {
                sequence += 1;
                let header = event.header.ok_or_else(|| {
                    PyreflyServiceError::Protocol("terminal header is absent".to_owned())
                })?;
                let ordered_module_digests = accepted
                    .iter()
                    .map(|module| module.module_digest.clone())
                    .collect::<Vec<_>>();
                let expected_overall_digest = b3(&ordered_module_digests
                    .iter()
                    .flat_map(std::string::String::as_bytes)
                    .copied()
                    .collect::<Vec<_>>());
                let requested_module_ids = request
                    .modules
                    .iter()
                    .map(|module| module.module_id.as_str())
                    .collect::<std::collections::BTreeSet<_>>();
                let rechecked_module_ids = event
                    .rechecked_module_ids
                    .iter()
                    .map(String::as_str)
                    .collect::<std::collections::BTreeSet<_>>();
                let rechecked_scope_valid = rechecked_module_ids.len()
                    == event.rechecked_module_ids.len()
                    && rechecked_module_ids.is_subset(&requested_module_ids);
                let terminal_state =
                    WireProviderRunState::try_from(event.terminal_state).map_err(|_| {
                        PyreflyServiceError::Protocol(
                            "run terminal state is unregistered".to_owned(),
                        )
                    })?;
                let cancellation_expected = cancellation_requested.load(Ordering::Acquire);
                let accepted_cancellation =
                    cancellation_expected && terminal_state == WireProviderRunState::Cancelled;
                if accepted_cancellation {
                    open_module.take();
                    pending.take();
                }
                let success_outcomes = event.capability_outcomes.iter().all(|outcome| {
                    (request.modules.is_empty()
                        && outcome.owner_capability_state_code == 10
                        && outcome.completeness_state_code == 10
                        && outcome.reason_code == "PYREFLY_EMPTY_SELECTED_INVENTORY")
                        || (!request.modules.is_empty()
                            && outcome.owner_capability_state_code == 40
                            && outcome.completeness_state_code == 20
                            && outcome.reason_code == "PYREFLY_QUERY_SLICE_PARTIAL")
                });
                let cancelled_outcomes = event.capability_outcomes.iter().all(|outcome| {
                    outcome.owner_capability_state_code == 30
                        && outcome.completeness_state_code == 40
                        && outcome.reason_code == "PYREFLY_CANCELLED"
                });
                if open_module.is_some()
                    || pending.is_some()
                    || terminal.is_some()
                    || !analysis_progress_seen
                    || !header_matches(&header, request, sequence)
                    || event.ordered_module_digests != ordered_module_digests
                    || event.overall_digest != expected_overall_digest
                    || !rechecked_scope_valid
                    || (!accepted_cancellation
                        && event.removed_module_ids != request.expected_removed_module_ids)
                    || event.sandbox_profile_digest != request.sandbox_profile_digest
                    || event.trust_profile != TRUST_PROFILE
                    || !((terminal_state == WireProviderRunState::Succeeded && success_outcomes)
                        || (accepted_cancellation && cancelled_outcomes))
                {
                    return Err(PyreflyServiceError::Protocol(
                        "run terminal identity, order, or state differs".to_owned(),
                    ));
                }
                cancelled_terminal = terminal_state == WireProviderRunState::Cancelled;
                terminal = Some((
                    event
                        .capability_outcomes
                        .iter()
                        .map(|outcome| outcome.capability_code)
                        .collect::<Vec<_>>(),
                    event.overall_digest,
                    event.rechecked_module_ids,
                    event.removed_module_ids,
                    event.sandbox_profile_digest,
                    event.trust_profile,
                ));
            }
            Event::RunProgress(event) => {
                sequence += 1;
                let header = event.header.ok_or_else(|| {
                    PyreflyServiceError::Protocol("progress header is absent".to_owned())
                })?;
                if analysis_progress_seen
                    || open_module.is_some()
                    || pending.is_some()
                    || !accepted.is_empty()
                    || terminal.is_some()
                    || !header_matches(&header, request, sequence)
                    || event.completed_modules != 0
                    || usize::try_from(event.total_modules).ok() != Some(request.modules.len())
                {
                    return Err(PyreflyServiceError::Protocol(
                        "analysis-start progress identity or module census differs".to_owned(),
                    ));
                }
                if let Some(analysis_started) = &analysis_started {
                    analysis_started.store(true, Ordering::Release);
                }
                analysis_progress_seen = true;
            }
        }
    }
    let (
        capability_codes,
        overall_digest,
        rechecked_module_ids,
        removed_module_ids,
        sandbox_profile_digest,
        trust_profile,
    ) = terminal
        .ok_or_else(|| PyreflyServiceError::Protocol("stream ended before terminal".to_owned()))?;
    if cancelled_terminal {
        if !cancellation_requested.load(Ordering::Acquire) {
            return Err(PyreflyServiceError::Protocol(
                "sidecar returned cancelled without a daemon cancellation request".to_owned(),
            ));
        }
        return Err(PyreflyServiceError::Cancelled);
    }
    if accepted.len() != request.modules.len()
        || capability_codes != request.requested_capability_codes
    {
        return Err(PyreflyServiceError::Protocol(
            "accepted module or capability census differs".to_owned(),
        ));
    }
    allocation.claim_new_batches(
        accepted
            .iter()
            .flat_map(|module| module.relations.iter().map(|relation| &relation.batch)),
        262_144,
    )?;
    Ok(AcceptedPyreflyRun {
        provider_run_id: request.provider_run_id.clone(),
        workspace_id: request.workspace_id.clone(),
        analysis_context_id: request.analysis_context_id.clone(),
        canonical_workspace_id: request.canonical_workspace_id,
        canonical_analysis_context_id: request.canonical_analysis_context_id,
        source_generation: request.source_generation,
        modules: allocation.retain_measured_vec(accepted, |module| {
            module
                .module_id
                .capacity()
                .saturating_add(module.module_name.capacity())
                .saturating_add(module.module_digest.capacity())
        })?,
        capability_codes: allocation.retain_measured_vec(capability_codes, |_| 0)?,
        overall_digest,
        rechecked_module_ids: allocation
            .retain_measured_vec(rechecked_module_ids, String::capacity)?,
        removed_module_ids: allocation.retain_measured_vec(removed_module_ids, String::capacity)?,
        sandbox_profile_digest,
        trust_profile,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::pin::Pin;
    use std::process::{Child, Command as ProcessCommand, Stdio};
    use std::time::{Duration, Instant};

    use arrow_ipc::writer::StreamWriter;
    use tokio_stream::Stream;
    use tokio_stream::wrappers::UnixListenerStream;
    use tonic::{Request, Response, Status};

    use super::*;

    #[tokio::test]
    async fn wp79_pyrefly_cancelled_join_cannot_turn_missing_child_into_joined_absence() {
        struct ReleaseOnDrop(Arc<AtomicBool>);
        impl Drop for ReleaseOnDrop {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }
        let (job, _cancel) = test_job(b"{}", 7, 7);
        let budget = job.resource_budget().clone();
        let baseline = budget.observation().used.memory_bytes;
        let root = crate::cancellation::StructuredCancellationScope::try_root_with_control_reserve(
            "provider-fixture",
            std::num::NonZeroUsize::new(1).unwrap(),
            std::num::NonZeroUsize::new(2).unwrap(),
        )
        .unwrap();
        let cleanup_tasks = root.child_control("pyrefly").unwrap();
        let native_envelope =
            crate::provider_contracts::allocation::reserve_native_state(&job).unwrap();
        let state = Arc::new(PyreflyProcessState::default());
        let worker_state = Arc::clone(&state);
        let live = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let _release_on_exit = ReleaseOnDrop(Arc::clone(&release));
        let worker_live = Arc::clone(&live);
        let worker_release = Arc::clone(&release);
        let operation = cleanup_tasks
            .spawn_blocking_owned("process-owner", native_envelope, move |_| {
                worker_live.store(true, Ordering::Release);
                while !worker_release.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(1));
                }
                worker_live.store(false, Ordering::Release);
                *worker_state.terminal.lock().unwrap() = Some(Ok(()));
            })
            .await
            .unwrap();
        while !live.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        assert!(
            tokio::time::timeout(Duration::from_millis(5), operation.wait())
                .await
                .is_err()
        );
        let mut process = SupervisedPyreflyWorkspace {
            process: state,
            socket: PathBuf::new(),
            cancellation_grace: Duration::from_millis(5),
            maximum_wall_time: Duration::from_secs(1),
            analysis_started: Arc::new(AtomicBool::new(false)),
            sandbox_profile_digest: b3(b"fixture"),
            client: None,
            compatibility: None,
            context_handle: None,
            completed_generations: 0,
            local_module_inventory: None,
            cleanup_tasks,
        };
        assert!(process.join_process(true).await.is_err());
        assert!(live.load(Ordering::Acquire));
        assert!(budget.observation().used.memory_bytes > baseline);
        let health = root
            .child_control("health")
            .unwrap()
            .spawn_async_owned(
                "ping",
                crate::cancellation::TaskCancellationMode::AbortableAsync,
                (),
                async { 7_u8 },
            )
            .await
            .unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_millis(50), health.wait())
                .await
                .unwrap()
                .unwrap(),
            7
        );
        release.store(true, Ordering::Release);
        process.join_process(true).await.unwrap();
        assert!(!live.load(Ordering::Acquire));
        drop(process);
        assert_eq!(budget.observation().used.memory_bytes, baseline);
    }

    #[test]
    fn pyrefly_preparation_unavailable_has_an_exact_typed_status() {
        let detail = b"codefabric.pyrefly.preparation-unavailable.v1";
        let status = Status::with_details(
            tonic::Code::FailedPrecondition,
            "missing captured bundle",
            prost::bytes::Bytes::from_static(detail),
        );
        let error = context_open_error(status);
        assert!(matches!(error, PyreflyServiceError::PreparationUnavailable));
        assert_eq!(error.run_gap(), Some(PyreflyRunGap::PreparationUnavailable));
        for status in [
            Status::failed_precondition("codefabric.pyrefly.preparation-unavailable.v1"),
            Status::with_details(
                tonic::Code::Internal,
                "missing captured bundle",
                prost::bytes::Bytes::from_static(detail),
            ),
            Status::with_details(
                tonic::Code::FailedPrecondition,
                "missing captured bundle",
                prost::bytes::Bytes::from_static(b"unknown-detail"),
            ),
        ] {
            assert!(matches!(
                context_open_error(status),
                PyreflyServiceError::Protocol(_)
            ));
        }
    }

    #[test]
    fn call_definition_admission_rejects_wrong_sources_and_partial_coordinates() {
        use arrow_schema::{Field, Schema};
        let bytes = b"def chosen(): pass\n";
        let digest = *blake3::hash(bytes).as_bytes();
        let sources = BTreeMap::from([(
            "file:one",
            (
                digest,
                crate::source_encoding::DecodedSource::select(bytes, true).unwrap(),
            ),
        )]);
        let schema = Arc::new(Schema::new(vec![
            Field::new("target_file_id", arrow_schema::DataType::Utf8, true),
            Field::new(
                "target_content_digest",
                arrow_schema::DataType::FixedSizeBinary(32),
                true,
            ),
            Field::new("target_start_byte", arrow_schema::DataType::UInt64, true),
            Field::new("target_end_byte", arrow_schema::DataType::UInt64, true),
            Field::new("target_source_mapping", arrow_schema::DataType::Utf8, false),
        ]));
        for (file, hash, start, end, mapping, valid) in [
            (
                Some("file:one"),
                Some(digest),
                Some(4),
                Some(10),
                "exact_checker_definition",
                true,
            ),
            (
                Some("file:other"),
                Some(digest),
                Some(4),
                Some(10),
                "exact_checker_definition",
                false,
            ),
            (
                Some("file:one"),
                Some([9; 32]),
                Some(4),
                Some(10),
                "exact_checker_definition",
                false,
            ),
            (
                Some("file:one"),
                Some(digest),
                Some(4),
                Some(999),
                "exact_checker_definition",
                false,
            ),
            (
                Some("file:one"),
                None,
                Some(4),
                Some(10),
                "exact_checker_definition",
                false,
            ),
            (None, None, None, None, "definition_outside_inventory", true),
            (None, None, Some(4), None, "definition_unavailable", false),
        ] {
            let mut hashes = arrow_array::builder::FixedSizeBinaryBuilder::new(32);
            match hash {
                Some(hash) => hashes.append_value(hash).unwrap(),
                None => hashes.append_null(),
            }
            let batch = RecordBatch::try_new(
                Arc::clone(&schema),
                vec![
                    Arc::new(StringArray::from(vec![file])),
                    Arc::new(hashes.finish()),
                    Arc::new(UInt64Array::from(vec![start])),
                    Arc::new(UInt64Array::from(vec![end])),
                    Arc::new(StringArray::from(vec![mapping])),
                ],
            )
            .unwrap();
            assert_eq!(
                validate_call_definition_pins(&batch, &sources).is_ok(),
                valid
            );
        }
    }
    use crate::provider_contracts::{
        CancellationHandle, CancellationProbe, ContextIdentity, ProviderBuildIdentity,
        ProviderContextBinding, ProviderFamilyIdentity, ProviderFamilyRequest, ProviderIdentity,
        ProviderJobSpec, ProviderPolicyIdentity, ProviderProgramIdentity, ProviderProtocolIdentity,
        ProviderRelationIdentity, ProviderResourceCeilingSpec, ProviderResourceCeilings,
        ProviderRunBinding, ProviderRunIdentity, ProviderRunProvenance, ProviderSchemaIdentity,
        ProviderScopeIdentity, ProviderSourceBinding, SourceIdentity, SuiteIdentity,
    };
    use crate::provider_sandbox::{
        GeneratedSandboxProfile, ProviderLaunchRequest, ProviderProcessLimits,
        ProviderSandboxLaunchMaterial, ProviderSandboxLauncher, ProviderTrustProfile,
        SandboxCapabilityMatrix, SandboxMechanism,
    };
    use crate::rpc::generated::codefabric::provider::v1::{
        CancelAcknowledgement, CapabilityOutcome,
    };
    use crate::rpc::generated::codefabric::pyrefly::v1::analyze_event::Event as MockEvent;
    use crate::rpc::generated::codefabric::pyrefly::v1::pyrefly_sidecar_server::{
        PyreflySidecar, PyreflySidecarServer,
    };
    use crate::rpc::generated::codefabric::pyrefly::v1::{
        AnalyzeEvent, CloseContextRequest, CloseContextResponse, HelloAck, ModuleBegin,
        OpenContextResponse, RelationIpcFrameEvent, RunAccepted, RunProgress, RunTerminal,
        ShutdownRequest, ShutdownResponse,
    };

    fn test_job(
        context_manifest: &[u8],
        generation: u64,
        run_marker: u8,
    ) -> (ProviderJob, CancellationHandle) {
        let inventory = crate::provider_contracts::ProviderSourceInventory::try_new(
            [1; 16],
            generation,
            [2; 32],
            &[b"module.py".to_vec()],
            vec![crate::provider_contracts::ProviderInventoryMember {
                relative_path: b"module.py".to_vec(),
                disposition: crate::provider_contracts::ProviderInputDisposition::Captured {
                    file_id: [4; 16],
                    digest: *blake3::hash(b"value: int = 1\n").as_bytes(),
                    byte_length: 15,
                },
                selected_for_provider: true,
            }],
            vec![b"module.py".to_vec()],
            None,
        )
        .unwrap();
        test_inventory_job(context_manifest, run_marker, inventory)
    }

    fn test_inventory_job(
        context_manifest: &[u8],
        run_marker: u8,
        inventory: crate::provider_contracts::ProviderSourceInventory,
    ) -> (ProviderJob, CancellationHandle) {
        let generation = inventory.source_generation();
        let requested_units = u64::try_from(inventory.selected_files().count()).unwrap();
        let modules = inventory
            .members()
            .iter()
            .filter(|member| member.selected_for_provider)
            .map(|member| {
                let crate::provider_contracts::ProviderInputDisposition::Captured {
                    file_id, ..
                } = member.disposition
                else {
                    unreachable!()
                };
                crate::provider_contracts::ProviderModuleBinding {
                    file_id,
                    qualified_name: Path::new(std::str::from_utf8(&member.relative_path).unwrap())
                        .file_stem()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_owned(),
                    relative_path: member.relative_path.clone(),
                }
            })
            .collect();
        let (cancellation, probe) = CancellationProbe::pair(1).unwrap();
        let requests = PyreflyRelation::ALL
            .into_iter()
            .map(|relation| {
                ProviderFamilyRequest::try_new(
                    ProviderFamilyIdentity::try_new(format!(
                        "codefabric.provider-family.v2.3.{}",
                        relation.relation_id()
                    ))
                    .unwrap(),
                    ProviderRelationIdentity::try_new(relation.relation_id()).unwrap(),
                    ProviderSchemaIdentity::try_new(format!(
                        "codefabric.provider-schema.v2.3.{}",
                        relation.relation_id()
                    ))
                    .unwrap(),
                    relation.schema(),
                    ProviderScopeIdentity::try_new("workspace:test").unwrap(),
                    requested_units,
                )
                .unwrap()
            })
            .collect();
        let ceilings = ProviderResourceCeilings::try_new(ProviderResourceCeilingSpec {
            max_relations: PyreflyRelation::ALL.len(),
            max_batches_per_relation: MAX_MODULES_PER_RUN,
            max_input_bytes: 64 * 1024 * 1024,
            max_rows: MAX_RELATION_ROWS,
            max_bytes: u64::try_from(MAX_TOTAL_RELATION_BYTES).unwrap(),
            max_diagnostics: 1_000,
            max_work_units: 1_000_000,
            max_wall_millis: 30_000,
            max_visited_nodes: 1_000_000,
            max_traversal_depth: 256,
            max_workers: 2,
            max_retained_revisions: 1,
            cancellation_poll_work_units: 1,
            cancellation_ack_millis: 2_000,
        })
        .unwrap();
        let workspace_id = inventory.workspace_id();
        let job = ProviderJob::try_new(ProviderJobSpec {
            resource_budget: crate::provider_contracts::fixture_provider_budget(
                inventory.workspace_id(),
                [run_marker; 16],
            ),
            suite: SuiteIdentity::try_new("codefabric-relational-data-fabric@2.3").unwrap(),
            provider: ProviderIdentity::try_new("pyrefly-python").unwrap(),
            protocol: ProviderProtocolIdentity::try_new("codefabric.pyrefly.provider.v1").unwrap(),
            source: ProviderSourceBinding::from_inventory_fixture(
                SourceIdentity::try_new(format!("source:{generation}")).unwrap(),
                inventory,
            ),
            context: crate::provider_contracts::fixture_provider_context(
                workspace_id,
                ProviderContextBinding::try_new(
                    ContextIdentity::try_new("context:test").unwrap(),
                    [3; 16],
                    [3; 32],
                    *blake3::hash(context_manifest).as_bytes(),
                )
                .unwrap()
                .with_modules(modules)
                .unwrap(),
            ),
            run: ProviderRunBinding::try_new(
                ProviderRunIdentity::try_new(format!("run:{run_marker}")).unwrap(),
                [run_marker; 16],
            )
            .unwrap(),
            lane: ProviderLane::Pyrefly,
            trust: ProviderTrustPosture::LocalSidecarConstrained,
            requests,
            ceilings,
            deadline: Instant::now() + Duration::from_secs(30),
            cancellation: probe,
            provenance: ProviderRunProvenance::new(
                ProviderBuildIdentity::try_new("pyrefly-sidecar-pinned-source").unwrap(),
                ProviderPolicyIdentity::try_new("policy:test").unwrap(),
                ProviderProgramIdentity::try_new("program:test").unwrap(),
            ),
        })
        .unwrap();
        (job, cancellation)
    }

    fn test_workspace_input(root: &Path, context_manifest: &[u8]) -> PyreflyWorkspaceInput {
        let source = root.join("module.py");
        std::fs::write(&source, b"value: int = 1\n").unwrap();
        PyreflyWorkspaceInput {
            workspace_id: crate::identity::encode_public_id(
                crate::identity::IdentityDomain::Workspace,
                None,
                [1; 16],
            )
            .unwrap(),
            canonical_workspace_id: [1; 16],
            context_manifest: context_manifest.to_vec(),
            source_snapshot_lease_id: "lease:test".to_owned(),
            changed_module_ids: vec![
                crate::identity::encode_public_id(
                    crate::identity::IdentityDomain::SourceFile,
                    None,
                    [4; 16],
                )
                .unwrap(),
            ],
            modules: vec![PyreflyModuleInput {
                module_id: crate::identity::encode_public_id(
                    crate::identity::IdentityDomain::SourceFile,
                    None,
                    [4; 16],
                )
                .unwrap(),
                module_name: "module".to_owned(),
                file_id: crate::identity::encode_public_id(
                    crate::identity::IdentityDomain::SourceFile,
                    None,
                    [4; 16],
                )
                .unwrap(),
                source_blob_path: source.clone(),
                provider_source_blob_path: source.clone(),
                content_digest: b3(&std::fs::read(source).unwrap()),
            }],
        }
    }

    async fn connect_wire_client(socket: &Path) -> WireClient<Channel> {
        let socket = socket.to_path_buf();
        let channel = Endpoint::from_static("http://[::]:50051")
            .connect_with_connector(service_fn(move |_| {
                let socket = socket.clone();
                async move { UnixStream::connect(socket).await.map(TokioIo::new) }
            }))
            .await
            .unwrap();
        WireClient::new(channel)
            .max_decoding_message_size(4 * 1024 * 1024)
            .max_encoding_message_size(4 * 1024 * 1024)
    }

    enum TestSidecarProcess {
        Direct(Child),
        #[cfg(target_os = "linux")]
        Contained(ProviderProcessGroupChild),
    }
    impl TestSidecarProcess {
        fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
            match self {
                Self::Direct(child) => child.try_wait(),
                #[cfg(target_os = "linux")]
                Self::Contained(child) => child.try_wait(),
            }
        }
    }
    struct TestChild(Option<TestSidecarProcess>);
    impl Drop for TestChild {
        fn drop(&mut self) {
            match &mut self.0 {
                Some(TestSidecarProcess::Direct(child)) => {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                #[cfg(target_os = "linux")]
                Some(TestSidecarProcess::Contained(child)) => {
                    let _ = child.kill_group();
                    let _ = child.wait_group_empty(Duration::from_secs(3));
                    let _ = child.wait();
                }
                None => {}
            }
        }
    }

    #[derive(Clone, Copy)]
    enum MalformedMode {
        StaleGeneration,
        MissingModuleEnd,
        WrongArrowUniverse,
    }

    struct MalformedSidecar {
        mode: MalformedMode,
        source_manifest_digest: String,
    }

    #[tonic::async_trait]
    impl PyreflySidecar for MalformedSidecar {
        async fn handshake(&self, request: Request<Hello>) -> Result<Response<HelloAck>, Status> {
            let request = request.into_inner();
            Ok(Response::new(HelloAck {
                protocol_major: 1,
                protocol_minor: 0,
                negotiated_feature_bits: REQUIRED_FEATURE_BITS | OPTIONAL_FEATURE_BITS,
                sidecar_build: "malformed-test-sidecar".to_owned(),
                pyrefly_source_digest: PYREFLY_SOURCE_DIGEST.to_owned(),
                supported_python_versions: request.supported_python_versions,
                observation_schema_digests: request.observation_schema_digests,
                maximum_frame_bytes: request.maximum_frame_bytes,
                maximum_arrow_ipc_bytes: request.maximum_arrow_ipc_bytes,
                sandbox_profile_digest: request.sandbox_profile_digest,
            }))
        }

        async fn open_context(
            &self,
            request: Request<OpenContextRequest>,
        ) -> Result<Response<OpenContextResponse>, Status> {
            let request = request.into_inner();
            Ok(Response::new(OpenContextResponse {
                context_handle: "malformed-context".to_owned(),
                context_manifest_digest: request.context_manifest_digest,
                opened_at_unix_ms: 1,
                sandbox_profile_digest: request.sandbox_profile_digest,
            }))
        }

        type AnalyzeModulesStream =
            Pin<Box<dyn Stream<Item = Result<AnalyzeEvent, Status>> + Send + 'static>>;

        async fn analyze_modules(
            &self,
            request: Request<tonic::Streaming<AnalyzeCommand>>,
        ) -> Result<Response<Self::AnalyzeModulesStream>, Status> {
            let mut commands = request.into_inner();
            let command = commands
                .message()
                .await?
                .ok_or_else(|| Status::invalid_argument("missing start"))?;
            let Some(Command::Start(start)) = command.command else {
                return Err(Status::invalid_argument("first command is not start"));
            };
            let header = |sequence, generation| AnalyzeEventHeader {
                provider_run_id: start.provider_run_id.clone(),
                workspace_id: start.workspace_id.clone(),
                analysis_context_id: start.analysis_context_id.clone(),
                source_generation: generation,
                sequence,
                context_manifest_digest: start.context_manifest_digest.clone(),
                source_manifest_digest: self.source_manifest_digest.clone(),
                sandbox_profile_digest: start.sandbox_profile_digest.clone(),
            };
            let accepted_generation = if matches!(self.mode, MalformedMode::StaleGeneration) {
                start.source_generation.saturating_sub(1)
            } else {
                start.source_generation
            };
            let mut events = vec![AnalyzeEvent {
                event: Some(MockEvent::RunAccepted(RunAccepted {
                    header: Some(header(0, accepted_generation)),
                    granted_frame_credits: 4,
                    granted_credit_bytes: MAX_UNACKNOWLEDGED_BYTES,
                })),
            }];
            if !matches!(self.mode, MalformedMode::StaleGeneration) {
                let module_id = start.modules[0].module_id.clone();
                events.push(AnalyzeEvent {
                    event: Some(MockEvent::RunProgress(RunProgress {
                        header: Some(header(1, start.source_generation)),
                        completed_modules: 0,
                        total_modules: u32::try_from(start.modules.len()).unwrap(),
                    })),
                });
                events.push(AnalyzeEvent {
                    event: Some(MockEvent::ModuleBegin(ModuleBegin {
                        header: Some(header(2, start.source_generation)),
                        module_id: module_id.clone(),
                    })),
                });
                match self.mode {
                    MalformedMode::MissingModuleEnd => {
                        events.push(AnalyzeEvent {
                            event: Some(MockEvent::RunTerminal(RunTerminal {
                                removed_module_ids: Vec::new(),
                                header: Some(header(3, start.source_generation)),
                                ordered_module_digests: Vec::new(),
                                capability_outcomes: start
                                    .requested_capability_codes
                                    .iter()
                                    .map(|code| CapabilityOutcome {
                                        capability_code: *code,
                                        owner_capability_state_code: 10,
                                        completeness_state_code: 10,
                                        reason_code: "PYREFLY_SUCCEEDED".to_owned(),
                                    })
                                    .collect(),
                                overall_digest: b3(&[]),
                                terminal_state: WireProviderRunState::Succeeded as i32,
                                rechecked_module_ids: start
                                    .modules
                                    .iter()
                                    .map(|module| module.module_id.clone())
                                    .collect(),
                                sandbox_profile_digest: start.sandbox_profile_digest.clone(),
                                trust_profile: TRUST_PROFILE.to_owned(),
                            })),
                        });
                    }
                    MalformedMode::WrongArrowUniverse => {
                        let relation = PyreflyRelation::ModuleContext;
                        let identity = crate::relation_ipc_contract::relation_wire_identity(
                            relation.relation_id(),
                            &relation.schema_digest(),
                            &start.provider_run_id,
                            &module_id,
                            &self.source_manifest_digest,
                            &start.context_manifest_digest,
                        )
                        .unwrap();
                        let batch = RecordBatch::new_empty(relation.schema());
                        let mut arrow_ipc = Vec::new();
                        {
                            let mut writer =
                                StreamWriter::try_new(&mut arrow_ipc, &batch.schema()).unwrap();
                            writer.write(&batch).unwrap();
                            writer.finish().unwrap();
                        }
                        let mut frame = crate::relation_ipc_proto::encode_relation_frames(
                            identity,
                            &arrow_ipc,
                            1,
                            0,
                            &crate::relation_ipc_proto::RelationCoverage::complete(1),
                        )
                        .unwrap()
                        .remove(0);
                        let Some(
                            crate::rpc::generated::codefabric::provider::v1::relation_ipc_frame::Frame::Open(
                                open,
                            ),
                        ) = frame.frame.as_mut()
                        else {
                            unreachable!("first relation frame is open")
                        };
                        open.arrow_type_universe = "arrow-array@60.0.0".to_owned();
                        events.push(AnalyzeEvent {
                            event: Some(MockEvent::RelationIpcFrame(RelationIpcFrameEvent {
                                header: Some(header(3, start.source_generation)),
                                module_id,
                                observation_family_code: relation.family_code(),
                                frame: Some(frame),
                            })),
                        });
                    }
                    MalformedMode::StaleGeneration => unreachable!(),
                }
            }
            Ok(Response::new(Box::pin(tokio_stream::iter(
                events.into_iter().map(Ok),
            ))))
        }

        async fn cancel_run(
            &self,
            request: Request<CancelRunRequest>,
        ) -> Result<Response<CancelAcknowledgement>, Status> {
            Ok(Response::new(CancelAcknowledgement {
                provider_run_id: request.into_inner().provider_run_id,
                state: CancelAcknowledgementState::NotFound as i32,
                acknowledged_at_unix_ms: 1,
                terminal_state: None,
                cleaning_up_components: Vec::new(),
                forced_termination: false,
            }))
        }

        async fn close_context(
            &self,
            _request: Request<CloseContextRequest>,
        ) -> Result<Response<CloseContextResponse>, Status> {
            Ok(Response::new(CloseContextResponse { closed: true }))
        }

        async fn shutdown(
            &self,
            _request: Request<ShutdownRequest>,
        ) -> Result<Response<ShutdownResponse>, Status> {
            Ok(Response::new(ShutdownResponse { accepted: true }))
        }
    }

    fn malformed_request(root: &Path) -> PyreflyRunRequest {
        let source = root.join("module.py");
        std::fs::write(&source, b"value: int = 1\n").unwrap();
        PyreflyRunRequest {
            resource_budget: crate::provider_contracts::fixture_provider_budget([1; 16], [1; 16]),
            max_input_bytes: 64 * 1024 * 1024,
            max_output_bytes: 128 * 1024 * 1024,
            changed_module_ids: vec!["module-malformed".to_owned()],
            expected_removed_module_ids: Vec::new(),
            provider_run_id: "11111111111111111111111111111111".to_owned(),
            workspace_id: crate::identity::encode_public_id(
                crate::identity::IdentityDomain::Workspace,
                None,
                [1; 16],
            )
            .unwrap(),
            analysis_context_id: crate::identity::encode_public_id(
                crate::identity::IdentityDomain::AnalysisContext,
                None,
                [2; 16],
            )
            .unwrap(),
            canonical_workspace_id: [1; 16],
            canonical_analysis_context_id: [2; 16],
            source_generation: 7,
            context_manifest: b"{\"python\":\"3.14\"}".to_vec(),
            source_snapshot_lease_id: "lease-malformed".to_owned(),
            source_manifest_digest: b3(b"source-manifest"),
            modules: vec![PyreflyModuleInput {
                module_id: "module-malformed".to_owned(),
                module_name: "module".to_owned(),
                file_id: crate::identity::encode_public_id(
                    crate::identity::IdentityDomain::SourceFile,
                    None,
                    [3; 16],
                )
                .unwrap(),
                source_blob_path: source.clone(),
                provider_source_blob_path: source.clone(),
                content_digest: b3(&std::fs::read(source).unwrap()),
            }],
            requested_capability_codes: vec![90],
            deadline_unix_ms: i64::MAX,
            sandbox_profile_digest: format!("sha256:{}", "11".repeat(32)),
            output_schema_bundle_digest: schema_bundle_digest(),
        }
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Three adversarial servers exercise the production stream validator end to end.
    async fn wp34_ops_pyrefly_stale_generation_rejection_falsification() {
        let directory = tempfile::tempdir().unwrap();
        let request = malformed_request(directory.path());
        let mut failures = Vec::new();
        for (index, mode) in [
            MalformedMode::StaleGeneration,
            MalformedMode::MissingModuleEnd,
            MalformedMode::WrongArrowUniverse,
        ]
        .into_iter()
        .enumerate()
        {
            let socket = PathBuf::from(format!(
                "/tmp/cfpy-malformed-{}-{index}.sock",
                std::process::id()
            ));
            let _ = std::fs::remove_file(&socket);
            let listener = tokio::net::UnixListener::bind(&socket).unwrap();
            let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
            let service = MalformedSidecar {
                mode,
                source_manifest_digest: request.source_manifest_digest.clone(),
            };
            let server = tokio::spawn(async move {
                tonic::transport::Server::builder()
                    .add_service(PyreflySidecarServer::new(service))
                    .serve_with_incoming_shutdown(UnixListenerStream::new(listener), async move {
                        let _ = shutdown_receiver.await;
                    })
                    .await
            });
            let client = connect_wire_client(&socket).await;
            let error =
                analyze_pyrefly_uds_inner(client, &request, Arc::new(AtomicBool::new(false)), None)
                    .await
                    .unwrap_err();
            failures.push(error.to_string());
            let _ = shutdown_sender.send(());
            server.await.unwrap().unwrap();
            let _ = std::fs::remove_file(socket);
        }
        assert!(failures[0].contains("accepted correlation or sequence differs"));
        assert!(failures[1].contains("run terminal identity, order, or state differs"));
        assert!(failures[2].contains("relation protobuf envelope is invalid"));
        assert_eq!(
            failures.len(),
            3,
            "no malformed stream reached an accepted run"
        );
    }

    #[tokio::test]
    async fn chunked_inventory_upload_advances_with_a_single_queued_frame() {
        let modules = (0..193)
            .map(|index| ModuleRequest {
                module_id: format!("module-{index}"),
                module_name: format!("m{index}"),
                ..ModuleRequest::default()
            })
            .collect::<Vec<_>>();
        let changed = modules
            .iter()
            .map(|module| module.module_id.clone())
            .collect::<Vec<_>>();
        let mut start = AnalyzeModulesRequest {
            expected_module_count: Some(modules.len() as u32),
            deadline_unix_ms: now_unix_millis() + 2_000,
            ..AnalyzeModulesRequest::default()
        };
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        let mut commands = tokio_stream::StreamExt::map(ReceiverStream::new(receiver), Ok);
        let (sent, received) = tokio::time::timeout(Duration::from_secs(1), async {
            tokio::join!(
                send_inventory(&sender, &modules, &changed, true),
                inventory_stream::receive_inventory(&mut start, &mut commands)
            )
        })
        .await
        .expect("upload cannot wait for RPC response headers");
        sent.unwrap();
        received.unwrap();
        assert_eq!(start.modules, modules);
        assert_eq!(start.changed_module_ids, changed);
    }

    #[tokio::test]
    async fn incomplete_inventory_upload_never_replaces_the_start_inventory() {
        let chunk = AnalyzeCommand {
            command: Some(Command::InventoryChunk(AnalyzeInventoryChunk {
                sequence: 0,
                modules: vec![ModuleRequest {
                    module_id: "one".to_owned(),
                    ..ModuleRequest::default()
                }],
                changed_module_ids: vec![],
            })),
        };
        let end = AnalyzeCommand {
            command: Some(Command::InventoryEnd(AnalyzeInventoryEnd {
                chunk_count: 1,
                module_count: 2,
                changed_module_count: 0,
            })),
        };
        for commands in [
            vec![chunk.clone()],
            vec![chunk.clone(), chunk.clone(), end.clone()],
            vec![chunk, end],
        ] {
            let mut start = AnalyzeModulesRequest {
                provider_run_id: "run".to_owned(),
                expected_module_count: Some(2),
                deadline_unix_ms: now_unix_millis() + 2_000,
                ..AnalyzeModulesRequest::default()
            };
            let mut commands = tokio_stream::iter(commands.into_iter().map(Ok));
            assert!(
                inventory_stream::receive_inventory(&mut start, &mut commands)
                    .await
                    .is_err()
            );
            assert!(start.modules.is_empty());
            assert!(start.changed_module_ids.is_empty());
        }
        let mut start = AnalyzeModulesRequest {
            provider_run_id: "run".to_owned(),
            expected_module_count: Some(2),
            deadline_unix_ms: now_unix_millis() + 25,
            ..AnalyzeModulesRequest::default()
        };
        let mut commands = futures::stream::pending();
        assert_eq!(
            inventory_stream::receive_inventory(&mut start, &mut commands)
                .await
                .unwrap_err()
                .code(),
            tonic::Code::DeadlineExceeded
        );
        start.deadline_unix_ms = now_unix_millis() + 2_000;
        let mut commands = tokio_stream::iter([Ok(AnalyzeCommand {
            command: Some(Command::Cancel(CancelRunRequest {
                provider_run_id: "run".to_owned(),
                reason: "cancel".to_owned(),
            })),
        })]);
        assert_eq!(
            inventory_stream::receive_inventory(&mut start, &mut commands)
                .await
                .unwrap_err()
                .code(),
            tonic::Code::Cancelled
        );
        assert!(start.modules.is_empty());
    }

    #[tokio::test]
    async fn wp34_ops_real_pyrefly_shutdown_joins_serving_process() {
        real_pyrefly_session(false, 1).await;
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn contained_pyrefly_reads_mapped_blobs_and_returns_real_semantics() {
        real_pyrefly_session(true, 1).await;
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn contained_pyrefly_chunked_inventory_resolves_across_chunk_boundaries() {
        real_pyrefly_session(true, 70).await;
    }

    async fn real_pyrefly_session(contained: bool, module_count: usize) {
        let executable = std::env::var_os("CODEFABRIC_PYREFLY_SIDECAR_BIN")
            .expect("relation-IPC operations gate must supply the built sidecar binary");
        let executable = std::fs::canonicalize(executable).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let input_root = directory.path().join("view");
        let output_root = directory.path().join("output");
        std::fs::create_dir(&input_root).unwrap();
        std::fs::create_dir(&output_root).unwrap();
        let socket = output_root.join("pyrefly.sock");
        let mut sandbox_profile_digest = format!("sha256:{}", "11".repeat(32));
        let child = if contained {
            #[cfg(target_os = "linux")]
            {
                let profile = GeneratedSandboxProfile::generate(
                    ProviderTrustProfile::UntrustedSandboxed,
                    SandboxMechanism::LinuxBubblewrap,
                    &input_root,
                    executable.parent().unwrap(),
                    &output_root,
                )
                .unwrap();
                sandbox_profile_digest = profile.sha256_digest.clone();
                let policy = crate::provider_sandbox::CompiledProviderSeccomp::compile().unwrap();
                let request = ProviderLaunchRequest {
                    host_executable: executable.clone(),
                    contained_executable: Path::new("/dependencies")
                        .join(executable.file_name().unwrap()),
                    arguments: vec!["--serve".into(), "unix:///output/pyrefly.sock".into()],
                    environment: BTreeMap::from([("PATH".into(), "/usr/bin:/bin".into())]),
                    output_root: output_root.clone(),
                    limits: ProviderProcessLimits {
                        cpu_seconds: 30,
                        open_files: 256,
                        resident_memory_bytes: 16 * 1024 * 1024 * 1024,
                        output_file_bytes: 256 * 1024 * 1024,
                        process_count: 256,
                    },
                };
                TestSidecarProcess::Contained(
                    ProviderSandboxLauncher::new(SandboxCapabilityMatrix::probe_current_host())
                        .launch(
                            &request,
                            &profile,
                            ProviderSandboxLaunchMaterial::LinuxSeccomp(&policy),
                        )
                        .unwrap(),
                )
            }
            #[cfg(not(target_os = "linux"))]
            {
                panic!("this contained process fixture requires Linux");
            }
        } else {
            TestSidecarProcess::Direct(
                ProcessCommand::new(executable)
                    .arg("--serve")
                    .arg(format!("unix://{}", socket.display()))
                    .env("CODEFABRIC_SANDBOX_PROFILE_DIGEST", &sandbox_profile_digest)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap(),
            )
        };
        let mut child = TestChild(Some(child));
        tokio::time::timeout(Duration::from_secs(5), async {
            while !socket.exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("sidecar must bind its private UDS within the startup bound");

        let mut client = connect_wire_client(&socket).await;
        validate_handshake(&mut client, &sandbox_profile_digest)
            .await
            .unwrap();
        // Cross the real deployed context and Arrow protocol boundaries; process startup
        // alone cannot detect a checker which ignores the configured Python version.
        let source = b"import sys\ndef previous() -> int:\n    return 1\ndef current() -> str:\n    return 'current'\nif sys.version_info < (3, 14):\n    selected = previous\nelse:\n    selected = current\nvalue = selected()\n";
        let mut input = test_workspace_input(&input_root, b"{}");
        if contained {
            input.modules[0].provider_source_blob_path = "/workspace/module.py".into();
        }
        let mut source = source.to_vec();
        if module_count > 1 {
            source.extend_from_slice(
                format!(
                    "from extra_{} import chosen\nother = chosen()\n",
                    module_count - 1
                )
                .as_bytes(),
            );
        }
        std::fs::write(&input.modules[0].source_blob_path, &source).unwrap();
        input.modules[0].content_digest = b3(&source);
        for index in 1..module_count {
            let name = format!("extra_{index}");
            let file = format!("{name}.py");
            let path = input_root.join(&file);
            let bytes = b"def chosen() -> int:\n    return 42\n";
            std::fs::write(&path, bytes).unwrap();
            let id = crate::identity::encode_public_id(
                crate::identity::IdentityDomain::SourceFile,
                None,
                (index as u128).to_be_bytes(),
            )
            .unwrap();
            input.modules.push(PyreflyModuleInput {
                module_id: id.clone(),
                file_id: id,
                module_name: name,
                source_blob_path: path.clone(),
                provider_source_blob_path: if contained {
                    Path::new("/workspace").join(file)
                } else {
                    path
                },
                content_digest: b3(bytes),
            });
        }
        let module_map = input.modules.iter().map(|module| serde_json::json!({
            "module_name": module.module_name, "file_id": module.file_id,
            "relative_path": module.source_blob_path.file_name().unwrap().as_encoded_bytes(),
            "root_id": "workspace", "is_stub": false, "is_package": false,
        })).collect::<Vec<_>>();
        let manifest = serde_json::to_vec(&serde_json::json!({
            "context_kind": "python", "python_language_version": "3.14",
            "implementation_profile": "cpython-semantics", "platform_tag": "linux",
            "module_roots": ["workspace"], "source_roots": [], "stub_roots": [], "dependency_roots": [],
            "namespace_package_policy": "pep420", "import_precedence": ["explicit-stub-roots", "workspace-module-roots", "workspace-source-roots", "authorized-dependency-roots", "typeshed-stdlib", "typeshed-third-party"],
            "typeshed_bundle_digest": null, "lockfile_artifacts": [], "project_config_artifacts": [],
            "pyrefly_bundle_digest": null, "ruff_bundle_digest": b3(b"fixture-ruff"),
            "provider_bundle_version": "configured-context-v1", "platforms": ["linux"],
            "root_bindings": [{"root_id": "workspace", "relative_path": [46]}],
            "module_map": module_map,
            "configuration_namespace": [46], "configuration_roots": [{"root_id": "workspace", "relative_path": [46]}],
            "configuration_policy_identity": vec![1; 32]
        })).unwrap();
        input.context_manifest = manifest.clone();
        // Exercise a changed-file hint within a larger complete inventory.
        let changed_paths = vec![b"module.py".to_vec()];
        let inventory = crate::provider_contracts::ProviderSourceInventory::try_new(
            [1; 16],
            1,
            [2; 32],
            &input
                .modules
                .iter()
                .map(|module| {
                    module
                        .source_blob_path
                        .file_name()
                        .unwrap()
                        .as_encoded_bytes()
                        .to_vec()
                })
                .collect::<Vec<_>>(),
            input
                .modules
                .iter()
                .map(|module| {
                    let bytes = std::fs::read(&module.source_blob_path).unwrap();
                    crate::provider_contracts::ProviderInventoryMember {
                        relative_path: module
                            .source_blob_path
                            .file_name()
                            .unwrap()
                            .as_encoded_bytes()
                            .to_vec(),
                        disposition:
                            crate::provider_contracts::ProviderInputDisposition::Captured {
                                file_id: crate::identity::decode_public_id(
                                    crate::identity::IdentityDomain::SourceFile,
                                    None,
                                    &module.file_id,
                                )
                                .unwrap(),
                                digest: *blake3::hash(&bytes).as_bytes(),
                                byte_length: bytes.len() as u64,
                            },
                        selected_for_provider: true,
                    }
                })
                .collect(),
            changed_paths,
            None,
        )
        .unwrap();
        let (job, _cancel) = test_inventory_job(&manifest, 1, inventory);
        let request = request_from_job(
            &job,
            &input,
            &sandbox_profile_digest,
            Duration::from_secs(30),
        )
        .unwrap();
        let accepted = until_job_stop(
            &job,
            analyze_pyrefly_uds_inner(
                client.clone(),
                &request,
                Arc::new(AtomicBool::new(false)),
                None,
            ),
        )
        .await
        .unwrap();
        assert_eq!(accepted.modules.len(), module_count);
        let targets = accepted
            .modules
            .iter()
            .find(|module| module.module_id == input.modules[0].module_id)
            .unwrap()
            .relations
            .iter()
            .find(|r| r.relation == PyreflyRelation::CallTarget)
            .unwrap();
        let targets = targets
            .batch
            .column_by_name("qualified_target")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let mut expected = std::collections::BTreeSet::from(["module.current".to_owned()]);
        if module_count > 1 {
            expected.insert(format!("extra_{}.chosen", module_count - 1));
        }
        assert_eq!(
            targets
                .iter()
                .flatten()
                .map(ToOwned::to_owned)
                .collect::<std::collections::BTreeSet<_>>(),
            expected
        );
        PyreflyProviderRunResult::try_new(&job, accepted).unwrap();
        assert!(
            client
                .shutdown(ShutdownRequest {
                    reason: "test-workspace-drain".to_owned(),
                })
                .await
                .unwrap()
                .into_inner()
                .accepted
        );

        let status = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(status) = child.0.as_mut().unwrap().try_wait().unwrap() {
                    break status;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("cooperative shutdown must join the real sidecar process");
        assert!(status.success());
        child.0 = None;
        assert!(!socket.exists(), "joined sidecar must remove its UDS");
    }

    #[test]
    fn pyrefly_complete_inventory_and_empty_withdrawal_contract() {
        use crate::provider_contracts::{
            ProviderPartitionAction, ProviderSourceInventory, ProviderSourceSelection,
            admit_provider_result,
        };
        let root = tempfile::tempdir().unwrap();
        let manifest = b"{\"python\":\"3.14\"}";
        let (job, _owner) = test_job(manifest, 7, 7);
        let input = test_workspace_input(root.path(), manifest);
        let sandbox = format!("sha256:{}", "11".repeat(32));
        let request = request_from_job(&job, &input, &sandbox, Duration::from_secs(30)).unwrap();
        assert_eq!(request.canonical_analysis_context_id, [3; 16]);
        assert_eq!(request.changed_module_ids.len(), 1);
        let previous_checker_modules = request
            .modules
            .iter()
            .map(|module| module.module_id.clone())
            .collect();
        assert!(checker_removals(&previous_checker_modules, &request.modules).is_empty());
        let mut invalid = input.clone();
        invalid.modules.clear();
        assert!(request_from_job(&job, &invalid, &sandbox, Duration::from_secs(30)).is_err());
        let mut invalid = input.clone();
        invalid.changed_module_ids.clear();
        assert!(request_from_job(&job, &invalid, &sandbox, Duration::from_secs(30)).is_err());
        invalid = input.clone();
        invalid.modules[0].module_name = "wrong.qualified.name".to_owned();
        assert!(request_from_job(&job, &invalid, &sandbox, Duration::from_secs(30)).is_err());
        invalid = input.clone();
        invalid.canonical_workspace_id = [4; 16];
        assert!(request_from_job(&job, &invalid, &sandbox, Duration::from_secs(30)).is_err());

        let ProviderSourceSelection::Inventory(previous) = job.source().selection() else {
            unreachable!()
        };
        let empty = ProviderSourceInventory::try_new(
            [1; 16],
            8,
            [8; 32],
            &[],
            vec![],
            vec![],
            Some(previous),
        )
        .unwrap();
        let (deleted, _deleted_owner) = test_inventory_job(manifest, 8, empty);
        let mut input = input;
        input.modules.clear();
        input.changed_module_ids.clear();
        let request =
            request_from_job(&deleted, &input, &sandbox, Duration::from_secs(30)).unwrap();
        assert!(request.modules.is_empty());
        assert!(
            request.expected_removed_module_ids.is_empty(),
            "a fresh checker has no local removals even though CPG partitions must withdraw"
        );
        assert_eq!(
            checker_removals(&previous_checker_modules, &request.modules).len(),
            1
        );
        let accepted = AcceptedPyreflyRun {
            provider_run_id: request.provider_run_id,
            workspace_id: request.workspace_id,
            analysis_context_id: request.analysis_context_id,
            canonical_workspace_id: request.canonical_workspace_id,
            canonical_analysis_context_id: request.canonical_analysis_context_id,
            source_generation: request.source_generation,
            modules: crate::resource_budget::ChargedSlice::for_test(vec![]),
            capability_codes: crate::resource_budget::ChargedSlice::for_test(
                request.requested_capability_codes,
            ),
            overall_digest: b3(b""),
            rechecked_module_ids: crate::resource_budget::ChargedSlice::for_test(vec![]),
            removed_module_ids: crate::resource_budget::ChargedSlice::for_test(
                request.expected_removed_module_ids,
            ),
            sandbox_profile_digest: sandbox,
            trust_profile: TRUST_PROFILE.to_owned(),
        };
        let mut wrong_context = accepted.clone();
        wrong_context.canonical_analysis_context_id = [99; 16];
        assert!(PyreflyProviderRunResult::try_new(&deleted, wrong_context).is_err());
        let empty_result = PyreflyProviderRunResult::try_new(&deleted, accepted).unwrap();
        assert_eq!(
            empty_result.result().relations().len(),
            PyreflyRelation::ALL.len()
        );
        assert!(empty_result.result().relations().iter().all(|relation| {
            relation.batches().iter().all(|batch| batch.num_rows() == 0)
                && !relation.batches().is_empty()
        }));
        let admitted = admit_provider_result(deleted, empty_result.result().clone()).unwrap();
        assert_eq!(
            admitted
                .result()
                .support()
                .partitions
                .iter()
                .filter(|support| support.partition.action == ProviderPartitionAction::Withdraw)
                .count(),
            PyreflyRelation::ALL.len()
        );
    }

    #[test]
    fn pyrefly_native_state_containment_integrity() {
        let (job, _cancellation) = test_job(b"{\"python\":\"3.14\"}", 1, 1);
        let matrix = SandboxCapabilityMatrix::probe_current_host();
        match require_exact_pyrefly_containment(&job, &matrix) {
            Ok(mechanism) => assert_ne!(mechanism, SandboxMechanism::None),
            Err(error) => {
                assert!(matches!(error, PyreflyServiceError::TrustUnavailable));
                assert_eq!(error.run_gap(), Some(PyreflyRunGap::TrustUnavailable));
            }
        }
    }

    #[test]
    fn pyrefly_context_trust_and_memory_faults() {
        let root = tempfile::tempdir().unwrap();
        let context_manifest = b"{\"python\":\"3.14\",\"typeshed\":\"pinned\"}";
        let (job, _cancellation) = test_job(context_manifest, 7, 7);
        let input = test_workspace_input(root.path(), context_manifest);
        let request = request_from_job(
            &job,
            &input,
            &format!("sha256:{}", "11".repeat(32)),
            Duration::from_secs(30),
        )
        .unwrap();
        assert_eq!(request.source_generation, 7);
        assert_eq!(request.modules.len(), 1);

        let mut drifted = input.clone();
        drifted.context_manifest = b"{\"python\":\"3.13\"}".to_vec();
        assert!(matches!(
            request_from_job(
                &job,
                &drifted,
                &format!("sha256:{}", "11".repeat(32)),
                Duration::from_secs(30),
            ),
            Err(PyreflyServiceError::Invalid(_))
        ));
        let trust_gap = PyreflyProviderRunResult::gap(
            &job,
            ProviderUnknownCause::TrustLoss,
            "Pyrefly containment trust is unavailable",
        )
        .unwrap();
        assert_eq!(
            trust_gap.result().terminal(),
            ProviderTerminalStatus::Failed
        );
        assert!(trust_gap.accepted().is_none());
        assert_eq!(trust_gap.result().gaps().len(), PyreflyRelation::ALL.len());
    }

    #[test]
    fn pyrefly_cooperative_drain_reconstruction() {
        let root = tempfile::tempdir().unwrap();
        let first_manifest = b"{\"python\":\"3.14\",\"search\":[\"src\"]}";
        let second_manifest = b"{\"python\":\"3.14\",\"search\":[\"lib\"]}";
        let (first_job, cancellation) = test_job(first_manifest, 1, 1);
        let first_input = test_workspace_input(root.path(), first_manifest);
        let first_request = request_from_job(
            &first_job,
            &first_input,
            &format!("sha256:{}", "11".repeat(32)),
            Duration::from_secs(30),
        )
        .unwrap();
        let first_compatibility = context_compatibility(&first_job, &first_request);

        let (second_job, _second_cancellation) = test_job(second_manifest, 2, 2);
        let second_input = test_workspace_input(root.path(), second_manifest);
        let second_request = request_from_job(
            &second_job,
            &second_input,
            &format!("sha256:{}", "11".repeat(32)),
            Duration::from_secs(30),
        )
        .unwrap();
        assert_ne!(
            first_compatibility,
            context_compatibility(&second_job, &second_request)
        );
        cancellation.cancel();
        assert!(first_job.cancellation().is_cancelled());
        let cancelled = PyreflyProviderRunResult::gap(
            &first_job,
            ProviderUnknownCause::Cancelled,
            "Pyrefly provider run was cooperatively cancelled",
        )
        .unwrap();
        assert_eq!(
            cancelled.result().terminal(),
            ProviderTerminalStatus::Cancelled
        );
    }

    #[tokio::test]
    async fn wp34_neg_trusted_local_pyrefly_process_substitution_is_rejected() {
        rejected_process_construction(false).await;
    }

    #[tokio::test]
    async fn wp79_pyrefly_cancelled_construction_retains_actual_process_group() {
        rejected_process_construction(true).await;
    }

    async fn rejected_process_construction(cancel_construction: bool) {
        use std::os::fd::AsRawFd as _;

        struct ReleaseOnDrop(Arc<AtomicBool>);
        impl Drop for ReleaseOnDrop {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        let dependencies = root.path().join("dependencies");
        let output = root.path().join("output");
        for path in [&workspace, &dependencies, &output] {
            std::fs::create_dir(path).unwrap();
        }
        let profile = GeneratedSandboxProfile::generate(
            ProviderTrustProfile::TrustedLocal,
            SandboxMechanism::None,
            &workspace,
            &dependencies,
            &output,
        )
        .unwrap();
        let launcher = ProviderSandboxLauncher::new(SandboxCapabilityMatrix::probe_current_host());
        let (job, _cancellation) = test_job(b"{\"python\":\"3.14\"}", 1, 1);
        let cleanup =
            crate::cancellation::StructuredCancellationScope::try_root_with_control_reserve(
                "pyrefly-fixture",
                std::num::NonZeroUsize::new(8).unwrap(),
                std::num::NonZeroUsize::new(2).unwrap(),
            )
            .unwrap()
            .child_control("cleanup")
            .unwrap();
        let budget = job.resource_budget().clone();
        let baseline = budget.observation().used.memory_bytes;
        let child_pid = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let observed_pid = Arc::clone(&child_pid);
        let release = Arc::new(AtomicBool::new(false));
        let _release_on_exit = ReleaseOnDrop(Arc::clone(&release));
        let worker_release = Arc::clone(&release);
        let pinned_output = crate::secure_path::open_absolute_directory_nofollow(&output).unwrap();
        let pinned_path = PathBuf::from(format!("/proc/self/fd/{}", pinned_output.as_raw_fd()));
        let mut construction = Box::pin(SupervisedPyreflyWorkspace::try_new(
            &job,
            pinned_output,
            cleanup.clone(),
            move || {
                let child = launcher.launch(
                    &ProviderLaunchRequest {
                        host_executable: "/bin/sleep".into(),
                        contained_executable: "/bin/sleep".into(),
                        arguments: vec!["30".to_owned()],
                        environment: BTreeMap::from([(
                            "PATH".to_owned(),
                            "/usr/bin:/bin".to_owned(),
                        )]),
                        output_root: profile.output_root.clone(),
                        limits: ProviderProcessLimits {
                            cpu_seconds: 30,
                            open_files: 16,
                            resident_memory_bytes: 64 * 1024 * 1024,
                            output_file_bytes: 1024,
                            process_count: 4,
                        },
                    },
                    &profile,
                    ProviderSandboxLaunchMaterial::None,
                )?;
                observed_pid.store(child.id(), Ordering::Release);
                while cancel_construction && !worker_release.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Ok(child)
            },
        ));
        if cancel_construction {
            tokio::select! {
                _ = &mut construction => panic!("readiness cannot escape while launcher is live"),
                () = async { while child_pid.load(Ordering::Acquire) == 0 { tokio::task::yield_now().await; } } => {},
            }
            drop(construction);
            assert!(cleanup.is_cancelled());
            assert_eq!(std::fs::read_link(&pinned_path).unwrap(), output);
            assert!(budget.observation().used.memory_bytes > baseline);
            assert!(
                cleanup
                    .cancel_and_join(Duration::from_millis(5))
                    .await
                    .is_err()
            );
            release.store(true, Ordering::Release);
        } else {
            assert!(matches!(
                construction.await,
                Err(PyreflyServiceError::TrustUnavailable)
            ));
        }
        cleanup
            .cancel_and_join(Duration::from_secs(5))
            .await
            .unwrap();
        assert_ne!(
            child_pid.load(Ordering::Acquire),
            0,
            "a live process was actually launched and rejected"
        );
        let pid = rustix::process::Pid::from_raw(
            i32::try_from(child_pid.load(Ordering::Acquire)).unwrap(),
        )
        .unwrap();
        assert_eq!(
            rustix::process::test_kill_process_group(pid),
            Err(rustix::io::Errno::SRCH)
        );
        assert_eq!(budget.observation().used.memory_bytes, baseline);
        assert_ne!(
            std::fs::read_link(&pinned_path).ok().as_ref(),
            Some(&output)
        );
    }
}
