# syntax=docker/dockerfile:1
# 构建：podman build -t cooservice:latest .
# 运行：见 compose.yml

FROM rust:1.98-bookworm AS builder

WORKDIR /src

# rust-s3 默认走 native-tls，编译期需要 OpenSSL 头文件
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release -p router \
    && cp target/release/router /usr/local/bin/router

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/bin/router /usr/local/bin/router
COPY migrations /migrations

ENV RUST_LOG=info \
    BIND_ADDR=0.0.0.0:8081 \
    REQUEST_TIMEOUT_SECS=30 \
    MIGRATIONS_DIR=/migrations

# 容器内默认监听 8081；DATABASE_URL 必须外部注入，镜像里不带默认值
EXPOSE 8081

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8081/healthz || exit 1

USER 65532:65532

CMD ["router"]
