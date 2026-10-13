mod app;
pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod shared;

use std::process::ExitCode;

use crate::shared::{envs, tracer};

#[tokio::main]
async fn main() -> ExitCode {
    // Before tracing exists, bootstrap failures can only be reported on stderr.
    let tracer_guard = match bootstrap().await {
        Ok(started) => started,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };

    let mut exit = match app::run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error.context("application run").log();
            ExitCode::FAILURE
        }
    };
    if let Err(error) = tracer_guard.shutdown() {
        eprintln!("Tracing shutdown failed: {error}");
        exit = ExitCode::FAILURE;
    }
    exit
}

/// Environment, crypto providers and tracing, in that order.
async fn bootstrap() -> Result<tracer::TracerGuard, String> {
    envs::init().map_err(|error| format!("Configuration failed: {error}"))?;
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| "Failed to initialize the TLS crypto provider")?;
    jsonwebtoken::crypto::rust_crypto::DEFAULT_PROVIDER
        .install_default()
        .map_err(|_| "Failed to initialize the JWT crypto provider")?;
    tracer::init_tracing().await.map_err(|error| format!("Tracing initialization failed: {error}"))
}
