//! Stable-daemon adapter for the isolated Pyrefly sidecar.
//!
//! Only application-owned Arrow batches and correlation DTOs cross this module. No Pyrefly
//! library type is linked into or exposed by the stable daemon.

use std::collections::BTreeMap;
use std::io::Cursor;
use std::io::Read as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use arrow_array::{BinaryArray, RecordBatch, StringArray};
use arrow_ipc::reader::FileReader;
use arrow_schema::{DataType, Field, Schema};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use tokio::net::UnixStream;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Endpoint;
use tower::service_fn;

use crate::model_generated::schema_tables::{
    PROVIDER_OBSERVATION_SCHEMAS, ProviderObservationLogicalType, ProviderObservationSchema,
};
use crate::rpc::generated::codefabric::provider::v1::{
    BlobReference, CancelAcknowledgementState, ChunkAccepted, ChunkRejected,
    ProviderRunState as WireProviderRunState, SourceSnapshotLease,
};
use crate::rpc::generated::codefabric::pyrefly::v1::analyze_command::Command;
use crate::rpc::generated::codefabric::pyrefly::v1::analyze_event::Event;
use crate::rpc::generated::codefabric::pyrefly::v1::pyrefly_sidecar_client::PyreflySidecarClient as WireClient;
use crate::rpc::generated::codefabric::pyrefly::v1::{
    AnalyzeCommand, AnalyzeEventHeader, AnalyzeModulesRequest, CancelRunRequest, Hello,
    ModuleRequest, OpenContextRequest,
};

const PYREFLY_SOURCE_DIGEST: &str =
    "b3:1b9e72144644d1b3df0bdca564496566238543dfb7f576980a8408714327fc3e";
pub(crate) const SANDBOX_PROFILE_DIGEST: &str =
    "b3:8a663d1d6ddbcf830a09e28c7ee6bcd65b433fd9b69b597dbe99f02c78ce8e15";
const REQUIRED_FEATURE_BITS: u64 = (1_u64 << 17) | (1_u64 << 32);
const OPTIONAL_FEATURE_BITS: u64 = 1_u64 << 33;
const MAX_UNACKNOWLEDGED_BYTES: u64 = 16 * 1024 * 1024;
const RESOURCE_PROFILE_ID: &str = "sidecar-semantic-standard";
const TRUST_PROFILE: &str = "UNTRUSTED_SANDBOXED";
const SIDECAR_START_TIMEOUT: Duration = Duration::from_secs(15);
static SIDECAR_INSTANCE_NONCE: AtomicU64 = AtomicU64::new(0);

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
    pub output_schema_bundle_digest: String,
}

/// One fully verified application-owned module observation.
#[derive(Clone, Debug)]
pub struct AcceptedPyreflyModule {
    pub module_id: String,
    pub module_name: String,
    pub canonical_file_id: [u8; 16],
    pub source_bytes: Vec<u8>,
    pub arrow_ipc: Vec<u8>,
    pub batch: RecordBatch,
    pub schema_digest: String,
    pub chunk_digest: String,
    pub module_digest: String,
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
    #[error("Pyrefly run was cancelled after sidecar acknowledgement")]
    Cancelled,
    #[error("Pyrefly source read failed at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

struct SupervisedSidecar {
    child: Child,
    socket: PathBuf,
    private_root: PathBuf,
    workspace_manifest_digest: [u8; 32],
    restart_attempt: u32,
}

impl Drop for SupervisedSidecar {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket);
        let _ = std::fs::remove_dir_all(&self.private_root);
    }
}

/// Production Pyrefly lane driver installed behind the shared semantic provider adapter.
pub struct PyreflyProviderDriver {
    binary: PathBuf,
    state_root: PathBuf,
    supervisors: Mutex<BTreeMap<String, SupervisedSidecar>>,
    results: Mutex<BTreeMap<String, Result<AcceptedPyreflyRun, String>>>,
}

impl PyreflyProviderDriver {
    /// Construct the production driver over one exact sidecar executable and daemon state root.
    ///
    /// # Errors
    ///
    /// Rejects relative or absent launch material before any provider job can be registered.
    pub fn new(binary: PathBuf, state_root: PathBuf) -> Result<Self, PyreflyServiceError> {
        if !binary.is_absolute()
            || !binary.is_file()
            || !state_root.is_absolute()
            || binary.file_name().and_then(|value| value.to_str())
                != Some("codefabric-pyrefly-sidecar")
        {
            return Err(PyreflyServiceError::Invalid(
                "Pyrefly driver launch material is invalid".to_owned(),
            ));
        }
        std::fs::create_dir_all(&state_root).map_err(|source| PyreflyServiceError::Io {
            path: state_root.clone(),
            source,
        })?;
        Ok(Self {
            binary,
            state_root,
            supervisors: Mutex::new(BTreeMap::new()),
            results: Mutex::new(BTreeMap::new()),
        })
    }

