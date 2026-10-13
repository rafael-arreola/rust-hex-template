use std::collections::HashSet;
use std::marker::PhantomData;

use async_trait::async_trait;
use jsonwebtoken::Algorithm;
use serde::de::DeserializeOwned;

use super::jwks::{JwksConfig, JwksVerifier, RegisteredClaims};
use crate::domain::core::error::{DomainError, DomainResult, StatusCode};
use crate::domain::core::identity::{Actor, AuthPrincipal};
use crate::domain::port::system::auth::AuthProvider;
use crate::shared::http_client::HttpClient;

/// Interprets issuer-specific claims only after the engine verifies the complete JWT.
pub trait AuthClaims: DeserializeOwned + Send + Sync + 'static {
    /// Profiles deriving the client from a machine subject must require an explicit allowlist.
    const REQUIRES_CLIENT_ALLOWLIST: bool = false;

    fn get_client_id(&self) -> DomainResult<&str>;

    /// A subject is not automatically a human user, especially for client-credentials tokens.
    fn get_user(&self) -> Option<&str> {
        None
    }

    fn get_permissions(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Verified claims, not an authorization decision. Does not retain the credential or expose Debug.
pub struct VerifiedJwt<X> {
    registered: RegisteredClaims,
    claims: X,
}

impl<X> VerifiedJwt<X> {
    pub fn claims(&self) -> &X {
        &self.claims
    }

    pub fn subject(&self) -> &str {
        &self.registered.sub
    }

    pub fn issuer(&self) -> Option<&str> {
        self.registered.iss.as_deref()
    }

    pub fn expires_at(&self) -> u64 {
        self.registered.exp
    }

    pub fn into_claims(self) -> X {
        self.claims
    }
}

/// Explicit bootstrap configuration; constructing the builder performs no network I/O.
pub struct BuildAuthProvider<X> {
    name: Option<&'static str>,
    jwks_url: Option<String>,
    issuer: Option<String>,
    audiences: Vec<String>,
    algorithm: Algorithm,
    allowed_client_ids: Vec<String>,
    claims: PhantomData<fn() -> X>,
}

impl<X> Default for BuildAuthProvider<X> {
    fn default() -> Self {
        Self::new()
    }
}

impl<X> BuildAuthProvider<X> {
    pub fn new() -> Self {
        Self {
            name: None,
            jwks_url: None,
            issuer: None,
            audiences: Vec::new(),
            algorithm: Algorithm::RS256,
            allowed_client_ids: Vec::new(),
            claims: PhantomData,
        }
    }

    pub fn name(mut self, name: &'static str) -> Self {
        self.name = Some(name);
        self
    }

    pub fn jwks_url(mut self, url: impl Into<String>) -> Self {
        self.jwks_url = Some(url.into());
        self
    }

    pub fn issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuer = Some(issuer.into());
        self
    }

    pub fn audiences(mut self, audiences: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.audiences = audiences.into_iter().map(Into::into).collect();
        self
    }

    /// Only RS256 is supported; other algorithms fail during build, never degrade verification.
    pub fn algorithm(mut self, algorithm: Algorithm) -> Self {
        self.algorithm = algorithm;
        self
    }

    pub fn allowed_client_ids(
        mut self,
        clients: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.allowed_client_ids = clients.into_iter().map(Into::into).collect();
        self
    }
}

impl<X: AuthClaims> BuildAuthProvider<X> {
    #[tracing::instrument(skip_all)]
    pub async fn build(self, http: HttpClient) -> DomainResult<AuthEngine<X>> {
        let name = self
            .name
            .filter(|name| {
                !name.is_empty() && name.trim() == *name && !name.chars().any(char::is_control)
            })
            .ok_or_else(|| {
                DomainError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Authentication requires a non-empty provider name",
                )
                .with_trace_code("AUTH_PROVIDER_NAME_INVALID")
            })?;
        if self.allowed_client_ids.iter().any(|client| {
            client.trim().is_empty()
                || client.trim() != client
                || client.chars().any(char::is_control)
        }) {
            return Err(DomainError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "JWT client allowlist contains an invalid identifier",
            )
            .with_trace_code("AUTH_ALLOWLIST_INVALID"));
        }
        if X::REQUIRES_CLIENT_ALLOWLIST && self.allowed_client_ids.is_empty() {
            return Err(DomainError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "JWT claims profile requires a client allowlist",
            )
            .with_trace_code("AUTH_ALLOWLIST_REQUIRED"));
        }
        let config = JwksConfig {
            jwks_url: self.jwks_url.ok_or_else(|| {
                DomainError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "JWT configuration requires a JWKS endpoint",
                )
                .with_trace_code("AUTH_JWKS_URL_MISSING")
            })?,
            issuer: self.issuer,
            audiences: self.audiences,
            algorithm: self.algorithm,
        };
        let verifier = JwksVerifier::new(config, http).await?;
        Ok(AuthEngine {
            name,
            verifier,
            allowed_client_ids: self.allowed_client_ids.into_iter().collect(),
            claims: PhantomData,
        })
    }
}

/// One authentication implementation for every issuer-specific claims structure.
pub struct AuthEngine<X> {
    name: &'static str,
    verifier: JwksVerifier,
    allowed_client_ids: HashSet<String>,
    claims: PhantomData<fn() -> X>,
}

impl<X: AuthClaims> AuthEngine<X> {
    /// Verifies the JWT and schema without applying client allowlists or route permissions.
    #[tracing::instrument(skip_all, fields(provider = self.name))]
    pub async fn verify(&self, credential: &str) -> DomainResult<VerifiedJwt<X>> {
        let (registered, claims) = self.verifier.verify(credential).await?;
        Ok(VerifiedJwt { registered, claims })
    }
}

#[async_trait]
impl<X: AuthClaims> AuthProvider for AuthEngine<X> {
    fn name(&self) -> &'static str {
        self.name
    }

    #[tracing::instrument(skip_all, fields(provider = self.name))]
    async fn authenticate(&self, credential: &str) -> DomainResult<AuthPrincipal> {
        let verified = self.verify(credential).await?;
        let claims = verified.claims();
        let client_id = claims.get_client_id()?.trim();
        if client_id.is_empty() || client_id.chars().any(char::is_control) {
            return Err(DomainError::new(
                StatusCode::UNAUTHORIZED,
                "JWT has an invalid OAuth client identifier",
            )
            .with_trace_code("JWT_CLIENT_ID_INVALID"));
        }
        if !self.allowed_client_ids.is_empty() && !self.allowed_client_ids.contains(client_id) {
            return Err(DomainError::new(
                StatusCode::FORBIDDEN,
                "Authenticated OAuth client is not allowed",
            )
            .with_trace_code("JWT_CLIENT_NOT_ALLOWED"));
        }
        let user_id = claims
            .get_user()
            .map(|user| {
                let user = user.trim();
                if user.is_empty() || user.chars().any(char::is_control) {
                    Err(DomainError::new(
                        StatusCode::UNAUTHORIZED,
                        "JWT has an invalid user identifier",
                    )
                    .with_trace_code("JWT_USER_ID_INVALID"))
                } else {
                    Ok(user.to_owned())
                }
            })
            .transpose()?;
        let permissions = claims
            .get_permissions()
            .into_iter()
            .map(|permission| permission.trim().to_owned())
            .filter(|permission| !permission.is_empty())
            .collect();
        Ok(AuthPrincipal {
            actor: Actor {
                provider: self.name,
                client_id: client_id.to_owned(),
                subject: Some(verified.subject().to_owned()),
                user_id,
            },
            permissions,
        })
    }
}
