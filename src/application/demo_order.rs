use crate::domain::entities::demo_order::{DemoOrder, DemoOrderId};
use crate::domain::entities::demo_product::DemoProductId;
use crate::domain::entities::demo_user::DemoUserId;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::pagination::Pagination;
use crate::domain::port::demo_order::DemoOrderRepositoryPort;
use crate::domain::port::demo_product::DemoProductRepositoryPort;
use crate::domain::port::demo_user::DemoUserRepositoryPort;
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

    #[tracing::instrument(skip_all, fields(%user_id, %product_id, %quantity))]
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
            return Err(DomainError::Invalid {
                field: "quantity",
                reason: format!("DemoOrder quantity must be positive, got {}", quantity),
            });
        }

        // Friendly fast-fail: lets the client see how much is actually left.
        // It is NOT the guard — the reservation below is. Never rely on a
        // read-then-check like this one for correctness under concurrency.
        if product.stock < quantity {
            return Err(DomainError::business_rule(format!(
                "Insufficient stock: requested {}, available {}",
                quantity, product.stock
            )));
        }

        let total_price = product.price * (quantity as f64);

        let pid =
            product.id.as_ref().ok_or_else(|| DomainError::internal("DemoProduct missing ID"))?;

        // Authoritative guard: check and decrement in a single atomic operation.
        let reserved = self.demo_product_repo.try_reserve_stock(pid, quantity).await?;
        if !reserved {
            return Err(DomainError::business_rule(format!(
                "Insufficient stock: {} units are no longer available",
                quantity
            )));
        }

        let now = chrono::Utc::now();
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

        // Stock is already reserved. Any failure from here on must compensate,
        // otherwise the units are lost. A single-service template does this
        // inline; once the order lives in another service, this is the seam
        // where a saga or an outbox belongs.
        let id = match self.demo_order_repo.create(&order).await {
            Ok(id) => id,
            Err(create_error) => {
                if let Err(release_error) =
                    self.demo_product_repo.release_stock(pid, quantity).await
                {
                    tracing::error!(
                        product_id = %pid,
                        %quantity,
                        %release_error,
                        "Stock compensation failed; product stock needs manual reconciliation"
                    );
                }
                return Err(create_error);
            }
        };
        order.id = Some(id);

        tracing::info!(
            order_id = %order.id.as_deref().unwrap_or("unknown"),
            %total_price,
            "DemoOrder created"
        );
        Ok(order)
    }

    #[tracing::instrument(skip_all, fields(%id))]
    pub async fn get_order(&self, id: &DemoOrderId) -> DomainResult<DemoOrder> {
        self.demo_order_repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::not_found("DemoOrder", id.to_string()))
    }

    #[tracing::instrument(skip_all)]
    pub async fn list_orders(&self, pagination: Pagination) -> DomainResult<Vec<DemoOrder>> {
        self.demo_order_repo.find_all(pagination).await
    }

    #[tracing::instrument(skip_all)]
    pub async fn count_orders(&self) -> DomainResult<u64> {
        self.demo_order_repo.count().await
    }

    #[tracing::instrument(skip_all, fields(%id))]
    pub async fn delete_order(&self, id: &DemoOrderId) -> DomainResult<()> {
        let deleted = self.demo_order_repo.delete(id).await?;
        if !deleted {
            return Err(DomainError::not_found("DemoOrder", id.to_string()));
        }
        tracing::info!("DemoOrder soft-deleted");
        Ok(())
    }

    #[tracing::instrument(skip_all, fields(%user_id))]
    pub async fn list_orders_by_user(
        &self,
        user_id: &DemoUserId,
        pagination: Pagination,
    ) -> DomainResult<Vec<DemoOrder>> {
        let user_exists = self.demo_user_repo.find_by_id(user_id).await?;
        if user_exists.is_none() {
            return Err(DomainError::not_found("DemoUser", user_id.to_string()));
        }

        self.demo_order_repo.find_by_user_id(user_id, pagination).await
    }
}

