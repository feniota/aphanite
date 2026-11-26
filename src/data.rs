//! [`DatabaseAccessor`] and everything (except storage) related to database

mod value_types;
pub use value_types::{Unsigned16, Unsigned32};
pub mod entity;
mod migrations;
pub use migrations::run_migrations;

use anyhow::{Result, anyhow};
use argon2::PasswordVerifier;
use argon2::password_hash::PasswordHasher;
use entity::{game_profiles, register_tokens, tokens, users};
use entity::{game_profiles::Model as GameProfile, tokens::Model as Token, users::Model as User};
use sea_orm::prelude::ChronoUtc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    IntoActiveModel, ModelTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
    TransactionTrait,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};
use uuid::Uuid;

const CLEAN_INTERVAL: Duration = Duration::from_hours(24);
const TOKEN_TTL_SECS: u64 = 24 * 3600;
const MAX_TOKENS_PER_USER: usize = 10;

#[derive(Clone)]
pub struct DatabaseAccessor {
    db: DatabaseConnection,
    _cleanup: Arc<async_channel::Sender<()>>,
}

impl DatabaseAccessor {
    pub fn new(db: DatabaseConnection) -> Self {
        let (cleanup, exit) = async_channel::bounded(1);
        ntex::rt::spawn(cleanup_task(db.clone(), exit));
        let _cleanup = Arc::new(cleanup);
        Self { db, _cleanup }
    }

