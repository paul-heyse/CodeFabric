use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use codefabric::rpc::generated::codefabric::cpgd::v1::cpg_query_service_client::CpgQueryServiceClient;
use codefabric::rpc::generated::codefabric::cpgd::v1::cpg_query_service_server::{
    CpgQueryService, CpgQueryServiceServer,
};
use codefabric::rpc::generated::codefabric::cpgd::v1::{
    ArtifactReadyEvent, AttachQueryRequest, CancelQueryRequest, CancelQueryResponse,
    HandshakeRequest, HandshakeResponse, ProgressEvent, QueryEvent, ReadResultRequest,
    ReleaseResultRequest, ReleaseResultResponse, ResponseChunkEvent, ResultChunk,
    SnapshotPinnedEvent, StartQueryRequest, StartQueryResponse, StatusRequest, StatusResponse,
    StreamQueryRequest, TerminalEvent, ValidateQueryRequest, ValidateQueryResponse,
};
use codefabric::rpc::generated::codefabric::provider::v1::ProviderJobSpec;
use codefabric::rpc::generated::codefabric::pyrefly::v1::Hello;
use codefabric::rpc::generated::codefabric::rustc::v1::CompilationAccepted;
use codefabric::rpc::{
    AuthorizedUnixStream, MAX_CONTROL_MESSAGE_BYTES, SameUserInterceptor, negotiate_feature_bits,
};
use hyper_util::rt::TokioIo;
use prost::Message;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_stream::Stream;
use tokio_stream::StreamExt as _;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::service::InterceptorLayer;
use tonic::transport::server::Connected;
use tonic::transport::{Channel, Endpoint, Server};
use tonic::{Request, Response, Status};
use tower::service_fn;

const SUPPORTED_FEATURES: u64 = 0b1111;
type QueryStream = Pin<Box<dyn Stream<Item = Result<QueryEvent, Status>> + Send>>;
type ResultStream = Pin<Box<dyn Stream<Item = Result<ResultChunk, Status>> + Send>>;

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
        let negotiated = negotiate_feature_bits(
            request.required_feature_bits,
            request.optional_feature_bits,
            SUPPORTED_FEATURES,
        )?;
        if request.adapter_version == "delay" {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        let daemon_version = if request.adapter_version == "large-response" {
            "x".repeat(MAX_CONTROL_MESSAGE_BYTES + 1)
        } else {
            request.adapter_version
        };
        Ok(Response::new(HandshakeResponse {
            daemon_version,
            negotiated_feature_bits: negotiated,
            negotiated_rpc_version: "1.0".to_owned(),
            negotiated_semantic_query_version: "1.3".to_owned(),
            ..HandshakeResponse::default()
        }))
    }

    async fn get_status(
        &self,
        _request: Request<StatusRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        Err(Status::unimplemented("test probe"))
    }

    async fn validate_query(
        &self,
        _request: Request<ValidateQueryRequest>,
    ) -> Result<Response<ValidateQueryResponse>, Status> {
        Err(Status::unimplemented("test probe"))
    }

    async fn start_query(
        &self,
        _request: Request<StartQueryRequest>,
    ) -> Result<Response<StartQueryResponse>, Status> {
        Err(Status::unimplemented("test probe"))
    }

    type StreamQueryStream = QueryStream;

    async fn stream_query(
        &self,
        _request: Request<StreamQueryRequest>,
    ) -> Result<Response<Self::StreamQueryStream>, Status> {
        Ok(Response::new(Box::pin(tokio_stream::empty())))
    }

    type AttachQueryStream = QueryStream;

    async fn attach_query(
        &self,
        _request: Request<AttachQueryRequest>,
    ) -> Result<Response<Self::AttachQueryStream>, Status> {
        Ok(Response::new(Box::pin(tokio_stream::empty())))
    }

    async fn cancel_query(
        &self,
        _request: Request<CancelQueryRequest>,
    ) -> Result<Response<CancelQueryResponse>, Status> {
        Err(Status::unimplemented("test probe"))
    }

    type ReadResultStream = ResultStream;

    async fn read_result(
        &self,
        _request: Request<ReadResultRequest>,
    ) -> Result<Response<Self::ReadResultStream>, Status> {
        Ok(Response::new(Box::pin(tokio_stream::empty())))
    }

    async fn release_result(
        &self,
        _request: Request<ReleaseResultRequest>,
    ) -> Result<Response<ReleaseResultResponse>, Status> {
        Err(Status::unimplemented("test probe"))
    }
}

