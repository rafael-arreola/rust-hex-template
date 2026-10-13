//! HTTP authentication and route-local authorization, separate from domain identity types.

use std::sync::Arc;

use axum::{
    extract::{FromRequestParts, Request, State},
    http::{HeaderMap, header::AUTHORIZATION, request::Parts},
    middleware::{self, Next},
    response::Response,
    routing::MethodRouter,
};

use super::context::ContextRequestExt;
use super::error::ApiError;
use crate::domain::core::error::{DomainError, StatusCode};
use crate::domain::core::identity::AuthPrincipal;
use crate::domain::port::system::auth::AuthProvider;

// Transport bound only; the authentication provider validates the credential's own format.
const MAX_AUTHORIZATION_HEADER_BYTES: usize = 16 * 1024 + 64;

/// Reads the principal inserted by `auth_middleware`; it never reads or retains a credential.
impl<S> FromRequestParts<S> for AuthPrincipal
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<AuthPrincipal>().cloned().ok_or_else(|| {
            DomainError::new(StatusCode::UNAUTHORIZED, "Missing authentication context")
                .with_trace_code("AUTH_CONTEXT_MISSING")
                .into()
        })
    }
}

/// Accepts exactly one `Authorization: Bearer <credential>` header and returns the credential.
fn extract_bearer_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    let mut values = headers.get_all(AUTHORIZATION).iter();
    let value = values.next().ok_or_else(|| {
        DomainError::new(StatusCode::UNAUTHORIZED, "Missing Authorization header")
            .with_trace_code("AUTH_HEADER_MISSING")
    })?;
    if values.next().is_some() || value.as_bytes().len() > MAX_AUTHORIZATION_HEADER_BYTES {
        return Err(DomainError::new(StatusCode::UNAUTHORIZED, "Unsupported Authorization header")
            .with_trace_code("AUTH_HEADER_UNSUPPORTED")
            .into());
    }

    let (scheme, credential) = value
        .to_str()
        .map_err(|_| {
            DomainError::new(StatusCode::UNAUTHORIZED, "Malformed Authorization header")
                .with_trace_code("AUTH_HEADER_MALFORMED")
        })?
        .trim()
        .split_once(|character: char| character.is_ascii_whitespace())
        .ok_or_else(|| {
            DomainError::new(StatusCode::UNAUTHORIZED, "Authorization requires a Bearer credential")
                .with_trace_code("AUTH_BEARER_REQUIRED")
        })?;
    if !scheme.eq_ignore_ascii_case("Bearer") {
        return Err(DomainError::new(StatusCode::UNAUTHORIZED, "Unsupported Authorization scheme")
            .with_trace_code("AUTH_SCHEME_UNSUPPORTED")
            .into());
    }
    Ok(credential.trim())
}

#[tracing::instrument(skip_all, fields(provider = provider.name()))]
pub async fn auth_middleware(
    State(provider): State<Arc<dyn AuthProvider>>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let credential = extract_bearer_token(request.headers())?;
    let principal = provider.authenticate(credential).await?;
    request.headers_mut().remove(AUTHORIZATION);
    // The actor also travels in the operation context, so services read it with `ctx.get::<Actor>()`.
    request.context().insert(principal.actor.clone());
    request.extensions_mut().insert(principal);
    Ok(next.run(request).await)
}

/// Route permission policy. Empty or blank requirements stay rejecting layers, never open routes.
#[derive(Clone, Copy)]
pub enum Policy {
    Single(&'static str),
    Any(&'static [&'static str]),
    All(&'static [&'static str]),
}

impl Policy {
    fn is_valid(self) -> bool {
        match self {
            Self::Single(permission) => !permission.trim().is_empty(),
            // Every member is validated, including for ANY: a malformed policy is not partial.
            Self::Any(permissions) | Self::All(permissions) => {
                !permissions.is_empty()
                    && permissions.iter().all(|permission| !permission.trim().is_empty())
            }
        }
    }

    fn accepts(self, principal: &AuthPrincipal) -> bool {
        match self {
            Self::Single(permission) => principal.has_permission(permission),
            Self::Any(permissions) => principal.has_any_permission(permissions),
            Self::All(permissions) => principal.has_all_permissions(permissions),
        }
    }
}

async fn require(
    State(policy): State<Policy>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if !policy.is_valid() {
        return Err(DomainError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Route authorization requires non-empty permissions",
        )
        .with_trace_code("AUTH_POLICY_INVALID")
        .into());
    }
    let principal = request.extensions().get::<AuthPrincipal>().ok_or_else(|| {
        DomainError::new(StatusCode::UNAUTHORIZED, "Missing authentication context")
            .with_trace_code("AUTH_CONTEXT_MISSING")
    })?;
    if !policy.accepts(principal) {
        return Err(DomainError::new(
            StatusCode::FORBIDDEN,
            "Authenticated principal lacks the required permissions",
        )
        .with_trace_code("AUTH_PERMISSION_DENIED")
        .into());
    }
    Ok(next.run(request).await)
}

/// Call after adding handlers, with authentication mounted outside the permission route layers.
pub trait RequirePermissionExt {
    fn require_policy(self, policy: Policy) -> Self;

    fn require_permission(self, permission: &'static str) -> Self
    where
        Self: Sized,
    {
        self.require_policy(Policy::Single(permission))
    }

    fn require_any_permission(self, permissions: &'static [&'static str]) -> Self
    where
        Self: Sized,
    {
        self.require_policy(Policy::Any(permissions))
    }

    fn require_all_permissions(self, permissions: &'static [&'static str]) -> Self
    where
        Self: Sized,
    {
        self.require_policy(Policy::All(permissions))
    }
}

impl<S> RequirePermissionExt for MethodRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn require_policy(self, policy: Policy) -> Self {
        self.route_layer(middleware::from_fn_with_state(policy, require))
    }
}
