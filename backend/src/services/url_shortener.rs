use std::fmt::Display;

use actix_web::HttpResponse;
use anyhow::{Context, Result, bail};
use rand::distr::{Alphanumeric, SampleString};

use crate::DatabasePool;

const ID_SIZE: u8 = 5;

#[derive(Debug, Clone)]
pub struct ShortenedUrl {
    pub id: String,
    pub full_url: String,
    pub visits: u32,
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
        }
    }

    pub fn from_parts(
        id: impl Into<String>,
        url: impl Into<String>,
        visits: impl Into<u32>,
    ) -> Self {
        Self {
            id: id.into(),
            full_url: url.into(),
            visits: visits.into(),
        }
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

        if let Some(existing_shortened_url) = sqlx::query!(
            "SELECT id FROM shortened_urls WHERE full_url = ? LIMIT 1",
            url
        )
        .fetch_optional(&self.db_pool)
        .await?
        {
            return Ok(existing_shortened_url.id);
        }

        let new_shortened_url = ShortenedUrl::new(url);
        sqlx::query!(
            "INSERT INTO shortened_urls VALUES (?, ?, ?)",
            new_shortened_url.id,
            new_shortened_url.full_url,
            new_shortened_url.visits
        )
        .execute(&self.db_pool)
        .await?;
        Ok(new_shortened_url.id)
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Option<ShortenedUrl>> {
        sqlx::query!("SELECT * FROM shortened_urls WHERE id = ? LIMIT 1", id)
            .fetch_optional(&self.db_pool)
            .await
            .map(|record| {
                record.map(|record| {
                    ShortenedUrl::from_parts(record.id, record.full_url, record.visits as u32)
                })
            })
            .context(format!("Could not get shortened url for {id}"))
    }

    pub async fn increment_visit_by_id(&self, id: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE shortened_urls SET visits = visits + 1 WHERE id = ?",
            id
        )
        .execute(&self.db_pool)
        .await
        .map(|_| ())
        .context(format!("Could not increment visits of {id}"))
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
        let shortened = ShortenedUrl::from_parts("abc12", "https://example.com", 7u32);
        assert_eq!(shortened.id, "abc12");
        assert_eq!(shortened.full_url, "https://example.com");
        assert_eq!(shortened.visits, 7);
    }

    #[test]
    fn display_formats_as_id_colon_url() {
        let shortened = ShortenedUrl::from_parts("abc12", "https://example.com", 0u32);
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
