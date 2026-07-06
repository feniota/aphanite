//! Generic abstract layer over the specific file storage

use anyhow::{Result, anyhow};
use std::path::PathBuf;
use std::sync::Arc;
use toasty::{Db, Model};
use tokio::fs::File as TokioFile;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

#[derive(Clone)]
pub enum StorageConfiguration {
    Local(LocalStorageConfiguration),
    S3(S3StorageConfiguration),
}

impl StorageConfiguration {
    fn local(self) -> Option<LocalStorageConfiguration> {
        if let Self::Local(x) = self {
            Some(x)
        } else {
            None
        }
    }

    fn is_local(&self) -> bool {
        matches!(self, Self::Local(_))
    }

    fn from_config(config: &crate::config::AppConfig) -> Self {
        if let crate::config::StorageType::Local = config.storage.storage_type {
            Self::Local(LocalStorageConfiguration {
                path: config.storage.local.path.clone(),
            })
        } else {
            let region = s3::Region::Custom {
                region: config.storage.s3.region.clone(),
                endpoint: config.storage.s3.endpoint.clone(),
            };
            // When the first parameter is Some() this function always returns an Ok()
            let credentials = s3::creds::Credentials::new(
                Some(&config.storage.s3.access_key),
                Some(&config.storage.s3.secret_key),
                None,
                None,
                None,
            )
            .unwrap();
            let bucket =
                s3::bucket::Bucket::new(&config.storage.s3.bucket_name, region, credentials);
            if let Err(e) = bucket {
                tracing::error!("Failed to create S3 bucket: {}", e.to_string());
                std::process::exit(1);
            }
            Self::S3(S3StorageConfiguration {
                bucket: Arc::new(bucket.unwrap()),
                allowed_domains: config.storage.s3.domains.clone(),
            })
        }
    }
}

#[derive(Clone)]
pub struct LocalStorageConfiguration {
    /// Root directory to store the files in
    ///
    /// This should typically be `data/assets`
    pub path: PathBuf,
}

#[derive(Clone)]
pub struct S3StorageConfiguration {
    pub bucket: Arc<Box<s3::bucket::Bucket>>,
    pub allowed_domains: Vec<String>,
}

/// A file stored in [`AssetStorage`]
///
/// This struct itself is only metadata. You need a [`AssetStorage`] instance to access its content.
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

    /// The "data" of this file, with which AssetStorage instance can acquire the web URL to this file
    ///
    /// - For [`Local`](StorageConfiguration::Local), this is the path relative to [`LocalStorageConfiguration::path`]
    /// - For [`S3`](StorageConfiguration::S3), this is the key of the associated S3 object
    pub(in crate::storage) data: String,

    /// Reference count of this file
    ///
    /// NOTE: This is maintained by ourselves and unrelated to database logic.
    ///
    /// Whenever a file is [created](AssetStorage::create_file) but its hash already existing, we point that "newly created" [`File`] instance to the existing one, and increase this by 1.
    ///
    /// Whenever a file is [deleted](AssetStorage::delete_file), we decrease this by 1, and only when this reaches 0 would that file get deleted.
    pub(in crate::storage) ref_count: u16,
}

impl AsRef<Uuid> for File {
    fn as_ref(&self) -> &Uuid {
        &self.id
    }
}

#[derive(Clone)]
pub struct AssetStorage {
    db: Db,
    conf: Arc<StorageConfiguration>,
    tmp: PathBuf,

    /// The base URL of asset storage service, with ending /
    ///
    /// Example: "https://aphanite.example.com/path/to/aphanite/assets/"
    base_url: String,
    self_domain: String,
}
/// Representation type of the returned file URL
///
/// Used in [`AssetStorage::get_url_by_file`]
#[allow(unused)]
pub enum FileUrlType {
    Uuid,
    Hash,
}

impl AssetStorage {
    /// Create a new AssetStorage instance based on the given parameter
    pub fn from_config(db: Db, config: &crate::config::AppConfig) -> Self {
        let conf = Arc::new(StorageConfiguration::from_config(config));
        let subpath = config.service.path.clone().unwrap_or("".into());
        let subpath = subpath.trim_start_matches("/").trim_end_matches("/");
        let mut base_path = format!(
            "{}://{}/{}",
            if config.service.tls { "https" } else { "http" },
            config.service.domain,
            &subpath
        );
        if !subpath.is_empty() {
            base_path = format!("{}/", base_path);
        }
        let base_url = format!("{}assets/", base_path);

        let tmp = config.service.data_path.join("tmp");
        if let Err(e) = std::fs::create_dir_all(&tmp) {
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                panic!("{}", e);
            }
        }

