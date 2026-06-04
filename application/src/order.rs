use domain::entities::order::{Order, OrderId};
use domain::entities::product::ProductId;
use domain::entities::user::UserId;
use domain::error::{DomainError, DomainResult};
use domain::pagination::Pagination;
use domain::port::order::OrderRepositoryPort;
use domain::port::product::ProductRepositoryPort;
use domain::port::user::UserRepositoryPort;
use std::sync::Arc;

#[derive(Clone)]
pub struct OrderService {
    order_repo: Arc<dyn OrderRepositoryPort>,
    user_repo: Arc<dyn UserRepositoryPort>,
    product_repo: Arc<dyn ProductRepositoryPort>,
}

impl OrderService {
    pub fn new(
        order_repo: Arc<dyn OrderRepositoryPort>,
        user_repo: Arc<dyn UserRepositoryPort>,
        product_repo: Arc<dyn ProductRepositoryPort>,
    ) -> Self {
        Self { order_repo, user_repo, product_repo }
    }

    #[tracing::instrument(skip_all, fields(%user_id, %product_id, %quantity))]
    pub async fn create_order(
        &self,
        user_id: &UserId,
        product_id: &ProductId,
        quantity: i32,
    ) -> DomainResult<Order> {
        let user_exists = self.user_repo.find_by_id(user_id).await?;
        if user_exists.is_none() {
            return Err(DomainError::not_found("User", user_id.to_string()));
        }

        let product = self
            .product_repo
            .find_by_id(product_id)
            .await?
            .ok_or_else(|| DomainError::not_found("Product", product_id.to_string()))?;

        if product.stock < quantity {
            return Err(DomainError::business_rule(format!(
                "Insufficient stock: requested {}, available {}",
                quantity, product.stock
            )));
        }

        let total_price = product.price * (quantity as f64);

        let pid = product.id.as_ref().ok_or_else(|| DomainError::internal("Product missing ID"))?;

        let stock_updated = self.product_repo.update_stock(pid, -quantity).await?;
        if !stock_updated {
            return Err(DomainError::business_rule(
                "Failed to reserve stock — product may have been modified concurrently",
            ));
        }

        let now = chrono::Utc::now();
        let mut order = Order {
            id: None,
            user_id: user_id.clone(),
            product_id: product_id.clone(),
            quantity,
            total_price,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };

        let id = self.order_repo.create(&order).await?;
        order.id = Some(id);

        tracing::info!(
            order_id = %order.id.as_deref().unwrap_or("unknown"),
            %total_price,
            "Order created"
        );
        Ok(order)
    }

    #[tracing::instrument(skip_all, fields(%id))]
    pub async fn get_order(&self, id: &OrderId) -> DomainResult<Order> {
        self.order_repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::not_found("Order", id.to_string()))
    }

    #[tracing::instrument(skip_all)]
    pub async fn list_orders(&self, pagination: Pagination) -> DomainResult<Vec<Order>> {
        self.order_repo.find_all(pagination).await
    }

    #[tracing::instrument(skip_all)]
    pub async fn count_orders(&self) -> DomainResult<u64> {
        self.order_repo.count().await
    }

    #[tracing::instrument(skip_all, fields(%id))]
    pub async fn delete_order(&self, id: &OrderId) -> DomainResult<()> {
        let deleted = self.order_repo.delete(id).await?;
        if !deleted {
            return Err(DomainError::not_found("Order", id.to_string()));
        }
        tracing::info!("Order soft-deleted");
        Ok(())
    }

    #[tracing::instrument(skip_all, fields(%user_id))]
    pub async fn list_orders_by_user(
        &self,
        user_id: &UserId,
        pagination: Pagination,
    ) -> DomainResult<Vec<Order>> {
        let user_exists = self.user_repo.find_by_id(user_id).await?;
        if user_exists.is_none() {
            return Err(DomainError::not_found("User", user_id.to_string()));
        }

        self.order_repo.find_by_user_id(user_id, pagination).await
    }
}
