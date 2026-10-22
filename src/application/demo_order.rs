use crate::domain::core::error::{DomainError, DomainResult, ResultExt, StatusCode};
use crate::domain::core::pagination::{PageResult, Pagination};
use crate::domain::entities::demo_order::{DemoOrder, DemoOrderId};
use crate::domain::entities::demo_product::DemoProductId;
use crate::domain::entities::demo_user::DemoUserId;
use crate::domain::port::repositories::demo_order::DemoOrderRepositoryPort;
use crate::domain::port::repositories::demo_product::DemoProductRepositoryPort;
use crate::domain::port::repositories::demo_user::DemoUserRepositoryPort;
use chrono::Utc;
use std::sync::Arc;

#[derive(Clone)]
pub struct DemoOrderService {
    order_repo: Arc<dyn DemoOrderRepositoryPort>,
    user_repo: Arc<dyn DemoUserRepositoryPort>,
    product_repo: Arc<dyn DemoProductRepositoryPort>,
}

impl DemoOrderService {
    pub fn new(
        order_repo: Arc<dyn DemoOrderRepositoryPort>,
        user_repo: Arc<dyn DemoUserRepositoryPort>,
        product_repo: Arc<dyn DemoProductRepositoryPort>,
    ) -> Self {
        Self { order_repo, user_repo, product_repo }
    }

    #[tracing::instrument(skip_all, fields(operation = "create_order", %quantity))]
    pub async fn create_order(
        &self,
        user_id: &DemoUserId,
        product_id: &DemoProductId,
        quantity: i32,
    ) -> DomainResult<DemoOrder> {
        if self.user_repo.find_by_id(user_id).await?.is_none() {
            return Err(user_not_found(user_id));
        }

        let product = self.product_repo.find_by_id(product_id).await?.ok_or_else(|| {
            DomainError::new(
                StatusCode::NOT_FOUND,
                format!("DemoProduct id {product_id} not found"),
            )
            .with_public("DemoProduct not found")
            .with_trace_code("DEMO_PRODUCT_NOT_FOUND")
        })?;

        if quantity <= 0 {
            return Err(DomainError::new(
                StatusCode::BAD_REQUEST,
                format!("DemoOrder quantity {quantity}"),
            )
            .with_public("Invalid quantity: must be positive")
            .with_trace_code("INVALID_ORDER_QUANTITY"));
        }

        // Fast-fail before writing; the available stock only reaches logs.
        // It is NOT the guard — the reservation below is. Never rely on a
        // read-then-check like this one for correctness under concurrency.
        if product.stock < quantity {
            return Err(DomainError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("Requested {quantity}, available {}", product.stock),
            )
            .with_code("INSUFFICIENT_STOCK")
            .with_public("Insufficient stock for the requested quantity")
            .with_trace_code("ORDER_STOCK_INSUFFICIENT"));
        }

        let total_price = product.price * f64::from(quantity);

        let pid = product.id.as_ref().ok_or_else(|| {
            DomainError::new(StatusCode::INTERNAL_SERVER_ERROR, "DemoProduct missing ID")
                .with_trace_code("PRODUCT_ID_MISSING")
        })?;

        let now = Utc::now();
        // Authoritative guard: check and decrement in a single atomic operation.
        let reserved = self
            .product_repo
            .try_reserve_stock(pid, quantity, now)
            .await
            .context("reserving stock for a new order")?;
        if !reserved {
            return Err(DomainError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("Atomic reservation of {quantity} units was rejected"),
            )
            .with_code("INSUFFICIENT_STOCK")
            .with_public("Insufficient stock for the requested quantity")
            .with_trace_code("ORDER_STOCK_RACE_REJECTED"));
        }

        let mut order = DemoOrder {
            id: None,
            user_id: user_id.clone(),
            product_id: product_id.clone(),
            quantity,
            total_price,
            created_at: now,
            updated_at: now,
        };

        // Inline compensation handles reported insert failures, not cancellation:
        // dropping this future after reservation can leave stock reserved.
        let id = match self.order_repo.create(&order).await {
            Ok(id) => id,
            Err(create_error) => {
                match self.product_repo.release_stock(pid, quantity, now).await {
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
        self.order_repo.find_by_id(id).await?.ok_or_else(|| order_not_found(id))
    }

    #[tracing::instrument(skip_all, fields(page = pagination.page(), limit = pagination.limit()))]
    pub async fn list_orders(&self, pagination: Pagination) -> DomainResult<PageResult<DemoOrder>> {
        self.order_repo.find_all(pagination).await
    }

    #[tracing::instrument(skip_all, fields(operation = "list_orders_by_user", page = pagination.page(), limit = pagination.limit()))]
    pub async fn list_orders_by_user(
        &self,
        user_id: &DemoUserId,
        pagination: Pagination,
    ) -> DomainResult<PageResult<DemoOrder>> {
        if self.user_repo.find_by_id(user_id).await?.is_none() {
            return Err(user_not_found(user_id));
        }

        self.order_repo.find_by_user_id(user_id, pagination).await
    }

    #[tracing::instrument(skip_all, fields(operation = "delete_order"))]
    pub async fn delete_order(&self, id: &DemoOrderId) -> DomainResult<()> {
        if !self.order_repo.delete(id).await? {
            return Err(order_not_found(id));
        }
        tracing::info!("DemoOrder deleted");
        Ok(())
    }
}

fn order_not_found(id: &DemoOrderId) -> DomainError {
    DomainError::new(StatusCode::NOT_FOUND, format!("DemoOrder id {id} not found"))
        .with_public("DemoOrder not found")
        .with_trace_code("DEMO_ORDER_NOT_FOUND")
}

fn user_not_found(id: &DemoUserId) -> DomainError {
    DomainError::new(StatusCode::NOT_FOUND, format!("DemoUser id {id} not found"))
        .with_public("DemoUser not found")
        .with_trace_code("DEMO_USER_NOT_FOUND")
}
