//! Application-owned Pyrefly gRPC service over the released private protocol.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio_stream::Stream;
use tokio_stream::wrappers::{ReceiverStream, UnixListenerStream};
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use crate::protocol::generated::codefabric::provider::v1::{
    CancelAcknowledgement, CancelAcknowledgementState, CapabilityOutcome, ProviderRunState,
    SourceSnapshotLease,
};
use crate::protocol::generated::codefabric::pyrefly::v1::analyze_command::Command;
use crate::protocol::generated::codefabric::pyrefly::v1::analyze_event::Event;
use crate::protocol::generated::codefabric::pyrefly::v1::pyrefly_sidecar_server::{
    PyreflySidecar, PyreflySidecarServer,
};
use crate::protocol::generated::codefabric::pyrefly::v1::{
    AnalyzeCommand, AnalyzeEvent, AnalyzeEventHeader, CancelRunRequest, CloseContextRequest,
    CloseContextResponse, Hello, HelloAck, ModuleBegin, ModuleEnd, ObservationBatchChunk,
    OpenContextRequest, OpenContextResponse, RunAccepted, RunTerminal, ShutdownRequest,
    ShutdownResponse,
};

const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
const MAX_ARROW_CHUNK_BYTES: usize = 64 * 1024 * 1024;
const PYREFLY_SOURCE_DIGEST: &str =
    "b3:1b9e72144644d1b3df0bdca564496566238543dfb7f576980a8408714327fc3e";
const SANDBOX_PROFILE_DIGEST: &str =
    "b3:8a663d1d6ddbcf830a09e28c7ee6bcd65b433fd9b69b597dbe99f02c78ce8e15";

#[derive(Clone)]
struct OpenContext {
    workspace_id: String,
    analysis_context_id: String,
    manifest_digest: String,
    lease: SourceSnapshotLease,
}

#[derive(Clone, Default)]
struct Service {
    contexts: Arc<Mutex<BTreeMap<String, OpenContext>>>,
}

fn now_millis() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(i64::MAX)
}

