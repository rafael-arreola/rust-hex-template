use std::borrow::Cow;
use std::fmt;

/// Closed failure category. It decides the default code, public message and severity; adapters map
/// it to their own status (HTTP, task retry) without the domain knowing any transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    NotFound,
    AlreadyExists,
    Invalid,
    Unauthorized,
    Forbidden,
    BusinessRule,
    Unavailable,
    Timeout,
    Internal,
}

impl ErrorKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::NotFound => "NOT_FOUND",
            Self::AlreadyExists => "ALREADY_EXISTS",
            Self::Invalid => "INVALID_INPUT",
            Self::Unauthorized => "UNAUTHORIZED",
            Self::Forbidden => "FORBIDDEN",
            Self::BusinessRule => "BUSINESS_RULE_VIOLATION",
            Self::Unavailable => "SERVICE_UNAVAILABLE",
            Self::Timeout => "TIMEOUT",
            Self::Internal => "INTERNAL_ERROR",
        }
    }

    pub fn severity(self) -> ErrorSeverity {
        match self {
            Self::NotFound | Self::AlreadyExists | Self::Invalid => ErrorSeverity::Info,
            Self::Unauthorized | Self::Forbidden | Self::BusinessRule => ErrorSeverity::Warn,
            Self::Unavailable | Self::Timeout | Self::Internal => ErrorSeverity::Error,
        }
    }

    fn default_public_message(self) -> &'static str {
        match self {
            Self::NotFound => "The requested resource was not found.",
            Self::AlreadyExists => "The resource already exists.",
            Self::Invalid => "The request is invalid.",
            Self::Unauthorized => "A valid authentication credential is required.",
            Self::Forbidden => "You do not have permission to perform this operation.",
            Self::BusinessRule => "The operation is not allowed in the current state.",
            Self::Unavailable => {
                "A required service is temporarily unavailable. Please retry later."
            }
            Self::Timeout => "The request took too long to complete. Please retry.",
            Self::Internal => "An internal error occurred. Use the trace_id to report it.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Info,
    Warn,
    Error,
}

#[derive(Debug)]
struct Inner {
    kind: ErrorKind,
    code: &'static str,
    public: Cow<'static, str>,
    internal: Option<String>,
    context: Vec<Cow<'static, str>>,
}

/// Failure with two separate views:
/// - Public: `code` and `public_message()`, the only data a client ever receives.
/// - Internal: `internal_detail()` and `context()`, only for logs.
///
/// Constructors name where their text goes. Client failures (`invalid`, `business_rule`, ...)
/// take public text; server and authentication failures (`internal`, `unauthorized`, ...) take
/// internal text and answer with their category's generic message. Neither view may carry raw
/// driver or provider messages, credentials or personal data. Boxed, so `DomainResult<T>` stays
/// one pointer wide on the error path.
pub struct DomainError(Box<Inner>);

pub type DomainResult<T> = Result<T, DomainError>;

impl DomainError {
    pub fn new(kind: ErrorKind) -> Self {
        Self(Box::new(Inner {
            kind,
            code: kind.code(),
            public: Cow::Borrowed(kind.default_public_message()),
            internal: None,
            context: Vec::new(),
        }))
    }

    /// Public: `"<entity> not found"`. The identifier only reaches logs.
    pub fn not_found(entity: &'static str, id: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound)
            .with_public(format!("{entity} not found"))
            .with_internal(format!("{entity} id {}", id.into()))
    }

    /// Public: the entity and field in conflict, never the conflicting value.
    pub fn duplicate(entity: &'static str, field: &'static str) -> Self {
        Self::new(ErrorKind::AlreadyExists)
            .with_public(format!("{entity} already exists: {field} is already in use"))
    }

    /// Public: `"Invalid <field>: <reason>"`. Keep received values out of `reason`; add them with
    /// `with_internal` when they help diagnose.
    pub fn invalid(field: &'static str, reason: impl Into<Cow<'static, str>>) -> Self {
        Self::new(ErrorKind::Invalid).with_public(format!("Invalid {field}: {}", reason.into()))
    }

    pub fn required(field: &'static str) -> Self {
        Self::new(ErrorKind::Invalid)
            .with_code("REQUIRED_FIELD")
            .with_public(format!("{field} is required"))
    }

