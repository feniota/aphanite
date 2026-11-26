//! SeaORM model matching the original toasty model.

use sea_orm::entity::prelude::*;

/// User to Instance relationship maps
///
/// This is used to implement multiple-multiple relationships between two models and should NOT be used directly.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "user_instances")]
// `pub` here is not required but without it rustc would produce a warning :(
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(indexed)]
    pub user_id: Uuid,
    #[sea_orm(indexed)]
    pub instance_id: Uuid,
    #[sea_orm(belongs_to, from = "user_id", to = "id")]
    pub user: BelongsTo<super::users::Entity>,
    #[sea_orm(belongs_to, from = "instance_id", to = "id")]
    pub instance: BelongsTo<super::instances::Entity>,
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
