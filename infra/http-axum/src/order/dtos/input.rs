use serde::Deserialize;
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
