//! SeaORM model matching the original toasty model.

use sea_orm::entity::prelude::*;

/// A Minecraft player profile
/// This is NOT intended to be used in data exchange with Yggdrasil clients for some database-specific fields.
/// Please use [`ExchangeableGameProfile`](crate::service::yggdrasil::types::ExchangeableGameProfile) instead.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "game_profiles")]
pub struct Model {
    /// UUID of this player profile
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// Player name of this profile
    #[sea_orm(column_type = "Text")]
    #[sea_orm(indexed)]
    pub name: String,
    /// Internal field for database relationship
    #[sea_orm(indexed)]
    pub owner_id: Uuid,
    #[sea_orm(has_one)]
    pub textures: HasOne<super::profile_textures::Entity>,
    /// Associated user account of this profile
    #[sea_orm(belongs_to, from = "owner_id", to = "id")]
    pub owner: BelongsTo<super::users::Entity>,
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
