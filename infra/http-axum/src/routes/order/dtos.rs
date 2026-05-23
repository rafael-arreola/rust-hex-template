use domain::entities::order::{Order, OrderId};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateOrderInput {
    #[validate(length(equal = 24, message = "Invalid User ID format"))]
    pub user_id: String,

    #[validate(length(equal = 24, message = "Invalid Product ID format"))]
    pub product_id: String,

    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: i32,
}

#[derive(Serialize)]
pub struct OrderOutput {
    pub id: String,
    pub user_id: String,
    pub product_id: String,
    pub quantity: i32,
    pub total_price: f64,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Order> for OrderOutput {
    fn from(order: Order) -> Self {
        Self {
            id: order
                .id
                .map(|id: OrderId| id.into_inner())
                .unwrap_or_default(),
            user_id: order.user_id.into_inner(),
            product_id: order.product_id.into_inner(),
            quantity: order.quantity,
            total_price: order.total_price,
            created_at: order.created_at.to_rfc3339(),
            updated_at: order.updated_at.to_rfc3339(),
        }
    }
}
