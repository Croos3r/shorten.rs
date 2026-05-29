use actix_web::{App, HttpServer, web::Data};
use sqlx::SqlitePool;

use shorten_rs::{configure, services::url_shortener::UrlShortenerService};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let pool = SqlitePool::connect("database.sqlite").await.unwrap();
    let url_shortener_service = UrlShortenerService::new(pool.clone());
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(url_shortener_service.clone()))
            .configure(configure)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
