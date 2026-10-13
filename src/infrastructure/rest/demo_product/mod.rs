mod dtos;
mod handlers;

use std::sync::Arc;

use axum::{
    Router,
    routing::{get, patch, post},
};

use self::handlers::{create_product, delete_product, get_product, list_products, update_metadata};
use crate::application::demo_product::DemoProductService;

pub fn router(service: Arc<DemoProductService>) -> Router {
    Router::new()
        .route("/", post(create_product).get(list_products))
        .route("/{id}", get(get_product).delete(delete_product))
        .route("/{id}/metadata", patch(update_metadata))
        .with_state(service)
}
