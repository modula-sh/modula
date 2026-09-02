use tonic::{Code, Status};

use modula_core::error::ApiError;

pub fn to_status(e: ApiError) -> Status {
    match e {
        ApiError::BadRequest(msg) => Status::new(Code::InvalidArgument, msg),
        ApiError::NotFound(msg) => Status::new(Code::NotFound, msg),
        ApiError::Forbidden(msg) => Status::new(Code::PermissionDenied, msg),
        ApiError::Conflict(msg) => Status::new(Code::AlreadyExists, msg),
        ApiError::Internal(msg) => Status::new(Code::Internal, msg),
    }
}

#[allow(dead_code)]
pub fn internal(e: impl std::fmt::Display) -> Status {
    Status::new(Code::Internal, e.to_string())
}
