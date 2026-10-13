use std::sync::Arc;

use axum::extract::State;

use super::dtos::{CreateDemoUserInput, DemoUserOutput, UpdateDemoUserInput};
use crate::application::demo_user::DemoUserService;
use crate::domain::core::pagination::Pagination;
use crate::domain::entities::demo_user::DemoUserId;
use crate::infrastructure::http::{
    ApiError, ApiResponse, Paginated, PaginationQuery, PathParam, ValidatedBody, ValidatedQuery,
    paginated,
};

#[tracing::instrument(skip_all)]
pub(super) async fn create_user(
    State(service): State<Arc<DemoUserService>>,
    ValidatedBody(input): ValidatedBody<CreateDemoUserInput>,
) -> Result<ApiResponse<DemoUserOutput>, ApiError> {
    let user = service.create_user(&input.name, &input.email).await?;
    Ok(ApiResponse::created(user.try_into()?))
}

#[tracing::instrument(skip_all)]
pub(super) async fn get_user(
    State(service): State<Arc<DemoUserService>>,
    PathParam(id): PathParam<DemoUserId>,
) -> Result<ApiResponse<DemoUserOutput>, ApiError> {
    let user = service.get_user(&id).await?;
    Ok(ApiResponse::ok(user.try_into()?))
}

#[tracing::instrument(skip_all)]
pub(super) async fn list_users(
    State(service): State<Arc<DemoUserService>>,
    ValidatedQuery(query): ValidatedQuery<PaginationQuery>,
) -> Result<ApiResponse<Paginated<DemoUserOutput>>, ApiError> {
    let pagination = Pagination::try_from(query)?;
    let result = service.list_users(pagination).await?;
    paginated(result, pagination)
}

#[tracing::instrument(skip_all)]
pub(super) async fn update_user(
    State(service): State<Arc<DemoUserService>>,
    PathParam(id): PathParam<DemoUserId>,
    ValidatedBody(input): ValidatedBody<UpdateDemoUserInput>,
) -> Result<ApiResponse<DemoUserOutput>, ApiError> {
    let user = service.update_user(&id, &input.name, &input.email).await?;
    Ok(ApiResponse::ok(user.try_into()?))
}

#[tracing::instrument(skip_all)]
pub(super) async fn delete_user(
    State(service): State<Arc<DemoUserService>>,
    PathParam(id): PathParam<DemoUserId>,
) -> Result<ApiResponse<()>, ApiError> {
    service.delete_user(&id).await?;
    Ok(ApiResponse::empty())
}
