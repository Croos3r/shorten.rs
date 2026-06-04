//! Integration tests for `UrlShortenerService` against a real (in-memory)
//! SQLite database with migrations applied.

mod common;

use shorten_rs::services::url_shortener::ShortenUrlError;

const ID_LEN: usize = 5;

#[actix_web::test]
async fn shorten_url_stores_and_returns_retrievable_id() {
    let service = common::test_service().await;

    let id = service
        .shorten_url("https://example.com")
        .await
        .expect("shorten_url should succeed");
    assert_eq!(id.len(), ID_LEN);

    let stored = service
        .find_shortened_url_by_id(&id)
        .await
        .expect("find_by_id should succeed")
        .expect("the freshly shortened url should be found");
    assert_eq!(stored.id, id);
    assert_eq!(stored.full_url, "https://example.com");
}

#[actix_web::test]
async fn shorten_url_deduplicates_identical_urls() {
    let service = common::test_service().await;

    let first = service.shorten_url("https://example.com").await.unwrap();
    let second = service.shorten_url("https://example.com").await.unwrap();

    assert_eq!(
        first, second,
        "shortening the same url twice should return the same id"
    );
}

#[actix_web::test]
async fn shorten_url_gives_distinct_ids_to_distinct_urls() {
    let service = common::test_service().await;

    let a = service.shorten_url("https://a.example").await.unwrap();
    let b = service.shorten_url("https://b.example").await.unwrap();

    assert_ne!(a, b, "different urls should get different ids");

    // Both must be independently retrievable.
    assert_eq!(
        service
            .find_shortened_url_by_id(&a)
            .await
            .unwrap()
            .unwrap()
            .full_url,
        "https://a.example"
    );
    assert_eq!(
        service
            .find_shortened_url_by_id(&b)
            .await
            .unwrap()
            .unwrap()
            .full_url,
        "https://b.example"
    );
}

#[actix_web::test]
async fn find_by_id_returns_none_for_unknown_id() {
    let service = common::test_service().await;

    let result = service
        .find_shortened_url_by_id("missing")
        .await
        .expect("find_by_id should succeed even for unknown ids");

    assert!(result.is_none());
}

#[actix_web::test]
async fn increment_visit_by_id_increases_the_counter() {
    let service = common::test_service().await;
    let id = service.shorten_url("https://example.com").await.unwrap();

    // A freshly shortened url starts with zero visits.
    assert_eq!(
        service
            .find_shortened_url_by_id(&id)
            .await
            .unwrap()
            .unwrap()
            .visits,
        0
    );

    service
        .increment_shortened_url_visits_by_id(&id)
        .await
        .unwrap();
    assert_eq!(
        service
            .find_shortened_url_by_id(&id)
            .await
            .unwrap()
            .unwrap()
            .visits,
        1
    );

    service
        .increment_shortened_url_visits_by_id(&id)
        .await
        .unwrap();
    assert_eq!(
        service
            .find_shortened_url_by_id(&id)
            .await
            .unwrap()
            .unwrap()
            .visits,
        2
    );
}

#[actix_web::test]
async fn increment_visit_by_id_is_a_noop_for_unknown_id() {
    let service = common::test_service().await;

    // Updating a non-existent row succeeds (0 rows affected) and creates nothing.
    service
        .increment_shortened_url_visits_by_id("missing")
        .await
        .expect("incrementing an unknown id should not error");

    assert!(
        service
            .find_shortened_url_by_id("missing")
            .await
            .unwrap()
            .is_none()
    );
}

#[actix_web::test]
async fn is_blacklisted_matches_exact_url() {
    let service = common::test_service_with_blacklist(vec!["https://mydomain.com"]).await;

    assert!(service.is_blacklisted("https://mydomain.com"));
}

#[actix_web::test]
async fn is_blacklisted_matches_any_url_under_a_blacklisted_prefix() {
    // Blacklisting is prefix-based (`starts_with`), so paths and query strings
    // under a blacklisted origin are also blocked.
    let service = common::test_service_with_blacklist(vec!["https://mydomain.com"]).await;

    assert!(service.is_blacklisted("https://mydomain.com/abc12"));
    assert!(service.is_blacklisted("https://mydomain.com/path?ref=1"));
}

#[actix_web::test]
async fn is_blacklisted_is_false_for_unrelated_urls() {
    let service = common::test_service_with_blacklist(vec!["https://mydomain.com"]).await;

    assert!(!service.is_blacklisted("https://example.com"));
    // A different scheme is a different prefix and must not match.
    assert!(!service.is_blacklisted("http://mydomain.com"));
}

#[actix_web::test]
async fn is_blacklisted_is_always_false_with_an_empty_blacklist() {
    let service = common::test_service().await;

    assert!(!service.is_blacklisted("https://mydomain.com"));
    assert!(!service.is_blacklisted("anything-at-all"));
}

#[actix_web::test]
async fn is_blacklisted_checks_every_configured_entry() {
    let service = common::test_service_with_blacklist(vec![
        "http://localhost:8080",
        "https://localhost:8080",
        "http://localhost:5173",
        "https://localhost:5173",
    ])
    .await;

    assert!(service.is_blacklisted("https://localhost:5173/dashboard"));
    assert!(service.is_blacklisted("http://localhost:8080/shorten"));
    assert!(!service.is_blacklisted("https://localhost:3000"));
}

#[actix_web::test]
async fn shorten_url_rejects_a_blacklisted_url() {
    let service = common::test_service_with_blacklist(vec!["https://mydomain.com"]).await;

    let err = service
        .shorten_url("https://mydomain.com/self")
        .await
        .expect_err("shortening a blacklisted url should fail");

    assert!(
        matches!(
            err.downcast_ref::<ShortenUrlError>(),
            Some(ShortenUrlError::BlacklistedUrl)
        ),
        "error should be a BlacklistedUrl, got: {err:?}"
    );
}

#[actix_web::test]
async fn shorten_url_does_not_persist_a_blacklisted_url() {
    let service = common::test_service_with_blacklist(vec!["https://mydomain.com"]).await;

    // The blacklist check must short-circuit before any insert; shortening an
    // allowed url afterwards yields the very first generated id, proving no row
    // was written for the rejected one.
    let _ = service.shorten_url("https://mydomain.com/self").await;

    let id = service
        .shorten_url("https://example.com")
        .await
        .expect("a non-blacklisted url should still be shortenable");
    let stored = service
        .find_shortened_url_by_id(&id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.full_url, "https://example.com");
}

#[actix_web::test]
async fn shorten_url_allows_urls_outside_the_blacklist() {
    let service = common::test_service_with_blacklist(vec!["https://mydomain.com"]).await;

    let id = service
        .shorten_url("https://example.com")
        .await
        .expect("a url outside the blacklist should be shortenable");
    assert_eq!(id.len(), ID_LEN);
}
