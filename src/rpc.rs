//! Local gRPC transport primitives and generated production contracts.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::UnixStream;
use tonic::service::Interceptor;
use tonic::transport::server::Connected;
use tonic::{Request, Status};

use crate::registries::{CpgdFeatureMask, RustcFeatureMask};

/// Maximum encoded and decoded control-message size on both client and server.
pub const MAX_CONTROL_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
/// Maximum uncompressed query-event or result payload chunk.
pub const MAX_PAYLOAD_CHUNK_BYTES: usize = 1024 * 1024;

/// Wire-mask behavior required by the one registry-generated feature family type.
pub trait WireFeatureMask: Copy {
    fn from_wire(bits: u64) -> Self;
    fn bits(self) -> u64;
}

impl WireFeatureMask for CpgdFeatureMask {
    fn from_wire(bits: u64) -> Self {
        Self::from_wire(bits)
    }

    fn bits(self) -> u64 {
        self.bits()
    }
}

impl WireFeatureMask for RustcFeatureMask {
    fn from_wire(bits: u64) -> Self {
        Self::from_wire(bits)
    }

    fn bits(self) -> u64 {
        self.bits()
    }
}

/// Negotiate one typed feature family while enforcing both peers' required capabilities.
///
/// # Errors
///
/// Returns `FailedPrecondition` when the client requires an unsupported bit or omits a bit the
/// service registry marks required.
pub fn negotiate_feature_bits<M: WireFeatureMask>(
    client_required: M,
    client_optional: M,
    service_supported: M,
    service_required: M,
) -> Result<M, Status> {
    let requested = client_required.bits() | client_optional.bits();
    let unsupported = client_required.bits() & !service_supported.bits();
    if unsupported != 0 {
        return Err(Status::failed_precondition(format!(
            "unsupported required feature bits: {unsupported:#018x}"
        )));
    }
    let missing = service_required.bits() & !requested;
    if missing != 0 {
        return Err(Status::failed_precondition(format!(
            "client omitted service-required feature bits: {missing:#018x}"
        )));
    }
    Ok(M::from_wire(requested & service_supported.bits()))
}

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
        let identity = SameUserInterceptor::new(allowed_uid).authenticate_stream(&inner)?;
        Ok(Self { inner, identity })
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

    /// Apply the single local peer-UID policy to one accepted Unix stream.
    ///
    /// # Errors
    ///
    /// Returns the peer-credential syscall error or `PermissionDenied` for another UID.
    pub fn authenticate_stream(&self, stream: &UnixStream) -> io::Result<VerifiedPeerIdentity> {
        let credentials = stream.peer_cred()?;
        let identity = VerifiedPeerIdentity {
            uid: credentials.uid(),
            gid: credentials.gid(),
            pid: credentials.pid().and_then(|pid| u32::try_from(pid).ok()),
        };
        Self::authorize_identity(self.expected_uid, identity).map_err(|status| {
            io::Error::new(io::ErrorKind::PermissionDenied, status.message().to_owned())
        })?;
        Ok(identity)
    }

    fn authorize_identity(expected_uid: u32, identity: VerifiedPeerIdentity) -> Result<(), Status> {
        if identity.uid != expected_uid {
            return Err(Status::permission_denied("Unix peer UID is not authorized"));
        }
        Ok(())
    }
}

impl Interceptor for SameUserInterceptor {
    fn call(&mut self, request: Request<()>) -> Result<Request<()>, Status> {
        let identity = request
            .extensions()
            .get::<VerifiedPeerIdentity>()
            .ok_or_else(|| Status::unauthenticated("Unix peer identity is missing"))?;
        Self::authorize_identity(self.expected_uid, *identity)?;
        Ok(request)
    }
}

/// Generated types and client/server surfaces from the one production FDS.
#[allow(clippy::all, clippy::pedantic)]
pub mod generated {
    pub mod codefabric {
        pub mod cpgd {
            pub mod v1 {
                include!("generated/codefabric.cpgd.v1.rs");
            }
        }
        pub mod provider {
            pub mod v1 {
                include!("generated/codefabric.provider.v1.rs");
            }
        }
        pub mod pyrefly {
            pub mod v1 {
                include!("generated/codefabric.pyrefly.v1.rs");
            }
        }
        pub mod rustc {
            pub mod v1 {
                include!("generated/codefabric.rustc.v1.rs");
            }
        }
    }
}
