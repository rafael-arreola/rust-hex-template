use std::borrow::Cow;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

pub use axum::http::StatusCode;
pub use tracing::Level;

#[derive(Debug)]
struct Inner {
    status: StatusCode,
    public_code: &'static str,
    public_message: Cow<'static, str>,
    trace_code: Option<&'static str>,
    breadcrumb: Vec<String>,
    level: Level,
    logged: AtomicBool,
}

/// Two-layer failure value:
/// - **Public layer (JSON client response):** `status()`, `code()` (`cause`) and `public_message()`
///   (`data.message`), populated with safe defaults for the [`StatusCode`] unless overridden with
///   [`with_code`](Self::with_code) or [`with_public`](Self::with_public).
/// - **Developer layer (tracing logs):** `trace_code()` (`error.code`), `breadcrumb()`
///   (`error.breadcrumb`, ordered from deepest root cause to outermost caller) and `level()`.
///
/// A `DomainError` logs itself automatically once — either when converted into an HTTP response,
/// when explicitly emitted via [`log`](Self::log)/[`warn`](Self::warn)/[`info`](Self::info)/[`error`](Self::error),
/// or when dropped without having been chained into another `DomainError`.
pub struct DomainError(Box<Inner>);

pub type DomainResult<T> = Result<T, DomainError>;

impl DomainError {
    /// Creates an error from a standard [`StatusCode`] and its initial developer breadcrumb entry.
    pub fn new(status: StatusCode, detail: impl Into<String>) -> Self {
        let detail = detail.into();
        let breadcrumb = if detail.is_empty() {
            vec![default_public_message(status).to_string()]
        } else {
            vec![detail]
        };
        Self(Box::new(Inner {
            status,
            public_code: default_public_code(status),
            public_message: Cow::Borrowed(default_public_message(status)),
            trace_code: None,
            breadcrumb,
            level: default_level(status),
            logged: AtomicBool::new(false),
        }))
    }

    /// Replaces the public `cause` code sent to the client.
    pub fn with_code(mut self, code: &'static str) -> Self {
        self.0.public_code = code;
        self
    }

    /// Replaces the public `data.message` sent to the client; must be safe to expose.
    pub fn with_public(mut self, message: impl Into<Cow<'static, str>>) -> Self {
        self.0.public_message = message.into();
        self
    }

    /// Sets a specific diagnostic code for developer logs (`error.code`).
    pub fn with_trace_code(mut self, code: &'static str) -> Self {
        self.0.trace_code = Some(code);
        self
    }

    /// Overrides the tracing severity level used when this error is logged.
    pub fn with_level(mut self, level: Level) -> Self {
        self.0.level = level;
        self
    }

    /// Appends an outer execution step to the developer breadcrumb (deepest to outermost).
    pub fn context(mut self, step: impl Into<String>) -> Self {
        self.0.breadcrumb.push(step.into());
        self
    }

    /// Prepends the breadcrumb of an inner `DomainError` and marks it as consumed so it is not
    /// logged separately on drop.
    pub fn chain(mut self, mut previous: DomainError) -> Self {
        previous.0.logged.store(true, Ordering::Relaxed);
        let mut combined = std::mem::take(&mut previous.0.breadcrumb);
        combined.append(&mut self.0.breadcrumb);
        self.0.breadcrumb = combined;
        if self.0.trace_code.is_none() {
            self.0.trace_code = previous.0.trace_code;
        }
        self
    }

    pub fn status(&self) -> StatusCode {
        self.0.status
    }

    pub fn code(&self) -> &'static str {
        self.0.public_code
    }

    pub fn public_message(&self) -> &str {
        &self.0.public_message
    }

    pub fn trace_code(&self) -> Option<&'static str> {
        self.0.trace_code
    }

    pub fn breadcrumb(&self) -> &[String] {
        &self.0.breadcrumb
    }

    pub fn level(&self) -> Level {
        self.0.level
    }

    /// Emits the error once using its configured tracing level.
    pub fn log(&self) {
        self.emit(self.0.level);
    }

    /// Emits the error once forcing `INFO` level.
    pub fn info(&self) {
        self.emit(Level::INFO);
    }

    /// Emits the error once forcing `WARN` level.
    pub fn warn(&self) {
        self.emit(Level::WARN);
    }

    /// Emits the error once forcing `ERROR` level.
    pub fn error(&self) {
        self.emit(Level::ERROR);
    }

    fn emit(&self, level: Level) {
        if self.0.logged.swap(true, Ordering::Relaxed) {
            return;
        }
        let status_code = self.0.status.as_u16();
        let cause = self.0.public_code;
        let code = self.0.trace_code.unwrap_or(cause);
        let breadcrumb = &self.0.breadcrumb;
        match level {
            Level::ERROR => tracing::error!(
                http.status_code = status_code,
                error.cause = cause,
                error.code = code,
                error.breadcrumb = ?breadcrumb,
                "Operation failed"
            ),
            Level::WARN => tracing::warn!(
                http.status_code = status_code,
                error.cause = cause,
                error.code = code,
                error.breadcrumb = ?breadcrumb,
                "Operation rejected"
            ),
            Level::INFO | Level::DEBUG | Level::TRACE => tracing::info!(
                http.status_code = status_code,
                error.cause = cause,
                error.code = code,
                error.breadcrumb = ?breadcrumb,
                "Operation rejected"
            ),
        }
    }
}

