use std::io;

use crate::{caller::LocalCaller, Error, LocalIpcEndpoint};

/// The engine's side of the local IPC transport.
///
/// `LocalListener::bind` prepares and binds the socket/pipe with platform
/// security (directory permissions, peer-UID check on Unix; DACL on Windows),
/// handles stale/live endpoint probing, and produces a `Stream` of accepted
/// connections for `tonic::transport::Server::serve_with_incoming`.
///
/// Each accepted connection is wrapped in `LocalPeerStream`, which implements
/// `tonic::transport::server::Connected` with `ConnectInfo = LocalCaller` — the
/// single chokepoint through which per-caller identity flows into gRPC handlers.
pub struct LocalListener {
    inner: ListenerInner,
}

impl LocalListener {
    /// Bind `endpoint`. Sets up the socket/pipe, validates security properties,
    /// and probes for stale or live engines. Fails fast; no background setup.
    pub async fn bind(endpoint: &LocalIpcEndpoint) -> Result<Self, Error> {
        Self::bind_inner(endpoint).await
    }

    /// Returns a stream of accepted peer connections, suitable for
    /// `tonic::transport::Server::serve_with_incoming`. Each item carries a
    /// `LocalPeerStream` that exposes the peer's `LocalCaller` via `Connected`.
    pub fn incoming(self) -> impl tokio_stream::Stream<Item = Result<LocalPeerStream, io::Error>> {
        self.incoming_inner()
    }

    #[cfg(unix)]
    async fn bind_inner(endpoint: &LocalIpcEndpoint) -> Result<Self, Error> {
        use tokio::net::UnixListener;

        if let Some(parent) = endpoint.path().parent() {
            modula_platform::ipc_security::setup_socket_dir(parent)?;
        }
        crate::stale::handle_stale(endpoint).await?;
        let listener = UnixListener::bind(endpoint.path())?;
        modula_platform::ipc_security::secure_socket(endpoint.path())?;
        Ok(Self {
            inner: ListenerInner::Unix(listener),
        })
    }

    #[cfg(unix)]
    fn incoming_inner(
        self,
    ) -> impl tokio_stream::Stream<Item = Result<LocalPeerStream, io::Error>> {
        use async_stream::stream;

        let ListenerInner::Unix(listener) = self.inner;
        stream! {
            loop {
                match listener.accept().await {
                    Ok((stream, _addr)) => {
                        match modula_platform::ipc_security::check_peer_uid(&stream) {
                            Ok(uid) => yield Ok(LocalPeerStream::unix(stream, uid)),
                            Err(e) => {
                                tracing::warn!("IPC connection rejected: {e}");
                            }
                        }
                    }
                    Err(e) => yield Err(e),
                }
            }
        }
    }

    #[cfg(windows)]
    async fn bind_inner(endpoint: &LocalIpcEndpoint) -> Result<Self, Error> {
        crate::stale::handle_stale(endpoint).await?;
        let name = endpoint
            .path()
            .to_str()
            .ok_or_else(|| {
                Error::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "pipe name must be valid UTF-8",
                ))
            })?
            .to_owned();
        let first =
            modula_platform::pipe_security::create_first_pipe_instance(&name).map_err(Error::Io)?;
        Ok(Self {
            inner: ListenerInner::Pipe {
                name,
                current: first,
            },
        })
    }

    #[cfg(windows)]
    fn incoming_inner(
        self,
    ) -> impl tokio_stream::Stream<Item = Result<LocalPeerStream, io::Error>> {
        use async_stream::stream;

        let (name, mut current) = match self.inner {
            ListenerInner::Pipe { name, current } => (name, current),
        };
        stream! {
            loop {
                if let Err(e) = current.connect().await {
                    yield Err(e);
                    return;
                }
                let next = match modula_platform::pipe_security::create_next_pipe_instance(&name) {
                    Ok(s) => s,
                    Err(e) => {
                        yield Err(e);
                        return;
                    }
                };
                let connected = std::mem::replace(&mut current, next);
                yield Ok(LocalPeerStream::pipe(connected));
            }
        }
    }
}

#[cfg(unix)]
enum ListenerInner {
    Unix(tokio::net::UnixListener),
}

#[cfg(windows)]
enum ListenerInner {
    Pipe {
        name: String,
        current: tokio::net::windows::named_pipe::NamedPipeServer,
    },
}

// ─── LocalPeerStream ─────────────────────────────────────────────────────────

/// An accepted IPC connection, wrapping the platform stream with the peer's
/// `LocalCaller`. Implements `tonic::transport::server::Connected` so that
/// tonic injects `LocalCaller` into each request's extensions — the single
/// chokepoint handlers use to read caller identity.
pub struct LocalPeerStream {
    inner: PeerStreamInner,
    caller: LocalCaller,
}

impl LocalPeerStream {
    pub fn caller(&self) -> &LocalCaller {
        &self.caller
    }

    #[cfg(unix)]
    fn unix(stream: tokio::net::UnixStream, uid: u32) -> Self {
        Self {
            inner: PeerStreamInner::Unix(stream),
            caller: LocalCaller::new(uid),
        }
    }

    #[cfg(windows)]
    fn pipe(server: tokio::net::windows::named_pipe::NamedPipeServer) -> Self {
        Self {
            inner: PeerStreamInner::Pipe(server),
            caller: LocalCaller::new(),
        }
    }
}

#[cfg(unix)]
enum PeerStreamInner {
    Unix(tokio::net::UnixStream),
}

#[cfg(windows)]
enum PeerStreamInner {
    Pipe(tokio::net::windows::named_pipe::NamedPipeServer),
}

impl tonic::transport::server::Connected for LocalPeerStream {
    type ConnectInfo = LocalCaller;
    fn connect_info(&self) -> Self::ConnectInfo {
        self.caller.clone()
    }
}

#[cfg(unix)]
impl tokio::io::AsyncRead for LocalPeerStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        let PeerStreamInner::Unix(inner) = &mut self.get_mut().inner;
        std::pin::Pin::new(inner).poll_read(cx, buf)
    }
}

#[cfg(unix)]
impl tokio::io::AsyncWrite for LocalPeerStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        let PeerStreamInner::Unix(inner) = &mut self.get_mut().inner;
        std::pin::Pin::new(inner).poll_write(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        let PeerStreamInner::Unix(inner) = &mut self.get_mut().inner;
        std::pin::Pin::new(inner).poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        let PeerStreamInner::Unix(inner) = &mut self.get_mut().inner;
        std::pin::Pin::new(inner).poll_shutdown(cx)
    }
}

#[cfg(unix)]
impl Unpin for LocalPeerStream {}

#[cfg(windows)]
impl tokio::io::AsyncRead for LocalPeerStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        let PeerStreamInner::Pipe(inner) = &mut self.get_mut().inner;
        std::pin::Pin::new(inner).poll_read(cx, buf)
    }
}

#[cfg(windows)]
impl tokio::io::AsyncWrite for LocalPeerStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        let PeerStreamInner::Pipe(inner) = &mut self.get_mut().inner;
        std::pin::Pin::new(inner).poll_write(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        let PeerStreamInner::Pipe(inner) = &mut self.get_mut().inner;
        std::pin::Pin::new(inner).poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        let PeerStreamInner::Pipe(inner) = &mut self.get_mut().inner;
        std::pin::Pin::new(inner).poll_shutdown(cx)
    }
}

#[cfg(windows)]
impl Unpin for LocalPeerStream {}
