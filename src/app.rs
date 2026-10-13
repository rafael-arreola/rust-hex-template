use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use futures::FutureExt;
use mongodb::bson::doc;

use crate::application::{
    demo_order::DemoOrderService, demo_product::DemoProductService, demo_user::DemoUserService,
};
use crate::domain::core::error::DomainResult;
use crate::infrastructure::http::{ServerLauncher, ServerOptions, health};
use crate::infrastructure::persistence::mongo::{
    demo_order::repository::DemoOrderRepository, demo_product::repository::DemoProductRepository,
    demo_user::repository::DemoUserRepository,
};
use crate::infrastructure::providers::mongo;
use crate::infrastructure::rest;
use crate::shared::envs;

/// Owns process resources and composes adapters; business decisions remain in application.
pub async fn run() -> DomainResult<()> {
    let env = envs::get();
    let database = mongo::connect(&env.service_name, &env.mongo_url, &env.mongo_db).await?;

    let users = Arc::new(DemoUserRepository::new(&database).await?);
    let products = Arc::new(DemoProductRepository::new(&database).await?);
    let orders = Arc::new(DemoOrderRepository::new(&database).await?);
    let api = rest::router(
        Arc::new(DemoUserService::new(users.clone())),
        Arc::new(DemoProductService::new(products.clone())),
        Arc::new(DemoOrderService::new(orders, users, products)),
    );

    let probes = health::router(move || {
        let database = database.clone();
        async move { database.run_command(doc! { "ping": 1 }).await.is_ok() }.boxed()
    });

    let options = ServerOptions {
        port: env.port,
        drain_timeout: Duration::from_secs(env.drain_timeout_secs),
        request_timeout: Duration::from_secs(env.request_timeout_secs),
        body_limit: env.body_limit_bytes,
        msgpack_enabled: env.msgpack_enabled,
    };
    ServerLauncher::new(Router::new().merge(probes).nest("/api/v1", api), options).run().await
}
