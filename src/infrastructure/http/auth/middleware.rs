use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{HeaderMap, header::AUTHORIZATION},
    middleware::Next,
    response::Response,
};

use crate::domain::core::error::DomainError;
use crate::domain::port::auth::AuthProvider;
use crate::infrastructure::http::context::ContextRequestExt;
use crate::infrastructure::http::error::ApiError;

// Transport bound only; the authentication provider validates the credential's own format.
const MAX_AUTHORIZATION_HEADER_BYTES: usize = 16 * 1024 + 64;

/// Accepts exactly one `Authorization: Bearer <credential>` header and returns the credential.
pub fn extract_bearer_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    let mut values = headers.get_all(AUTHORIZATION).iter();
    let value =
        values.next().ok_or_else(|| DomainError::unauthorized("Missing Authorization header"))?;
    if values.next().is_some() || value.as_bytes().len() > MAX_AUTHORIZATION_HEADER_BYTES {
        return Err(DomainError::unauthorized("Unsupported Authorization header").into());
    }

    let (scheme, credential) = value
        .to_str()
        .map_err(|_| DomainError::unauthorized("Malformed Authorization header"))?
        .trim()
        .split_once(|character: char| character.is_ascii_whitespace())
        .ok_or_else(|| DomainError::unauthorized("Authorization requires a Bearer credential"))?;
    if !scheme.eq_ignore_ascii_case("Bearer") {
        return Err(DomainError::unauthorized("Unsupported Authorization scheme").into());
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
