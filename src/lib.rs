pub mod controller;
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
