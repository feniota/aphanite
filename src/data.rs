//! [`DatabaseAccessor`](crate::data::DatabaseAccessor) and everything (except storage) related to database

use crate::types::{Token, User};
use anyhow::{anyhow, Result};
use argon2::PasswordVerifier;
use jiff::ToSpan;
use toasty::Db;
use tracing::error;
use uuid::Uuid;

const TOKEN_TTL_SECS: i64 = 24 * 3600;

#[derive(Clone)]
pub struct DatabaseAccessor {
    db: Db,
}

impl DatabaseAccessor {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn verify_user(&self, email: &str, password: &str) -> Result<User> {
        let mut db = self.db.clone();
        let user = User::get_by_email(&mut db, email).await?;
        let parsed_hash = argon2::PasswordHash::new(&user.password)
            .expect("Database error: Unable to parse the user password");
        argon2::Argon2::default().verify_password(password.as_bytes(), &parsed_hash)?;

        Ok(user)
    }
    pub async fn verify_token(
        &self,
        access_token: &Uuid,
        client_token: Option<&str>,
    ) -> Result<()> {
        let mut db = self.db.clone();
        let token = Token::get_by_access_token(&mut db, access_token).await?;

        if token.created_at + TOKEN_TTL_SECS.seconds() < jiff::Timestamp::now() {
            if let Err(e) = Token::delete_by_access_token(&mut db, access_token).await {
                error!("{e}")
            }
            return Err(anyhow!("The access token has expired."));
        }

        match client_token {
            None => Ok(()),
            Some(v) => {
                if v == token.client_token {
                    Ok(())
                } else {
                    Err(anyhow!("Client token does not match."))
                }
            }
        }
    }
}
