//! Application endpoints. The transport kit in `infrastructure::http` never imports this tree.

mod demo_order;
mod demo_product;
mod demo_user;

use std::sync::Arc;

use axum::Router;

use crate::application::{
    demo_order::DemoOrderService, demo_product::DemoProductService, demo_user::DemoUserService,
};

/// Routes are relative to the API prefix and public until the composition root mounts authentication.
pub fn router(
    users: Arc<DemoUserService>,
    products: Arc<DemoProductService>,
    orders: Arc<DemoOrderService>,
) -> Router {
    Router::new()
        .nest("/users", demo_user::router(users))
        .nest("/products", demo_product::router(products))
        .nest("/orders", demo_order::router(orders))
}
