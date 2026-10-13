use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::core::values;

pub struct DemoUserMarker;
pub type DemoUserId = values::DomainId<DemoUserMarker>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DemoUser {
    pub id: Option<DemoUserId>,
    pub name: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
