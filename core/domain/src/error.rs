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
}

pub type DomainResult<T> = std::result::Result<T, DomainError>;
