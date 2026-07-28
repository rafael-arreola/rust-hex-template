use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::values;

pub struct DemoUserMarker;
pub type DemoUserId = values::DomainId<DemoUserMarker>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DemoUser {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<DemoUserId>,
    pub name: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<DateTime<Utc>>,
}

impl DemoUser {
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}
