//! [`DatabaseAccessor`] and everything (except storage) related to database

use crate::service::yggdrasil::types::GameProfile;
use crate::types::{Token, User};
use anyhow::{Result, anyhow};
use argon2::PasswordVerifier;
use argon2::password_hash::{PasswordHasher, SaltString, rand_core::OsRng};
use jiff::ToSpan;
use toasty::Db;
use tracing::error;
use uuid::Uuid;

const TOKEN_TTL_SECS: i64 = 24 * 3600;
const MAX_TOKENS_PER_USER: usize = 10;

#[derive(Clone)]
pub struct DatabaseAccessor {
    db: Db,
}

impl DatabaseAccessor {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Get a reference to the underlying database handle
    pub fn db(&self) -> &Db {
        &self.db
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
        client_token: &Option<String>,
    ) -> Result<User> {
        let mut db = self.db.clone();
        let token = Token::get_by_access_token(&mut db, access_token).await?;

        if token.created_at + TOKEN_TTL_SECS.seconds() < jiff::Timestamp::now() {
            if let Err(e) = Token::delete_by_access_token(&mut db, access_token).await {
                error!("{e}")
            }
            return Err(anyhow!("The access token has expired."));
        }

        let user = token.user().exec(&mut db).await?;

        match client_token {
            None => Ok(user),
            Some(v) => {
                if *v == *token.client_token {
                    Ok(user)
                } else {
                    Err(anyhow!("Client token does not match."))
                }
            }
        }
    }
    pub async fn match_profile(&self, access_token: &Uuid, profile_id: &Uuid) -> Result<()> {
        let mut db = self.db.clone();
        let token = Token::get_by_access_token(&mut db, access_token).await?;

        if token.created_at + TOKEN_TTL_SECS.seconds() < jiff::Timestamp::now() {
            if let Err(e) = Token::delete_by_access_token(&mut db, access_token).await {
                error!("{e}")
            }
            return Err(anyhow!("The access token has expired."));
        }
        if token.profile_id.is_none() {
            return Err(anyhow!("The token does not match to the profile."));
        }
        if let Some(profile) = token.profile().exec(&mut db).await?
            && profile.id == *profile_id
        {
            Ok(())
        } else {
            Err(anyhow!("The token does not match to the profile."))
        }
    }
    pub async fn delete_token(&self, access_token: &Uuid) -> Result<()> {
        let mut db = self.db.clone();
        Token::delete_by_access_token(&mut db, access_token).await?;
        Ok(())
    }
    pub async fn clear_token(&self, user_id: &Uuid) -> Result<()> {
        let mut db = self.db.clone();
        Token::delete_by_user_id(&mut db, user_id).await?;
        Ok(())
    }
    pub async fn create_token(
        &self,
        user_id: &Uuid,
        client_token: &str,
        selected_profile_id: Option<&Uuid>,
    ) -> Result<Uuid> {
        let mut db = self.db.clone();

        if Token::filter_by_user_id(user_id)
            .count()
            .exec(&mut db)
            .await?
            >= MAX_TOKENS_PER_USER as u64
        {
            let oldest = Token::filter_by_user_id(user_id)
                .order_by(Token::fields().created_at().asc())
                .limit(1)
                .one()
                .exec(&mut db)
                .await?;
            Token::delete_by_access_token(&mut db, oldest.access_token).await?;
        }

        let token_create = Token::create()
            .client_token(client_token)
            .user_id(user_id)
            .profile_id(selected_profile_id.copied());

        Ok(token_create.exec(&mut db).await?.access_token)
    }
    pub async fn query_profile(&self, profile_id: &Uuid) -> Result<GameProfile> {
        let mut db = self.db.clone();
        Ok(GameProfile::get_by_id(&mut db, profile_id).await?)
    }
    pub async fn query_profile_by_name(&self, name: &str) -> Result<Vec<GameProfile>> {
        let mut db = self.db.clone();
        Ok(GameProfile::filter_by_name(name).exec(&mut db).await?)
    }
    pub async fn query_profile_by_user(&self, user_id: &Uuid) -> Result<Vec<GameProfile>> {
        let mut db = self.db.clone();
        Ok(GameProfile::filter_by_owner_id(user_id)
            .exec(&mut db)
            .await?)
    }

    /// Update the password of a user.
    ///
    /// This hashes the password with Argon2 and stores the PHC string.
    pub async fn update_user_password(&self, user_id: &Uuid, new_password: &str) -> Result<()> {
        let mut db = self.db.clone();

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = argon2::Argon2::default();
        let hashed_password = argon2
            .hash_password(new_password.as_bytes(), &salt)
            .map_err(|e| anyhow!("Password hashing failed: {}", e))?
            .to_string();

        let mut user = User::get_by_id(&mut db, user_id).await?;
        user.update()
            .password(&hashed_password)
            .exec(&mut db)
            .await?;
        Ok(())
    }

    pub async fn query_totp(&self, email: &str) -> Option<String> {
        let mut db = self.db.clone();
        match User::get_by_email(&mut db, email).await {
            Ok(v) => {
                if v.totp_active {
                    v.totp_secret
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }
}
