use std::{
    fmt::Display,
    time::{Duration, SystemTime},
};

use actix_web::HttpResponse;
use anyhow::{Context, Result, bail};
use rand::distr::{Alphanumeric, SampleString};
use sqlx::{Executor, Sqlite};

use crate::DatabasePool;

const ID_SIZE: u8 = 5;

#[derive(Debug, Clone)]
pub struct ShortenedUrl {
    pub id: String,
    pub full_url: String,
    pub visits: u32,
    pub expire_at: SystemTime,
}

impl Display for ShortenedUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.id, self.full_url)
    }
}

impl ShortenedUrl {
    pub fn new(url: impl Into<String>) -> Self {
        let mut rng = rand::rng();
        let id = Alphanumeric.sample_string(&mut rng, ID_SIZE as usize);
        Self {
            id,
            full_url: url.into(),
            visits: 0,
            expire_at: SystemTime::now() + Duration::from_hours(24),
        }
    }

    pub fn from_parts(
        id: impl Into<String>,
        full_url: impl Into<String>,
        visits: impl Into<u32>,
        expire_at: impl Into<SystemTime>,
    ) -> Self {
        Self {
            id: id.into(),
            full_url: full_url.into(),
            visits: visits.into(),
            expire_at: expire_at.into(),
        }
    }

    pub async fn save(&self, executor: impl Executor<'_, Database = Sqlite>) -> Result<()> {
        sqlx::query!(
            "INSERT INTO shortened_urls VALUES (?, ?, ?, ?)",
            self.id,
            self.full_url,
            self.visits,
            self.expire_at
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as u32
        )
        .execute(executor)
        .await
        .map(|_| ())
        .context("Could not save {self}")
    }

    pub async fn increment_visits_by_id(
        id: impl Into<&str>,
        executor: impl Executor<'_, Database = Sqlite>,
    ) -> Result<()> {
        let id = id.into();
        sqlx::query!(
            "UPDATE shortened_urls SET visits = visits + 1 WHERE id = ?",
            id,
        )
        .execute(executor)
        .await
        .map(|_| ())
        .context(format!("Could not increment visits of {id}"))
    }

    pub async fn find_by_url(
        url: impl Into<&str>,
        executor: impl Executor<'_, Database = Sqlite>,
    ) -> Result<Option<ShortenedUrl>> {
        let url = url.into();

        sqlx::query!(
                "SELECT * FROM shortened_urls WHERE full_url = ? AND expire_at > unixepoch(current_timestamp) LIMIT 1", 
                url
            )
            .fetch_optional(executor)
        .await
        .map(|record| {
            record.map(|record| {
                ShortenedUrl::from_parts(
                    record.id,
                    record.full_url,
                    record.visits as u32,
                    SystemTime::UNIX_EPOCH + Duration::from_secs(record.expire_at as u64),
                )
            })
        })
        .context(format!("Could not get shortened url for {url}"))
    }

    pub async fn find_by_url_with_expired(
        url: impl Into<&str>,
        executor: impl Executor<'_, Database = Sqlite>,
    ) -> Result<Option<ShortenedUrl>> {
        let url = url.into();

        sqlx::query!(
            "SELECT * FROM shortened_urls WHERE full_url = ? LIMIT 1",
            url
        )
        .fetch_optional(executor)
        .await
        .map(|record| {
            record.map(|record| {
                ShortenedUrl::from_parts(
                    record.id,
                    record.full_url,
                    record.visits as u32,
                    SystemTime::UNIX_EPOCH + Duration::from_secs(record.expire_at as u64),
                )
            })
        })
        .context(format!("Could not get shortened url for {url}"))
    }

    pub async fn find_by_id(
        id: impl Into<&str>,
        executor: impl Executor<'_, Database = Sqlite>,
    ) -> Result<Option<Self>> {
        let id = id.into();

        sqlx::query!(
            "SELECT * FROM shortened_urls WHERE id = ? AND expire_at > unixepoch(current_timestamp) LIMIT 1",
            id
        )
        .fetch_optional(executor)
        .await
        .map(|record| {
            record.map(|record| {
                ShortenedUrl::from_parts(
                    record.id,
                    record.full_url,
                    record.visits as u32,
                    SystemTime::UNIX_EPOCH + Duration::from_secs(record.expire_at as u64),
                )
            })
        })
        .context(format!("Could not get shortened url for {id}"))
    }

    pub async fn find_by_id_with_expired(
        id: impl Into<&str>,
        executor: impl Executor<'_, Database = Sqlite>,
    ) -> Result<Option<Self>> {
        let id = id.into();

        sqlx::query!("SELECT * FROM shortened_urls WHERE id = ? LIMIT 1", id)
            .fetch_optional(executor)
            .await
            .map(|record| {
                record.map(|record| {
                    ShortenedUrl::from_parts(
                        record.id,
                        record.full_url,
                        record.visits as u32,
                        SystemTime::UNIX_EPOCH + Duration::from_secs(record.expire_at as u64),
                    )
                })
            })
            .context(format!("Could not get shortened url for {id}"))
    }
}

