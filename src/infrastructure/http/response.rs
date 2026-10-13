use axum::{
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use opentelemetry::trace::TraceContextExt;
use serde::Serialize;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use super::error::ApiError;
use super::format::BodyFormat;
use crate::domain::core::error::{DomainError, DomainResult};
use crate::domain::core::pagination::{PageResult, Pagination};

#[derive(Debug, Serialize)]
pub struct Paginated<T> {
    pub data: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub limit: u32,
}

#[derive(Debug, Serialize)]
pub(super) struct ErrorDetail {
    pub message: String,
}

/// Standard HTTP envelope. Only errors contain `cause`; 204 responses contain no envelope.
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub trace_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<&'static str>,

    #[serde(skip)]
    pub status: StatusCode,
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        if self.status == StatusCode::NO_CONTENT {
            return StatusCode::NO_CONTENT.into_response();
        }

        let format = BodyFormat::negotiated();
        match format.encode(&self) {
            Ok(bytes) => (
                self.status,
                [(header::CONTENT_TYPE, HeaderValue::from_static(format.content_type()))],
                bytes,
            )
                .into_response(),
            Err(format) => ApiError::from(
                DomainError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("{format} response serialization failed"),
                )
                .with_trace_code("HTTP_RESPONSE_SERIALIZATION_FAILED"),
            )
            .into_response(),
        }
    }
}

impl<T> ApiResponse<T> {
    fn new(status: StatusCode, data: Option<T>, cause: Option<&'static str>) -> Self {
        let context = tracing::Span::current().context();
        // OpenTelemetry's all-zero invalid ID preserves the String contract when tracing is
        // unavailable; it is not a fabricated correlation identifier.
        let trace_id = context.span().span_context().trace_id().to_string();
        Self { trace_id, data, cause, status }
    }

    pub fn ok(data: T) -> Self {
        Self::new(StatusCode::OK, Some(data), None)
    }

    pub fn created(data: T) -> Self {
        Self::new(StatusCode::CREATED, Some(data), None)
    }
}

impl ApiResponse<()> {
    pub fn empty() -> Self {
        Self::new(StatusCode::NO_CONTENT, None, None)
    }
}

impl ApiResponse<ErrorDetail> {
    pub(super) fn error(code: &'static str, message: String, status: StatusCode) -> Self {
        Self::new(status, Some(ErrorDetail { message }), Some(code))
    }
}

impl<T> ApiResponse<Paginated<T>> {
    pub fn paginated(data: Vec<T>, total: u64, page: u32, limit: u32) -> Self {
        Self::new(StatusCode::OK, Some(Paginated { data, total, page, limit }), None)
    }
}

/// Maps every item to its output DTO and builds the paginated envelope for the same request.
pub fn paginated<E, O>(
    result: PageResult<E>,
    pagination: Pagination,
) -> Result<ApiResponse<Paginated<O>>, ApiError>
where
    O: TryFrom<E, Error = DomainError>,
{
    let data = result.items.into_iter().map(O::try_from).collect::<DomainResult<Vec<_>>>()?;
    Ok(ApiResponse::paginated(data, result.total, pagination.page(), pagination.limit()))
}
