#![allow(dead_code)]
use redis::aio::MultiplexedConnection;

#[derive(Clone)]
pub struct RedisProvider {
    connection: MultiplexedConnection,
    prefix: String,
}

impl std::fmt::Debug for RedisProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisProvider")
            .field("connection", &"<MultiplexedConnection>")
            .finish()
    }
}

impl RedisProvider {
    pub async fn new(url: &str, prefix: &str) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(url)?;
        let mut conn = client.get_multiplexed_async_connection().await?;

        redis::cmd("PING").query_async::<()>(&mut conn).await?;

        Ok(RedisProvider {
            connection: conn,
            prefix: prefix.to_string(),
        })
    }

    pub fn connection(&self) -> MultiplexedConnection {
        self.connection.clone()
    }

    pub fn get_path(&self, key: &[&str]) -> String {
        format!("{}:{}", self.prefix, key.join(":"))
    }
}
