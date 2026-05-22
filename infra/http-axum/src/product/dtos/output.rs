use domain::entities::product::{Product, ProductId};
use serde::Serialize;

#[derive(Serialize)]
pub struct ProductOutput {
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

impl From<Product> for ProductOutput {
    fn from(product: Product) -> Self {
        Self {
            id: product
                .id
                .map(|id: ProductId| id.into_inner())
                .unwrap_or_default(),
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
