FROM rust AS builder
WORKDIR /mnt
RUN --mount=type=bind,source=./code,target=/mnt cargo build --release --target-dir /tmp

FROM debian:bookworm-slim
COPY --from=builder /tmp/release/benchmark /usr/local/bin/benchmark
