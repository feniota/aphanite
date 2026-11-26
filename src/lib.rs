use crate::cli::Args;
use crate::config::{AppConfig, DatabaseConfig, TlsConfig};
use crate::data::DatabaseAccessor;
use crate::data::entity::{game_profiles, users};
use crate::kv_cache::KVCache;
use crate::storage::AssetStorage;
use anyhow::{Context, Result};
use clap::Parser;
use ntex::web;
use ntex::web::middleware;
use std::rc::Rc;
use std::sync::Arc;
use tracing::{error, info};

use sea_orm::{ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, QueryFilter, Set};

mod cli;
pub mod config;
pub mod data;
pub mod kv_cache;
pub mod service;
pub mod storage;
pub mod types;

#[derive(Clone)]
pub struct AppState {
    pub da: DatabaseAccessor,
    pub cfg: Arc<AppConfig>,
    pub assets: AssetStorage<Rc<ntex::client::Client>>,
    pub kv: KVCache,
    pub rsa_pubkey: rsa::PublicKey,
    pub http_client: Rc<ntex::client::Client>,
}

async fn build_db(config: &AppConfig) -> Result<sea_orm::DatabaseConnection> {
    let db = match &config.database {
        DatabaseConfig::Sqlite => {
            let db_path = &config.service.data_path.join("db.sqlite");
            let db_path_str = db_path
                .to_str()
                .expect("FATAL: Database path is not a valid UTF-8 string!");
            tracing::debug!("Using db at: sqlite:{}", db_path_str);
            sqlx::SqlitePool::connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(db_path)
                    .create_if_missing(true),
            )
            .await?
            .into()
        }
        DatabaseConfig::Postgres(url) => {
            let db_url = url.clone();
            let redacted_db_url = db_url
                .split_once('@')
                .map(|(_, rest)| format!("<redacted>@{rest}"))
                .unwrap_or_else(|| db_url.clone());
            tracing::debug!("Using db at: {redacted_db_url}");
            sqlx::PgPool::connect(&db_url).await?.into()
        }
    };

    Ok(db)
}

fn build_client() -> Rc<ntex::client::Client> {
    let cfg = ntex::client::ClientConfig::default()
        .set_header(
            "User-Agent",
            concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")),
        )
        .expect("Invalid User-Agent");

    #[cfg(feature = "native-certs")]
    {
        use rustls_platform_verifier::BuilderVerifierExt;

        let rustls = rustls::ClientConfig::builder()
            .with_platform_verifier()
            .expect("Failed to initialize platform certificate verifier")
            .with_no_client_auth();

        Rc::new(ntex::client::Client::builder().rustls(rustls).build(cfg))
    }

    #[cfg(not(feature = "native-certs"))]
    {
        // Embed Mozilla roots so the binary does not depend on a system CA store
        Rc::new(ntex::client::Client::builder().build(cfg))
    }
}

fn build_tls(config: &mut AppConfig) -> Result<Option<rustls::ServerConfig>> {
    use futures_lite::StreamExt;

    Ok(match config.service.tls {
        TlsConfig::Manual(ref cert) => Some(
            rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(vec![cert.certificate.clone()], cert.private_key.clone_key())
                .context("Invalid TLS certificate or private key")?,
        ),
        TlsConfig::AcmeTlsAlpn(ref mut acme) => {
            let mut state = acme
                .take()
                .context("The ACME configuration has been taken.")?
                .state();
            let resolver = state.resolver();
            ntex::rt::spawn(async move {
                while let Some(result) = state.next().await {
                    match result {
                        Ok(i) => info!("ACME: {i:#?}"),
                        Err(e) => error!("ACME: {e}"),
                    }
                }
            });
            Some(
                rustls::ServerConfig::builder()
                    .with_no_client_auth()
                    .with_cert_resolver(resolver),
            )
        }
        TlsConfig::Disabled | TlsConfig::External => None,
    })
}

