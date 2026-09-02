use std::fs;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::{Output, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use codefabric::rpc::generated::codefabric::cpgd::v2::cpg_query_service_client::CpgQueryServiceClient;
use codefabric::rpc::generated::codefabric::cpgd::v2::cpg_query_service_server::{
    CpgQueryService, CpgQueryServiceServer,
};
use codefabric::rpc::generated::codefabric::cpgd::v2::get_reference_request::Operation as ReferenceOperation;
use codefabric::rpc::generated::codefabric::cpgd::v2::get_reference_response::Result as ReferenceResult;
use codefabric::rpc::generated::codefabric::cpgd::v2::input_answer::Value as WireInputValue;
use codefabric::rpc::generated::codefabric::cpgd::v2::query_event::Event as WireQueryEvent;
use codefabric::rpc::generated::codefabric::cpgd::v2::resource_selector::Selector as WireResourceSelector;
use codefabric::rpc::generated::codefabric::cpgd::v2::start_query_request::Leg;
use codefabric::rpc::generated::codefabric::cpgd::v2::start_query_response::Outcome;
use codefabric::rpc::generated::codefabric::cpgd::v2::{
    AcceptedQuery, AuthorityGeneration, CancelQueryRequest, CancelQueryResponse,
    CancellationAcknowledgement, GetReferenceRequest, GetReferenceResponse, GetStatusRequest,
    GetStatusResponse, HandshakeRequest, HandshakeResponse, InitialQueryStart, InputAnswer,
    ManifestSelector, PageSelector, QueryChallengeContinuation, QueryEvent, QueryExecutionState,
    QuerySubmission, ReadResourceRequest, ReferenceCompletionRequest, ReferenceKind,
    ReferenceReadRequest, ReferenceTemplateVariable, ReleaseResourceRequest,
    ReleaseResourceResponse, ReleaseState, RequestContext, ResourceChunk, ResourceSelector,
    ResultLimits, StartQueryRequest, StartQueryResponse, ValidateQueryRequest,
    ValidateQueryResponse, WatchQueryRequest,
};
use codefabric::rpc::generated::codefabric::provider::v1::ProviderJobSpec;
use codefabric::rpc::generated::codefabric::pyrefly::v1::Hello;
use codefabric::rpc::generated::codefabric::rustc::v1::CompilationAccepted;
use codefabric::rpc::{AuthorizedUnixStream, MAX_CONTROL_MESSAGE_BYTES, SameUserInterceptor};
use codefabric::rpc_interop_test_support::{
    INTEROP_DAEMON_GENERATION, INTEROP_SEMANTIC_PROFILE, ProductionRpcInteropControl,
    interop_workspace_public_id, production_rpc_interop_fixture,
};
use codefabric::session_authority::SESSION_METADATA_KEY;
use hyper_util::rt::TokioIo;
use prost::Message;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{UnixListener, UnixStream};
use tokio::process::Command;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_stream::Stream;
use tokio_stream::StreamExt as _;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::metadata::MetadataValue;
use tonic::service::InterceptorLayer;
use tonic::transport::server::Connected;
use tonic::transport::{Channel, Endpoint, Server};
use tonic::{Request, Response, Status};
use tower::service_fn;

type QueryStream = Pin<Box<dyn Stream<Item = Result<QueryEvent, Status>> + Send>>;
type ResourceStream = Pin<Box<dyn Stream<Item = Result<ResourceChunk, Status>> + Send>>;

#[derive(Clone)]
struct ProbeService {
    invocations: Arc<AtomicUsize>,
}

#[tonic::async_trait]
impl CpgQueryService for ProbeService {
    async fn handshake(
        &self,
        request: Request<HandshakeRequest>,
    ) -> Result<Response<HandshakeResponse>, Status> {
        self.invocations.fetch_add(1, Ordering::SeqCst);
        let request = request.into_inner();
        if request.adapter_version == "delay" {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        let session_id = if request.adapter_version == "large-response" {
            "x".repeat(MAX_CONTROL_MESSAGE_BYTES + 1)
        } else {
            "session:v2-probe".to_owned()
        };
        Ok(Response::new(HandshakeResponse {
            session_token: vec![0x41; 32],
            authority: Some(AuthorityGeneration {
                session_id,
                session_generation: 1,
                daemon_generation: 1,
                supervisor_generation: 1,
                policy_generation: 1,
                revocation_generation: 0,
            }),
            selected_minor: 0,
            selected_semantic_profile: "codefabric.semantic-query.v2".to_owned(),
            ..HandshakeResponse::default()
        }))
    }

    async fn get_status(
        &self,
        _request: Request<GetStatusRequest>,
    ) -> Result<Response<GetStatusResponse>, Status> {
        Ok(Response::new(GetStatusResponse::default()))
    }

    async fn get_reference(
        &self,
        request: Request<GetReferenceRequest>,
    ) -> Result<Response<GetReferenceResponse>, Status> {
        let _request = request.into_inner();
        Ok(Response::new(GetReferenceResponse::default()))
    }

    async fn validate_query(
        &self,
        _request: Request<ValidateQueryRequest>,
    ) -> Result<Response<ValidateQueryResponse>, Status> {
        Ok(Response::new(ValidateQueryResponse::default()))
    }

    async fn start_query(
        &self,
        _request: Request<StartQueryRequest>,
    ) -> Result<Response<StartQueryResponse>, Status> {
        self.invocations.fetch_add(1, Ordering::SeqCst);
        Ok(Response::new(StartQueryResponse {
            outcome: Some(Outcome::Accepted(AcceptedQuery {
                daemon_query_id: "query:v2-probe".to_owned(),
                ..AcceptedQuery::default()
            })),
        }))
    }

    type WatchQueryStream = QueryStream;

    async fn watch_query(
        &self,
        _request: Request<WatchQueryRequest>,
    ) -> Result<Response<Self::WatchQueryStream>, Status> {
        Ok(Response::new(Box::pin(tokio_stream::empty())))
    }

    async fn cancel_query(
        &self,
        _request: Request<CancelQueryRequest>,
    ) -> Result<Response<CancelQueryResponse>, Status> {
        Ok(Response::new(CancelQueryResponse::default()))
    }

    type ReadResourceStream = ResourceStream;

    async fn read_resource(
        &self,
        _request: Request<ReadResourceRequest>,
    ) -> Result<Response<Self::ReadResourceStream>, Status> {
        Ok(Response::new(Box::pin(tokio_stream::empty())))
    }

    async fn release_resource(
        &self,
        _request: Request<ReleaseResourceRequest>,
    ) -> Result<Response<ReleaseResourceResponse>, Status> {
        Ok(Response::new(ReleaseResourceResponse::default()))
    }
}

struct RunningServer {
    socket: PathBuf,
    directory: PathBuf,
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), tonic::transport::Error>>,
    invocations: Arc<AtomicUsize>,
}

struct RunningProductionServer {
    socket: PathBuf,
    directory: tempfile::TempDir,
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), tonic::transport::Error>>,
    control: ProductionRpcInteropControl,
}

