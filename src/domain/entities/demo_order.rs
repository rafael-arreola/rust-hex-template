use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::core::values;
use crate::domain::entities::demo_product::DemoProductId;
use crate::domain::entities::demo_user::DemoUserId;

pub struct DemoOrderMarker;
pub type DemoOrderId = values::DomainId<DemoOrderMarker>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DemoOrder {
    pub id: Option<DemoOrderId>,
    pub user_id: DemoUserId,
    pub product_id: DemoProductId,
    pub quantity: i32,
    pub total_price: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
