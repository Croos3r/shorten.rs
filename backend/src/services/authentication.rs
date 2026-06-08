use std::fmt::Display;

use actix_web::HttpResponse;
use anyhow::{Result, bail};
use argon2::{Argon2, PasswordHash, PasswordVerifier};

use crate::{DatabasePool, services::users::User};

#[derive(Debug, Clone)]
pub struct AuthenticationService {
    db_pool: DatabasePool,
}

#[derive(Debug)]
pub enum UserRegistrationError {
    EmailAlreadyTaken,
}

impl Display for UserRegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRegistrationError::EmailAlreadyTaken => write!(f, "This email is already taken"),
        }
    }
}

impl From<UserRegistrationError> for HttpResponse {
    fn from(value: UserRegistrationError) -> Self {
        match value {
            UserRegistrationError::EmailAlreadyTaken => HttpResponse::Conflict(),
        }
        .body(value.to_string())
    }
}

impl AuthenticationService {
    pub fn new(db_pool: DatabasePool) -> Self {
        Self { db_pool }
    }

    pub async fn authenticate_credentials(
        &self,
        email: impl Into<String>,
        password: impl Into<&[u8]>,
    ) -> Option<User> {
        let Ok(user) = User::find_by_email(email, &self.db_pool).await else {
            return None;
        };

        let argon2 = Argon2::default();
        user.and_then(|user| {
            argon2
                .verify_password(
                    password.into(),
                    &PasswordHash::new(&user.hashed_password).ok()?,
                )
                .is_ok()
                .then_some(user)
        })
    }

    pub async fn register_user(
        &self,
        name: impl Into<String>,
        email: &str,
        password: impl Into<&str>,
    ) -> Result<()> {
        if User::exists(email, &self.db_pool).await? {
            bail!(UserRegistrationError::EmailAlreadyTaken);
        }

        let user = User::new(name, email, password)?;

        user.save(&self.db_pool).await?;

        Ok(())
    }
}
