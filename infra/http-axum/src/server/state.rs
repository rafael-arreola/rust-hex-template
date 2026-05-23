use axum::extract::FromRef;
use std::sync::Arc;
use usecases::{order::OrderService, product::ProductService, user::UserService};

#[derive(Clone)]
pub struct AppState {
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

impl_from_ref!(AppState, user_service, UserService);
impl_from_ref!(AppState, product_service, ProductService);
impl_from_ref!(AppState, order_service, OrderService);
