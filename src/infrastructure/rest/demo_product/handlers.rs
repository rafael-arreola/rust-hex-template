use std::sync::Arc;

use axum::extract::State;

use super::dtos::{CreateDemoProductInput, DemoProductOutput, UpdateDemoProductMetadataInput};
use crate::application::demo_product::DemoProductService;
use crate::domain::core::pagination::Pagination;
use crate::domain::entities::demo_product::{DemoProductId, DemoProductMetadata};
use crate::infrastructure::http::{
    ApiError, ApiResponse, Paginated, PaginationQuery, PathParam, ValidatedBody, ValidatedQuery,
    paginated,
};

#[tracing::instrument(skip_all)]
pub(super) async fn create_product(
    State(service): State<Arc<DemoProductService>>,
    ValidatedBody(input): ValidatedBody<CreateDemoProductInput>,
) -> Result<ApiResponse<DemoProductOutput>, ApiError> {
    let metadata = DemoProductMetadata {
        description: input.description,
        category: input.category,
        tags: input.tags.unwrap_or_default(),
        sku: input.sku,
    };
    let product = service.create_product(&input.name, input.price, input.stock, metadata).await?;
    Ok(ApiResponse::created(product.try_into()?))
}

#[tracing::instrument(skip_all)]
pub(super) async fn get_product(
    State(service): State<Arc<DemoProductService>>,
    PathParam(id): PathParam<String>,
) -> Result<ApiResponse<DemoProductOutput>, ApiError> {
    let product = service.get_product(&DemoProductId::new(id)).await?;
    Ok(ApiResponse::ok(product.try_into()?))
}

#[tracing::instrument(skip_all)]
pub(super) async fn list_products(
    State(service): State<Arc<DemoProductService>>,
    ValidatedQuery(query): ValidatedQuery<PaginationQuery>,
) -> Result<ApiResponse<Paginated<DemoProductOutput>>, ApiError> {
    let pagination = Pagination::try_from(query)?;
    let result = service.list_products(pagination).await?;
    paginated(result, pagination)
}

#[tracing::instrument(skip_all)]
pub(super) async fn update_metadata(
    State(service): State<Arc<DemoProductService>>,
    PathParam(id): PathParam<String>,
    ValidatedBody(input): ValidatedBody<UpdateDemoProductMetadataInput>,
) -> Result<ApiResponse<DemoProductOutput>, ApiError> {
    let metadata = DemoProductMetadata {
        description: input.description,
        category: input.category,
        tags: input.tags,
        sku: input.sku,
    };
    let product = service.update_metadata(&DemoProductId::new(id), metadata).await?;
    Ok(ApiResponse::ok(product.try_into()?))
}

#[tracing::instrument(skip_all)]
pub(super) async fn delete_product(
    State(service): State<Arc<DemoProductService>>,
    PathParam(id): PathParam<String>,
) -> Result<ApiResponse<()>, ApiError> {
    service.delete_product(&DemoProductId::new(id)).await?;
    Ok(ApiResponse::empty())
}
