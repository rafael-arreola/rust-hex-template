use axum::{
    extract::rejection::BytesRejection,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};

use super::response::ApiResponse;
use crate::domain::core::error::{DomainError, ErrorKind, ErrorSeverity};

/// HTTP boundary failures have their own status and cause, without changing the domain taxonomy.
/// Their diagnostics never include deserializer messages or rejected input values.
#[derive(Debug)]
pub enum HttpRejection {
    InvalidBody,
    BodyContract,
    BodyValidation,
    BodyRead,
    InvalidQuery,
    QueryValidation,
    InvalidPath,
    PathConfiguration,
    PayloadTooLarge,
    UnsupportedMediaType,
    UnsupportedContentEncoding,
    NotAcceptable,
    RouteNotFound,
    MethodNotAllowed,
    ResponseSerialization { format: &'static str },
}

impl HttpRejection {
    fn status(&self) -> StatusCode {
        match self {
            Self::InvalidBody
            | Self::BodyValidation
            | Self::BodyRead
            | Self::InvalidQuery
            | Self::QueryValidation
            | Self::InvalidPath => StatusCode::BAD_REQUEST,
            Self::BodyContract => StatusCode::UNPROCESSABLE_ENTITY,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::UnsupportedMediaType | Self::UnsupportedContentEncoding => {
                StatusCode::UNSUPPORTED_MEDIA_TYPE
            }
            Self::NotAcceptable => StatusCode::NOT_ACCEPTABLE,
            Self::RouteNotFound => StatusCode::NOT_FOUND,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::PathConfiguration | Self::ResponseSerialization { .. } => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Self::PayloadTooLarge => "PAYLOAD_TOO_LARGE",
            Self::UnsupportedMediaType => "UNSUPPORTED_MEDIA_TYPE",
            Self::UnsupportedContentEncoding => "UNSUPPORTED_CONTENT_ENCODING",
            Self::NotAcceptable => "NOT_ACCEPTABLE",
            Self::RouteNotFound => "ROUTE_NOT_FOUND",
            Self::MethodNotAllowed => "METHOD_NOT_ALLOWED",
            Self::PathConfiguration | Self::ResponseSerialization { .. } => "INTERNAL_ERROR",
            _ => "INVALID_INPUT",
        }
    }

    fn public_message(&self) -> &'static str {
        match self {
            Self::InvalidBody => "The request body is malformed.",
            Self::BodyContract => "The request body does not match the expected contract.",
            Self::BodyValidation => "The request body failed validation.",
            Self::BodyRead => "The request body could not be read.",
            Self::InvalidQuery => "The query parameters are invalid.",
            Self::QueryValidation => "The query parameters failed validation.",
            Self::InvalidPath => "The route parameters are invalid.",
            Self::PayloadTooLarge => "The request body exceeds the maximum allowed size.",
            Self::UnsupportedMediaType => "The request Content-Type is not supported.",
            Self::UnsupportedContentEncoding => "The request Content-Encoding is not supported.",
            Self::NotAcceptable => "The requested response media type is not supported.",
            Self::RouteNotFound => "The requested resource was not found.",
            Self::MethodNotAllowed => "The HTTP method is not supported for this resource.",
            Self::PathConfiguration | Self::ResponseSerialization { .. } => {
                "An internal error occurred. Use the trace_id to report it."
            }
        }
    }

    fn internal_message(&self) -> String {
        match self {
            Self::ResponseSerialization { format } => {
                format!("{format} response serialization failed")
            }
            Self::PathConfiguration => {
                "Path extraction failed because the handler and route are incompatible.".to_string()
            }
            _ => self.public_message().to_string(),
        }
    }
}

#[derive(Debug)]
enum ErrorSource {
    Domain(DomainError),
    Http(HttpRejection),
}

/// Logging happens only when an error actually becomes an HTTP response, not when it is converted.
#[derive(Debug)]
pub struct ApiError {
    source: ErrorSource,
}

impl From<DomainError> for ApiError {
    fn from(error: DomainError) -> Self {
        Self { source: ErrorSource::Domain(error) }
    }
}

impl From<HttpRejection> for ApiError {
    fn from(rejection: HttpRejection) -> Self {
        Self { source: ErrorSource::Http(rejection) }
    }
}

impl From<BytesRejection> for ApiError {
    fn from(rejection: BytesRejection) -> Self {
        let rejection = if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
            HttpRejection::PayloadTooLarge
        } else {
            HttpRejection::BodyRead
        };
        rejection.into()
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (code, status, message, severity, internal, context) = match self.source {
            ErrorSource::Domain(error) => {
                let status = match error.kind() {
                    ErrorKind::NotFound => StatusCode::NOT_FOUND,
                    ErrorKind::AlreadyExists => StatusCode::CONFLICT,
                    ErrorKind::Invalid => StatusCode::BAD_REQUEST,
                    ErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
                    ErrorKind::Forbidden => StatusCode::FORBIDDEN,
                    ErrorKind::BusinessRule => StatusCode::UNPROCESSABLE_ENTITY,
                    ErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
                    ErrorKind::Timeout => StatusCode::GATEWAY_TIMEOUT,
                    ErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
                };
                let internal =
                    error.internal_detail().unwrap_or(error.public_message()).to_string();
                let context = error.context_trail().join(" -> ");
                let message = error.public_message().to_string();
                (error.code(), status, message, error.severity(), internal, context)
            }
            ErrorSource::Http(rejection) => {
                let status = rejection.status();
                // Server faults are errors; a wrong method on an existing route deserves a warning.
                let severity = match rejection {
                    _ if status.is_server_error() => ErrorSeverity::Error,
                    HttpRejection::MethodNotAllowed => ErrorSeverity::Warn,
                    _ => ErrorSeverity::Info,
                };
                let message = rejection.public_message().to_string();
                (
                    rejection.code(),
                    status,
                    message,
                    severity,
                    rejection.internal_message(),
                    String::new(),
                )
            }
        };

        let status_code = status.as_u16();
        match severity {
            ErrorSeverity::Error => tracing::error!(
                error.code = code, error.internal = %internal, error.context = %context,
                http.status_code = status_code, "Request failed"
            ),
            ErrorSeverity::Warn => tracing::warn!(
                error.code = code, error.internal = %internal, error.context = %context,
                http.status_code = status_code, "Request rejected"
            ),
            ErrorSeverity::Info => tracing::info!(
                error.code = code, error.internal = %internal, error.context = %context,
                http.status_code = status_code, "Request rejected"
            ),
        }

        let mut response = ApiResponse::error(code, message, status).into_response();
        if status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
    }
}
