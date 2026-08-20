use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::{SystemTime, UNIX_EPOCH};

use codefabric::rpc::generated::ProbeEnvelope;
use codefabric::rpc::generated::wave_zero_probe_client::WaveZeroProbeClient;
use codefabric::rpc::generated::wave_zero_probe_server::{WaveZeroProbe, WaveZeroProbeServer};
use codefabric::rpc::{AuthorizedUnixStream, MAX_CONTROL_MESSAGE_BYTES, SameUserInterceptor};
use hyper_util::rt::TokioIo;
use prost::Message;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::service::InterceptorLayer;
use tonic::transport::server::Connected;
use tonic::transport::{Channel, Endpoint, Server};
use tonic::{Request, Response, Status};
use tower::service_fn;

#[derive(Clone)]
struct ProbeService {
    invocations: Arc<AtomicUsize>,
}

#[tonic::async_trait]
impl WaveZeroProbe for ProbeService {
    async fn round_trip(
        &self,
        request: Request<ProbeEnvelope>,
    ) -> Result<Response<ProbeEnvelope>, Status> {
        self.invocations.fetch_add(1, Ordering::SeqCst);
        let message = request.into_inner();
        let response_size = usize::try_from(message.response_bytes)
            .map_err(|_| Status::invalid_argument("response size is not representable"))?;
        let payload = if response_size == 0 {
            message.payload
        } else {
            vec![b'x'; response_size]
        };
        Ok(Response::new(ProbeEnvelope {
            payload,
            response_bytes: 0,
        }))
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
) -> WaveZeroProbeServer<ProbeService> {
    let service = WaveZeroProbeServer::new(ProbeService { invocations });
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

fn configured_client(channel: Channel, enforce_limits: bool) -> WaveZeroProbeClient<Channel> {
    let client = WaveZeroProbeClient::new(channel);
    if enforce_limits {
        client
            .max_decoding_message_size(MAX_CONTROL_MESSAGE_BYTES)
            .max_encoding_message_size(MAX_CONTROL_MESSAGE_BYTES)
    } else {
        client
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn authenticated_uds_round_trip_propagates_peer_identity() {
    let uid = current_uid();
    let server = start_authenticated_server(uid, true);
    let mut client = configured_client(channel(&server.socket).await, true);
    let response = client
        .round_trip(ProbeEnvelope {
            payload: b"codefabric".to_vec(),
            response_bytes: 0,
        })
        .await
        .expect("same-UID request")
        .into_inner();
    assert_eq!(response.payload, b"codefabric");
    assert_eq!(server.invocations.load(Ordering::SeqCst), 1);
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_or_mismatched_identity_is_rejected_before_handler_dispatch() {
    let uid = current_uid();

    let missing = start_unidentified_server(uid);
    let mut missing_client = configured_client(channel(&missing.socket).await, true);
    let missing_status = missing_client
        .round_trip(ProbeEnvelope::default())
        .await
        .expect_err("missing identity must fail");
    assert_eq!(missing_status.code(), tonic::Code::Unauthenticated);
    assert_eq!(missing.invocations.load(Ordering::SeqCst), 0);
    missing.stop().await;

    let mismatched = start_authenticated_server(uid.wrapping_add(1), true);
    let mut mismatched_client = configured_client(channel(&mismatched.socket).await, true);
    let mismatch_status = mismatched_client
        .round_trip(ProbeEnvelope::default())
        .await
        .expect_err("different configured UID must fail");
    assert_eq!(mismatch_status.code(), tonic::Code::PermissionDenied);
    assert_eq!(mismatched.invocations.load(Ordering::SeqCst), 0);
    mismatched.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn rust_client_and_server_apply_symmetric_four_mib_limits() {
    let uid = current_uid();
    let oversized = MAX_CONTROL_MESSAGE_BYTES + 1;

    let server_decode = start_authenticated_server(uid, true);
    let mut unbounded_client = configured_client(channel(&server_decode.socket).await, false);
    let server_decode_status = unbounded_client
        .round_trip(ProbeEnvelope {
            payload: vec![0; oversized],
            response_bytes: 0,
        })
        .await
        .expect_err("server decode limit");
    assert_eq!(server_decode_status.code(), tonic::Code::OutOfRange);
    assert_eq!(server_decode.invocations.load(Ordering::SeqCst), 0);
    server_decode.stop().await;

    let client_encode = start_authenticated_server(uid, false);
    let mut limited_client = configured_client(channel(&client_encode.socket).await, true);
    let client_encode_status = limited_client
        .round_trip(ProbeEnvelope {
            payload: vec![0; oversized],
            response_bytes: 0,
        })
        .await
        .expect_err("client encode limit");
    assert_eq!(client_encode_status.code(), tonic::Code::Internal);
    assert_eq!(client_encode.invocations.load(Ordering::SeqCst), 0);
    client_encode.stop().await;

    let server_encode = start_authenticated_server(uid, true);
    let mut unbounded_client = configured_client(channel(&server_encode.socket).await, false);
    let server_encode_status = unbounded_client
        .round_trip(ProbeEnvelope {
            payload: Vec::new(),
            response_bytes: u32::try_from(oversized).expect("response size"),
        })
        .await
        .expect_err("server encode limit");
    assert_eq!(server_encode_status.code(), tonic::Code::OutOfRange);
    assert_eq!(server_encode.invocations.load(Ordering::SeqCst), 1);
    server_encode.stop().await;

    let client_decode = start_authenticated_server(uid, false);
    let mut limited_client = configured_client(channel(&client_decode.socket).await, true);
    let client_decode_status = limited_client
        .round_trip(ProbeEnvelope {
            payload: Vec::new(),
            response_bytes: u32::try_from(oversized).expect("response size"),
        })
        .await
        .expect_err("client decode limit");
    assert_eq!(client_decode_status.code(), tonic::Code::OutOfRange);
    assert_eq!(client_decode.invocations.load(Ordering::SeqCst), 1);
    client_decode.stop().await;
}

#[test]
fn rust_protobuf_matches_the_shared_wire_fixture() {
    let fixture_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("contracts/fixtures/proto/wave0_probe.json");
    let fixture: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture_path).expect("read fixture")).expect("JSON");
    let payload = fixture["payload_utf8"].as_str().expect("fixture payload");
    let expected = fixture["wire_hex"].as_str().expect("fixture wire bytes");
    let encoded = ProbeEnvelope {
        payload: payload.as_bytes().to_vec(),
        response_bytes: 0,
    }
    .encode_to_vec();
    assert_eq!(hex(&encoded), expected);
    assert_eq!(
        ProbeEnvelope::decode(encoded.as_slice())
            .expect("decode")
            .payload,
        payload.as_bytes()
    );
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(char::from(HEX[usize::from(byte >> 4)]));
        result.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    result
}
