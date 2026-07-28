pub mod dtos;

use crate::application::demo_product::DemoProductService;
use crate::domain::entities::demo_product::{DemoProductId, DemoProductMetadata};
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
    routing::{get, patch, post},
};
use serde::Deserialize;
use std::sync::Arc;
use validator::Validate;

use self::dtos::{CreateDemoProductInput, DemoProductOutput, UpdateDemoProductMetadataInput};

#[derive(Debug, Deserialize, Validate)]
pub struct DemoProductQuery {
    #[validate(range(min = 1))]
    pub page: Option<u32>,

    #[validate(range(min = 1, max = 100))]
    pub limit: Option<u32>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_product).get(list_products))
        .route("/{id}", get(get_product).delete(delete_product))
        .route("/{id}/metadata", patch(update_metadata))
}

#[tracing::instrument(skip_all)]
pub async fn create_product(
    State(service): State<Arc<DemoProductService>>,
    ValidatedBody(req): ValidatedBody<CreateDemoProductInput>,
) -> Result<GenericApiResponse<DemoProductOutput>, ApiError> {
    let metadata = DemoProductMetadata {
        description: req.description,
        category: req.category,
        tags: req.tags.unwrap_or_default(),
        sku: req.sku,
    };

    let product = service.create_product(&req.name, req.price, req.stock, metadata).await?;
    Ok(GenericApiResponse::success(product.into()))
}

#[tracing::instrument(skip_all)]
pub async fn get_product(
    State(service): State<Arc<DemoProductService>>,
    Path(id): Path<String>,
) -> Result<GenericApiResponse<DemoProductOutput>, ApiError> {
    let product_id = DemoProductId::new(id);
    let product = service.get_product(&product_id).await?;
    Ok(GenericApiResponse::success(product.into()))
}

#[tracing::instrument(skip_all)]
pub async fn list_products(
    State(service): State<Arc<DemoProductService>>,
    Query(query): Query<DemoProductQuery>,
) -> Result<GenericApiResponse<GenericPagination<DemoProductOutput>>, ApiError> {
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let pagination = Pagination { page, limit };

    let products = service.list_products(pagination).await?;
    let total = service.count_products().await?;
    let dtos: Vec<DemoProductOutput> = products.into_iter().map(Into::into).collect();
    Ok(GenericApiResponse::paginated(dtos, total, page, limit))
}

#[tracing::instrument(skip_all)]
pub async fn update_metadata(
    State(service): State<Arc<DemoProductService>>,
    Path(id): Path<String>,
    ValidatedBody(req): ValidatedBody<UpdateDemoProductMetadataInput>,
) -> Result<GenericApiResponse<DemoProductOutput>, ApiError> {
    let product_id = DemoProductId::new(id);
    let metadata = DemoProductMetadata {
        description: req.description,
        category: req.category,
        tags: req.tags,
        sku: req.sku,
    };

    let product = service.update_metadata(&product_id, metadata).await?;
    Ok(GenericApiResponse::success(product.into()))
}

#[tracing::instrument(skip_all)]
pub async fn delete_product(
    State(service): State<Arc<DemoProductService>>,
    Path(id): Path<String>,
) -> Result<GenericApiResponse<()>, ApiError> {
    let product_id = DemoProductId::new(id);
    service.delete_product(&product_id).await?;
    Ok(GenericApiResponse::success(()))
}
