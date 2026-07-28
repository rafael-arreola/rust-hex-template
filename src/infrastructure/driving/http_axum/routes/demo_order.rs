pub mod dtos;

use crate::application::demo_order::DemoOrderService;
use crate::domain::entities::demo_order::DemoOrderId;
use crate::domain::entities::demo_product::DemoProductId;
use crate::domain::entities::demo_user::DemoUserId;
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

use self::dtos::{CreateDemoOrderInput, DemoOrderOutput};

#[derive(Debug, Deserialize, Validate)]
pub struct DemoOrderQuery {
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
    State(service): State<Arc<DemoOrderService>>,
    ValidatedBody(req): ValidatedBody<CreateDemoOrderInput>,
) -> Result<GenericApiResponse<DemoOrderOutput>, ApiError> {
    let user_id = DemoUserId::new(req.user_id);
    let product_id = DemoProductId::new(req.product_id);
    let order = service.create_order(&user_id, &product_id, req.quantity).await?;
    Ok(GenericApiResponse::success(order.into()))
}

#[tracing::instrument(skip_all)]
pub async fn delete_order(
    State(service): State<Arc<DemoOrderService>>,
    Path(id): Path<String>,
) -> Result<GenericApiResponse<()>, ApiError> {
    let order_id = DemoOrderId::new(id);
    service.delete_order(&order_id).await?;
    Ok(GenericApiResponse::success(()))
}

#[tracing::instrument(skip_all)]
pub async fn get_order(
    State(service): State<Arc<DemoOrderService>>,
    Path(id): Path<String>,
) -> Result<GenericApiResponse<DemoOrderOutput>, ApiError> {
    let order_id = DemoOrderId::new(id);
    let order = service.get_order(&order_id).await?;
    Ok(GenericApiResponse::success(order.into()))
}

#[tracing::instrument(skip_all)]
pub async fn list_orders(
    State(service): State<Arc<DemoOrderService>>,
    Query(query): Query<DemoOrderQuery>,
) -> Result<GenericApiResponse<GenericPagination<DemoOrderOutput>>, ApiError> {
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let pagination = Pagination { page, limit };

    let orders = service.list_orders(pagination).await?;
    let total = service.count_orders().await?;
    let dtos: Vec<DemoOrderOutput> = orders.into_iter().map(Into::into).collect();
    Ok(GenericApiResponse::paginated(dtos, total, page, limit))
}
