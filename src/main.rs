pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod shared;

use crate::application::{
    demo_order::DemoOrderService, demo_product::DemoProductService, demo_user::DemoUserService,
};
use crate::infrastructure::driven::mongo::{
    demo_order::repository::DemoOrderRepository, demo_product::repository::DemoProductRepository,
    demo_user::repository::DemoUserRepository, provider::MongoProvider,
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
        eprintln!("Failed to install rustls crypto provider: {:?}", e);
        return;
    }

    let env = config::get();

    let tracer_guard = match tracer::init_tracing().await {
        Ok(guard) => Some(guard),
        Err(e) => {
            eprintln!("Failed to initialize tracing: {}", e);
            None
        }
    };

    // Every exit path goes through here so buffered spans always get flushed.
    serve(env).await;

    if let Some(guard) = tracer_guard {
        guard.shutdown();
    }
}

async fn serve(env: &'static shared::config::Env) {
    tracing::info!("Starting {} (env: {})", env.service_name, env.service_env);

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

    // --- Repositories ---
    // `new` is async and fallible because it also ensures the collection's
    // indexes; there is no separate `create_indexes()` step to forget.
    let demo_user_repo = match DemoUserRepository::new(&db).await {
        Ok(repo) => Arc::new(repo),
        Err(e) => {
            tracing::error!("Failed to initialize DemoUserRepository: {}", e);
            return;
        }
    };
    let demo_product_repo = match DemoProductRepository::new(&db).await {
        Ok(repo) => Arc::new(repo),
        Err(e) => {
            tracing::error!("Failed to initialize DemoProductRepository: {}", e);
            return;
        }
    };
    let demo_order_repo = match DemoOrderRepository::new(&db).await {
        Ok(repo) => Arc::new(repo),
        Err(e) => {
            tracing::error!("Failed to initialize DemoOrderRepository: {}", e);
            return;
        }
    };

    // --- Application services ---
    // No `as Arc<dyn …Port>` casts: Rust coerces `Arc<Concrete>` to
    // `Arc<dyn Trait>` on its own at the call site.
    let demo_user_service = Arc::new(DemoUserService::new(demo_user_repo.clone()));
    let demo_product_service = Arc::new(DemoProductService::new(demo_product_repo.clone()));
    let demo_order_service =
        Arc::new(DemoOrderService::new(demo_order_repo, demo_user_repo, demo_product_repo));

    let state =
        AppState { health_checker, demo_user_service, demo_product_service, demo_order_service };

    ServerLauncher::new(state)
        .with_cors_origins(env.cors_origins.clone())
        .with_http(env.port)
        .with_drain_timeout(env.drain_timeout_secs)
        .with_request_timeout(env.request_timeout_secs)
        .with_msgpack(env.msgpack_enabled)
        .run()
        .await;
}
