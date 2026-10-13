mod trace;

use axum::{
    body::Body,
    extract::State,
    http::{HeaderValue, Request, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::time::Duration;

use super::error::{ApiError, HttpRejection};
use crate::domain::core::error::DomainError;

pub(super) use trace::trace_context;

/// The budget includes body buffering and decoding; layer inside tracing and negotiation.
pub async fn request_timeout(
    State(budget): State<Duration>,
    req: Request<Body>,
    next: Next,
) -> Response {
    match tokio::time::timeout(budget, next.run(req)).await {
        Ok(response) => response,
        Err(_) => ApiError::from(DomainError::timeout("HTTP request exceeded its time budget"))
            .into_response(),
    }
}

/// Validate before Tower's decoder, so its unsupported-encoding shortcut cannot emit a bare 415.
pub async fn content_encoding(mut req: Request<Body>, next: Next) -> Response {
    let mut values = req.headers().get_all(header::CONTENT_ENCODING).iter();
    let encoding = match (values.next(), values.next()) {
        (None, _) => None,
        (Some(value), None) => match value.to_str().map(str::trim) {
            Ok(value) if value.eq_ignore_ascii_case("gzip") => Some("gzip"),
            Ok(value) if value.eq_ignore_ascii_case("identity") => Some("identity"),
            _ => return ApiError::from(HttpRejection::UnsupportedContentEncoding).into_response(),
        },
        (Some(_), Some(_)) => {
            return ApiError::from(HttpRejection::UnsupportedContentEncoding).into_response();
        }
    };
    if let Some(encoding) = encoding {
        req.headers_mut().insert(header::CONTENT_ENCODING, HeaderValue::from_static(encoding));
    }
    next.run(req).await
}
