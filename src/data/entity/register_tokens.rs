//! A registration token; its UUID v7 ID is the token value.

use sea_orm::entity::prelude::*;

/// A registration token for self-registration on private instances
///
/// The ID of this model (a UUID v7) IS the token value, so clients just
/// present this UUID as their registration proof.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "register_tokens")]
pub struct Model {
    /// The token value (UUID v7)
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// Expiration timestamp of this token
    pub expires_at: DateTimeUtc,
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
