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

# A throwaway migrated database so the compile-time `query!` checks pass. The
# running app applies migrations itself (see src/main.rs), so this DB is only
# needed during the build.
RUN for f in migrations/*.sql; do sqlite3 /tmp/compile.sqlite < "$f"; done
ENV DATABASE_URL=sqlite:/tmp/compile.sqlite

RUN cargo build --release

# ---- Runtime ----
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# curl is used by the container healthcheck; libssl3 by tls-native-tls.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/shorten-rs /usr/local/bin/shorten-rs

# The app reads these at startup (see src/main.rs). 0.0.0.0 makes the published
# port reachable from outside the container; it creates and migrates the
# database file on first start.
ENV HOST=0.0.0.0 \
    PORT=8080 \
    DATABASE_URL=sqlite:/data/database.sqlite

EXPOSE 8080
VOLUME ["/data"]

CMD ["shorten-rs"]
