pub mod dtos;

use crate::application::user::UserService;
use crate::domain::entities::user::{User, UserId};
use crate::domain::pagination::Pagination;
use crate::infrastructure::driving::http_axum::server::{
    error::ApiError,
    response::{GenericApiResponse, GenericPagination},
    state::AppState,
    validation::ValidatedBody,
};
use axum::{
    Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use serde::Deserialize;
use std::sync::Arc;
use validator::Validate;

use self::dtos::{CreateUserInput, UserOutput};

#[derive(Debug, Deserialize, Validate)]
pub struct UserQuery {
    #[validate(range(min = 1))]
    pub page: Option<u32>,

    #[validate(range(min = 1, max = 100))]
    pub limit: Option<u32>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_user).get(list_users))
        .route("/{id}", get(get_user).delete(delete_user))
}

#[tracing::instrument(skip_all)]
pub async fn create_user(
    State(service): State<Arc<UserService>>,
    ValidatedBody(req): ValidatedBody<CreateUserInput>,
) -> Result<GenericApiResponse<UserOutput>, ApiError> {
    let user: User = service.create_user(&req.name, &req.email).await?;
    Ok(GenericApiResponse::success(user.into()))
}

#[tracing::instrument(skip_all)]
pub async fn get_user(
    State(service): State<Arc<UserService>>,
    Path(id): Path<String>,
) -> Result<GenericApiResponse<UserOutput>, ApiError> {
    let user_id = UserId::new(id);
    let user: User = service.get_user(&user_id).await?;
    Ok(GenericApiResponse::success(user.into()))
}

#[tracing::instrument(skip_all)]
pub async fn list_users(
    State(service): State<Arc<UserService>>,
    Query(query): Query<UserQuery>,
) -> Result<GenericApiResponse<GenericPagination<UserOutput>>, ApiError> {
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let pagination = Pagination { page, limit };

    let users: Vec<User> = service.list_users(pagination).await?;
    let total = service.count_users().await?;
    let data: Vec<UserOutput> = users.into_iter().map(Into::into).collect();

    Ok(GenericApiResponse::paginated(data, total, page, limit))
}

#[tracing::instrument(skip_all)]
pub async fn delete_user(
    State(service): State<Arc<UserService>>,
    Path(id): Path<String>,
) -> Result<GenericApiResponse<()>, ApiError> {
    let user_id = UserId::new(id);
    service.delete_user(&user_id).await?;
    Ok(GenericApiResponse::success(()))
}