    /// Encode the lane-owned request into the opaque shared invocation slot.
    ///
    /// # Errors
    ///
    /// Returns a serialization failure without mutating provider state.
    pub fn invocation_manifest(
        request: &PyreflyRunRequest,
    ) -> Result<Arc<[u8]>, PyreflyServiceError> {
        serde_json::to_vec(request)
            .map(Arc::from)
            .map_err(|error| PyreflyServiceError::Invalid(error.to_string()))
    }

    /// Take the application-owned result produced by one terminal `ProviderRuntime` run.
    #[must_use]
    pub fn take_result(&self, provider_run_id: &str) -> Option<Result<AcceptedPyreflyRun, String>> {
        self.results
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(provider_run_id)
    }

    fn supervisor_key(request: &PyreflyRunRequest) -> String {
        format!("{}:{}", request.workspace_id, request.analysis_context_id)
    }

    fn launch_sidecar(
        &self,
        work: &crate::provider_runtime::SemanticProviderWork,
        request: &PyreflyRunRequest,
        restart_attempt: u32,
    ) -> Result<SupervisedSidecar, String> {
        use crate::provider_sandbox::{
            GeneratedSandboxProfile, ProviderLaunchRequest, ProviderProcessLimits,
            ProviderSandboxLaunchMaterial, ProviderSandboxLauncher, SandboxCapabilityMatrix,
            SandboxMechanism,
        };

        let observation = crate::provider_sandbox::probe_host_sandbox();
        let matrix = SandboxCapabilityMatrix::evaluate(&observation);
        let row = matrix
            .row(work.trust_profile)
            .ok_or_else(|| "SANDBOX_UNAVAILABLE".to_owned())?;
        if !row.available
            || work.trust_profile
                != crate::provider_sandbox::ProviderTrustProfile::UntrustedSandboxed
        {
            return Err("SANDBOX_UNAVAILABLE".to_owned());
        }
        let instance_nonce = SIDECAR_INSTANCE_NONCE.fetch_add(1, Ordering::Relaxed);
        let private_root = PathBuf::from("/tmp").join(format!(
            "cfpy-{}-{}-{}-{instance_nonce}",
            std::process::id(),
            &crate::integrity::framed_digest(Self::supervisor_key(request).as_bytes())[3..11],
            restart_attempt
        ));
        std::fs::create_dir(&private_root).map_err(|error| error.to_string())?;
        std::fs::set_permissions(&private_root, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
        let profile = GeneratedSandboxProfile::generate(
            work.trust_profile,
            row.mechanism,
            &work.workspace_view.workspace_root,
            &work.workspace_view.dependency_root,
            &private_root,
        )
        .map_err(|error| error.to_string())?;
        let profile_path = profile
            .materialize(&self.state_root.join("sandbox-profiles"))
            .map_err(|error| error.to_string())?;
        let socket = private_root.join("run.sock");
        let _ = std::fs::remove_file(&socket);
        let launch = ProviderLaunchRequest {
            executable: self.binary.clone(),
            arguments: vec!["--serve".to_owned(), format!("unix://{}", socket.display())],
            output_root: private_root.clone(),
            limits: ProviderProcessLimits {
                cpu_seconds: 120,
                open_files: 256,
                address_space_bytes: 4 * 1024 * 1024 * 1024,
                output_file_bytes: 512 * 1024 * 1024,
            },
        };
        let launcher = ProviderSandboxLauncher::new(matrix);
        let child = match profile.mechanism {
            SandboxMechanism::DarwinSeatbelt => launcher.launch(
                &launch,
                &profile,
                ProviderSandboxLaunchMaterial::DarwinProfile(&profile_path),
            ),
            SandboxMechanism::LinuxBubblewrap | SandboxMechanism::None => {
                return Err("SANDBOX_UNAVAILABLE".to_owned());
            }
        }
        .map_err(|error| error.to_string())?;
        let mut sidecar = SupervisedSidecar {
            child,
            socket,
            private_root,
            workspace_manifest_digest: work.workspace_view.manifest_digest,
            restart_attempt,
        };
        let start_deadline = Instant::now() + SIDECAR_START_TIMEOUT;
        while Instant::now() < start_deadline {
            if sidecar.socket.exists() {
                return Ok(sidecar);
            }
            if sidecar
                .child
                .try_wait()
                .map_err(|error| error.to_string())?
                .is_some()
            {
                let mut detail = String::new();
                if let Some(mut stderr) = sidecar.child.stderr.take() {
                    let _ = stderr.read_to_string(&mut detail);
                }
                return Err(format!(
                    "PYREFLY_SIDECAR_CRASHED_BEFORE_READY:{}",
                    detail.trim()
                ));
            }
            thread::sleep(Duration::from_millis(10));
        }
        Err("PYREFLY_SIDECAR_START_TIMEOUT".to_owned())
    }

    fn ensure_sidecar(
        &self,
        work: &crate::provider_runtime::SemanticProviderWork,
        request: &PyreflyRunRequest,
    ) -> Result<PathBuf, String> {
        let key = Self::supervisor_key(request);
        let mut supervisors = self
            .supervisors
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let healthy = supervisors.get_mut(&key).is_some_and(|sidecar| {
            sidecar.workspace_manifest_digest == work.workspace_view.manifest_digest
                && sidecar.socket.exists()
                && sidecar
                    .child
                    .try_wait()
                    .is_ok_and(|status| status.is_none())
        });
        if !healthy {
            let restart_attempt = supervisors
                .remove(&key)
                .map_or(0, |sidecar| sidecar.restart_attempt.saturating_add(1));
            if restart_attempt > 0 {
                let exponent = restart_attempt.min(5);
                thread::sleep(Duration::from_millis(25_u64 << exponent));
            }
            let sidecar = self.launch_sidecar(work, request, restart_attempt)?;
            let socket = sidecar.socket.clone();
            supervisors.insert(key, sidecar);
            return Ok(socket);
        }
        Ok(supervisors
            .get(&key)
            .expect("healthy supervisor remains registered")
            .socket
            .clone())
    }
}

fn digest_payload(value: &str) -> Result<[u8; 32], crate::provider_runtime::ProviderRuntimeError> {
    let payload = value
        .strip_prefix("b3:")
        .filter(|payload| payload.len() == 64)
        .ok_or_else(|| {
            crate::provider_runtime::ProviderRuntimeError::Protocol(
                "Pyrefly output digest is invalid".to_owned(),
            )
        })?;
    let mut decoded = [0_u8; 32];
    for (index, pair) in payload.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let text = std::str::from_utf8(pair).map_err(|_| {
            crate::provider_runtime::ProviderRuntimeError::Protocol(
                "Pyrefly output digest is not UTF-8 hex".to_owned(),
            )
        })?;
        decoded[index] = u8::from_str_radix(text, 16).map_err(|_| {
            crate::provider_runtime::ProviderRuntimeError::Protocol(
                "Pyrefly output digest is not lowercase hex".to_owned(),
            )
        })?;
    }
    Ok(decoded)
}