        Self {
            db,
            conf,
            tmp: config.service.data_path.join("tmp"),
            base_url,
            self_domain: config.service.domain.clone(),
        }
    }

    /// Get all the possible domains that may appear in URLs that [`get_url()`](Self::get_url) or [`get_url_by_file()`](Self::get_url_by_file) returns
    pub fn whitelisted_domains(&self) -> Vec<String> {
        let mut domains = vec![self.self_domain.clone()];
        if let StorageConfiguration::S3(conf) = StorageConfiguration::clone(&self.conf) {
            domains.append(&mut conf.allowed_domains.clone());
        }
        domains
    }

    /// Get an axum [`Router`](axum::Router) hosting the files in this storage
    ///
    /// The returned router should be mounted behind `/assets/`.
    ///
    /// # Behavior
    ///
    /// - For local storage, this hosts files directly to the client.
    /// - For S3 storage, this acquires a pre-signed URL with a TTL of 15min and 302 the client to that URL.
    ///
    /// # Router usage
    ///
    /// This router hosts files in the storage to the public. Both UUID([`File::id`]) and hash([`File::hash`]) are supported for convenience.
    ///
    /// - If the requested path contains hyphen (en dash `-`), that path is considered as a UUID;
    /// - If the requested path does not have hyphens, that path is considered as a hash.
    pub fn router(&self) -> axum::Router {
        use axum::routing::get;
        if self.conf.is_local() {
            axum::Router::new()
                .route("/{path}", get(local_file_axum_handler::get_file))
                .with_state(self.clone())
        } else {
            async fn get_s3_file(
                path: axum::extract::Path<String>,
                state: axum::extract::State<AssetStorage>,
            ) -> crate::Result<axum::response::Response> {
                use axum::body::Body;
                use axum::response::Response;

                let axum::extract::Path(path) = path;
                let axum::extract::State(state) = state;

                let mut db = state.db.clone();
                let conf = match StorageConfiguration::clone(&state.conf) {
                    StorageConfiguration::S3(c) => c,
                    _ => unreachable!(),
                };

                let file = match if path.contains('-') {
                    File::get_by_id(
                        &mut db,
                        uuid::Uuid::parse_str(&path).map_err(|_| {
                            crate::Error::error(400, "Requested path is not valid UUID")
                        })?,
                    )
                    .await
                } else {
                    File::get_by_hash(&mut db, path).await
                } {
                    Ok(x) => x,
                    Err(e) if e.is_record_not_found() => {
                        return Err(crate::Error::error(404, "Requested record not found"));
                    }
                    Err(e) => return Err(e.into()),
                };

                let presigned_url = conf
                    .bucket
                    .presign_get(
                        &file.data, 900, // 15 minutes TTL
                        None,
                    )
                    .await
                    .map_err(|e| {
                        crate::Error::new(
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Failed to generate presigned URL: {}", e),
                        )
                    })?;

                Ok(Response::builder()
                    .status(axum::http::StatusCode::FOUND)
                    .header(axum::http::header::LOCATION, presigned_url)
                    .body(Body::empty())?)
            }
            axum::Router::new()
                .route("/{path}", get(get_s3_file))
                .with_state(self.clone())
        }
    }

    /// Get the URL to a specific [`File`] by its ID
    ///
    /// This is a convenience wrapper around [`get_url_by_file`](Self::get_url_by_file) and hash representation is used here. If you need a UUID URL, please use `get_url_by_file` instead.
    pub async fn get_url<FileId>(&self, uuid: FileId) -> Option<String>
    where
        FileId: AsRef<Uuid>,
    {
        let f = self.get_file(uuid).await?;
        Some(self.get_url_by_file(&f, FileUrlType::Hash))
    }

    /// Get the URL to a specific [`File`] with an instance
    pub fn get_url_by_file(&self, file: &File, path_representation: FileUrlType) -> String {
        let path = match path_representation {
            FileUrlType::Hash => file.hash.clone(),
            FileUrlType::Uuid => file.id.as_hyphenated().to_string(),
        };
        format!("{}{}", self.base_url, path)
    }

    /// Get a [`File`] instance by its ID
    pub async fn get_file<FileId>(&self, uuid: FileId) -> Option<File>
    where
        FileId: AsRef<Uuid>,
    {
        let mut db = self.db.clone();
        File::get_by_id(&mut db, uuid.as_ref()).await.ok()
    }

    /// Create a [`File`] instance and write the data provided by `input_stream`, consuming that Reader
    pub async fn create_file<R>(&self, mut input_stream: R) -> Result<File>
    where
        R: Unpin + AsyncRead,
    {
        // First consume the input stream to a temporary file and hash it
        let temp_file = self.tmp.join(Uuid::now_v7().as_hyphenated().to_string());
        let hash = {
            let mut hasher = blake3::Hasher::new();
            let mut fs_file = TokioFile::create(&temp_file).await?;
            let mut buffer: [u8; 8192] = [0; 8192];

            loop {
                let count = input_stream.read(&mut buffer).await?;
                if count == 0 {
                    break;
                }
                hasher.update(&buffer[..count]);
                fs_file.write_all(&buffer[..count]).await?;
            }
            fs_file.flush().await?;
            fs_file.sync_data().await?;
            hasher.finalize().to_hex()
        };

        // Then check if there is existing item that shares this hash
        let mut db = self.db.clone();
        if let Ok(mut f) = File::get_by_hash(&mut db, hash.as_str()).await {
            // there is existing item; dropping the temp file
            let _ = tokio::fs::remove_file(&temp_file).await;
            let new_rc = f
                .ref_count
                .checked_add(1)
                .ok_or_else(|| anyhow!("File reference count overflowed"))?;
            f.update().ref_count(new_rc).exec(&mut db).await?;

            return Ok(f);
        }

        match StorageConfiguration::clone(&self.conf) {
            StorageConfiguration::Local(conf) => {
                tokio::fs::create_dir_all(&conf.path).await?;
                let file_id = Uuid::now_v7();
                let file_id_str = file_id.as_hyphenated().to_string();
                let f = File::create()
                    .id(file_id)
                    .hash(hash.as_str())
                    .ref_count(1)
                    .data(&file_id_str)
                    .exec(&mut db)
                    .await?;
                if let Err(_) = tokio::fs::rename(&temp_file, conf.path.join(&file_id_str)).await {
                    // Seems like the temp file and our storage are on different partitions; copying instead
                    tracing::warn!(
                        "Failed to rename {} to {}! Are these two directories on different partitions? Try copying instead...",
                        &temp_file.display(),
                        &conf.path.display()
                    );
                    tokio::fs::copy(&temp_file, conf.path.join(&file_id_str)).await?;
                    tokio::fs::remove_file(&temp_file).await?;
                }
                Ok(f)
            }
            StorageConfiguration::S3(conf) => {
                let content = tokio::fs::read(&temp_file).await?;
                if let Err(e) = conf.bucket.put_object(hash.as_str(), &content).await {
                    // Clean up temp file before propagating the error
                    let _ = tokio::fs::remove_file(&temp_file).await;
                    return Err(anyhow!("Failed to upload to S3: {}", e));
                }
                tokio::fs::remove_file(&temp_file).await?;

                let f = File::create()
                    .hash(hash.as_str())
                    .ref_count(1)
                    .data(hash.as_str())
                    .exec(&mut db)
                    .await?;
                Ok(f)
            }
        }
    }

    /// Delete a file in the storage
    ///
    /// # Note
    ///
    /// Actual file content (in the FS or S3 bucket) is managed separately through a reference count system.
    /// Thus, one MUST call this function to decrease the file's reference count when unlinking a File with its owner data. Otherwise, in the actual storage the file might persist forever.
    ///
    /// # Returns
    ///
    /// The hash of the deleted file (if any)
    pub async fn delete_file<FileId>(&self, uuid: FileId) -> Option<String>
    where
        FileId: AsRef<Uuid>,
    {
        let mut db = self.db.clone();
        let mut f = File::get_by_id(&mut db, uuid.as_ref()).await.ok()?;

        if f.ref_count > 1 {
            let new_rc = f.ref_count.checked_sub(1)?;
            f.update().ref_count(new_rc).exec(&mut db).await.ok()?;
            return Some(f.hash);
        }

        match StorageConfiguration::clone(&self.conf) {
            StorageConfiguration::Local(conf) => {
                File::delete_by_id(&mut db, &f.id).await.ok()?;
                let path = conf.path.join(&f.data);
                match tokio::fs::remove_file(path).await {
                    Ok(_) => Some(f.hash),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(f.hash),
                    Err(_) => None,
                }
            }
            StorageConfiguration::S3(conf) => {
                File::delete_by_id(&mut db, &f.id).await.ok()?;
                if let Err(e) = conf.bucket.delete_object(&f.data).await {
                    tracing::warn!("Failed to delete S3 object (hash={}): {}", f.hash, e);
                }
                Some(f.hash)
            }
        }
    }
}

