use std::{
    fmt::Display,
    time::{Duration, SystemTime},
};

use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use rand_core::OsRng;

use anyhow::{Context, Result};
use sqlx::{Executor, Sqlite};

use crate::DatabasePool;

const USER_PAGE_SIZE: u32 = 50;

#[derive(Debug, Clone)]
pub struct User {
    pub name: String,
    pub email: String,
    pub hashed_password: String,
    pub registered_at: SystemTime,
}

impl Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl User {
    pub fn new<'a>(
        name: impl Into<String>,
        email: impl Into<String>,
        password: impl Into<&'a str>,
    ) -> Result<Self> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        Ok(Self {
            name: name.into(),
            email: email.into(),
            hashed_password: argon2
                .hash_password(password.into().as_bytes(), &salt)
                .expect("Password must be hashed")
                .to_string(),
            registered_at: SystemTime::now(),
        })
    }

    pub fn from_parts(
        name: impl Into<String>,
        email: impl Into<String>,
        hashed_password: impl Into<String>,
        registered_at: impl Into<SystemTime>,
    ) -> Self {
        Self {
            name: name.into(),
            email: email.into(),
            hashed_password: hashed_password.into(),
            registered_at: registered_at.into(),
        }
    }

    pub async fn save(&self, executor: impl Executor<'_, Database = Sqlite>) -> Result<()> {
        sqlx::query!(
            "INSERT INTO users VALUES (?, ?, ?, ?)",
            self.name,
            self.email,
            self.hashed_password,
            self.registered_at
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_secs() as u32
        )
        .execute(executor)
        .await
        .map(|_| ())
        .context(format!("Could not save {self}"))
    }

    pub async fn find_by_email(
        email: impl Into<String>,
        executor: impl Executor<'_, Database = Sqlite>,
    ) -> Result<Option<User>> {
        let email = email.into();
        sqlx::query!("SELECT * FROM users WHERE email = ? LIMIT 1", email)
            .fetch_optional(executor)
            .await
            .map(|record| {
                record.map(|record| {
                    Self::from_parts(
                        record.name,
                        record.email,
                        record.hashed_password,
                        SystemTime::UNIX_EPOCH + Duration::from_secs(record.registered_at as u64),
                    )
                })
            })
            .context(format!("Could not find user for {email}"))
    }

    pub async fn find_by_name(
        name: impl Into<String>,
        executor: impl Executor<'_, Database = Sqlite>,
    ) -> Result<Option<User>> {
        let name = name.into();
        sqlx::query!("SELECT * FROM users WHERE name = ? LIMIT 1", name)
            .fetch_optional(executor)
            .await
            .map(|record| {
                record.map(|record| {
                    Self::from_parts(
                        record.name,
                        record.email,
                        record.hashed_password,
                        SystemTime::UNIX_EPOCH + Duration::from_secs(record.registered_at as u64),
                    )
                })
            })
            .context(format!("Could not find user for {name}"))
    }

    pub async fn exists(
        email: impl Into<String>,
        executor: impl Executor<'_, Database = Sqlite>,
    ) -> Result<bool> {
        let email = email.into();
        sqlx::query_scalar!("SELECT 1 FROM users WHERE email = ? LIMIT 1", email)
            .fetch_optional(executor)
            .await
            .map(|res| res.is_some())
            .context(format!("Could not check presence of {email}"))
    }

    pub async fn get_users_page(
        page: u32,
        page_size: u32,
        executor: impl Executor<'_, Database = Sqlite>,
    ) -> Result<Vec<Self>> {
        let (start, end) = (page_size * page, page_size * (page + 1));
        sqlx::query!("SELECT * FROM users LIMIT ?,?", start, end)
            .fetch_all(executor)
            .await
            .map(|res| {
                res.into_iter()
                    .map(|record| {
                        User::from_parts(
                            record.name,
                            record.email,
                            record.hashed_password,
                            SystemTime::UNIX_EPOCH
                                + Duration::from_secs(record.registered_at as u64),
                        )
                    })
                    .collect()
            })
            .context(format!("Could not get users at page {page}"))
    }
}

#[derive(Debug, Clone)]
pub struct UsersService {
    db_pool: DatabasePool,
}

impl UsersService {
    pub fn new(db_pool: DatabasePool) -> Self {
        Self { db_pool }
    }

    pub async fn get_users_page(&self, page: u32) -> Result<Vec<User>> {
        User::get_users_page(page, USER_PAGE_SIZE, &self.db_pool).await
    }
}
