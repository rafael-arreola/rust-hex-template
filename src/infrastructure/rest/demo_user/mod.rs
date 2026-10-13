mod dtos;
mod handlers;

use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};

use self::handlers::{create_user, delete_user, get_user, list_users, update_user};
use crate::application::demo_user::DemoUserService;

pub fn router(service: Arc<DemoUserService>) -> Router {
    Router::new()
        .route("/", post(create_user).get(list_users))
        .route("/{id}", get(get_user).patch(update_user).delete(delete_user))
        .with_state(service)
}
