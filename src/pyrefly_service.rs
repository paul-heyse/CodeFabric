//! Stable-daemon adapter for the isolated Pyrefly sidecar.
//!
//! Only application-owned Arrow batches and correlation DTOs cross this module. No Pyrefly
//! library type is linked into or exposed by the stable daemon.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use arrow_array::{Array as _, FixedSizeBinaryArray, RecordBatch, StringArray, UInt64Array};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use tokio::net::UnixStream;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Endpoint;
use tower::service_fn;

use crate::cancellation::Cancellation;
use crate::fabric::production_kernel::CompiledProviderAuthority;
use crate::production_provider_recipe::CompiledProviderLane;
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
    AnalyzeCommand, AnalyzeEventHeader, AnalyzeModulesRequest, CancelRunRequest, Hello,
    ModuleRequest, OpenContextRequest,
};

#[path = "pyrefly_relation_schema.rs"]
mod relation_schema;

pub use relation_schema::PyreflyRelation;
pub(crate) use relation_schema::schema_bundle_digest;
use relation_schema::schema_digests;

const PYREFLY_SOURCE_DIGEST: &str =
    "b3:1b9e72144644d1b3df0bdca564496566238543dfb7f576980a8408714327fc3e";
const REQUIRED_FEATURE_BITS: u64 = (1_u64 << 17) | (1_u64 << 32);
const OPTIONAL_FEATURE_BITS: u64 = 1_u64 << 33;
const MAX_UNACKNOWLEDGED_BYTES: u64 = 16 * 1024 * 1024;
const MAX_MODULES_PER_RUN: usize = 64;
const MAX_SOURCE_BYTES_PER_MODULE: u64 = 8 * 1024 * 1024;
const MAX_SOURCE_BYTES_PER_RUN: u64 = 64 * 1024 * 1024;
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
    pub content_digest: String,
}

/// Complete identity and capability selection for one sidecar run.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PyreflyRunRequest {
    pub provider_run_id: String,
    pub workspace_id: String,
    pub analysis_context_id: String,
    pub canonical_workspace_id: [u8; 16],
    pub canonical_analysis_context_id: [u8; 16],
    pub source_generation: u64,
    pub context_manifest: Vec<u8>,
    pub source_snapshot_lease_id: String,
    pub source_manifest_digest: String,
    pub modules: Vec<PyreflyModuleInput>,
    pub requested_capability_codes: Vec<u32>,
    pub deadline_unix_ms: i64,
    pub sandbox_profile_digest: String,
    pub output_schema_bundle_digest: String,
}

/// One fully verified application-owned module observation.
#[derive(Clone, Debug)]
pub struct AcceptedPyreflyModule {
    pub module_id: String,
    pub module_name: String,
    pub canonical_file_id: [u8; 16],
    pub source_bytes: Vec<u8>,
    pub module_digest: String,
    /// Complete target-route application-owned relation set.
    pub relations: Vec<AcceptedPyreflyRelation>,
}

/// One independently schema-validated relation-scoped Arrow stream.
#[derive(Clone, Debug)]
pub struct AcceptedPyreflyRelation {
    pub relation: PyreflyRelation,
    pub arrow_ipc: Vec<u8>,
    pub batch: RecordBatch,
    pub schema_digest: String,
    pub arrow_ipc_digest: String,
    pub row_count: u64,
}

struct PendingPyreflyModule {
    module_id: String,
    module_name: String,
    canonical_file_id: [u8; 16],
    source_bytes: Vec<u8>,
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
    pub modules: Vec<AcceptedPyreflyModule>,
    pub capability_codes: Vec<u32>,
    pub overall_digest: String,
    pub rechecked_module_ids: Vec<String>,
    pub sandbox_profile_digest: String,
    pub trust_profile: String,
}

