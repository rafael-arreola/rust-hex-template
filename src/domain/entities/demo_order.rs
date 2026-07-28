use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::entities::demo_product::DemoProductId;
use crate::domain::entities::demo_user::DemoUserId;
use crate::domain::values;

pub struct DemoOrderMarker;
pub type DemoOrderId = values::DomainId<DemoOrderMarker>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DemoOrder {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<DemoOrderId>,
    pub user_id: DemoUserId,
    pub product_id: DemoProductId,
    pub quantity: i32,
    pub total_price: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<DateTime<Utc>>,
}

impl DemoOrder {
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}
