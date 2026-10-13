use axum::{
    extract::{Request, State},
    middleware::{self, Next},
    response::Response,
    routing::MethodRouter,
};

use crate::domain::core::error::DomainError;
use crate::domain::core::identity::AuthPrincipal;
use crate::infrastructure::http::error::ApiError;

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

pub async fn require(
    State(policy): State<Policy>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if !policy.is_valid() {
        return Err(
            DomainError::internal("Route authorization requires non-empty permissions").into()
        );
    }
    let principal = request
        .extensions()
        .get::<AuthPrincipal>()
        .ok_or_else(|| DomainError::unauthorized("Missing authentication context"))?;
    if !policy.accepts(principal) {
        return Err(DomainError::forbidden(
            "Authenticated principal lacks the required permissions",
        )
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
