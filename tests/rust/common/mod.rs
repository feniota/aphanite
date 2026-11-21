//! Shared test helpers — database setup, user/profile creation, AppState construction.

use aphanite::AppState;
use aphanite::config::{
    AppConfig, DatabaseBackend, DatabaseConfig, LocalStorageConfig, ReverseProxyType,
    S3StorageConfig, ServiceConfig, StorageConfig, StorageType, TurnstileSettings, YggdrasilConfig,
};
use aphanite::kv_cache::KVCache;
use aphanite::storage::AssetStorage;
use aphanite::types::User;
use argon2::{Argon2, password_hash::PasswordHasher};
use rsa::PrivateKey;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::sync::{Arc, Once};
use uuid::Uuid;

pub type TestApp = ntex::service::Pipeline<
    ntex::http::Request,
    ntex::http::Response,
    ntex::web::WebError<ntex::web::AppState<AppState>>,
>;

/// Send an in-process HTTP request through the ntex application.
pub async fn request(
    app: &TestApp,
    method: ntex::http::Method,
    uri: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> ntex::http::Response {
    let mut request = ntex::web::test::TestRequest::with_uri(uri).method(method);
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    if let Some(body) = body {
        request = request.json(&body);
    }
    ntex::web::test::call_service(app, request.to_request()).await
}

/// Login to the General API and return the access token (Bearer).
#[allow(
    dead_code,
    reason = "Shared helper used by other integration test binaries"
)]
pub async fn login(app: &TestApp, email: &str, password: &str) -> String {
    let response = request(
        app,
        ntex::http::Method::POST,
        "/api/auth/login",
        None,
        Some(serde_json::json!({"email": email, "password": password})),
    )
    .await;
    assert_eq!(
        response.status(),
        ntex::http::StatusCode::OK,
        "login failed"
    );
    let body = ntex::web::test::read_body(response).await;
    let value: serde_json::Value =
        serde_json::from_slice(&body).expect("failed to parse login response");
    value["payload"]["access_token"]
        .as_str()
        .expect("missing access_token in login response")
        .to_string()
}

static CRYPTO_PROVIDER_INSTALL_LOCK: Once = Once::new();

/// Construct an [`AppState`] for integration testing.
///
/// Uses an in-memory Turso database and a minimal configuration.
pub async fn new_test_state(tmp_dir: &Path) -> anyhow::Result<AppState> {
    // Load CryptoProvider
    CRYPTO_PROVIDER_INSTALL_LOCK.call_once(|| {
        rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .unwrap()
    });

    // Create in-memory Turso database with all models registered
    let driver = toasty_driver_turso::Turso::in_memory();
    let db = toasty::Db::builder()
        .models(toasty::models!(
            User,
            aphanite::types::Token,
            aphanite::types::Instance,
            aphanite::types::UserInstance,
            aphanite::types::RegisterToken,
            aphanite::storage::File,
            aphanite::service::yggdrasil::types::GameProfile,
            aphanite::service::yggdrasil::types::ProfileTextures,
        ))
        .build(driver)
        .await?;
    db.push_schema().await?;

    // Generate a test RSA key (2048-bit for speed)
    let private_key = PrivateKey::new(&mut rand::rng(), 2048)?;
    let rsa_pubkey = private_key.as_public_key().clone();

    let config = AppConfig {
        service: ServiceConfig {
            listen: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 3000,
            path: None,
            domain: "localhost:3000".into(),
            data_path: tmp_dir.to_path_buf(),
            tls: false,
            client_ip: ReverseProxyType::Disabled,
            public: false,
            turnstile: TurnstileSettings {
                enabled: false,
                site_key: String::new(),
                secret_key: String::new(),
            },
        },
        storage: StorageConfig {
            storage_type: StorageType::Local,
            local: LocalStorageConfig {
                path: tmp_dir.join("assets"),
            },
            s3: S3StorageConfig {
                bucket_name: String::new(),
                endpoint: String::new(),
                region: String::new(),
                access_key: String::new(),
                secret_key: String::new(),
                domains: vec![],
            },
        },
        database: DatabaseConfig {
            backend: DatabaseBackend::Turso,
            postgres_url: String::new(),
        },
        yggdrasil: YggdrasilConfig {
            private_key,
            server_name: Some("Aphanite Test".into()),
            homepage: None,
            register_page: None,
        },
    };

    let storage = AssetStorage::from_config(db.clone(), &config);

    Ok(AppState {
        da: aphanite::data::DatabaseAccessor::new(db.clone()),
        cfg: Arc::new(config),
        assets: storage,
        kv: KVCache::new(),
        rsa_pubkey,
        http_client: ntex::client::Client::new(),
        base_path: String::new(),
    })
}

/// Password hash for the default test password `"password123"`.
pub fn test_password_hash() -> String {
    Argon2::default()
        .hash_password(b"password123")
        .unwrap()
        .to_string()
}

/// Create a test user with the given email and the default password `"password123"`.
pub async fn create_test_user(state: &AppState, email: &str) -> User {
    let mut db = state.da.db().clone();

    // Return existing user if already present
    if let Ok(user) = User::get_by_email(&mut db, email).await {
        return user;
    }

    User::create()
        .email(email)
        .nickname(email)
        .password(test_password_hash())
        .preferred_language("en_US")
        .permission(0)
        .exec(&mut db)
        .await
        .expect("failed to create test user")
}

/// Create a test profile for a user.
#[allow(
    dead_code,
    reason = "Shared helper used by other integration test binaries"
)]
pub async fn create_test_profile(
    state: &AppState,
    user_id: Uuid,
    name: &str,
) -> aphanite::service::yggdrasil::types::GameProfile {
    let mut db = state.da.db().clone();
    aphanite::service::yggdrasil::types::GameProfile::create()
        .owner_id(user_id)
        .name(name)
        .exec(&mut db)
        .await
        .expect("failed to create test profile")
}
