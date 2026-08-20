//! Local gRPC transport primitives and generated Wave 0 compatibility contract.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::UnixStream;
use tonic::service::Interceptor;
use tonic::transport::server::Connected;
use tonic::{Request, Status};

/// Maximum encoded and decoded control-message size on both client and server.
pub const MAX_CONTROL_MESSAGE_BYTES: usize = 4 * 1024 * 1024;

/// Peer identity captured from the accepted Unix socket before gRPC dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedPeerIdentity {
    uid: u32,
    gid: u32,
    pid: Option<u32>,
}

impl VerifiedPeerIdentity {
    /// Effective user ID supplied by the operating system.
    #[must_use]
    pub const fn uid(self) -> u32 {
        self.uid
    }

    /// Effective group ID supplied by the operating system.
    #[must_use]
    pub const fn gid(self) -> u32 {
        self.gid
    }

    /// Peer process ID when the platform supplies one.
    #[must_use]
    pub const fn pid(self) -> Option<u32> {
        self.pid
    }
}

/// Unix stream accepted only after its kernel-supplied peer UID matches policy.
#[derive(Debug)]
pub struct AuthorizedUnixStream {
    inner: UnixStream,
    identity: VerifiedPeerIdentity,
}

impl AuthorizedUnixStream {
    /// Authenticate an accepted stream and capture request-extension metadata.
    ///
    /// # Errors
    ///
    /// Returns the peer-credential syscall error or `PermissionDenied` for a UID
    /// outside the configured local-service policy.
    pub fn authenticate(inner: UnixStream, allowed_uid: u32) -> io::Result<Self> {
        let credentials = inner.peer_cred()?;
        if credentials.uid() != allowed_uid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Unix peer UID is not authorized",
            ));
        }
        Ok(Self {
            inner,
            identity: VerifiedPeerIdentity {
                uid: credentials.uid(),
                gid: credentials.gid(),
                pid: credentials.pid().and_then(|pid| u32::try_from(pid).ok()),
            },
        })
    }
}

impl Connected for AuthorizedUnixStream {
    type ConnectInfo = VerifiedPeerIdentity;

    fn connect_info(&self) -> Self::ConnectInfo {
        self.identity
    }
}

impl AsyncRead for AuthorizedUnixStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(context, buffer)
    }
}

impl AsyncWrite for AuthorizedUnixStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.inner).poll_write(context, buffer)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.inner).poll_flush(context)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.inner).poll_shutdown(context)
    }
}

/// Pre-dispatch interceptor requiring tonic's connection metadata to match one UID.
#[derive(Clone, Copy, Debug)]
pub struct SameUserInterceptor {
    expected_uid: u32,
}

impl SameUserInterceptor {
    /// Construct a same-user policy for the daemon owner's effective UID.
    #[must_use]
    pub const fn new(expected_uid: u32) -> Self {
        Self { expected_uid }
    }
}

impl Interceptor for SameUserInterceptor {
    fn call(&mut self, request: Request<()>) -> Result<Request<()>, Status> {
        let identity = request
            .extensions()
            .get::<VerifiedPeerIdentity>()
            .ok_or_else(|| Status::unauthenticated("Unix peer identity is missing"))?;
        if identity.uid != self.expected_uid {
            return Err(Status::permission_denied("Unix peer UID is not authorized"));
        }
        Ok(request)
    }
}

/// Generated Wave 0 probe types and client/server surfaces.
#[allow(clippy::all, clippy::pedantic)]
pub mod generated {
    include!("generated/codefabric.wave0.v1.rs");
}
