//! Shared test helpers — database setup, user/profile creation, AppState construction.

use aphanite::AppState;
use aphanite::config::{
    AppConfig, DatabaseBackend, DatabaseConfig, LocalStorageConfig, ReverseProxyType,
    S3StorageConfig, ServiceConfig, StorageConfig, StorageType, TurnstileSettings, YggdrasilConfig,
};
use aphanite::kv_cache::KVCache;
use aphanite::storage::AssetStorage;
use aphanite::types::User;
use argon2::{
    Argon2,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};
use rsa::RsaPrivateKey;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

/// Login to the General API and return the access token (Bearer).
pub async fn login(app: &axum::Router, email: &str, password: &str) -> String {
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::json;
    use tower::ServiceExt;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": email, "password": password}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value =
        serde_json::from_slice(&body).expect("failed to parse login response");
    v["payload"]["access_token"]
        .as_str()
        .expect("missing access_token in login response")
        .to_string()
}

/// Construct an [`AppState`] for integration testing.
///
/// Uses an in-memory SQLite database and a minimal configuration.
pub async fn new_test_state(tmp_dir: &Path) -> anyhow::Result<AppState> {
    // Create in-memory SQLite database with all models registered
    let db = toasty::Db::builder()
        .models(toasty::models!(
            aphanite::types::User,
            aphanite::types::Token,
            aphanite::types::Instance,
            aphanite::types::UserInstance,
            aphanite::types::RegisterToken,
            aphanite::storage::File,
            aphanite::service::yggdrasil::types::GameProfile,
            aphanite::service::yggdrasil::types::ProfileTextures,
        ))
        .connect("sqlite::memory:")
        .await?;
    db.push_schema().await?;

    // Generate a test RSA key (2048-bit for speed)
    let private_key = RsaPrivateKey::new(&mut rand::rng(), 2048)?;
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
            backend: DatabaseBackend::Sqlite,
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
        http_client: reqwest::Client::new(),
    })
}

/// Password hash for the default test password `"pass"`.
pub fn test_password_hash() -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(b"pass", &salt)
        .unwrap()
        .to_string()
}

/// Create a test user with the given email and the default password `"pass"`.
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
