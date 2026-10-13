use crate::domain::core::error::{DomainError, DomainResult, ResultExt};
use crate::domain::core::pagination::{PageResult, Pagination};
use crate::domain::entities::demo_order::{DemoOrder, DemoOrderId};
use crate::domain::entities::demo_product::DemoProductId;
use crate::domain::entities::demo_user::DemoUserId;
use crate::domain::port::demo_order::DemoOrderRepositoryPort;
use crate::domain::port::demo_product::DemoProductRepositoryPort;
use crate::domain::port::demo_user::DemoUserRepositoryPort;
use chrono::Utc;
use std::sync::Arc;

#[derive(Clone)]
pub struct DemoOrderService {
    demo_order_repo: Arc<dyn DemoOrderRepositoryPort>,
    demo_user_repo: Arc<dyn DemoUserRepositoryPort>,
    demo_product_repo: Arc<dyn DemoProductRepositoryPort>,
}

impl DemoOrderService {
    pub fn new(
        demo_order_repo: Arc<dyn DemoOrderRepositoryPort>,
        demo_user_repo: Arc<dyn DemoUserRepositoryPort>,
        demo_product_repo: Arc<dyn DemoProductRepositoryPort>,
    ) -> Self {
        Self { demo_order_repo, demo_user_repo, demo_product_repo }
    }

    #[tracing::instrument(skip_all, fields(operation = "create_order", %quantity))]
    pub async fn create_order(
        &self,
        user_id: &DemoUserId,
        product_id: &DemoProductId,
        quantity: i32,
    ) -> DomainResult<DemoOrder> {
        let user_exists = self.demo_user_repo.find_by_id(user_id).await?;
        if user_exists.is_none() {
            return Err(DomainError::not_found("DemoUser", user_id.to_string()));
        }

        let product = self
            .demo_product_repo
            .find_by_id(product_id)
            .await?
            .ok_or_else(|| DomainError::not_found("DemoProduct", product_id.to_string()))?;

        if quantity <= 0 {
            return Err(DomainError::invalid("quantity", "must be positive")
                .with_internal(format!("DemoOrder quantity {quantity}")));
        }

        // Fast-fail before writing; the available stock only reaches logs.
        // It is NOT the guard — the reservation below is. Never rely on a
        // read-then-check like this one for correctness under concurrency.
        if product.stock < quantity {
            return Err(DomainError::business_rule(
                "Insufficient stock for the requested quantity",
            )
            .with_code("INSUFFICIENT_STOCK")
            .with_internal(format!("Requested {quantity}, available {}", product.stock)));
        }

        let total_price = product.price * f64::from(quantity);

        let pid =
            product.id.as_ref().ok_or_else(|| DomainError::internal("DemoProduct missing ID"))?;

        let now = Utc::now();
        // Authoritative guard: check and decrement in a single atomic operation.
        let reserved = self
            .demo_product_repo
            .try_reserve_stock(pid, quantity, now)
            .await
            .context("reserving stock for a new order")?;
        if !reserved {
            return Err(DomainError::business_rule(
                "Insufficient stock for the requested quantity",
            )
            .with_code("INSUFFICIENT_STOCK")
            .with_internal(format!("Atomic reservation of {quantity} units was rejected")));
        }

        let mut order = DemoOrder {
            id: None,
            user_id: user_id.clone(),
            product_id: product_id.clone(),
            quantity,
            total_price,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };

        // Inline compensation handles reported insert failures, not cancellation:
        // dropping this future after reservation can leave stock reserved.
        let id = match self.demo_order_repo.create(&order).await {
            Ok(id) => id,
            Err(create_error) => {
                match self.demo_product_repo.release_stock(pid, quantity, now).await {
                    Ok(true) => {}
                    Ok(false) => {
                        tracing::error!(
                            product_id = %pid,
                            %quantity,
                            "Stock compensation matched no product; stock needs manual reconciliation"
                        );
                    }
                    // The compensation never reaches a handler, so its failure is logged here.
                    Err(release_error) => release_error
                        .context(format!(
                            "releasing {quantity} units of product {pid} after a failed order \
                             insert; stock needs manual reconciliation"
                        ))
                        .log(),
                }
                return Err(create_error.context("inserting the order after reserving stock"));
            }
        };

        tracing::info!(order_id = %id, %total_price, "DemoOrder created");
        order.id = Some(id);
        Ok(order)
    }

    #[tracing::instrument(skip_all, fields(operation = "get_order"))]
    pub async fn get_order(&self, id: &DemoOrderId) -> DomainResult<DemoOrder> {
        self.demo_order_repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::not_found("DemoOrder", id.to_string()))
    }

    #[tracing::instrument(skip_all, fields(page = pagination.page(), limit = pagination.limit()))]
    pub async fn list_orders(&self, pagination: Pagination) -> DomainResult<PageResult<DemoOrder>> {
        self.demo_order_repo.find_all(pagination).await
    }

    #[tracing::instrument(skip_all, fields(operation = "delete_order"))]
    pub async fn delete_order(&self, id: &DemoOrderId) -> DomainResult<()> {
        let now = Utc::now();
        let deleted = self.demo_order_repo.delete(id, now).await?;
        if !deleted {
            return Err(DomainError::not_found("DemoOrder", id.to_string()));
        }
        tracing::info!("DemoOrder soft-deleted");
        Ok(())
    }

    #[tracing::instrument(skip_all, fields(operation = "list_orders_by_user", page = pagination.page(), limit = pagination.limit()))]
    pub async fn list_orders_by_user(
        &self,
        user_id: &DemoUserId,
        pagination: Pagination,
    ) -> DomainResult<PageResult<DemoOrder>> {
        let user_exists = self.demo_user_repo.find_by_id(user_id).await?;
        if user_exists.is_none() {
            return Err(DomainError::not_found("DemoUser", user_id.to_string()));
        }

        self.demo_order_repo.find_by_user_id(user_id, pagination).await
    }
}
