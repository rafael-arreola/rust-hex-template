---
name: architecture-audit
description: Auditar que el código respeta las invariantes del template — fronteras de capas, soft-delete, naming, higiene de Cargo.toml, tracing — mediante greps ejecutables con resultado esperado. Úsala antes de un PR grande, tras un merge conflictivo, o cuando pidan revisar/verificar la arquitectura.
---

# Auditoría de arquitectura

Cada check es un comando con su resultado esperado. Un check que falla es un hallazgo: repórtalo con archivo:línea y la invariante violada (referencia en `AGENTS.md`). No "arregles" nada sin confirmar que no es una excepción documentada.

## 1. Fronteras de capas (dependencias de módulos)

```bash
# domain no importa NADA fuera de sí mismo → esperado: vacío
grep -rn "use crate::" src/domain --include="*.rs" | grep -v "use crate::domain"

# application solo importa domain y shared → esperado: vacío
grep -rn "use crate::infrastructure" src/application --include="*.rs"

# driven (mongo/redis) no importa application ni driving → esperado: vacío
grep -rn "use crate::application\|use crate::infrastructure::driving" src/infrastructure/driven --include="*.rs"

# driving (http_axum) no importa driven → esperado: vacío
grep -rn "use crate::infrastructure::driven" src/infrastructure/driving --include="*.rs"

# shared no importa módulos locales → esperado: vacío
grep -rn "use crate::domain\|use crate::application\|use crate::infrastructure" src/shared --include="*.rs"
```

## 2. Fugas de tipos entre capas

```bash
# DTOs (*Input/*Output) solo existen en http_axum → esperado: vacío
grep -rEn "\b\w+(Input|Output)\b" src/domain src/application --include="*.rs"

# Tipos del driver BSON solo en el adapter mongo → esperado: vacío
grep -rn "ObjectId\|bson::" src/domain src/application src/infrastructure/driving --include="*.rs"

# Models (*Model) solo en mongo → esperado: vacío
grep -rEn "\b\w+Model\b" src/domain src/application src/infrastructure/driving --include="*.rs"
```

## 3. Persistencia: soft-delete y snake_case

```bash
# Sin hard-deletes → esperado: vacío
grep -rn "delete_one\|delete_many\|find_one_and_delete\|drop(" src --include="*.rs"

# Sin campos camelCase en documentos/queries → esperado: vacío
grep -rEn '"[a-z]+[A-Z][a-zA-Z]*"' src/infrastructure/driven/mongo --include="*.rs"

# Revisión manual: cada find_one/find/update_one/count_documents en repositorios
# debe filtrar deleted_at. Listar para inspección:
grep -rn "find_one(\|\.find(\|update_one(\|count_documents(" src/infrastructure/driven/mongo --include="*.rs"
```

Excepción esperada del último check: `create_indexes` y el `insert_one` de `create` no llevan filtro.

## 4. Errores y panics

```bash
# unwrap/expect fuera de tests (clippy ya lo niega en build; esto es doble check) 
cargo clippy --all-targets -- -D warnings 2>&1 | grep -i "unwrap\|expect" 

# Errores de driver sin mapear: todo .await? en repos debe venir de un map_err
# Revisión manual sobre:
grep -rn "\.await?" src/infrastructure/driven --include="*.rs" | grep -v "map_err" 
```

Todo error externo debe convertirse vía `DomainError::database` / `DomainError::internal` / `DomainError::external_service`.

```bash
# Separación público/privado: el message del cliente sale de public_message(),
# nunca de to_string()/Display, en el From<DomainError> → esperado: vacío
grep -n "message = err.to_string()\|message: err.to_string()" \
  src/infrastructure/driving/http_axum/server/error.rs

# Log único en la frontera: services y repos no loggean errores que propagan
# con `?` (el choke point de server/error.rs ya los loggea) → revisar cada hit;
# solo es legítimo loggear errores que NO se propagan (p. ej. degradación de cache)
grep -rn "tracing::error!\|tracing::warn!" src/application src/infrastructure/driven --include="*.rs"

# Toda variante nueva de DomainError debe cubrir las tres vistas → comparar conteos
grep -c "Self::" src/domain/error.rs   # code(), public_message() y severity() deben listar todas
```

## 5. Observabilidad

```bash
# TraceLayer prohibido (crea traces raíz desconectados) → esperado: vacío
grep -rn "TraceLayer" src --include="*.rs"

# reqwest::Client directo fuera de shared/http_client.rs → esperado: vacío
grep -rn "reqwest::Client::new\|reqwest::ClientBuilder" src --include="*.rs" | grep -v "shared/http_client.rs"

# tracing-stackdriver prohibido → esperado: vacío
grep -n "tracing-stackdriver" Cargo.toml

# Métodos públicos de services sin #[tracing::instrument] — listar y revisar:
grep -rn -B1 "pub async fn" src/application --include="*.rs" | grep -v "instrument"
```

Además: las versiones de `opentelemetry*`, `tracing-opentelemetry` y el feature de `reqwest-tracing` deben apuntar al mismo minor de OTel que exige el driver `mongodb` (hoy 0.31 / `opentelemetry_0_31`) — verificar en `Cargo.toml`.

## 6. Estructura de módulos y naming

```bash
# Convención moderna: nunca foo/mod.rs → esperado: vacío
find src -name "mod.rs"

# Archivos en plural (naming exige singular) → revisar cada hit
find src -name "*s.rs" | grep -vE "(entities|services|routes|values|macros|dtos|status)\.rs$"
```

Spot-checks manuales con la tabla de naming de `AGENTS.md`: traits `{Entity}RepositoryPort`, structs de infra sin prefijo tecnológico, colecciones plural snake_case, rutas plural, DTOs `*Input`/`*Output`.

## 7. Higiene de Cargo.toml

```bash
# Orden alfabético por grupos
cargo sort --grouped --check

# Dependencias declaradas pero no usadas (el test definitivo es cargo check sin la dep)
for dep in $(grep -E '^[a-z0-9_-]+ ?=' Cargo.toml | cut -d' ' -f1 | tr '-' '_'); do
  grep -rqn "${dep}::" src/ --include="*.rs" || grep -rqn "use ${dep}" src/ --include="*.rs" || echo "SOSPECHOSA: ${dep}"
done
```

Nota: el loop da falsos positivos con deps usadas solo vía derive/macros (`serde`, `thiserror`, `validator`, `futures`) o features transitivas (`rustls`) — confirmar cada sospechosa manualmente antes de reportar. Deps usadas solo en `#[cfg(test)]` pertenecen a `[dev-dependencies]`.

## 8. Wiring completo

Para cada entidad E con repositorio, verificar la cadena completa:

```bash
ENTITY=user  # cambiar por la entidad
grep -n "$ENTITY" src/domain/entities.rs src/domain/port.rs src/application.rs \
  src/infrastructure/driven/mongo.rs src/infrastructure/driving/http_axum/routes.rs \
  src/infrastructure/driving/http_axum/server/state.rs src/main.rs
```

Esperado: aparece en los 7. Además `main.rs` debe llamar `create_indexes()` con fail-fast, y `state.rs` debe tener el `impl_from_ref!`.

## Formato del reporte

Lista de hallazgos ordenada por severidad: (1) violaciones de frontera, (2) soft-delete/errores sin mapear, (3) observabilidad, (4) naming/higiene. Cada hallazgo: `archivo:línea`, invariante violada, y fix propuesto en una línea. Si un check pasa, no lo menciones.
