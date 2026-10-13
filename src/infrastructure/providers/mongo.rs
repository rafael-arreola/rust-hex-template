use crate::domain::core::error::{DomainError, DomainResult, StatusCode};
use mongodb::{Client, Database, options::ClientOptions};
use std::future::IntoFuture;
use std::time::Duration;
use tokio::time::timeout;

/// Resolves the configuration, builds the client and pings before any repository uses it.
pub async fn connect(app_name: &str, mongo_url: &str, mongo_db: &str) -> DomainResult<Database> {
    let mut client_options =
        timeout(Duration::from_secs(10), ClientOptions::parse(mongo_url).into_future())
            .await
            .map_err(|_| {
                DomainError::new(StatusCode::GATEWAY_TIMEOUT, "MongoDB configuration discovery")
                    .with_trace_code("MONGO_CONFIG_TIMEOUT")
            })?
            .map_err(|_| {
                DomainError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to resolve MongoDB configuration",
                )
                .with_trace_code("MONGO_CONFIG_INVALID")
            })?;

    client_options.app_name = Some(app_name.to_string());
    client_options.connect_timeout = Some(Duration::from_secs(3));
    client_options.server_selection_timeout = Some(Duration::from_secs(10));

    let client = Client::with_options(client_options).map_err(|_| {
        DomainError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "MongoDB: Failed to initialize the client",
        )
        .with_trace_code("MONGO_CLIENT_INIT_FAILED")
    })?;
    let db = client.database(mongo_db);
    db.run_command(bson::doc! {"ping": 1}).await.map_err(|_| {
        DomainError::new(StatusCode::SERVICE_UNAVAILABLE, "MongoDB: Initial ping failed")
            .with_trace_code("MONGO_PING_FAILED")
    })?;

    tracing::info!("MongoDB initialized");
    Ok(db)
}
