//! Accepted gRPC query handles and immutable canonical result artifacts.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::future::Future;
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use futures::{Stream, stream};
use serde::Serialize;
use thiserror::Error;
use tokio::sync::{Mutex, Notify};
use tonic::service::InterceptorLayer;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use crate::fabric::ServingQuerySession;
use crate::identity::{SemanticFingerprintDomain, semantic_fingerprint};
use crate::integrity::{frame_digest, framed_digest};
use crate::lifecycle::{FreshnessAdmission, FreshnessBarrier, FreshnessState};
use crate::registries::QUERY_FORM_VALUES;
use crate::rpc::generated::codefabric::cpgd::v1::cpg_query_service_server::CpgQueryService;
use crate::rpc::generated::codefabric::cpgd::v1::cpg_query_service_server::CpgQueryServiceServer;
use crate::rpc::generated::codefabric::cpgd::v1::query_event::Event;
use crate::rpc::generated::codefabric::cpgd::v1::{
    ArtifactReadyEvent, AttachQueryRequest, BundleIdentity, CancelQueryRequest,
    CancelQueryResponse, CancellationState, DeliveryPreference, EffectiveLimitsProfile,
    HandshakeRequest, HandshakeResponse, PayloadCompression, QueryEvent, QueryEventHeader,
    QueryExecutionState, ReadResultRequest, ReadinessSummary, ReleaseResultRequest,
    ReleaseResultResponse, ResultChunk, SnapshotPinnedEvent, StartQueryRequest, StartQueryResponse,
    StatusRequest, StatusResponse, StreamQueryRequest, TerminalEvent, ValidateQueryRequest,
    ValidateQueryResponse, WorkspaceClaim, WorkspaceReadiness,
};
use crate::rpc::{
    AuthorizedUnixStream, MAX_CONTROL_MESSAGE_BYTES, MAX_PAYLOAD_CHUNK_BYTES, SameUserInterceptor,
    negotiate_feature_bits,
};
use crate::security::{
    KeyedAuthenticator, SecurityMacDomain, authenticator_hex, local_token_digest,
};
use crate::semantic_query::QueryForm;
use crate::semantic_query::{
    ExecutedSemanticResponse, FreshnessPolicy, SemanticQueryError, SemanticSnapshotResponse,
    ValidatedSemanticRequest, execute_request, snapshot_response, validate_request,
};

const SUPPORTED_FEATURE_BITS: u64 = 0b1111;
const RESULT_LEASE_SECONDS: i64 = 1_800;

type QueryStream = Pin<Box<dyn Stream<Item = Result<QueryEvent, Status>> + Send>>;
type ArtifactStream = Pin<Box<dyn Stream<Item = Result<ResultChunk, Status>> + Send>>;

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
        })
}

fn opaque_bytes(domain: SemanticFingerprintDomain, value: &str) -> Vec<u8> {
    let mut fingerprint = semantic_fingerprint(domain);
    fingerprint.update(value.as_bytes());
    fingerprint.finalize().to_vec()
}

#[derive(Clone, Debug)]
struct ResultArtifact {
    id: String,
    checksum: String,
    lease_token: String,
    lease_expires_at_unix_ms: i64,
    bytes: Arc<[u8]>,
}

#[derive(Clone, Debug)]
pub struct ResultArtifactStore {
    root: PathBuf,
    lease_secret: [u8; 32],
}

impl ResultArtifactStore {
    /// Create a private immutable result root.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the private root cannot be created or operating-system entropy
    /// cannot be read.
    pub fn new(root: PathBuf) -> Result<Self, std::io::Error> {
        fs::create_dir_all(&root)?;
        let mut lease_secret = [0_u8; 32];
        fs::File::open("/dev/urandom")?.read_exact(&mut lease_secret)?;
        Ok(Self { root, lease_secret })
    }

    fn insert(
        &self,
        bytes: Vec<u8>,
        agent_id: &str,
        workspace_id: &str,
        snapshot_id: &str,
    ) -> Result<ResultArtifact, Status> {
        let checksum = framed_digest(&bytes);
        let mut identity = semantic_fingerprint(SemanticFingerprintDomain::ResultArtifact);
        for field in [workspace_id, agent_id, snapshot_id, checksum.as_str()] {
            identity.update(&(field.len() as u64).to_be_bytes());
            identity.update(field.as_bytes());
        }
        let identity = identity.finalize();
        let id = format!("result:{}", &frame_digest(identity)[3..35]);
        let final_path = self.root.join(format!("{}.json", &checksum[3..]));
        if final_path.exists() {
            let existing = fs::read(&final_path)
                .map_err(|_| Status::internal("result artifact read failed"))?;
            if existing != bytes {
                return Err(Status::data_loss("result artifact identity collision"));
            }
        } else {
            let temporary =
                self.root
                    .join(format!(".{}.{}.tmp", &checksum[3..], std::process::id()));
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)
                .map_err(|_| Status::internal("result staging failed"))?;
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|_| Status::internal("result staging write failed"))?;
            fs::rename(&temporary, &final_path)
                .map_err(|_| Status::internal("result artifact publication failed"))?;
        }
        let mut lease = KeyedAuthenticator::new(&self.lease_secret, SecurityMacDomain::ResultLease);
        lease.update(id.as_bytes());
        lease.update(agent_id.as_bytes());
        lease.update(workspace_id.as_bytes());
        let lease_token = format!("lease:{}", authenticator_hex(lease.finalize()));
        Ok(ResultArtifact {
            id,
            checksum,
            lease_token,
            lease_expires_at_unix_ms: now_millis().saturating_add(RESULT_LEASE_SECONDS * 1_000),
            bytes: bytes.into(),
        })
    }
}

#[async_trait]
pub trait SemanticQueryBackend: Send + Sync + 'static {
    async fn execute(
        &self,
        request: ValidatedSemanticRequest,
    ) -> Result<ExecutedSemanticResponse, SemanticQueryError>;

    async fn public_snapshot(
        &self,
        workspace_id: &str,
    ) -> Result<SemanticSnapshotResponse, SemanticQueryError>;
}

/// Capability-token identity and the exact workspace claims granted to one adapter profile.
#[derive(Clone, Debug)]
pub struct QueryAuthorization {
    token_digest: [u8; 32],
    claims: BTreeMap<String, WorkspaceClaim>,
}

impl QueryAuthorization {
    /// Construct a non-empty local capability and closed claim set.
    ///
    /// # Errors
    ///
    /// Rejects empty token material, duplicate/empty workspace identities, or empty permissions.
    pub fn new(capability_token: &[u8], claims: Vec<WorkspaceClaim>) -> Result<Self, Status> {
        if capability_token.len() < 16 || capability_token.len() > 4_096 {
            return Err(Status::invalid_argument(
                "capability token length is outside the local profile",
            ));
        }
        let mut by_workspace = BTreeMap::new();
        for claim in claims {
            if claim.workspace_id.is_empty()
                || claim.permission_claims.is_empty()
                || by_workspace
                    .insert(claim.workspace_id.clone(), claim)
                    .is_some()
            {
                return Err(Status::invalid_argument(
                    "workspace claims are empty or duplicated",
                ));
            }
        }
        if by_workspace.is_empty() {
            return Err(Status::invalid_argument("workspace claim set is empty"));
        }
        Ok(Self {
            token_digest: capability_digest(capability_token),
            claims: by_workspace,
        })
    }

