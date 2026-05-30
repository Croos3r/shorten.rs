use std::env;

use actix_web::{App, HttpServer, web::Data};
use sqlx::SqlitePool;

use shorten_rs::{configure, services::url_shortener::UrlShortenerService};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Configurable via environment so the same binary runs locally and in a
    // container. Defaults preserve the original local behaviour.
    let database_url =
        env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:database.sqlite".to_string());
    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8080);

    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let url_shortener_service = UrlShortenerService::new(pool.clone());
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(url_shortener_service.clone()))
            .configure(configure)
    })
    .bind((host, port))?
    .run()
    .await
}
