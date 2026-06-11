//! End-to-end tests for the custom-expiration feature on `POST /shorten`.
//!
//! The rule under test (controller): `user.and(expire_in).unwrap_or_default()`.
//! A requested `expire_in` is honoured **only** when the request carries a
//! session for a known user; anonymous requests fall back to the one-hour
//! default. We assert on the persisted `expire_at` rather than on a (currently
//! absent) response field, reading it back through the same service the handler
//! writes to.

mod common;

use std::time::{Duration, SystemTime};

use actix_session::{Session, SessionMiddleware, storage::CookieSessionStore};
use actix_web::{
    App, HttpResponse,
    cookie::Key,
    http::StatusCode,
    test,
    web::{self, Data, Path},
};
use shorten_rs::configure;
use shorten_rs::services::url_shortener::UrlShortenerService;
use shorten_rs::services::users::UsersService;

const SEEDED_EMAIL: &str = "alice@example.com";

/// Writes an email into the session, standing in for a successful `/login` so a
/// test can obtain a signed session cookie without driving the real auth flow.
async fn set_email(session: Session, email: Path<String>) -> HttpResponse {
    session
        .insert("email", email.into_inner())
        .expect("inserting an email into the session should succeed");
    HttpResponse::Ok().finish()
}

/// Builds a url-shortener service plus a users service pre-seeded with a single
/// known user, so the `AuthenticatedUser` extractor has someone to resolve.
async fn seeded_services() -> (UrlShortenerService, UsersService) {
    let url_service = common::test_service().await;
    let users_service =
        common::test_users_service_with_user("Alice", SEEDED_EMAIL, "hunter2").await;
    (url_service, users_service)
}

/// Initialises a test app with a cookie session, the real routes (via
/// `configure`), and a `/testlogin/{email}` helper for minting session cookies.
///
/// A macro rather than a function so we don't have to name Actix's nested
/// `Service` return type.
macro_rules! init_app {
    ($url_service:expr, $users_service:expr) => {
        test::init_service(
            App::new()
                .wrap(SessionMiddleware::new(
                    CookieSessionStore::default(),
                    Key::generate(),
                ))
                .app_data(Data::new($url_service))
                .app_data(Data::new($users_service))
                .route("/testlogin/{email}", web::get().to(set_email))
                .configure(configure),
        )
        .await
    };
}

/// Hits `/testlogin/{email}` and returns the session cookie the middleware sets,
/// so it can be replayed onto a subsequent authenticated request.
macro_rules! login_cookie {
    ($app:expr, $email:expr) => {{
        let req = test::TestRequest::get()
            .uri(&format!("/testlogin/{}", $email))
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        resp.response()
            .cookies()
            .next()
            .expect("login should set a session cookie")
            .into_owned()
    }};
}

/// POSTs `/shorten?{query}` (optionally with a session cookie), asserts a 200,
/// and returns the persisted `expire_at` of the stored row.
macro_rules! shorten_and_fetch_expiry {
    ($app:expr, $url_service:expr, $query:expr $(, $cookie:expr)?) => {{
        #[allow(unused_mut)]
        let mut req = test::TestRequest::post().uri(&format!("/shorten?{}", $query));
        $( req = req.cookie($cookie); )?
        let resp = test::call_service(&$app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::OK, "shorten should succeed");
        let id = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
        $url_service
            .find_shortened_url_by_id(&id)
            .await
            .expect("lookup should succeed")
            .expect("the freshly shortened url should be stored")
            .expire_at
    }};
}

#[actix_web::test]
async fn logged_in_user_can_set_a_custom_week_long_expiration() {
    let (url_service, users_service) = seeded_services().await;
    let app = init_app!(url_service.clone(), users_service);
    let cookie = login_cookie!(app, SEEDED_EMAIL);

    let t0 = SystemTime::now();
    let expire_at = shorten_and_fetch_expiry!(
        app,
        url_service,
        "url=https://example.com&expire_in=week",
        cookie
    );

    assert!(
        expire_at > t0 + Duration::from_hours(6 * 24),
        "a logged-in user's `week` request must push expiry ~7 days out, got {expire_at:?}"
    );
    assert!(
        expire_at < t0 + Duration::from_hours(8 * 24),
        "expiry should be ~7 days out, not further, got {expire_at:?}"
    );
}

#[actix_web::test]
async fn logged_in_user_can_set_a_one_day_expiration() {
    let (url_service, users_service) = seeded_services().await;
    let app = init_app!(url_service.clone(), users_service);
    let cookie = login_cookie!(app, SEEDED_EMAIL);

    let t0 = SystemTime::now();
    let expire_at = shorten_and_fetch_expiry!(
        app,
        url_service,
        "url=https://example.com&expire_in=day",
        cookie
    );

    assert!(
        expire_at > t0 + Duration::from_hours(23),
        "a `day` request should push expiry ~24h out, got {expire_at:?}"
    );
    assert!(
        expire_at < t0 + Duration::from_hours(25),
        "a `day` request should not exceed ~24h, got {expire_at:?}"
    );
}

#[actix_web::test]
async fn anonymous_user_expiration_is_ignored_and_defaults_to_one_hour() {
    let (url_service, users_service) = seeded_services().await;
    let app = init_app!(url_service.clone(), users_service);

    // No session cookie: the requested `week` must be discarded in favour of the
    // one-hour default.
    let t0 = SystemTime::now();
    let expire_at =
        shorten_and_fetch_expiry!(app, url_service, "url=https://example.com&expire_in=week");

    assert!(
        expire_at < t0 + Duration::from_hours(2),
        "an anonymous user's `week` must be ignored and default to ~1h, got {expire_at:?}"
    );
    assert!(
        expire_at > t0 + Duration::from_secs(50 * 60),
        "the default expiry should still be ~1h out, got {expire_at:?}"
    );
}

#[actix_web::test]
async fn logged_in_user_without_an_expiration_defaults_to_one_hour() {
    let (url_service, users_service) = seeded_services().await;
    let app = init_app!(url_service.clone(), users_service);
    let cookie = login_cookie!(app, SEEDED_EMAIL);

    // Authenticated, but no `expire_in` supplied: still the one-hour default.
    let t0 = SystemTime::now();
    let expire_at = shorten_and_fetch_expiry!(app, url_service, "url=https://example.com", cookie);

    assert!(
        expire_at < t0 + Duration::from_hours(2),
        "omitting `expire_in` should default to ~1h even when logged in, got {expire_at:?}"
    );
    assert!(
        expire_at > t0 + Duration::from_secs(50 * 60),
        "the default expiry should still be ~1h out, got {expire_at:?}"
    );
}

#[actix_web::test]
async fn a_session_for_an_unknown_user_does_not_unlock_custom_expiration() {
    // A cookie whose email matches no user resolves to an anonymous user, so the
    // custom `expire_in` must still be ignored — guards the `user.and(..)` gate
    // against keying off "has a cookie" rather than "is a real user".
    let (url_service, users_service) = seeded_services().await;
    let app = init_app!(url_service.clone(), users_service);
    let cookie = login_cookie!(app, "ghost@example.com");

    let t0 = SystemTime::now();
    let expire_at = shorten_and_fetch_expiry!(
        app,
        url_service,
        "url=https://example.com&expire_in=week",
        cookie
    );

    assert!(
        expire_at < t0 + Duration::from_hours(2),
        "an unrecognised session must not unlock custom expiry, got {expire_at:?}"
    );
}
