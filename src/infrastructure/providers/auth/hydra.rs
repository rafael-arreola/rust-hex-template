use serde::Deserialize;

use super::engine::AuthClaims;
use crate::domain::core::error::{DomainError, DomainResult};

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
        Err(DomainError::unauthorized("Hydra JWT has no OAuth client identifier"))
    }
}