/// Closed sidecar/transport/Arrow validation failures.
#[derive(Debug, thiserror::Error)]
pub enum PyreflyServiceError {
    #[error("Pyrefly request is invalid: {0}")]
    Invalid(String),
    #[error("Pyrefly sidecar transport failed: {0}")]
    Transport(String),
    #[error("Pyrefly sidecar protocol failed: {0}")]
    Protocol(String),
    #[error("Pyrefly observation Arrow IPC failed: {0}")]
    Arrow(String),
    #[error("Pyrefly run was cancelled and its disposable sidecar process was joined")]
    Cancelled,
    #[error(
        "Pyrefly run exceeded its release-owned deadline and its disposable sidecar process was joined"
    )]
    TimedOut,
    #[error("Pyrefly disposable sidecar process could not be terminated and joined: {0}")]
    ProcessTermination(String),
    #[error("the exact untrusted Pyrefly containment profile is unavailable")]
    TrustUnavailable,
    #[error("Pyrefly source read failed at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Exact incomplete provider outcome returned instead of an accepted Pyrefly batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PyreflyRunGap {
    Cancelled,
    TimedOut,
    ProcessFailure,
    TrustUnavailable,
}

impl PyreflyServiceError {
    /// Translate process lifecycle failures into the exact provider-lane gap category.
    #[must_use]
    pub const fn run_gap(&self) -> Option<PyreflyRunGap> {
        match self {
            Self::Cancelled => Some(PyreflyRunGap::Cancelled),
            Self::TimedOut => Some(PyreflyRunGap::TimedOut),
            Self::ProcessTermination(_) | Self::Transport(_) => Some(PyreflyRunGap::ProcessFailure),
            Self::TrustUnavailable => Some(PyreflyRunGap::TrustUnavailable),
            Self::Invalid(_) | Self::Protocol(_) | Self::Arrow(_) | Self::Io { .. } => None,
        }
    }
}

/// One contained, run-disposable Pyrefly process created by the sole provider sandbox launcher.
///
/// The process-group child has no public constructor and therefore cannot be substituted with an
/// arbitrary PID. Cancellation or deadline consumes this owner, terminates the complete group,
/// waits for emptiness, and prevents any result from entering admission.
pub(crate) struct DisposablePyreflySidecarProcess {
    child: Option<ProviderProcessGroupChild>,
    socket: PathBuf,
    cancellation_grace: Duration,
    maximum_wall_time: Duration,
    analysis_started: Arc<AtomicBool>,
    sandbox_profile_digest: String,
}

impl DisposablePyreflySidecarProcess {
    /// Bind one launcher-owned process group to the exact compiled Pyrefly recipe and private UDS.
    pub(crate) fn try_new(
        compiled_authority: &CompiledProviderAuthority,
        child: ProviderProcessGroupChild,
        socket: PathBuf,
    ) -> Result<Self, PyreflyServiceError> {
        let profile = compiled_authority.execution_profile(CompiledProviderLane::Pyrefly);
        let sandbox_profile_digest = child.sandbox_profile_digest().to_owned();
        if profile.provider_id != "pyrefly-python"
            || profile.placement != "SIDECAR"
            || profile.resource_profile_id != RESOURCE_PROFILE_ID
            || profile.max_parser_workers == 0
            || !socket.is_absolute()
        {
            return Err(PyreflyServiceError::Invalid(
                "compiled Pyrefly process authority or private socket is invalid".to_owned(),
            ));
        }
        if child.trust_profile()
            != crate::provider_sandbox::ProviderTrustProfile::UntrustedSandboxed
            || !valid_sandbox_profile_digest(&sandbox_profile_digest)
        {
            return Err(PyreflyServiceError::TrustUnavailable);
        }
        Ok(Self {
            child: Some(child),
            socket,
            cancellation_grace: Duration::from_millis(u64::from(profile.cancellation_ack_millis)),
            maximum_wall_time: Duration::from_millis(profile.max_wall_millis),
            analysis_started: Arc::new(AtomicBool::new(false)),
            sandbox_profile_digest,
        })
    }

    /// Clone the real protocol acceptance signal for supervision and cancellation evidence.
    #[must_use]
    pub(crate) fn analysis_started_signal(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.analysis_started)
    }

    async fn terminate_and_join(&mut self) -> Result<(), PyreflyServiceError> {
        let Some(mut child) = self.child.take() else {
            return Ok(());
        };
        let grace = self.cancellation_grace;
        let socket = self.socket.clone();
        tokio::task::spawn_blocking(move || {
            child.terminate_group()?;
            if !child.wait_group_empty(grace)? {
                child.kill_group()?;
            }
            if !child.wait_group_empty(grace)? {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Pyrefly process group remained live after unconditional termination",
                ));
            }
            let _ = child.wait();
            let _ = std::fs::remove_file(socket);
            Ok::<(), std::io::Error>(())
        })
        .await
        .map_err(|error| PyreflyServiceError::ProcessTermination(error.to_string()))?
        .map_err(|error| PyreflyServiceError::ProcessTermination(error.to_string()))
    }
}

