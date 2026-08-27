//! Stable-daemon adapter for the isolated Pyrefly sidecar.
//!
//! Only application-owned Arrow batches and correlation DTOs cross this module. No Pyrefly
//! library type is linked into or exposed by the stable daemon.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use arrow_array::{BinaryArray, RecordBatch, StringArray};
use arrow_ipc::reader::FileReader;
use arrow_schema::{DataType, Field, Schema};
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tonic::transport::Endpoint;
use tower::service_fn;

use crate::rpc::generated::codefabric::provider::v1::{
    BlobReference, ProviderRunState, SourceSnapshotLease,
};
use crate::rpc::generated::codefabric::pyrefly::v1::analyze_command::Command;
use crate::rpc::generated::codefabric::pyrefly::v1::analyze_event::Event;
use crate::rpc::generated::codefabric::pyrefly::v1::pyrefly_sidecar_client::PyreflySidecarClient as WireClient;
use crate::rpc::generated::codefabric::pyrefly::v1::{
    AnalyzeCommand, AnalyzeEventHeader, AnalyzeModulesRequest, Hello, ModuleRequest,
    OpenContextRequest,
};

const OBSERVATION_FAMILY_CODE: u32 = 110;
const SCHEMA_DESCRIPTOR: &str =
    include_str!("../contracts/schema/provider-observations/pyrefly-module-v1.json");
const PYREFLY_SOURCE_DIGEST: &str =
    "b3:1b9e72144644d1b3df0bdca564496566238543dfb7f576980a8408714327fc3e";
const SANDBOX_PROFILE_DIGEST: &str =
    "b3:8a663d1d6ddbcf830a09e28c7ee6bcd65b433fd9b69b597dbe99f02c78ce8e15";

/// One immutable Python module admitted to the sidecar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PyreflyModuleInput {
    pub module_id: String,
    pub module_name: String,
    pub file_id: String,
    pub source_blob_path: PathBuf,
    pub content_digest: String,
}

/// Complete identity and capability selection for one sidecar run.
#[derive(Clone, Debug, Eq, PartialEq)]
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
    #[error("Pyrefly source read failed at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