#[derive(Debug)]
pub enum ShortenUrlError {
    BlacklistedUrl,
}

impl Display for ShortenUrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShortenUrlError::BlacklistedUrl => {
                write!(f, "This url is blacklisted and cannot be shortened")
            }
        }
    }
}

impl From<ShortenUrlError> for HttpResponse {
    fn from(val: ShortenUrlError) -> Self {
        match val {
            ShortenUrlError::BlacklistedUrl => HttpResponse::BadRequest(),
        }
        .body(val.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct UrlShortenerService {
    db_pool: DatabasePool,
    blacklisted_urls: Vec<String>,
}

impl UrlShortenerService {
    pub fn new(db_pool: DatabasePool, blacklisted_urls: Vec<String>) -> Self {
        Self {
            db_pool,
            blacklisted_urls,
        }
    }

    pub fn is_blacklisted(&self, url: &str) -> bool {
        self.blacklisted_urls
            .iter()
            .any(|blacklisted_url| url.starts_with(blacklisted_url))
    }

    pub async fn shorten_url(&self, url: &str) -> Result<String> {
        if self.is_blacklisted(url) {
            bail!(ShortenUrlError::BlacklistedUrl)
        }

        if let Some(existing_shortened_url) = ShortenedUrl::find_by_url(url, &self.db_pool).await? {
            return Ok(existing_shortened_url.id);
        }

        let new_shortened_url = ShortenedUrl::new(url);
        new_shortened_url.save(&self.db_pool).await?;
        Ok(new_shortened_url.id)
    }

    pub async fn find_shortened_url_by_id(&self, id: &str) -> Result<Option<ShortenedUrl>> {
        ShortenedUrl::find_by_id(id, &self.db_pool).await
    }

    pub async fn increment_shortened_url_visits_by_id(&self, id: &str) -> Result<()> {
        ShortenedUrl::increment_visits_by_id(id, &self.db_pool).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_generates_five_char_alphanumeric_id() {
        let url = "https://example.com";
        let shortened = ShortenedUrl::new(url);
        assert_eq!(shortened.id.len(), ID_SIZE as usize);
        assert!(shortened.id.chars().all(|c| c.is_ascii_alphanumeric()));
        assert_eq!(shortened.full_url, url);
        assert_eq!(shortened.visits, 0);
    }

    #[test]
    fn new_does_not_generate_a_constant_id() {
        // Guards against an accidentally constant/broken generator without
        // relying on any two specific samples differing (which has a tiny but
        // non-zero collision chance for 5 alphanumeric chars). A working random
        // generator will virtually never produce the same id ten times.
        let first = ShortenedUrl::new("https://example.com").id;
        let all_identical = (0..10)
            .map(|_| ShortenedUrl::new("https://example.com").id)
            .all(|id| id == first);
        assert!(!all_identical, "id generator appears to be constant");
    }

    #[test]
    fn from_parts_sets_all_fields() {
        let expire_at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let shortened = ShortenedUrl::from_parts("abc12", "https://example.com", 7u32, expire_at);
        assert_eq!(shortened.id, "abc12");
        assert_eq!(shortened.full_url, "https://example.com");
        assert_eq!(shortened.visits, 7);
        assert_eq!(shortened.expire_at, expire_at);
    }

    #[test]
    fn new_sets_expire_at_about_24_hours_in_the_future() {
        let before = SystemTime::now();
        let shortened = ShortenedUrl::new("https://example.com");
        let after = SystemTime::now();

        // The expiry is "now + 24h"; bracket it by the clock readings taken
        // immediately before and after construction so the assertion holds
        // regardless of how much time elapsed in between.
        assert!(shortened.expire_at >= before + Duration::from_hours(24));
        assert!(shortened.expire_at <= after + Duration::from_hours(24));
    }

    #[test]
    fn display_formats_as_id_colon_url() {
        let shortened =
            ShortenedUrl::from_parts("abc12", "https://example.com", 0u32, SystemTime::now());
        assert_eq!(shortened.to_string(), "abc12: https://example.com");
    }

    #[test]
    fn blacklisted_url_error_displays_human_readable_message() {
        assert_eq!(
            ShortenUrlError::BlacklistedUrl.to_string(),
            "This url is blacklisted and cannot be shortened"
        );
    }

    #[test]
    fn blacklisted_url_error_converts_to_400_response_with_message_body() {
        use actix_web::http::StatusCode;

        let response: HttpResponse = ShortenUrlError::BlacklistedUrl.into();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
