//! Generic abstract layer over the specific file storage

use crate::config::StorageConfig;
use anyhow::{Result, anyhow};
use compio::buf::{IntoInner, IoBuf};
use compio::fs::{self, File as AsyncFile};
use compio::io::{AsyncReadAt, AsyncWriteAtExt};
use futures_lite::{AsyncRead, AsyncReadExt};
use rusty_s3::S3Action;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use toasty::{Db, Model};
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
        match &config.storage {
            StorageConfig::Local(local) => Self::Local(LocalStorageConfiguration {
                path: local.path.clone(),
            }),
            StorageConfig::S3(s3) => {
                let credentials =
                    rusty_s3::Credentials::new(s3.access_key.clone(), s3.secret_key.clone());
                let bucket = match rusty_s3::Bucket::new(
                    s3.endpoint.clone(),
                    rusty_s3::UrlStyle::VirtualHost,
                    s3.bucket_name.clone(),
                    s3.region.clone(),
                ) {
                    Ok(bucket) => bucket,
                    Err(e) => {
                        tracing::error!("Failed to create S3 bucket: {}", e);
                        std::process::exit(1);
                    }
                };
                Self::S3(S3StorageConfiguration {
                    bucket: Arc::new(bucket),
                    credentials: Arc::new(credentials),
                    allowed_domains: s3.domains.clone(),
                })
            }
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
    pub bucket: Arc<rusty_s3::Bucket>,
    pub credentials: Arc<rusty_s3::Credentials>,
    pub allowed_domains: Vec<String>,
}

impl S3StorageConfiguration {
    async fn put_object(
        &self,
        client: &ntex::client::Client,
        key: &str,
        content: Vec<u8>,
    ) -> Result<()> {
        let url = self
            .bucket
            .put_object(Some(&self.credentials), key)
            .sign(Duration::from_secs(900));
        let response = client
            .put(url.as_str())
            .content_length(content.len() as u64)
            .send_body(content)
            .await
            .map_err(|e| anyhow!("Failed to send S3 upload: {e}"))?;
        Self::check_response(response).await
    }

    async fn delete_object(&self, client: &ntex::client::Client, key: &str) -> Result<()> {
        let url = self
            .bucket
            .delete_object(Some(&self.credentials), key)
            .sign(Duration::from_secs(900));
        let response = client
            .delete(url.as_str())
            .send()
            .await
            .map_err(|e| anyhow!("Failed to send S3 deletion: {e}"))?;
        Self::check_response(response).await
    }

    async fn check_response(response: ntex::client::ClientResponse) -> Result<()> {
        let status = response.status();
        // Consume the body so the connection can be reused on successful requests.
        let body = response
            .body()
            .await
            .map_err(|e| anyhow!("Failed to read S3 response (HTTP {status}): {e}"))?;
        anyhow::ensure!(
            status.is_success(),
            "S3 returned HTTP {status}: {}",
            String::from_utf8_lossy(&body),
        );
        Ok(())
    }
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
pub struct MissingClient;

#[derive(Clone)]
pub struct AssetStorage<C = Rc<ntex::client::Client>> {
    db: Db,
    conf: Arc<StorageConfiguration>,
    tmp: PathBuf,

    /// The base URL of asset storage service, with ending /
    ///
    /// Example: "https://aphanite.example.com/path/to/aphanite/assets/"
    base_url: String,
    self_domain: String,

    client: C,
}
/// Representation type of the returned file URL
///
/// Used in [`AssetStorage::get_url_by_file`]
#[allow(unused)]
pub enum FileUrlType {
    Uuid,
    Hash,
}

impl AssetStorage<MissingClient> {
    /// Create a new AssetStorage instance based on the given parameter
    pub fn from_config(db: Db, config: &crate::config::AppConfig) -> Self {
        let conf = Arc::new(StorageConfiguration::from_config(config));
        let base_url = format!("{}assets/", config.service.base_url);

        let tmp = config.service.data_path.join("tmp");
        if let Err(e) = std::fs::create_dir_all(&tmp)
            && e.kind() != std::io::ErrorKind::AlreadyExists
        {
            panic!("{}", e);
        }

        Self {
            db,
            conf,
            tmp: config.service.data_path.join("tmp"),
            base_url,
            self_domain: config.service.domain.clone(),
            client: MissingClient,
        }
    }

