# Multi-stage build for the StellarTrace API.
FROM rust:1-slim-bookworm AS builder
WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN cargo build --release -p stellartrace-api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/target/release/stellartrace-api /app/stellartrace-api
COPY dashboard /app/dashboard
COPY config /app/config

ENV STELLARTRACE_BIND_ADDR=0.0.0.0:8080
EXPOSE 8080

ENTRYPOINT ["/app/stellartrace-api"]
