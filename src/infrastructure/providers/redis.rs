use redis::aio::MultiplexedConnection;
use std::{fmt, io, time::Duration};
use tokio::time;

#[derive(Clone)]
pub struct RedisProvider {
    connection: MultiplexedConnection,
    prefix: String,
}

impl fmt::Debug for RedisProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedisProvider").field("connection", &"<MultiplexedConnection>").finish()
    }
}

impl RedisProvider {
    pub async fn new(url: &str, prefix: &str) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(url)?;
        time::timeout(Duration::from_secs(10), async {
            let mut connection = client.get_multiplexed_async_connection().await?;
            redis::cmd("PING").query_async::<()>(&mut connection).await?;

            Ok(Self { connection, prefix: prefix.to_string() })
        })
        .await
        .map_err(|_| {
            redis::RedisError::from(io::Error::new(
                io::ErrorKind::TimedOut,
                "Redis initialization timed out",
            ))
        })?
    }

    pub fn connection(&self) -> MultiplexedConnection {
        self.connection.clone()
    }

    pub fn get_path(&self, key: &[&str]) -> String {
        format!("{}:{}", self.prefix, key.join(":"))
    }
}
