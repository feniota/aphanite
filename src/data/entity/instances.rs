//! SeaORM model matching the original toasty model.

use sea_orm::entity::prelude::*;

/// An modpack instance
///
/// This is used only in Phenocryst.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "instances")]
pub struct Model {
    /// The ID of the instance
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// Name of this instance
    #[sea_orm(column_type = "Text")]
    pub name: String,
    /// Description of this instance
    #[sea_orm(column_type = "Text")]
    pub description: String,
    /// Hash of the modpack file
    ///
    /// This should be a BLAKE3 hash in lowercase hex format.
    #[sea_orm(column_type = "Text")]
    pub hash: String,
    /// File of the modpack
    pub file: Uuid,
    #[sea_orm(has_many)]
    pub user_instances: HasMany<super::user_instances::Entity>,
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
