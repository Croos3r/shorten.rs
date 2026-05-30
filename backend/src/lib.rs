// Private: handlers are an implementation detail wired up through `configure`,
// not part of the supported public API.
mod controller;
pub mod services;

use std::sync::LazyLock;

use actix_web::web::ServiceConfig;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use validator::Validate;

use crate::controller::{redirect_to_url_for_id, shorten_url};

static RE_HTTP_SCHEME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^https?://.+").unwrap());

pub type DatabasePool = SqlitePool;

/// Registers the application's HTTP routes on an Actix [`ServiceConfig`].
///
/// Shared between the binary's `HttpServer` and the integration tests so both
/// exercise the exact same routing setup.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(shorten_url).service(redirect_to_url_for_id);
}

#[derive(Debug, Deserialize, Serialize, Clone, Validate)]
pub struct ShortenUrlDto {
    #[validate(url, regex(path = *RE_HTTP_SCHEME))]
    pub(crate) url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dto(url: &str) -> ShortenUrlDto {
        ShortenUrlDto {
            url: url.to_string(),
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
