// Private: handlers are an implementation detail wired up through `configure`,
// not part of the supported public API.
mod controller;
pub mod dtos;
pub mod extractors;
pub mod services;

use actix_web::web::ServiceConfig;
use sqlx::SqlitePool;

use crate::controller::{login, logout, redirect_to_url_for_id, register, shorten_url};

pub type DatabasePool = SqlitePool;

/// Registers the application's HTTP routes on an Actix [`ServiceConfig`].
///
/// Shared between the binary's `HttpServer` and the integration tests so both
/// exercise the exact same routing setup.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(shorten_url)
        .service(logout)
        .service(redirect_to_url_for_id)
        .service(register)
        .service(login);
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtos::ShortenUrlDto;
    use validator::Validate;

    fn dto(url: &str) -> ShortenUrlDto {
        ShortenUrlDto {
            url: url.to_string(),
            expire_in: None,
        }
    }

    #[test]
    fn accepts_http_and_https_urls() {
        assert!(dto("http://example.com").validate().is_ok());
        assert!(dto("https://example.com/path?q=1").validate().is_ok());
    }

    #[test]
    fn rejects_urls_without_http_scheme() {
        // Valid URL but wrong scheme (regex requires http/https).
        assert!(dto("ftp://example.com").validate().is_err());
        // Missing scheme entirely.
        assert!(dto("example.com").validate().is_err());
    }

    #[test]
    fn rejects_non_url_input() {
        assert!(dto("not a url").validate().is_err());
        assert!(dto("").validate().is_err());
    }
}