impl RunningProductionServer {
    async fn stop(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task
            .await
            .expect("production server task")
            .expect("production server exit");
        if self.socket.exists() {
            fs::remove_file(&self.socket).expect("remove production test socket");
        }
        drop(self.directory);
    }
}

impl RunningServer {
    async fn stop(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.await.expect("server task").expect("server exit");
        if self.socket.exists() {
            fs::remove_file(&self.socket).expect("remove test socket");
        }
        fs::remove_dir(&self.directory).expect("remove test directory");
    }
}

fn test_socket(label: &str) -> (PathBuf, PathBuf, u32) {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let directory = Path::new("/tmp").join(format!("cf-{label}-{}-{nonce:x}", std::process::id()));
    fs::create_dir(&directory).expect("create per-test directory");
    let uid = fs::metadata(&directory).expect("directory metadata").uid();
    let socket = directory.join("cpgd.sock");
    (directory, socket, uid)
}

async fn start_production_server(label: &str, launch_grant: [u8; 32]) -> RunningProductionServer {
    let directory = tempfile::Builder::new()
        .prefix(&format!("cf-{label}-"))
        .tempdir_in("/tmp")
        .expect("create production RPC test directory");
    start_production_server_in(directory, launch_grant, std::process::id()).await
}

async fn start_production_server_in(
    directory: tempfile::TempDir,
    launch_grant: [u8; 32],
    peer_pid: u32,
) -> RunningProductionServer {
    let socket = directory.path().join("cpgd.sock");
    let listener = UnixListener::bind(&socket).expect("bind production RPC socket");
    let allowed_uid = current_uid();
    let incoming = UnixListenerStream::new(listener).filter_map(move |result| match result {
        Ok(stream) => AuthorizedUnixStream::authenticate(stream, allowed_uid)
            .ok()
            .map(Ok),
        Err(error) => Some(Err(error)),
    });
    let (service, control) = production_rpc_interop_fixture(
        &directory.path().join("query-coordinator.sqlite"),
        allowed_uid,
        peer_pid,
        launch_grant,
    )
    .await;
    let service = CpgQueryServiceServer::new(service)
        .max_decoding_message_size(MAX_CONTROL_MESSAGE_BYTES)
        .max_encoding_message_size(MAX_CONTROL_MESSAGE_BYTES);
    let (shutdown, shutdown_receiver) = oneshot::channel();
    let task = tokio::spawn(async move {
        Server::builder()
            .add_service(service)
            .serve_with_incoming_shutdown(incoming, async {
                let _ = shutdown_receiver.await;
            })
            .await
    });
    RunningProductionServer {
        socket,
        directory,
        shutdown: Some(shutdown),
        task,
        control,
    }
}

fn configured_service(
    invocations: Arc<AtomicUsize>,
    enforce_limits: bool,
) -> CpgQueryServiceServer<ProbeService> {
    let service = CpgQueryServiceServer::new(ProbeService { invocations });
    if enforce_limits {
        service
            .max_decoding_message_size(MAX_CONTROL_MESSAGE_BYTES)
            .max_encoding_message_size(MAX_CONTROL_MESSAGE_BYTES)
    } else {
        service
    }
}

