use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use opentelemetry::trace::TraceContextExt;
use serde::Serialize;
use std::sync::Arc;
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Type-erased handle to the original response value, stored in the response
/// extensions so the MessagePack negotiation middleware can encode it once,
/// directly from the value — no JSON byte round-trip.
#[derive(Clone)]
pub struct NegotiablePayload(pub Arc<dyn erased_serde::Serialize + Send + Sync>);

#[derive(Debug, Serialize)]
pub struct GenericPagination<T> {
    pub data: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub limit: u32,
}

/// Body of an error response, carried inside `data`. Kept as an object (not a
/// bare string) so error payloads can grow fields without breaking clients.
#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub message: String,
}

/// Standard HTTP envelope — every response, success or error, has the same
/// shape: `trace_id` for correlation and `data` for the payload. Errors add
/// `cause` with the stable machine-readable code from `DomainError::code()`.
///
/// ```json
/// { "trace_id": "4bf9…", "data": { "id": "u1", "name": "Ada" } }
/// { "trace_id": "4bf9…", "data": { "message": "User not found: u9" }, "cause": "NOT_FOUND" }
/// ```
#[derive(Debug, Serialize)]
pub struct GenericApiResponse<T> {
    pub trace_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,

    /// Stable, machine-readable error code. Present only on errors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<&'static str>,

    #[serde(skip)]
    pub status: StatusCode,
}

impl<T> IntoResponse for GenericApiResponse<T>
where
    T: Serialize + Send + Sync + 'static,
{
    fn into_response(self) -> Response {
        let status = self.status;
        let shared = Arc::new(self);

        let mut response = (status, Json(&*shared)).into_response();
        response.extensions_mut().insert(NegotiablePayload(shared));
        response
    }
}

impl<T> GenericApiResponse<T> {
    fn get_current_trace_id() -> String {
        let context = tracing::Span::current().context();
        let span = context.span();
        let span_context = span.span_context();

        if span_context.is_valid() {
            format!("{:032x}", span_context.trace_id())
        } else {
            "00000000000000000000000000000000".to_string()
        }
    }

    pub fn success(data: T) -> Self {
        Self {
            trace_id: Self::get_current_trace_id(),
            data: Some(data),
            cause: None,
            status: StatusCode::OK,
        }
    }
}

impl GenericApiResponse<ErrorDetail> {
    pub fn error(code: &'static str, message: String, status: StatusCode) -> Self {
        Self {
            trace_id: Self::get_current_trace_id(),
            data: Some(ErrorDetail { message }),
            cause: Some(code),
            status,
        }
    }
}

impl<T: Serialize> GenericApiResponse<GenericPagination<T>> {
    /// Wraps a paginated collection with metadata.
    ///
    /// # Example
    /// ```ignore
    /// let pagination = Pagination { page: 1, limit: 20 };
    /// let users = service.list_users(pagination.clone()).await?;
    /// let total = service.count_users().await?;
    ///
    /// Ok(GenericApiResponse::paginated(
    ///     users.into_iter().map(Into::into).collect(),
    ///     total,
    ///     pagination.page,
    ///     pagination.limit,
    /// ))
    /// ```
    pub fn paginated(data: Vec<T>, total: u64, page: u32, limit: u32) -> Self {
        GenericApiResponse {
            trace_id: Self::get_current_trace_id(),
            data: Some(GenericPagination { data, total, page, limit }),
            cause: None,
            status: StatusCode::OK,
        }
    }
}