// ---------------------------------------------------------------------------
// Cross-entity testing: one fake per port, as described in
// `.agents/skills/test-entity`. This is the service that orchestrates three
// repositories, so it is also where the interesting failure modes live —
// overselling and lost stock after a partial failure.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::demo_product::{
        DemoProduct, DemoProductMetadata, DemoProductStatus,
    };
    use crate::domain::entities::demo_user::DemoUser;
    use std::sync::Mutex;

    const USER_ID: &str = "00000000000000000000000a";
    const PRODUCT_ID: &str = "00000000000000000000000b";
    const MISSING_ID: &str = "ffffffffffffffffffffffff";

    // --- Fakes -------------------------------------------------------------

    #[derive(Default)]
    struct FakeDemoUserRepository {
        existing: Option<DemoUserId>,
    }

    #[async_trait::async_trait]
    impl DemoUserRepositoryPort for FakeDemoUserRepository {
        async fn find_by_id(&self, id: &DemoUserId) -> DomainResult<Option<DemoUser>> {
            if self.existing.as_ref() == Some(id) {
                let now = chrono::Utc::now();
                return Ok(Some(DemoUser {
                    id: Some(id.clone()),
                    name: "Ada".into(),
                    email: "ada@example.com".into(),
                    created_at: now,
                    updated_at: now,
                    deleted_at: None,
                }));
            }
            Ok(None)
        }

        async fn create(&self, _user: &DemoUser) -> DomainResult<DemoUserId> {
            unimplemented!("not exercised by DemoOrderService")
        }
        async fn find_by_email(&self, _email: &str) -> DomainResult<Option<DemoUser>> {
            unimplemented!("not exercised by DemoOrderService")
        }
        async fn find_all(&self, _pagination: Pagination) -> DomainResult<Vec<DemoUser>> {
            unimplemented!("not exercised by DemoOrderService")
        }
        async fn update(&self, _id: &DemoUserId, _user: &DemoUser) -> DomainResult<bool> {
            unimplemented!("not exercised by DemoOrderService")
        }
        async fn delete(&self, _id: &DemoUserId) -> DomainResult<bool> {
            unimplemented!("not exercised by DemoOrderService")
        }
        async fn count(&self) -> DomainResult<u64> {
            unimplemented!("not exercised by DemoOrderService")
        }
    }

    /// Mirrors the real adapter's contract: `try_reserve_stock` checks
    /// availability and decrements **while holding the lock**, exactly as the
    /// MongoDB `$gte` guard does in a single `update_one`.
    struct FakeDemoProductRepository {
        stock: Mutex<i32>,
        price: f64,
        exists: bool,
        /// When set, `find_by_id` blocks until every contender has read the
        /// same stock snapshot. Without it the fake never yields, tokio runs
        /// each task to completion, and the race the test exists to catch
        /// simply never happens.
        read_barrier: Option<Arc<tokio::sync::Barrier>>,
    }

    impl FakeDemoProductRepository {
        fn with_stock(stock: i32) -> Self {
            Self { stock: Mutex::new(stock), price: 10.0, exists: true, read_barrier: None }
        }

        fn missing() -> Self {
            Self { stock: Mutex::new(0), price: 0.0, exists: false, read_barrier: None }
        }

        fn racing(stock: i32, contenders: usize) -> Self {
            Self {
                read_barrier: Some(Arc::new(tokio::sync::Barrier::new(contenders))),
                ..Self::with_stock(stock)
            }
        }

        fn stock(&self) -> i32 {
            *self.stock.lock().unwrap()
        }
    }

    #[async_trait::async_trait]
    impl DemoProductRepositoryPort for FakeDemoProductRepository {
        async fn find_by_id(&self, id: &DemoProductId) -> DomainResult<Option<DemoProduct>> {
            if !self.exists {
                return Ok(None);
            }
            let snapshot = self.stock();
            if let Some(barrier) = &self.read_barrier {
                barrier.wait().await;
            }
            let now = chrono::Utc::now();
            Ok(Some(DemoProduct {
                id: Some(id.clone()),
                name: "Widget".into(),
                price: self.price,
                stock: snapshot,
                status: DemoProductStatus::Active,
                metadata: DemoProductMetadata::default(),
                created_at: now,
                updated_at: now,
                deleted_at: None,
            }))
        }

        async fn try_reserve_stock(
            &self,
            _id: &DemoProductId,
            quantity: i32,
        ) -> DomainResult<bool> {
            let mut stock = self.stock.lock().unwrap();
            if !self.exists || *stock < quantity {
                return Ok(false);
            }
            *stock -= quantity;
            Ok(true)
        }

        async fn release_stock(&self, _id: &DemoProductId, quantity: i32) -> DomainResult<bool> {
            *self.stock.lock().unwrap() += quantity;
            Ok(true)
        }

        async fn create(&self, _product: &DemoProduct) -> DomainResult<DemoProductId> {
            unimplemented!("not exercised by DemoOrderService")
        }
        async fn find_all(&self, _pagination: Pagination) -> DomainResult<Vec<DemoProduct>> {
            unimplemented!("not exercised by DemoOrderService")
        }
        async fn update_metadata(
            &self,
            _id: &DemoProductId,
            _metadata: &DemoProductMetadata,
        ) -> DomainResult<bool> {
            unimplemented!("not exercised by DemoOrderService")
        }
        async fn delete(&self, _id: &DemoProductId) -> DomainResult<bool> {
            unimplemented!("not exercised by DemoOrderService")
        }
        async fn count(&self) -> DomainResult<u64> {
            unimplemented!("not exercised by DemoOrderService")
        }
    }

    #[derive(Default)]
    struct FakeDemoOrderRepository {
        orders: Mutex<Vec<DemoOrder>>,
        fail_create: bool,
    }

    impl FakeDemoOrderRepository {
        fn failing() -> Self {
            Self { fail_create: true, ..Default::default() }
        }

        fn len(&self) -> usize {
            self.orders.lock().unwrap().len()
        }
    }

    #[async_trait::async_trait]
    impl DemoOrderRepositoryPort for FakeDemoOrderRepository {
        async fn create(&self, order: &DemoOrder) -> DomainResult<DemoOrderId> {
            if self.fail_create {
                return Err(DomainError::database("simulated write failure"));
            }
            let mut orders = self.orders.lock().unwrap();
            let id = DemoOrderId::new(format!("{:024x}", orders.len() + 1));
            let mut stored = order.clone();
            stored.id = Some(id.clone());
            orders.push(stored);
            Ok(id)
        }

        async fn find_by_id(&self, id: &DemoOrderId) -> DomainResult<Option<DemoOrder>> {
            Ok(self
                .orders
                .lock()
                .unwrap()
                .iter()
                .find(|o| o.id.as_ref() == Some(id) && !o.is_deleted())
                .cloned())
        }

        async fn find_all(&self, _pagination: Pagination) -> DomainResult<Vec<DemoOrder>> {
            Ok(self.orders.lock().unwrap().iter().filter(|o| !o.is_deleted()).cloned().collect())
        }

        async fn find_by_user_id(
            &self,
            user_id: &DemoUserId,
            _pagination: Pagination,
        ) -> DomainResult<Vec<DemoOrder>> {
            Ok(self
                .orders
                .lock()
                .unwrap()
                .iter()
                .filter(|o| &o.user_id == user_id && !o.is_deleted())
                .cloned()
                .collect())
        }

        async fn delete(&self, id: &DemoOrderId) -> DomainResult<bool> {
            let mut orders = self.orders.lock().unwrap();
            match orders.iter_mut().find(|o| o.id.as_ref() == Some(id) && !o.is_deleted()) {
                Some(order) => {
                    order.deleted_at = Some(chrono::Utc::now());
                    Ok(true)
                }
                None => Ok(false),
            }
        }

        async fn count(&self) -> DomainResult<u64> {
            Ok(self.orders.lock().unwrap().iter().filter(|o| !o.is_deleted()).count() as u64)
        }
    }

    // --- Wiring ------------------------------------------------------------

    struct Harness {
        service: DemoOrderService,
        products: Arc<FakeDemoProductRepository>,
        orders: Arc<FakeDemoOrderRepository>,
    }

    fn harness_with(
        products: FakeDemoProductRepository,
        orders: FakeDemoOrderRepository,
        known_user: bool,
    ) -> Harness {
        let products = Arc::new(products);
        let orders = Arc::new(orders);
        let users = Arc::new(FakeDemoUserRepository {
            existing: known_user.then(|| DemoUserId::new(USER_ID)),
        });

        Harness {
            service: DemoOrderService::new(orders.clone(), users, products.clone()),
            products,
            orders,
        }
    }

    fn harness(stock: i32) -> Harness {
        harness_with(
            FakeDemoProductRepository::with_stock(stock),
            FakeDemoOrderRepository::default(),
            true,
        )
    }

    fn user_id() -> DemoUserId {
        DemoUserId::new(USER_ID)
    }

    fn product_id() -> DemoProductId {
        DemoProductId::new(PRODUCT_ID)
    }

    // --- Tests -------------------------------------------------------------

    #[tokio::test]
    async fn create_order_reserves_exactly_the_ordered_units() {
        let harness = harness(10);

        let order = harness.service.create_order(&user_id(), &product_id(), 3).await.unwrap();

        assert!(order.id.is_some());
        assert_eq!(order.total_price, 30.0);
        assert_eq!(harness.products.stock(), 7);
    }

    #[tokio::test]
    async fn create_order_rejects_unknown_user() {
        let harness = harness_with(
            FakeDemoProductRepository::with_stock(10),
            FakeDemoOrderRepository::default(),
            false,
        );

        let error = harness.service.create_order(&user_id(), &product_id(), 1).await.unwrap_err();

        assert_eq!(error.code(), "NOT_FOUND");
        assert_eq!(harness.products.stock(), 10, "a rejected order must not touch stock");
    }

    #[tokio::test]
    async fn create_order_rejects_unknown_product() {
        let harness = harness_with(
            FakeDemoProductRepository::missing(),
            FakeDemoOrderRepository::default(),
            true,
        );

        let error =
            harness.service.create_order(&user_id(), &DemoProductId::new(MISSING_ID), 1).await;

        assert_eq!(error.unwrap_err().code(), "NOT_FOUND");
    }

    #[tokio::test]
    async fn create_order_rejects_insufficient_stock() {
        let harness = harness(2);

        let error = harness.service.create_order(&user_id(), &product_id(), 5).await.unwrap_err();

        assert_eq!(error.code(), "BUSINESS_RULE_VIOLATION");
        assert_eq!(harness.products.stock(), 2);
    }

    #[tokio::test]
    async fn create_order_rejects_non_positive_quantity() {
        let harness = harness(10);

        let error = harness.service.create_order(&user_id(), &product_id(), 0).await.unwrap_err();

        assert_eq!(error.code(), "INVALID_INPUT");
        assert_eq!(harness.products.stock(), 10);
    }

    /// Regression test for the compensation path: the reservation succeeded,
    /// the order write failed, and the units must go back. Without the
    /// compensation in `create_order` the stock silently ends at 8.
    #[tokio::test]
    async fn failed_order_write_releases_the_reserved_stock() {
        let harness = harness_with(
            FakeDemoProductRepository::with_stock(10),
            FakeDemoOrderRepository::failing(),
            true,
        );

        let error = harness.service.create_order(&user_id(), &product_id(), 2).await.unwrap_err();

        assert_eq!(error.code(), "INTERNAL_ERROR");
        assert_eq!(harness.orders.len(), 0);
        assert_eq!(harness.products.stock(), 10, "reserved units must be compensated back");
    }

    /// Regression test for the oversell race.
    ///
    /// Every task reads the same pre-check snapshot and sees enough stock, so
    /// the friendly `product.stock < quantity` check lets them all through —
    /// by design. Only the atomic reservation may decide the winners.
    ///
    /// Scope: this pins the *service's* use of the port. That the MongoDB
    /// query is itself atomic is a property of the `$gte` guard in
    /// `driven/mongo/demo_product/repository.rs` and needs an integration test
    /// against a real database to verify.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_orders_never_oversell() {
        const CONTENDERS: i32 = 8;
        let available = CONTENDERS - 1;

        let harness = harness_with(
            FakeDemoProductRepository::racing(available, CONTENDERS as usize),
            FakeDemoOrderRepository::default(),
            true,
        );
        let service = Arc::new(harness.service);

        // `.collect()` is load-bearing: a lazy iterator would spawn one task,
        // block on it at the first `await`, and deadlock against a barrier
        // that is still waiting for the other contenders.
        let attempts: Vec<_> = (0..CONTENDERS)
            .map(|_| {
                let service = service.clone();
                tokio::spawn(
                    async move { service.create_order(&user_id(), &product_id(), 1).await },
                )
            })
            .collect();

        let mut succeeded = 0;
        for attempt in attempts {
            if attempt.await.unwrap().is_ok() {
                succeeded += 1;
            }
        }

        assert_eq!(succeeded, available, "exactly the available units may be sold");
        assert_eq!(harness.products.stock(), 0, "stock must never go negative");
        assert_eq!(harness.orders.len() as i32, available);
    }
}