fn start_authenticated_server(expected_uid: u32, enforce_limits: bool) -> RunningServer {
    let (directory, socket, _) = test_socket("authenticated-v2-rpc");
    let listener = UnixListener::bind(&socket).expect("bind authenticated socket");
    let incoming = UnixListenerStream::new(listener).filter_map(move |result| match result {
        Ok(stream) => AuthorizedUnixStream::authenticate(stream, expected_uid)
            .ok()
            .map(Ok),
        Err(error) => Some(Err(error)),
    });
    let invocations = Arc::new(AtomicUsize::new(0));
    let service = configured_service(Arc::clone(&invocations), enforce_limits);
    let (shutdown, shutdown_receiver) = oneshot::channel();
    let task = tokio::spawn(async move {
        Server::builder()
            .add_service(service)
            .serve_with_incoming_shutdown(incoming, async {
                let _ = shutdown_receiver.await;
            })
            .await
    });
    RunningServer {
        socket,
        directory,
        shutdown: Some(shutdown),
        task,
        invocations,
    }
}

#[derive(Debug)]
struct UnidentifiedUnixStream(UnixStream);

impl Connected for UnidentifiedUnixStream {
    type ConnectInfo = ();
    fn connect_info(&self) -> Self::ConnectInfo {}
}

impl AsyncRead for UnidentifiedUnixStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_read(context, buffer)
    }
}

impl AsyncWrite for UnidentifiedUnixStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(context, buffer)
    }
    fn poll_flush(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(context)
    }
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(context)
    }
}

fn start_unidentified_server(expected_uid: u32) -> RunningServer {
    let (directory, socket, _) = test_socket("unidentified-v2-rpc");
    let listener = UnixListener::bind(&socket).expect("bind unidentified socket");
    let incoming =
        UnixListenerStream::new(listener).map(|result| result.map(UnidentifiedUnixStream));
    let invocations = Arc::new(AtomicUsize::new(0));
    let service = configured_service(Arc::clone(&invocations), true);
    let (shutdown, shutdown_receiver) = oneshot::channel();
    let task = tokio::spawn(async move {
        Server::builder()
            .layer(InterceptorLayer::new(SameUserInterceptor::new(
                expected_uid,
            )))
            .add_service(service)
            .serve_with_incoming_shutdown(incoming, async {
                let _ = shutdown_receiver.await;
            })
            .await
    });
    RunningServer {
        socket,
        directory,
        shutdown: Some(shutdown),
        task,
        invocations,
    }
}

async fn channel(socket: &Path) -> Channel {
    let socket = socket.to_owned();
    Endpoint::from_static("http://[::]:50051")
        .connect_with_connector(service_fn(move |_| {
            let socket = socket.clone();
            async move { UnixStream::connect(socket).await.map(TokioIo::new) }
        }))
        .await
        .expect("connect UDS channel")
}

fn configured_client(channel: Channel, enforce_limits: bool) -> CpgQueryServiceClient<Channel> {
    let client = CpgQueryServiceClient::new(channel);
    if enforce_limits {
        client
            .max_decoding_message_size(MAX_CONTROL_MESSAGE_BYTES)
            .max_encoding_message_size(MAX_CONTROL_MESSAGE_BYTES)
    } else {
        client
    }
}

fn authenticated_request<T>(message: T, session_token: &[u8]) -> Request<T> {
    let mut request = Request::new(message);
    request.metadata_mut().insert_bin(
        SESSION_METADATA_KEY,
        MetadataValue::from_bytes(session_token),
    );
    request
}

fn request_context(
    _authority: &AuthorityGeneration,
    correlation_id: impl Into<String>,
) -> RequestContext {
    RequestContext {
        correlation_id: correlation_id.into(),
        remaining_budget: Some(prost_types::Duration {
            seconds: 10,
            nanos: 0,
        }),
    }
}

fn production_handshake(launch_grant: [u8; 32], adapter_version: &str) -> HandshakeRequest {
    HandshakeRequest {
        launch_grant: launch_grant.to_vec(),
        adapter_version: adapter_version.to_owned(),
        minimum_minor: 0,
        maximum_minor: 0,
        desired_semantic_profiles: vec![INTEROP_SEMANTIC_PROFILE.to_owned()],
        maximum_resource_chunk_bytes: 64 * 1_024,
        remaining_budget: Some(prost_types::Duration {
            seconds: 10,
            nanos: 0,
        }),
        correlation_id: format!("{adapter_version}-handshake"),
        ..HandshakeRequest::default()
    }
}

