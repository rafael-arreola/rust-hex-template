use crate::domain::error::{DomainError, ErrorSeverity};
use crate::infrastructure::driving::http_axum::server::response::GenericApiResponse;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

/// Presentation-layer error. Clients rely on `code` (stable, machine-readable),
/// users read `message` — always built from `DomainError::public_message()`,
/// never from `Display` (that is the internal, logs-only view).
#[derive(Debug)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
    pub status: StatusCode,
}

impl ApiError {
    pub fn bad_request(message: String) -> Self {
        Self { code: "INVALID_INPUT", message, status: StatusCode::BAD_REQUEST }
    }
}

impl From<DomainError> for ApiError {
    fn from(err: DomainError) -> Self {
        let code = err.code();
        let internal = err.to_string();
        let message = err.public_message();

        let status = match &err {
            DomainError::NotFound { .. } => StatusCode::NOT_FOUND,
            DomainError::AlreadyExists { .. } => StatusCode::CONFLICT,
            DomainError::Invalid { .. } | DomainError::Required { .. } => StatusCode::BAD_REQUEST,
            DomainError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            DomainError::Forbidden(_) => StatusCode::FORBIDDEN,
            DomainError::BusinessRule(_) => StatusCode::UNPROCESSABLE_ENTITY,
            DomainError::ExternalService { .. }
            | DomainError::Database(_)
            | DomainError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        // Single logging choke point: every domain error crossing the HTTP
        // boundary is logged exactly once, with full internal detail, at the
        // severity the error itself declares. The active request span adds
        // the trace_id, so the log line correlates with the response envelope.
        match err.severity() {
            ErrorSeverity::Error => {
                tracing::error!(%code, %internal, status = status.as_u16(), "Request failed");
            }
            ErrorSeverity::Warn => {
                tracing::warn!(%code, %internal, status = status.as_u16(), "Request rejected");
            }
            ErrorSeverity::Info => {
                tracing::info!(%code, %internal, status = status.as_u16(), "Request rejected");
            }
        }

        ApiError { code, message, status }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        GenericApiResponse::error(self.code, self.message, self.status).into_response()
    }
}
