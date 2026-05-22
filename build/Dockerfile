####################################################################################################
## Planner Stage - generates the dependency recipe
####################################################################################################
FROM rust:bookworm AS planner
RUN cargo install cargo-chef
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

####################################################################################################
## Cacher Stage - builds only dependencies (cached between builds)
####################################################################################################
FROM rust:bookworm AS cacher
RUN cargo install cargo-chef
WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

####################################################################################################
## Builder Stage - builds only your source code
####################################################################################################
FROM rust:bookworm AS builder
RUN update-ca-certificates

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

COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo

COPY . .

RUN cargo build --release

RUN strip -s /app/target/release/service && \
    cp /app/target/release/service /app/service

####################################################################################################
## Final image Stage
####################################################################################################
FROM gcr.io/distroless/cc-debian12
USER app:app
WORKDIR /app

COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

COPY --from=builder /app/service ./service

CMD ["/app/service"]
