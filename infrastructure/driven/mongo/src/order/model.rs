use domain::entities::order::{Order, OrderId};
use domain::entities::product::ProductId;
use domain::entities::user::UserId;
use mongodb::bson::{self, oid::ObjectId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderModel {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub user_id: ObjectId,
    pub product_id: ObjectId,
    pub quantity: i32,
    pub total_price: f64,
    pub created_at: bson::DateTime,
    pub updated_at: bson::DateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<bson::DateTime>,
}

impl From<Order> for OrderModel {
    fn from(order: Order) -> Self {
        let user_oid = ObjectId::parse_str(&*order.user_id).unwrap_or_default();
        let product_oid = ObjectId::parse_str(&*order.product_id).unwrap_or_default();
        let id = order.id.and_then(|id| ObjectId::parse_str(&*id).ok());

        Self {
            id,
            user_id: user_oid,
            product_id: product_oid,
            quantity: order.quantity,
            total_price: order.total_price,
            created_at: bson::DateTime::from_chrono(order.created_at),
            updated_at: bson::DateTime::from_chrono(order.updated_at),
            deleted_at: order.deleted_at.map(bson::DateTime::from_chrono),
        }
    }
}

impl From<OrderModel> for Order {
    fn from(model: OrderModel) -> Self {
        Self {
            id: model.id.map(|oid| OrderId::new(oid.to_hex())),
            user_id: UserId::new(model.user_id.to_hex()),
            product_id: ProductId::new(model.product_id.to_hex()),
            quantity: model.quantity,
            total_price: model.total_price,
            created_at: model.created_at.to_chrono(),
            updated_at: model.updated_at.to_chrono(),
            deleted_at: model.deleted_at.map(|dt| dt.to_chrono()),
        }
    }
}
