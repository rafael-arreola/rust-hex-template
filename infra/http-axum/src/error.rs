use crate::response::GenericApiResponse;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::error::DomainError;
use thiserror::Error;

/// Presentation layer error type that bridges Domain errors to HTTP responses.
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Validation failed: {0}")]
    BadRequest(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Unauthorized access: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Business logic error: {0}")]
    UnprocessableEntity(String),

    #[error("Internal server error")]
    Internal(String),
}

impl From<DomainError> for ApiError {
    fn from(err: DomainError) -> Self {
        match err {
            DomainError::Invalid { field, reason } => {
                ApiError::BadRequest(format!("Invalid {}: {}", field, reason))
            }
            DomainError::Required { field } => {
                ApiError::BadRequest(format!("{} is required", field))
            }
            DomainError::NotFound { entity, id } => {
                ApiError::NotFound(format!("{} not found: {}", entity, id))
            }
            DomainError::AlreadyExists { entity, details } => {
                ApiError::Conflict(format!("{} already exists: {}", entity, details))
            }
            DomainError::Unauthorized(msg) => ApiError::Unauthorized(msg),
            DomainError::Forbidden(msg) => ApiError::Forbidden(msg),
            DomainError::BusinessRule(msg) => ApiError::UnprocessableEntity(msg),
            DomainError::ExternalService { service, message } => {
                tracing::error!("External service error [{}]: {}", service, message);
                ApiError::Internal(format!("External service error: {}", service))
            }
            DomainError::Database(msg) => {
                tracing::error!("Database error: {}", msg);
                ApiError::Internal("Database error occurred".to_string())
            }
            DomainError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                ApiError::Internal("Internal server error".to_string())
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            ApiError::UnprocessableEntity(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        GenericApiResponse::<()>::error(message, status).into_response()
    }
}
