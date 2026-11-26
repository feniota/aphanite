//! SeaORM model matching the original toasty model.

use sea_orm::entity::prelude::*;

/// Texture of a player profile
/// This type is designed especially for database storage and guaranteed NOT to be compatible with the API payload type.
/// For API usage use [`TexturesPayload`](crate::service::yggdrasil::types::TexturesPayload) instead.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "profile_textures")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub created_at: DateTimeUtc,
    #[sea_orm(indexed)]
    pub profile_id: Uuid,
    pub skin_model: SkinModel,
    pub skin_file: Option<Uuid>,
    pub cape_file: Option<Uuid>,
    #[sea_orm(belongs_to, from = "profile_id", to = "id")]
    pub profile: BelongsTo<super::game_profiles::Entity>,
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
        if insert && self.created_at.is_not_set() {
            self.created_at = sea_orm::Set(ChronoUtc::now());
        }
        Ok(self)
    }
}

/// Numeric database representation and API names match the original SkinModel.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    serde::Serialize,
    serde::Deserialize,
)]
#[sea_orm(rs_type = "i64", db_type = "BigInteger")]
#[serde(rename_all = "lowercase")]
pub enum SkinModel {
    #[sea_orm(num_value = 0)]
    Default,
    #[sea_orm(num_value = 1)]
    Slim,
}
