use crate::config::{AppConfig, DatabaseBackend};
use crate::kv_cache::KVCache;
use crate::storage::AssetStorage;
use crate::types::RegisterToken;
use clap::Parser;
use rsa::RsaPublicKey;
use std::sync::Arc;
use tracing::info;

mod cli;
mod config;
mod data;
mod kv_cache;
mod service;
mod storage;
mod types;

#[derive(Clone)]
struct AppState {
    da: data::DatabaseAccessor,
    cfg: Arc<AppConfig>,
    assets: AssetStorage,
    kv: KVCache,
    rsa_pubkey: RsaPublicKey,
    http_client: reqwest::Client,
}

#[tokio::main]
async fn main() {
    let res: anyhow::Result<()> = async {
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

        let db_url = if let DatabaseBackend::Sqlite = config.database.backend {
            let db_path = &config.service.data_path.join("db.sqlite");
            let db_path_str = db_path
                .to_str()
                .expect("FATAL: Database path is not a valid UTF-8 string!");
            format!("sqlite:{}", db_path_str)
        } else {
            config.database.postgres_url.clone()
        };

        let db = toasty::Db::builder()
            .models(toasty::models!(crate::*))
            .connect(&db_url)
            .await?;

        #[cfg(debug_assertions)]
        {
            if args.with_test_user {
                // Create a test user with fixed informations
                let mut db = db.clone();

                use argon2::{
                    Argon2,
                    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
                };
                let uuid = uuid::uuid!("11451419-1981-8011-8451-419198101145");
                let email = "test@aphanite.example.com";
                let password = b"01234567890";
                let name = "Aphanite_Test";
                let salt = SaltString::generate(&mut OsRng);
                let argon2 = Argon2::default();
                let hashed_password = argon2.hash_password(password, &salt)?.to_string();

                if types::User::get_by_id(&mut db, &uuid).await.is_err() {
                    tracing::debug!("Creating test user");
                    types::User::create()
                        .email(email)
                        .id(uuid)
                        .nickname(name)
                        .password(hashed_password)
                        .preferred_language("zh_CN")
                        .permission(1)
                        .totp_active(false)
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
        }

        if let Some(cli::Command::CreateAdmin {
            email,
            nickname,
            password,
        }) = &args.command
        {
            use argon2::password_hash::{PasswordHasher, SaltString, rand_core::OsRng};
            use types::User;

            let nickname = nickname.clone().unwrap_or_else(|| email.clone());
            let salt = SaltString::generate(&mut OsRng);
            let argon2 = argon2::Argon2::default();
            let hashed_password = argon2
                .hash_password(password.as_bytes(), &salt)
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
                .totp_active(false)
                .exec(&mut db)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create admin user: {e}"))?;

            tracing::info!("Admin user created: {email} (nickname: {nickname})");
            return Ok(());
        }

        let storage = AssetStorage::from_config(db.clone(), &config);
        let storage_router = storage.router();

        let listen = args.listen.unwrap_or(config.service.listen);
        let port = args.port.unwrap_or(config.service.port);
        let state = AppState {
            assets: storage,
            da: data::DatabaseAccessor::new(db.clone()),
            kv: KVCache::new(),
            cfg: Arc::new(config),
            rsa_pubkey,
            http_client: reqwest::Client::new(),
        };

        let scheduler_db = db.clone();
        let sched = tokio_cron_scheduler::JobScheduler::new().await?;
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
        sched.start().await?;

        use tower::ServiceBuilder;
        use tower_http::trace::TraceLayer;

        let app = service::router(state)
            .nest("/assets", storage_router)
            .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()));

        info!("Service listening on http://{}:{}", listen, port);
        if !(args.debug || args.verbose) {
            eprintln!("Service listening on http://{}:{}", listen, port);
        }

        let listener = tokio::net::TcpListener::bind((listen, port)).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
    .await;
    if let Err(e) = res {
        tracing::error!("Error occurred! Details: {}", e);
        std::process::exit(1);
    }
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
        tracing::info!("Cleaned up {count} expired registration token(s)");
    }
}
