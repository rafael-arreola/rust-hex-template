//! Validating request extractors; rejections stay inside the HTTP envelope.

use axum::{
    body::Bytes,
    extract::{FromRequest, FromRequestParts, Path, Query, Request},
    http::request::Parts,
};
use serde::{Deserialize, de::DeserializeOwned};
use validator::{Validate, ValidationErrors, ValidationErrorsKind};

use super::error::ApiError;
use super::format::BodyFormat;
use crate::domain::core::error::{DomainError, DomainResult, StatusCode};
use crate::domain::core::pagination::Pagination;

/// Deserializes supported JSON/MessagePack media types and executes `Validate`.
/// MessagePack input remains supported when response negotiation is disabled.
#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedBody<T>(pub T);

impl<T, S> FromRequest<S> for ValidatedBody<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let format = BodyFormat::of_request(req.headers())?;
        let bytes = Bytes::from_request(req, state).await.map_err(ApiError::from)?;
        let value: T = format.decode(&bytes)?;

        value.validate().map_err(|errors| {
            let message = validation_message(&errors, "The request body failed validation.");
            ApiError::from(
                DomainError::new(StatusCode::BAD_REQUEST, message.clone())
                    .with_public(message)
                    .with_trace_code("HTTP_BODY_VALIDATION_FAILED"),
            )
        })?;
        Ok(Self(value))
    }
}

/// Preserves Axum's distinction between invalid input (400) and broken handler wiring (500).
#[derive(Debug, Clone, Copy, Default)]
pub struct PathParam<T>(pub T);

impl<T, S> FromRequestParts<S> for PathParam<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(value) = Path::<T>::from_request_parts(parts, state).await.map_err(|error| {
            let domain_error = if error.status().is_server_error() {
                DomainError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Path extraction failed because the handler and route are incompatible.",
                )
                .with_trace_code("HTTP_PATH_CONFIGURATION_ERROR")
            } else {
                DomainError::new(StatusCode::BAD_REQUEST, "The route parameters are invalid.")
                    .with_public("The route parameters are invalid.")
                    .with_trace_code("HTTP_INVALID_PATH")
            };
            ApiError::from(domain_error)
        })?;
        Ok(Self(value))
    }
}

/// Both query deserialization and validation failures stay inside the HTTP envelope.
#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedQuery<T>(pub T);

impl<T, S> FromRequestParts<S> for ValidatedQuery<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state).await.map_err(|_| {
            ApiError::from(
                DomainError::new(StatusCode::BAD_REQUEST, "The query parameters are invalid.")
                    .with_public("The query parameters are invalid.")
                    .with_trace_code("HTTP_INVALID_QUERY"),
            )
        })?;

        value.validate().map_err(|errors| {
            let message = validation_message(&errors, "The query parameters failed validation.");
            ApiError::from(
                DomainError::new(StatusCode::BAD_REQUEST, message.clone())
                    .with_public(message)
                    .with_trace_code("HTTP_QUERY_VALIDATION_FAILED"),
            )
        })?;
        Ok(Self(value))
    }
}

/// Shared transport query for list endpoints; `Pagination::new` remains the authoritative check.
#[derive(Deserialize, Validate)]
pub struct PaginationQuery {
    #[validate(range(min = 1, message = "Invalid page: must be at least 1"))]
    pub page: Option<u32>,
    #[validate(range(min = 1, max = 100, message = "Invalid limit: must be between 1 and 100"))]
    pub limit: Option<u32>,
}

impl TryFrom<PaginationQuery> for Pagination {
    type Error = DomainError;

    fn try_from(query: PaginationQuery) -> DomainResult<Self> {
        Self::new(query.page.unwrap_or(1), query.limit.unwrap_or(20))
    }
}

/// Formats static DTO validation messages without inspecting rejected input values.
fn validation_message(errors: &ValidationErrors, fallback: &'static str) -> String {
    let mut messages = Vec::new();
    collect_validation_messages(errors, "", &mut messages);
    messages.sort();
    messages.dedup();
    if messages.is_empty() { fallback.to_string() } else { messages.join("; ") }
}

fn collect_validation_messages(errors: &ValidationErrors, prefix: &str, out: &mut Vec<String>) {
    for (field, kind) in errors.errors() {
        let path = if prefix.is_empty() { field.to_string() } else { format!("{prefix}.{field}") };
        match kind {
            ValidationErrorsKind::Field(field_errors) => {
                for error in field_errors {
                    match &error.message {
                        Some(message) if !message.trim().is_empty() => {
                            out.push(message.trim().to_string());
                        }
                        _ => out.push(format!("Invalid {path}")),
                    }
                }
            }
            ValidationErrorsKind::Struct(nested) => {
                collect_validation_messages(nested, &path, out);
            }
            ValidationErrorsKind::List(list) => {
                for (index, nested) in list {
                    collect_validation_messages(nested, &format!("{path}[{index}]"), out);
                }
            }
        }
    }
}
