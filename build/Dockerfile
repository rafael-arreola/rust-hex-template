# syntax=docker/dockerfile:1
####################################################################################################
## Base Stage - Configures fast linker, certificates and build tooling for all stages
####################################################################################################
FROM rust:1.99.0-bookworm AS base
RUN update-ca-certificates

# Install LLD linker to make compiling and linking significantly faster across all stages
RUN apt-get update && apt-get install -y lld clang && rm -rf /var/lib/apt/lists/*
ENV RUSTFLAGS="-C link-arg=-fuse-ld=lld"
# Prevent "Error in the HTTP2 framing layer" network errors in container/CI environments
ENV CARGO_HTTP_MULTIPLEXING=false

# cargo-chef is installed once, pinned, so planner and cacher reuse this cached layer
RUN cargo install cargo-chef --version 0.1.78 --locked && rustup component add rustfmt

####################################################################################################
## Planner Stage - generates the dependency recipe
####################################################################################################
FROM base AS planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

####################################################################################################
## Cacher Stage - builds only dependencies (cached between builds)
####################################################################################################
FROM base AS cacher
WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --locked --recipe-path recipe.json

####################################################################################################
## Builder Stage - builds only your source code
####################################################################################################
FROM base AS builder
WORKDIR /app

COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo

COPY . .

# Deploy gate: formatting must match before a release is built
RUN cargo fmt --all -- --check

RUN cargo build --release --locked

# Copy the compiled binary under a fixed name, whatever the package is called. Symbols are already
# stripped by `profile.release.strip = true` in Cargo.toml, so no extra `strip` pass is needed.
RUN BIN_PATH=$(find target/release -maxdepth 1 -type f -executable ! -name "*.so" ! -name "*.d" | head -n 1) && \
    cp "$BIN_PATH" /app/service

####################################################################################################
## Final image Stage
####################################################################################################
FROM gcr.io/distroless/cc-debian12
WORKDIR /app

COPY --from=builder --chown=10001:10001 --chmod=755 /app/service ./service

USER 10001:10001

CMD ["/app/service"]
