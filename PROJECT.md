# Políticas del Servicio

Este documento registra las reglas de negocio, la postura de autenticación y las garantías de consistencia acordadas para el servicio construido sobre esta plantilla. Antes de implementar una regla que no figure aquí, debe acordarse y documentarse en la sección correspondiente.

## 1. Dominio y Reglas de Negocio

- Los agregados `DemoUser`, `DemoProduct` y `DemoOrder` son ejemplos de referencia para ilustrar la conexión entre capas; sus decisiones no constituyen políticas de producción.
- El borrado de recursos es físico por defecto. Si un agregado requiere borrado lógico, retención histórica o auditoría, se especificará aquí junto con su impacto en índices únicos y consultas.

## 2. Autenticación y Autorización

- Estado actual: **Público (plantilla base)**. La infraestructura de verificación JWT/JWKS (`Auth0Claims`, `HydraClaims`, `HydraMachineToMachineClaims`) está disponible pero desconectada por defecto.
- Antes de exponer endpoints de negocio en un entorno compartido o productivo, se debe activar `auth_middleware` y declarar permisos por método HTTP, o dejar registrada en esta sección la justificación aprobada por IT/Security para mantener rutas públicas.

## 3. Consistencia, Dinero e Idempotencia

- Las operaciones sobre un único documento utilizan condiciones atómicas en base de datos (como la reserva de stock con `$gte` e `$inc` y las restricciones de unicidad por índice).
- Las operaciones que abarquen múltiples escrituras o efectos externos deben definir aquí su estrategia de consistencia (transacciones, patrón outbox, tareas idempotentes con nombre o compensación) y la representación monetaria exacta (enteros en unidad mínima o decimal de precisión fija, nunca `f64`).
