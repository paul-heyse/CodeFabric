//! Application-owned Pyrefly gRPC service over the released private protocol.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    CloseContextResponse, Hello, HelloAck, ModuleBegin, ModuleEnd, OpenContextRequest,
    OpenContextResponse, RelationIpcFrameEvent, RunAccepted, RunTerminal, ShutdownRequest,
    ShutdownResponse,
};
use crate::relation_ipc_contract::{RelationWireIdentity, relation_wire_identity};
use crate::relation_ipc_proto::{
    RelationCoverage, decode_flow_control_ack, encode_relation_frames,
};

const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
const MAX_ARROW_CHUNK_BYTES: usize = 64 * 1024 * 1024;
const MAX_OUTSTANDING_CHUNKS: u32 = 4;
const MAX_UNACKNOWLEDGED_BYTES: u64 = 16 * 1024 * 1024;
const MAX_CONTEXTS: usize = 4;
const MAX_MEMORY_MIB: u64 = 4096;
const MAX_MODULES_PER_RUN: usize = 64;
const MAX_SOURCE_BYTES_PER_MODULE: u64 = 8 * 1024 * 1024;
const MAX_SOURCE_BYTES_PER_RUN: u64 = 64 * 1024 * 1024;
const MAX_TOTAL_RELATION_BYTES: usize = 256 * 1024 * 1024;
const REQUIRED_FEATURE_BITS: u64 = (1_u64 << 17) | (1_u64 << 32);
const OPTIONAL_FEATURE_BITS: u64 = 1_u64 << 33;
const RESOURCE_PROFILE_ID: &str = "sidecar-semantic-standard";
const TRUST_PROFILE: &str = "UNTRUSTED_SANDBOXED";
const PYREFLY_SOURCE_DIGEST: &str =
    "b3:1b9e72144644d1b3df0bdca564496566238543dfb7f576980a8408714327fc3e";
const SANDBOX_PROFILE_DIGEST: &str =
    "b3:8a663d1d6ddbcf830a09e28c7ee6bcd65b433fd9b69b597dbe99f02c78ce8e15";

struct OpenContext {
    workspace_id: String,
    analysis_context_id: String,
    manifest_digest: String,
    lease: Mutex<SourceSnapshotLease>,
    latest_generation: AtomicU64,
    semantic: Mutex<crate::pyrefly_link::SemanticContext>,
}

struct CreditState {
    available_chunks: u32,
    available_bytes: u64,
    outstanding: BTreeMap<(Vec<u8>, u64), OutstandingPayload>,
    next_ack_sequence: BTreeMap<Vec<u8>, u64>,
    rejected: Option<String>,
}

#[derive(Clone, Copy)]
struct OutstandingPayload {
    identity: RelationWireIdentity,
    bytes: u64,
}

struct ActiveRun {
    context_handle: String,
    source_generation: u64,
    cancelled: AtomicBool,
    superseded: AtomicBool,
    terminal: Mutex<Option<ProviderRunState>>,
    credits: Mutex<CreditState>,
    credit_notify: tokio::sync::Notify,
}

impl ActiveRun {
    fn request_cancel(&self, superseded: bool) {
        self.cancelled.store(true, Ordering::Release);
        if superseded {
            self.superseded.store(true, Ordering::Release);
        }
        self.credit_notify.notify_waiters();
    }

