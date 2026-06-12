use axum::{
    Json,
    body::Bytes,
    extract::{FromRequest, Request},
    http::header,
};
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::infrastructure::driving::http_axum::server::error::ApiError;

/// Content-negotiating, validating body extractor.
///
/// Deserializes the request body straight into `T` according to
/// `Content-Type` — `application/vnd.msgpack` via MessagePack, anything else
/// via JSON — then runs the `validator` rules. Input negotiation is always
/// active, independent of the response negotiation flag.
#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedBody<T>(pub T);

impl<T, S> FromRequest<S> for ValidatedBody<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let is_msgpack = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|content_type| content_type.contains("msgpack"));

        let value: T = if is_msgpack {
            let bytes = Bytes::from_request(req, state)
                .await
                .map_err(|e| ApiError::bad_request(e.to_string()))?;

            rmp_serde::from_slice(&bytes)
                .map_err(|e| ApiError::bad_request(format!("Invalid MessagePack body: {}", e)))?
        } else {
            let Json(value) = Json::<T>::from_request(req, state)
                .await
                .map_err(|e| ApiError::bad_request(e.to_string()))?;
            value
        };

        value.validate().map_err(|e| ApiError::bad_request(e.to_string()))?;

        Ok(ValidatedBody(value))
    }
}
