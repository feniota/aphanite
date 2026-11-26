//! Shared test helpers — database setup, user/profile creation, AppState construction.

use aphanite::AppState;
use aphanite::config::{
    AppConfig, ClientIpType, DatabaseConfig, LocalStorageConfig, ServiceConfig, StorageConfig,
    TurnstileSettings, YggdrasilConfig,
};
use aphanite::data::entity::{game_profiles, users};
use aphanite::kv_cache::KVCache;
use aphanite::storage::AssetStorage;
use argon2::{Argon2, password_hash::PasswordHasher};
use rsa::PrivateKey;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, QueryFilter, Set};
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::sync::{Arc, Once};
use users::Model as User;
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
/// Uses an in-memory SQLite database and a minimal configuration.
pub async fn new_test_state(tmp_dir: &Path) -> anyhow::Result<AppState> {
    // Load CryptoProvider
    CRYPTO_PROVIDER_INSTALL_LOCK.call_once(|| {
        rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .unwrap()
    });

    // Use the same migrations as production in an isolated in-memory database.
    let db = sea_orm::Database::connect("sqlite::memory:").await?;
    aphanite::data::run_migrations(&db).await?;

    // Generate a test RSA key (2048-bit for speed)
    let private_key = PrivateKey::new(&mut rand::rng(), 2048)?;
    let rsa_pubkey = private_key.as_public_key().clone();

    let config = AppConfig {
        service: ServiceConfig {
            listen: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 3000,
            path: String::new(),
            domain: "localhost:3000".into(),
            name: "Aphanite Test".into(),
            base_url: url::Url::parse("http://localhost:3000/")?,
            data_path: tmp_dir.to_path_buf(),
            tls: aphanite::config::TlsConfig::Disabled,
            client_ip: ClientIpType::Disabled,
            public: false,
            turnstile: TurnstileSettings {
                enabled: false,
                site_key: String::new(),
                secret_key: String::new(),
            },
        },
        storage: StorageConfig::Local(LocalStorageConfig {
            path: tmp_dir.join("assets"),
        }),
        database: DatabaseConfig::Sqlite,
        yggdrasil: YggdrasilConfig {
            private_key,
            homepage: None,
            register_page: None,
        },
    };

    let http_client = std::rc::Rc::new(ntex::client::Client::new());
    let storage =
        AssetStorage::from_config(db.clone(), &config).install_client(http_client.clone());

    Ok(AppState {
        da: aphanite::data::DatabaseAccessor::new(db.clone()),
        cfg: Arc::new(config),
        assets: storage,
        kv: KVCache::new(),
        rsa_pubkey,
        http_client,
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
    let db = state.da.db().clone();

    // Return existing user if already present
    if let Ok(user) = users::Entity::find()
        .filter(users::Column::Email.eq(email))
        .one(&db)
        .await
        .and_then(|record| record.ok_or_else(|| DbErr::RecordNotFound("User not found".into())))
    {
        return user;
    }

    users::ActiveModel {
        email: Set(email.to_owned()),
        nickname: Set(email.to_owned()),
        password: Set(test_password_hash().to_owned()),
        preferred_language: Set("en_US".to_owned()),
        permission: Set((0).into()),
        ..Default::default()
    }
    .insert(&db)
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
) -> game_profiles::Model {
    let db = state.da.db().clone();
    game_profiles::ActiveModel {
        owner_id: Set(user_id),
        name: Set(name.to_owned()),
        ..Default::default()
    }
    .insert(&db)
    .await
    .expect("failed to create test profile")
}
