####################################################################################################
## Base Stage - Configures fast linker and certificates for all stages
####################################################################################################
FROM rust:bookworm AS base
RUN update-ca-certificates

# Install LLD linker to make compiling and linking significantly faster across all stages
RUN apt-get update && apt-get install -y lld clang && rm -rf /var/lib/apt/lists/*
ENV RUSTFLAGS="-C link-arg=-fuse-ld=lld"

####################################################################################################
## Planner Stage - generates the dependency recipe
####################################################################################################
FROM base AS planner
RUN cargo install cargo-chef
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

####################################################################################################
## Cacher Stage - builds only dependencies (cached between builds)
####################################################################################################
FROM base AS cacher
RUN cargo install cargo-chef
WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

####################################################################################################
## Builder Stage - builds only your source code
####################################################################################################
FROM base AS builder
WORKDIR /app

COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo

COPY . .

RUN cargo build --release

RUN strip -s /app/target/release/service

####################################################################################################
## Final image Stage
####################################################################################################
FROM gcr.io/distroless/cc-debian12
WORKDIR /app

COPY --from=builder --chown=10001:10001 --chmod=755 /app/target/release/service ./service

USER 10001:10001

CMD ["/app/service"]
