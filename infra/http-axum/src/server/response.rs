use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use opentelemetry::trace::TraceContextExt;
use serde::Serialize;
use tracing_opentelemetry::OpenTelemetrySpanExt;

#[derive(Debug, Serialize)]
pub struct GenericPagination<T> {
    pub data: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub limit: u32,
}

#[derive(Debug, Serialize)]
pub struct GenericApiResponse<T> {
    pub trace_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,

    #[serde(skip)]
    pub status: StatusCode,
}

impl<T> IntoResponse for GenericApiResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        (self.status, Json(self)).into_response()
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

    pub fn error(cause: String, status: StatusCode) -> Self {
        Self {
            trace_id: Self::get_current_trace_id(),
            data: None,
            cause: Some(cause),
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
            data: Some(GenericPagination {
                data,
                total,
                page,
                limit,
            }),
            cause: None,
            status: StatusCode::OK,
        }
    }
}
