mod dtos;
mod handlers;

use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};

use self::handlers::{create_order, delete_order, get_order, list_orders};
use crate::application::demo_order::DemoOrderService;

pub fn router(service: Arc<DemoOrderService>) -> Router {
    Router::new()
        .route("/", post(create_order).get(list_orders))
        .route("/{id}", get(get_order).delete(delete_order))
        .with_state(service)
}
