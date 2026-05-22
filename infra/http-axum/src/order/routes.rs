use crate::{
    error::ApiError,
    order::dtos::{CreateOrderInput, OrderOutput},
    response::GenericApiResponse,
    state::AppState,
    validation::ValidatedJson,
};
use axum::{
    Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use domain::entities::order::OrderId;
use domain::entities::product::ProductId;
use domain::entities::user::UserId;
use domain::pagination::Pagination;
use serde::Deserialize;
use std::sync::Arc;
use usecases::order::OrderService;
use validator::Validate;

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
        .route("/{id}", get(get_order))
}

#[tracing::instrument(skip_all)]
pub async fn create_order(
    State(service): State<Arc<OrderService>>,
    ValidatedJson(req): ValidatedJson<CreateOrderInput>,
) -> Result<GenericApiResponse<OrderOutput>, ApiError> {
    let user_id = UserId::new(req.user_id);
    let product_id = ProductId::new(req.product_id);
    let order = service
        .create_order(&user_id, &product_id, req.quantity)
        .await?;
    Ok(GenericApiResponse::success(order.into()))
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
) -> Result<GenericApiResponse<Vec<OrderOutput>>, ApiError> {
    let pagination = Pagination {
        page: query.page.unwrap_or(1),
        limit: query.limit.unwrap_or(20),
    };

    let orders = service.list_orders(pagination).await?;
    let dtos = orders.into_iter().map(Into::into).collect();
    Ok(GenericApiResponse::success(dtos))
}
