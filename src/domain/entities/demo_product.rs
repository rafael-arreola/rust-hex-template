use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::core::values;

pub struct DemoProductMarker;
pub type DemoProductId = values::DomainId<DemoProductMarker>;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "lowercase")]
pub enum DemoProductStatus {
    #[default]
    Draft,
    Active,
    Archived,
    OutOfStock,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DemoProductMetadata {
    pub description: Option<String>,
    pub category: String,
    pub tags: Vec<String>,
    pub sku: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DemoProduct {
    pub id: Option<DemoProductId>,
    pub name: String,
    pub price: f64,
    pub stock: i32,
    pub status: DemoProductStatus,
    pub metadata: DemoProductMetadata,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
