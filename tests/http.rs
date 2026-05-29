//! End-to-end HTTP tests driving the real Actix routes (via `configure`)
//! against an in-memory database.

mod common;

use actix_web::{
    App,
    http::{StatusCode, header},
    test,
    web::Data,
};
use shorten_rs::configure;

/// Initialises an Actix test app wired to the given service.
///
/// A macro rather than a function so we don't have to name Actix's nested
/// `Service` return type (which references crates we don't depend on directly).
macro_rules! init_app {
    ($service:expr) => {
        test::init_service(
            App::new()
                .app_data(Data::new($service))
                .configure(configure),
        )
        .await
    };
}

#[actix_web::test]
async fn post_shorten_with_valid_url_returns_200_and_id() {
    let app = init_app!(common::test_service().await);

    let req = test::TestRequest::post()
        .uri("/shorten?url=https://example.com")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    assert_eq!(body.len(), 5, "id body should be 5 chars, got {body:?}");
}

#[actix_web::test]
async fn post_shorten_with_invalid_url_returns_400() {
    let app = init_app!(common::test_service().await);

    let req = test::TestRequest::post()
        .uri("/shorten?url=not-a-url")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn post_shorten_without_url_param_returns_400() {
    let app = init_app!(common::test_service().await);

    let req = test::TestRequest::post().uri("/shorten").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn get_existing_id_redirects_to_full_url() {
    let service = common::test_service().await;
    let id = service.shorten_url("https://example.com").await.unwrap();
    let app = init_app!(service);

    let req = test::TestRequest::get().uri(&format!("/{id}")).to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);
    let location = resp
        .headers()
        .get(header::LOCATION)
        .expect("redirect must set a Location header");
    assert_eq!(location, "https://example.com");
}

#[actix_web::test]
async fn get_unknown_id_returns_404() {
    let app = init_app!(common::test_service().await);

    let req = test::TestRequest::get().uri("/missing").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn shorten_then_redirect_round_trip() {
    let app = init_app!(common::test_service().await);

    // Shorten.
    let shorten = test::TestRequest::post()
        .uri("/shorten?url=https://round.trip.example/page")
        .to_request();
    let shorten_resp = test::call_service(&app, shorten).await;
    assert_eq!(shorten_resp.status(), StatusCode::OK);
    let id = String::from_utf8(test::read_body(shorten_resp).await.to_vec()).unwrap();

    // Redirect using the returned id.
    let redirect = test::TestRequest::get().uri(&format!("/{id}")).to_request();
    let redirect_resp = test::call_service(&app, redirect).await;

    assert_eq!(redirect_resp.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        redirect_resp.headers().get(header::LOCATION).unwrap(),
        "https://round.trip.example/page"
    );
}
