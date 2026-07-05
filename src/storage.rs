//! Generic abstract layer over the specific file storage

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use toasty::{Db, Embed, Model};
use tokio::io::AsyncRead;
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
    /// Root directory to store the files in
    ///
    /// This should typically be `data/assets`
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

    /// The BLAKE3 hash of this file, in lowercase hex form
    #[unique]
    pub hash: String,

    /// The "data" of this file, with which AssetsStorage instance can acquire the web URL to this file
    ///
    /// - For [`Local`](StorageType::Local), this is the path relative to [`LocalStorageConfiguration::path`]
    /// - For [`S3`](StorageType::S3), this is the key of the associated S3 object
    pub(in crate::storage) data: String,

    /// Reference count of this file
    ///
    /// NOTE: This is maintained by ourselves and unrelated to database logic.
    ///
    /// Whenever a file is [created](AssetsStorage::create_file) but its hash already existing, we point that "newly created" [`File`] instance to the existing one, and increase this by 1.
    ///
    /// Whenever a file is [deleted](AssetsStorage::delete_file), we decrease this by
    pub(in crate::storage) ref_count: u16,
}

impl AsRef<Uuid> for File {
    fn as_ref(&self) -> &Uuid {
        &self.id
    }
}

pub struct FileReader {
    pub file: File,
    storage: Arc<AssetsStorage>,
}

impl AsyncRead for FileReader {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        todo!();
    }
}

#[derive(Clone)]
pub struct AssetsStorage {
    db: Db,
    conf: Arc<StorageConfiguration>,
}

impl AssetsStorage {
    /// Create a new AssetsStorage instance based on the given parameter
    pub fn new(db: Db, config: StorageConfiguration) -> Self {
        Self {
            db,
            conf: Arc::new(config),
        }
    }

    /// Get all the possible domains that may appear in URLs that this struct returns
    pub fn whitelist_domain(&self) -> Option<String> {
        None
    }

    /// Get an axum [`Router`](axum::Router) hosting the files in this storage
    ///
    /// The returned router (if any) should be mounted behind `/assets/`.
    ///
    /// # Returns
    /// - Some(router): if the storage is [`Local`](StorageConfiguration::Local) and an router is needed;
    /// - None: if the storage is [`S3`](StorageConfiguration::S3) and an router is not needed.
    pub fn router(&self) -> Option<axum::Router> {
        todo!();
    }

    /// Get the URL to a specific [`File`] by its ID
    pub async fn get_url<FileId>(&self, uuid: FileId) -> Option<String>
    where
        FileId: AsRef<Uuid>,
    {
        todo!();
    }

    /// Get the URL to a specific [`File`] with an instance
    pub async fn get_url_by_file(&self, file: &File) -> Option<String> {
        todo!();
    }

    /// Get a [`File`] instance by its ID
    pub async fn get_file<FileId>(&self, uuid: FileId) -> Option<File>
    where
        FileId: AsRef<Uuid>,
    {
        todo!();
    }

    /// Create a [`File`] instance and write the data provided by `input_stream`, consuming that Reader
    pub async fn create_file<R>(&self, input_stream: R) -> Result<File>
    where
        R: AsyncRead,
    {
        todo!();
    }

    pub async fn delete_file<FileId>(&self, uuid: FileId) -> Option<String>
    where
        FileId: AsRef<Uuid>,
    {
        todo!();
    }
}