fn require_exact_pyrefly_containment(
    compiled_authority: &CompiledProviderAuthority,
    matrix: &SandboxCapabilityMatrix,
) -> Result<SandboxMechanism, PyreflyServiceError> {
    let profile = compiled_authority.execution_profile(CompiledProviderLane::Pyrefly);
    let row = matrix
        .row(ProviderTrustProfile::UntrustedSandboxed)
        .ok_or(PyreflyServiceError::TrustUnavailable)?;
    if profile.provider_id != "pyrefly-python"
        || profile.placement != "SIDECAR"
        || profile.resource_profile_id != RESOURCE_PROFILE_ID
        || !row.available
        || row.mechanism == SandboxMechanism::None
    {
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

async fn wait_for_pyrefly_stop(
    cancellation: &Cancellation,
    deadline_unix_ms: i64,
) -> PyreflyStopCause {
    loop {
        if cancellation.is_cancelled() {
            return PyreflyStopCause::Cancelled;
        }
        if now_unix_millis() >= deadline_unix_ms {
            return PyreflyStopCause::TimedOut;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

fn b3(bytes: &[u8]) -> String {
    format!("b3:{}", blake3::hash(bytes).to_hex())
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

fn parse_digest(value: &str) -> Result<[u8; 32], PyreflyServiceError> {
    let encoded = value
        .strip_prefix("b3:")
        .filter(|encoded| encoded.len() == 64)
        .ok_or_else(|| PyreflyServiceError::Protocol("digest is not b3-32".to_owned()))?;
    let mut result = [0_u8; 32];
    for (index, chunk) in encoded.as_bytes().chunks_exact(2).enumerate() {
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
    bytes: Arc<[u8]>,
}

fn read_immutable_blob(
    input: &PyreflyModuleInput,
) -> Result<AdmittedImmutableBlob, PyreflyServiceError> {
    if !input.source_blob_path.is_absolute()
        || !input.source_blob_path.is_file()
        || input
            .source_blob_path
            .metadata()
            .map(|metadata| metadata.len() > MAX_SOURCE_BYTES_PER_MODULE)
            .unwrap_or(true)
    {
        return Err(PyreflyServiceError::Invalid(
            "module source blob path or bounded size is invalid".to_owned(),
        ));
    }
    let bytes =
        std::fs::read(&input.source_blob_path).map_err(|source| PyreflyServiceError::Io {
            path: input.source_blob_path.clone(),
            source,
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
            read_only_uri: format!("file://{}", input.source_blob_path.display()),
        },
        bytes: Arc::from(bytes),
    })
}

/// Execute and fully validate one real Pyrefly sidecar stream over its private UDS.
///
/// # Errors
///
/// Rejects malformed inputs, transport/handshake drift, stream correlation or sequence errors,
/// invalid Arrow IPC, and any non-success terminal.
#[allow(clippy::too_many_lines)] // One sidecar stream validator keeps every ordered correlation and terminal check adjacent.
pub(crate) async fn analyze_pyrefly_uds(
    mut process: DisposablePyreflySidecarProcess,
    request: &PyreflyRunRequest,
    cancellation: &Cancellation,
) -> Result<AcceptedPyreflyRun, PyreflyServiceError> {
    if request.sandbox_profile_digest != process.sandbox_profile_digest {
        process.terminate_and_join().await?;
        return Err(PyreflyServiceError::TrustUnavailable);
    }
    let socket = process.socket.clone();
    process.analysis_started.store(false, Ordering::Release);
    let analysis = analyze_pyrefly_uds_inner(
        &socket,
        request,
        None,
        Some(Arc::clone(&process.analysis_started)),
    );
    let release_deadline = now_unix_millis()
        .saturating_add(i64::try_from(process.maximum_wall_time.as_millis()).unwrap_or(i64::MAX));
    let stop = wait_for_pyrefly_stop(cancellation, request.deadline_unix_ms.min(release_deadline));
    tokio::pin!(analysis);
    tokio::pin!(stop);
    let outcome = tokio::select! {
        biased;
        result = &mut analysis => Ok(result),
        cause = &mut stop => {
            Err(cause)
        }
    };
    process.terminate_and_join().await?;
    match outcome {
        Ok(result) => result,
        Err(PyreflyStopCause::Cancelled) => Err(PyreflyServiceError::Cancelled),
        Err(PyreflyStopCause::TimedOut) => Err(PyreflyServiceError::TimedOut),
    }
}

#[allow(clippy::too_many_lines)] // One sidecar stream validator keeps every ordered correlation and terminal check adjacent.
async fn analyze_pyrefly_uds_inner(
    socket: &Path,
    request: &PyreflyRunRequest,
    cancellation: Option<crate::cancellation::Cancellation>,
    analysis_started: Option<Arc<AtomicBool>>,
) -> Result<AcceptedPyreflyRun, PyreflyServiceError> {
    if request.modules.is_empty()
        || request.modules.len() > MAX_MODULES_PER_RUN
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
    let relation_schema_digests = schema_digests();
    let admitted_blobs = request
        .modules
        .iter()
        .map(read_immutable_blob)
        .collect::<Result<Vec<_>, _>>()?;
    let admitted_source_total = admitted_blobs.iter().try_fold(0_u64, |total, blob| {
        total.checked_add(blob.reference.byte_length)
    });
    if admitted_source_total.is_none_or(|total| total > MAX_SOURCE_BYTES_PER_RUN) {
        return Err(PyreflyServiceError::Invalid(
            "Pyrefly admitted source bytes exceed the per-run bound".to_owned(),
        ));
    }
    let blobs = admitted_blobs
        .iter()
        .map(|blob| blob.reference.clone())
        .collect::<Vec<_>>();
    let admitted_source_bytes = request
        .modules
        .iter()
        .zip(&admitted_blobs)
        .map(|(module, blob)| (module.module_id.clone(), Arc::clone(&blob.bytes)))
        .collect::<BTreeMap<_, _>>();
    let context_digest = b3(&request.context_manifest);
    let lease = SourceSnapshotLease {
        lease_id: request.source_snapshot_lease_id.clone(),
        workspace_id: request.workspace_id.clone(),
        source_generation: request.source_generation,
        source_manifest_digest: request.source_manifest_digest.clone(),
        expires_at_unix_ms: request.deadline_unix_ms,
        blobs: blobs.clone(),
    };
    let socket = socket.to_path_buf();
    let channel = Endpoint::from_static("http://[::]:50051")
        .connect_with_connector(service_fn(move |_| {
            let socket = socket.clone();
            async move { UnixStream::connect(socket).await.map(TokioIo::new) }
        }))
        .await
        .map_err(|error| PyreflyServiceError::Transport(error.to_string()))?;
    let mut client = WireClient::new(channel)
        .max_decoding_message_size(4 * 1024 * 1024)
        .max_encoding_message_size(4 * 1024 * 1024);
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
            sandbox_profile_digest: request.sandbox_profile_digest.clone(),
        })
        .await
        .map_err(|error| PyreflyServiceError::Protocol(error.to_string()))?
        .into_inner();
    if acknowledgement.protocol_major != 1
        || acknowledgement.protocol_minor != 0
        || acknowledgement.negotiated_feature_bits != REQUIRED_FEATURE_BITS | OPTIONAL_FEATURE_BITS
        || acknowledgement.pyrefly_source_digest != PYREFLY_SOURCE_DIGEST
        || acknowledgement.observation_schema_digests != relation_schema_digests
        || acknowledgement.sandbox_profile_digest != request.sandbox_profile_digest
    {
        return Err(PyreflyServiceError::Protocol(
            "handshake acknowledgement identity differs".to_owned(),
        ));
    }
    let opened = client
        .open_context(OpenContextRequest {
            workspace_id: request.workspace_id.clone(),
            analysis_context_id: request.analysis_context_id.clone(),
            immutable_context_manifest: request.context_manifest.clone(),
            context_manifest_digest: context_digest.clone(),
            source_snapshot_lease: Some(lease),
            resource_profile_id: RESOURCE_PROFILE_ID.to_owned(),
            maximum_contexts: 4,
            maximum_memory_mib: 4096,
            sandbox_profile_digest: request.sandbox_profile_digest.clone(),
        })
        .await
        .map_err(|error| PyreflyServiceError::Protocol(error.to_string()))?
        .into_inner();
    if opened.context_handle.is_empty()
        || opened.context_manifest_digest != context_digest
        || opened.sandbox_profile_digest != request.sandbox_profile_digest
    {
        return Err(PyreflyServiceError::Protocol(
            "opened context identity differs".to_owned(),
        ));
    }
    let modules = request
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
        .collect();
    let start = AnalyzeCommand {
        command: Some(Command::Start(AnalyzeModulesRequest {
            provider_run_id: request.provider_run_id.clone(),
            workspace_id: request.workspace_id.clone(),
            analysis_context_id: request.analysis_context_id.clone(),
            context_handle: opened.context_handle,
            context_manifest_digest: context_digest.clone(),
            source_generation: request.source_generation,
            source_snapshot_lease_id: request.source_snapshot_lease_id.clone(),
            modules,
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
    let cancel_client = client.clone();
    let mut stream = client
        .analyze_modules(ReceiverStream::new(command_receiver))
        .await
        .map_err(|error| PyreflyServiceError::Protocol(error.to_string()))?
        .into_inner();
    let mut cancel_task = cancellation.clone().map(|cancellation| {
        let mut client = cancel_client;
        let command_sender = command_sender.clone();
        let provider_run_id = request.provider_run_id.clone();
        tokio::spawn(async move {
            while !cancellation.is_cancelled() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            let cancel = CancelRunRequest {
                provider_run_id: provider_run_id.clone(),
                reason: "daemon-runtime-cancelled".to_owned(),
            };
            command_sender
                .send(AnalyzeCommand {
                    command: Some(Command::Cancel(cancel.clone())),
                })
                .await
                .map_err(|_| {
                    PyreflyServiceError::Protocol(
                        "sidecar cancellation command stream closed".to_owned(),
                    )
                })?;
            let acknowledgement =
                tokio::time::timeout(Duration::from_secs(2), client.cancel_run(cancel))
                    .await
                    .map_err(|_| {
                        PyreflyServiceError::Protocol(
                            "sidecar cancellation acknowledgement exceeded two seconds".to_owned(),
                        )
                    })?
                    .map_err(|error| PyreflyServiceError::Protocol(error.to_string()))?
                    .into_inner();
            if acknowledgement.provider_run_id != provider_run_id
                || !matches!(
                    CancelAcknowledgementState::try_from(acknowledgement.state),
                    Ok(CancelAcknowledgementState::CancellationRequested
                        | CancelAcknowledgementState::AlreadyTerminal)
                )
            {
                return Err(PyreflyServiceError::Protocol(
                    "sidecar cancellation acknowledgement identity differs".to_owned(),
                ));
            }
            Ok(())
        })
    });
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
                    .as_ref()
                    .to_vec();
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
                    let row_count = u64::try_from(batch.num_rows()).unwrap_or(u64::MAX);
                    let arrow_ipc = assembled.ipc_bytes;
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
                    relations,
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
                let expected_rechecked = Vec::<String>::new();
                let terminal_state =
                    WireProviderRunState::try_from(event.terminal_state).map_err(|_| {
                        PyreflyServiceError::Protocol(
                            "run terminal state is unregistered".to_owned(),
                        )
                    })?;
                let cancellation_expected = cancellation
                    .as_ref()
                    .is_some_and(crate::cancellation::Cancellation::is_cancelled);
                let accepted_cancellation =
                    cancellation_expected && terminal_state == WireProviderRunState::Cancelled;
                if accepted_cancellation {
                    open_module.take();
                    pending.take();
                }
                let success_outcomes = event.capability_outcomes.iter().all(|outcome| {
                    outcome.owner_capability_state_code == 40
                        && outcome.completeness_state_code == 20
                        && outcome.reason_code == "PYREFLY_QUERY_SLICE_PARTIAL"
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
                    || event.rechecked_module_ids != expected_rechecked
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
        sandbox_profile_digest,
        trust_profile,
    ) = terminal
        .ok_or_else(|| PyreflyServiceError::Protocol("stream ended before terminal".to_owned()))?;
    if cancelled_terminal {
        let task = cancel_task.take().ok_or_else(|| {
            PyreflyServiceError::Protocol(
                "sidecar returned cancelled without a daemon cancellation request".to_owned(),
            )
        })?;
        task.await
            .map_err(|error| PyreflyServiceError::Protocol(error.to_string()))??;
        return Err(PyreflyServiceError::Cancelled);
    }
    if let Some(task) = cancel_task {
        task.abort();
    }
    if accepted.len() != request.modules.len()
        || capability_codes != request.requested_capability_codes
    {
        return Err(PyreflyServiceError::Protocol(
            "accepted module or capability census differs".to_owned(),
        ));
    }
    Ok(AcceptedPyreflyRun {
        provider_run_id: request.provider_run_id.clone(),
        workspace_id: request.workspace_id.clone(),
        analysis_context_id: request.analysis_context_id.clone(),
        canonical_workspace_id: request.canonical_workspace_id,
        canonical_analysis_context_id: request.canonical_analysis_context_id,
        source_generation: request.source_generation,
        modules: accepted,
        capability_codes,
        overall_digest,
        rechecked_module_ids,
        sandbox_profile_digest,
        trust_profile,
    })
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;

    use arrow_ipc::writer::StreamWriter;
    use tokio_stream::Stream;
    use tokio_stream::wrappers::UnixListenerStream;
    use tonic::{Request, Response, Status};

    use super::*;
    use crate::fabric::production_kernel::CompiledSemanticRelease;
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
            let error = analyze_pyrefly_uds_inner(&socket, &request, None, None)
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wp34_ops_real_pyrefly_analysis_cancellation_joins_disposable_process() {
        let executable = std::env::var("CODEFABRIC_PYREFLY_SIDECAR_BIN")
            .expect("WP34 OPS must build and select the real Pyrefly sidecar binary");
        let executable = std::fs::canonicalize(executable)
            .expect("selected Pyrefly sidecar binary must exist exactly");
        assert!(executable.is_file());
        let release = CompiledSemanticRelease::current();
        let matrix = SandboxCapabilityMatrix::probe_current_host();
        match require_exact_pyrefly_containment(release.provider_authority(), &matrix) {
            Err(error) => {
                assert!(matches!(error, PyreflyServiceError::TrustUnavailable));
                assert_eq!(error.run_gap(), Some(PyreflyRunGap::TrustUnavailable));
            }
            Ok(mechanism) => {
                assert_eq!(
                    mechanism,
                    SandboxMechanism::DarwinSeatbelt,
                    "an available Linux row is impossible until the compiled seccomp policy and delegated cgroup are release-owned launch material"
                );
                #[cfg(not(target_os = "macos"))]
                panic!("only the exact Darwin containment path can currently be advertised");
                #[cfg(target_os = "macos")]
                {
                    let root = tempfile::tempdir().unwrap();
                    let workspace = root.path().join("workspace");
                    let output = root.path().join("output");
                    for path in [&workspace, &output] {
                        std::fs::create_dir(path).unwrap();
                    }
                    let dependencies = executable
                        .parent()
                        .expect("sidecar binary has a dependency root");
                    let profile = GeneratedSandboxProfile::generate(
                        ProviderTrustProfile::UntrustedSandboxed,
                        mechanism,
                        &workspace,
                        dependencies,
                        &output,
                    )
                    .unwrap();
                    let materialized_profile =
                        profile.materialize(&root.path().join("profiles")).unwrap();
                    let socket = output.join("pyrefly-cancel.sock");
                    let child = ProviderSandboxLauncher::new(matrix)
                        .launch(
                            &ProviderLaunchRequest {
                                host_executable: executable.clone(),
                                contained_executable: executable,
                                arguments: vec![
                                    "--serve".to_owned(),
                                    format!("unix://{}", socket.display()),
                                ],
                                environment: BTreeMap::from([(
                                    "PATH".to_owned(),
                                    "/usr/bin:/bin".to_owned(),
                                )]),
                                output_root: profile.output_root.clone(),
                                limits: ProviderProcessLimits {
                                    cpu_seconds: 60,
                                    open_files: 128,
                                    address_space_bytes: 8 * 1024 * 1024 * 1024,
                                    output_file_bytes: 512 * 1024 * 1024,
                                    process_count: 32,
                                },
                            },
                            &profile,
                            ProviderSandboxLaunchMaterial::DarwinProfile(&materialized_profile),
                        )
                        .unwrap();
                    let process = DisposablePyreflySidecarProcess::try_new(
                        release.provider_authority(),
                        child,
                        socket.clone(),
                    )
                    .unwrap();
                    tokio::time::timeout(Duration::from_secs(10), async {
                        while !socket.exists() {
                            tokio::time::sleep(Duration::from_millis(5)).await;
                        }
                    })
                    .await
                    .expect("real Pyrefly sidecar did not bind its private socket");

                    let source_text = (0..20_000)
                        .map(|index| {
                            format!(
                                "class Item{index}:\n    value: int = {index}\n\ndef use_{index}(item: Item{index}) -> int:\n    return item.value\n"
                            )
                        })
                        .collect::<String>();
                    let source_path = workspace.join("module.py");
                    std::fs::write(&source_path, source_text.as_bytes()).unwrap();
                    let mut request = malformed_request(root.path());
                    request.provider_run_id = "run-real-cancel".to_owned();
                    request.source_snapshot_lease_id = "lease-real-cancel".to_owned();
                    request.deadline_unix_ms = now_unix_millis() + 60_000;
                    request.sandbox_profile_digest = profile.sha256_digest.clone();
                    request.modules[0] = PyreflyModuleInput {
                        module_id: "module-real-cancel".to_owned(),
                        module_name: "real_cancel".to_owned(),
                        file_id: crate::identity::encode_public_id(
                            crate::identity::IdentityDomain::SourceFile,
                            None,
                            [0x77; 16],
                        )
                        .unwrap(),
                        source_blob_path: source_path.clone(),
                        content_digest: b3(&std::fs::read(source_path).unwrap()),
                    };
                    let started = process.analysis_started_signal();
                    let cancellation = Cancellation::with_check_interval(1);
                    let cancel = cancellation.clone();
                    let cancellation_task = tokio::spawn(async move {
                        tokio::time::timeout(Duration::from_secs(30), async {
                            while !started.load(Ordering::Acquire) {
                                tokio::time::sleep(Duration::from_millis(1)).await;
                            }
                        })
                        .await
                        .expect("real Pyrefly producer never entered blocking analysis");
                        cancel.cancel();
                    });
                    let started_at = std::time::Instant::now();
                    let error = analyze_pyrefly_uds(process, &request, &cancellation)
                        .await
                        .expect_err("cancelled real analysis must not return an accepted batch");
                    cancellation_task.await.unwrap();
                    assert!(matches!(error, PyreflyServiceError::Cancelled));
                    assert_eq!(error.run_gap(), Some(PyreflyRunGap::Cancelled));
                    assert!(!socket.exists());
                    assert!(started_at.elapsed() < Duration::from_secs(20));
                }
            }
        }
    }

    #[test]
    fn wp34_neg_trusted_local_pyrefly_process_substitution_is_rejected() {
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
        let child = ProviderSandboxLauncher::new(SandboxCapabilityMatrix::probe_current_host())
            .launch(
                &ProviderLaunchRequest {
                    host_executable: "/bin/sleep".into(),
                    contained_executable: "/bin/sleep".into(),
                    arguments: vec!["30".to_owned()],
                    environment: BTreeMap::from([("PATH".to_owned(), "/usr/bin:/bin".to_owned())]),
                    output_root: profile.output_root.clone(),
                    limits: ProviderProcessLimits {
                        cpu_seconds: 30,
                        open_files: 16,
                        address_space_bytes: 64 * 1024 * 1024,
                        output_file_bytes: 1024,
                        process_count: 4,
                    },
                },
                &profile,
                ProviderSandboxLaunchMaterial::None,
            )
            .unwrap();
        let release = CompiledSemanticRelease::current();
        let result = DisposablePyreflySidecarProcess::try_new(
            release.provider_authority(),
            child,
            output.join("pyrefly.sock"),
        );
        assert!(matches!(result, Err(PyreflyServiceError::TrustUnavailable)));
    }
}
