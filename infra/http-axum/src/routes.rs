use crate::state::AppState;
use axum::Router;

pub fn app_router() -> Router<AppState> {
    Router::new()
        .nest("/users", crate::user::routes::router())
        .nest("/products", crate::product::routes::router())
        .nest("/orders", crate::order::routes::router())
}
