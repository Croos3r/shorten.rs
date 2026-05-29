mod controller;
mod services;

use std::sync::LazyLock;

use actix_web::{App, HttpServer, web::Data};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use validator::Validate;

use crate::{
    controller::{redirect_to_url_for_id, shorten_url},
    services::url_shortener::UrlShortenerService,
};

static RE_HTTP_SCHEME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^https?://.+").unwrap());

pub type DatabasePool = SqlitePool;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let pool = SqlitePool::connect("database.sqlite").await.unwrap();
    let url_shortener_service = UrlShortenerService::new(pool.clone());
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(url_shortener_service.clone()))
            .service(shorten_url)
            .service(redirect_to_url_for_id)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}

#[derive(Debug, Deserialize, Serialize, Clone, Validate)]
pub struct ShortenUrlDto {
    #[validate(url, regex(path = *RE_HTTP_SCHEME))]
    pub(crate) url: String,
}
