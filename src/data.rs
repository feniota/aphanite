//! [`DatabaseAccessor`] and everything (except storage) related to database

use crate::service::yggdrasil::types::GameProfile;
use crate::types::{RegisterToken, Token, User};
use anyhow::{Result, anyhow};
use argon2::PasswordVerifier;
use argon2::password_hash::PasswordHasher;
use async_compat::CompatExt;
use jiff::ToSpan;
use std::sync::Arc;
use std::time::Duration;
use toasty::{Db, Executor};
use tracing::{error, info};
use uuid::Uuid;

mod migrations;
pub use migrations::init;

const CLEAN_INTERVAL: Duration = Duration::from_hours(24);
const TOKEN_TTL_SECS: i64 = 24 * 3600;
const MAX_TOKENS_PER_USER: usize = 10;

#[derive(Clone)]
pub struct DatabaseAccessor {
    db: Db,
    _cleanup: Arc<async_channel::Sender<()>>,
}

impl DatabaseAccessor {
    pub fn new(db: Db) -> Self {
        let (cleanup, exit) = async_channel::bounded(1);
        ntex::rt::spawn(cleanup_task(db.clone(), exit).compat());
        let _cleanup = Arc::new(cleanup);
        Self { db, _cleanup }
    }

    /// Get a reference to the underlying database handle
    pub fn db(&self) -> &Db {
        &self.db
    }

    pub async fn verify_user(&self, email: &str, password: &str) -> Result<User> {
        let mut db = self.db.clone();
        let user = User::get_by_email(&mut db, email).await?;
        let password = password.to_owned();
        let password_hash = user.password.clone();
        // Argon2 is CPU-bound; hashing on an async worker serializes concurrent
        // logins and delays later request handling (e.g. the rate-limit probe).
        ntex::rt::spawn_blocking(move || {
            let parsed_hash = argon2::PasswordHash::new(&password_hash)
                .expect("Database error: Unable to parse the user password");
            argon2::Argon2::default().verify_password(password.as_bytes(), &parsed_hash)
        })
        .await
        .map_err(|e| anyhow!("Password verification task failed: {e}"))??;

        Ok(user)
    }
    pub async fn verify_token(
        &self,
        access_token: &Uuid,
        client_token: &Option<String>,
    ) -> Result<User> {
        Ok(self
            .verify_token_and_fetch(access_token, client_token)
            .await?
            .0)
    }

    /// Same as [`verify_token`], but also returns the stored token so callers
    /// can inspect its client token and profile binding.
    pub async fn verify_token_and_fetch(
        &self,
        access_token: &Uuid,
        client_token: &Option<String>,
    ) -> Result<(User, Token)> {
        let mut db = self.db.clone();
        let token = Token::get_by_access_token(&mut db, access_token).await?;

        if token.created_at + TOKEN_TTL_SECS.seconds() < jiff::Timestamp::now() {
            if let Err(e) = Token::delete_by_access_token(&mut db, access_token).await {
                error!("{e}")
            }
            return Err(anyhow!("The access token has expired."));
        }

        if let Some(v) = client_token
            && *v != token.client_token
        {
            return Err(anyhow!("Client token does not match."));
        }

        let user = token.user().exec(&mut db).await?;
        Ok((user, token))
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
            let profile = GameProfile::get_by_id(&mut db, profile_id).await;
            if let Err(e) = profile {
                anyhow::bail!("The specified profile is not found: {}", e);
            }
            if profile?.owner_id != token.user_id {
                return Err(anyhow!("The token does not match to the profile."));
            }
            return Ok(());
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
        let mut tx = db.transaction().await?;
        let token =
            Self::create_token_executor(&mut tx, user_id, client_token, selected_profile_id)
                .await?;
        tx.commit().await?;
        Ok(token)
    }

    /// Same as [`create_token`] but runs on an existing executor (e.g. inside an outer transaction).
    /// The caller is responsible for committing the surrounding transaction.
    pub async fn create_token_executor(
        executor: &mut dyn Executor,
        user_id: &Uuid,
        client_token: &str,
        selected_profile_id: Option<&Uuid>,
    ) -> Result<Uuid> {
        if Token::filter_by_user_id(user_id)
            .count()
            .exec(executor)
            .await?
            >= MAX_TOKENS_PER_USER as u64
        {
            let oldest = Token::filter_by_user_id(user_id)
                .order_by(Token::fields().created_at().asc())
                .limit(1)
                .one()
                .exec(executor)
                .await?;
            Token::delete_by_access_token(executor, oldest.access_token).await?;
        }

        let token_create = Token::create()
            .client_token(client_token)
            .user_id(user_id)
            .profile_id(selected_profile_id.copied());

        Ok(token_create.exec(executor).await?.access_token)
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

        let argon2 = argon2::Argon2::default();
        let hashed_password = argon2
            .hash_password(new_password.as_bytes())
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
            Ok(v) => v.totp_secret,
            Err(_) => None,
        }
    }
}

async fn cleanup_expired_register_tokens(db: &Db) {
    let mut db = db.clone();
    let now = jiff::Timestamp::now();
    loop {
        let oldest = match RegisterToken::all()
            .order_by(RegisterToken::fields().expires_at().asc())
            .limit(100)
            .exec(&mut db)
            .await
        {
            Ok(tokens) => tokens,
            Err(e) => {
                error!("Failed to query register tokens for cleanup: {e}");
                break;
            }
        };
        if oldest.is_empty() {
            break;
        }
        let expired: Vec<_> = oldest.into_iter().filter(|t| t.expires_at <= now).collect();
        if expired.is_empty() {
            break;
        }
        let count = expired.len();
        for token in expired {
            if let Err(e) = RegisterToken::delete_by_id(&mut db, &token.id).await {
                error!("Failed to delete expired register token {}: {e}", token.id);
            }
        }
        info!("Cleaned up {count} expired registration token(s)");
    }
}

async fn cleanup_task(db: Db, exit: async_channel::Receiver<()>) {
    while !exit.is_closed() {
        futures_lite::future::race(
            async {
                loop {
                    cleanup_expired_register_tokens(&db).await;
                    ntex::time::sleep(CLEAN_INTERVAL).await;
                }
            },
            async {
                let _ = exit.recv().await;
            },
        )
        .await;
    }
}
