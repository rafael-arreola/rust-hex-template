use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
};
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::infrastructure::http::error::{ApiError, HttpRejection};
use crate::infrastructure::http::format::BodyFormat;

/// Deserializes supported JSON/MessagePack media types and executes `Validate`.
/// MessagePack input remains supported when response negotiation is disabled.
#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedBody<T>(pub T);

impl<T, S> FromRequest<S> for ValidatedBody<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let format = BodyFormat::of_request(req.headers())?;
        let bytes = Bytes::from_request(req, state).await.map_err(ApiError::from)?;
        let value: T = format.decode(&bytes)?;

        value.validate().map_err(|_| ApiError::from(HttpRejection::BodyValidation))?;
        Ok(Self(value))
    }
}
