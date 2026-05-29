use std::fmt::Display;

use anyhow::{Context, Result};
use rand::distr::{Alphanumeric, SampleString};

use crate::DatabasePool;

const ID_SIZE: u8 = 5;

#[derive(Debug, Clone)]
pub struct ShortenedUrl {
    pub id: String,
    pub full_url: String,
    pub visits: usize,
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
        visits: impl Into<usize>,
    ) -> Self {
        Self {
            id: id.into(),
            full_url: url.into(),
            visits: visits.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UrlShortenerService {
    db_pool: DatabasePool,
}

impl UrlShortenerService {
    pub fn new(db_pool: DatabasePool) -> Self {
        Self { db_pool }
    }

    pub async fn shorten_url(&self, url: &str) -> Result<String> {
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
            "INSERT INTO shortened_urls VALUES (?, ?)",
            new_shortened_url.id,
            new_shortened_url.full_url
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
                record.map(|record| ShortenedUrl::from_parts(record.id, record.full_url, 0usize))
            })
            .context(format!("Could not get shortened url for {id}"))
    }
}
