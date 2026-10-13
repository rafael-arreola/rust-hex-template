//! Generic JWT authentication. Issuer-specific structures provide extraction, not verification.

pub mod auth0;
mod document;
pub mod engine;
pub mod hydra;
mod jwks;

pub use auth0::Auth0Claims;
pub use engine::{AuthClaims, AuthEngine, BuildAuthProvider, VerifiedJwt};
pub use hydra::{HydraClaims, HydraMachineToMachineClaims};

/// Limits the raw credential before header decoding or any JWKS refresh.
pub const MAX_CREDENTIAL_BYTES: usize = 16 * 1024;
