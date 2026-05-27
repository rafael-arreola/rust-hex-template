pub mod dtos;

use crate::server::{
    error::ApiError,
    response::{GenericApiResponse, GenericPagination},
    state::AppState,
    validation::ValidatedJson,
};
use axum::{
    Router,
    extract::{Path, Query, State},
    routing::{get, patch, post},
};
use domain::entities::product::{ProductId, ProductMetadata};
use domain::pagination::Pagination;
use serde::Deserialize;
use std::sync::Arc;
use usecases::product::ProductService;
use validator::Validate;

use self::dtos::{CreateProductInput, ProductOutput, UpdateProductMetadataInput};

#[derive(Debug, Deserialize, Validate)]
pub struct ProductQuery {
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
    State(service): State<Arc<ProductService>>,
    ValidatedJson(req): ValidatedJson<CreateProductInput>,
) -> Result<GenericApiResponse<ProductOutput>, ApiError> {
    let metadata = ProductMetadata {
        description: req.description,
        category: req.category,
        tags: req.tags.unwrap_or_default(),
        sku: req.sku,
    };

    let product = service
        .create_product(&req.name, req.price, req.stock, metadata)
        .await?;
    Ok(GenericApiResponse::success(product.into()))
}

#[tracing::instrument(skip_all)]
pub async fn get_product(
    State(service): State<Arc<ProductService>>,
    Path(id): Path<String>,
) -> Result<GenericApiResponse<ProductOutput>, ApiError> {
    let product_id = ProductId::new(id);
    let product = service.get_product(&product_id).await?;
    Ok(GenericApiResponse::success(product.into()))
}

#[tracing::instrument(skip_all)]
pub async fn list_products(
    State(service): State<Arc<ProductService>>,
    Query(query): Query<ProductQuery>,
) -> Result<GenericApiResponse<GenericPagination<ProductOutput>>, ApiError> {
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let pagination = Pagination { page, limit };

    let products = service.list_products(pagination).await?;
    let total = service.count_products().await?;
    let dtos: Vec<ProductOutput> = products.into_iter().map(Into::into).collect();
    Ok(GenericApiResponse::paginated(dtos, total, page, limit))
}

#[tracing::instrument(skip_all)]
pub async fn update_metadata(
    State(service): State<Arc<ProductService>>,
    Path(id): Path<String>,
    ValidatedJson(req): ValidatedJson<UpdateProductMetadataInput>,
) -> Result<GenericApiResponse<ProductOutput>, ApiError> {
    let product_id = ProductId::new(id);
    let metadata = ProductMetadata {
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
    State(service): State<Arc<ProductService>>,
    Path(id): Path<String>,
) -> Result<GenericApiResponse<()>, ApiError> {
    let product_id = ProductId::new(id);
    service.delete_product(&product_id).await?;
    Ok(GenericApiResponse::success(()))
}
