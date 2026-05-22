use serde::Deserialize;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateProductInput {
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
pub struct UpdateProductMetadataInput {
    pub description: Option<String>,

    #[validate(length(min = 1, message = "Category is required"))]
    pub category: String,

    pub tags: Vec<String>,

    #[validate(length(min = 1, message = "SKU is required"))]
    pub sku: String,
}
