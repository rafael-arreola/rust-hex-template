use redis::aio::MultiplexedConnection;
use std::{fmt, time::Duration};
use tokio::time;

use crate::domain::core::error::{DomainError, DomainResult, StatusCode};

#[derive(Clone)]
pub struct RedisProvider {
    connection: MultiplexedConnection,
    prefix: String,
}

impl fmt::Debug for RedisProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedisProvider")
            .field("connection", &"<MultiplexedConnection>")
            .field("prefix", &self.prefix)
            .finish_non_exhaustive()
    }
}

impl RedisProvider {
    #[tracing::instrument(skip_all, fields(dependency = "redis"))]
    pub async fn new(url: &str, prefix: &str) -> DomainResult<Self> {
        let client = redis::Client::open(url).map_err(|_| {
            DomainError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "Redis: Failed to initialize the client",
            )
            .with_trace_code("REDIS_CLIENT_INIT_FAILED")
        })?;
        let provider = time::timeout(Duration::from_secs(10), async {
            let mut connection = client.get_multiplexed_async_connection().await.map_err(|_| {
                DomainError::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Redis: Failed to establish connection",
                )
                .with_trace_code("REDIS_CONNECTION_FAILED")
            })?;
            redis::cmd("PING").query_async::<()>(&mut connection).await.map_err(|_| {
                DomainError::new(StatusCode::SERVICE_UNAVAILABLE, "Redis: Initial ping failed")
                    .with_trace_code("REDIS_PING_FAILED")
            })?;

            Ok::<Self, DomainError>(Self { connection, prefix: prefix.to_string() })
        })
        .await
        .map_err(|_| {
            DomainError::new(StatusCode::GATEWAY_TIMEOUT, "Redis initialization")
                .with_trace_code("REDIS_INIT_TIMEOUT")
        })??;

        tracing::info!("Redis initialized");
        Ok(provider)
    }

    pub fn connection(&self) -> MultiplexedConnection {
        self.connection.clone()
    }

    pub fn get_path(&self, key: &[&str]) -> String {
        format!("{}:{}", self.prefix, key.join(":"))
    }
}