fn production_submission(semantic_request_id: &str) -> QuerySubmission {
    let request = serde_json::json!({
        "specification": "composable semantic CPG fact query",
        "version": "2.0",
        "semantic_request_id": semantic_request_id,
        "scope": {"workspace_id": interop_workspace_public_id()},
        "freshness": {"policy": "best_available_snapshot"},
        "queries": [{
            "request": "find code entities",
            "query_id": "query-clause:interop",
            "looking_for": "syntax nodes",
            "within": [],
            "where": [],
            "return": {"limit": {"maximum_results": 3}}
        }]
    });
    let canonical_request_json =
        serde_json_canonicalizer::to_vec(&request).expect("canonical interop query");
    QuerySubmission {
        request_checksum: codefabric::integrity::framed_digest(&canonical_request_json),
        canonical_request_json,
        semantic_request_id: Some(semantic_request_id.to_owned()),
        semantic_profile: INTEROP_SEMANTIC_PROFILE.to_owned(),
        result_limits: Some(ResultLimits {
            maximum_result_bytes: 1 << 20,
            maximum_result_pages: 4,
        }),
    }
}

async fn start_production_query(
    client: &mut CpgQueryServiceClient<Channel>,
    authority: &AuthorityGeneration,
    session_token: &[u8],
    query: QuerySubmission,
    correlation_prefix: &str,
) -> AcceptedQuery {
    let initial = authenticated_request(
        StartQueryRequest {
            context: Some(request_context(
                authority,
                format!("{correlation_prefix}-initial"),
            )),
            leg: Some(Leg::Initial(InitialQueryStart { query: Some(query) })),
        },
        session_token,
    );
    let mut outcome = client
        .start_query(initial)
        .await
        .expect("production initial start")
        .into_inner()
        .outcome
        .expect("production initial outcome");
    for expected_round in 1..=3 {
        let Outcome::InputChallenge(challenge) = outcome else {
            panic!("expected guarded input round {expected_round}");
        };
        assert_eq!(challenge.round, expected_round);
        assert_eq!(challenge.requirements.len(), 1);
        let continuation = QueryChallengeContinuation {
            daemon_continuation: challenge.daemon_continuation,
            challenge_id: challenge.challenge_id,
            round: challenge.round,
            answers: vec![InputAnswer {
                semantic_field_id: format!("field:round-{expected_round}"),
                value: Some(WireInputValue::ChoiceId(format!(
                    "choice:round-{expected_round}"
                ))),
            }],
        };
        outcome = client
            .start_query(authenticated_request(
                StartQueryRequest {
                    context: Some(request_context(
                        authority,
                        format!("{correlation_prefix}-round-{expected_round}"),
                    )),
                    leg: Some(Leg::Continuation(continuation)),
                },
                session_token,
            ))
            .await
            .expect("production guarded continuation")
            .into_inner()
            .outcome
            .expect("production continuation outcome");
    }
    let Outcome::Accepted(accepted) = outcome else {
        panic!("third guarded round must admit the query");
    };
    assert!(matches!(
        QueryExecutionState::try_from(accepted.state),
        Ok(QueryExecutionState::Queued | QueryExecutionState::Running)
    ));
    accepted
}

async fn read_production_resource(
    client: &mut CpgQueryServiceClient<Channel>,
    authority: &AuthorityGeneration,
    session_token: &[u8],
    public_handle: &str,
    selector: WireResourceSelector,
    correlation_id: &str,
) -> Vec<u8> {
    let mut stream = client
        .read_resource(authenticated_request(
            ReadResourceRequest {
                context: Some(request_context(authority, correlation_id)),
                public_handle: public_handle.to_owned(),
                selector: Some(ResourceSelector {
                    selector: Some(selector),
                }),
                offset: 0,
                maximum_bytes: 7,
            },
            session_token,
        ))
        .await
        .expect("read production resource")
        .into_inner();
    let mut content = Vec::new();
    let mut expected_offset = 0_u64;
    let mut saw_end = false;
    while let Some(chunk) = stream.message().await.expect("resource stream message") {
        assert_eq!(chunk.public_handle, public_handle);
        assert_eq!(chunk.offset, expected_offset);
        expected_offset = expected_offset.saturating_add(chunk.content.len() as u64);
        saw_end = chunk.end_of_resource;
        content.extend_from_slice(&chunk.content);
    }
    assert!(saw_end, "resource stream must end explicitly");
    content
}

async fn release_production_resource(
    client: &mut CpgQueryServiceClient<Channel>,
    authority: &AuthorityGeneration,
    session_token: &[u8],
    public_handle: &str,
    release_id: &str,
) -> ReleaseResourceResponse {
    client
        .release_resource(authenticated_request(
            ReleaseResourceRequest {
                context: Some(request_context(authority, format!("release-{release_id}"))),
                public_handle: public_handle.to_owned(),
                release_id: release_id.to_owned(),
            },
            session_token,
        ))
        .await
        .expect("release production resource")
        .into_inner()
}

fn current_uid() -> u32 {
    fs::metadata(".").expect("cwd metadata").uid()
}

async fn successful_command(command: &mut Command, label: &str) -> Output {
    let output = tokio::time::timeout(Duration::from_secs(120), command.output())
        .await
        .unwrap_or_else(|_| panic!("{label} timeout"))
        .unwrap_or_else(|error| panic!("launch {label}: {error}"));
    assert!(
        output.status.success(),
        "{label} failed:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout[..output.stdout.len().min(16_384)]),
        String::from_utf8_lossy(&output.stderr[..output.stderr.len().min(16_384)])
    );
    output
}