mod local_file_axum_handler {
    use crate::types::Error as AphaniteError;
    use crate::types::Result as AphaniteResult;
    use axum::extract::{Path, State};
    use axum::http::{HeaderMap, StatusCode, header};
    use axum::response::Response;
    use tokio::fs::File as TokioFile;
    use tokio::io::AsyncReadExt;

    pub async fn get_file(
        Path(path): Path<String>,
        State(state): State<super::AssetStorage>,
        header: HeaderMap,
    ) -> AphaniteResult<Response> {
        use axum::body::{Body, Bytes};
        use std::cmp::min;
        use std::io::SeekFrom;
        use tokio::io::AsyncSeekExt;
        use tokio_stream::wrappers::ReceiverStream;

        let mut db = state.db.clone();
        let conf = super::StorageConfiguration::clone(&state.conf)
            .local()
            .unwrap();
        let file = match if path.contains('-') {
            super::File::get_by_id(
                &mut db,
                uuid::Uuid::parse_str(&path)
                    .map_err(|_| AphaniteError::error(400, "Requested path is not valid UUID"))?,
            )
            .await
        } else {
            super::File::get_by_hash(&mut db, path).await
        } {
            Ok(x) => x,
            Err(e) if e.is_record_not_found() => {
                return Err(AphaniteError::error(404, "Requested record not found"));
            }
            Err(e) => return Err(e.into()),
        };

        let file_path = conf.path.join(&file.data);
        let mut fs_file = match TokioFile::open(&file_path).await {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(AphaniteError::error(404, "Requested file not found"));
            }
            Err(e) => return Err(e.into()),
        };

