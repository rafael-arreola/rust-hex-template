use serde::Deserialize;

use super::engine::AuthClaims;
use crate::domain::core::error::{DomainError, DomainResult};

/// Auth0's external schema; registered JWT claims are validated independently by the engine.
#[derive(Deserialize)]
pub struct Auth0Claims {
    // Legacy public namespaces remain aliases, not a dependency on an application catalog.
    #[serde(default, alias = "https://oauth.99minutos.com/cid")]
    pub oauth_cid: Option<String>,
    #[serde(default)]
    pub cid: Option<String>,
    #[serde(default)]
    pub azp: Option<String>,
    /// Explicit user identity supplied by the issuer, not inferred from an arbitrary subject.
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default, alias = "https://oauth.99minutos.com/permissions")]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

impl AuthClaims for Auth0Claims {
    fn get_client_id(&self) -> DomainResult<&str> {
        [self.oauth_cid.as_deref(), self.cid.as_deref(), self.azp.as_deref()]
            .into_iter()
            .flatten()
            .map(str::trim)
            .find(|identifier| !identifier.is_empty())
            .ok_or_else(|| DomainError::unauthorized("Auth0 JWT has no OAuth client identifier"))
    }

    fn get_user(&self) -> Option<&str> {
        self.user_id.as_deref()
    }

    fn get_permissions(&self) -> Vec<String> {
        let mut permissions = self.permissions.clone();
        if let Some(scope) = &self.scope {
            permissions.extend(scope.split_whitespace().map(str::to_owned));
        }
        permissions
    }
}
