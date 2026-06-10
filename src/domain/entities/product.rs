use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::values;

#[derive(Debug, Clone)]
pub struct ProductMarker;
pub type ProductId = values::DomainId<ProductMarker>;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProductStatus {
    #[default]
    Draft,
    Active,
    Archived,
    OutOfStock,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ProductMetadata {
    pub description: Option<String>,
    pub category: String,
    pub tags: Vec<String>,
    pub sku: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Product {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<ProductId>,
    pub name: String,
    pub price: f64,
    pub stock: i32,
    pub status: ProductStatus,
    pub metadata: ProductMetadata,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Product {
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}