    pub fn invalid_param(param: &'static str, entity: &'static str) -> Self {
        Self::invalid(param, format!("Invalid {entity} identifier"))
    }

    /// Public: `message`. Give the rule its own code with `with_code` when clients branch on it.
    pub fn business_rule(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new(ErrorKind::BusinessRule).with_public(message)
    }

    /// Internal: `detail`. The client only learns that a credential is required.
    pub fn unauthorized(detail: impl Into<String>) -> Self {
        Self::new(ErrorKind::Unauthorized).with_internal(detail)
    }

    /// Internal: `detail`. The client only learns that permission is missing.
    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self::new(ErrorKind::Forbidden).with_internal(detail)
    }

    /// Internal: the dependency and what failed.
    pub fn service_unavailable(service: &'static str, detail: impl Into<String>) -> Self {
        Self::new(ErrorKind::Unavailable).with_internal(format!("{service}: {}", detail.into()))
    }

    /// Internal: the operation that ran out of time.
    pub fn timeout(operation: impl Into<String>) -> Self {
        Self::new(ErrorKind::Timeout).with_internal(operation)
    }

    /// Internal: `detail`. The client only receives the generic message and its trace_id.
    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal).with_internal(detail)
    }

    /// Replaces the stable code the client receives, e.g. `INSUFFICIENT_STOCK`.
    pub fn with_code(mut self, code: &'static str) -> Self {
        self.0.code = code;
        self
    }

    /// Replaces the message the client receives; it must be safe to show to anyone.
    pub fn with_public(mut self, message: impl Into<Cow<'static, str>>) -> Self {
        self.0.public = message.into();
        self
    }

    /// Sets the diagnostic detail written to logs and never returned to the client.
    pub fn with_internal(mut self, detail: impl Into<String>) -> Self {
        self.0.internal = Some(detail.into());
        self
    }

    /// Appends where the failure passed through, innermost first; logs only.
    pub fn context(mut self, context: impl Into<Cow<'static, str>>) -> Self {
        self.0.context.push(context.into());
        self
    }

    pub fn kind(&self) -> ErrorKind {
        self.0.kind
    }

    pub fn code(&self) -> &'static str {
        self.0.code
    }

    pub fn severity(&self) -> ErrorSeverity {
        self.0.kind.severity()
    }

    pub fn public_message(&self) -> &str {
        &self.0.public
    }

    pub fn internal_detail(&self) -> Option<&str> {
        self.0.internal.as_deref()
    }

    pub fn context_trail(&self) -> &[Cow<'static, str>] {
        &self.0.context
    }

    /// Logs the internal view of a failure that never reaches a transport boundary (background
    /// work, compensations). Errors returned to a handler are logged there instead; do not do both.
    pub fn log(&self) {
        let internal = self.internal_detail().unwrap_or(self.public_message());
        let context = self.context_trail().join(" -> ");
        match self.severity() {
            ErrorSeverity::Error => {
                tracing::error!(error.code = self.code(), error.internal = %internal, error.context = %context, "Operation failed");
            }
            ErrorSeverity::Warn => {
                tracing::warn!(error.code = self.code(), error.internal = %internal, error.context = %context, "Operation rejected");
            }
            ErrorSeverity::Info => {
                tracing::info!(error.code = self.code(), error.internal = %internal, error.context = %context, "Operation rejected");
            }
        }
    }
}

/// Internal view: code, detail and context. Formatting an error never yields only its public text.
impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {}",
            self.code(),
            self.internal_detail().unwrap_or(self.public_message())
        )?;
        if !self.0.context.is_empty() {
            write!(formatter, " | context: {}", self.0.context.join(" -> "))?;
        }
        Ok(())
    }
}

impl fmt::Debug for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, formatter)
    }
}

impl std::error::Error for DomainError {}

/// Adds internal context while an error propagates: `repo.find(id).await.context("loading order")?`.
pub trait ResultExt<T> {
    fn context(self, context: impl Into<Cow<'static, str>>) -> DomainResult<T>;
}

impl<T> ResultExt<T> for DomainResult<T> {
    fn context(self, context: impl Into<Cow<'static, str>>) -> DomainResult<T> {
        self.map_err(|error| error.context(context))
    }
}
