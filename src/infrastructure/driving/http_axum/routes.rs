pub mod demo_order;
pub mod demo_product;
pub mod demo_user;

use crate::infrastructure::driving::http_axum::server::state::AppState;
use axum::Router;

pub fn app_router() -> Router<AppState> {
    Router::new()
        .nest("/users", demo_user::router())
        .nest("/products", demo_product::router())
        .nest("/orders", demo_order::router())
}
