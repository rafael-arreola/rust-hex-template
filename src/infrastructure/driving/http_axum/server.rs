pub mod error;
pub mod health;
pub mod middleware;
pub mod response;
pub mod state;
pub mod validation;

use axum::Router;
use axum::{
    body::Body,
    extract::DefaultBodyLimit,
    http::{HeaderValue, Request, Response, header},
    routing::get,
};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::signal;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    decompression::RequestDecompressionLayer,
};

use self::health::{healthz, readyz};
use self::response::NegotiablePayload;
use self::state::AppState;
use crate::infrastructure::driving::http_axum::routes;

/// Maximum request body size enforced by the body-limit layer.
const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone)]
pub struct ServerLauncher {
    state: AppState,
    http_port: Option<u16>,
    cors_origins: Option<String>,
    drain_timeout: Duration,
    msgpack_enabled: bool,
}

impl ServerLauncher {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            http_port: None,
            cors_origins: None,
            drain_timeout: Duration::from_secs(10),
            msgpack_enabled: true,
        }
    }

    /// Toggles `Accept: application/vnd.msgpack` content negotiation.
    /// On by default; the swap only costs an extra encode on requests that
    /// actually ask for MessagePack. Disable via `ENABLE_MSGPACK=false`.
    pub fn with_msgpack(mut self, enabled: bool) -> Self {
        self.msgpack_enabled = enabled;
        self
    }

    pub fn with_http(mut self, port: u16) -> Self {
        self.http_port = Some(port);
        self
    }

    pub fn with_cors_origins(mut self, cors_origins: String) -> Self {
        self.cors_origins = Some(cors_origins);
        self
    }

    pub fn with_drain_timeout(mut self, secs: u64) -> Self {
        self.drain_timeout = Duration::from_secs(secs);
        self
    }

    pub async fn run(self) {
        if let Some(port) = self.http_port {
            let state = self.state.clone();
            let drain_timeout = self.drain_timeout;

            let cors_origins_str = self.cors_origins.unwrap_or_else(|| "*".to_string());

            let cors = if cors_origins_str == "*" {
                CorsLayer::permissive().allow_methods(Any).allow_headers(Any)
            } else {
                let origins: Vec<_> =
                    cors_origins_str.split(',').filter_map(|s| s.parse().ok()).collect();

                CorsLayer::new().allow_methods(Any).allow_headers(Any).allow_origin(origins)
            };

            let mut rest_router = Router::new()
                .route("/healthz", get(healthz))
                .route("/readyz", get(readyz))
                .nest("/api/v1", routes::app_router());

            if self.msgpack_enabled {
                rest_router = rest_router.layer(axum::middleware::from_fn(msgpack_negotiation));
            }

            let rest_router = rest_router
                .layer(axum::middleware::from_fn(middleware::trace_context))
                .layer(CompressionLayer::new())
                .layer(RequestDecompressionLayer::new())
                .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
                .layer(cors)
                .with_state(state);

            let rest_addr = SocketAddr::from(([0, 0, 0, 0], port));
            tracing::info!(
                "REST Server listening on {} (drain_timeout={:?})",
                rest_addr,
                drain_timeout,
            );

            match tokio::net::TcpListener::bind(rest_addr).await {
                Ok(listener) => {
                    // Draining starts the moment the signal arrives; in-flight
                    // connections get at most `drain_timeout` to finish.
                    let (drain_started_tx, drain_started_rx) = tokio::sync::oneshot::channel();

                    let graceful =
                        axum::serve(listener, rest_router).with_graceful_shutdown(async move {
                            shutdown_signal("REST").await;
                            let _ = drain_started_tx.send(());
                        });

                    let drain_deadline = async move {
                        if drain_started_rx.await.is_ok() {
                            tokio::time::sleep(drain_timeout).await;
                        } else {
                            std::future::pending::<()>().await;
                        }
                    };

                    tokio::select! {
                        result = graceful => match result {
                            Ok(()) => tracing::info!("Shutdown complete for REST"),
                            Err(e) => tracing::error!("Server error: {}", e),
                        },
                        _ = drain_deadline => {
                            tracing::warn!(
                                "Drain timeout ({:?}) exceeded; aborting in-flight connections",
                                drain_timeout,
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to bind to {}: {}", rest_addr, e);
                }
            }
        }
    }
}

/// Response-side content negotiation for `Accept: application/vnd.msgpack`.
///
/// Handlers always produce JSON; when the client asks for MessagePack, this
/// middleware re-encodes the response **once, directly from the original
/// value** — `GenericApiResponse` stores a type-erased handle to itself in the
/// response extensions ([`NegotiablePayload`]), so no JSON byte parsing or
/// intermediate `Value` tree is involved. Responses without the extension
/// (e.g. health checks) pass through untouched.
pub async fn msgpack_negotiation(
    req: Request<Body>,
    next: axum::middleware::Next,
) -> Response<Body> {
    let wants_msgpack = req
        .headers()
        .get(header::ACCEPT)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|accept| accept.contains("application/vnd.msgpack"));

    let mut response = next.run(req).await;

    let payload = response.extensions_mut().remove::<NegotiablePayload>();

    if !wants_msgpack {
        return response;
    }

    let Some(payload) = payload else {
        return response;
    };

    match rmp_serde::to_vec_named(&*payload.0) {
        Ok(msgpack_bytes) => {
            let headers = response.headers_mut();
            headers
                .insert(header::CONTENT_TYPE, HeaderValue::from_static("application/vnd.msgpack"));
            headers.remove(header::CONTENT_LENGTH);
            *response.body_mut() = Body::from(msgpack_bytes);
            response
        }
        Err(e) => {
            tracing::warn!("Failed to encode MessagePack response; returning JSON: {}", e);
            response
        }
    }
}

async fn shutdown_signal(name: &str) {
    let ctrl_c = async {
        match signal::ctrl_c().await {
            Ok(()) => {}
            Err(e) => {
                tracing::error!("Failed to install Ctrl+C handler: {}", e);
            }
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                tracing::error!("Failed to install SIGTERM handler: {}", e);
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Signal received, draining in-flight connections for {}...", name);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::driving::http_axum::server::response::GenericApiResponse;
    use crate::infrastructure::driving::http_axum::server::validation::ValidatedBody;
    use axum::http::{Request as HttpRequest, StatusCode};
    use axum::routing::post;
    use serde::{Deserialize, Serialize};
    use tower::ServiceExt;
    use validator::Validate;

    #[derive(Debug, Serialize, Deserialize, Validate)]
    struct EchoInput {
        #[validate(length(min = 1))]
        name: String,
    }

    async fn echo(ValidatedBody(input): ValidatedBody<EchoInput>) -> GenericApiResponse<EchoInput> {
        GenericApiResponse::success(input)
    }

    fn test_app() -> Router {
        Router::new()
            .route("/echo", post(echo))
            .layer(axum::middleware::from_fn(msgpack_negotiation))
    }

    async fn body_bytes(response: Response<Body>) -> Vec<u8> {
        axum::body::to_bytes(response.into_body(), MAX_BODY_BYTES).await.unwrap().to_vec()
    }

    #[tokio::test]
    async fn accepts_msgpack_input_and_returns_json_by_default() {
        let payload = rmp_serde::to_vec_named(&EchoInput { name: "ada".into() }).unwrap();

        let request = HttpRequest::post("/echo")
            .header(header::CONTENT_TYPE, "application/vnd.msgpack")
            .body(Body::from(payload))
            .unwrap();

        let response = test_app().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            response.headers()[header::CONTENT_TYPE].to_str().unwrap().contains("json"),
            "without Accept negotiation the response stays JSON"
        );

        let entry: serde_json::Value = serde_json::from_slice(&body_bytes(response).await).unwrap();
        assert_eq!(entry["data"]["name"], "ada");
    }

    #[tokio::test]
    async fn returns_msgpack_when_accept_asks_for_it() {
        let request = HttpRequest::post("/echo")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/vnd.msgpack")
            .body(Body::from(r#"{"name":"ada"}"#))
            .unwrap();

        let response = test_app().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/vnd.msgpack");

        let entry: serde_json::Value = rmp_serde::from_slice(&body_bytes(response).await).unwrap();
        assert_eq!(entry["data"]["name"], "ada");
    }

    #[tokio::test]
    async fn rejects_invalid_msgpack_body() {
        let request = HttpRequest::post("/echo")
            .header(header::CONTENT_TYPE, "application/vnd.msgpack")
            .body(Body::from(vec![0xc1, 0xff, 0x00]))
            .unwrap();

        let response = test_app().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn validates_after_negotiated_deserialization() {
        let payload = rmp_serde::to_vec_named(&EchoInput { name: "".into() }).unwrap();

        let request = HttpRequest::post("/echo")
            .header(header::CONTENT_TYPE, "application/vnd.msgpack")
            .body(Body::from(payload))
            .unwrap();

        let response = test_app().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