async fn create_admin(args: &Args, db: &sea_orm::DatabaseConnection) -> Result<bool> {
    if let Some(cli::Command::CreateAdmin {
        email,
        nickname,
        password,
    }) = &args.command
    {
        use argon2::password_hash::PasswordHasher;

        let nickname = nickname.clone().unwrap_or_else(|| email.clone());
        let argon2 = argon2::Argon2::default();
        let hashed_password = argon2
            .hash_password(password.as_bytes())
            .map_err(|e| anyhow::anyhow!("Password hashing failed: {e}"))?
            .to_string();

        let db = db.clone();
        if users::Entity::find()
            .filter(users::Column::Email.eq(email))
            .one(&db)
            .await
            .and_then(|record| record.ok_or_else(|| DbErr::RecordNotFound("User not found".into())))
            .is_ok()
        {
            anyhow::bail!("A user with email '{email}' already exists");
        }

        users::ActiveModel {
            email: Set(email.to_owned()),
            nickname: Set(nickname.to_owned()),
            password: Set(hashed_password.to_owned()),
            preferred_language: Set("zh_CN".to_owned()),
            permission: Set(1.into()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create admin user: {e}"))?;

        info!("Admin user created: {email} (nickname: {nickname})");
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(debug_assertions)]
async fn debug_user(args: &Args, db: &sea_orm::DatabaseConnection) -> Result<()> {
    if args.with_test_user {
        // Create a test user with fixed informations
        let db = db.clone();

        use argon2::{Argon2, password_hash::PasswordHasher};
        let uuid = uuid::uuid!("11451419-1981-8011-8451-419198101145");
        let email = "test@aphanite.example.com";
        let password = b"01234567890";
        let name = "Aphanite_Test";
        let argon2 = Argon2::default();
        let hashed_password = argon2.hash_password(password)?.to_string();

        if users::Entity::find_by_id(uuid)
            .one(&db)
            .await
            .and_then(|record| record.ok_or_else(|| DbErr::RecordNotFound("User not found".into())))
            .is_err()
        {
            tracing::debug!("Creating test user");
            users::ActiveModel {
                email: Set(email.to_owned()),
                id: Set(uuid),
                nickname: Set(name.to_owned()),
                password: Set(hashed_password.to_owned()),
                preferred_language: Set("zh_CN".to_owned()),
                permission: Set(1.into()),
                ..Default::default()
            }
            .insert(&db)
            .await?;

            game_profiles::ActiveModel {
                name: Set(name.to_owned()),
                owner_id: Set(uuid),
                ..Default::default()
            }
            .insert(&db)
            .await?;
            tracing::warn!("Test user created!");
            tracing::warn!(
                "Its email: {}, password: \"{}\" and it has a profile named \"{}\"",
                email,
                "01234567890",
                name
            );
        }
    }

    Ok(())
}

pub async fn start() -> Result<()> {
    // Load CryptoProvider
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install CryptoProvider");

    let args = Args::parse();
    cli::cli(&args);

    let mut config = AppConfig::read(&args);

    info!("Setting up data directory");
    if !std::fs::exists(&config.service.data_path)? {
        compio::fs::create_dir(&config.service.data_path).await?;
    }

    info!("Setting up ORM");
    let db = build_db(&config).await?;

    info!("Running database migrations");
    data::run_migrations(&db).await?;

    #[cfg(debug_assertions)]
    debug_user(&args, &db).await?;

    if create_admin(&args, &db).await? {
        return Ok(());
    };

    // Load TLS certificate
    let tls = build_tls(&mut config)?;

    // Build Application State
    let listen = args.listen.unwrap_or(config.service.listen);
    let port = args.port.unwrap_or(config.service.port);
    let storage = AssetStorage::from_config(db.clone(), &config);
    let da = DatabaseAccessor::new(db);
    let kv = KVCache::new();
    let rsa_pubkey = config.yggdrasil.private_key.as_public_key().clone();
    let cfg = Arc::new(config);

    // Build Server
    let server = web::HttpServer::with_config(
        async move || {
            let http_client = build_client();
            Ok::<_, std::io::Error>(web::AppState::new(AppState {
                assets: storage.install_client(http_client.clone()),
                da: da.clone(),
                kv: kv.clone(),
                cfg: cfg.clone(),
                rsa_pubkey: rsa_pubkey.clone(),
                http_client,
            }))
        },
        async |state: &web::AppState<AppState>| {
            // ntex scopes match by path prefix; an empty scope matches nothing.
            let prefix = if state.cfg.service.path.is_empty() {
                "/".to_string()
            } else {
                format!("/{}", state.cfg.service.path)
            };
            let api_location = format!("{}api/yggdrasil/", state.cfg.service.base_url);

            web::App::new()
                .middleware(middleware::Logger::default())
                .middleware(middleware::from_fn(async |req, next| next.call(req).await))
                .service(web::scope("/assets").configure(AssetStorage::configure))
                .service(
                    web::scope(prefix)
                        .middleware(
                            middleware::DefaultHeaders::new()
                                .header("X-Authlib-Injector-API-Location", api_location),
                        )
                        .configure(|cfg| {
                            service::configure(cfg);
                            if !state.cfg.service.path.is_empty() {
                                cfg.service(
                                    web::scope("/assets").configure(AssetStorage::configure),
                                );
                            }
                        })
                        .default_service(web::to_with_state(service::frontend::serve)),
                )
        },
    )
    .graceful_shutdown();

    // Start server
    let scheme = if tls.is_some() { "https" } else { "http" };
    let server = match tls {
        None => server.bind((listen, port), ntex::SharedCfg::default()),
        Some(tls) => server.bind_rustls((listen, port), &tls, ntex::SharedCfg::default()),
    }?
    .run();
    info!("Service listening on {}://{}:{}", scheme, listen, port);
    if !(args.debug || args.verbose) {
        eprintln!("Service listening on {}://{}:{}", scheme, listen, port);
    }

    server.await?;
    info!("Server stopped");
    Ok(())
}
