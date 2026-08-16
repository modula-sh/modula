pub mod caller;
mod endpoint;
pub mod server;
mod stale;
#[cfg(test)]
mod tests;

pub use caller::LocalCaller;
pub use endpoint::{ClientIdentity, EngineEndpoint, LocalIpcEndpoint};
pub use server::{LocalListener, LocalPeerStream};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no socket path: set MODULA_ENGINE_SOCKET or pass --socket")]
    NoSocketPath,
    #[error("engine already running at {0}; stop it first")]
    AlreadyRunning(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Transport(#[from] tonic::transport::Error),
}
