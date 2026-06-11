pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod shared;

use crate::application::{order::OrderService, product::ProductService, user::UserService};
use crate::domain::port::{
    order::OrderRepositoryPort, product::ProductRepositoryPort, user::UserRepositoryPort,
};
use crate::infrastructure::driven::mongo::{
    order::repository::OrderRepository, product::repository::ProductRepository,
    provider::MongoProvider, user::repository::UserRepository,
};
#[allow(unused_imports)]
use crate::infrastructure::driven::redis::RedisProvider;
use crate::infrastructure::driving::http_axum::server::health::HealthChecker;
use crate::infrastructure::driving::http_axum::{AppState, ServerLauncher};
use crate::shared::config;
use crate::shared::tracer;
use mongodb::bson::doc;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    if let Err(e) = rustls::crypto::ring::default_provider().install_default() {
        tracing::error!("Failed to install rustls crypto provider: {:?}", e);
        return;
    }

    let env = config::get();

    if let Err(e) = tracer::init_tracing().await {
        eprintln!("Failed to initialize tracing: {}", e);
    }

    tracing::info!("Starting {} (env: {})", env.service_name, env.app_env);

    // --- MongoDB ---
    let mongo = match MongoProvider::new(&env.service_name, &env.mongo_url, &env.mongo_db).await {
        Ok(mongo) => mongo,
        Err(e) => {
            tracing::error!("Failed to connect to MongoDB: {}", e);
            return;
        }
    };
    let db = mongo.get_database();

    // --- Redis ---
    // let _redis = match RedisProvider::new(&env.redis_url, &env.redis_prefix).await {
    //     Ok(redis) => redis,
    //     Err(e) => {
    //         tracing::error!("Failed to connect to Redis: {}", e);
    //         return;
    //     }
    // };

    let health_db = db.clone();
    let health_checker: HealthChecker = Arc::new(move || {
        let db = health_db.clone();
        Box::pin(async move { db.run_command(doc! { "ping": 1 }).await.is_ok() })
    });

    let user_repo = Arc::new(UserRepository::new(&db));
    let product_repo = Arc::new(ProductRepository::new(&db));
    let order_repo = Arc::new(OrderRepository::new(&db));

    tracing::info!("Creating database indexes...");
    if let Err(e) = user_repo.create_indexes().await {
        tracing::error!("Failed to create user indexes: {}", e);
        return;
    }
    if let Err(e) = product_repo.create_indexes().await {
        tracing::error!("Failed to create product indexes: {}", e);
        return;
    }
    if let Err(e) = order_repo.create_indexes().await {
        tracing::error!("Failed to create order indexes: {}", e);
        return;
    }

    let user_service = Arc::new(UserService::new(user_repo.clone() as Arc<dyn UserRepositoryPort>));
    let product_service =
        Arc::new(ProductService::new(product_repo.clone() as Arc<dyn ProductRepositoryPort>));
    let order_service = Arc::new(OrderService::new(
        order_repo as Arc<dyn OrderRepositoryPort>,
        user_repo as Arc<dyn UserRepositoryPort>,
        product_repo as Arc<dyn ProductRepositoryPort>,
    ));

    let state = AppState { health_checker, user_service, product_service, order_service };

    ServerLauncher::new(state)
        .with_cors_origins(env.cors_origins.clone())
        .with_http(env.port)
        .with_drain_timeout(env.drain_timeout_secs)
        .run()
        .await;
}
