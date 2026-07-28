---
name: add-domain-service
description: Agregar un domain service — lógica de negocio pura, sin I/O ni dependencias — al módulo domain/services. Úsala cuando una regla de negocio (cálculo, política, validación compleja) se repita entre casos de uso o no pertenezca a una sola entidad.
---

# Agregar un domain service

Un domain service es cálculo puro sobre entidades y primitivos. Si necesita un repositorio, un cliente HTTP o cualquier `await`, **no** es un domain service: eso va en `application/{entity}.rs` (caso de uso) o en `application/shared/` (sub-flujo reutilizable con I/O).

## Dónde y cómo

Archivo: `src/domain/services/{service}.rs` — registrar `pub mod {service};` en `src/domain/services/mod.rs`.

Ejemplo canónico en el repo: `src/domain/services/demo_pricing.rs`.

```rust
use crate::domain::entities::demo_order::DemoOrder;

/// Pure business logic — zero I/O, zero constructor dependencies.
pub struct DiscountPolicy;

impl DiscountPolicy {
    pub fn new() -> Self {
        Self
    }

    /// Regla: pedidos > 1000 obtienen 10% de descuento.
    pub fn apply(&self, order: &DemoOrder) -> f64 {
        if order.total_price > 1000.0 { order.total_price * 0.90 } else { order.total_price }
    }
}

impl Default for DiscountPolicy {
    fn default() -> Self {
        Self::new()
    }
}
```

## Reglas

- Stateless: constructor sin parámetros (`new()` + `impl Default`).
- Opera exclusivamente sobre entidades de dominio y primitivos; cero imports fuera de `crate::domain`.
- Sin `async`, sin `Result` de infraestructura. Si la regla puede fallar como regla de negocio, devuelve `DomainResult<T>` con `DomainError::business_rule(...)`.
- Se invoca **solo** desde application services (o desde otros domain services) — nunca desde handlers ni repositorios.

## Integración en un caso de uso

En el `{Entity}Service` de application, el domain service es un campo construido en `new()` — no se inyecta por `Arc` porque no tiene estado ni I/O:

```rust
#[derive(Clone)]
pub struct DemoOrderService {
    demo_order_repo: Arc<dyn DemoOrderRepositoryPort>,
    discount: DiscountPolicy,
}

impl DemoOrderService {
    pub fn new(demo_order_repo: Arc<dyn DemoOrderRepositoryPort>) -> Self {
        Self { demo_order_repo, discount: DiscountPolicy::new() }
    }
}
```

Nota: el campo debe implementar `Clone` o construirse barato; para structs unitarios agrega `#[derive(Clone)]` al domain service si el application service lo necesita.

## Tests

La pureza los hace triviales — tests unitarios inline, sin tokio:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_discount_over_threshold() {
        let order = DemoOrder { total_price: 2000.0, /* ... */ };
        assert_eq!(DiscountPolicy::new().apply(&order), 1800.0);
    }
}
```

Todo domain service nuevo entra con sus tests en el mismo PR: es la capa más barata de testear del sistema y la que más duele si se rompe en silencio.