struct RunningServer {
    socket: PathBuf,
    directory: PathBuf,
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), tonic::transport::Error>>,
    invocations: Arc<AtomicUsize>,
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

fn current_uid() -> u32 {
    let (directory, _, uid) = test_socket("uid-probe");
    fs::remove_dir(directory).expect("remove UID probe directory");
    uid
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
    let (directory, socket, actual_uid) = test_socket("authenticated-rpc");
    let listener = UnixListener::bind(&socket).expect("bind authenticated socket");
    let incoming = UnixListenerStream::new(listener).map(move |result| {
        result.and_then(|stream| AuthorizedUnixStream::authenticate(stream, actual_uid))
    });
    let invocations = Arc::new(AtomicUsize::new(0));
    let service = configured_service(Arc::clone(&invocations), enforce_limits);
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

fn start_transport_probe_server() -> RunningServer {
    let (directory, socket, _) = test_socket("transport-probe-rpc");
    let listener = UnixListener::bind(&socket).expect("bind transport probe socket");
    let incoming = UnixListenerStream::new(listener);
    let invocations = Arc::new(AtomicUsize::new(0));
    let service = configured_service(Arc::clone(&invocations), true);
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
    let (directory, socket, _) = test_socket("unidentified-rpc");
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

#[tokio::test(flavor = "multi_thread")]
async fn wp10_behavioral_acceptance() {
    let server = start_transport_probe_server();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = r"
import asyncio
import sys
import grpc
from codefabric_cpg_mcp.daemon.channel import create_local_channel
from codefabric_cpg_mcp.daemon.generated import cpg_query_service_pb2 as pb
from codefabric_cpg_mcp.daemon.generated import cpg_query_service_pb2_grpc as rpc

async def main():
    async with create_local_channel(f'unix:{sys.argv[1]}') as channel:
        response = await rpc.CpgQueryServiceStub(channel).Handshake(
            pb.HandshakeRequest(adapter_version='python-client', required_feature_bits=1)
        )
        assert response.daemon_version == 'python-client'
        assert response.negotiated_feature_bits == 1

asyncio.run(main())
";
    let socket = server.socket.clone();
    let status = tokio::task::spawn_blocking(move || {
        Command::new("env")
            .args([
                "-u",
                "VIRTUAL_ENV",
                "-u",
                "UV_PROJECT_ENVIRONMENT",
                "uv",
                "run",
                "--frozen",
                "--project",
                "codefabric-cpg-mcp",
                "python",
                "-c",
                script,
            ])
            .arg(socket)
            .current_dir(root)
            .status()
            .expect("launch Python client")
    })
    .await
    .expect("Python client task");
    let invocations = server.invocations.load(Ordering::SeqCst);
    server.stop().await;
    assert!(status.success());
    assert_eq!(invocations, 1);
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
    let status = client
        .handshake(request)
        .await
        .expect_err("deadline must cancel the slow request");
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
    let mut missing_client = configured_client(channel(&missing.socket).await, true);
    let missing_status = missing_client
        .handshake(HandshakeRequest::default())
        .await
        .expect_err("missing identity must fail");
    assert_eq!(missing_status.code(), tonic::Code::Unauthenticated);
    assert_eq!(missing.invocations.load(Ordering::SeqCst), 0);
    missing.stop().await;

    let mismatched = start_authenticated_server(uid.wrapping_add(1), true);
    let mut mismatched_client = configured_client(channel(&mismatched.socket).await, true);
    let mismatch_status = mismatched_client
        .handshake(HandshakeRequest::default())
        .await
        .expect_err("different configured UID must fail");
    assert_eq!(mismatch_status.code(), tonic::Code::PermissionDenied);
    assert_eq!(mismatched.invocations.load(Ordering::SeqCst), 0);
    mismatched.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn rust_client_and_server_apply_symmetric_four_mib_limits() {
    let oversized = MAX_CONTROL_MESSAGE_BYTES + 1;

    let server_decode = start_authenticated_server(current_uid(), true);
    let mut unbounded_client = configured_client(channel(&server_decode.socket).await, false);
    let status = unbounded_client
        .handshake(HandshakeRequest {
            adapter_version: "x".repeat(oversized),
            ..HandshakeRequest::default()
        })
        .await
        .expect_err("server decode limit");
    assert_eq!(status.code(), tonic::Code::OutOfRange);
    assert_eq!(server_decode.invocations.load(Ordering::SeqCst), 0);
    server_decode.stop().await;

    let client_encode = start_authenticated_server(current_uid(), false);
    let mut limited_client = configured_client(channel(&client_encode.socket).await, true);
    let status = limited_client
        .handshake(HandshakeRequest {
            adapter_version: "x".repeat(oversized),
            ..HandshakeRequest::default()
        })
        .await
        .expect_err("client encode limit");
    assert_eq!(status.code(), tonic::Code::Internal);
    assert_eq!(client_encode.invocations.load(Ordering::SeqCst), 0);
    client_encode.stop().await;

    let server_encode = start_authenticated_server(current_uid(), true);
    let mut unbounded_client = configured_client(channel(&server_encode.socket).await, false);
    let status = unbounded_client
        .handshake(HandshakeRequest {
            adapter_version: "large-response".to_owned(),
            ..HandshakeRequest::default()
        })
        .await
        .expect_err("server encode limit");
    assert_eq!(status.code(), tonic::Code::OutOfRange);
    server_encode.stop().await;

    let client_decode = start_authenticated_server(current_uid(), false);
    let mut limited_client = configured_client(channel(&client_decode.socket).await, true);
    let status = limited_client
        .handshake(HandshakeRequest {
            adapter_version: "large-response".to_owned(),
            ..HandshakeRequest::default()
        })
        .await
        .expect_err("client decode limit");
    assert_eq!(status.code(), tonic::Code::OutOfRange);
    client_decode.stop().await;
}

#[test]
fn wp10_structural_acceptance() {
    use codefabric::rpc::generated::codefabric::cpgd::v1::query_event::Event;

    let variants = [
        Event::SnapshotPinned(SnapshotPinnedEvent::default()),
        Event::Progress(ProgressEvent::default()),
        Event::ResponseChunk(ResponseChunkEvent::default()),
        Event::ArtifactReady(ArtifactReadyEvent::default()),
        Event::Terminal(TerminalEvent::default()),
    ];
    assert_eq!(variants.len(), 5);
    assert_eq!(
        codefabric::rpc::generated::codefabric::cpgd::v1::FreshnessPolicy::RequireCurrentForTargets
            as i32,
        3
    );
}

#[test]
fn wp10_negative_zero_state() {
    let status = negotiate_feature_bits(1 << 63, 0, SUPPORTED_FEATURES)
        .expect_err("unknown required feature bit");
    assert_eq!(status.code(), tonic::Code::FailedPrecondition);
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
        agent_instance_id: "agent:test".to_owned(),
        workspace_id: "ws:test".to_owned(),
        semantic_query_version: "1.3".to_owned(),
        canonical_request_json: br#"{"kind":"lookup"}"#.to_vec(),
        request_checksum: "b3:test".to_owned(),
        idempotency_key: "idem:test".to_owned(),
        ..StartQueryRequest::default()
    });
    round_trip(&ProviderJobSpec {
        provider_run_id: "run:test".to_owned(),
        workspace_id: "ws:test".to_owned(),
        analysis_context_id: "context:source".to_owned(),
        source_generation: 7,
        resource_profile_id: "in-process-syntax-standard".to_owned(),
        ..ProviderJobSpec::default()
    });

    let pre_profile_wire = b"\x0a\x08run:test\x12\x07ws:test\x1a\x0econtext:source\x20\x07";
    let decoded = ProviderJobSpec::decode(pre_profile_wire.as_slice()).expect("old wire");
    assert!(decoded.resource_profile_id.is_empty());
    round_trip(&Hello {
        protocol_major: 1,
        maximum_arrow_chunk_bytes: 1_048_576,
        ..Hello::default()
    });
    round_trip(&CompilationAccepted {
        provider_run_id: "run:test".to_owned(),
        compilation_unit_id: "unit:test".to_owned(),
        accepted_generation: 7,
        ..CompilationAccepted::default()
    });
}
