mod common;

use actix_session::{Session, SessionMiddleware, storage::CookieSessionStore};
use actix_web::{
    App, HttpResponse,
    cookie::Key,
    http::StatusCode,
    test,
    web::{self, Data, Path},
};
use shorten_rs::extractors::authenticated_user::AuthenticatedUser;

/// Reports the result of the [`AuthenticatedUser`] extractor: the resolved
/// user's email when one is found, or `anonymous` when the extractor yields
/// `None` (no cookie, or no matching user).
async fn whoami(user: AuthenticatedUser) -> HttpResponse {
    match user.0 {
        Some(user) => HttpResponse::Ok().body(user.email),
        None => HttpResponse::Ok().body("anonymous"),
    }
}

/// Writes an email into the session, standing in for a successful `/login` so
/// tests can obtain a signed session cookie without wiring up the real auth
/// service.
async fn set_email(session: Session, email: Path<String>) -> HttpResponse {
    session
        .insert("email", email.into_inner())
        .expect("inserting an email into the session should succeed");
    HttpResponse::Ok().finish()
}

/// Initialises a test app with a cookie session and the `/login` + `/whoami`
/// routes, registering whatever `app_data` values are passed.
///
/// A macro rather than a function so we don't have to name Actix's nested
/// `Service` return type, and so the "no `UsersService` registered" case can be
/// expressed by simply passing nothing.
macro_rules! init_session_app {
    ($($data:expr),* $(,)?) => {
        test::init_service(
            App::new()
                .wrap(SessionMiddleware::new(
                    CookieSessionStore::default(),
                    Key::generate(),
                ))
                $(.app_data($data))*
                .route("/login/{email}", web::get().to(set_email))
                .route("/whoami", web::get().to(whoami)),
        )
        .await
    };
}

/// Hits `/login/{email}` and returns the session cookie the middleware sets, so
/// it can be replayed onto a subsequent `/whoami` request.
macro_rules! login_cookie {
    ($app:expr, $email:expr) => {{
        let req = test::TestRequest::get()
            .uri(&format!("/login/{}", $email))
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

#[actix_web::test]
async fn no_session_cookie_yields_an_anonymous_user() {
    let app = init_session_app!(Data::new(common::test_users_service().await));

    let req = test::TestRequest::get().uri("/whoami").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        test::read_body(resp).await,
        "anonymous",
        "a request without a session cookie must resolve to None"
    );
}

#[actix_web::test]
async fn session_email_matching_a_known_user_resolves_to_that_user() {
    let app = init_session_app!(Data::new(
        common::test_users_service_with_user("Alice", "alice@example.com", "hunter2").await
    ));
    let cookie = login_cookie!(app, "alice@example.com");

    let req = test::TestRequest::get()
        .uri("/whoami")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        test::read_body(resp).await,
        "alice@example.com",
        "the extractor should load the user whose email is in the session"
    );
}

#[actix_web::test]
async fn session_email_with_no_matching_user_yields_an_anonymous_user() {
    // The session carries an email, but no such user exists in the database, so
    // `find_user_by_email` returns `Ok(None)` and the extractor yields `None`.
    let app = init_session_app!(Data::new(common::test_users_service().await));
    let cookie = login_cookie!(app, "ghost@example.com");

    let req = test::TestRequest::get()
        .uri("/whoami")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        test::read_body(resp).await,
        "anonymous",
        "an email with no matching user must resolve to None, not an error"
    );
}

#[actix_web::test]
async fn missing_users_service_returns_500() {
    // Without a `UsersService` in app_data the extractor cannot resolve a user
    // and must surface an internal server error rather than silently succeeding.
    let app = init_session_app!();

    let req = test::TestRequest::get().uri("/whoami").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
