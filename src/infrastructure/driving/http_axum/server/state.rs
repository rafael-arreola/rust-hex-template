use crate::application::{
    demo_order::DemoOrderService, demo_product::DemoProductService, demo_user::DemoUserService,
};
use axum::extract::FromRef;
use std::sync::Arc;

use crate::infrastructure::driving::http_axum::server::health::HealthChecker;

#[derive(Clone)]
pub struct AppState {
    pub health_checker: HealthChecker,
    pub demo_user_service: Arc<DemoUserService>,
    pub demo_product_service: Arc<DemoProductService>,
    pub demo_order_service: Arc<DemoOrderService>,
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

impl_from_ref!(AppState, demo_user_service, DemoUserService);
impl_from_ref!(AppState, demo_product_service, DemoProductService);
impl_from_ref!(AppState, demo_order_service, DemoOrderService);
