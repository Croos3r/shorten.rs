//! Integration tests for `UrlShortenerService` against a real (in-memory)
//! SQLite database with migrations applied.

mod common;

use std::time::{Duration, SystemTime};

use shorten_rs::dtos::ExpirationOptions;
use shorten_rs::services::url_shortener::{ShortenUrlError, ShortenedUrl, UrlShortenerService};

const ID_LEN: usize = 5;

#[actix_web::test]
async fn shorten_url_stores_and_returns_retrievable_id() {
    let service = common::test_service().await;

    let id = service
        .shorten_url("https://example.com", ExpirationOptions::Hour)
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
async fn shorten_url_gives_a_fresh_id_to_each_identical_url() {
    // Deduplication was removed: shortening the same url twice now mints two
    // independent entries rather than reusing the first id.
    let service = common::test_service().await;

    let first = service
        .shorten_url("https://example.com", ExpirationOptions::Hour)
        .await
        .unwrap();
    let second = service
        .shorten_url("https://example.com", ExpirationOptions::Hour)
        .await
        .unwrap();

    assert_ne!(
        first, second,
        "without dedup, the same url shortened twice should get distinct ids"
    );
    for id in [&first, &second] {
        assert_eq!(
            service
                .find_shortened_url_by_id(id)
                .await
                .unwrap()
                .unwrap()
                .full_url,
            "https://example.com",
            "both freshly minted ids must resolve back to the original url"
        );
    }
}

#[actix_web::test]
async fn shorten_url_gives_distinct_ids_to_distinct_urls() {
    let service = common::test_service().await;

    let a = service
        .shorten_url("https://a.example", ExpirationOptions::Hour)
        .await
        .unwrap();
    let b = service
        .shorten_url("https://b.example", ExpirationOptions::Hour)
        .await
        .unwrap();

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
    let id = service
        .shorten_url("https://example.com", ExpirationOptions::Hour)
        .await
        .unwrap();

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
        .shorten_url("https://mydomain.com/self", ExpirationOptions::Hour)
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
    // Build the service over an owned pool so we can inspect the table directly.
    let pool = common::test_pool().await;
    let service = UrlShortenerService::new(pool.clone(), vec!["https://mydomain.com".into()]);

    // The blacklist check must short-circuit before any insert. Assert the
    // rejection here too so the test stands on its own: a `shorten_url` that
    // wrongly returned `Ok` for a blacklisted url (even without persisting)
    // must not slip through just because the later non-persistence check passes.
    let err = service
        .shorten_url("https://mydomain.com/self", ExpirationOptions::Hour)
        .await
        .expect_err("shortening a blacklisted url should fail");
    assert!(
        matches!(
            err.downcast_ref::<ShortenUrlError>(),
            Some(ShortenUrlError::BlacklistedUrl)
        ),
        "error should be a BlacklistedUrl, got: {err:?}"
    );

    // Assert directly that no row was written for the rejected url (the
    // `_with_expired` variant ignores expiry, so even a stray row would surface)
    // rather than inferring non-persistence from an unrelated later shorten.
    let persisted = ShortenedUrl::find_by_url_with_expired("https://mydomain.com/self", &pool)
        .await
        .expect("query should succeed");
    assert!(
        persisted.is_none(),
        "a blacklisted url must never be persisted"
    );
}

#[actix_web::test]
async fn shorten_url_allows_urls_outside_the_blacklist() {
    let service = common::test_service_with_blacklist(vec!["https://mydomain.com"]).await;

    let id = service
        .shorten_url("https://example.com", ExpirationOptions::Hour)
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
async fn shorten_url_ignores_a_pre_existing_expired_entry_for_the_same_url() {
    // With dedup gone, shortening always mints a fresh id; this pins that a stale
    // expired row for the same url neither blocks the new entry nor resurfaces.
    //
    // The seeded id deliberately contains a `-`: `shorten_url` only mints
    // alphanumeric ids, so this can never collide with a generated one, keeping
    // the `assert_ne!` about logical reuse rather than RNG luck.
    let seeded_id = "exp-d";
    let past = SystemTime::now() - Duration::from_secs(60);
    let (service, _pool) = service_with_seeded_url(seeded_id, "https://reuse.example", past).await;

    let new_id = service
        .shorten_url("https://reuse.example", ExpirationOptions::Hour)
        .await
        .expect("shortening should succeed");

    assert_ne!(
        new_id, seeded_id,
        "a fresh id must be minted, not the expired one"
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
    assert!(
        service
            .find_shortened_url_by_id(seeded_id)
            .await
            .unwrap()
            .is_none(),
        "the pre-existing expired entry must stay hidden"
    );
}

#[actix_web::test]
async fn re_shortening_a_valid_url_creates_a_new_entry_without_touching_the_original() {
    // Without dedup, re-shortening a url that already has a live entry mints a
    // *separate* entry and leaves the original's id and `expire_at` untouched.
    //
    // The seeded id contains a `-` so it can never collide with the alphanumeric
    // id `shorten_url` generates, keeping the new-entry assertions collision-proof.
    let seeded_id = "ttl-d";
    let future = SystemTime::now() + Duration::from_secs(3600);
    let (service, _pool) = service_with_seeded_url(seeded_id, "https://ttl.example", future).await;

    // Read the persisted (second-granularity) expiry before re-shortening.
    let before = service
        .find_shortened_url_by_id(seeded_id)
        .await
        .unwrap()
        .unwrap()
        .expire_at;

    let id = service
        .shorten_url("https://ttl.example", ExpirationOptions::Hour)
        .await
        .expect("shortening should succeed");
    assert_ne!(
        id, seeded_id,
        "re-shortening should mint a new id, not reuse the live one"
    );

    let after = service
        .find_shortened_url_by_id(seeded_id)
        .await
        .unwrap()
        .unwrap()
        .expire_at;
    assert_eq!(
        before, after,
        "the original entry's expiry must be untouched by an unrelated new shorten"
    );
}
