####################################################################################################
## Builder Stage
####################################################################################################
FROM rust:bookworm AS builder

# 1. Instalar dependencias del sistema
RUN apt-get update && \
    apt-get install -y protobuf-compiler jq && \
    rm -rf /var/lib/apt/lists/*

# 2. Configurar usuario
ENV USER=app
ENV UID=10001
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"

WORKDIR /app

# 3. Cacheo de Dependencias
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/bin && \
    echo "fn main() {println!(\"dummy\")}" > src/main.rs && \
    echo "fn main() {println!(\"dummy\")}" > src/bin/dummy.rs

RUN cargo build --release

# 4. Compilación Real
RUN rm src/*.rs
COPY . .
RUN touch src/main.rs
RUN cargo build --release --bin rustlang-ddd-hex

####################################################################################################
## Final Image Stage (Debian + NGINX)
####################################################################################################
FROM debian:bookworm-slim

# 1. Instalar NGINX y Certificados
RUN apt-get update && \
    apt-get install -y nginx ca-certificates curl && \
    rm -rf /var/lib/apt/lists/* && \
    rm /etc/nginx/sites-enabled/default

# 2. Copiar configuración NGINX y Entrypoint
COPY build/nginx/nginx.conf /etc/nginx/conf.d/default.conf
COPY build/nginx/entrypoint.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh

# 3. Copiar Usuarios y Binario Rust
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group
WORKDIR /app
COPY --from=builder /app/target/release/rustlang-ddd-hex ./service

# 4. Configurar Entorno
# El puerto que expone el contenedor es el que usa NGINX ($PORT), no el de Rust
ENV PORT=8080 
EXPOSE 8080

# Usamos el script de entrada para levantar ambos procesos
ENTRYPOINT ["/app/entrypoint.sh"]