impl Drop for DomainError {
    fn drop(&mut self) {
        self.emit(self.0.level);
    }
}

fn default_public_code(status: StatusCode) -> &'static str {
    match status {
        StatusCode::BAD_REQUEST => "INVALID_INPUT",
        StatusCode::UNAUTHORIZED => "UNAUTHORIZED",
        StatusCode::FORBIDDEN => "FORBIDDEN",
        StatusCode::NOT_FOUND => "NOT_FOUND",
        StatusCode::METHOD_NOT_ALLOWED => "METHOD_NOT_ALLOWED",
        StatusCode::NOT_ACCEPTABLE => "NOT_ACCEPTABLE",
        StatusCode::CONFLICT => "ALREADY_EXISTS",
        StatusCode::PAYLOAD_TOO_LARGE => "PAYLOAD_TOO_LARGE",
        StatusCode::UNSUPPORTED_MEDIA_TYPE => "UNSUPPORTED_MEDIA_TYPE",
        StatusCode::UNPROCESSABLE_ENTITY => "BUSINESS_RULE_VIOLATION",
        StatusCode::TOO_MANY_REQUESTS => "TOO_MANY_REQUESTS",
        StatusCode::SERVICE_UNAVAILABLE => "SERVICE_UNAVAILABLE",
        StatusCode::GATEWAY_TIMEOUT => "TIMEOUT",
        _ => "INTERNAL_ERROR",
    }
}

fn default_public_message(status: StatusCode) -> &'static str {
    match status {
        StatusCode::BAD_REQUEST => "The request is invalid.",
        StatusCode::UNAUTHORIZED => "A valid authentication credential is required.",
        StatusCode::FORBIDDEN => "You do not have permission to perform this operation.",
        StatusCode::NOT_FOUND => "The requested resource was not found.",
        StatusCode::METHOD_NOT_ALLOWED => "The HTTP method is not supported for this resource.",
        StatusCode::NOT_ACCEPTABLE => "The requested response media type is not supported.",
        StatusCode::CONFLICT => "The resource already exists.",
        StatusCode::PAYLOAD_TOO_LARGE => "The request body exceeds the maximum allowed size.",
        StatusCode::UNSUPPORTED_MEDIA_TYPE => "The request media type is not supported.",
        StatusCode::UNPROCESSABLE_ENTITY => "The operation is not allowed in the current state.",
        StatusCode::TOO_MANY_REQUESTS => "Too many requests. Please retry later.",
        StatusCode::SERVICE_UNAVAILABLE => {
            "A required service is temporarily unavailable. Please retry later."
        }
        StatusCode::GATEWAY_TIMEOUT => "The request took too long to complete. Please retry.",
        _ => "An internal error occurred. Use the trace_id to report it.",
    }
}

fn default_level(status: StatusCode) -> Level {
    if status.is_server_error() {
        Level::ERROR
    } else if matches!(
        status,
        StatusCode::UNAUTHORIZED
            | StatusCode::FORBIDDEN
            | StatusCode::METHOD_NOT_ALLOWED
            | StatusCode::UNPROCESSABLE_ENTITY
    ) {
        Level::WARN
    } else {
        Level::INFO
    }
}

/// Internal view: code and breadcrumb trail. Formatting an error never yields only its public text.
impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {}",
            self.0.trace_code.unwrap_or(self.0.public_code),
            self.0.breadcrumb.join(" -> ")
        )
    }
}

impl fmt::Debug for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, formatter)
    }
}

impl std::error::Error for DomainError {}

/// Appends an execution breadcrumb while an error propagates: `repo.find(id).await.context("loading order")?`.
pub trait ResultExt<T> {
    fn context(self, step: impl Into<String>) -> DomainResult<T>;
}

impl<T> ResultExt<T> for DomainResult<T> {
    fn context(self, step: impl Into<String>) -> DomainResult<T> {
        self.map_err(|error| error.context(step))
    }
}
