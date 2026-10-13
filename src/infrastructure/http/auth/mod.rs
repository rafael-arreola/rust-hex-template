//! HTTP authentication and route-local authorization, separate from domain identity types.

mod middleware;
mod permissions;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::domain::core::error::DomainError;
use crate::domain::core::identity::AuthPrincipal;
use crate::infrastructure::http::error::ApiError;

pub use middleware::auth_middleware;
pub use permissions::{Policy, RequirePermissionExt};

/// Reads the principal inserted by `auth_middleware`; it never reads or retains a credential.
impl<S> FromRequestParts<S> for AuthPrincipal
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthPrincipal>()
            .cloned()
            .ok_or_else(|| DomainError::unauthorized("Missing authentication context").into())
    }
}
