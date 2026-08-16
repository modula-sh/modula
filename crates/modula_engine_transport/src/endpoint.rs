use std::fmt;
use std::path::{Path, PathBuf};

use tonic::transport::{Channel, Endpoint};

use crate::Error;

/// A local IPC endpoint — the gRPC address the engine binds and clients dial.
/// On Unix this is a Unix-domain socket path; on Windows a named-pipe name.
/// Construction is per-call-site; the type is never a process-global singleton.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalIpcEndpoint {
    path: PathBuf,
}

impl LocalIpcEndpoint {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Resolve the local IPC endpoint with the standard override order:
    /// 1. `socket_path` — explicit `--socket PATH` flag
    /// 2. `MODULA_ENGINE_SOCKET` env var
    /// 3. Default per-user path from `modula_platform::paths::engine_socket_path`
    pub fn resolve(socket_path: Option<PathBuf>) -> Result<Self, Error> {
        let path = socket_path
            .or_else(|| std::env::var_os("MODULA_ENGINE_SOCKET").map(PathBuf::from))
            .or_else(modula_platform::paths::engine_socket_path)
            .ok_or(Error::NoSocketPath)?;
        Ok(Self::new(path))
    }

    /// Open a gRPC `Channel` to this endpoint.
    pub async fn connect(&self) -> Result<Channel, Error> {
        self.connect_inner().await
    }

    #[cfg(unix)]
    async fn connect_inner(&self) -> Result<Channel, Error> {
        use std::sync::Arc;

        use hyper_util::rt::TokioIo;
        use tower::service_fn;

        let path = Arc::new(self.path.clone());
        let channel = Endpoint::from_static("http://[::]:50051")
            .connect_with_connector(service_fn(move |_: tonic::transport::Uri| {
                let p = Arc::clone(&path);
                async move {
                    let stream = tokio::net::UnixStream::connect(p.as_path()).await?;
                    Ok::<_, std::io::Error>(TokioIo::new(stream))
                }
            }))
            .await?;
        Ok(channel)
    }

    #[cfg(windows)]
    async fn connect_inner(&self) -> Result<Channel, Error> {
        use std::sync::Arc;

        use hyper_util::rt::TokioIo;
        use tower::service_fn;

        let name = Arc::new(
            self.path
                .to_str()
                .ok_or_else(|| {
                    Error::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "pipe name is not valid UTF-8",
                    ))
                })?
                .to_owned(),
        );
        let channel = Endpoint::from_static("http://[::]:50051")
            .connect_with_connector(service_fn(move |_: tonic::transport::Uri| {
                let n = Arc::clone(&name);
                async move {
                    loop {
                        match tokio::net::windows::named_pipe::ClientOptions::new().open(n.as_str())
                        {
                            Ok(client) => break Ok(TokioIo::new(client)),
                            Err(e) if e.raw_os_error() == Some(231) => {
                                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                            }
                            Err(e) => break Err(e),
                        }
                    }
                }
            }))
            .await?;
        Ok(channel)
    }
}

impl fmt::Display for LocalIpcEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path.display())
    }
}

/// Placeholder for future remote gRPC credentials (TLS/mTLS client identity).
/// Present so call sites don't change when a remote transport is added.
#[derive(Debug, Clone)]
pub struct ClientIdentity;

/// Typed engine endpoint. Use `LocalIpcEndpoint::resolve()` to construct it;
/// never a process-global singleton.
#[derive(Debug, Clone)]
pub enum EngineEndpoint {
    /// Default: gRPC over local IPC (Unix-domain socket or Windows named pipe).
    LocalIpc(LocalIpcEndpoint),
    /// Future: gRPC over TLS/mTLS to a remote node. Not implemented.
    RemoteGrpc {
        uri: tonic::transport::Uri,
        identity: ClientIdentity,
    },
}

impl EngineEndpoint {
    pub fn as_local_ipc(&self) -> Option<&LocalIpcEndpoint> {
        match self {
            EngineEndpoint::LocalIpc(ipc) => Some(ipc),
            EngineEndpoint::RemoteGrpc { .. } => None,
        }
    }
}

impl fmt::Display for EngineEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineEndpoint::LocalIpc(ipc) => write!(f, "ipc:{ipc}"),
            EngineEndpoint::RemoteGrpc { uri, .. } => write!(f, "{uri}"),
        }
    }
}
