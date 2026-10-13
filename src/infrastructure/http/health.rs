use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use futures::future::BoxFuture;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Readiness capability injected by the composition root, independent of any database driver.
type ReadinessCheck = Arc<dyn Fn() -> BoxFuture<'static, bool> + Send + Sync>;

static DRAINING: AtomicBool = AtomicBool::new(false);
const READINESS_BUDGET: Duration = Duration::from_secs(2);

/// Public probes with plain statuses: `/healthz` (liveness) and `/readyz` (readiness).
pub fn router(ready: impl Fn() -> BoxFuture<'static, bool> + Send + Sync + 'static) -> Router {
    let ready: ReadinessCheck = Arc::new(ready);
    Router::new().route("/healthz", get(healthz)).route("/readyz", get(readyz)).with_state(ready)
}

pub(super) fn start_draining() {
    DRAINING.store(true, Ordering::SeqCst);
}

fn is_draining() -> bool {
    DRAINING.load(Ordering::SeqCst)
}

/// Liveness remains healthy during drain: only readiness should remove the process from service.
#[tracing::instrument(skip_all)]
async fn healthz() -> impl IntoResponse {
    StatusCode::OK
}

/// A probe cannot occupy a request indefinitely, nor become ready after shutdown began mid-ping.
#[tracing::instrument(skip_all)]
async fn readyz(State(ready): State<ReadinessCheck>) -> impl IntoResponse {
    if is_draining() {
        return StatusCode::SERVICE_UNAVAILABLE;
    }

    let result = tokio::time::timeout(READINESS_BUDGET, ready()).await;
    if matches!(result, Ok(true)) && !is_draining() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}
