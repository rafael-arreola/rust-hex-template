pub mod dtos;

use crate::application::order::OrderService;
use crate::domain::entities::order::OrderId;
use crate::domain::entities::product::ProductId;
use crate::domain::entities::user::UserId;
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

use self::dtos::{CreateOrderInput, OrderOutput};

#[derive(Debug, Deserialize, Validate)]
pub struct OrderQuery {
    #[validate(range(min = 1))]
    pub page: Option<u32>,

    #[validate(range(min = 1, max = 100))]
    pub limit: Option<u32>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_order).get(list_orders))
        .route("/{id}", get(get_order).delete(delete_order))
}

#[tracing::instrument(skip_all)]
pub async fn create_order(
    State(service): State<Arc<OrderService>>,
    ValidatedBody(req): ValidatedBody<CreateOrderInput>,
) -> Result<GenericApiResponse<OrderOutput>, ApiError> {
    let user_id = UserId::new(req.user_id);
    let product_id = ProductId::new(req.product_id);
    let order = service.create_order(&user_id, &product_id, req.quantity).await?;
    Ok(GenericApiResponse::success(order.into()))
}

#[tracing::instrument(skip_all)]
pub async fn delete_order(
    State(service): State<Arc<OrderService>>,
    Path(id): Path<String>,
) -> Result<GenericApiResponse<()>, ApiError> {
    let order_id = OrderId::new(id);
    service.delete_order(&order_id).await?;
    Ok(GenericApiResponse::success(()))
}

#[tracing::instrument(skip_all)]
pub async fn get_order(
    State(service): State<Arc<OrderService>>,
    Path(id): Path<String>,
) -> Result<GenericApiResponse<OrderOutput>, ApiError> {
    let order_id = OrderId::new(id);
    let order = service.get_order(&order_id).await?;
    Ok(GenericApiResponse::success(order.into()))
}

#[tracing::instrument(skip_all)]
pub async fn list_orders(
    State(service): State<Arc<OrderService>>,
    Query(query): Query<OrderQuery>,
) -> Result<GenericApiResponse<GenericPagination<OrderOutput>>, ApiError> {
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let pagination = Pagination { page, limit };

    let orders = service.list_orders(pagination).await?;
    let total = service.count_orders().await?;
    let dtos: Vec<OrderOutput> = orders.into_iter().map(Into::into).collect();
    Ok(GenericApiResponse::paginated(dtos, total, page, limit))
}
