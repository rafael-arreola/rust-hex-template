//! Generic JWT authentication. Issuer-specific structures provide extraction, not verification.

pub mod engine;
mod jwks;

use serde::Deserialize;

use crate::domain::core::error::{DomainError, DomainResult, StatusCode};

pub use engine::{AuthClaims, AuthEngine, BuildAuthProvider, VerifiedJwt};

/// Limits the raw credential before header decoding or any JWKS refresh.
pub const MAX_CREDENTIAL_BYTES: usize = 16 * 1024;

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
            .ok_or_else(|| {
                DomainError::new(
                    StatusCode::UNAUTHORIZED,
                    "Auth0 JWT has no OAuth client identifier",
                )
                .with_trace_code("AUTH0_CLIENT_ID_MISSING")
            })
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

#[derive(Deserialize)]
#[serde(untagged)]
pub enum HydraScopes {
    Delimited(String),
    Listed(Vec<String>),
}

impl HydraScopes {
    /// Scopes remain data; a custom AuthClaims implementation may explicitly map them to permissions.
    pub fn tokens(&self) -> Vec<&str> {
        match self {
            Self::Delimited(scopes) => scopes.split_whitespace().collect(),
            Self::Listed(scopes) => {
                scopes.iter().map(|scope| scope.trim()).filter(|scope| !scope.is_empty()).collect()
            }
        }
    }
}

#[derive(Deserialize)]
pub struct HydraExt {
    #[serde(default)]
    pub client_id: Option<String>,
}

/// Strict by default. The M2M type parameter is selected in code, never by an unverified claim.
#[derive(Deserialize)]
pub struct HydraClaims<const MACHINE_TO_MACHINE: bool = false> {
    pub sub: String,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub ext: Option<HydraExt>,
    #[serde(default)]
    pub scp: Option<HydraScopes>,
}

/// Explicit MachineToMachineSubject profile; the builder requires a non-empty client allowlist.
pub type HydraMachineToMachineClaims = HydraClaims<true>;

impl<const MACHINE_TO_MACHINE: bool> AuthClaims for HydraClaims<MACHINE_TO_MACHINE> {
    const REQUIRES_CLIENT_ALLOWLIST: bool = MACHINE_TO_MACHINE;

    fn get_client_id(&self) -> DomainResult<&str> {
        let explicit = [
            self.client_id.as_deref(),
            self.ext.as_ref().and_then(|extension| extension.client_id.as_deref()),
        ]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|identifier| !identifier.is_empty());
        if let Some(identifier) = explicit {
            return Ok(identifier);
        }
        if MACHINE_TO_MACHINE {
            return Ok(self.sub.trim());
        }
        Err(DomainError::new(StatusCode::UNAUTHORIZED, "Hydra JWT has no OAuth client identifier")
            .with_trace_code("HYDRA_CLIENT_ID_MISSING"))
    }
}
