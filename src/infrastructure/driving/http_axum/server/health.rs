use axum::{extract::State, http::StatusCode, response::IntoResponse};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Type alias for a readiness check function injected at startup.
pub type HealthChecker = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync>;

/// Liveness probe — always returns 200 if the process is alive.
#[tracing::instrument(skip_all)]
pub async fn healthz() -> impl IntoResponse {
    StatusCode::OK
}

/// Readiness probe — pings external dependencies via the injected checker.
#[tracing::instrument(skip_all)]
pub async fn readyz(State(checker): State<HealthChecker>) -> impl IntoResponse {
    if checker().await { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE }
}
