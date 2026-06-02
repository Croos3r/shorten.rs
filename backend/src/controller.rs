use actix_web::{
    HttpResponse, Responder, get, http, post,
    web::{Data, Path},
};
use actix_web_validator::Query;

use crate::{
    ShortenUrlDto,
    services::url_shortener::{ShortenUrlError, UrlShortenerService},
};

#[post("/shorten")]
pub async fn shorten_url(
    url_shortener_service: Data<UrlShortenerService>,
    query: Query<ShortenUrlDto>,
) -> impl Responder {
    let url = query.into_inner().url;
    match url_shortener_service.shorten_url(&url).await {
        Ok(id) => HttpResponse::Ok().body(id),
        Err(err) => err
            .downcast::<ShortenUrlError>()
            .map(ShortenUrlError::into)
            .unwrap_or_else(|_| {
                HttpResponse::InternalServerError().body("An unknown error has occured")
            }),
    }
}

#[get("/{id}")]
pub async fn redirect_to_url_for_id(
    id: Path<String>,
    url_shortener_service: Data<UrlShortenerService>,
) -> impl Responder {
    let id = id.into_inner();
    let shortened_url = match url_shortener_service.find_by_id(&id).await {
        Ok(Some(shortened_url)) => shortened_url,
        Ok(None) => return HttpResponse::NotFound().body("No url for this id"),
        Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
    };
    let _ = url_shortener_service
        .increment_visit_by_id(&id)
        .await
        .map_err(|err| eprintln!("Could not increment visits of {id}: {err}"));
    HttpResponse::TemporaryRedirect()
        .insert_header((http::header::LOCATION, shortened_url.full_url.clone()))
        .body(format!("Redirecting to {}...", shortened_url.full_url))
}
