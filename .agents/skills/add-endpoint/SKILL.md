---
name: add-endpoint
description: Agregar un endpoint o caso de uso a una entidad existente del template — decide qué capas tocar (port, repository, service, handler, DTO, ruta) y en qué orden. Úsala cuando pidan un endpoint, ruta, operación o caso de uso nuevo sobre una entidad ya scaffoldeada.
---

# Agregar un endpoint a una entidad existente

Antes de escribir código, responde tres preguntas — cada "sí" agrega capas a tocar:

1. **¿Necesita un acceso a datos que hoy no existe?** → método nuevo en el port + implementación en el repository (+ índice si filtra/ordena por un campo nuevo).
2. **¿Es un caso de uso nuevo o una variante de uno existente?** → método nuevo en el `{Entity}Service`.
3. **¿Recibe body?** → DTO `*Input` con `Validate`. **¿Devuelve una forma nueva?** → DTO `*Output` con `From<Entity>`.

Orden de trabajo: port → repository → service → DTOs → handler → ruta.

## Paso a paso (ejemplo real: `PUT /api/v1/users/{id}`)

Este ejemplo es literal: `UserService::update_user` ya existe en `src/application/user.rs` pero **no tiene ruta registrada** — es el endpoint pendiente perfecto para calibrar el patrón.

### 1. Port y repository — solo si falta el acceso a datos

`UserRepositoryPort::update` ya existe; si tu caso necesita un método nuevo:

- Firma solo con tipos de dominio/primitivos, devuelve `DomainResult<T>`.
- Implementación en `repository.rs` con filtro `deleted_at: { "$exists": false }`, `.map_err(DomainError::database)`, y `$set` con `updated_at` cuando mute estado.
- Si la query filtra por un campo nuevo, agrega el índice en `create_indexes()` (idempotente, con nombre explícito).

### 2. Service — la lógica vive aquí

- `#[tracing::instrument(skip_all, fields(%id))]` — al menos un field siempre.
- Validación **semántica** (existencia, unicidad, reglas de negocio) contra los puertos; devuelve `DomainError` por constructor (`not_found`, `duplicate`, `business_rule`...).
- Nunca recibe ni devuelve DTOs.

### 3. DTO — `routes/user/dtos.rs`

```rust
#[derive(Deserialize, Validate)]
pub struct UpdateUserInput {
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    pub name: String,
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
}
```

Validación **sintáctica** (formato, longitud, rangos) va aquí, no en el service.

### 4. Handler — `routes/user.rs`

Cero lógica de negocio. Los 5 pasos canónicos:

```rust
#[tracing::instrument(skip_all)]
pub async fn update_user(
    State(service): State<Arc<UserService>>,
    Path(id): Path<String>,
    ValidatedBody(req): ValidatedBody<UpdateUserInput>,
) -> Result<GenericApiResponse<UserOutput>, ApiError> {
    let user_id = UserId::new(id);
    let user = service.update_user(&user_id, &req.name, &req.email).await?;
    Ok(GenericApiResponse::success(user.into()))
}
```

### 5. Ruta — `router()` de la entidad

```rust
.route("/{id}", get(get_user).put(update_user).delete(delete_user))
```

Rutas anidadas de relación (`GET /users/{id}/orders`) van en el router de la entidad **padre** de la URL, llamando al service correspondiente vía `AppState`.

## Reglas que el reviewer va a mirar

- El handler no toca puertos ni repositorios: solo el service.
- Respuesta siempre vía `GenericApiResponse` (`success` / `paginated`); jamás JSON manual. Los errores fluyen con `?` — `ApiError: From<DomainError>` centraliza código y status.
- Body de entrada siempre con `ValidatedBody<T>` (negocia JSON/MessagePack y valida). **Cuidado**: derivar `Validate` en structs de query (`UserQuery`) no ejecuta nada con el extractor `Query` plano — si necesitas validar query params, hazlo explícito en el handler o crea un extractor `ValidatedQuery` espejo de `ValidatedBody`.
- Listados: service devuelve `Vec<Entity>` + `count()`; el handler arma `GenericApiResponse::paginated(data, total, page, limit)`.
- Ni el handler ni el service loggean errores que propagan con `?` — el `From<DomainError> for ApiError` (`server/error.rs`) es el único punto de logging: registra el detalle interno (`Display`) con la severidad de `severity()` y responde al cliente solo con `public_message()`.
- No agregues variantes nuevas a `DomainError` sin actualizar sus **tres vistas** en `domain/error.rs` (`code()`, `public_message()`, `severity()`) y el mapeo de status en `server/error.rs`. Si la variante carga detalle de infraestructura, su `public_message()` debe ser genérico — el test `infrastructure_detail_never_leaks_into_public_message` te lo recuerda.

## Verificación

1. `cargo check` y luego `quality-gate`.
2. Test del service (skill `test-entity`) para la regla semántica nueva.
3. Smoke con curl verificando el envelope y el código de error estable (`cause`) en el caso negativo.
