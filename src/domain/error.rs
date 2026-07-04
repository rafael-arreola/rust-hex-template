use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("{entity} not found: {id}")]
    NotFound { entity: &'static str, id: String },

    #[error("{entity} already exists: {details}")]
    AlreadyExists { entity: &'static str, details: String },

    #[error("Invalid {field}: {reason}")]
    Invalid { field: &'static str, reason: String },

    #[error("{field} is required")]
    Required { field: &'static str },

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Business rule violated: {0}")]
    BusinessRule(String),

    #[error("External service error: {service} - {message}")]
    ExternalService { service: String, message: String },

    #[error("Database error: {0}")]
    Database(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl DomainError {
    /// Returns a stable, machine-readable code for every error variant.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "NOT_FOUND",
            Self::AlreadyExists { .. } => "ALREADY_EXISTS",
            Self::Invalid { .. } => "INVALID_INPUT",
            Self::Required { .. } => "REQUIRED_FIELD",
            Self::Unauthorized(_) => "UNAUTHORIZED",
            Self::Forbidden(_) => "FORBIDDEN",
            Self::BusinessRule(_) => "BUSINESS_RULE_VIOLATION",
            Self::ExternalService { .. } => "EXTERNAL_SERVICE_UNAVAILABLE",
            Self::Database(_) => "INTERNAL_ERROR",
            Self::Internal(_) => "INTERNAL_ERROR",
        }
    }

    /// Client-safe message. `Display`/`to_string()` remains the INTERNAL
    /// message (full detail, logs only); this is the only text allowed to
    /// cross a driving boundary.
    pub fn public_message(&self) -> String {
        match self {
            // Built from the client's own data — safe as-is.
            Self::NotFound { .. }
            | Self::AlreadyExists { .. }
            | Self::Invalid { .. }
            | Self::Required { .. }
            | Self::Unauthorized(_)
            | Self::Forbidden(_)
            | Self::BusinessRule(_) => self.to_string(),

            // Carry infrastructure detail — clients get a generic view.
            Self::ExternalService { service, .. } => {
                format!("The '{service}' service is currently unavailable. Please retry later.")
            }
            Self::Database(_) | Self::Internal(_) => {
                "An internal error occurred. Use the trace_id to report it.".to_string()
            }
        }
    }

    /// Severity the driving boundary must log this error with.
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::NotFound { .. }
            | Self::AlreadyExists { .. }
            | Self::Invalid { .. }
            | Self::Required { .. } => ErrorSeverity::Info,
            Self::Unauthorized(_) | Self::Forbidden(_) | Self::BusinessRule(_) => {
                ErrorSeverity::Warn
            }
            Self::ExternalService { .. } | Self::Database(_) | Self::Internal(_) => {
                ErrorSeverity::Error
            }
        }
    }

    pub fn not_found(entity: &'static str, id: impl Into<String>) -> Self {
        Self::NotFound { entity, id: id.into() }
    }

    pub fn duplicate(entity: &'static str, field: &'static str, value: impl Into<String>) -> Self {
        Self::AlreadyExists {
            entity,
            details: format!("{} '{}' already in use", field, value.into()),
        }
    }

    pub fn invalid_param(
        param: &'static str,
        entity: &'static str,
        value: impl Into<String>,
    ) -> Self {
        Self::Invalid { field: param, reason: format!("Invalid {} ID: {}", entity, value.into()) }
    }

    pub fn business_rule(message: impl Into<String>) -> Self {
        Self::BusinessRule(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    pub fn database(message: impl Into<String>) -> Self {
        Self::Database(message.into())
    }

    /// Constructor: failure reported by an external dependency (always mapped,
    /// never propagated raw).
    pub fn external_service(service: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ExternalService { service: service.into(), message: message.into() }
    }
}

/// Log severity a driving boundary must use when reporting a [`DomainError`].
///
/// Declared here — next to [`DomainError::code`] — so every adapter (HTTP,
/// pubsub, gRPC) logs the same error with the same severity, decided once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// Expected, client-caused failures (missing lookups, bad input).
    Info,
    /// Business/security signals worth operator attention.
    Warn,
    /// Internal failures that require action.
    Error,
}

pub type DomainResult<T> = std::result::Result<T, DomainError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infrastructure_detail_never_leaks_into_public_message() {
        let secret = "mongodb://user:pass@10.0.0.5:27017 connection refused";

        for error in [
            DomainError::database(secret),
            DomainError::internal(secret),
            DomainError::external_service("payments", secret),
        ] {
            assert!(error.to_string().contains(secret), "internal view keeps full detail");
            assert!(
                !error.public_message().contains(secret),
                "public view must not carry internal detail: {}",
                error.public_message()
            );
        }
    }

    #[test]
    fn client_caused_errors_keep_their_display_text() {
        let error = DomainError::not_found("User", "u9");
        assert_eq!(error.public_message(), error.to_string());
    }

    #[test]
    fn severity_matches_error_nature() {
        assert_eq!(DomainError::not_found("User", "u9").severity(), ErrorSeverity::Info);
        assert_eq!(DomainError::business_rule("no stock").severity(), ErrorSeverity::Warn);
        assert_eq!(DomainError::database("boom").severity(), ErrorSeverity::Error);
        assert_eq!(
            DomainError::external_service("payments", "boom").severity(),
            ErrorSeverity::Error
        );
    }
}
