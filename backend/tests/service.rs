//! Integration tests for `UrlShortenerService` against a real (in-memory)
//! SQLite database with migrations applied.

mod common;

use std::time::{Duration, SystemTime};

use shorten_rs::services::url_shortener::{ShortenUrlError, ShortenedUrl, UrlShortenerService};

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

// --- Expiration -----------------------------------------------------------
//
// `find_by_id`/`find_by_url` (used by the service) only return rows whose
// `expire_at` is still in the future; the `_with_expired` variants ignore the
// expiry. These tests seed rows directly through `ShortenedUrl::save` so we can
// control `expire_at` precisely, including times in the past.

/// Persists a single url with an explicit expiry, returning a service backed by
/// the same pool plus the pool itself for direct lookups.
async fn service_with_seeded_url(
    id: &str,
    url: &str,
    expire_at: SystemTime,
) -> (UrlShortenerService, shorten_rs::DatabasePool) {
    let pool = common::test_pool().await;
    ShortenedUrl::from_parts(id, url, 0u32, expire_at)
        .save(&pool)
        .await
        .expect("seeding a row should succeed");
    let service = UrlShortenerService::new(pool.clone(), vec![]);
    (service, pool)
}

#[actix_web::test]
async fn find_by_id_hides_an_expired_url() {
    let past = SystemTime::now() - Duration::from_secs(60);
    let (service, _pool) = service_with_seeded_url("exprd", "https://expired.example", past).await;

    let found = service
        .find_shortened_url_by_id("exprd")
        .await
        .expect("query should succeed");
    assert!(found.is_none(), "an expired url must not be returned");
}

#[actix_web::test]
async fn find_by_id_returns_a_not_yet_expired_url() {
    let future = SystemTime::now() + Duration::from_secs(3600);
    let (service, _pool) = service_with_seeded_url("alive", "https://alive.example", future).await;

    let found = service
        .find_shortened_url_by_id("alive")
        .await
        .expect("query should succeed")
        .expect("a url that has not expired must still be returned");
    assert_eq!(found.full_url, "https://alive.example");
}

#[actix_web::test]
async fn find_by_id_with_expired_still_returns_an_expired_url() {
    let past = SystemTime::now() - Duration::from_secs(60);
    let (_service, pool) = service_with_seeded_url("exprd", "https://expired.example", past).await;

    let found = ShortenedUrl::find_by_id_with_expired("exprd", &pool)
        .await
        .expect("query should succeed")
        .expect("the *_with_expired variant must ignore expiry");
    assert_eq!(found.full_url, "https://expired.example");
}

#[actix_web::test]
async fn find_by_url_with_expired_still_returns_an_expired_url() {
    let past = SystemTime::now() - Duration::from_secs(60);
    let (_service, pool) = service_with_seeded_url("exprd", "https://expired.example", past).await;

    let found = ShortenedUrl::find_by_url_with_expired("https://expired.example", &pool)
        .await
        .expect("query should succeed")
        .expect("the *_with_expired variant must ignore expiry");
    assert_eq!(found.id, "exprd");
}

#[actix_web::test]
async fn shorten_url_does_not_deduplicate_onto_an_expired_entry() {
    // Dedup keys off `find_by_url`, which excludes expired rows, so shortening a
    // url whose only existing entry has expired must mint a fresh id rather than
    // resurrect the dead one.
    let past = SystemTime::now() - Duration::from_secs(60);
    let (service, _pool) = service_with_seeded_url("exprd", "https://reuse.example", past).await;

    let new_id = service
        .shorten_url("https://reuse.example")
        .await
        .expect("shortening should succeed");

    assert_ne!(
        new_id, "exprd",
        "an expired entry must not be reused for dedup"
    );
    // The freshly minted id is retrievable; the expired one remains hidden.
    assert_eq!(
        service
            .find_shortened_url_by_id(&new_id)
            .await
            .unwrap()
            .unwrap()
            .full_url,
        "https://reuse.example"
    );
}

#[actix_web::test]
async fn re_shortening_a_valid_url_does_not_extend_its_expiry() {
    // Dedup is a *fixed* window, not a sliding one: shortening a url that
    // already has a live entry returns the existing id without touching
    // `expire_at`. This pins that behaviour so a switch to sliding TTL can't
    // happen silently.
    let future = SystemTime::now() + Duration::from_secs(3600);
    let (service, _pool) = service_with_seeded_url("ttlid", "https://ttl.example", future).await;

    // Read the persisted (second-granularity) expiry before re-shortening.
    let before = service
        .find_shortened_url_by_id("ttlid")
        .await
        .unwrap()
        .unwrap()
        .expire_at;

    let id = service
        .shorten_url("https://ttl.example")
        .await
        .expect("shortening should succeed");
    assert_eq!(id, "ttlid", "a live entry should be reused for dedup");

    let after = service
        .find_shortened_url_by_id("ttlid")
        .await
        .unwrap()
        .unwrap()
        .expire_at;
    assert_eq!(
        before, after,
        "re-shortening a still-valid url must not extend its expiry"
    );
}