async fn wait_for_path(path: &Path, label: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while !path.exists() {
        assert!(tokio::time::Instant::now() < deadline, "{label}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(encoded, "{byte:02x}").expect("write hex");
    }
    encoded
}

#[tokio::test(flavor = "multi_thread")]
async fn wp10_behavioral_acceptance() {
    let server = start_authenticated_server(current_uid(), true);
    let mut client = configured_client(channel(&server.socket).await, true);
    let response = client
        .handshake(HandshakeRequest {
            launch_grant: vec![0x31; 32],
            adapter_version: "rust-v2-client".to_owned(),
            desired_semantic_profiles: vec!["codefabric.semantic-query.v2".to_owned()],
            maximum_resource_chunk_bytes: 65_536,
            ..HandshakeRequest::default()
        })
        .await
        .expect("v2 handshake")
        .into_inner();
    assert_eq!(response.authority.expect("authority").daemon_generation, 1);
    assert_eq!(response.session_token, vec![0x41; 32]);
    assert_eq!(server.invocations.load(Ordering::SeqCst), 1);
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn wp45_rust_tonic_uds_accepts_generated_rust_client() {
    let launch_grant = [0x31; 32];
    let server = start_production_server("wp45-rust-production", launch_grant).await;
    let (reference, reference_content) = server.control.seed_reference().await;
    let mut client = configured_client(channel(&server.socket).await, true);
    let handshake = client
        .handshake(production_handshake(
            launch_grant,
            "rust-generated-production-v2-client",
        ))
        .await
        .expect("generated Rust client handshake")
        .into_inner();
    let authority = handshake.authority.clone().expect("handshake authority");
    assert_eq!(authority.daemon_generation, INTEROP_DAEMON_GENERATION);
    assert_eq!(authority.supervisor_generation, 9);
    assert_eq!(authority.policy_generation, 8);
    assert_eq!(authority.revocation_generation, 9);
    assert_eq!(handshake.selected_minor, 0);
    assert_eq!(
        handshake.selected_semantic_profile,
        INTEROP_SEMANTIC_PROFILE
    );
    assert_eq!(handshake.session_token.len(), 32);
    assert_eq!(
        handshake
            .effective_limits
            .as_ref()
            .expect("effective limits")
            .maximum_challenge_rounds,
        3
    );

    let completion = client
        .get_reference(authenticated_request(
            GetReferenceRequest {
                context: Some(request_context(&authority, "rust-completion-deny")),
                operation: Some(ReferenceOperation::Completion(ReferenceCompletionRequest {
                    variable: ReferenceTemplateVariable::Kind as i32,
                    prefix: String::new(),
                    kind: None,
                    selector: Some("authorization-obscured-selector".to_owned()),
                    maximum_candidates: 16,
                })),
            },
            &handshake.session_token,
        ))
        .await
        .expect("bootstrapping completion denial")
        .into_inner();
    let Some(ReferenceResult::Completion(completion)) = completion.result else {
        panic!("expected typed reference completion");
    };
    assert!(completion.candidates.is_empty());
    assert_eq!(completion.total, 0);
    assert!(!completion.has_more);

    let reference_denial = client
        .get_reference(authenticated_request(
            GetReferenceRequest {
                context: Some(request_context(&authority, "rust-reference-deny")),
                operation: Some(ReferenceOperation::Read(ReferenceReadRequest {
                    kind: ReferenceKind::Guide as i32,
                    version: Some("2.3".to_owned()),
                })),
            },
            &handshake.session_token,
        ))
        .await
        .expect_err("missing live reference authority must fail closed");
    assert_eq!(reference_denial.code(), tonic::Code::PermissionDenied);

    let submission = production_submission("request:rust-production-result");
    let validation = client
        .validate_query(authenticated_request(
            ValidateQueryRequest {
                context: Some(request_context(&authority, "rust-pure-validation")),
                query: Some(submission.clone()),
            },
            &handshake.session_token,
        ))
        .await
        .expect("pure production validation")
        .into_inner()
        .preparation
        .expect("validation preparation");
    assert!(validation.errors.is_empty());
    assert_eq!(
        validation.semantic_request_id,
        "request:rust-production-result"
    );
    assert_eq!(validation.input_requirements.len(), 1);
    assert_eq!(
        validation.input_requirements[0].semantic_field_id,
        "field:round-1"
    );

    server.control.mark_ready();
    let accepted = start_production_query(
        &mut client,
        &authority,
        &handshake.session_token,
        submission,
        "rust-production-result",
    )
    .await;
    let query_id = accepted.daemon_query_id.clone();
    let observed = server.control.wait_for_query_count(1).await;
    assert_eq!(observed, vec![query_id.clone()]);
    let registration = server.control.publish_result(&query_id).await;

    let mut watch = client
        .watch_query(authenticated_request(
            WatchQueryRequest {
                context: Some(request_context(&authority, "rust-watch-result")),
                daemon_query_id: query_id.clone(),
                cursor: None,
            },
            &handshake.session_token,
        ))
        .await
        .expect("watch production result")
        .into_inner();
    let mut saw_snapshot = false;
    let mut saw_progress = false;
    let mut result_ready = None;
    let mut terminal_state = None;
    while let Some(event) = watch.message().await.expect("watch event") {
        match event.event.expect("typed watch event") {
            WireQueryEvent::SnapshotPinned(_) => saw_snapshot = true,
            WireQueryEvent::Progress(_) => saw_progress = true,
            WireQueryEvent::ResultReady(ready) => result_ready = Some(ready),
            WireQueryEvent::Terminal(terminal) => {
                terminal_state = Some(terminal.state);
                break;
            }
        }
    }
    assert!(saw_snapshot);
    assert!(saw_progress);
    assert_eq!(terminal_state, Some(QueryExecutionState::Succeeded as i32));
    let ready = result_ready.expect("ResultReady event");
    assert_eq!(ready.package_id, registration.package_id);
    assert_eq!(ready.total_rows, 3);
    assert_eq!(ready.total_pages, registration.total_pages);
    let manifest = ready.manifest.expect("public manifest descriptor");
    assert_eq!(manifest.public_handle, registration.manifest.public_handle);
    assert_eq!(ready.pages.len(), registration.pages.len());

    let manifest_bytes = read_production_resource(
        &mut client,
        &authority,
        &handshake.session_token,
        &manifest.public_handle,
        WireResourceSelector::Manifest(ManifestSelector {}),
        "rust-read-manifest",
    )
    .await;
    let public_manifest: serde_json::Value =
        serde_json::from_slice(&manifest_bytes).expect("public manifest JSON");
    let public_manifest_text = public_manifest.to_string();
    assert!(!public_manifest_text.contains("packages/"));
    assert!(!public_manifest_text.contains("object_store"));

    for page in &ready.pages {
        let page_bytes = read_production_resource(
            &mut client,
            &authority,
            &handshake.session_token,
            &page.public_handle,
            WireResourceSelector::Page(PageSelector {
                page_ordinal: page.page_ordinal.expect("page ordinal"),
            }),
            "rust-read-page",
        )
        .await;
        let batches =
            arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(page_bytes), None)
                .expect("Arrow IPC stream")
                .collect::<Result<Vec<_>, _>>()
                .expect("Arrow IPC batches");
        assert_eq!(
            batches
                .iter()
                .map(arrow::record_batch::RecordBatch::num_rows)
                .sum::<usize>(),
            3
        );
    }

    let reference_bytes = read_production_resource(
        &mut client,
        &authority,
        &handshake.session_token,
        &reference.public_handle,
        WireResourceSelector::Reference(ReferenceReadRequest {
            kind: ReferenceKind::Guide as i32,
            version: Some("2.3".to_owned()),
        }),
        "rust-read-reference",
    )
    .await;
    assert_eq!(reference_bytes, reference_content);
    let released_reference = release_production_resource(
        &mut client,
        &authority,
        &handshake.session_token,
        &reference.public_handle,
        "release:rust-reference",
    )
    .await;
    assert_eq!(released_reference.state, ReleaseState::Released as i32);
    let replayed_reference = release_production_resource(
        &mut client,
        &authority,
        &handshake.session_token,
        &reference.public_handle,
        "release:rust-reference",
    )
    .await;
    assert_eq!(
        replayed_reference.state,
        ReleaseState::AlreadyReleased as i32
    );
    assert!(replayed_reference.idempotent_replay);

    for (index, page) in ready.pages.iter().enumerate() {
        let released = release_production_resource(
            &mut client,
            &authority,
            &handshake.session_token,
            &page.public_handle,
            &format!("release:rust-page-{index}"),
        )
        .await;
        assert_eq!(released.state, ReleaseState::Released as i32);
    }
    let released_manifest = release_production_resource(
        &mut client,
        &authority,
        &handshake.session_token,
        &manifest.public_handle,
        "release:rust-manifest",
    )
    .await;
    assert_eq!(released_manifest.state, ReleaseState::Released as i32);

    let cancellation_submission = production_submission("request:rust-production-cancel");
    let cancelled = start_production_query(
        &mut client,
        &authority,
        &handshake.session_token,
        cancellation_submission,
        "rust-production-cancel",
    )
    .await;
    let observed = server.control.wait_for_query_count(2).await;
    assert_eq!(observed[1], cancelled.daemon_query_id);
    let cancellation_id = "cancel:rust-production";
    let cancel_request = || {
        authenticated_request(
            CancelQueryRequest {
                context: Some(request_context(&authority, "rust-cancel")),
                daemon_query_id: cancelled.daemon_query_id.clone(),
                cancellation_id: cancellation_id.to_owned(),
            },
            &handshake.session_token,
        )
    };
    let first_cancel = client
        .cancel_query(cancel_request())
        .await
        .expect("cancel production query")
        .into_inner();
    assert_eq!(
        first_cancel.acknowledgement,
        CancellationAcknowledgement::Accepted as i32
    );
    let replay_cancel = client
        .cancel_query(cancel_request())
        .await
        .expect("replay production cancellation")
        .into_inner();
    assert_eq!(
        replay_cancel.acknowledgement,
        CancellationAcknowledgement::Replayed as i32
    );
    assert!(replay_cancel.idempotent_replay);

    let mut cancelled_watch = client
        .watch_query(authenticated_request(
            WatchQueryRequest {
                context: Some(request_context(&authority, "rust-watch-cancelled")),
                daemon_query_id: cancelled.daemon_query_id,
                cursor: None,
            },
            &handshake.session_token,
        ))
        .await
        .expect("watch cancelled production query")
        .into_inner();
    let cancelled_terminal = loop {
        let event = cancelled_watch
            .message()
            .await
            .expect("cancel watch event")
            .expect("cancel watch terminal");
        if let Some(WireQueryEvent::Terminal(terminal)) = event.event {
            break terminal;
        }
    };
    assert_eq!(
        cancelled_terminal.state,
        QueryExecutionState::Cancelled as i32
    );
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "cross-domain oracle; builds and installs the pinned adapter wheel"]
async fn wp45_rust_tonic_uds_accepts_generated_python_client() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = tempfile::Builder::new()
        .prefix("cf-wp45-python-production-")
        .tempdir_in("/tmp")
        .expect("create installed-wheel oracle directory");
    let dist = directory.path().join("dist");
    successful_command(
        Command::new("uv")
            .arg("build")
            .arg("--project")
            .arg(repository.join("codefabric-cpg-mcp"))
            .arg("--wheel")
            .arg("--out-dir")
            .arg(&dist),
        "build adapter wheel",
    )
    .await;
    let wheels = fs::read_dir(&dist)
        .expect("read wheel directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "whl"))
        .collect::<Vec<_>>();
    assert_eq!(wheels.len(), 1, "expected exactly one adapter wheel");
    let venv = directory.path().join("venv");
    successful_command(
        Command::new("uv")
            .arg("venv")
            .arg("--python")
            .arg("3.14")
            .arg(&venv),
        "create isolated adapter venv",
    )
    .await;
    let python = venv.join("bin/python");
    successful_command(
        Command::new("uv")
            .arg("pip")
            .arg("install")
            .arg("--python")
            .arg(&python)
            .arg(&wheels[0]),
        "install adapter wheel",
    )
    .await;

    let socket = directory.path().join("cpgd.sock");
    let reference_info = directory.path().join("reference.json");
    let ready_request = directory.path().join("ready.request");
    let ready_ack = directory.path().join("ready.ack");
    let launch_grant = [0x32; 32];
    let script = repository.join("tests/integration/wp45_generated_python_client.py");
    let child = Command::new(&python)
        .arg(&script)
        .arg(&socket)
        .arg(hex_bytes(&launch_grant))
        .arg(interop_workspace_public_id())
        .arg(&reference_info)
        .arg(&ready_request)
        .arg(&ready_ack)
        .current_dir(directory.path())
        .env_remove("PYTHONPATH")
        .env("FASTMCP_MCP_CAMELCASE_COMPAT", "false")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn installed-wheel generated Python client");
    let child_pid = child.id().expect("installed-wheel client PID");
    let server = start_production_server_in(directory, launch_grant, child_pid).await;
    let (reference, reference_content) = server.control.seed_reference().await;
    let reference_json = serde_json_canonicalizer::to_vec(&serde_json::json!({
        "public_handle": reference.public_handle,
        "content_hex": hex_bytes(&reference_content),
    }))
    .expect("reference handoff JSON");
    fs::write(&reference_info, reference_json).expect("write reference handoff");

    wait_for_path(
        &ready_request,
        "Python client did not complete bootstrapping checks",
    )
    .await;
    server.control.mark_ready();
    fs::write(&ready_ack, b"ready\n").expect("acknowledge Ready lifecycle");
    let observed = server.control.wait_for_query_count(1).await;
    server.control.publish_result(&observed[0]).await;

    let output = tokio::time::timeout(Duration::from_secs(60), child.wait_with_output())
        .await
        .expect("installed-wheel generated Python client timeout")
        .expect("join installed-wheel generated Python client");
    assert!(
        output.status.success(),
        "installed-wheel generated Python client failed:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout[..output.stdout.len().min(16_384)]),
        String::from_utf8_lossy(&output.stderr[..output.stderr.len().min(16_384)])
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "python-installed-wheel-generated-production-client-ok"
    );
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn rust_client_deadline_cancels_a_slow_rpc() {
    let server = start_authenticated_server(current_uid(), true);
    let mut client = configured_client(channel(&server.socket).await, true);
    let mut request = Request::new(HandshakeRequest {
        adapter_version: "delay".to_owned(),
        ..HandshakeRequest::default()
    });
    request.set_timeout(Duration::from_millis(10));
    let status = client.handshake(request).await.expect_err("deadline");
    assert!(matches!(
        status.code(),
        tonic::Code::Cancelled | tonic::Code::DeadlineExceeded
    ));
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_or_mismatched_identity_is_rejected_before_handler_dispatch() {
    let uid = current_uid();
    let missing = start_unidentified_server(uid);
    let mut client = configured_client(channel(&missing.socket).await, true);
    let status = client
        .handshake(HandshakeRequest::default())
        .await
        .expect_err("missing peer extension");
    assert_eq!(status.code(), tonic::Code::Unauthenticated);
    assert_eq!(missing.invocations.load(Ordering::SeqCst), 0);
    missing.stop().await;

    let mismatched = start_authenticated_server(uid.wrapping_add(1), true);
    let result = Endpoint::from_static("http://[::]:50051")
        .connect_with_connector(service_fn({
            let socket = mismatched.socket.clone();
            move |_| {
                let socket = socket.clone();
                async move { UnixStream::connect(socket).await.map(TokioIo::new) }
            }
        }))
        .await;
    if let Ok(channel) = result {
        let mut client = configured_client(channel, true);
        client
            .handshake(HandshakeRequest::default())
            .await
            .expect_err("wrong UID must fail before handler dispatch");
    }
    assert_eq!(mismatched.invocations.load(Ordering::SeqCst), 0);
    mismatched.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn rust_client_and_server_apply_symmetric_four_mib_limits() {
    let oversized = MAX_CONTROL_MESSAGE_BYTES + 1;
    let server = start_authenticated_server(current_uid(), true);
    let mut client = configured_client(channel(&server.socket).await, false);
    let status = client
        .handshake(HandshakeRequest {
            adapter_version: "x".repeat(oversized),
            ..HandshakeRequest::default()
        })
        .await
        .expect_err("server decode limit");
    assert_eq!(status.code(), tonic::Code::OutOfRange);
    assert_eq!(server.invocations.load(Ordering::SeqCst), 0);
    server.stop().await;

    let server = start_authenticated_server(current_uid(), true);
    let mut client = configured_client(channel(&server.socket).await, false);
    let status = client
        .handshake(HandshakeRequest {
            adapter_version: "large-response".to_owned(),
            ..HandshakeRequest::default()
        })
        .await
        .expect_err("server encode limit");
    assert_eq!(status.code(), tonic::Code::OutOfRange);
    server.stop().await;
}

#[test]
fn wp10_structural_acceptance() {
    use codefabric::rpc::generated::codefabric::cpgd::v2::query_event::Event;
    use codefabric::rpc::generated::codefabric::cpgd::v2::{
        ProgressEvent, ResultReadyEvent, SnapshotPinnedEvent, TerminalEvent,
    };
    let variants = [
        Event::SnapshotPinned(SnapshotPinnedEvent::default()),
        Event::Progress(ProgressEvent::default()),
        Event::ResultReady(ResultReadyEvent::default()),
        Event::Terminal(TerminalEvent::default()),
    ];
    assert_eq!(variants.len(), 4);
    assert_eq!(ReferenceKind::Snapshot as i32, 6);
    let outcomes = [
        Outcome::Accepted(AcceptedQuery::default()),
        Outcome::InputChallenge(Default::default()),
        Outcome::ValidationRejection(Default::default()),
    ];
    assert_eq!(outcomes.len(), 3);
}

#[test]
fn wp10_operational_acceptance() {
    fn round_trip<M>(message: &M)
    where
        M: Message + Default + PartialEq + std::fmt::Debug,
    {
        let bytes = message.encode_to_vec();
        assert_eq!(&M::decode(bytes.as_slice()).expect("decode"), message);
    }
    round_trip(&StartQueryRequest {
        context: None,
        leg: Some(Leg::Initial(InitialQueryStart {
            query: Some(QuerySubmission {
                canonical_request_json: br#"{"kind":"lookup"}"#.to_vec(),
                request_checksum: "b3:test".to_owned(),
                semantic_request_id: Some("request:test".to_owned()),
                semantic_profile: "codefabric.semantic-query.v2".to_owned(),
                result_limits: Some(ResultLimits {
                    maximum_result_bytes: 1_048_576,
                    maximum_result_pages: 16,
                }),
            }),
        })),
    });
    round_trip(&ProviderJobSpec {
        provider_run_id: "run:test".to_owned(),
        workspace_id: "ws:test".to_owned(),
        analysis_context_id: "context:source".to_owned(),
        source_generation: 7,
        resource_profile_id: "in-process-syntax-standard".to_owned(),
        ..ProviderJobSpec::default()
    });
    round_trip(&Hello {
        protocol_major: 1,
        maximum_arrow_ipc_bytes: 1_048_576,
        ..Hello::default()
    });
    round_trip(&CompilationAccepted {
        provider_run_id: "run:test".to_owned(),
        compilation_unit_id: "unit:test".to_owned(),
        accepted_generation: 7,
        ..CompilationAccepted::default()
    });
}
