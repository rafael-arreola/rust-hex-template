use axum::{extract::State, http::StatusCode, response::IntoResponse};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Type alias for a readiness check function injected at startup.
pub type HealthChecker = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync>;

/// Set once a shutdown signal arrives. Process-wide by nature: there is one
/// listener and one drain, so a static is the honest representation.
static DRAINING: AtomicBool = AtomicBool::new(false);

/// Marks the process as draining, so `/readyz` starts failing immediately.
pub fn start_draining() {
    DRAINING.store(true, Ordering::SeqCst);
}

pub fn is_draining() -> bool {
    DRAINING.load(Ordering::SeqCst)
}

/// Liveness probe — always returns 200 if the process is alive.
///
/// Stays 200 while draining on purpose: a failing liveness probe tells the
/// orchestrator to *kill* the process, which is the opposite of a graceful
/// drain. Readiness is what should turn red.
#[tracing::instrument(skip_all)]
pub async fn healthz() -> impl IntoResponse {
    StatusCode::OK
}

/// Readiness probe — pings external dependencies via the injected checker.
#[tracing::instrument(skip_all)]
pub async fn readyz(State(checker): State<HealthChecker>) -> impl IntoResponse {
    // Report not-ready as soon as the drain starts so the load balancer stops
    // sending new traffic *before* the listener closes. Without this the pod
    // keeps receiving requests it is about to stop serving.
    if is_draining() {
        return StatusCode::SERVICE_UNAVAILABLE;
    }

    if checker().await { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE }
}
