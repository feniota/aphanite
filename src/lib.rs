use crate::config::{AppConfig, DatabaseBackend};
use crate::kv_cache::KVCache;
use crate::storage::AssetStorage;
use crate::types::RegisterToken;
use clap::Parser;
use rsa::PublicKey;
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
    pub da: data::DatabaseAccessor,
    pub cfg: Arc<AppConfig>,
    pub assets: AssetStorage,
    pub kv: KVCache,
    pub rsa_pubkey: PublicKey,
    pub http_client: reqwest::Client,
    /// Clean subdirectory prefix (e.g. "aphanite"), empty if serving at root.
    pub base_path: String,
}

async fn build_db(config: &AppConfig) -> anyhow::Result<toasty::Db> {
    Ok(
        if matches!(config.database.backend, DatabaseBackend::Turso) {
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
        } else {
            let db_url = config.database.postgres_url.clone();
            let redacted_db_url = db_url
                .split_once('@')
                .map(|(_, rest)| format!("<redacted>@{rest}"))
                .unwrap_or_else(|| db_url.clone());
            tracing::debug!("Using db at: {redacted_db_url}");
            toasty::Db::builder()
                .models(toasty::models!(crate::*))
                .connect(&db_url)
                .await?
        },
    )
}

#[cfg(debug_assertions)]
async fn debug_user(args: &cli::Args, db: &toasty::Db) -> anyhow::Result<()> {
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

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("Shutdown signal received, stopping server");
}

async fn cleanup_expired_register_tokens(db: &toasty::Db) {
    let mut db = db.clone();
    let now = jiff::Timestamp::now();
    loop {
        let oldest = match RegisterToken::all()
            .order_by(RegisterToken::fields().expires_at().asc())
            .limit(100)
            .exec(&mut db)
            .await
        {
            Ok(tokens) => tokens,
            Err(e) => {
                tracing::error!("Failed to query register tokens for cleanup: {e}");
                break;
            }
        };
        if oldest.is_empty() {
            break;
        }
        let expired: Vec<_> = oldest.into_iter().filter(|t| t.expires_at <= now).collect();
        if expired.is_empty() {
            break;
        }
        let count = expired.len();
        for token in expired {
            if let Err(e) = RegisterToken::delete_by_id(&mut db, &token.id).await {
                tracing::error!("Failed to delete expired register token {}: {e}", token.id);
            }
        }
        info!("Cleaned up {count} expired registration token(s)");
    }
}

pub async fn start() -> anyhow::Result<()> {
    let args = cli::Args::parse();
    cli::cli(&args);

    let config = AppConfig::read(&args);
    let rsa_pubkey = config.yggdrasil.private_key.as_public_key().clone();

    info!("Setting up data directory");
    if !std::fs::exists(&config.service.data_path)? {
        std::fs::create_dir(&config.service.data_path)?;
    }

    info!("Running database migrations");
    data::init(&config).await?;

    info!("Setting up ORM");
    let db = build_db(&config).await?;

    #[cfg(debug_assertions)]
    debug_user(&args, &db).await?;

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
        return Ok(());
    }

    let base_path = config.service.path_prefix().to_string();

    let storage = AssetStorage::from_config(db.clone(), &config);

    let listen = args.listen.unwrap_or(config.service.listen);
    let port = args.port.unwrap_or(config.service.port);

    let http_client;
    #[cfg(feature = "bundle-certs")]
    {
        // Embed Mozilla roots so the binary does not depend on a system CA store
        http_client = reqwest::Client::builder()
            .tls_certs_only(
                webpki_root_certs::TLS_SERVER_ROOT_CERTS
                    .iter()
                    .map(|cert| reqwest::Certificate::from_der(cert.as_ref()))
                    .collect::<Result<Vec<_>, _>>()?,
            )
            .build()?;
    }
    #[cfg(not(feature = "bundle-certs"))]
    {
        http_client = reqwest::Client::new();
    }

    let state = AppState {
        assets: storage,
        da: data::DatabaseAccessor::new(db.clone()),
        kv: KVCache::new(),
        cfg: Arc::new(config),
        rsa_pubkey,
        http_client,
        base_path,
    };

    let scheduler_db = db.clone();
    let mut sched = tokio_cron_scheduler::JobScheduler::new().await?;
    sched
        .add(tokio_cron_scheduler::Job::new_async(
            "0 0 * * * *",
            move |_uuid, _lock| {
                let db = scheduler_db.clone();
                Box::pin(async move {
                    cleanup_expired_register_tokens(&db).await;
                })
            },
        )?)
        .await?;
    use ntex::web::{self, middleware};

    let server = web::HttpServer::with_config(
        async move || Ok::<_, std::io::Error>(web::AppState::new(state.clone())),
        async |state: &web::AppState<AppState>| {
            // ntex scopes match by path prefix; an empty scope matches nothing.
            let prefix = if state.base_path.is_empty() {
                "/".to_string()
            } else {
                format!("/{}", state.base_path)
            };
            let api_location = format!("{}api/yggdrasil/", state.cfg.service.base_url());

            web::App::new()
                .middleware(middleware::Logger::default())
                .service(web::scope("/assets").configure(AssetStorage::configure))
                .service(
                    web::scope(prefix)
                        .middleware(
                            middleware::DefaultHeaders::new()
                                .header("X-Authlib-Injector-API-Location", api_location),
                        )
                        .configure(|cfg| {
                            service::configure(cfg);
                            if !state.base_path.is_empty() {
                                cfg.service(
                                    web::scope("/assets").configure(AssetStorage::configure),
                                );
                            }
                        })
                        .default_service(web::to_with_state(service::frontend::serve)),
                )
        },
    )
    .disable_signals()
    .bind((listen, port), ntex::SharedCfg::default())?;

    sched.start().await?;
    let server = server.run();

    info!("Service listening on http://{}:{}", listen, port);
    if !(args.debug || args.verbose) {
        eprintln!("Service listening on http://{}:{}", listen, port);
    }

    tokio::pin!(server);
    let server_result = tokio::select! {
        result = &mut server => result,
        _ = shutdown_signal() => {
            server.stop(true).await;
            server.await
        }
    };
    sched.shutdown().await?;
    server_result?;
    info!("Server stopped");
    Ok(())
}
