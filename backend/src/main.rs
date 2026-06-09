use std::{env, str::FromStr};

use actix_cors::Cors;
use actix_session::{SessionMiddleware, storage::CookieSessionStore};
use actix_web::{App, HttpServer, cookie::Key, middleware::Logger, web::Data};
use env_logger::Env;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use shorten_rs::{
    configure,
    services::{
        authentication::AuthenticationService, url_shortener::UrlShortenerService,
        users::UsersService,
    },
};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Configurable via environment so the same binary runs locally and in a
    // container. Defaults preserve the original local behaviour.
    dotenv::dotenv().ok();
    env_logger::init_from_env(Env::default().default_filter_or("info"));
    let database_url =
        env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:database.sqlite".to_string());
    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8080);

    let session_secret_key_raw =
        env::var("SESSION_SECRET_KEY").expect("SESSION_SECRET_KEY must be set");
    let session_secret_key =
        Key::try_from(session_secret_key_raw.as_bytes()).unwrap_or_else(|err| {
            eprintln!("Could not use SESSION_SECRET_KEY: {err}");
            eprintln!("Generating random one...");
            Key::generate()
        });

    let options = SqliteConnectOptions::from_str(&database_url)
        .expect("DATABASE_URL is not a valid SQLite connection string")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("failed to connect to the database");

    let blacklisted_urls: Vec<String> = vec![
        env::var("API_DOMAIN").expect("API_DOMAIN must be set"),
        env::var("WEBSITE_DOMAIN").expect("WEBSITE_DOMAIN must be set"),
    ]
    .into_iter()
    .flat_map(|url| vec![format!("http://{url}"), format!("https://{url}")])
    .collect();

    // Own the schema lifecycle: apply any pending migrations on startup. sqlx
    // tracks applied migrations, so a fresh database is initialised and future
    // migrations are picked up without re-running existing ones.
    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("failed to run database migrations");

    let url_shortener_service = UrlShortenerService::new(pool.clone(), blacklisted_urls);
    let users_service = UsersService::new(pool.clone());
    let authentication_service = AuthenticationService::new(pool.clone());
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(Cors::default().allow_any_origin())
            .wrap(SessionMiddleware::new(
                CookieSessionStore::default(),
                session_secret_key.clone(),
            ))
            .app_data(Data::new(url_shortener_service.clone()))
            .app_data(Data::new(users_service.clone()))
            .app_data(Data::new(authentication_service.clone()))
            .configure(configure)
    })
    .bind((host, port))?
    .run()
    .await
}
