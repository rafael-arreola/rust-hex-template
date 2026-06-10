use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::entities::product::ProductId;
use crate::domain::entities::user::UserId;
use crate::domain::values;

#[derive(Debug, Clone)]
pub struct OrderMarker;
pub type OrderId = values::DomainId<OrderMarker>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Order {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<OrderId>,
    pub user_id: UserId,
    pub product_id: ProductId,
    pub quantity: i32,
    pub total_price: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Order {
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}
