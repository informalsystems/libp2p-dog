FROM rust:latest AS builder
WORKDIR /app
COPY . .
WORKDIR /app/benchmark
RUN cargo build --release --target-dir /tmp --bin libp2p-dog-benchmark

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /tmp/release/libp2p-dog-benchmark /usr/local/bin/benchmark
