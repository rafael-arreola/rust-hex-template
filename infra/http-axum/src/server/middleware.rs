use axum::{
    body::Body,
    http::{HeaderName, HeaderValue, Request, StatusCode},
    response::Response,
};

/// Injects or propagates an `X-Request-Id` header on every request.
///
/// - Reads from the incoming `X-Request-Id` header if present.
/// - Otherwise generates a new UUIDv7.
/// - Attaches the value to the response header and the tracing span.
pub async fn request_id(mut req: Request<Body>, next: axum::middleware::Next) -> Response<Body> {
    let header_name = HeaderName::from_static("x-request-id");

    let id = req
        .headers()
        .get(&header_name)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());

    req.extensions_mut().insert(RequestId(id.clone()));

    tracing::Span::current().record("request_id", &id);

    let mut response = next.run(req).await;

    if response.status() != StatusCode::NOT_MODIFIED {
        response.headers_mut().insert(
            header_name,
            HeaderValue::from_str(&id).unwrap_or(HeaderValue::from_static("unknown")),
        );
    }

    response
}

/// Newtype stored in request extensions so downstream handlers can retrieve it.
#[derive(Clone, Debug)]
pub struct RequestId(pub String);
