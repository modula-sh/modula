/// The engine's domain error type. gRPC handlers map it to `tonic::Status`
/// (see `grpc::error::to_status`); the service layer raises it directly.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
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

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        ApiError::Internal(format!("json: {e}"))
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        // Only the mappings that are correct at *every* call site live here: a
        // UNIQUE violation is always a 409. Everything else — including
        // `RowNotFound` — is a server fault; genuine 404s are produced
        // explicitly at the query site (fetch_optional + ok_or_else) where the
        // missing entity is known.
        match e {
            sqlx::Error::Database(db) if db.is_unique_violation() => {
                ApiError::Conflict(db.message().to_string())
            }
            other => ApiError::Internal(format!("sqlx: {other}")),
        }
    }
}

impl From<modula_rpc::status::DomainError> for ApiError {
    fn from(e: modula_rpc::status::DomainError) -> Self {
        use modula_rpc::status::DomainError as D;
        match e {
            D::BadRequest(msg) => ApiError::BadRequest(msg),
            D::NotFound(msg) => ApiError::NotFound(msg),
            D::Forbidden(msg) => ApiError::Forbidden(msg),
            D::Conflict(msg) => ApiError::Conflict(msg),
            D::Internal(msg) => ApiError::Internal(msg),
        }
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
