//! SeaORM model matching the original toasty model.

use crate::data::Unsigned32;
use sea_orm::entity::prelude::*;

/// A player account
///
/// This is used across Yggdrasil and Phenocryst.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    /// The UUID of the User
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// The nickname of the User
    #[sea_orm(column_type = "Text")]
    pub nickname: String,
    /// The email of the User
    #[sea_orm(column_type = "Text", unique)]
    pub email: String,
    /// PHC representation of Argon2 password hash & salt
    ///
    /// See [`PasswordHash`](argon2::PasswordHash) for details.
    #[sea_orm(column_type = "Text")]
    pub password: String,
    /// Preferred language of this user
    #[sea_orm(column_type = "Text")]
    pub preferred_language: String,
    /// Permissions of this user
    pub permission: Unsigned32,
    /// The TOTP secret of this user;
    ///
    /// The TOTP system can reduce the use of password, and is applied as 1FA here.
    /// This should be a very long and random string at least 128 bits (16 bytes).
    #[sea_orm(column_type = "Text", nullable)]
    pub totp_secret: Option<String>,
    /// Instances that this user is allowed to play (Phenocryst only)
    #[sea_orm(has_many, via = "user_instances")]
    pub instances: HasMany<super::instances::Entity>,
    /// Internal relationship constraint; should NOT be used
    #[sea_orm(has_many)]
    pub instances_relation: HasMany<super::user_instances::Entity>,
    #[sea_orm(has_many)]
    pub profiles: HasMany<super::game_profiles::Entity>,
    #[sea_orm(has_many)]
    pub tokens: HasMany<super::tokens::Entity>,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if insert && self.id.is_not_set() {
            self.id = sea_orm::Set(Uuid::now_v7());
        }

        Ok(self)
    }
}
