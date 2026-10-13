use axum::{
    extract::rejection::BytesRejection,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};

use super::response::ApiResponse;
use crate::domain::core::error::DomainError;

/// HTTP error wrapper around [`DomainError`]. Logs once when converted into an HTTP response.
#[derive(Debug)]
pub struct ApiError(DomainError);

impl From<DomainError> for ApiError {
    fn from(error: DomainError) -> Self {
        Self(error)
    }
}

impl From<BytesRejection> for ApiError {
    fn from(rejection: BytesRejection) -> Self {
        let error = if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
            DomainError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "The request body exceeds the maximum allowed size.",
            )
            .with_trace_code("HTTP_PAYLOAD_TOO_LARGE")
        } else {
            DomainError::new(StatusCode::BAD_REQUEST, "The request body could not be read.")
                .with_public("The request body could not be read.")
                .with_trace_code("HTTP_BODY_READ_FAILED")
        };
        Self(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        self.0.log();
        let status = self.0.status();
        let mut response =
            ApiResponse::error(self.0.code(), self.0.public_message().to_owned(), status)
                .into_response();
        if status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
    }
}
