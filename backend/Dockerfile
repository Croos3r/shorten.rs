# syntax=docker/dockerfile:1

# ---- Builder ----
FROM rust:1-bookworm AS builder
WORKDIR /app

# sqlite3 builds the compile-time database that sqlx's `query!` macros verify
# against; openssl/pkg-config are needed by the tls-native-tls feature.
RUN apt-get update \
    && apt-get install -y --no-install-recommends sqlite3 pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY src ./src

# Build a migrated SQLite database so the compile-time `query!` checks pass,
# and keep it around to seed runtime volumes from a known-good schema.
RUN mkdir -p /build \
    && for f in migrations/*.sql; do sqlite3 /build/template.sqlite < "$f"; done
ENV DATABASE_URL=sqlite:/build/template.sqlite

RUN cargo build --release

# ---- Runtime ----
FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/shorten-rs /usr/local/bin/shorten-rs
COPY --from=builder /build/template.sqlite /app/template.sqlite
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

# The app reads these at startup (see src/main.rs). 0.0.0.0 makes the published
# port reachable from outside the container.
ENV HOST=0.0.0.0 \
    PORT=8080 \
    DATABASE_URL=sqlite:/data/database.sqlite

EXPOSE 8080
VOLUME ["/data"]

ENTRYPOINT ["docker-entrypoint.sh"]
CMD ["shorten-rs"]