    fn authorize_handshake(
        &self,
        request: &HandshakeRequest,
    ) -> Result<Vec<WorkspaceClaim>, Status> {
        let proof = request
            .credential_proof
            .as_ref()
            .ok_or_else(|| Status::unauthenticated("capability proof is missing"))?;
        if proof.credential_id.is_empty()
            || request.agent_instance_id.is_empty()
            || !constant_time_equal(
                &self.token_digest,
                &capability_digest(&proof.capability_token),
            )
        {
            return Err(Status::unauthenticated("capability proof differs"));
        }
        if request.desired_workspace_ids.is_empty() {
            return Err(Status::invalid_argument("desired workspace set is empty"));
        }
        request
            .desired_workspace_ids
            .iter()
            .map(|workspace_id| {
                self.claims
                    .get(workspace_id)
                    .cloned()
                    .ok_or_else(|| Status::permission_denied("workspace is not authorized"))
            })
            .collect()
    }

    fn authorize_workspace(&self, workspace_id: &str) -> Result<(), Status> {
        self.claims
            .contains_key(workspace_id)
            .then_some(())
            .ok_or_else(|| Status::permission_denied("workspace is not authorized"))
    }
}

fn capability_digest(token: &[u8]) -> [u8; 32] {
    local_token_digest(SecurityMacDomain::LocalCapabilityToken, token)
}

fn constant_time_equal(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct PublicStatusView {
    ready: bool,
    workspace_id: String,
    agent_instance_id: String,
    snapshot: SemanticSnapshotResponse,
    versions: BTreeMap<String, String>,
    supported_languages: Vec<String>,
    supported_request_forms: Vec<String>,
    capability_statuses: Vec<BTreeMap<String, String>>,
    freshness_state: &'static str,
    service_limits: PublicServiceLimits,
    notices: Vec<String>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_field_names)] // The public JSON contract intentionally uses the registered maximum_* field names.
struct PublicServiceLimits {
    maximum_control_message_bytes: u64,
    maximum_payload_chunk_bytes: u64,
    maximum_concurrent_queries: u32,
}

#[async_trait]
impl SemanticQueryBackend for ServingQuerySession {
    async fn execute(
        &self,
        request: ValidatedSemanticRequest,
    ) -> Result<ExecutedSemanticResponse, SemanticQueryError> {
        execute_request(self, request).await
    }

    async fn public_snapshot(
        &self,
        workspace_id: &str,
    ) -> Result<SemanticSnapshotResponse, SemanticQueryError> {
        let manifest = self.snapshot_manifest();
        if manifest.body.workspace_id != workspace_id {
            return Err(SemanticQueryError::Invalid(
                "status workspace differs from the pinned snapshot".to_owned(),
            ));
        }
        Ok(snapshot_response(&manifest))
    }
}

#[derive(Clone, Debug, Default)]
struct QueryHandleState {
    events: Vec<QueryEvent>,
    terminal_state: Option<QueryExecutionState>,
}

#[derive(Debug)]
struct QueryHandle {
    resume_token: Vec<u8>,
    cancel_token: Vec<u8>,
    agent_instance_id: String,
    workspace_id: String,
    cancelled: AtomicBool,
    state: Mutex<QueryHandleState>,
    changed: Notify,
}

pub struct ProductionQueryService<B> {
    backend: Arc<B>,
    authorization: QueryAuthorization,
    artifacts: Arc<ResultArtifactStore>,
    artifact_records: Arc<Mutex<BTreeMap<String, ResultArtifact>>>,
    handles: Arc<Mutex<BTreeMap<String, Arc<QueryHandle>>>>,
    idempotency: Arc<Mutex<BTreeMap<String, String>>>,
    freshness: FreshnessBarrier,
    freshness_timeout: std::time::Duration,
    query_bundle: BundleIdentity,
}

