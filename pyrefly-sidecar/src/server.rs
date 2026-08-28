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
    CloseContextResponse, Hello, HelloAck, ModuleBegin, ModuleEnd, ObservationBatchChunk,
    OpenContextRequest, OpenContextResponse, RunAccepted, RunTerminal, ShutdownRequest,
    ShutdownResponse,
};

const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
const MAX_ARROW_CHUNK_BYTES: usize = 64 * 1024 * 1024;
const MAX_OUTSTANDING_CHUNKS: u32 = 4;
const MAX_UNACKNOWLEDGED_BYTES: u64 = 16 * 1024 * 1024;
const MAX_CONTEXTS: usize = 4;
const MAX_MEMORY_MIB: u64 = 4096;
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
    outstanding: BTreeMap<u64, u64>,
    rejected: Option<String>,
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
    let expected_schema = crate::pyrefly_link::schema_digest();
    if hello.protocol_major != 1
        || hello.protocol_minor != 0
        || hello.required_feature_bits != REQUIRED_FEATURE_BITS
        || hello.optional_feature_bits & !OPTIONAL_FEATURE_BITS != 0
        || hello.observation_schema_digests != [expected_schema]
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

async fn receive_run_control(
    mut commands: tonic::Streaming<AnalyzeCommand>,
    run: Arc<ActiveRun>,
    provider_run_id: String,
) {
    loop {
        let command = match commands.message().await {
            Ok(Some(command)) => command.command,
            Ok(None) => return,
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
            Some(Command::ChunkAccepted(accepted)) => {
                let Some(bytes) = credits.outstanding.remove(&accepted.sequence) else {
                    credits.rejected = Some("credit acknowledgement sequence is unknown".into());
                    drop(credits);
                    run.request_cancel(false);
                    return;
                };
                let outstanding_bytes = credits.outstanding.values().sum::<u64>();
                let maximum_available_chunks = MAX_OUTSTANDING_CHUNKS
                    .saturating_sub(u32::try_from(credits.outstanding.len()).unwrap_or(u32::MAX));
                credits.available_chunks = credits
                    .available_chunks
                    .saturating_add(accepted.next_credit_chunks)
                    .min(maximum_available_chunks);
                let maximum_available_bytes =
                    MAX_UNACKNOWLEDGED_BYTES.saturating_sub(outstanding_bytes);
                credits.available_bytes = credits
                    .available_bytes
                    .saturating_add(accepted.next_credit_bytes.max(bytes))
                    .min(maximum_available_bytes);
            }
            Some(Command::ChunkRejected(rejected)) => {
                if !credits.outstanding.contains_key(&rejected.sequence)
                    || rejected.error_code.is_empty()
                {
                    credits.rejected = Some("chunk rejection identity differs".into());
                } else {
                    credits.rejected = Some(rejected.error_code);
                }
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

async fn reserve_chunk_credit(run: &ActiveRun, sequence: u64, bytes: u64) -> Result<(), Status> {
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
                credits.outstanding.insert(sequence, bytes);
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
                10
            } else {
                30
            },
            completeness_state_code: if state == ProviderRunState::Succeeded {
                10
            } else {
                40
            },
            reason_code: match state {
                ProviderRunState::Succeeded => "PYREFLY_SUCCEEDED",
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
            || !valid_digest(&start.output_schema_bundle_digest)
            || start.sandbox_profile_digest != SANDBOX_PROFILE_DIGEST
            || start.trust_profile != TRUST_PROFILE
            || start.resource_profile_id != RESOURCE_PROFILE_ID
            || start.modules.is_empty()
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
            let analysis_context = Arc::clone(&context);
            let analysis = match tokio::task::spawn_blocking(move || {
                analysis_context
                    .semantic
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .analyze_modules(&inputs)
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
                            rechecked_module_ids: analysis.rechecked_module_ids,
                            sandbox_profile_digest: SANDBOX_PROFILE_DIGEST.to_owned(),
                            trust_profile: TRUST_PROFILE.to_owned(),
                        })),
                    }))
                    .await;
                return;
            }
            let rechecked_module_ids = analysis.rechecked_module_ids;
            let mut sequence = 0_u64;
            let mut module_digests = Vec::new();
            let mut interrupted = None;
            for (module, analysis) in start.modules.iter().zip(analysis.modules) {
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
                sequence += 1;
                let chunk_digest = b3(&analysis.arrow_ipc);
                if let Err(error) = reserve_chunk_credit(
                    &run,
                    sequence,
                    u64::try_from(analysis.arrow_ipc.len()).unwrap_or(u64::MAX),
                )
                .await
                {
                    let _ = sender.send(Err(error)).await;
                    return;
                }
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
                    .max_encoding_message_size(MAX_ARROW_CHUNK_BYTES + MAX_FRAME_BYTES),
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
            observation_schema_digests: vec![crate::pyrefly_link::schema_digest()],
            maximum_frame_bytes: u64::try_from(MAX_FRAME_BYTES).unwrap(),
            maximum_arrow_chunk_bytes: u64::try_from(MAX_ARROW_CHUNK_BYTES).unwrap(),
            sandbox_profile_digest: SANDBOX_PROFILE_DIGEST.to_owned(),
        }
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
            [crate::pyrefly_link::schema_digest()]
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
                rejected: None,
            }),
            credit_notify: tokio::sync::Notify::new(),
        });
        reserve_chunk_credit(&run, 1, 4).await.unwrap();
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(20),
                reserve_chunk_credit(&run, 2, 1),
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
            assert_eq!(credits.outstanding.remove(&1), Some(4));
            credits.available_chunks = 1;
            credits.available_bytes = 4;
        }
        run.credit_notify.notify_waiters();
        reserve_chunk_credit(&run, 2, 1).await.unwrap();

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
