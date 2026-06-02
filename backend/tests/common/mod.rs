//! Shared test helpers.
//!
//! Living under `tests/common/` (rather than being a top-level `tests/*.rs`
//! file) keeps Cargo from compiling this as its own test binary; instead each
//! integration test pulls it in with `mod common;`.

use shorten_rs::DatabasePool;
use shorten_rs::services::url_shortener::UrlShortenerService;
use sqlx::sqlite::SqlitePoolOptions;

/// Builds a fresh, isolated in-memory SQLite pool with all migrations applied.
///
/// `max_connections(1)` keeps the single connection alive for the lifetime of
/// the pool, so the in-memory database (which lives only as long as its
/// connection) persists for the whole test.
pub async fn test_pool() -> DatabasePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("failed to open in-memory sqlite pool");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("failed to run migrations on test pool");
    pool
}

/// Convenience wrapper returning a service backed by a fresh in-memory database.
pub async fn test_service() -> UrlShortenerService {
    UrlShortenerService::new(test_pool().await, vec![])
}
