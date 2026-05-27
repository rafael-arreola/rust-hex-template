use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::error::DomainError;
use serde::Serialize;

/// Lightweight payload for error responses. Clients rely on `code`,
/// users read `message`. Never expose internal details in `message`.
#[derive(Debug, Serialize)]
pub struct ApiErrorPayload {
    pub code: &'static str,
    pub message: String,
}

/// Presentation-layer error. Carries the HTTP status plus the structured payload.
#[derive(Debug)]
pub struct ApiError {
    pub payload: ApiErrorPayload,
    pub status: StatusCode,
}

impl ApiError {
    pub fn bad_request(message: String) -> Self {
        Self {
            payload: ApiErrorPayload { code: "INVALID_INPUT", message },
            status: StatusCode::BAD_REQUEST,
        }
    }
}

impl From<DomainError> for ApiError {
    fn from(err: DomainError) -> Self {
        let code = err.code();
        let message = err.to_string();

        let status = match &err {
            DomainError::NotFound { .. } => StatusCode::NOT_FOUND,
            DomainError::AlreadyExists { .. } => StatusCode::CONFLICT,
            DomainError::Invalid { .. } | DomainError::Required { .. } => StatusCode::BAD_REQUEST,
            DomainError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            DomainError::Forbidden(_) => StatusCode::FORBIDDEN,
            DomainError::BusinessRule(_) => StatusCode::UNPROCESSABLE_ENTITY,
            DomainError::ExternalService { .. }
            | DomainError::Database(_)
            | DomainError::Internal(_) => {
                tracing::error!(%code, %message, "Internal error mapped to 500");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };

        ApiError { payload: ApiErrorPayload { code, message }, status }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        crate::server::response::GenericApiResponse::<()>::error(
            self.payload.code,
            self.payload.message,
            self.status,
        )
        .into_response()
    }
}