    pub fn install_client(
        &self,
        client: Rc<ntex::client::Client>,
    ) -> AssetStorage<Rc<ntex::client::Client>> {
        AssetStorage {
            db: self.db.clone(),
            conf: self.conf.clone(),
            tmp: self.tmp.clone(),
            base_url: self.base_url.clone(),
            self_domain: self.self_domain.clone(),
            client,
        }
    }
}

impl AssetStorage<Rc<ntex::client::Client>> {
    /// Get all the possible domains that may appear in URLs that [`get_url()`](Self::get_url) or [`get_url_by_file()`](Self::get_url_by_file) returns
    pub fn whitelisted_domains(&self) -> Vec<String> {
        let mut domains = vec![self.self_domain.clone()];
        if let StorageConfiguration::S3(conf) = StorageConfiguration::clone(&self.conf) {
            domains.append(&mut conf.allowed_domains.clone());
        }
        domains
    }

    /// Register ntex routes hosting files in this storage
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
    pub fn configure(cfg: &mut ntex::web::ServiceConfig<ntex::web::AppState<crate::AppState>>) {
        use ntex::web;
        cfg.service(get_file);

        #[web::get("/{path}", state = crate::service::HttpState)]
        async fn get_file(
            state: crate::AppState,
            path: web::types::Path<String>,
            request: web::HttpRequest,
        ) -> crate::service::Result<web::HttpResponse> {
            if state.assets.conf.is_local() {
                local_file_handler::get_file(path.into_inner(), &state.assets, request.headers())
                    .await
            } else {
                get_s3_file(path.into_inner(), &state.assets).await
            }
        }

        async fn get_s3_file(
            path: String,
            state: &AssetStorage<Rc<ntex::client::Client>>,
        ) -> crate::service::Result<web::HttpResponse> {
            use ntex::web::HttpResponse;

            let mut db = state.db.clone();
            let conf = match StorageConfiguration::clone(&state.conf) {
                StorageConfiguration::S3(c) => c,
                _ => unreachable!(),
            };

            let file = match if path.contains('-') {
                File::get_by_id(
                    &mut db,
                    Uuid::parse_str(&path).map_err(|_| {
                        crate::service::Error::error(400, "Requested path is not valid UUID")
                    })?,
                )
                .await
            } else {
                File::get_by_hash(&mut db, path).await
            } {
                Ok(x) => x,
                Err(e) if e.is_record_not_found() => {
                    return Err(crate::service::Error::error(
                        404,
                        "Requested record not found",
                    ));
                }
                Err(e) => return Err(e.into()),
            };

            let presigned_url = conf
                .bucket
                .get_object(Some(&conf.credentials), &file.data)
                .sign(Duration::from_secs(900));
            Ok(HttpResponse::Found()
                .header(ntex::http::header::LOCATION, presigned_url.as_str())
                .build())
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
            let mut fs_file = AsyncFile::create(&temp_file).await?;
            let mut buffer = vec![0_u8; 8192];
            let mut offset = 0_u64;

            loop {
                let count = input_stream.read(&mut buffer).await?;
                if count == 0 {
                    break;
                }
                hasher.update(&buffer[..count]);
                // Completion I/O owns the buffer until the write finishes.
                let (result, slice) = fs_file
                    .write_all_at(buffer.slice(..count), offset)
                    .await
                    .into_parts();
                buffer = slice.into_inner();
                result?;
                offset += count as u64;
            }
            fs_file.sync_data().await?;
            hasher.finalize().to_hex()
        };

        // Then check if there is existing item that shares this hash
        let mut db = self.db.clone();
        if let Ok(mut f) = File::get_by_hash(&mut db, hash.as_str()).await {
            // there is existing item; dropping the temp file
            let _ = fs::remove_file(&temp_file).await;
            let new_rc = f
                .ref_count
                .checked_add(1)
                .ok_or_else(|| anyhow!("File reference count overflowed"))?;
            f.update().ref_count(new_rc).exec(&mut db).await?;

            return Ok(f);
        }

        match StorageConfiguration::clone(&self.conf) {
            StorageConfiguration::Local(conf) => {
                fs::create_dir_all(&conf.path).await?;
                let file_id = Uuid::now_v7();
                let file_id_str = file_id.as_hyphenated().to_string();
                let f = File::create()
                    .id(file_id)
                    .hash(hash.as_str())
                    .ref_count(1)
                    .data(&file_id_str)
                    .exec(&mut db)
                    .await?;
                if fs::rename(&temp_file, conf.path.join(&file_id_str))
                    .await
                    .is_err()
                {
                    // Seems like the temp file and our storage are on different partitions; copying instead
                    tracing::warn!(
                        "Failed to rename {} to {}! Are these two directories on different partitions? Try copying instead...",
                        &temp_file.display(),
                        &conf.path.display()
                    );
                    copy_file(&temp_file, &conf.path.join(&file_id_str)).await?;
                    fs::remove_file(&temp_file).await?;
                }
                Ok(f)
            }
            StorageConfiguration::S3(conf) => {
                let content = fs::read(&temp_file).await?;
                if let Err(e) = conf.put_object(&self.client, hash.as_str(), content).await {
                    // Clean up temp file before propagating the error
                    let _ = fs::remove_file(&temp_file).await;
                    return Err(anyhow!("Failed to upload to S3: {}", e));
                }
                fs::remove_file(&temp_file).await?;

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
                match fs::remove_file(path).await {
                    Ok(_) => Some(f.hash),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(f.hash),
                    Err(_) => None,
                }
            }
            StorageConfiguration::S3(conf) => {
                File::delete_by_id(&mut db, &f.id).await.ok()?;
                if let Err(e) = conf.delete_object(&self.client, &f.data).await {
                    tracing::warn!("Failed to delete S3 object (hash={}): {}", f.hash, e);
                }
                Some(f.hash)
            }
        }
    }
}

/// Copy across filesystems using owned buffers and positional completion I/O.
async fn copy_file(source: &std::path::Path, destination: &std::path::Path) -> std::io::Result<()> {
    let source = AsyncFile::open(source).await?;
    let mut destination = AsyncFile::create(destination).await?;
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut offset = 0_u64;
    loop {
        let (result, returned_buffer) = source.read_at(buffer, offset).await.into_parts();
        buffer = returned_buffer;
        let count = result?;
        if count == 0 {
            break;
        }
        let (result, slice) = destination
            .write_all_at(buffer.slice(..count), offset)
            .await
            .into_parts();
        buffer = slice.into_inner();
        result?;
        offset += count as u64;
    }
    destination.sync_data().await
}

mod local_file_handler {
    use crate::service::Error as AphaniteError;
    use crate::service::Result as AphaniteResult;
    use compio::buf::{IntoInner, IoBuf};
    use compio::fs::File as AsyncFile;
    use compio::io::AsyncReadAt;
    use ntex::http::{HeaderMap, StatusCode, header};
    use ntex::web::HttpResponse as Response;

