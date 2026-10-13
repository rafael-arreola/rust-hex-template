use std::collections::HashSet;
use std::fmt;

/// Verified identity that can cross application boundaries without carrying a credential.
#[derive(Clone, PartialEq, Eq)]
pub struct Actor {
    pub provider: &'static str,
    pub client_id: String,
    /// The issuer's subject is distinct from the OAuth client, including in M2M profiles.
    pub subject: Option<String>,
    /// Optional user extracted explicitly by the issuer profile; a subject may identify a machine.
    pub user_id: Option<String>,
}

impl fmt::Debug for Actor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Actor")
            .field("provider", &self.provider)
            .field("client_id", &"<redacted>")
            .field("subject", &self.subject.as_ref().map(|_| "<redacted>"))
            .field("user_id", &self.user_id.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

/// Authentication result; permissions are literal names, not patterns or business catalogs.
#[derive(Clone, PartialEq, Eq)]
pub struct AuthPrincipal {
    pub actor: Actor,
    pub permissions: HashSet<String>,
}

impl AuthPrincipal {
    pub fn has_permission(&self, permission: &str) -> bool {
        let permission = permission.trim();
        !permission.is_empty() && self.permissions.contains(permission)
    }

    /// Empty or malformed requirements never authorize, even for an ALL policy.
    pub fn has_all_permissions(&self, permissions: &[&str]) -> bool {
        !permissions.is_empty()
            && permissions.iter().all(|permission| self.has_permission(permission))
    }

    pub fn has_any_permission(&self, permissions: &[&str]) -> bool {
        !permissions.is_empty()
            && permissions.iter().all(|permission| !permission.trim().is_empty())
            && permissions.iter().any(|permission| self.has_permission(permission))
    }
}

impl fmt::Debug for AuthPrincipal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthPrincipal")
            .field("actor", &self.actor)
            .field("permission_count", &self.permissions.len())
            .finish()
    }
}