    fn terminal_state(&self) -> Option<ProviderRunState> {
        *self
            .terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[derive(Clone)]
struct Service {
    contexts: Arc<Mutex<BTreeMap<String, Arc<OpenContext>>>>,
    runs: Arc<Mutex<BTreeMap<String, Arc<ActiveRun>>>>,
    state_root: Arc<PathBuf>,
}

impl Service {
    fn new(state_root: &Path) -> Result<Self, String> {
        if !state_root.is_absolute() {
            return Err("Pyrefly sidecar state root must be absolute".to_owned());
        }
        fs::create_dir_all(state_root)
            .map_err(|error| format!("create Pyrefly sidecar state root: {error}"))?;
        Ok(Self {
            contexts: Arc::new(Mutex::new(BTreeMap::new())),
            runs: Arc::new(Mutex::new(BTreeMap::new())),
            state_root: Arc::new(state_root.to_owned()),
        })
    }
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

fn validate_hello(hello: &Hello) -> Result<(), Status> {
    let expected_schemas = crate::pyrefly_link::schema_digests();
    if hello.protocol_major != 1
        || hello.protocol_minor != 0
        || hello.required_feature_bits != REQUIRED_FEATURE_BITS
        || hello.optional_feature_bits & !OPTIONAL_FEATURE_BITS != 0
        || hello.observation_schema_digests != expected_schemas
        || hello.maximum_frame_bytes != u64::try_from(MAX_FRAME_BYTES).unwrap()
        || hello.maximum_arrow_chunk_bytes != u64::try_from(MAX_ARROW_CHUNK_BYTES).unwrap()
        || hello.sandbox_profile_digest != SANDBOX_PROFILE_DIGEST
    {
        return Err(Status::failed_precondition(
            "Pyrefly handshake protocol, schema, feature, sandbox, or limit identity differs",
        ));
    }
    Ok(())
}

fn register_relation_stream(
    credits: &mut CreditState,
    identity: RelationWireIdentity,
) -> Result<(), &'static str> {
    if credits
        .next_ack_sequence
        .insert(identity.stream_id.to_vec(), 0)
        .is_some()
    {
        return Err("relation stream identity is duplicated");
    }
    Ok(())
}

fn release_relation_credit(
    credits: &mut CreditState,
    frame: &crate::relation_ipc_proto_types::RelationIpcFrame,
) -> Result<(), &'static str> {
    let acknowledgement = decode_flow_control_ack(frame)?;
    let stream_key = acknowledgement.header.identity.stream_id.to_vec();
    let expected_ack_sequence = credits
        .next_ack_sequence
        .get_mut(&stream_key)
        .ok_or("credit acknowledgement stream identity is unknown")?;
    if acknowledgement.header.sequence != *expected_ack_sequence {
        return Err("credit acknowledgement sequence is duplicate or out of order");
    }
    *expected_ack_sequence = expected_ack_sequence
        .checked_add(1)
        .ok_or("credit acknowledgement sequence space is exhausted")?;
    if acknowledgement.cancelled {
        return Err("relation stream was cancelled by the daemon");
    }
    let payload_sequence = acknowledgement
        .acknowledged_sequence
        .ok_or("credit acknowledgement payload sequence is absent")?;
    let outstanding_key = (stream_key, payload_sequence);
    let outstanding = credits
        .outstanding
        .get(&outstanding_key)
        .copied()
        .ok_or("credit acknowledgement payload sequence is unknown")?;
    if outstanding.identity != acknowledgement.header.identity
        || acknowledgement.released_bytes != outstanding.bytes
    {
        return Err("credit acknowledgement identity or byte release differs from the payload");
    }

    credits.outstanding.remove(&outstanding_key);
    let outstanding_bytes = credits
        .outstanding
        .values()
        .try_fold(0_u64, |total, payload| total.checked_add(payload.bytes))
        .ok_or("outstanding credit accounting overflowed")?;
    let maximum_available_chunks = MAX_OUTSTANDING_CHUNKS
        .checked_sub(u32::try_from(credits.outstanding.len()).unwrap_or(u32::MAX))
        .ok_or("outstanding chunk accounting exceeds its bound")?;
    let available_chunks = credits
        .available_chunks
        .checked_add(1)
        .ok_or("available chunk credit accounting overflowed")?;
    if available_chunks > maximum_available_chunks {
        return Err("chunk acknowledgement exceeds the bounded credit window");
    }
    let maximum_available_bytes = MAX_UNACKNOWLEDGED_BYTES
        .checked_sub(outstanding_bytes)
        .ok_or("outstanding byte accounting exceeds its bound")?;
    let available_bytes = credits
        .available_bytes
        .checked_add(acknowledgement.released_bytes)
        .ok_or("available byte credit accounting overflowed")?;
    if available_bytes > maximum_available_bytes {
        return Err("byte acknowledgement exceeds the bounded credit window");
    }
    credits.available_chunks = available_chunks;
    credits.available_bytes = available_bytes;
    Ok(())
}

async fn receive_run_control(
    mut commands: tonic::Streaming<AnalyzeCommand>,
    run: Arc<ActiveRun>,
    provider_run_id: String,
) {
    loop {
        let command = match commands.message().await {
            Ok(Some(command)) => command.command,
            Ok(None) => {
                run.credits
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .rejected = Some("control stream closed before provider terminal".to_owned());
                run.request_cancel(false);
                return;
            }
            Err(error) => {
                run.credits
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .rejected = Some(format!("control stream failed: {error}"));
                run.request_cancel(false);
                return;
            }
        };
        let mut credits = run
            .credits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match command {
            Some(Command::RelationIpcAck(frame)) => {
                if let Err(error) = release_relation_credit(&mut credits, &frame) {
                    credits.rejected = Some(error.to_owned());
                    drop(credits);
                    run.request_cancel(false);
                    return;
                }
            }
            Some(Command::ChunkAccepted(_) | Command::ChunkRejected(_)) => {
                credits.rejected =
                    Some("legacy whole-relation chunk control is no longer admitted".into());
                drop(credits);
                run.request_cancel(false);
                return;
            }
            Some(Command::Cancel(cancel)) => {
                if cancel.provider_run_id != provider_run_id || cancel.reason.is_empty() {
                    credits.rejected = Some("stream cancellation identity differs".into());
                    drop(credits);
                    run.request_cancel(false);
                    return;
                }
                drop(credits);
                run.request_cancel(false);
                return;
            }
            Some(Command::Start(_)) | None => {
                credits.rejected = Some("analysis stream contains an invalid command".into());
                drop(credits);
                run.request_cancel(false);
                return;
            }
        }
        drop(credits);
        run.credit_notify.notify_waiters();
    }
}

async fn reserve_relation_credit(
    run: &ActiveRun,
    identity: RelationWireIdentity,
    sequence: u64,
    bytes: u64,
) -> Result<(), Status> {
    if bytes > MAX_UNACKNOWLEDGED_BYTES {
        return Err(Status::resource_exhausted(
            "one Pyrefly chunk exceeds the unacknowledged byte limit",
        ));
    }
    loop {
        let notified = run.credit_notify.notified();
        {
            let mut credits = run
                .credits
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(error) = &credits.rejected {
                return Err(Status::failed_precondition(error.clone()));
            }
            if run.cancelled.load(Ordering::Acquire) {
                return Err(Status::cancelled("Pyrefly run was cancelled"));
            }
            if credits.available_chunks > 0 && credits.available_bytes >= bytes {
                credits.available_chunks -= 1;
                credits.available_bytes -= bytes;
                if credits
                    .outstanding
                    .insert(
                        (identity.stream_id.to_vec(), sequence),
                        OutstandingPayload { identity, bytes },
                    )
                    .is_some()
                {
                    return Err(Status::failed_precondition(
                        "relation payload sequence is duplicated",
                    ));
                }
                return Ok(());
            }
        }
        notified.await;
    }
}

async fn await_all_chunk_acknowledgements(run: &ActiveRun) -> Result<(), Status> {
    loop {
        let notified = run.credit_notify.notified();
        {
            let credits = run
                .credits
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(error) = &credits.rejected {
                return Err(Status::failed_precondition(error.clone()));
            }
            if run.cancelled.load(Ordering::Acquire) {
                return Err(Status::cancelled("Pyrefly run was cancelled"));
            }
            if credits.outstanding.is_empty() {
                return Ok(());
            }
        }
        notified.await;
    }
}

fn interrupted_state(
    run: &ActiveRun,
    context: &OpenContext,
    source_generation: u64,
) -> Option<ProviderRunState> {
    if run.superseded.load(Ordering::Acquire)
        || context.latest_generation.load(Ordering::Acquire) != source_generation
    {
        Some(ProviderRunState::Superseded)
    } else if run.cancelled.load(Ordering::Acquire) {
        Some(ProviderRunState::Cancelled)
    } else {
        None
    }
}

fn capability_outcomes(
    start: &crate::protocol::generated::codefabric::pyrefly::v1::AnalyzeModulesRequest,
    state: ProviderRunState,
) -> Vec<CapabilityOutcome> {
    start
        .requested_capability_codes
        .iter()
        .map(|capability_code| CapabilityOutcome {
            capability_code: *capability_code,
            owner_capability_state_code: if state == ProviderRunState::Succeeded {
                40
            } else {
                30
            },
            completeness_state_code: if state == ProviderRunState::Succeeded {
                20
            } else {
                40
            },
            reason_code: match state {
                ProviderRunState::Succeeded => "PYREFLY_QUERY_SLICE_PARTIAL",
                ProviderRunState::Superseded => "PYREFLY_SUPERSEDED",
                ProviderRunState::Cancelled => "PYREFLY_CANCELLED",
                _ => "PYREFLY_FAILED",
            }
            .to_owned(),
        })
        .collect()
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
        || blob.byte_length > MAX_SOURCE_BYTES_PER_MODULE
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
        validate_hello(&hello)?;
        Ok(Response::new(HelloAck {
            protocol_major: 1,
            protocol_minor: 0,
            negotiated_feature_bits: REQUIRED_FEATURE_BITS
                | (hello.optional_feature_bits & OPTIONAL_FEATURE_BITS),
            sidecar_build: "codefabric-pyrefly-sidecar 0.1.0".to_owned(),
            pyrefly_source_digest: PYREFLY_SOURCE_DIGEST.to_owned(),
            supported_python_versions: hello.supported_python_versions,
            observation_schema_digests: crate::pyrefly_link::schema_digests(),
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
            || !valid_digest(&lease.source_manifest_digest)
            || lease.expires_at_unix_ms <= now_millis()
            || request.resource_profile_id != RESOURCE_PROFILE_ID
            || request.maximum_contexts != u32::try_from(MAX_CONTEXTS).unwrap()
            || request.maximum_memory_mib != MAX_MEMORY_MIB
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
            ]
            .concat())[3..35]
        );
        let mut contexts = self
            .contexts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(existing) = contexts.get(&handle) {
            let current = existing.latest_generation.load(Ordering::Acquire);
            if lease.source_generation < current {
                return Err(Status::failed_precondition(
                    "Pyrefly context reopen attempted an older generation",
                ));
            }
            *existing
                .lease
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = lease.clone();
            existing
                .latest_generation
                .store(lease.source_generation, Ordering::Release);
        } else {
            if contexts.len() >= MAX_CONTEXTS {
                return Err(Status::resource_exhausted(
                    "Pyrefly negotiated context capacity is exhausted",
                ));
            }
            let semantic_root = self.state_root.join("contexts");
            let semantic = crate::pyrefly_link::SemanticContext::new(&semantic_root, &handle)
                .map_err(Status::internal)?;
            contexts.insert(
                handle.clone(),
                Arc::new(OpenContext {
                    workspace_id: request.workspace_id,
                    analysis_context_id: request.analysis_context_id,
                    manifest_digest: request.context_manifest_digest.clone(),
                    latest_generation: AtomicU64::new(lease.source_generation),
                    lease: Mutex::new(lease),
                    semantic: Mutex::new(semantic),
                }),
            );
        }
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
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&start.context_handle)
            .cloned()
            .ok_or_else(|| Status::not_found("Pyrefly context handle is unknown"))?;
        let lease = context
            .lease
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let declared_source_bytes = start.modules.iter().try_fold(0_u64, |total, module| {
            total.checked_add(
                module
                    .source_blob
                    .as_ref()
                    .map_or(u64::MAX, |blob| blob.byte_length),
            )
        });
        if start.workspace_id != context.workspace_id
            || start.analysis_context_id != context.analysis_context_id
            || start.context_manifest_digest != context.manifest_digest
            || start.source_snapshot_lease_id != lease.lease_id
            || start.source_generation != lease.source_generation
            || start.source_generation != context.latest_generation.load(Ordering::Acquire)
            || start.deadline_unix_ms <= now_millis()
            || start.initial_chunk_credits == 0
            || start.initial_chunk_credits > MAX_OUTSTANDING_CHUNKS
            || start.initial_credit_bytes == 0
            || start.initial_credit_bytes > MAX_UNACKNOWLEDGED_BYTES
            || start.output_schema_bundle_digest != crate::pyrefly_link::schema_bundle_digest()
            || start.sandbox_profile_digest != SANDBOX_PROFILE_DIGEST
            || start.trust_profile != TRUST_PROFILE
            || start.resource_profile_id != RESOURCE_PROFILE_ID
            || start.modules.is_empty()
            || start.modules.len() > MAX_MODULES_PER_RUN
            || declared_source_bytes.is_none_or(|total| total > MAX_SOURCE_BYTES_PER_RUN)
        {
            return Err(Status::failed_precondition(
                "Pyrefly analysis identity, lease, credits, or deadline differs",
            ));
        }
        let run = Arc::new(ActiveRun {
            context_handle: start.context_handle.clone(),
            source_generation: start.source_generation,
            cancelled: AtomicBool::new(false),
            superseded: AtomicBool::new(false),
            terminal: Mutex::new(None),
            credits: Mutex::new(CreditState {
                available_chunks: start.initial_chunk_credits,
                available_bytes: start.initial_credit_bytes,
                outstanding: BTreeMap::new(),
                next_ack_sequence: BTreeMap::new(),
                rejected: None,
            }),
            credit_notify: tokio::sync::Notify::new(),
        });
        {
            let mut runs = self
                .runs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if runs.contains_key(&start.provider_run_id) {
                return Err(Status::already_exists(
                    "Pyrefly provider run already exists",
                ));
            }
            for existing in runs.values() {
                if existing.context_handle == start.context_handle
                    && existing.source_generation < start.source_generation
                    && existing.terminal_state().is_none()
                {
                    existing.request_cancel(true);
                }
            }
            runs.insert(start.provider_run_id.clone(), Arc::clone(&run));
        }
        tokio::spawn(receive_run_control(
            commands,
            Arc::clone(&run),
            start.provider_run_id.clone(),
        ));
        let (sender, receiver) = tokio::sync::mpsc::channel(8);
        tokio::spawn(async move {
            let source_manifest = lease.source_manifest_digest.clone();
            let _ = sender
                .send(Ok(AnalyzeEvent {
                    event: Some(Event::RunAccepted(RunAccepted {
                        header: Some(header(&start, &source_manifest, 0)),
                        granted_chunk_credits: start.initial_chunk_credits,
                        granted_credit_bytes: start.initial_credit_bytes,
                    })),
                }))
                .await;
            let inputs = start
                .modules
                .iter()
                .map(|module| {
                    validate_source(module, &lease).map(|source_path| {
                        crate::pyrefly_link::ModuleInput {
                            module_id: module.module_id.clone(),
                            module_name: module.module_name.clone(),
                            file_id: module.file_id.clone(),
                            source_path,
                            source_digest: module.source_digest.clone(),
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
            let run_identity = crate::pyrefly_link::AnalysisRunIdentity {
                provider_run_id: start.provider_run_id.clone(),
                analysis_context_id: start.analysis_context_id.clone(),
                semantic_environment_digest: start.context_manifest_digest.clone(),
                source_generation: start.source_generation,
            };
            let analysis_context = Arc::clone(&context);
            let analysis = match tokio::task::spawn_blocking(move || {
                analysis_context
                    .semantic
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .analyze_modules(&run_identity, &inputs)
            })
            .await
            {
                Ok(Ok(analysis)) => analysis,
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
            let total_relation_bytes =
                analysis
                    .modules
                    .iter()
                    .try_fold(0_usize, |module_total, module| {
                        module
                            .relations
                            .iter()
                            .try_fold(module_total, |total, relation| {
                                total.checked_add(relation.arrow_ipc.len())
                            })
                    });
            if total_relation_bytes.is_none_or(|total| total > MAX_TOTAL_RELATION_BYTES) {
                let _ = sender
                    .send(Err(Status::resource_exhausted(
                        "Pyrefly relation streams exceed the per-run byte bound",
                    )))
                    .await;
                return;
            }
            if let Some(state) = interrupted_state(&run, &context, start.source_generation) {
                *run.terminal
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(state);
                let _ = sender
                    .send(Ok(AnalyzeEvent {
                        event: Some(Event::RunTerminal(RunTerminal {
                            header: Some(header(&start, &source_manifest, 1)),
                            ordered_module_digests: Vec::new(),
                            capability_outcomes: capability_outcomes(&start, state),
                            overall_digest: b3(&[]),
                            terminal_state: state as i32,
                            rechecked_module_ids: analysis.proven_rechecked_module_ids,
                            sandbox_profile_digest: SANDBOX_PROFILE_DIGEST.to_owned(),
                            trust_profile: TRUST_PROFILE.to_owned(),
                        })),
                    }))
                    .await;
                return;
            }
            let rechecked_module_ids = analysis.proven_rechecked_module_ids;
            let mut sequence = 0_u64;
            let mut module_digests = Vec::new();
            let mut interrupted = None;
            'modules: for (module, analysis) in start.modules.iter().zip(analysis.modules) {
                if let Some(state) = interrupted_state(&run, &context, start.source_generation) {
                    interrupted = Some(state);
                    break;
                }
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
                let mut family_counts = BTreeMap::new();
                for relation in analysis.relations {
                    let family_code = relation.relation.family_code();
                    if family_counts
                        .insert(family_code, relation.row_count)
                        .is_some()
                    {
                        let _ = sender
                            .send(Err(Status::internal(
                                "Pyrefly emitted one relation family more than once",
                            )))
                            .await;
                        return;
                    }
                    let identity = match relation_wire_identity(
                        relation.relation.relation_id(),
                        &relation.schema_digest,
                        &start.provider_run_id,
                        &module.module_id,
                        &source_manifest,
                        &start.context_manifest_digest,
                    ) {
                        Ok(identity) => identity,
                        Err(error) => {
                            let _ = sender.send(Err(Status::internal(error))).await;
                            return;
                        }
                    };
                    let registration = {
                        let mut credits = run
                            .credits
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        register_relation_stream(&mut credits, identity)
                    };
                    if let Err(error) = registration {
                        let _ = sender.send(Err(Status::internal(error))).await;
                        return;
                    }
                    let frames = match encode_relation_frames(
                        identity,
                        &relation.arrow_ipc,
                        1,
                        relation.row_count,
                        &RelationCoverage::complete(1),
                    ) {
                        Ok(frames) => frames,
                        Err(error) => {
                            let _ = sender.send(Err(Status::internal(error))).await;
                            return;
                        }
                    };
                    for frame in frames {
                        sequence += 1;
                        if let Some(
                            crate::relation_ipc_proto_types::relation_ipc_frame::Frame::Payload(
                                payload,
                            ),
                        ) = frame.frame.as_ref()
                        {
                            let frame_sequence = payload
                                .header
                                .as_ref()
                                .map_or(u64::MAX, |header| header.sequence);
                            if let Err(error) = reserve_relation_credit(
                                &run,
                                identity,
                                frame_sequence,
                                u64::try_from(payload.arrow_ipc_fragment.len()).unwrap_or(u64::MAX),
                            )
                            .await
                            {
                                if let Some(state) =
                                    interrupted_state(&run, &context, start.source_generation)
                                {
                                    interrupted = Some(state);
                                    break 'modules;
                                }
                                let _ = sender.send(Err(error)).await;
                                return;
                            }
                        }
                        if sender
                            .send(Ok(AnalyzeEvent {
                                event: Some(Event::RelationIpcFrame(RelationIpcFrameEvent {
                                    header: Some(header(&start, &source_manifest, sequence)),
                                    module_id: module.module_id.clone(),
                                    observation_family_code: family_code,
                                    frame: Some(frame),
                                })),
                            }))
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                }
                sequence += 1;
                module_digests.push(analysis.module_digest.clone());
                if sender
                    .send(Ok(AnalyzeEvent {
                        event: Some(Event::ModuleEnd(ModuleEnd {
                            header: Some(header(&start, &source_manifest, sequence)),
                            module_id: module.module_id.clone(),
                            family_counts: family_counts.into_iter().collect(),
                            module_digest: analysis.module_digest,
                        })),
                    }))
                    .await
                    .is_err()
                {
                    return;
                }
            }
            if interrupted.is_none()
                && let Err(error) = await_all_chunk_acknowledgements(&run).await
            {
                if let Some(state) = interrupted_state(&run, &context, start.source_generation) {
                    interrupted = Some(state);
                } else {
                    let _ = sender.send(Err(error)).await;
                    return;
                }
            }
            if interrupted.is_none() {
                interrupted = interrupted_state(&run, &context, start.source_generation);
            }
            if let Some(state) = interrupted {
                *run.terminal
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(state);
                sequence += 1;
                let overall_digest = b3(module_digests
                    .iter()
                    .flat_map(std::string::String::as_bytes)
                    .copied()
                    .collect::<Vec<_>>()
                    .as_slice());
                let _ = sender
                    .send(Ok(AnalyzeEvent {
                        event: Some(Event::RunTerminal(RunTerminal {
                            header: Some(header(&start, &source_manifest, sequence)),
                            ordered_module_digests: module_digests,
                            capability_outcomes: capability_outcomes(&start, state),
                            overall_digest,
                            terminal_state: state as i32,
                            rechecked_module_ids,
                            sandbox_profile_digest: SANDBOX_PROFILE_DIGEST.to_owned(),
                            trust_profile: TRUST_PROFILE.to_owned(),
                        })),
                    }))
                    .await;
                return;
            }
            sequence += 1;
            let overall_digest = b3(module_digests
                .iter()
                .flat_map(std::string::String::as_bytes)
                .copied()
                .collect::<Vec<_>>()
                .as_slice());
            let outcomes = capability_outcomes(&start, ProviderRunState::Succeeded);
            *run.terminal
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some(ProviderRunState::Succeeded);
            let _ = sender
                .send(Ok(AnalyzeEvent {
                    event: Some(Event::RunTerminal(RunTerminal {
                        header: Some(header(&start, &source_manifest, sequence)),
                        ordered_module_digests: module_digests,
                        capability_outcomes: outcomes,
                        overall_digest,
                        terminal_state: ProviderRunState::Succeeded as i32,
                        rechecked_module_ids,
                        sandbox_profile_digest: SANDBOX_PROFILE_DIGEST.to_owned(),
                        trust_profile: TRUST_PROFILE.to_owned(),
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
        let request = request.into_inner();
        if request.provider_run_id.is_empty() || request.reason.is_empty() {
            return Err(Status::invalid_argument(
                "Pyrefly cancellation identity and reason are required",
            ));
        }
        let run = self
            .runs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&request.provider_run_id)
            .cloned();
        let (state, terminal_state, cleaning_up_components) = match run {
            None => (CancelAcknowledgementState::NotFound, None, Vec::new()),
            Some(run) => {
                if let Some(terminal) = run.terminal_state() {
                    (
                        CancelAcknowledgementState::AlreadyTerminal,
                        Some(terminal as i32),
                        Vec::new(),
                    )
                } else {
                    run.request_cancel(false);
                    (
                        CancelAcknowledgementState::CancellationRequested,
                        None,
                        vec!["query-transaction".to_owned(), "staged-output".to_owned()],
                    )
                }
            }
        };
        Ok(Response::new(CancelAcknowledgement {
            provider_run_id: request.provider_run_id,
            state: state as i32,
            acknowledged_at_unix_ms: now_millis(),
            terminal_state,
            cleaning_up_components,
            forced_termination: false,
        }))
    }

    async fn close_context(
        &self,
        request: Request<CloseContextRequest>,
    ) -> Result<Response<CloseContextResponse>, Status> {
        let context_handle = request.into_inner().context_handle;
        for run in self
            .runs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .filter(|run| run.context_handle == context_handle)
        {
            if run.terminal_state().is_none() {
                run.request_cancel(false);
            }
        }
        let closed = self
            .contexts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&context_handle)
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
        let state_root = socket
            .parent()
            .ok_or_else(|| "Pyrefly socket has no parent state root".to_owned())?
            .join("pyrefly-state");
        let service = Service::new(&state_root)?;
        let listener = tokio::net::UnixListener::bind(socket)
            .map_err(|error| format!("bind Pyrefly sidecar socket: {error}"))?;
        fs::set_permissions(socket, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("secure Pyrefly sidecar socket: {error}"))?;
        let incoming = UnixListenerStream::new(listener);
        Server::builder()
            .add_service(
                PyreflySidecarServer::new(service)
                    .max_decoding_message_size(MAX_FRAME_BYTES)
                    .max_encoding_message_size(MAX_FRAME_BYTES),
            )
            .serve_with_incoming(incoming)
            .await
            .map_err(|error| format!("serve Pyrefly sidecar: {error}"))
    });
    let _ = fs::remove_file(socket);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello() -> Hello {
        Hello {
            protocol_major: 1,
            protocol_minor: 0,
            required_feature_bits: REQUIRED_FEATURE_BITS,
            optional_feature_bits: OPTIONAL_FEATURE_BITS,
            daemon_build: "protocol-test".to_owned(),
            supported_python_versions: vec!["3.14".to_owned()],
            observation_schema_digests: crate::pyrefly_link::schema_digests(),
            maximum_frame_bytes: u64::try_from(MAX_FRAME_BYTES).unwrap(),
            maximum_arrow_chunk_bytes: u64::try_from(MAX_ARROW_CHUNK_BYTES).unwrap(),
            sandbox_profile_digest: SANDBOX_PROFILE_DIGEST.to_owned(),
        }
    }

    fn relation_identity() -> RelationWireIdentity {
        RelationWireIdentity {
            relation_id: [1; 16],
            stream_id: [2; 16],
            schema_fingerprint: [3; 32],
            source_pin: [4; 32],
            context_pin: [5; 32],
        }
    }

    #[test]
    fn relation_acknowledgements_return_only_explicitly_accepted_credit() {
        let identity = relation_identity();
        let mut credits = CreditState {
            available_chunks: MAX_OUTSTANDING_CHUNKS - 1,
            available_bytes: MAX_UNACKNOWLEDGED_BYTES - 4,
            outstanding: [(
                (identity.stream_id.to_vec(), 7),
                OutstandingPayload { identity, bytes: 4 },
            )]
            .into_iter()
            .collect(),
            next_ack_sequence: [(identity.stream_id.to_vec(), 0)].into_iter().collect(),
            rejected: None,
        };
        let accepted =
            crate::relation_ipc_proto::flow_control_ack_frame(identity, 0, Some(7), 4, false)
                .unwrap();
        release_relation_credit(&mut credits, &accepted).unwrap();
        assert!(credits.outstanding.is_empty());
        assert_eq!(credits.available_chunks, MAX_OUTSTANDING_CHUNKS);
        assert_eq!(credits.available_bytes, MAX_UNACKNOWLEDGED_BYTES);

        let mut excessive = CreditState {
            available_chunks: MAX_OUTSTANDING_CHUNKS - 1,
            available_bytes: MAX_UNACKNOWLEDGED_BYTES - 4,
            outstanding: [(
                (identity.stream_id.to_vec(), 8),
                OutstandingPayload { identity, bytes: 4 },
            )]
            .into_iter()
            .collect(),
            next_ack_sequence: [(identity.stream_id.to_vec(), 0)].into_iter().collect(),
            rejected: None,
        };
        let excessive_ack =
            crate::relation_ipc_proto::flow_control_ack_frame(identity, 0, Some(8), 5, false)
                .unwrap();
        assert_eq!(
            release_relation_credit(&mut excessive, &excessive_ack),
            Err("credit acknowledgement identity or byte release differs from the payload")
        );
        assert_eq!(
            excessive
                .outstanding
                .get(&(identity.stream_id.to_vec(), 8))
                .map(|payload| payload.bytes),
            Some(4)
        );
        assert_eq!(excessive.available_bytes, MAX_UNACKNOWLEDGED_BYTES - 4);
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Handshake, context fencing, credit stalls, idempotent cancel, and shutdown form one protocol session.
    async fn pyrefly_protocol_conformance() {
        let state_root = std::env::temp_dir().join(format!(
            "codefabric-pyrefly-protocol-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&state_root);
        let service = Service::new(&state_root).unwrap();

        let mut mismatch = hello();
        mismatch.protocol_major = 2;
        assert_eq!(
            service
                .handshake(Request::new(mismatch))
                .await
                .unwrap_err()
                .code(),
            tonic::Code::FailedPrecondition
        );
        let acknowledgement = service
            .handshake(Request::new(hello()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(
            acknowledgement.negotiated_feature_bits,
            REQUIRED_FEATURE_BITS | OPTIONAL_FEATURE_BITS
        );
        assert_eq!(
            acknowledgement.observation_schema_digests,
            crate::pyrefly_link::schema_digests()
        );

        let manifest = b"{\"python\":\"3.14\"}".to_vec();
        let manifest_digest = b3(&manifest);
        let open = |generation| OpenContextRequest {
            workspace_id: "workspace-protocol".to_owned(),
            analysis_context_id: "context-protocol".to_owned(),
            immutable_context_manifest: manifest.clone(),
            context_manifest_digest: manifest_digest.clone(),
            source_snapshot_lease: Some(SourceSnapshotLease {
                lease_id: format!("lease-{generation}"),
                workspace_id: "workspace-protocol".to_owned(),
                source_generation: generation,
                source_manifest_digest: b3(format!("generation-{generation}").as_bytes()),
                expires_at_unix_ms: now_millis() + 60_000,
                blobs: Vec::new(),
            }),
            resource_profile_id: RESOURCE_PROFILE_ID.to_owned(),
            maximum_contexts: u32::try_from(MAX_CONTEXTS).unwrap(),
            maximum_memory_mib: MAX_MEMORY_MIB,
        };
        let opened = service
            .open_context(Request::new(open(2)))
            .await
            .unwrap()
            .into_inner();
        assert!(!opened.context_handle.is_empty());
        assert_eq!(
            service
                .open_context(Request::new(open(1)))
                .await
                .unwrap_err()
                .code(),
            tonic::Code::FailedPrecondition
        );

        let run = Arc::new(ActiveRun {
            context_handle: opened.context_handle,
            source_generation: 2,
            cancelled: AtomicBool::new(false),
            superseded: AtomicBool::new(false),
            terminal: Mutex::new(None),
            credits: Mutex::new(CreditState {
                available_chunks: 1,
                available_bytes: 4,
                outstanding: BTreeMap::new(),
                next_ack_sequence: [(relation_identity().stream_id.to_vec(), 0)]
                    .into_iter()
                    .collect(),
                rejected: None,
            }),
            credit_notify: tokio::sync::Notify::new(),
        });
        let identity = relation_identity();
        reserve_relation_credit(&run, identity, 1, 4).await.unwrap();
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(20),
                reserve_relation_credit(&run, identity, 2, 1),
            )
            .await
            .is_err(),
            "a producer exceeding granted credits must stall"
        );
        {
            let mut credits = run
                .credits
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            assert_eq!(
                credits
                    .outstanding
                    .remove(&(identity.stream_id.to_vec(), 1))
                    .map(|payload| payload.bytes),
                Some(4)
            );
            credits.available_chunks = 1;
            credits.available_bytes = 4;
        }
        run.credit_notify.notify_waiters();
        reserve_relation_credit(&run, identity, 2, 1).await.unwrap();

        service
            .runs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert("run-protocol".to_owned(), Arc::clone(&run));
        let cancel = || {
            Request::new(CancelRunRequest {
                provider_run_id: "run-protocol".to_owned(),
                reason: "test cancellation".to_owned(),
            })
        };
        for _ in 0..2 {
            let acknowledgement = service.cancel_run(cancel()).await.unwrap().into_inner();
            assert_eq!(
                acknowledgement.state,
                CancelAcknowledgementState::CancellationRequested as i32
            );
        }
        assert_eq!(
            reserve_relation_credit(&run, identity, 3, 1)
                .await
                .unwrap_err()
                .code(),
            tonic::Code::Cancelled,
            "a cancelled producer must stop before emitting another relation payload"
        );
        *run.terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(ProviderRunState::Cancelled);
        assert_eq!(
            service
                .cancel_run(cancel())
                .await
                .unwrap()
                .into_inner()
                .state,
            CancelAcknowledgementState::AlreadyTerminal as i32
        );
        assert!(
            service
                .shutdown(Request::new(ShutdownRequest {
                    reason: "protocol complete".to_owned(),
                }))
                .await
                .unwrap()
                .into_inner()
                .accepted
        );
        drop(service);
        let _ = fs::remove_dir_all(state_root);
    }
}