    /// Get a reference to the underlying database handle
    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn verify_user(&self, email: &str, password: &str) -> Result<users::Model> {
        let user = users::Entity::find()
            .filter(users::Column::Email.eq(email))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("User not found."))?;
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
        let token = tokens::Entity::find_by_id(*access_token)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Token not found."))?;
        if token.created_at + Duration::from_secs(TOKEN_TTL_SECS) < ChronoUtc::now() {
            if let Err(e) = tokens::Entity::delete_by_id(*access_token)
                .exec(&self.db)
                .await
            {
                error!("{e}");
            }
            return Err(anyhow!("The access token has expired."));
        }
        if let Some(v) = client_token
            && *v != token.client_token
        {
            return Err(anyhow!("Client token does not match."));
        }
        let user = token
            .find_related(users::Entity)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("User not found."))?;
        Ok((user, token))
    }
    pub async fn match_profile(&self, access_token: &Uuid, profile_id: &Uuid) -> Result<()> {
        let token = tokens::Entity::find_by_id(*access_token)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Token not found."))?;
        if token.created_at + Duration::from_secs(TOKEN_TTL_SECS) < ChronoUtc::now() {
            if let Err(e) = tokens::Entity::delete_by_id(*access_token)
                .exec(&self.db)
                .await
            {
                error!("{e}");
            }
            return Err(anyhow!("The access token has expired."));
        }
        if token.profile_id.is_none() {
            let profile = game_profiles::Entity::find_by_id(*profile_id)
                .one(&self.db)
                .await?
                .ok_or_else(|| anyhow!("The specified profile is not found."))?;
            if profile.owner_id != token.user_id {
                return Err(anyhow!("The token does not match to the profile."));
            }
            return Ok(());
        }
        if let Some(profile) = token
            .find_related(game_profiles::Entity)
            .one(&self.db)
            .await?
            && profile.id == *profile_id
        {
            Ok(())
        } else {
            Err(anyhow!("The token does not match to the profile."))
        }
    }
    pub async fn delete_token(&self, access_token: &Uuid) -> Result<()> {
        tokens::Entity::delete_by_id(*access_token)
            .exec(&self.db)
            .await?;
        Ok(())
    }
    pub async fn clear_token(&self, user_id: &Uuid) -> Result<()> {
        tokens::Entity::delete_many()
            .filter(tokens::Column::UserId.eq(*user_id))
            .exec(&self.db)
            .await?;
        Ok(())
    }
    pub async fn create_token(
        &self,
        user_id: &Uuid,
        client_token: &str,
        selected_profile_id: Option<&Uuid>,
    ) -> Result<Uuid> {
        let tx = self.db.begin().await?;
        let token =
            Self::create_token_executor(&tx, user_id, client_token, selected_profile_id).await?;
        tx.commit().await?;
        Ok(token)
    }

    /// Same as [`create_token`] but runs on an existing executor (e.g. inside an outer transaction).
    /// The caller is responsible for committing the surrounding transaction.
    pub async fn create_token_executor<C: ConnectionTrait>(
        executor: &C,
        user_id: &Uuid,
        client_token: &str,
        selected_profile_id: Option<&Uuid>,
    ) -> Result<Uuid> {
        if tokens::Entity::find()
            .filter(tokens::Column::UserId.eq(*user_id))
            .count(executor)
            .await?
            >= MAX_TOKENS_PER_USER as u64
        {
            let oldest = tokens::Entity::find()
                .filter(tokens::Column::UserId.eq(*user_id))
                .order_by_asc(tokens::Column::CreatedAt)
                .one(executor)
                .await?
                .ok_or_else(|| anyhow!("Token not found."))?;
            tokens::Entity::delete_by_id(oldest.access_token)
                .exec(executor)
                .await?;
        }
        let token = tokens::ActiveModel {
            client_token: Set(client_token.to_owned()),
            user_id: Set(*user_id),
            profile_id: Set(selected_profile_id.copied()),
            ..Default::default()
        }
        .insert(executor)
        .await?;
        Ok(token.access_token)
    }
    pub async fn query_profile(&self, profile_id: &Uuid) -> Result<GameProfile> {
        game_profiles::Entity::find_by_id(*profile_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Profile not found."))
    }
    pub async fn query_profile_by_name(&self, name: &str) -> Result<Vec<GameProfile>> {
        Ok(game_profiles::Entity::find()
            .filter(game_profiles::Column::Name.eq(name))
            .all(&self.db)
            .await?)
    }
    pub async fn query_profile_by_user(&self, user_id: &Uuid) -> Result<Vec<GameProfile>> {
        Ok(game_profiles::Entity::find()
            .filter(game_profiles::Column::OwnerId.eq(*user_id))
            .all(&self.db)
            .await?)
    }

    /// Update the password of a user.
    ///
    /// This hashes the password with Argon2 and stores the PHC string.
    pub async fn update_user_password(&self, user_id: &Uuid, new_password: &str) -> Result<()> {
        let argon2 = argon2::Argon2::default();
        let hashed_password = argon2
            .hash_password(new_password.as_bytes())
            .map_err(|e| anyhow!("Password hashing failed: {}", e))?
            .to_string();

        let user = users::Entity::find_by_id(*user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("User not found."))?;
        let mut user = user.into_active_model();
        user.password = Set(hashed_password);
        user.update(&self.db).await?;
        Ok(())
    }

    pub async fn query_totp(&self, email: &str) -> Option<String> {
        users::Entity::find()
            .filter(users::Column::Email.eq(email))
            .one(&self.db)
            .await
            .ok()
            .flatten()
            .and_then(|user| user.totp_secret)
    }
}

async fn cleanup_expired_register_tokens(db: &DatabaseConnection) {
    let now = ChronoUtc::now();
    loop {
        let oldest = match register_tokens::Entity::find()
            .order_by_asc(register_tokens::Column::ExpiresAt)
            .limit(100)
            .all(db)
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
            if let Err(e) = register_tokens::Entity::delete_by_id(token.id)
                .exec(db)
                .await
            {
                error!("Failed to delete expired register token {}: {e}", token.id);
            }
        }
        info!("Cleaned up {count} expired registration token(s)");
    }
}

async fn cleanup_task(db: DatabaseConnection, exit: async_channel::Receiver<()>) {
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
