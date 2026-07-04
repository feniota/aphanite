//! Generic abstract layer over the specific file storage

use std::path::PathBuf;
use std::sync::Arc;
use toasty::{Db, Embed, Model};
use uuid::Uuid;

pub enum StorageConfiguration {
    Local(LocalStorageConfiguration),
    S3(S3StorageConfiguration),
}

#[derive(Clone, Copy, Embed)]
pub enum StorageType {
    Local,
    S3,
}

pub struct LocalStorageConfiguration {
    pub path: PathBuf,
}

pub struct S3StorageConfiguration {
    // TODO
}

#[derive(Model, Clone)]
pub struct File {
    /// The internal ID of this file
    #[key]
    #[auto]
    pub id: Uuid,

    /// The time of creation
    #[auto]
    pub created_at: jiff::Timestamp,

    /// The storage type of this file (i.e. S3 or Local?)
    pub storage: StorageType,

    /// The BLAKE3 hash of this file, in lowercase hex form
    #[unique]
    pub hash: String,

    /// The "data" of this file, with which AssetsStorage instance can acquire the web URL to this file
    pub data: String,
}

pub struct AssetsStorage {
    db: Db,
    storage_type: StorageConfiguration,
}

impl AssetsStorage {
    pub fn new(db: Db, storage_type: StorageConfiguration) -> Self {
        Self { db, storage_type }
    }
    pub fn whitelist_domain(&self) -> Option<String> {
        None
    }
    pub async fn get_url(&self, uuid: Uuid) -> Option<String> {
        todo!();
    }
}
