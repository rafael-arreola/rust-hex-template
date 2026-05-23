pub mod order;
pub mod product;
pub mod user;

use crate::server::state::AppState;
use axum::Router;

pub fn app_router() -> Router<AppState> {
    Router::new()
        .nest("/users", user::router())
        .nest("/products", product::router())
        .nest("/orders", order::router())
}