fn valid_bundle_digest(value: &str) -> bool {
    value.len() == 67
        && value.starts_with("b3:")
        && value[3..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn query_bundle_identity() -> BundleIdentity {
    let authority: serde_json::Value = serde_json::from_str(include_str!(
        "../contracts/bundles/query-language-bundle.json"
    ))
    .expect("generated query-language bundle must be valid JSON");
    let string = |field: &str| {
        authority[field]
            .as_str()
            .unwrap_or_else(|| panic!("query-language bundle is missing {field}"))
            .to_owned()
    };
    let bundle = BundleIdentity {
        bundle_id: string("artifact_id"),
        bundle_version: string("bundle_version"),
        bundle_digest: string("bundle_digest"),
    };
    assert_eq!(
        bundle.bundle_id, "codefabric.bundles.query-language-bundle",
        "query-language bundle identity is unexpected"
    );
    assert!(valid_bundle_digest(&bundle.bundle_digest));
    bundle
}

fn supported_query_forms() -> Vec<String> {
    QUERY_FORM_VALUES
        .iter()
        .filter_map(|entry| QueryForm::try_from(entry.code).ok())
        .filter(|form| form.currently_supported())
        .map(|form| form.registry_slug().to_owned())
        .collect()
}

impl<B> ProductionQueryService<B> {
    pub fn new(
        backend: Arc<B>,
        artifacts: ResultArtifactStore,
        authorization: QueryAuthorization,
    ) -> Self {
        Self {
            backend,
            authorization,
            artifacts: Arc::new(artifacts),
            artifact_records: Arc::new(Mutex::new(BTreeMap::new())),
            handles: Arc::new(Mutex::new(BTreeMap::new())),
            idempotency: Arc::new(Mutex::new(BTreeMap::new())),
            freshness: FreshnessBarrier::default(),
            freshness_timeout: std::time::Duration::from_secs(2),
            query_bundle: query_bundle_identity(),
        }
    }

    /// Attach the sole workspace freshness barrier and deployment-owned wait bound.
    #[must_use]
    pub fn with_freshness_barrier(
        mut self,
        freshness: FreshnessBarrier,
        timeout: std::time::Duration,
    ) -> Self {
        self.freshness = freshness;
        self.freshness_timeout = timeout;
        self
    }
}

/// Bind and serve the authenticated local gRPC boundary until the supplied shutdown completes.
///
/// # Errors
///
/// Returns socket binding/permission failures or a tonic transport failure.
pub async fn serve_query_uds<B, F>(
    socket: &Path,
    allowed_uid: u32,
    service: ProductionQueryService<B>,
    shutdown: F,
) -> Result<(), QueryTransportError>
where
    B: SemanticQueryBackend,
    F: Future<Output = ()> + Send + 'static,
{
    if socket.exists() {
        return Err(QueryTransportError::SocketExists(socket.to_path_buf()));
    }
    let listener =
        tokio::net::UnixListener::bind(socket).map_err(|source| QueryTransportError::Io {
            path: socket.to_path_buf(),
            source,
        })?;
    fs::set_permissions(socket, fs::Permissions::from_mode(0o600)).map_err(|source| {
        QueryTransportError::Io {
            path: socket.to_path_buf(),
            source,
        }
    })?;
    let incoming = stream::unfold(listener, move |listener| async move {
        let accepted = listener
            .accept()
            .await
            .and_then(|(stream, _)| AuthorizedUnixStream::authenticate(stream, allowed_uid));
        Some((accepted, listener))
    });
    let service = CpgQueryServiceServer::new(service)
        .max_decoding_message_size(MAX_CONTROL_MESSAGE_BYTES)
        .max_encoding_message_size(MAX_CONTROL_MESSAGE_BYTES);
    let result = Server::builder()
        .layer(InterceptorLayer::new(SameUserInterceptor::new(allowed_uid)))
        .add_service(service)
        .serve_with_incoming_shutdown(incoming, shutdown)
        .await;
    let _ = fs::remove_file(socket);
    result.map_err(QueryTransportError::Transport)
}

#[derive(Debug, Error)]
pub enum QueryTransportError {
    #[error("query socket already exists at {0}")]
    SocketExists(PathBuf),
    #[error("query transport I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("query transport failed: {0}")]
    Transport(tonic::transport::Error),
}

fn validate_checksum(expected: &str, bytes: &[u8]) -> Result<(), Status> {
    if expected != framed_digest(bytes) {
        return Err(Status::invalid_argument("request checksum differs"));
    }
    Ok(())
}

struct PublicBoundaryError {
    status: Status,
    canonical_record_json: String,
}

#[derive(serde::Serialize)]
struct PublicErrorRecord<'a> {
    code: u16,
    name: &'a str,
    phase: &'a str,
    severity: &'a str,
    retryability: &'a str,
    grpc_status: &'a str,
    mcp_mapping: &'a str,
    detail: &'a str,
}

fn grpc_code(name: &str) -> tonic::Code {
    match name {
        "ABORTED" => tonic::Code::Aborted,
        "ALREADY_EXISTS" => tonic::Code::AlreadyExists,
        "CANCELLED" => tonic::Code::Cancelled,
        "DATA_LOSS" => tonic::Code::DataLoss,
        "DEADLINE_EXCEEDED" => tonic::Code::DeadlineExceeded,
        "FAILED_PRECONDITION" => tonic::Code::FailedPrecondition,
        "INTERNAL" => tonic::Code::Internal,
        "INVALID_ARGUMENT" => tonic::Code::InvalidArgument,
        "NOT_FOUND" => tonic::Code::NotFound,
        "OUT_OF_RANGE" => tonic::Code::OutOfRange,
        "PERMISSION_DENIED" => tonic::Code::PermissionDenied,
        "RESOURCE_EXHAUSTED" => tonic::Code::ResourceExhausted,
        "UNAUTHENTICATED" => tonic::Code::Unauthenticated,
        "UNAVAILABLE" => tonic::Code::Unavailable,
        "UNIMPLEMENTED" => tonic::Code::Unimplemented,
        _ => tonic::Code::Internal,
    }
}

fn public_boundary_error(
    name: &'static str,
    phase: crate::registries::Phase,
    detail: &str,
) -> PublicBoundaryError {
    let entry = crate::registries::public_error(name)
        .expect("RPC boundary error identity must be registry generated");
    let phase =
        crate::registries::registry_state_name(crate::registries::PHASE_VALUES, phase as u16)
            .expect("generated phase has a registry name");
    let canonical_record_json = serde_json::to_string(&PublicErrorRecord {
        code: entry.code,
        name: entry.name,
        phase,
        severity: entry.severity,
        retryability: entry.retryability,
        grpc_status: entry.grpc_status,
        mcp_mapping: entry.mcp_mapping,
        detail,
    })
    .expect("public error record contains only JSON-safe strings");
    PublicBoundaryError {
        status: Status::new(grpc_code(entry.grpc_status), entry.public_message_template),
        canonical_record_json,
    }
}

fn event_header(query_id: &str, sequence: u64, snapshot_id: Option<String>) -> QueryEventHeader {
    QueryEventHeader {
        daemon_query_id: query_id.to_owned(),
        sequence,
        snapshot_id,
        event_at_unix_ms: now_millis(),
        event_checksum: framed_digest(format!("{query_id}:{sequence}").as_bytes()),
    }
}

fn event_checksum(event: &Event) -> Option<&str> {
    match event {
        Event::SnapshotPinned(value) => value.header.as_ref(),
        Event::Progress(value) => value.header.as_ref(),
        Event::ResponseChunk(value) => value.header.as_ref(),
        Event::ArtifactReady(value) => value.header.as_ref(),
        Event::Terminal(value) => value.header.as_ref(),
    }
    .map(|header| header.event_checksum.as_str())
}

fn stream_after(handle: Arc<QueryHandle>, after_sequence: u64) -> QueryStream {
    let after_sequence = usize::try_from(after_sequence).unwrap_or(usize::MAX);
    Box::pin(stream::unfold(
        (handle, after_sequence),
        |(handle, index)| async move {
            loop {
                let notified = handle.changed.notified();
                {
                    let state = handle.state.lock().await;
                    if let Some(event) = state.events.get(index).cloned() {
                        drop(notified);
                        return Some((Ok(event), (Arc::clone(&handle), index + 1)));
                    }
                    if state.terminal_state.is_some() {
                        drop(notified);
                        return None;
                    }
                }
                notified.await;
            }
        },
    ))
}

async fn append_terminal(handle: &QueryHandle, event: QueryEvent, state: QueryExecutionState) {
    let mut current = handle.state.lock().await;
    if current.terminal_state.is_none() {
        current.events.push(event);
        current.terminal_state = Some(state);
        drop(current);
        handle.changed.notify_waiters();
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)] // One spawned query owns this complete immutable execution context and terminal sequence.
async fn execute_accepted_query<B: SemanticQueryBackend>(
    backend: Arc<B>,
    artifacts: Arc<ResultArtifactStore>,
    artifact_records: Arc<Mutex<BTreeMap<String, ResultArtifact>>>,
    handle: Arc<QueryHandle>,
    query_id: String,
    validated: ValidatedSemanticRequest,
    deadline_unix_ms: i64,
    freshness: FreshnessBarrier,
    freshness_timeout: std::time::Duration,
) {
    let remaining = deadline_unix_ms.saturating_sub(now_millis());
    let executed = if remaining <= 0 {
        Err(public_boundary_error(
            "FRESHNESS_DEADLINE_EXCEEDED",
            crate::registries::Phase::Execution,
            "query deadline elapsed",
        ))
    } else {
        let request_timeout = std::time::Duration::from_millis(remaining.cast_unsigned());
        let admission = match validated.request.freshness_policy {
            FreshnessPolicy::CurrentRequired => FreshnessAdmission::RequireCurrent,
            FreshnessPolicy::WaitForCurrent => FreshnessAdmission::AwaitLatest,
            FreshnessPolicy::BestAvailableSnapshot => FreshnessAdmission::BestAvailable,
        };
        match freshness
            .admit_query(admission, freshness_timeout.min(request_timeout))
            .await
        {
            Ok(_) => {
                match tokio::time::timeout(request_timeout, backend.execute(validated)).await {
                    Ok(Ok(executed)) => Ok(executed),
                    Ok(Err(error)) => Err(public_boundary_error(
                        "INTERNAL",
                        crate::registries::Phase::Execution,
                        &error.to_string(),
                    )),
                    Err(_) => Err(public_boundary_error(
                        "FRESHNESS_DEADLINE_EXCEEDED",
                        crate::registries::Phase::Execution,
                        "query deadline elapsed",
                    )),
                }
            }
            Err(crate::lifecycle::LifecycleError::Stale) => Err(public_boundary_error(
                "CURRENT_FACTS_UNAVAILABLE",
                crate::registries::Phase::PolicyValidation,
                "current source state is not available",
            )),
            Err(crate::lifecycle::LifecycleError::Unavailable) => Err(public_boundary_error(
                "CURRENT_FACTS_UNAVAILABLE",
                crate::registries::Phase::PolicyValidation,
                "workspace source is unavailable",
            )),
            Err(error) => Err(public_boundary_error(
                "INTERNAL",
                crate::registries::Phase::PolicyValidation,
                &error.to_string(),
            )),
        }
    };
    if handle.cancelled.load(Ordering::Acquire) {
        return;
    }
    let executed = match executed {
        Ok(executed) => executed,
        Err(error) => {
            let execution_state = QueryExecutionState::Failed;
            let freshness_state = match freshness.state() {
                FreshnessState::Current => "CURRENT",
                FreshnessState::PotentiallyStale => "POTENTIALLY_STALE",
                FreshnessState::Unavailable => "UNAVAILABLE",
            };
            append_terminal(
                &handle,
                QueryEvent {
                    event: Some(Event::Terminal(TerminalEvent {
                        header: Some(event_header(&query_id, 1, None)),
                        execution_state: execution_state as i32,
                        availability_state: if error.status.code() == tonic::Code::Unavailable {
                            "UNAVAILABLE"
                        } else {
                            "FAILED"
                        }
                        .to_owned(),
                        freshness_state: freshness_state.to_owned(),
                        limit_state: "NOT_APPLIED".to_owned(),
                        dependency_state: "NOT_EXECUTED".to_owned(),
                        canonical_response_checksum: None,
                        canonical_error_record_json: Some(error.canonical_record_json.into_bytes()),
                        artifact_id: None,
                        result_row_count: 0,
                        result_byte_count: 0,
                        cleanup_state: "COMPLETE".to_owned(),
                    })),
                },
                execution_state,
            )
            .await;
            return;
        }
    };
    let snapshot_id = executed.response.snapshot.snapshot_id.clone();
    let Ok(artifact) = artifacts.insert(
        executed.canonical_bytes,
        &handle.agent_instance_id,
        &handle.workspace_id,
        &snapshot_id,
    ) else {
        append_terminal(
            &handle,
            QueryEvent {
                event: Some(Event::Terminal(TerminalEvent {
                    header: Some(event_header(&query_id, 1, Some(snapshot_id))),
                    execution_state: QueryExecutionState::Failed as i32,
                    availability_state: "UNAVAILABLE".to_owned(),
                    freshness_state: "UNKNOWN".to_owned(),
                    limit_state: "NOT_APPLIED".to_owned(),
                    dependency_state: "SATISFIED".to_owned(),
                    canonical_response_checksum: None,
                    canonical_error_record_json: None,
                    artifact_id: None,
                    result_row_count: 0,
                    result_byte_count: 0,
                    cleanup_state: "COMPLETE".to_owned(),
                })),
            },
            QueryExecutionState::Failed,
        )
        .await;
        return;
    };
    if handle.cancelled.load(Ordering::Acquire) {
        return;
    }
    let Ok(snapshot_bytes) = crate::contracts::jcs::canonicalize_value(
        &serde_json::to_value(&executed.response.snapshot).unwrap_or(serde_json::Value::Null),
    ) else {
        return;
    };
    artifact_records
        .lock()
        .await
        .insert(artifact.id.clone(), artifact.clone());
    let events = vec![
        QueryEvent {
            event: Some(Event::SnapshotPinned(SnapshotPinnedEvent {
                header: Some(event_header(&query_id, 1, Some(snapshot_id.clone()))),
                metadata_checksum: framed_digest(&snapshot_bytes),
                canonical_public_snapshot_metadata_json: snapshot_bytes,
            })),
        },
        QueryEvent {
            event: Some(Event::ArtifactReady(ArtifactReadyEvent {
                header: Some(event_header(&query_id, 2, Some(snapshot_id))),
                artifact_id: artifact.id.clone(),
                artifact_checksum: artifact.checksum.clone(),
                content_type: "application/json".to_owned(),
                encoding: PayloadCompression::Identity as i32,
                lease_expires_at_unix_ms: artifact.lease_expires_at_unix_ms,
                lease_token: artifact.lease_token.clone(),
            })),
        },
        QueryEvent {
            event: Some(Event::Terminal(TerminalEvent {
                header: Some(event_header(&query_id, 3, None)),
                execution_state: QueryExecutionState::Succeeded as i32,
                availability_state: "AVAILABLE".to_owned(),
                freshness_state: executed.response.freshness_state.to_owned(),
                limit_state: "NOT_APPLIED".to_owned(),
                dependency_state: "SATISFIED".to_owned(),
                canonical_response_checksum: Some(artifact.checksum.clone()),
                canonical_error_record_json: None,
                artifact_id: Some(artifact.id.clone()),
                result_row_count: executed
                    .response
                    .query_results
                    .iter()
                    .map(|result| result.output_row_count as u64)
                    .sum(),
                result_byte_count: artifact.bytes.len() as u64,
                cleanup_state: "RETAINED_BY_LEASE".to_owned(),
            })),
        },
    ];
    let mut state = handle.state.lock().await;
    if state.terminal_state.is_none() && !handle.cancelled.load(Ordering::Acquire) {
        state.events.extend(events);
        state.terminal_state = Some(QueryExecutionState::Succeeded);
        drop(state);
        handle.changed.notify_waiters();
    }
}

#[tonic::async_trait]
impl<B: SemanticQueryBackend> CpgQueryService for ProductionQueryService<B> {
    async fn handshake(
        &self,
        request: Request<HandshakeRequest>,
    ) -> Result<Response<HandshakeResponse>, Status> {
        let request = request.into_inner();
        let claims = self.authorization.authorize_handshake(&request)?;
        let rpc_versions = request
            .rpc_versions
            .as_ref()
            .ok_or_else(|| Status::failed_precondition("RPC version range is missing"))?;
        let semantic_versions = request
            .semantic_query_versions
            .as_ref()
            .ok_or_else(|| Status::failed_precondition("semantic version range is missing"))?;
        if rpc_versions.minimum != "1.0"
            || rpc_versions.maximum != "1.0"
            || semantic_versions.minimum != "1.3"
            || semantic_versions.maximum != "1.3"
        {
            return Err(Status::failed_precondition(
                "no accepted RPC or semantic query version overlap",
            ));
        }
        let negotiated = negotiate_feature_bits(
            request.required_feature_bits,
            request.optional_feature_bits,
            SUPPORTED_FEATURE_BITS,
        )?;
        Ok(Response::new(HandshakeResponse {
            daemon_instance_id: "codefabric-local-daemon".to_owned(),
            daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
            rust_build: env!("CARGO_PKG_VERSION").to_owned(),
            negotiated_rpc_version: "1.0".to_owned(),
            negotiated_semantic_query_version: "1.3".to_owned(),
            negotiated_feature_bits: negotiated,
            negotiated_compression: PayloadCompression::Identity as i32,
            installed_bundles: vec![self.query_bundle.clone()],
            active_schema_fingerprints: Vec::new(),
            effective_limits: Some(EffectiveLimitsProfile {
                maximum_control_message_bytes: MAX_CONTROL_MESSAGE_BYTES as u64,
                maximum_payload_chunk_bytes: MAX_PAYLOAD_CHUNK_BYTES as u64,
                maximum_inline_response_bytes: MAX_PAYLOAD_CHUNK_BYTES as u64,
                maximum_concurrent_queries: 4,
                query_orphan_replay_seconds: 300,
                profile_digest: frame_digest(
                    semantic_fingerprint(SemanticFingerprintDomain::LocalQueryLimits).finalize(),
                ),
            }),
            authorized_workspaces: claims,
            server_time_unix_ms: now_millis(),
            readiness: Some(ReadinessSummary {
                readiness: WorkspaceReadiness::Ready as i32,
                reason_code: None,
                active_snapshot_id: None,
                supported_language_codes: vec![10, 20],
                supported_query_forms: supported_query_forms(),
                capability_codes: Vec::new(),
            }),
        }))
    }

    async fn get_status(
        &self,
        request: Request<StatusRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let request = request.into_inner();
        self.authorization
            .authorize_workspace(&request.workspace_id)?;
        let snapshot = self
            .backend
            .public_snapshot(&request.workspace_id)
            .await
            .map_err(|error| Status::failed_precondition(error.to_string()))?;
        let status = PublicStatusView {
            ready: true,
            workspace_id: request.workspace_id.clone(),
            agent_instance_id: request.agent_instance_id,
            snapshot,
            versions: BTreeMap::from([
                ("daemon".to_owned(), env!("CARGO_PKG_VERSION").to_owned()),
                ("rpc".to_owned(), "1.0".to_owned()),
                ("semantic_query".to_owned(), "1.3".to_owned()),
            ]),
            supported_languages: vec!["python".to_owned(), "rust".to_owned()],
            supported_request_forms: supported_query_forms(),
            capability_statuses: Vec::new(),
            freshness_state: match self.freshness.state() {
                FreshnessState::Current => "CURRENT",
                FreshnessState::PotentiallyStale => "POTENTIALLY_STALE",
                FreshnessState::Unavailable => "UNAVAILABLE",
            },
            service_limits: PublicServiceLimits {
                maximum_control_message_bytes: MAX_CONTROL_MESSAGE_BYTES as u64,
                maximum_payload_chunk_bytes: MAX_PAYLOAD_CHUNK_BYTES as u64,
                maximum_concurrent_queries: 4,
            },
            notices: Vec::new(),
        };
        let canonical = crate::contracts::jcs::canonicalize_value(
            &serde_json::to_value(status)
                .map_err(|_| Status::internal("status serialization failed"))?,
        )
        .map_err(|_| Status::internal("status canonicalization failed"))?;
        Ok(Response::new(StatusResponse {
            workspace_id: request.workspace_id,
            readiness: WorkspaceReadiness::Ready as i32,
            status_checksum: framed_digest(&canonical),
            canonical_public_status_json: canonical,
            observed_at_unix_ms: now_millis(),
        }))
    }

    async fn validate_query(
        &self,
        request: Request<ValidateQueryRequest>,
    ) -> Result<Response<ValidateQueryResponse>, Status> {
        let request = request.into_inner();
        self.authorization
            .authorize_workspace(&request.workspace_id)?;
        validate_checksum(&request.request_checksum, &request.canonical_request_json)?;
        let validated = validate_request(&request.canonical_request_json)
            .map_err(|error| Status::invalid_argument(error.to_string()))?;
        if validated.request.workspace_id != request.workspace_id {
            return Err(Status::invalid_argument(
                "request workspace differs from RPC workspace",
            ));
        }
        Ok(Response::new(ValidateQueryResponse {
            valid: true,
            canonical_normalized_request_json: validated.canonical_bytes,
            normalized_request_checksum: validated.request_digest,
            effective_semantic_request_id: validated.request.semantic_request_id,
            provisional_snapshot_checks: vec!["workspace-authorized".to_owned()],
            canonical_error_records_json: Vec::new(),
            cost_class: "bounded-wave5".to_owned(),
        }))
    }

    async fn start_query(
        &self,
        request: Request<StartQueryRequest>,
    ) -> Result<Response<StartQueryResponse>, Status> {
        let request = request.into_inner();
        self.authorization
            .authorize_workspace(&request.workspace_id)?;
        if request.deadline_unix_ms <= now_millis() {
            return Err(Status::deadline_exceeded("query deadline elapsed"));
        }
        if request.idempotency_key.is_empty() || request.idempotency_key.len() > 256 {
            return Err(Status::invalid_argument("invalid idempotency key"));
        }
        if !matches!(
            DeliveryPreference::try_from(request.delivery_preference),
            Ok(DeliveryPreference::Inline
                | DeliveryPreference::Resource
                | DeliveryPreference::Auto)
        ) || PayloadCompression::try_from(request.payload_compression)
            != Ok(PayloadCompression::Identity)
        {
            return Err(Status::invalid_argument(
                "unsupported delivery or compression",
            ));
        }
        let mut idempotency = self.idempotency.lock().await;
        if let Some(existing) = idempotency.get(&request.idempotency_key) {
            let handle = self
                .handles
                .lock()
                .await
                .get(existing)
                .cloned()
                .ok_or_else(|| Status::internal("idempotency index points to a missing handle"))?;
            let terminal_state = handle
                .state
                .lock()
                .await
                .terminal_state
                .unwrap_or(QueryExecutionState::Accepted);
            return Ok(Response::new(StartQueryResponse {
                daemon_query_id: existing.clone(),
                resume_token: handle.resume_token.clone(),
                accepted_at_unix_ms: now_millis(),
                query_execution_state: terminal_state as i32,
                queue_class: "immediate".to_owned(),
                queue_position: None,
                negotiated_request_version: "1.3".to_owned(),
                negotiated_response_version: "1.3".to_owned(),
                effective_semantic_request_id: request.semantic_request_id.unwrap_or_default(),
            }));
        }
        validate_checksum(&request.request_checksum, &request.canonical_request_json)?;
        let validated = validate_request(&request.canonical_request_json)
            .map_err(|error| Status::invalid_argument(error.to_string()))?;
        if validated.request.workspace_id != request.workspace_id {
            return Err(Status::invalid_argument(
                "request workspace differs from RPC workspace",
            ));
        }
        let semantic_request_id = validated.request.semantic_request_id.clone();
        let query_id = format!("query:{}", &request.request_checksum[3..35]);
        let resume_token = opaque_bytes(SemanticFingerprintDomain::QueryResume, &query_id);
        // The accepted v1 response exposes one opaque handle token. It therefore
        // authorizes both replay and cancellation; future protocol majors may split it.
        let cancel_token = resume_token.clone();
        let handle = Arc::new(QueryHandle {
            resume_token: resume_token.clone(),
            cancel_token,
            agent_instance_id: request.agent_instance_id,
            workspace_id: request.workspace_id,
            cancelled: AtomicBool::new(false),
            state: Mutex::new(QueryHandleState::default()),
            changed: Notify::new(),
        });
        self.handles
            .lock()
            .await
            .insert(query_id.clone(), Arc::clone(&handle));
        idempotency.insert(request.idempotency_key, query_id.clone());
        drop(idempotency);
        tokio::spawn(execute_accepted_query(
            Arc::clone(&self.backend),
            Arc::clone(&self.artifacts),
            Arc::clone(&self.artifact_records),
            handle,
            query_id.clone(),
            validated,
            request.deadline_unix_ms,
            self.freshness.clone(),
            self.freshness_timeout,
        ));
        Ok(Response::new(StartQueryResponse {
            daemon_query_id: query_id,
            resume_token,
            accepted_at_unix_ms: now_millis(),
            query_execution_state: QueryExecutionState::Accepted as i32,
            queue_class: "accepted".to_owned(),
            queue_position: None,
            negotiated_request_version: "1.3".to_owned(),
            negotiated_response_version: "1.3".to_owned(),
            effective_semantic_request_id: semantic_request_id,
        }))
    }

    type StreamQueryStream = QueryStream;

    async fn stream_query(
        &self,
        request: Request<StreamQueryRequest>,
    ) -> Result<Response<Self::StreamQueryStream>, Status> {
        let request = request.into_inner();
        let handle = self
            .handles
            .lock()
            .await
            .get(&request.daemon_query_id)
            .cloned()
            .ok_or_else(|| Status::not_found("query handle not found"))?;
        if handle.resume_token != request.resume_token {
            return Err(Status::permission_denied("resume token differs"));
        }
        Ok(Response::new(stream_after(handle, request.after_sequence)))
    }

    type AttachQueryStream = QueryStream;

    async fn attach_query(
        &self,
        request: Request<AttachQueryRequest>,
    ) -> Result<Response<Self::AttachQueryStream>, Status> {
        let request = request.into_inner();
        self.authorization
            .authorize_workspace(&request.workspace_id)?;
        let handle = self
            .handles
            .lock()
            .await
            .get(&request.daemon_query_id)
            .cloned()
            .ok_or_else(|| Status::not_found("query handle not found"))?;
        if handle.workspace_id != request.workspace_id
            || handle.agent_instance_id != request.agent_instance_id
            || handle.resume_token != request.resume_token
        {
            return Err(Status::permission_denied(
                "query attachment identity differs",
            ));
        }
        if let Some(expected) = request.after_event_checksum.as_deref()
            && request.after_sequence != 0
        {
            let state = handle.state.lock().await;
            let index = usize::try_from(request.after_sequence - 1)
                .map_err(|_| Status::out_of_range("attachment sequence is invalid"))?;
            let actual = state
                .events
                .get(index)
                .and_then(|event| event.event.as_ref())
                .and_then(event_checksum)
                .ok_or_else(|| Status::out_of_range("attachment sequence is unavailable"))?;
            if actual != expected {
                return Err(Status::failed_precondition("attachment checksum differs"));
            }
        }
        Ok(Response::new(stream_after(handle, request.after_sequence)))
    }

    async fn cancel_query(
        &self,
        request: Request<CancelQueryRequest>,
    ) -> Result<Response<CancelQueryResponse>, Status> {
        let request = request.into_inner();
        self.authorization
            .authorize_workspace(&request.workspace_id)?;
        let Some(handle) = self
            .handles
            .lock()
            .await
            .get(&request.daemon_query_id)
            .cloned()
        else {
            return Ok(Response::new(CancelQueryResponse {
                daemon_query_id: request.daemon_query_id,
                state: CancellationState::NotFound as i32,
                acknowledged_at_unix_ms: now_millis(),
                terminal_state: None,
                cleaning_up_components: Vec::new(),
                forced_termination: false,
            }));
        };
        if handle.workspace_id != request.workspace_id
            || handle.agent_instance_id != request.agent_instance_id
            || handle.cancel_token != request.cancel_token
        {
            return Err(Status::permission_denied(
                "query cancellation identity differs",
            ));
        }
        let terminal = handle.state.lock().await.terminal_state;
        if let Some(terminal) = terminal {
            return Ok(Response::new(CancelQueryResponse {
                daemon_query_id: request.daemon_query_id,
                state: CancellationState::AlreadyTerminal as i32,
                acknowledged_at_unix_ms: now_millis(),
                terminal_state: Some(terminal as i32),
                cleaning_up_components: Vec::new(),
                forced_termination: false,
            }));
        }
        handle.cancelled.store(true, Ordering::Release);
        append_terminal(
            &handle,
            QueryEvent {
                event: Some(Event::Terminal(TerminalEvent {
                    header: Some(event_header(&request.daemon_query_id, 1, None)),
                    execution_state: QueryExecutionState::Cancelled as i32,
                    availability_state: "UNAVAILABLE".to_owned(),
                    freshness_state: "UNKNOWN".to_owned(),
                    limit_state: "NOT_APPLIED".to_owned(),
                    dependency_state: "NOT_EXECUTED".to_owned(),
                    canonical_response_checksum: None,
                    canonical_error_record_json: None,
                    artifact_id: None,
                    result_row_count: 0,
                    result_byte_count: 0,
                    cleanup_state: "COMPLETE".to_owned(),
                })),
            },
            QueryExecutionState::Cancelled,
        )
        .await;
        Ok(Response::new(CancelQueryResponse {
            daemon_query_id: request.daemon_query_id,
            state: CancellationState::Cancelled as i32,
            acknowledged_at_unix_ms: now_millis(),
            terminal_state: Some(QueryExecutionState::Cancelled as i32),
            cleaning_up_components: Vec::new(),
            forced_termination: false,
        }))
    }

    type ReadResultStream = ArtifactStream;

    async fn read_result(
        &self,
        request: Request<ReadResultRequest>,
    ) -> Result<Response<Self::ReadResultStream>, Status> {
        let request = request.into_inner();
        if PayloadCompression::try_from(request.accepted_compression)
            != Ok(PayloadCompression::Identity)
        {
            return Err(Status::invalid_argument(
                "only identity encoding is supported",
            ));
        }
        let records = self.artifact_records.lock().await;
        let artifact = records
            .get(&request.artifact_id)
            .ok_or_else(|| Status::not_found("result artifact not found"))?;
        if artifact.lease_token != request.lease_token {
            return Err(Status::permission_denied("result lease token differs"));
        }
        let offset = usize::try_from(request.offset)
            .map_err(|_| Status::out_of_range("result offset is invalid"))?;
        if offset > artifact.bytes.len() {
            return Err(Status::out_of_range(
                "result offset exceeds artifact length",
            ));
        }
        let maximum = usize::try_from(
            request
                .maximum_bytes
                .unwrap_or(MAX_PAYLOAD_CHUNK_BYTES as u64)
                .min(MAX_PAYLOAD_CHUNK_BYTES as u64),
        )
        .map_err(|_| Status::out_of_range("maximum result bytes is invalid"))?;
        if maximum == 0 {
            return Err(Status::invalid_argument(
                "maximum result bytes must be positive",
            ));
        }
        let end = offset.saturating_add(maximum).min(artifact.bytes.len());
        let payload = artifact.bytes[offset..end].to_vec();
        let event = ResultChunk {
            artifact_id: artifact.id.clone(),
            offset: request.offset,
            uncompressed_length: payload.len() as u64,
            payload_checksum: framed_digest(&payload),
            payload,
            artifact_checksum: artifact.checksum.clone(),
            content_type: "application/json".to_owned(),
            encoding: PayloadCompression::Identity as i32,
            final_chunk: end == artifact.bytes.len(),
            lease_expires_at_unix_ms: artifact.lease_expires_at_unix_ms,
        };
        Ok(Response::new(Box::pin(stream::once(
            async move { Ok(event) },
        ))))
    }

    async fn release_result(
        &self,
        request: Request<ReleaseResultRequest>,
    ) -> Result<Response<ReleaseResultResponse>, Status> {
        let request = request.into_inner();
        let mut records = self.artifact_records.lock().await;
        let released = records
            .get(&request.artifact_id)
            .is_some_and(|artifact| artifact.lease_token == request.lease_token);
        if released {
            records.remove(&request.artifact_id);
        }
        Ok(Response::new(ReleaseResultResponse {
            artifact_id: request.artifact_id,
            released,
            remaining_lease_expires_at_unix_ms: None,
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt as _;

    use hyper_util::rt::TokioIo;
    use tokio::net::UnixStream;
    use tokio::sync::oneshot;
    use tonic::transport::Endpoint;
    use tower::service_fn;

    use super::*;

    #[test]
    fn wp61_structural_acceptance() {
        use crate::registries::{PUBLIC_ERROR_ENTRIES, PUBLIC_ERROR_IDS, Phase};

        assert_eq!(PUBLIC_ERROR_ENTRIES.len(), PUBLIC_ERROR_IDS.len());
        for (entry, name) in PUBLIC_ERROR_ENTRIES.iter().zip(PUBLIC_ERROR_IDS) {
            assert_eq!(entry.name, *name);
            let boundary = public_boundary_error(entry.name, Phase::Execution, "fixture");
            assert_eq!(boundary.status.code(), grpc_code(entry.grpc_status));
            let record: serde_json::Value =
                serde_json::from_str(&boundary.canonical_record_json).unwrap();
            assert_eq!(record["code"], entry.code);
            assert_eq!(record["severity"], entry.severity);
            assert_eq!(record["retryability"], entry.retryability);
            assert_eq!(record["grpc_status"], entry.grpc_status);
            assert_eq!(record["mcp_mapping"], entry.mcp_mapping);
        }
    }

    #[test]
    fn wp56_operational_acceptance() {
        let bundle = query_bundle_identity();
        assert_eq!(bundle.bundle_id, "codefabric.bundles.query-language-bundle");
        assert_eq!(bundle.bundle_version, "1.0");
        assert_eq!(
            supported_query_forms(),
            vec![
                "find code entities",
                "retrieve facts about code",
                "follow code relationships",
            ]
        );
    }
    use crate::rpc::generated::codefabric::cpgd::v1::cpg_query_service_client::CpgQueryServiceClient;
    use crate::rpc::generated::codefabric::cpgd::v1::{CredentialProof, VersionRange};
    use crate::semantic_query::{
        QueryResultRecord, SemanticQueryResponse, SemanticSnapshotResponse,
    };

    struct FakeBackend;

    struct BlockingBackend {
        started: Arc<Notify>,
        release: Arc<Notify>,
    }

    fn authorization() -> QueryAuthorization {
        QueryAuthorization::new(
            b"test-capability-token",
            vec![WorkspaceClaim {
                workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                repository_id: None,
                worktree_id: None,
                workspace_kind: "directory".to_owned(),
                readiness: WorkspaceReadiness::Ready as i32,
                permission_claims: vec!["query".to_owned()],
            }],
        )
        .unwrap()
    }

    fn fake_snapshot() -> SemanticSnapshotResponse {
        SemanticSnapshotResponse {
            snapshot_id: "snapshot:00000000000000000000000000000000".to_owned(),
            workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
            repository_id: None,
            worktree_id: None,
            source_generation: 1,
            source_inventory_digest: framed_digest(b"inventory"),
            durable_base_publication: "publication:00000000000000000000000000000000".to_owned(),
            base_table_version_digest: framed_digest(b"tables"),
            overlay_generation: 0,
            overlay_checksum: framed_digest(b"overlay"),
            analysis_context_set_id: "context-set:00000000000000000000000000000000".to_owned(),
            analysis_context_ids: vec!["context:source".to_owned()],
            freshness_state: "CURRENT",
            source_trust_state: "CURRENT_BYTES_VERIFIED".to_owned(),
            event_stream_health: "HEALTHY".to_owned(),
            git_acceleration_status: "NOT_REQUIRED".to_owned(),
            git_operation_summary: None,
            pending_update_count: 0,
            ontology_version: "1.3".to_owned(),
            schema_bundle_version: "1.0".to_owned(),
            provider_bundle_version: "1.0".to_owned(),
            derivation_bundle_version: "1.0".to_owned(),
            query_language_version: "1.3".to_owned(),
            capability_summaries: Vec::new(),
            diagnostic_references: Vec::new(),
        }
    }

    #[async_trait]
    impl SemanticQueryBackend for FakeBackend {
        async fn execute(
            &self,
            request: ValidatedSemanticRequest,
        ) -> Result<ExecutedSemanticResponse, SemanticQueryError> {
            let response = SemanticQueryResponse {
                specification: "composable semantic CPG fact query response",
                version: "1.3",
                semantic_request_id: request.request.semantic_request_id,
                execution_state: "SUCCEEDED",
                availability_state: "AVAILABLE",
                completeness_state: "COMPLETE",
                freshness_state: "CURRENT",
                limit_state: "NOT_APPLIED",
                successful_query_count: 1,
                failed_query_count: 0,
                not_executed_dependency_count: 0,
                snapshot: fake_snapshot(),
                entities: BTreeMap::new(),
                facts: BTreeMap::new(),
                paths: BTreeMap::new(),
                groups: BTreeMap::new(),
                source_contexts: BTreeMap::new(),
                query_results: vec![QueryResultRecord {
                    query_id: "q1".to_owned(),
                    request: crate::semantic_query::QueryForm::FindEntities,
                    execution_state: "SUCCEEDED",
                    availability_state: "AVAILABLE",
                    completeness_state: "COMPLETE",
                    freshness_state: "CURRENT",
                    limit_state: "NOT_APPLIED",
                    dependency_state: "SATISFIED",
                    resolved_semantics: BTreeMap::new(),
                    entity_ids: Vec::new(),
                    fact_ids: Vec::new(),
                    path_ids: Vec::new(),
                    group_ids: Vec::new(),
                    source_context_ids: Vec::new(),
                    coverage: BTreeMap::new(),
                    errors: Vec::new(),
                    notices: Vec::new(),
                    output_row_count: 1,
                    result_checksum: framed_digest(b"row"),
                }],
                errors: Vec::new(),
            };
            let canonical_bytes = crate::contracts::jcs::canonicalize_value(
                &serde_json::to_value(&response).unwrap(),
            )?;
            Ok(ExecutedSemanticResponse {
                response_digest: framed_digest(&canonical_bytes),
                response,
                canonical_bytes,
            })
        }

        async fn public_snapshot(
            &self,
            workspace_id: &str,
        ) -> Result<SemanticSnapshotResponse, SemanticQueryError> {
            let snapshot = fake_snapshot();
            if snapshot.workspace_id != workspace_id {
                return Err(SemanticQueryError::Invalid("workspace differs".to_owned()));
            }
            Ok(snapshot)
        }
    }

    #[async_trait]
    impl SemanticQueryBackend for BlockingBackend {
        async fn execute(
            &self,
            request: ValidatedSemanticRequest,
        ) -> Result<ExecutedSemanticResponse, SemanticQueryError> {
            self.started.notify_one();
            self.release.notified().await;
            SemanticQueryBackend::execute(&FakeBackend, request).await
        }

        async fn public_snapshot(
            &self,
            workspace_id: &str,
        ) -> Result<SemanticSnapshotResponse, SemanticQueryError> {
            SemanticQueryBackend::public_snapshot(&FakeBackend, workspace_id).await
        }
    }

    fn canonical_request() -> Vec<u8> {
        crate::contracts::jcs::canonicalize_slice(br#"{"specification":"composable semantic CPG fact query","version":"1.3","semantic_request_id":"rpc-gate-b","workspace_id":"workspace:00000000000000000000000000000000","freshness_policy":"best_available_snapshot","queries":[{"query_id":"q1","request":"find code entities","label":null,"input":null,"where":null,"limit":{"first":1,"offset":0}}],"response_projection":null,"cost_budget":{"maximum_rows":1}}"#).unwrap()
    }

    fn canonical_current_required_request() -> Vec<u8> {
        crate::contracts::jcs::canonicalize_slice(br#"{"specification":"composable semantic CPG fact query","version":"1.3","semantic_request_id":"rpc-current","workspace_id":"workspace:00000000000000000000000000000000","freshness_policy":"current_required","queries":[{"query_id":"q1","request":"find code entities","label":null,"input":null,"where":null,"limit":{"first":1,"offset":0}}],"response_projection":null,"cost_budget":{"maximum_rows":1}}"#).unwrap()
    }

    fn handshake(token: &[u8]) -> HandshakeRequest {
        HandshakeRequest {
            rpc_versions: Some(VersionRange {
                minimum: "1.0".to_owned(),
                maximum: "1.0".to_owned(),
            }),
            semantic_query_versions: Some(VersionRange {
                minimum: "1.3".to_owned(),
                maximum: "1.3".to_owned(),
            }),
            desired_workspace_ids: vec!["workspace:00000000000000000000000000000000".to_owned()],
            credential_proof: Some(CredentialProof {
                credential_id: "test-credential".to_owned(),
                capability_token: token.to_vec(),
            }),
            agent_instance_id: "test-agent".to_owned(),
            ..HandshakeRequest::default()
        }
    }

    #[tokio::test]
    async fn wp39_structural_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let socket = root.path().join("query.sock");
        let service = ProductionQueryService::new(
            Arc::new(FakeBackend),
            ResultArtifactStore::new(root.path().join("results")).unwrap(),
            authorization(),
        );
        let allowed_uid = fs::metadata(root.path()).unwrap().uid();
        let (shutdown, shutdown_receiver) = oneshot::channel();
        let server_socket = socket.clone();
        let server = tokio::spawn(async move {
            serve_query_uds(&server_socket, allowed_uid, service, async {
                let _ = shutdown_receiver.await;
            })
            .await
        });
        while !socket.exists() {
            tokio::task::yield_now().await;
        }
        let connector_socket = socket.clone();
        let channel = Endpoint::try_from("http://[::]:50051")
            .unwrap()
            .connect_with_connector(service_fn(move |_| {
                let connector_socket = connector_socket.clone();
                async move {
                    UnixStream::connect(connector_socket)
                        .await
                        .map(TokioIo::new)
                }
            }))
            .await
            .unwrap();
        let mut client = CpgQueryServiceClient::new(channel);
        assert_eq!(
            client
                .handshake(handshake(b"wrong-capability-token"))
                .await
                .unwrap_err()
                .code(),
            tonic::Code::Unauthenticated
        );
        let response = client
            .handshake(handshake(b"test-capability-token"))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.authorized_workspaces.len(), 1);
        assert_eq!(response.negotiated_rpc_version, "1.0");
        shutdown.send(()).unwrap();
        server.await.unwrap().unwrap();
        assert!(!socket.exists());
    }

    #[tokio::test]
    async fn wp39_behavioral_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let service = ProductionQueryService::new(
            Arc::new(FakeBackend),
            ResultArtifactStore::new(root.path().to_path_buf()).unwrap(),
            authorization(),
        );
        let canonical = canonical_request();
        let started = service
            .start_query(Request::new(StartQueryRequest {
                agent_instance_id: "test-agent".to_owned(),
                workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                semantic_query_version: "1.3".to_owned(),
                canonical_request_json: canonical.clone(),
                request_checksum: framed_digest(&canonical),
                delivery_preference: DeliveryPreference::Resource as i32,
                deadline_unix_ms: now_millis() + 60_000,
                idempotency_key: "same-request".to_owned(),
                payload_compression: PayloadCompression::Identity as i32,
                ..StartQueryRequest::default()
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(
            started.query_execution_state,
            QueryExecutionState::Accepted as i32
        );
        let events = service
            .stream_query(Request::new(StreamQueryRequest {
                daemon_query_id: started.daemon_query_id,
                resume_token: started.resume_token,
                after_sequence: 0,
            }))
            .await
            .unwrap()
            .into_inner();
        let events = futures::StreamExt::collect::<Vec<_>>(events).await;
        assert_eq!(events.len(), 3);
        assert!(matches!(
            events[0].as_ref().unwrap().event.as_ref(),
            Some(Event::SnapshotPinned(_))
        ));
        assert!(matches!(
            events[2].as_ref().unwrap().event.as_ref(),
            Some(Event::Terminal(_))
        ));
        let artifact = service
            .artifact_records
            .lock()
            .await
            .values()
            .next()
            .cloned()
            .unwrap();
        let result = service
            .read_result(Request::new(ReadResultRequest {
                artifact_id: artifact.id,
                offset: 0,
                maximum_bytes: Some(MAX_PAYLOAD_CHUNK_BYTES as u64),
                lease_token: artifact.lease_token,
                accepted_compression: PayloadCompression::Identity as i32,
            }))
            .await
            .unwrap()
            .into_inner();
        let chunks = futures::StreamExt::collect::<Vec<_>>(result).await;
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].as_ref().unwrap().final_chunk);
    }

    #[tokio::test]
    async fn wp39_negative_zero_state() {
        let root = tempfile::tempdir().unwrap();
        let service = ProductionQueryService::new(
            Arc::new(FakeBackend),
            ResultArtifactStore::new(root.path().to_path_buf()).unwrap(),
            authorization(),
        );
        let canonical = canonical_request();
        let status = service
            .start_query(Request::new(StartQueryRequest {
                agent_instance_id: "test-agent".to_owned(),
                workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                canonical_request_json: canonical,
                request_checksum: framed_digest(b"wrong"),
                delivery_preference: DeliveryPreference::Inline as i32,
                deadline_unix_ms: now_millis() - 1,
                idempotency_key: "expired".to_owned(),
                payload_compression: PayloadCompression::Identity as i32,
                ..StartQueryRequest::default()
            }))
            .await
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::DeadlineExceeded);
    }

    #[tokio::test]
    async fn wp39_operational_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let backend = Arc::new(BlockingBackend {
            started: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
        });
        let service = ProductionQueryService::new(
            Arc::clone(&backend),
            ResultArtifactStore::new(root.path().to_path_buf()).unwrap(),
            authorization(),
        );
        let canonical = canonical_request();
        let started = service
            .start_query(Request::new(StartQueryRequest {
                agent_instance_id: "test-agent".to_owned(),
                workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                semantic_query_version: "1.3".to_owned(),
                canonical_request_json: canonical.clone(),
                request_checksum: framed_digest(&canonical),
                delivery_preference: DeliveryPreference::Resource as i32,
                deadline_unix_ms: now_millis() + 60_000,
                idempotency_key: "cancel-request".to_owned(),
                payload_compression: PayloadCompression::Identity as i32,
                ..StartQueryRequest::default()
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(
            started.query_execution_state,
            QueryExecutionState::Accepted as i32
        );
        backend.started.notified().await;
        let cancelled = service
            .cancel_query(Request::new(CancelQueryRequest {
                daemon_query_id: started.daemon_query_id.clone(),
                cancel_token: started.resume_token.clone(),
                agent_instance_id: "test-agent".to_owned(),
                workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                reason: "test cancellation".to_owned(),
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(cancelled.state, CancellationState::Cancelled as i32);
        let events = service
            .stream_query(Request::new(StreamQueryRequest {
                daemon_query_id: started.daemon_query_id,
                resume_token: started.resume_token,
                after_sequence: 0,
            }))
            .await
            .unwrap()
            .into_inner();
        let events = futures::StreamExt::collect::<Vec<_>>(events).await;
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0].as_ref().unwrap().event.as_ref(),
            Some(Event::Terminal(value))
                if value.execution_state == QueryExecutionState::Cancelled as i32
        ));
        backend.release.notify_one();
        tokio::task::yield_now().await;
        assert!(service.artifact_records.lock().await.is_empty());
    }

    #[tokio::test]
    async fn wp45_strict_current_query_is_rejected_before_backend_execution() {
        let root = tempfile::tempdir().unwrap();
        let freshness = FreshnessBarrier::default();
        let _ = freshness.admit();
        let service = ProductionQueryService::new(
            Arc::new(FakeBackend),
            ResultArtifactStore::new(root.path().to_path_buf()).unwrap(),
            authorization(),
        )
        .with_freshness_barrier(freshness, std::time::Duration::from_millis(50));
        let canonical = canonical_current_required_request();
        let started = service
            .start_query(Request::new(StartQueryRequest {
                agent_instance_id: "test-agent".to_owned(),
                workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                semantic_query_version: "1.3".to_owned(),
                canonical_request_json: canonical.clone(),
                request_checksum: framed_digest(&canonical),
                delivery_preference: DeliveryPreference::Resource as i32,
                deadline_unix_ms: now_millis() + 1_000,
                idempotency_key: "strict-stale".to_owned(),
                payload_compression: PayloadCompression::Identity as i32,
                ..StartQueryRequest::default()
            }))
            .await
            .unwrap()
            .into_inner();
        let events = service
            .stream_query(Request::new(StreamQueryRequest {
                daemon_query_id: started.daemon_query_id,
                resume_token: started.resume_token,
                after_sequence: 0,
            }))
            .await
            .unwrap()
            .into_inner();
        let events = futures::StreamExt::collect::<Vec<_>>(events).await;
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0].as_ref().unwrap().event.as_ref(),
            Some(Event::Terminal(value))
                if value.execution_state == QueryExecutionState::Failed as i32
                    && value.freshness_state == "POTENTIALLY_STALE"
        ));
        assert!(service.artifact_records.lock().await.is_empty());
    }
}
