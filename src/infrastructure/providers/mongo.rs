use crate::domain::core::error::{DomainError, DomainResult};
use mongodb::{Client, Database, options::ClientOptions};
use std::future::IntoFuture;
use std::time::Duration;
use tokio::time::timeout;

/// Resolves the configuration, builds the client and pings before any repository uses it.
pub async fn connect(app_name: &str, mongo_url: &str, mongo_db: &str) -> DomainResult<Database> {
    let mut client_options =
        timeout(Duration::from_secs(10), ClientOptions::parse(mongo_url).into_future())
            .await
            .map_err(|_| DomainError::timeout("MongoDB configuration discovery"))?
            .map_err(|_| DomainError::internal("Failed to resolve MongoDB configuration"))?;

    client_options.app_name = Some(app_name.to_string());
    client_options.connect_timeout = Some(Duration::from_secs(3));
    client_options.server_selection_timeout = Some(Duration::from_secs(10));

    let client = Client::with_options(client_options).map_err(|_| {
        DomainError::service_unavailable("MongoDB", "Failed to initialize the client")
    })?;
    let db = client.database(mongo_db);
    db.run_command(bson::doc! {"ping": 1})
        .await
        .map_err(|_| DomainError::service_unavailable("MongoDB", "Initial ping failed"))?;

    tracing::info!("MongoDB initialized");
    Ok(db)
}
