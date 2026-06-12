use crate::domain::error::DomainError;
use crate::infrastructure::driving::http_axum::server::response::GenericApiResponse;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

/// Presentation-layer error. Clients rely on `code` (stable, machine-readable),
/// users read `message`. Never expose internal details in `message`.
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

        ApiError { code, message, status }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        GenericApiResponse::error(self.code, self.message, self.status).into_response()
    }
}
