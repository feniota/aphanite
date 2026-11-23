use crate::cli::Args;
use crate::config::{AppConfig, DatabaseConfig};
use crate::data::DatabaseAccessor;
use crate::kv_cache::KVCache;
use crate::storage::AssetStorage;
use async_compat::CompatExt;
use clap::Parser;
use ntex::web;
use ntex::web::middleware;
use std::rc::Rc;
use std::sync::Arc;
use tracing::info;

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

async fn build_db(config: &AppConfig) -> anyhow::Result<toasty::Db> {
    Ok(match &config.database {
        DatabaseConfig::Turso => {
            let db_path = &config.service.data_path.join("db.sqlite");
            let db_path_str = db_path
                .to_str()
                .expect("FATAL: Database path is not a valid UTF-8 string!");
            tracing::debug!("Using db at: turso:{}", db_path_str);
            let driver = toasty_driver_turso::Turso::file(db_path_str)
                .concurrent_writes()
                .optimize_io();
            toasty::Db::builder()
                .models(toasty::models!(crate::*))
                .build(driver)
                .await?
        }
        DatabaseConfig::Postgres(url) => {
            let db_url = url.clone();
            let redacted_db_url = db_url
                .split_once('@')
                .map(|(_, rest)| format!("<redacted>@{rest}"))
                .unwrap_or_else(|| db_url.clone());
            tracing::debug!("Using db at: {redacted_db_url}");
            toasty::Db::builder()
                .models(toasty::models!(crate::*))
                .connect(&db_url)
                .await?
        }
    })
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

async fn create_admin(args: &Args, db: &toasty::Db) -> anyhow::Result<bool> {
    if let Some(cli::Command::CreateAdmin {
        email,
        nickname,
        password,
    }) = &args.command
    {
        use argon2::password_hash::PasswordHasher;
        use types::User;

        let nickname = nickname.clone().unwrap_or_else(|| email.clone());
        let argon2 = argon2::Argon2::default();
        let hashed_password = argon2
            .hash_password(password.as_bytes())
            .map_err(|e| anyhow::anyhow!("Password hashing failed: {e}"))?
            .to_string();

        let mut db = db.clone();
        if User::get_by_email(&mut db, email).await.is_ok() {
            anyhow::bail!("A user with email '{email}' already exists");
        }

        User::create()
            .email(email)
            .nickname(&nickname)
            .password(&hashed_password)
            .preferred_language("zh_CN")
            .permission(1)
            .exec(&mut db)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create admin user: {e}"))?;

        info!("Admin user created: {email} (nickname: {nickname})");
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(debug_assertions)]
async fn debug_user(args: &Args, db: &toasty::Db) -> anyhow::Result<()> {
    if args.with_test_user {
        // Create a test user with fixed informations
        let mut db = db.clone();

        use argon2::{Argon2, password_hash::PasswordHasher};
        let uuid = uuid::uuid!("11451419-1981-8011-8451-419198101145");
        let email = "test@aphanite.example.com";
        let password = b"01234567890";
        let name = "Aphanite_Test";
        let argon2 = Argon2::default();
        let hashed_password = argon2.hash_password(password)?.to_string();

        if types::User::get_by_id(&mut db, &uuid).await.is_err() {
            tracing::debug!("Creating test user");
            types::User::create()
                .email(email)
                .id(uuid)
                .nickname(name)
                .password(hashed_password)
                .preferred_language("zh_CN")
                .permission(1)
                .exec(&mut db)
                .await?;

            service::yggdrasil::types::GameProfile::create()
                .name(name)
                .owner_id(uuid)
                .exec(&mut db)
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

pub async fn start() -> anyhow::Result<()> {
    // Load CryptoProvider
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install CryptoProvider");

    let args = Args::parse();
    cli::cli(&args);

    let config = AppConfig::read(&args);
    let rsa_pubkey = config.yggdrasil.private_key.as_public_key().clone();

    info!("Setting up data directory");
    if !std::fs::exists(&config.service.data_path)? {
        compio::fs::create_dir(&config.service.data_path).await?;
    }

    info!("Running database migrations");
    data::init(&config).await?;

    info!("Setting up ORM");
    let db = build_db(&config).compat().await?;

    #[cfg(debug_assertions)]
    debug_user(&args, &db).await?;

    if create_admin(&args, &db).await? {
        return Ok(());
    };

    // Build Application State
    let storage = AssetStorage::from_config(db.clone(), &config);
    let listen = args.listen.unwrap_or(config.service.listen);
    let port = args.port.unwrap_or(config.service.port);
    let da = DatabaseAccessor::new(db.clone());
    let kv = KVCache::new();
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
                .middleware(middleware::from_fn(async |req, next| {
                    next.call(req).compat().await
                }))
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
    .bind((listen, port), ntex::SharedCfg::default())?;

    // Start server
    let server = server.run();
    info!("Service listening on http://{}:{}", listen, port);
    if !(args.debug || args.verbose) {
        eprintln!("Service listening on http://{}:{}", listen, port);
    }

    server.await?;
    info!("Server stopped");
    Ok(())
}
