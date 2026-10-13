use axum::extract::{FromRequestParts, Path};
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use crate::infrastructure::http::error::{ApiError, HttpRejection};

/// Preserves Axum's distinction between invalid input (400) and broken handler wiring (500).
#[derive(Debug, Clone, Copy, Default)]
pub struct PathParam<T>(pub T);

impl<T, S> FromRequestParts<S> for PathParam<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(value) = Path::<T>::from_request_parts(parts, state).await.map_err(|error| {
            let rejection = if error.status().is_server_error() {
                HttpRejection::PathConfiguration
            } else {
                HttpRejection::InvalidPath
            };
            ApiError::from(rejection)
        })?;
        Ok(Self(value))
    }
}
