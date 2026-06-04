use application::{order::OrderService, product::ProductService, user::UserService};
use axum::extract::FromRef;
use std::sync::Arc;

use crate::server::health::HealthChecker;

#[derive(Clone)]
pub struct AppState {
    pub health_checker: HealthChecker,
    pub user_service: Arc<UserService>,
    pub product_service: Arc<ProductService>,
    pub order_service: Arc<OrderService>,
}

/// Declares a `FromRef` impl for a service type inside `AppState`.
macro_rules! impl_from_ref {
    ($state:ty, $field:ident, $service:ty) => {
        impl FromRef<$state> for Arc<$service> {
            fn from_ref(state: &$state) -> Self {
                state.$field.clone()
            }
        }
    };
}

impl FromRef<AppState> for HealthChecker {
    fn from_ref(state: &AppState) -> Self {
        state.health_checker.clone()
    }
}

impl_from_ref!(AppState, user_service, UserService);
impl_from_ref!(AppState, product_service, ProductService);
impl_from_ref!(AppState, order_service, OrderService);