fn b3(bytes: &[u8]) -> String {
    format!("b3:{}", blake3::hash(bytes).to_hex())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 67
        && value.starts_with("b3:")
        && value[3..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn header(
    start: &crate::protocol::generated::codefabric::pyrefly::v1::AnalyzeModulesRequest,
    source_manifest_digest: &str,
    sequence: u64,
) -> AnalyzeEventHeader {
    AnalyzeEventHeader {
        provider_run_id: start.provider_run_id.clone(),
        workspace_id: start.workspace_id.clone(),
        analysis_context_id: start.analysis_context_id.clone(),
        source_generation: start.source_generation,
        sequence,
        context_manifest_digest: start.context_manifest_digest.clone(),
        source_manifest_digest: source_manifest_digest.to_owned(),
    }
}

fn source_path(uri: &str) -> Result<PathBuf, Status> {
    let raw = uri
        .strip_prefix("file://")
        .ok_or_else(|| Status::invalid_argument("Pyrefly source blob URI must use file://"))?;
    let path = PathBuf::from(raw);
    if !path.is_absolute() || !path.is_file() {
        return Err(Status::failed_precondition(
            "Pyrefly source blob is not an existing absolute file",
        ));
    }
    Ok(path)
}

fn validate_source(
    module: &crate::protocol::generated::codefabric::pyrefly::v1::ModuleRequest,
    lease: &SourceSnapshotLease,
) -> Result<PathBuf, Status> {
    let blob = module
        .source_blob
        .as_ref()
        .ok_or_else(|| Status::invalid_argument("Pyrefly module lacks a source blob"))?;
    if !valid_digest(&blob.content_digest)
        || blob.content_digest != module.source_digest
        || !lease.blobs.iter().any(|candidate| candidate == blob)
    {
        return Err(Status::failed_precondition(
            "Pyrefly module source is outside the opened immutable lease",
        ));
    }
    let path = source_path(&blob.read_only_uri)?;
    let bytes = fs::read(&path).map_err(|_| Status::data_loss("read Pyrefly source blob"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != blob.byte_length
        || b3(&bytes) != blob.content_digest
    {
        return Err(Status::data_loss("Pyrefly source blob digest differs"));
    }
    Ok(path)
}

#[tonic::async_trait]
impl PyreflySidecar for Service {
    async fn handshake(&self, request: Request<Hello>) -> Result<Response<HelloAck>, Status> {
        let hello = request.into_inner();
        if hello.protocol_major != 1
            || hello.protocol_minor != 0
            || hello.required_feature_bits != 0
            || hello.maximum_frame_bytes == 0
            || hello.maximum_arrow_chunk_bytes == 0
            || hello.sandbox_profile_digest != SANDBOX_PROFILE_DIGEST
        {
            return Err(Status::failed_precondition(
                "Pyrefly handshake identity or limits differ",
            ));
        }
        Ok(Response::new(HelloAck {
            protocol_major: 1,
            protocol_minor: 0,
            negotiated_feature_bits: hello.optional_feature_bits,
            sidecar_build: "codefabric-pyrefly-sidecar 0.1.0".to_owned(),
            pyrefly_source_digest: PYREFLY_SOURCE_DIGEST.to_owned(),
            supported_python_versions: hello.supported_python_versions,
            observation_schema_digests: vec![crate::pyrefly_link::schema_digest()],
            maximum_frame_bytes: u64::try_from(MAX_FRAME_BYTES).unwrap(),
            maximum_arrow_chunk_bytes: u64::try_from(MAX_ARROW_CHUNK_BYTES).unwrap(),
            sandbox_profile_digest: SANDBOX_PROFILE_DIGEST.to_owned(),
        }))
    }

    async fn open_context(
        &self,
        request: Request<OpenContextRequest>,
    ) -> Result<Response<OpenContextResponse>, Status> {
        let request = request.into_inner();
        let lease = request
            .source_snapshot_lease
            .ok_or_else(|| Status::invalid_argument("Pyrefly context lacks a source lease"))?;
        if request.workspace_id.is_empty()
            || request.analysis_context_id.is_empty()
            || !valid_digest(&request.context_manifest_digest)
            || b3(&request.immutable_context_manifest) != request.context_manifest_digest
            || lease.workspace_id != request.workspace_id
            || lease.expires_at_unix_ms <= now_millis()
        {
            return Err(Status::failed_precondition(
                "Pyrefly context identity, manifest, or lease differs",
            ));
        }
        let handle = format!(
            "pyrefly-context:{}",
            &b3(&[
                request.workspace_id.as_bytes(),
                request.analysis_context_id.as_bytes(),
                request.context_manifest_digest.as_bytes(),
                lease.lease_id.as_bytes(),
            ]
            .concat())[3..35]
        );
        self.contexts.lock().unwrap().insert(
            handle.clone(),
            OpenContext {
                workspace_id: request.workspace_id,
                analysis_context_id: request.analysis_context_id,
                manifest_digest: request.context_manifest_digest.clone(),
                lease,
            },
        );
        Ok(Response::new(OpenContextResponse {
            context_handle: handle,
            context_manifest_digest: request.context_manifest_digest,
            opened_at_unix_ms: now_millis(),
        }))
    }

    type AnalyzeModulesStream =
        Pin<Box<dyn Stream<Item = Result<AnalyzeEvent, Status>> + Send + 'static>>;

    #[allow(clippy::too_many_lines)] // One ordered provider stream keeps lease, module, digest, and terminal correlation adjacent.
    async fn analyze_modules(
        &self,
        request: Request<tonic::Streaming<AnalyzeCommand>>,
    ) -> Result<Response<Self::AnalyzeModulesStream>, Status> {
        let mut commands = request.into_inner();
        let first = commands
            .message()
            .await?
            .ok_or_else(|| Status::invalid_argument("Pyrefly analysis stream is empty"))?;
        let Some(Command::Start(start)) = first.command else {
            return Err(Status::failed_precondition(
                "Pyrefly analysis stream must begin with start",
            ));
        };
        let context = self
            .contexts
            .lock()
            .unwrap()
            .get(&start.context_handle)
            .cloned()
            .ok_or_else(|| Status::not_found("Pyrefly context handle is unknown"))?;
        if start.workspace_id != context.workspace_id
            || start.analysis_context_id != context.analysis_context_id
            || start.context_manifest_digest != context.manifest_digest
            || start.source_snapshot_lease_id != context.lease.lease_id
            || start.source_generation != context.lease.source_generation
            || start.deadline_unix_ms <= now_millis()
            || start.initial_chunk_credits == 0
            || start.initial_credit_bytes == 0
            || !valid_digest(&start.output_schema_bundle_digest)
            || start.modules.is_empty()
        {
            return Err(Status::failed_precondition(
                "Pyrefly analysis identity, lease, credits, or deadline differs",
            ));
        }
        let (sender, receiver) = tokio::sync::mpsc::channel(4);
        tokio::spawn(async move {
            let source_manifest = context.lease.source_manifest_digest.clone();
            let _ = sender
                .send(Ok(AnalyzeEvent {
                    event: Some(Event::RunAccepted(RunAccepted {
                        header: Some(header(&start, &source_manifest, 0)),
                        granted_chunk_credits: start.initial_chunk_credits.min(4),
                        granted_credit_bytes: start
                            .initial_credit_bytes
                            .min(u64::try_from(MAX_ARROW_CHUNK_BYTES).unwrap()),
                    })),
                }))
                .await;
            let inputs = start
                .modules
                .iter()
                .map(|module| {
                    validate_source(module, &context.lease).map(|source_path| {
                        crate::pyrefly_link::ModuleInput {
                            module_id: module.module_id.clone(),
                            module_name: module.module_name.clone(),
                            source_path,
                        }
                    })
                })
                .collect::<Result<Vec<_>, _>>();
            let inputs = match inputs {
                Ok(inputs) => inputs,
                Err(error) => {
                    let _ = sender.send(Err(error)).await;
                    return;
                }
            };
            let analyses = match tokio::task::spawn_blocking(move || {
                crate::pyrefly_link::analyze_modules(&inputs)
            })
            .await
            {
                Ok(Ok(analyses)) => analyses,
                Ok(Err(error)) => {
                    let _ = sender.send(Err(Status::internal(error))).await;
                    return;
                }
                Err(error) => {
                    let _ = sender
                        .send(Err(Status::internal(format!(
                            "Pyrefly analysis task failed: {error}"
                        ))))
                        .await;
                    return;
                }
            };
            let mut sequence = 0_u64;
            let mut module_digests = Vec::new();
            for (module, analysis) in start.modules.iter().zip(analyses) {
                sequence += 1;
                if sender
                    .send(Ok(AnalyzeEvent {
                        event: Some(Event::ModuleBegin(ModuleBegin {
                            header: Some(header(&start, &source_manifest, sequence)),
                            module_id: module.module_id.clone(),
                        })),
                    }))
                    .await
                    .is_err()
                {
                    return;
                }
                if analysis.module_id != module.module_id {
                    let _ = sender
                        .send(Err(Status::internal(
                            "Pyrefly module result order differs from the admitted request",
                        )))
                        .await;
                    return;
                }
                sequence += 1;
                let chunk_digest = b3(&analysis.arrow_ipc);
                if sender
                    .send(Ok(AnalyzeEvent {
                        event: Some(Event::ObservationBatchChunk(ObservationBatchChunk {
                            header: Some(header(&start, &source_manifest, sequence)),
                            module_id: module.module_id.clone(),
                            observation_family_code: crate::pyrefly_link::observation_family_code(),
                            arrow_ipc: analysis.arrow_ipc,
                            payload_reference: None,
                            schema_digest: analysis.schema_digest,
                            row_count: analysis.row_count,
                            chunk_digest,
                        })),
                    }))
                    .await
                    .is_err()
                {
                    return;
                }
                sequence += 1;
                module_digests.push(analysis.module_digest.clone());
                if sender
                    .send(Ok(AnalyzeEvent {
                        event: Some(Event::ModuleEnd(ModuleEnd {
                            header: Some(header(&start, &source_manifest, sequence)),
                            module_id: module.module_id.clone(),
                            family_counts: [(crate::pyrefly_link::observation_family_code(), 1)]
                                .into_iter()
                                .collect(),
                            module_digest: analysis.module_digest,
                        })),
                    }))
                    .await
                    .is_err()
                {
                    return;
                }
            }
            sequence += 1;
            let overall_digest = b3(module_digests
                .iter()
                .flat_map(std::string::String::as_bytes)
                .copied()
                .collect::<Vec<_>>()
                .as_slice());
            let outcomes = start
                .requested_capability_codes
                .iter()
                .map(|capability_code| CapabilityOutcome {
                    capability_code: *capability_code,
                    owner_capability_state_code: 10,
                    completeness_state_code: 10,
                    reason_code: "PYREFLY_SUCCEEDED".to_owned(),
                })
                .collect();
            let _ = sender
                .send(Ok(AnalyzeEvent {
                    event: Some(Event::RunTerminal(RunTerminal {
                        header: Some(header(&start, &source_manifest, sequence)),
                        ordered_module_digests: module_digests,
                        capability_outcomes: outcomes,
                        overall_digest,
                        terminal_state: ProviderRunState::Succeeded as i32,
                    })),
                }))
                .await;
        });
        Ok(Response::new(Box::pin(ReceiverStream::new(receiver))))
    }

    async fn cancel_run(
        &self,
        request: Request<CancelRunRequest>,
    ) -> Result<Response<CancelAcknowledgement>, Status> {
        Ok(Response::new(CancelAcknowledgement {
            provider_run_id: request.into_inner().provider_run_id,
            state: CancelAcknowledgementState::NotFound as i32,
            acknowledged_at_unix_ms: now_millis(),
            terminal_state: None,
            cleaning_up_components: Vec::new(),
            forced_termination: false,
        }))
    }

    async fn close_context(
        &self,
        request: Request<CloseContextRequest>,
    ) -> Result<Response<CloseContextResponse>, Status> {
        let closed = self
            .contexts
            .lock()
            .unwrap()
            .remove(&request.into_inner().context_handle)
            .is_some();
        Ok(Response::new(CloseContextResponse { closed }))
    }

    async fn shutdown(
        &self,
        _request: Request<ShutdownRequest>,
    ) -> Result<Response<ShutdownResponse>, Status> {
        Ok(Response::new(ShutdownResponse { accepted: true }))
    }
}

pub(crate) fn serve(socket: &Path) -> Result<(), String> {
    if socket.exists() {
        return Err("Pyrefly sidecar socket already exists".to_owned());
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("build Pyrefly sidecar runtime: {error}"))?;
    let result = runtime.block_on(async move {
        let listener = tokio::net::UnixListener::bind(socket)
            .map_err(|error| format!("bind Pyrefly sidecar socket: {error}"))?;
        fs::set_permissions(socket, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("secure Pyrefly sidecar socket: {error}"))?;
        let incoming = UnixListenerStream::new(listener);
        Server::builder()
            .add_service(
                PyreflySidecarServer::new(Service::default())
                    .max_decoding_message_size(MAX_FRAME_BYTES)
                    .max_encoding_message_size(MAX_ARROW_CHUNK_BYTES + MAX_FRAME_BYTES),
            )
            .serve_with_incoming(incoming)
            .await
            .map_err(|error| format!("serve Pyrefly sidecar: {error}"))
    });
    let _ = fs::remove_file(socket);
    result
}
