use std::sync::Arc;

use axum::extract::State;

use super::dtos::{CreateDemoOrderInput, DemoOrderOutput};
use crate::application::demo_order::DemoOrderService;
use crate::domain::core::pagination::Pagination;
use crate::domain::entities::{
    demo_order::DemoOrderId, demo_product::DemoProductId, demo_user::DemoUserId,
};
use crate::infrastructure::http::{
    ApiError, ApiResponse, Paginated, PaginationQuery, PathParam, ValidatedBody, ValidatedQuery,
    paginated,
};

#[tracing::instrument(skip_all)]
pub(super) async fn create_order(
    State(service): State<Arc<DemoOrderService>>,
    ValidatedBody(input): ValidatedBody<CreateDemoOrderInput>,
) -> Result<ApiResponse<DemoOrderOutput>, ApiError> {
    let user_id = DemoUserId::new(input.user_id);
    let product_id = DemoProductId::new(input.product_id);
    let order = service.create_order(&user_id, &product_id, input.quantity).await?;
    Ok(ApiResponse::created(order.try_into()?))
}

#[tracing::instrument(skip_all)]
pub(super) async fn get_order(
    State(service): State<Arc<DemoOrderService>>,
    PathParam(id): PathParam<DemoOrderId>,
) -> Result<ApiResponse<DemoOrderOutput>, ApiError> {
    let order = service.get_order(&id).await?;
    Ok(ApiResponse::ok(order.try_into()?))
}

#[tracing::instrument(skip_all)]
pub(super) async fn list_orders(
    State(service): State<Arc<DemoOrderService>>,
    ValidatedQuery(query): ValidatedQuery<PaginationQuery>,
) -> Result<ApiResponse<Paginated<DemoOrderOutput>>, ApiError> {
    let pagination = Pagination::try_from(query)?;
    let result = service.list_orders(pagination).await?;
    paginated(result, pagination)
}

#[tracing::instrument(skip_all)]
pub(super) async fn list_orders_by_user(
    State(service): State<Arc<DemoOrderService>>,
    PathParam(user_id): PathParam<DemoUserId>,
    ValidatedQuery(query): ValidatedQuery<PaginationQuery>,
) -> Result<ApiResponse<Paginated<DemoOrderOutput>>, ApiError> {
    let pagination = Pagination::try_from(query)?;
    let result = service.list_orders_by_user(&user_id, pagination).await?;
    paginated(result, pagination)
}

#[tracing::instrument(skip_all)]
pub(super) async fn delete_order(
    State(service): State<Arc<DemoOrderService>>,
    PathParam(id): PathParam<DemoOrderId>,
) -> Result<ApiResponse<()>, ApiError> {
    service.delete_order(&id).await?;
    Ok(ApiResponse::empty())
}
