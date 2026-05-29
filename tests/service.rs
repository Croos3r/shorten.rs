//! Integration tests for `UrlShortenerService` against a real (in-memory)
//! SQLite database with migrations applied.

mod common;

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
        .find_by_id(&id)
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
        service.find_by_id(&a).await.unwrap().unwrap().full_url,
        "https://a.example"
    );
    assert_eq!(
        service.find_by_id(&b).await.unwrap().unwrap().full_url,
        "https://b.example"
    );
}

#[actix_web::test]
async fn find_by_id_returns_none_for_unknown_id() {
    let service = common::test_service().await;

    let result = service
        .find_by_id("missing")
        .await
        .expect("find_by_id should succeed even for unknown ids");

    assert!(result.is_none());
}
