// ---------------------------------------------------------------------------
// Domain Service — Pure business logic without I/O.
//
// Domain services operate exclusively on domain entities and primitives.
// They do NOT depend on ports, repositories, or external infrastructure.
// They can be instantiated anywhere and called directly from application
// services or other domain services.
// ---------------------------------------------------------------------------

use crate::domain::entities::demo_order::DemoOrder;

/// Example domain service — pure calculation with no side effects.
pub struct DemoPricingService;

impl DemoPricingService {
    /// Creates a new `DemoPricingService`.
    ///
    /// Note: no constructor parameters — this service is stateless.
    pub fn new() -> Self {
        Self
    }

    /// Applies a volume discount to an order's total price.
    ///
    /// Business rule: orders over 1000 units get a 10% discount.
    pub fn apply_discount(&self, order: &DemoOrder) -> f64 {
        if order.total_price > 1000.0 { order.total_price * 0.90 } else { order.total_price }
    }

    /// Calculates tax for a given subtotal.
    ///
    /// Business rule: flat 16% VAT.
    pub fn calculate_tax(&self, subtotal: f64) -> f64 {
        subtotal * 0.16
    }
}

impl Default for DemoPricingService {
    fn default() -> Self {
        Self::new()
    }
}
