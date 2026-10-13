use axum::extract::{FromRequestParts, Query};
use axum::http::request::Parts;
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::infrastructure::http::error::{ApiError, HttpRejection};

/// Both query deserialization and validation failures stay inside the HTTP envelope.
#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedQuery<T>(pub T);

impl<T, S> FromRequestParts<S> for ValidatedQuery<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(|_| ApiError::from(HttpRejection::InvalidQuery))?;

        value.validate().map_err(|_| ApiError::from(HttpRejection::QueryValidation))?;
        Ok(Self(value))
    }
}
