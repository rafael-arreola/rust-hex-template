use crate::domain::entities::demo_product::{DemoProduct, DemoProductId};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateDemoProductInput {
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    pub name: String,

    #[validate(range(min = 0.0, message = "Price must be non-negative"))]
    pub price: f64,

    #[validate(range(min = 0, message = "Stock must be non-negative"))]
    pub stock: i32,

    #[validate(length(min = 1, message = "Category is required"))]
    pub category: String,

    #[validate(length(min = 1, message = "SKU is required"))]
    pub sku: String,

    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateDemoProductMetadataInput {
    pub description: Option<String>,

    #[validate(length(min = 1, message = "Category is required"))]
    pub category: String,

    pub tags: Vec<String>,

    #[validate(length(min = 1, message = "SKU is required"))]
    pub sku: String,
}

#[derive(Serialize)]
pub struct DemoProductOutput {
    pub id: String,
    pub name: String,
    pub price: f64,
    pub stock: i32,
    pub status: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub sku: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<DemoProduct> for DemoProductOutput {
    fn from(product: DemoProduct) -> Self {
        Self {
            id: product.id.map(|id: DemoProductId| id.into_inner()).unwrap_or_default(),
            name: product.name,
            price: product.price,
            stock: product.stock,
            status: format!("{:?}", product.status),
            description: product.metadata.description,
            category: Some(product.metadata.category),
            tags: Some(product.metadata.tags),
            sku: Some(product.metadata.sku),
            created_at: product.created_at.to_rfc3339(),
            updated_at: product.updated_at.to_rfc3339(),
        }
    }
}
