# syntax=docker/dockerfile:1

FROM rust:1.98-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
COPY static ./static
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/love-journal /usr/local/bin/love-journal
COPY --from=builder /app/static ./static
COPY --from=builder /app/migrations ./migrations
ENV HOST=0.0.0.0 \
    PORT=8080 \
    DATABASE_URL=sqlite:///data/journal.db \
    UPLOAD_DIR=/data/uploads \
    COOKIE_SECURE=true \
    RUST_LOG=love_journal=info,tower_http=warn
VOLUME ["/data"]
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8080/healthz || exit 1
CMD ["love-journal"]
