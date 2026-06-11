use actix_session::Session;
use actix_web::{
    HttpResponse, Responder, get, http, post,
    web::{Data, Path},
};
use actix_web_validator::Query;

use crate::{
    dtos::{LoginDto, RegisterDto, ShortenUrlDto},
    extractors::authenticated_user::AuthenticatedUser,
    services::{
        authentication::{AuthenticationService, UserRegistrationError},
        url_shortener::{ShortenUrlError, UrlShortenerService},
        users::User,
    },
};

#[post("/shorten")]
pub async fn shorten_url(
    url_shortener_service: Data<UrlShortenerService>,
    Query(ShortenUrlDto { url, expire_in, .. }): Query<ShortenUrlDto>,
    AuthenticatedUser(user): AuthenticatedUser,
) -> impl Responder {
    let expire_in = user.and(expire_in).unwrap_or_default();
    url_shortener_service
        .shorten_url(&url, expire_in)
        .await
        .map(|id| HttpResponse::Ok().body(id))
        .map_err(|err| {
            err.downcast::<ShortenUrlError>()
                .map(ShortenUrlError::into)
                .unwrap_or_else(|_| {
                    HttpResponse::InternalServerError().body("An unknown error has occured")
                })
        })
        .unwrap_or_else(|e| e)
}

#[get("/{id}")]
pub async fn redirect_to_url_for_id(
    id: Path<String>,
    url_shortener_service: Data<UrlShortenerService>,
) -> impl Responder {
    let shortened_url = match url_shortener_service.find_shortened_url_by_id(&id).await {
        Ok(Some(shortened_url)) => shortened_url,
        Ok(None) => return HttpResponse::NotFound().body("No url for this id"),
        Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
    };
    let _ = url_shortener_service
        .increment_shortened_url_visits_by_id(&id)
        .await
        .map_err(|err| eprintln!("Could not increment visits of {id}: {err}"));
    HttpResponse::TemporaryRedirect()
        .insert_header((http::header::LOCATION, shortened_url.full_url.clone()))
        .body(format!("Redirecting to {}...", shortened_url.full_url))
}

#[post("/register")]
pub async fn register(
    authentication_service: Data<AuthenticationService>,
    Query(RegisterDto {
        name,
        email,
        password,
        ..
    }): Query<RegisterDto>,
) -> impl Responder {
    if let Err(err) = authentication_service
        .register_user(name, &email, &*password)
        .await
    {
        if let Ok(err) = err.downcast::<UserRegistrationError>() {
            return err.into();
        } else {
            return HttpResponse::InternalServerError().body("An error occurred");
        }
    }

    HttpResponse::Ok().finish()
}

#[post("/login")]
pub async fn login(
    authentication_service: Data<AuthenticationService>,
    Query(LoginDto { email, password }): Query<LoginDto>,
    session: Session,
) -> impl Responder {
    let Some(User { email, .. }) = authentication_service
        .authenticate_credentials(email, password.as_bytes())
        .await
    else {
        return HttpResponse::NotFound().body("Could not find an user for those credentials");
    };

    if let Err(err) = session.insert("email", email) {
        return HttpResponse::InternalServerError().body(format!("Could not apply cookies: {err}"));
    }

    HttpResponse::Ok().finish()
}

#[get("/logout")]
pub async fn logout(session: Session) -> impl Responder {
    match session.get::<String>("email") {
        Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
        Ok(None) => HttpResponse::Unauthorized().body("You are not logged in"),
        Ok(Some(email)) => {
            session.purge();
            HttpResponse::Ok().body(format!("Logged out from {email}"))
        }
    }
}