    pub async fn get_file<C>(
        path: String,
        state: &super::AssetStorage<C>,
        header: &HeaderMap,
    ) -> AphaniteResult<Response> {
        use ntex::util::Bytes;
        use std::cmp::min;

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
        let fs_file = match AsyncFile::open(&file_path).await {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(AphaniteError::error(404, "Requested file not found"));
            }
            Err(e) => return Err(e.into()),
        };

        let total_len = fs_file.metadata().await?.len();

        let (result, sniff) = fs_file.read_at(vec![0_u8; 8192], 0).await.into_parts();
        let sniff_len = result?;
        let mime = infer::get(&sniff[..sniff_len])
            .map(|kind| kind.mime_type())
            .unwrap_or("application/octet-stream");

        let mut status = StatusCode::OK;
        let mut range_start = 0_u64;
        let mut content_len = total_len;
        let mut content_range: Option<String> = None;

        if let Some(range_header) = header.get(header::RANGE).and_then(|h| h.to_str().ok()) {
            let parse_fail =
                || AphaniteError::new(StatusCode::RANGE_NOT_SATISFIABLE, "Invalid Range header");

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

        let (tx, rx) = async_channel::bounded::<std::io::Result<Bytes>>(8);
        ntex::rt::spawn(async move {
            let mut remaining = content_len;
            let mut offset = range_start;
            let mut buffer = vec![0_u8; 64 * 1024];
            while remaining > 0 {
                let request_size = min(buffer.len() as u64, remaining) as usize;
                let (result, slice) = fs_file
                    .read_at(buffer.slice(..request_size), offset)
                    .await
                    .into_parts();
                buffer = slice.into_inner();
                match result {
                    Ok(0) => break,
                    Ok(n) => {
                        remaining -= n as u64;
                        offset += n as u64;
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

        let mut builder = Response::builder(status);
        builder
            .header(header::CONTENT_TYPE, mime)
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CONTENT_LENGTH, content_len.to_string());
        if let Some(v) = content_range {
            builder.header(header::CONTENT_RANGE, v);
        }
        Ok(builder.streaming(Box::pin(rx)))
    }
}

#[cfg(test)]
mod tests {
    fn mock_s3(
        status: &str,
    ) -> (
        super::S3StorageConfiguration,
        std::thread::JoinHandle<(String, Vec<u8>)>,
    ) {
        use std::io::{Read, Write};
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let status = status.to_owned();
        let task = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let (headers, body) = loop {
                let mut buffer = [0_u8; 1024];
                let count = stream.read(&mut buffer).unwrap();
                assert!(count > 0, "request ended before the body was complete");
                request.extend_from_slice(&buffer[..count]);
                if let Some(end) = request.windows(4).position(|x| x == b"\r\n\r\n") {
                    let headers = String::from_utf8(request[..end].to_vec()).unwrap();
                    let length = headers
                        .lines()
                        .filter_map(|line| line.split_once(':'))
                        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                        .map(|(_, value)| value.trim().parse::<usize>().unwrap())
                        .unwrap_or(0);
                    if request.len() >= end + 4 + length {
                        break (headers, request[end + 4..end + 4 + length].to_vec());
                    }
                }
            };
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
            (headers, body)
        });
        let bucket = rusty_s3::Bucket::new(
            endpoint.parse().unwrap(),
            rusty_s3::UrlStyle::Path,
            "assets",
            "test-region",
        )
        .unwrap();
        (
            super::S3StorageConfiguration {
                bucket: std::sync::Arc::new(bucket),
                credentials: std::sync::Arc::new(rusty_s3::Credentials::new("test", "secret")),
                allowed_domains: vec![],
            },
            task,
        )
    }

    #[ntex::test]
    async fn s3_put_sends_signed_body_and_explicit_length() {
        for content in [vec![0, 1, 128, 255], vec![]] {
            let (conf, request) = mock_s3("200 OK");
            conf.put_object(&ntex::client::Client::new(), "hash", content.clone())
                .await
                .unwrap();
            let (headers, body) = request.join().unwrap();
            assert!(headers.starts_with("PUT /assets/hash?"));
            assert!(headers.contains("X-Amz-Signature="));
            assert!(headers.contains("X-Amz-Expires=900"));
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains(&format!("content-length: {}", content.len()))
            );
            assert_eq!(body, content);
        }
    }

    #[ntex::test]
    async fn s3_put_rejects_http_errors() {
        let (conf, request) = mock_s3("403 Forbidden");
        let error = conf
            .put_object(&ntex::client::Client::new(), "hash", vec![1])
            .await
            .unwrap_err();
        assert!(error.to_string().contains("403"));
        request.join().unwrap();
    }

    #[ntex::test]
    async fn s3_delete_sends_signed_request_and_checks_status() {
        for (status, success) in [("204 No Content", true), ("403 Forbidden", false)] {
            let (conf, request) = mock_s3(status);
            let result = conf
                .delete_object(&ntex::client::Client::new(), "hash")
                .await;
            assert_eq!(result.is_ok(), success);
            let (headers, body) = request.join().unwrap();
            assert!(headers.starts_with("DELETE /assets/hash?"));
            assert!(headers.contains("X-Amz-Signature="));
            assert!(body.is_empty());
        }
    }

    #[ntex::test]
    async fn copy_file_preserves_content_and_truncates_destination() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let destination = tmp.path().join("destination");
        let content: Vec<u8> = (0..160_003).map(|i| (i % 251) as u8).collect();
        std::fs::write(&source, &content).unwrap();
        std::fs::write(&destination, vec![0_u8; content.len() + 100]).unwrap();
        super::copy_file(&source, &destination).await.unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), content);
        std::fs::write(&source, []).unwrap();
        super::copy_file(&source, &destination).await.unwrap();
        assert!(std::fs::read(&destination).unwrap().is_empty());
    }
}
