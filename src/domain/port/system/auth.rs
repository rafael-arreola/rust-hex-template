use async_trait::async_trait;

use crate::domain::core::error::DomainResult;
use crate::domain::core::identity::AuthPrincipal;

/// Credential verification selected at startup, independent of HTTP and the identity provider.
#[async_trait]
pub trait AuthProvider: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    /// Receives only the raw credential, without an Authorization scheme or surrounding spaces.
    async fn authenticate(&self, token: &str) -> DomainResult<AuthPrincipal>;
}
