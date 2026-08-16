use tonic::{Code, Status};

/// The shared domain error for the engine's data layer (`modula-db`) and its
/// services. Defined here so `modula-db` can return it and the engine services
/// can convert to `tonic::Status` without depending on the engine. gRPC
/// handlers map it at the edge (`grpc::error::to_status`).
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Internal(String),
}

impl From<sqlx::Error> for DomainError {
    fn from(e: sqlx::Error) -> Self {
        // Only the mappings that are correct at *every* call site live here: a
        // UNIQUE violation is always a 409. Everything else — including
        // `RowNotFound` — is a server fault; genuine 404s are produced
        // explicitly at the query site (fetch_optional + ok_or_else).
        match e {
            sqlx::Error::Database(db) if db.is_unique_violation() => {
                DomainError::Conflict(db.message().to_string())
            }
            other => DomainError::Internal(format!("sqlx: {other}")),
        }
    }
}

impl From<serde_json::Error> for DomainError {
    fn from(e: serde_json::Error) -> Self {
        DomainError::Internal(format!("json: {e}"))
    }
}

impl From<std::io::Error> for DomainError {
    fn from(e: std::io::Error) -> Self {
        DomainError::Internal(e.to_string())
    }
}

impl From<anyhow::Error> for DomainError {
    fn from(e: anyhow::Error) -> Self {
        DomainError::Internal(e.to_string())
    }
}

impl From<DomainError> for Status {
    fn from(e: DomainError) -> Self {
        match e {
            DomainError::BadRequest(msg) => Status::new(Code::InvalidArgument, msg),
            DomainError::NotFound(msg) => Status::new(Code::NotFound, msg),
            DomainError::Forbidden(msg) => Status::new(Code::PermissionDenied, msg),
            DomainError::Conflict(msg) => Status::new(Code::AlreadyExists, msg),
            DomainError::Internal(msg) => Status::new(Code::Internal, msg),
        }
    }
}

/// Convenience alias for RPC handler results.
pub type RpcResult<T> = Result<tonic::Response<T>, Status>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_request_maps_to_invalid_argument() {
        let s = Status::from(DomainError::BadRequest("missing field".into()));
        assert_eq!(s.code(), Code::InvalidArgument);
        assert_eq!(s.message(), "missing field");
    }

    #[test]
    fn not_found_maps_to_not_found() {
        let s = Status::from(DomainError::NotFound("task not found".into()));
        assert_eq!(s.code(), Code::NotFound);
    }

    #[test]
    fn conflict_maps_to_already_exists() {
        let s = Status::from(DomainError::Conflict("name collision".into()));
        assert_eq!(s.code(), Code::AlreadyExists);
    }

    #[test]
    fn internal_maps_to_internal() {
        let s = Status::from(DomainError::Internal("db error".into()));
        assert_eq!(s.code(), Code::Internal);
    }
}
