use std::time::SystemTime;

use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use rand_core::OsRng;

use anyhow::Result;

#[derive(Debug)]
pub struct User {
    pub name: String,
    pub email: String,
    pub hashed_password: String,
    pub registered_at: SystemTime,
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
}
