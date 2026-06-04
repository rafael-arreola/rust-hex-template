// ---------------------------------------------------------------------------
// Application Shared — Reusable sub-flows WITH I/O.
//
// These are cross-cutting orchestration helpers used by multiple application
// services. They depend on domain ports (repositories) and/or external
// clients. They are NOT domain services — they perform I/O.
//
// Example (uncomment when needed):
//
//   pub struct FraudChecker {
//       payment_client: Arc<PaymentClient>,
//   }
//
//   impl FraudChecker {
//       pub fn new(payment_client: Arc<PaymentClient>) -> Self { ... }
//       pub async fn check(&self, customer_id: &str) -> DomainResult<()> { ... }
//   }
// ---------------------------------------------------------------------------