impl crate::provider_runtime::SemanticProviderDriver for PyreflyProviderDriver {
    #[allow(clippy::too_many_lines)] // Keep production lane validation, supervision, cancellation, and terminal transfer in one adapter transaction.
    fn execute(
        &self,
        work: crate::provider_runtime::SemanticProviderWork,
        events: crate::provider_runtime::ProviderEventSink,
        cancellation: crate::cancellation::Cancellation,
    ) -> Result<
        crate::provider_runtime::ProviderCompletion,
        crate::provider_runtime::ProviderRuntimeError,
    > {
        use crate::provider_runtime::{ProviderCompletion, ProviderRuntimeError};

        let request: PyreflyRunRequest = serde_json::from_slice(&work.invocation_manifest)
            .map_err(|error| ProviderRuntimeError::InvalidJob(error.to_string()))?;
        if work.provider_id != "pyrefly-python"
            || work.capability_family != "PYTHON_SEMANTIC"
            || work.trust_profile
                != crate::provider_sandbox::ProviderTrustProfile::UntrustedSandboxed
            || request.source_generation != work.workspace_view.source_generation
            || request.modules.iter().any(|module| {
                !module
                    .source_blob_path
                    .starts_with(&work.workspace_view.workspace_root)
                    && !module
                        .source_blob_path
                        .starts_with(&work.workspace_view.dependency_root)
            })
        {
            return Err(ProviderRuntimeError::InvalidJob(
                "Pyrefly invocation differs from the admitted immutable workspace view".into(),
            ));
        }
        if cancellation.is_cancelled() {
            return Ok(ProviderCompletion {
                state: crate::registries::ProviderRunState::Cancelled,
                output_fingerprint: [0; 32],
                diagnostic_code: Some("PYREFLY_CANCELLED".into()),
            });
        }
        events.send_progress(0, 2, "sidecar-connect")?;
        let socket = self.ensure_sidecar(&work, &request).map_err(|error| {
            self.results
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(request.provider_run_id.clone(), Err(error.clone()));
            ProviderRuntimeError::Adapter { code: error }
        })?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| ProviderRuntimeError::Adapter {
                code: error.to_string(),
            })?;
        let result = runtime.block_on(analyze_pyrefly_uds_cancellable(
            &socket,
            &request,
            cancellation.clone(),
        ));
        match result {
            Ok(run) => {
                if cancellation.is_cancelled() {
                    self.results
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .insert(
                            request.provider_run_id.clone(),
                            Err("PYREFLY_CANCELLED".to_owned()),
                        );
                    return Ok(ProviderCompletion {
                        state: crate::registries::ProviderRunState::Cancelled,
                        output_fingerprint: [0; 32],
                        diagnostic_code: Some("PYREFLY_CANCELLED".into()),
                    });
                }
                let output_fingerprint = digest_payload(&run.overall_digest)?;
                events.send_progress(2, 2, "sidecar-terminal-verified")?;
                self.results
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(request.provider_run_id.clone(), Ok(run));
                Ok(ProviderCompletion {
                    state: crate::registries::ProviderRunState::Succeeded,
                    output_fingerprint,
                    diagnostic_code: None,
                })
            }
            Err(PyreflyServiceError::Cancelled) => {
                self.results
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(
                        request.provider_run_id.clone(),
                        Err("PYREFLY_CANCELLED".to_owned()),
                    );
                Ok(ProviderCompletion {
                    state: crate::registries::ProviderRunState::Cancelled,
                    output_fingerprint: [0; 32],
                    diagnostic_code: Some("PYREFLY_CANCELLED".into()),
                })
            }
            Err(error) => {
                let message = error.to_string();
                self.results
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(request.provider_run_id.clone(), Err(message.clone()));
                if matches!(error, PyreflyServiceError::Transport(_)) {
                    let key = Self::supervisor_key(&request);
                    self.supervisors
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .remove(&key);
                    let _ = self.ensure_sidecar(&work, &request);
                }
                Err(ProviderRuntimeError::Adapter { code: message })
            }
        }
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

fn pyrefly_observation_contract() -> Result<&'static ProviderObservationSchema, PyreflyServiceError>
{
    PROVIDER_OBSERVATION_SCHEMAS
        .iter()
        .find(|schema| schema.provider_id == "pyrefly-python")
        .ok_or_else(|| {
            PyreflyServiceError::Protocol(
                "generated Pyrefly observation schema is absent".to_owned(),
            )
        })
}

fn expected_schema(contract: &ProviderObservationSchema) -> Schema {
    Schema::new_with_metadata(
        contract
            .fields
            .iter()
            .map(|field| {
                let data_type = match field.logical_type {
                    ProviderObservationLogicalType::Utf8 => DataType::Utf8,
                    ProviderObservationLogicalType::Binary => DataType::Binary,
                    ProviderObservationLogicalType::Boolean => DataType::Boolean,
                    ProviderObservationLogicalType::UInt64 => DataType::UInt64,
                    ProviderObservationLogicalType::Utf8List => DataType::List(
                        std::sync::Arc::new(Field::new_list_field(DataType::Utf8, false)),
                    ),
                };
                Field::new(field.name, data_type, field.nullable)
            })
            .collect::<Vec<_>>(),
        [(
            "codefabric.schema".to_owned(),
            contract.canonical_descriptor.to_owned(),
        )]
        .into_iter()
        .collect(),
    )
}

fn decode_batch(
    bytes: &[u8],
    contract: &ProviderObservationSchema,
) -> Result<RecordBatch, PyreflyServiceError> {
    let mut reader = FileReader::try_new(Cursor::new(bytes), None)
        .map_err(|error| PyreflyServiceError::Arrow(error.to_string()))?;
    if reader.schema().as_ref() != &expected_schema(contract) {
        return Err(PyreflyServiceError::Arrow(
            "schema differs from the application-owned contract".to_owned(),
        ));
    }
    let batch = reader
        .next()
        .transpose()
        .map_err(|error| PyreflyServiceError::Arrow(error.to_string()))?
        .ok_or_else(|| PyreflyServiceError::Arrow("IPC contains no batch".to_owned()))?;
    if reader.next().is_some() || batch.num_rows() != 1 {
        return Err(PyreflyServiceError::Arrow(
            "IPC must contain exactly one one-row batch".to_owned(),
        ));
    }
    for column in ["type_table_json", "callees_json", "diagnostics_json"] {
        let values = batch
            .column_by_name(column)
            .and_then(|array| array.as_any().downcast_ref::<BinaryArray>())
            .ok_or_else(|| PyreflyServiceError::Arrow(format!("{column} is not binary")))?;
        serde_json::from_slice::<serde_json::Value>(values.value(0))
            .map_err(|error| PyreflyServiceError::Arrow(format!("{column}: {error}")))?;
    }
    Ok(batch)
}

fn expected_module_digest(
    batch: &RecordBatch,
    module_id: &str,
    module_name: &str,
) -> Result<String, PyreflyServiceError> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(module_id.as_bytes());
    bytes.extend_from_slice(module_name.as_bytes());
    for column in ["type_table_json", "callees_json", "diagnostics_json"] {
        let values = batch
            .column_by_name(column)
            .and_then(|array| array.as_any().downcast_ref::<BinaryArray>())
            .ok_or_else(|| PyreflyServiceError::Arrow(format!("{column} is not binary")))?;
        bytes.extend_from_slice(values.value(0));
    }
    Ok(b3(&bytes))
}

