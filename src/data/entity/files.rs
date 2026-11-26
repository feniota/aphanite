//! SeaORM model matching the original toasty model.

use crate::data::value_types::Unsigned16;
use sea_orm::entity::prelude::*;

/// A file stored in [`AssetStorage`](crate::storage::AssetStorage)
///
/// This struct itself is only metadata. You need a [`AssetStorage`](crate::storage::AssetStorage) instance to access its content.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "files")]
pub struct Model {
    /// The internal ID of this file
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// The time of creation
    pub created_at: DateTimeUtc,
    /// The BLAKE3 hash of this file, in lowercase hex form
    #[sea_orm(column_type = "Text", unique)]
    pub hash: String,
    /// The "data" of this file, with which AssetStorage instance can acquire the web URL to this file
    ///
    /// - For [`Local`](crate::config::StorageConfiguration::Local), this is the path relative to [`LocalStorageConfiguration::path`](crate::config::LocalStorageConfiguration::path)
    /// - For [`S3`](crate::config::StorageConfiguration::S3), this is the key of the associated S3 object
    #[sea_orm(column_type = "Text")]
    pub data: String,
    /// Reference count of this file
    ///
    /// NOTE: This is maintained by ourselves and unrelated to database logic.
    ///
    /// Whenever a file is [created](crate::storage::AssetStorage::create_file) but its hash already existing, we point that "newly created" [`File`](Model) instance to the existing one, and increase this by 1.
    ///
    /// Whenever a file is [deleted](crate::storage::AssetStorage::delete_file), we decrease this by 1, and only when this reaches 0 would that file get deleted.
    pub ref_count: Unsigned16,
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

impl AsRef<Uuid> for Model {
    fn as_ref(&self) -> &Uuid {
        &self.id
    }
}
