mod config;
mod routes;
mod server;
mod telemetry;

use domain::port::{
    order::OrderRepositoryPort, product::ProductRepositoryPort, user::UserRepositoryPort,
};
use infra_mongo::{
    order::repository::OrderRepository, product::repository::ProductRepository,
    provider::MongoProvider, user::repository::UserRepository,
};
use server::ServerLauncher;
use server::state::AppState;
use std::sync::Arc;
use usecases::{order::OrderService, product::ProductService, user::UserService};

#[tokio::main]
async fn main() {
    if let Err(e) = rustls::crypto::ring::default_provider().install_default() {
        tracing::error!("Failed to install rustls crypto provider: {:?}", e);
        return;
    }

    let env = config::get();

    if let Err(e) = telemetry::init_tracing().await {
        eprintln!("Failed to initialize tracing: {}", e);
    }

    tracing::info!("Starting {} (env: {})", env.service_name, env.app_env);

    let mongo = match MongoProvider::new(&env.service_name, &env.mongo_url, &env.mongo_db).await {
        Ok(mongo) => mongo,
        Err(e) => {
            tracing::error!("Failed to connect to MongoDB: {}", e);
            return;
        }
    };
    let db = mongo.get_database();

    let user_repo = Arc::new(UserRepository::new(&db));
    let product_repo = Arc::new(ProductRepository::new(&db));
    let order_repo = Arc::new(OrderRepository::new(&db));

    tracing::info!("Creating database indexes...");
    if let Err(e) = user_repo.create_indexes().await {
        tracing::error!("Failed to create user indexes: {}", e);
    }
    if let Err(e) = product_repo.create_indexes().await {
        tracing::error!("Failed to create product indexes: {}", e);
    }
    if let Err(e) = order_repo.create_indexes().await {
        tracing::error!("Failed to create order indexes: {}", e);
    }

    let user_service = Arc::new(UserService::new(
        user_repo.clone() as Arc<dyn UserRepositoryPort>
    ));
    let product_service = Arc::new(ProductService::new(
        product_repo.clone() as Arc<dyn ProductRepositoryPort>
    ));
    let order_service = Arc::new(OrderService::new(
        order_repo as Arc<dyn OrderRepositoryPort>,
        user_repo as Arc<dyn UserRepositoryPort>,
        product_repo as Arc<dyn ProductRepositoryPort>,
    ));

    let state = AppState {
        user_service,
        product_service,
        order_service,
    };

    ServerLauncher::new(state).with_http(env.port).run().await;
}
