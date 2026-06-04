//! Smoke test for the shared test database helper: migrations apply and the
//! schema is usable against a fresh in-memory database.

mod common;

#[actix_web::test]
async fn test_pool_applies_migrations_and_starts_empty() {
    let service = common::test_service().await;

    let missing = service
        .find_shortened_url_by_id("nope")
        .await
        .expect("query against migrated schema should succeed");

    assert!(
        missing.is_none(),
        "a fresh database should contain no shortened urls"
    );
}
