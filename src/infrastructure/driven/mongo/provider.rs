use crate::domain::error::{DomainError, DomainResult};
use mongodb::{Client, Database, options::ClientOptions};

#[derive(Clone)]
pub struct MongoProvider {
    db: Database,
}

impl MongoProvider {
    pub async fn new(app_name: &str, mongo_url: &str, mongo_db: &str) -> DomainResult<Self> {
        let mut client_options = ClientOptions::parse(mongo_url)
            .await
            .map_err(|e| DomainError::database(format!("Failed to parse MongoDB URI: {}", e)))?;

        client_options.app_name = Some(app_name.to_string());

        let client = Client::with_options(client_options)
            .map_err(|_| DomainError::database("Failed to initialize MongoDB client"))?;

        let db = client.database(mongo_db);

        db.run_command(bson::doc! {"ping": 1})
            .await
            .map_err(|e| DomainError::database(format!("Failed to ping MongoDB: {}", e)))?;

        tracing::info!("Connected to MongoDB: {}", mongo_db);

        Ok(Self { db })
    }

    pub fn get_database(&self) -> Database {
        self.db.clone()
    }
}
