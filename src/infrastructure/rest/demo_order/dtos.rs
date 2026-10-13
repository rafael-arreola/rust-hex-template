use crate::domain::core::error::{DomainError, DomainResult};
use crate::domain::entities::demo_order::DemoOrder;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateDemoOrderInput {
    #[validate(length(equal = 24, message = "Invalid DemoUser ID format"))]
    pub user_id: String,

    #[validate(length(equal = 24, message = "Invalid DemoProduct ID format"))]
    pub product_id: String,

    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: i32,
}

#[derive(Serialize)]
pub struct DemoOrderOutput {
    pub id: String,
    pub user_id: String,
    pub product_id: String,
    pub quantity: i32,
    pub total_price: f64,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<DemoOrder> for DemoOrderOutput {
    type Error = DomainError;

    fn try_from(order: DemoOrder) -> DomainResult<Self> {
        let id =
            order.id.ok_or_else(|| DomainError::internal("Persisted order has no identifier"))?;
        Ok(Self {
            id: id.into_inner(),
            user_id: order.user_id.into_inner(),
            product_id: order.product_id.into_inner(),
            quantity: order.quantity,
            total_price: order.total_price,
            created_at: order.created_at.to_rfc3339(),
            updated_at: order.updated_at.to_rfc3339(),
        })
    }
}
