use modula_engine_transport::Error as TransportError;

/// The single error every client method returns. It carries the engine's
/// human-readable detail message, so callers surface a clean one-liner without
/// re-deriving it (mirrors the old CLI `rpc_err` and Tauri `status_msg`, which
/// differed only in return type).
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// Dialing the engine failed (socket missing, engine not running).
    #[error("{0}")]
    Transport(#[from] TransportError),
    /// The engine rejected the call; carries the gRPC status detail message.
    #[error("{0}")]
    Rpc(String),
    /// A client-side lookup (`task_by_id`, `workspace_by_ref`, …) found nothing.
    #[error("{0}")]
    NotFound(String),
}

/// Lets Tauri commands keep returning `Result<_, String>` via `?`/`map_err`.
impl From<ClientError> for String {
    fn from(e: ClientError) -> Self {
        e.to_string()
    }
}

/// Map a `tonic::Status` to a [`ClientError::Rpc`] using its detail message, or
/// the status code when the server left the message empty.
/// Map a gRPC status to a client error, preferring its detail message. Public
/// for plugin-side clients.
pub fn rpc(status: tonic::Status) -> ClientError {
    let msg = status.message().trim();
    if msg.is_empty() {
        ClientError::Rpc(status.code().to_string())
    } else {
        ClientError::Rpc(msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_uses_detail_message() {
        let e = rpc(tonic::Status::not_found("no task with id x"));
        assert_eq!(e.to_string(), "no task with id x");
    }

    #[test]
    fn rpc_falls_back_to_code_when_empty() {
        let e = rpc(tonic::Status::unavailable(""));
        assert_eq!(e.to_string(), tonic::Code::Unavailable.to_string());
    }

    #[test]
    fn error_into_string() {
        let s: String = ClientError::NotFound("nope".into()).into();
        assert_eq!(s, "nope");
    }
}