fn header_matches(header: &AnalyzeEventHeader, request: &PyreflyRunRequest, sequence: u64) -> bool {
    header.provider_run_id == request.provider_run_id
        && header.workspace_id == request.workspace_id
        && header.analysis_context_id == request.analysis_context_id
        && header.source_generation == request.source_generation
        && header.sequence == sequence
        && header.context_manifest_digest == b3(&request.context_manifest)
        && header.source_manifest_digest == request.source_manifest_digest
}

struct AdmittedImmutableBlob {
    reference: BlobReference,
    bytes: Arc<[u8]>,
}

fn read_immutable_blob(
    input: &PyreflyModuleInput,
) -> Result<AdmittedImmutableBlob, PyreflyServiceError> {
    if !input.source_blob_path.is_absolute() || !input.source_blob_path.is_file() {
        return Err(PyreflyServiceError::Invalid(
            "module source blob path must be an existing absolute file".to_owned(),
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
pub async fn analyze_pyrefly_uds(
    socket: &Path,
    request: &PyreflyRunRequest,
) -> Result<AcceptedPyreflyRun, PyreflyServiceError> {
    analyze_pyrefly_uds_inner(socket, request, None).await
}

async fn analyze_pyrefly_uds_cancellable(
    socket: &Path,
    request: &PyreflyRunRequest,
    cancellation: crate::cancellation::Cancellation,
) -> Result<AcceptedPyreflyRun, PyreflyServiceError> {
    analyze_pyrefly_uds_inner(socket, request, Some(cancellation)).await
}

#[allow(clippy::too_many_lines)] // One sidecar stream validator keeps every ordered correlation and terminal check adjacent.
async fn analyze_pyrefly_uds_inner(
    socket: &Path,
    request: &PyreflyRunRequest,
    cancellation: Option<crate::cancellation::Cancellation>,
) -> Result<AcceptedPyreflyRun, PyreflyServiceError> {
    if request.modules.is_empty()
        || request.provider_run_id.is_empty()
        || request.workspace_id.is_empty()
        || request.analysis_context_id.is_empty()
        || request.canonical_workspace_id == [0; 16]
        || request.canonical_analysis_context_id == [0; 16]
        || !valid_digest(&request.source_manifest_digest)
        || !valid_digest(&request.output_schema_bundle_digest)
    {
        return Err(PyreflyServiceError::Invalid(
            "run identity, modules, or digests are incomplete".to_owned(),
        ));
    }
    let observation_contract = pyrefly_observation_contract()?;
    let observation_family_code = u32::from(observation_contract.observation_family_code);
    let observation_schema_digest = observation_contract.schema_digest.to_owned();
    let admitted_blobs = request
        .modules
        .iter()
        .map(read_immutable_blob)
        .collect::<Result<Vec<_>, _>>()?;
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
        .max_decoding_message_size(68 * 1024 * 1024)
        .max_encoding_message_size(4 * 1024 * 1024);
    let acknowledgement = client
        .handshake(Hello {
            protocol_major: 1,
            protocol_minor: 0,
            required_feature_bits: REQUIRED_FEATURE_BITS,
            optional_feature_bits: OPTIONAL_FEATURE_BITS,
            daemon_build: "codefabricd 0.1.0".to_owned(),
            supported_python_versions: vec!["3.14".to_owned()],
            observation_schema_digests: vec![observation_schema_digest.clone()],
            maximum_frame_bytes: 4 * 1024 * 1024,
            maximum_arrow_chunk_bytes: 64 * 1024 * 1024,
            sandbox_profile_digest: SANDBOX_PROFILE_DIGEST.to_owned(),
        })
        .await
        .map_err(|error| PyreflyServiceError::Protocol(error.to_string()))?
        .into_inner();
    if acknowledgement.protocol_major != 1
        || acknowledgement.protocol_minor != 0
        || acknowledgement.negotiated_feature_bits != REQUIRED_FEATURE_BITS | OPTIONAL_FEATURE_BITS
        || acknowledgement.pyrefly_source_digest != PYREFLY_SOURCE_DIGEST
        || acknowledgement.observation_schema_digests != [observation_schema_digest.clone()]
        || acknowledgement.sandbox_profile_digest != SANDBOX_PROFILE_DIGEST
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
        })
        .await
        .map_err(|error| PyreflyServiceError::Protocol(error.to_string()))?
        .into_inner();
    if opened.context_handle.is_empty() || opened.context_manifest_digest != context_digest {
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
            context_manifest_digest: context_digest,
            source_generation: request.source_generation,
            source_snapshot_lease_id: request.source_snapshot_lease_id.clone(),
            modules,
            requested_capability_codes: request.requested_capability_codes.clone(),
            deadline_unix_ms: request.deadline_unix_ms,
            output_schema_bundle_digest: request.output_schema_bundle_digest.clone(),
            initial_chunk_credits: 4,
            initial_credit_bytes: MAX_UNACKNOWLEDGED_BYTES,
            sandbox_profile_digest: SANDBOX_PROFILE_DIGEST.to_owned(),
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
    let mut accepted = Vec::new();
    let mut open_module: Option<String> = None;
    let mut pending: Option<AcceptedPyreflyModule> = None;
    let mut terminal = None;
    let mut cancelled_terminal = false;
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
                    || event.granted_chunk_credits != 4
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
                if open_module.is_some() || !header_matches(&header, request, sequence) {
                    return Err(PyreflyServiceError::Protocol(
                        "module-begin order or correlation differs".to_owned(),
                    ));
                }
                open_module = Some(event.module_id);
            }
            Event::ObservationBatchChunk(event) => {
                sequence += 1;
                let header = event.header.ok_or_else(|| {
                    PyreflyServiceError::Protocol("chunk header is absent".to_owned())
                })?;
                if open_module.as_deref() != Some(event.module_id.as_str())
                    || pending.is_some()
                    || !header_matches(&header, request, sequence)
                    || event.observation_family_code != observation_family_code
                    || event.schema_digest != observation_schema_digest
                    || event.chunk_digest != b3(&event.arrow_ipc)
                    || event.row_count != 1
                {
                    let _ = command_sender
                        .send(AnalyzeCommand {
                            command: Some(Command::ChunkRejected(ChunkRejected {
                                sequence,
                                error_code: "PYREFLY_CHUNK_IDENTITY_REJECTED".to_owned(),
                            })),
                        })
                        .await;
                    return Err(PyreflyServiceError::Protocol(
                        "chunk identity, digest, schema, or count differs".to_owned(),
                    ));
                }
                let batch = match decode_batch(&event.arrow_ipc, observation_contract) {
                    Ok(batch) => batch,
                    Err(error) => {
                        let _ = command_sender
                            .send(AnalyzeCommand {
                                command: Some(Command::ChunkRejected(ChunkRejected {
                                    sequence,
                                    error_code: "PYREFLY_CHUNK_ARROW_REJECTED".to_owned(),
                                })),
                            })
                            .await;
                        return Err(error);
                    }
                };
                let module_name = batch
                    .column_by_name("module_name")
                    .and_then(|array| array.as_any().downcast_ref::<StringArray>())
                    .map(|array| array.value(0).to_owned())
                    .ok_or_else(|| {
                        PyreflyServiceError::Arrow("module_name is not UTF-8".to_owned())
                    })?;
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
                let accepted_bytes = u64::try_from(event.arrow_ipc.len()).unwrap_or(u64::MAX);
                pending = Some(AcceptedPyreflyModule {
                    module_id: event.module_id,
                    module_name,
                    canonical_file_id,
                    source_bytes,
                    arrow_ipc: event.arrow_ipc,
                    batch,
                    schema_digest: event.schema_digest,
                    chunk_digest: event.chunk_digest,
                    module_digest: String::new(),
                });
                command_sender
                    .send(AnalyzeCommand {
                        command: Some(Command::ChunkAccepted(ChunkAccepted {
                            sequence,
                            next_credit_bytes: accepted_bytes,
                            next_credit_chunks: 1,
                        })),
                    })
                    .await
                    .map_err(|_| {
                        PyreflyServiceError::Protocol(
                            "chunk acknowledgement stream closed".to_owned(),
                        )
                    })?;
            }
            Event::ModuleEnd(event) => {
                sequence += 1;
                let header = event.header.ok_or_else(|| {
                    PyreflyServiceError::Protocol("module-end header is absent".to_owned())
                })?;
                let mut module = pending.take().ok_or_else(|| {
                    PyreflyServiceError::Protocol("module ended without a chunk".to_owned())
                })?;
                if open_module.take().as_deref() != Some(event.module_id.as_str())
                    || module.module_id != event.module_id
                    || event.family_counts.get(&observation_family_code) != Some(&1)
                    || !header_matches(&header, request, sequence)
                    || !valid_digest(&event.module_digest)
                    || event.module_digest
                        != expected_module_digest(
                            &module.batch,
                            &module.module_id,
                            &module.module_name,
                        )?
                {
                    return Err(PyreflyServiceError::Protocol(
                        "module terminal identity or counts differ".to_owned(),
                    ));
                }
                module.module_digest = event.module_digest;
                accepted.push(module);
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
                let expected_rechecked = request
                    .modules
                    .iter()
                    .map(|module| module.module_id.clone())
                    .collect::<Vec<_>>();
                let terminal_state =
                    WireProviderRunState::try_from(event.terminal_state).map_err(|_| {
                        PyreflyServiceError::Protocol(
                            "run terminal state is unregistered".to_owned(),
                        )
                    })?;
                let cancellation_expected = cancellation
                    .as_ref()
                    .is_some_and(crate::cancellation::Cancellation::is_cancelled);
                let success_outcomes = event.capability_outcomes.iter().all(|outcome| {
                    outcome.owner_capability_state_code == 10
                        && outcome.completeness_state_code == 10
                        && outcome.reason_code == "PYREFLY_SUCCEEDED"
                });
                let cancelled_outcomes = event.capability_outcomes.iter().all(|outcome| {
                    outcome.owner_capability_state_code == 30
                        && outcome.completeness_state_code == 40
                        && outcome.reason_code == "PYREFLY_CANCELLED"
                });
                if open_module.is_some()
                    || pending.is_some()
                    || terminal.is_some()
                    || !header_matches(&header, request, sequence)
                    || event.ordered_module_digests != ordered_module_digests
                    || event.overall_digest != expected_overall_digest
                    || event.rechecked_module_ids != expected_rechecked
                    || event.sandbox_profile_digest != SANDBOX_PROFILE_DIGEST
                    || event.trust_profile != TRUST_PROFILE
                    || !((terminal_state == WireProviderRunState::Succeeded && success_outcomes)
                        || (cancellation_expected
                            && terminal_state == WireProviderRunState::Cancelled
                            && cancelled_outcomes))
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
            Event::RunProgress(_) => {}
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

    use tokio_stream::Stream;
    use tokio_stream::wrappers::UnixListenerStream;
    use tonic::{Request, Response, Status};

    use super::*;
    use crate::rpc::generated::codefabric::provider::v1::{
        CancelAcknowledgement, CapabilityOutcome,
    };
    use crate::rpc::generated::codefabric::pyrefly::v1::analyze_event::Event as MockEvent;
    use crate::rpc::generated::codefabric::pyrefly::v1::pyrefly_sidecar_server::{
        PyreflySidecar, PyreflySidecarServer,
    };
    use crate::rpc::generated::codefabric::pyrefly::v1::{
        AnalyzeEvent, CloseContextRequest, CloseContextResponse, HelloAck, ModuleBegin,
        ObservationBatchChunk, OpenContextResponse, RunAccepted, RunTerminal, ShutdownRequest,
        ShutdownResponse,
    };

    #[derive(Clone, Copy)]
    enum MalformedMode {
        StaleGeneration,
        MissingModuleEnd,
        DigestMismatch,
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
                maximum_arrow_chunk_bytes: request.maximum_arrow_chunk_bytes,
                sandbox_profile_digest: SANDBOX_PROFILE_DIGEST.to_owned(),
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
            };
            let accepted_generation = if matches!(self.mode, MalformedMode::StaleGeneration) {
                start.source_generation.saturating_sub(1)
            } else {
                start.source_generation
            };
            let mut events = vec![AnalyzeEvent {
                event: Some(MockEvent::RunAccepted(RunAccepted {
                    header: Some(header(0, accepted_generation)),
                    granted_chunk_credits: 4,
                    granted_credit_bytes: MAX_UNACKNOWLEDGED_BYTES,
                })),
            }];
            if !matches!(self.mode, MalformedMode::StaleGeneration) {
                let module_id = start.modules[0].module_id.clone();
                events.push(AnalyzeEvent {
                    event: Some(MockEvent::ModuleBegin(ModuleBegin {
                        header: Some(header(1, start.source_generation)),
                        module_id: module_id.clone(),
                    })),
                });
                match self.mode {
                    MalformedMode::MissingModuleEnd => {
                        events.push(AnalyzeEvent {
                            event: Some(MockEvent::RunTerminal(RunTerminal {
                                header: Some(header(2, start.source_generation)),
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
                                sandbox_profile_digest: SANDBOX_PROFILE_DIGEST.to_owned(),
                                trust_profile: TRUST_PROFILE.to_owned(),
                            })),
                        });
                    }
                    MalformedMode::DigestMismatch => {
                        let contract = pyrefly_observation_contract().unwrap();
                        events.push(AnalyzeEvent {
                            event: Some(MockEvent::ObservationBatchChunk(ObservationBatchChunk {
                                header: Some(header(2, start.source_generation)),
                                module_id,
                                observation_family_code: u32::from(
                                    contract.observation_family_code,
                                ),
                                arrow_ipc: Vec::new(),
                                payload_reference: None,
                                schema_digest: contract.schema_digest.to_owned(),
                                row_count: 1,
                                chunk_digest: b3(b"not-the-empty-payload"),
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
            output_schema_bundle_digest: b3(include_bytes!(
                "../contracts/schema/schema-contract-ir.json"
            )),
        }
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Three adversarial servers exercise the production stream validator end to end.
    async fn pyrefly_stale_generation_rejection_falsification() {
        let directory = tempfile::tempdir().unwrap();
        let request = malformed_request(directory.path());
        let mut failures = Vec::new();
        for (index, mode) in [
            MalformedMode::StaleGeneration,
            MalformedMode::MissingModuleEnd,
            MalformedMode::DigestMismatch,
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
            let error = analyze_pyrefly_uds(&socket, &request).await.unwrap_err();
            failures.push(error.to_string());
            let _ = shutdown_sender.send(());
            server.await.unwrap().unwrap();
            let _ = std::fs::remove_file(socket);
        }
        assert!(failures[0].contains("accepted correlation or sequence differs"));
        assert!(failures[1].contains("run terminal identity, order, or state differs"));
        assert!(failures[2].contains("chunk identity, digest, schema, or count differs"));
        assert_eq!(
            failures.len(),
            3,
            "no malformed stream reached an accepted run"
        );
    }

    #[tokio::test]
    async fn pyrefly_crash_restart_operational_gate() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = directory.path().join("workspace");
        let dependencies = directory.path().join("dependencies");
        let output = directory.path().join("output");
        for path in [&workspace, &dependencies, &output] {
            std::fs::create_dir(path).unwrap();
        }
        let request = malformed_request(&workspace);
        let view = crate::source_image::ProviderWorkspaceView {
            workspace_id: request.canonical_workspace_id,
            source_generation: request.source_generation,
            workspace_root: workspace,
            dependency_root: dependencies,
            output_root: output,
            manifest_path: directory.path().join("manifest.json"),
            manifest_digest: [0x71; 32],
            dependency_manifest_digest: [0x72; 32],
            sandbox_profile_digest: SANDBOX_PROFILE_DIGEST.to_owned(),
            entries: Vec::new(),
        };
        let work = crate::provider_runtime::SemanticProviderWork {
            provider_id: "pyrefly-python".to_owned(),
            capability_family: "PYTHON_SEMANTIC".to_owned(),
            workspace_view: view,
            trust_profile: crate::provider_sandbox::ProviderTrustProfile::UntrustedSandboxed,
            invocation_manifest: PyreflyProviderDriver::invocation_manifest(&request).unwrap(),
        };
        let binary = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/debug/codefabric-pyrefly-sidecar");
        assert!(
            binary.is_file(),
            "build the sidecar before the WP08 operational oracle"
        );
        let driver =
            PyreflyProviderDriver::new(binary, directory.path().join("supervisor-state")).unwrap();
        let first_socket = driver.ensure_sidecar(&work, &request).unwrap();
        let first_request = request.clone();
        let first_socket_for_run = first_socket.clone();
        let active = tokio::spawn(async move {
            analyze_pyrefly_uds(&first_socket_for_run, &first_request).await
        });
        tokio::time::sleep(Duration::from_millis(10)).await;
        {
            let key = PyreflyProviderDriver::supervisor_key(&request);
            let mut supervisors = driver
                .supervisors
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let sidecar = supervisors.get_mut(&key).unwrap();
            sidecar.child.kill().unwrap();
            sidecar.child.wait().unwrap();
        }
        let rejection = active.await.unwrap().unwrap_err();
        assert!(matches!(
            rejection,
            PyreflyServiceError::Transport(_) | PyreflyServiceError::Protocol(_)
        ));

        let second_socket = driver.ensure_sidecar(&work, &request).unwrap();
        assert_ne!(first_socket, second_socket, "restart must reopen a new UDS");
        let restart_attempt = driver
            .supervisors
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&PyreflyProviderDriver::supervisor_key(&request))
            .unwrap()
            .restart_attempt;
        assert_eq!(restart_attempt, 1, "the first retry uses the backoff path");
        let recovered = analyze_pyrefly_uds(&second_socket, &request).await.unwrap();
        assert_eq!(recovered.source_generation, request.source_generation);
        assert_eq!(recovered.modules.len(), 1);
        assert_eq!(recovered.rechecked_module_ids, ["module-malformed"]);
        assert_eq!(recovered.sandbox_profile_digest, SANDBOX_PROFILE_DIGEST);
        assert_eq!(recovered.trust_profile, TRUST_PROFILE);
    }
}