fn b3(bytes: &[u8]) -> String {
    format!("b3:{}", blake3::hash(bytes).to_hex())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 67
        && value.starts_with("b3:")
        && value[3..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn schema_digest() -> String {
    b3(SCHEMA_DESCRIPTOR.as_bytes())
}

fn expected_schema() -> Schema {
    Schema::new_with_metadata(
        vec![
            Field::new("module_id", DataType::Utf8, false),
            Field::new("module_name", DataType::Utf8, false),
            Field::new("type_table_json", DataType::Binary, false),
            Field::new("callees_json", DataType::Binary, false),
            Field::new("diagnostics_json", DataType::Binary, false),
        ],
        [("codefabric.schema".to_owned(), SCHEMA_DESCRIPTOR.to_owned())]
            .into_iter()
            .collect(),
    )
}

fn decode_batch(bytes: &[u8]) -> Result<RecordBatch, PyreflyServiceError> {
    let mut reader = FileReader::try_new(Cursor::new(bytes), None)
        .map_err(|error| PyreflyServiceError::Arrow(error.to_string()))?;
    if reader.schema().as_ref() != &expected_schema() {
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

fn header_matches(header: &AnalyzeEventHeader, request: &PyreflyRunRequest, sequence: u64) -> bool {
    header.provider_run_id == request.provider_run_id
        && header.workspace_id == request.workspace_id
        && header.analysis_context_id == request.analysis_context_id
        && header.source_generation == request.source_generation
        && header.sequence == sequence
        && header.context_manifest_digest == b3(&request.context_manifest)
        && header.source_manifest_digest == request.source_manifest_digest
}

fn read_immutable_blob(input: &PyreflyModuleInput) -> Result<BlobReference, PyreflyServiceError> {
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
    Ok(BlobReference {
        blob_id: format!("blob:{}", &b3(&bytes)[3..35]),
        content_digest: b3(&bytes),
        byte_length: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        read_only_uri: format!("file://{}", input.source_blob_path.display()),
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
    let blobs = request
        .modules
        .iter()
        .map(read_immutable_blob)
        .collect::<Result<Vec<_>, _>>()?;
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
            required_feature_bits: 0,
            optional_feature_bits: 0,
            daemon_build: "codefabricd 0.1.0".to_owned(),
            supported_python_versions: vec!["3.14".to_owned()],
            observation_schema_digests: vec![schema_digest()],
            maximum_frame_bytes: 4 * 1024 * 1024,
            maximum_arrow_chunk_bytes: 64 * 1024 * 1024,
            sandbox_profile_digest: SANDBOX_PROFILE_DIGEST.to_owned(),
        })
        .await
        .map_err(|error| PyreflyServiceError::Protocol(error.to_string()))?
        .into_inner();
    if acknowledgement.protocol_major != 1
        || acknowledgement.protocol_minor != 0
        || acknowledgement.pyrefly_source_digest != PYREFLY_SOURCE_DIGEST
        || acknowledgement.observation_schema_digests != [schema_digest()]
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
            initial_credit_bytes: 64 * 1024 * 1024,
        })),
    };
    let mut stream = client
        .analyze_modules(tokio_stream::iter([start]))
        .await
        .map_err(|error| PyreflyServiceError::Protocol(error.to_string()))?
        .into_inner();
    let mut sequence = 0_u64;
    let mut accepted = Vec::new();
    let mut open_module: Option<String> = None;
    let mut pending: Option<AcceptedPyreflyModule> = None;
    let mut terminal = None;
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
                if sequence != 0 || !header_matches(&header, request, sequence) {
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
                    || event.observation_family_code != OBSERVATION_FAMILY_CODE
                    || event.schema_digest != schema_digest()
                    || event.chunk_digest != b3(&event.arrow_ipc)
                    || event.row_count != 1
                {
                    return Err(PyreflyServiceError::Protocol(
                        "chunk identity, digest, schema, or count differs".to_owned(),
                    ));
                }
                let batch = decode_batch(&event.arrow_ipc)?;
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
                let source_bytes =
                    std::fs::read(&requested_module.source_blob_path).map_err(|source| {
                        PyreflyServiceError::Io {
                            path: requested_module.source_blob_path.clone(),
                            source,
                        }
                    })?;
                if b3(&source_bytes) != requested_module.content_digest {
                    return Err(PyreflyServiceError::Invalid(
                        "module source changed after lease admission".to_owned(),
                    ));
                }
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
                    || event.family_counts.get(&OBSERVATION_FAMILY_CODE) != Some(&1)
                    || !header_matches(&header, request, sequence)
                    || !valid_digest(&event.module_digest)
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
                if open_module.is_some()
                    || pending.is_some()
                    || terminal.is_some()
                    || !header_matches(&header, request, sequence)
                    || event.terminal_state != ProviderRunState::Succeeded as i32
                    || event.ordered_module_digests
                        != accepted
                            .iter()
                            .map(|module| module.module_digest.clone())
                            .collect::<Vec<_>>()
                    || !valid_digest(&event.overall_digest)
                {
                    return Err(PyreflyServiceError::Protocol(
                        "run terminal identity, order, or state differs".to_owned(),
                    ));
                }
                terminal = Some((
                    event
                        .capability_outcomes
                        .iter()
                        .map(|outcome| outcome.capability_code)
                        .collect::<Vec<_>>(),
                    event.overall_digest,
                ));
            }
            Event::RunProgress(_) => {}
        }
    }
    let (capability_codes, overall_digest) = terminal
        .ok_or_else(|| PyreflyServiceError::Protocol("stream ended before terminal".to_owned()))?;
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
    })
}