        let total_len = fs_file.metadata().await?.len();

        let mut sniff = [0_u8; 8192];
        let sniff_len = fs_file.read(&mut sniff).await?;
        let mime = infer::get(&sniff[..sniff_len])
            .map(|kind| kind.mime_type())
            .unwrap_or("application/octet-stream");

        let mut status = StatusCode::OK;
        let mut range_start = 0_u64;
        let mut content_len = total_len;
        let mut content_range: Option<String> = None;

        if let Some(range_header) = header.get(header::RANGE).and_then(|h| h.to_str().ok()) {
            let parse_fail = || {
                AphaniteError::new(
                    StatusCode::RANGE_NOT_SATISFIABLE,
                    "Invalid Range header".to_string(),
                )
            };

            if !range_header.starts_with("bytes=") {
                return Err(parse_fail());
            }

            let range_spec = &range_header["bytes=".len()..];
            if range_spec.contains(',') {
                return Err(parse_fail());
            }

            let (start, end) = match range_spec
                .split_once('-')
                .map(|(x, y)| (x.trim(), y.trim()))
            {
                Some((start_s, end_s)) => {
                    if start_s.is_empty() {
                        // suffix-byte-range-spec: bytes=-N
                        let suffix_len = end_s.parse::<u64>().map_err(|_| parse_fail())?;
                        if suffix_len == 0 || total_len == 0 {
                            return Err(parse_fail());
                        }
                        let start = total_len.saturating_sub(suffix_len);
                        (start, total_len - 1)
                    } else {
                        let start = start_s.parse::<u64>().map_err(|_| parse_fail())?;
                        if total_len == 0 || start >= total_len {
                            return Err(parse_fail());
                        }
                        let end = if end_s.is_empty() {
                            total_len - 1
                        } else {
                            end_s.parse::<u64>().map_err(|_| parse_fail())?
                        };
                        if start > end {
                            return Err(parse_fail());
                        }
                        (start, end.min(total_len - 1))
                    }
                }
                None => return Err(parse_fail()),
            };

            status = StatusCode::PARTIAL_CONTENT;
            range_start = start;
            content_len = end - start + 1;
            content_range = Some(format!("bytes {start}-{end}/{total_len}"));
        }

        fs_file.seek(SeekFrom::Start(range_start)).await?;

        let (tx, rx) = tokio::sync::mpsc::channel::<std::io::Result<Bytes>>(8);
        tokio::spawn(async move {
            let mut remaining = content_len;
            let mut buffer = vec![0_u8; 64 * 1024];
            while remaining > 0 {
                let request_size = min(buffer.len(), remaining as usize);
                match fs_file.read(&mut buffer[..request_size]).await {
                    Ok(0) => break,
                    Ok(n) => {
                        remaining -= n as u64;
                        if tx
                            .send(Ok(Bytes::copy_from_slice(&buffer[..n])))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                        break;
                    }
                }
            }
        });

        let mut builder = Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, mime)
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CONTENT_LENGTH, content_len.to_string());
        if let Some(v) = content_range {
            builder = builder.header(header::CONTENT_RANGE, v);
        }
        Ok(builder.body(Body::from_stream(ReceiverStream::new(rx)))?)
    }
}
