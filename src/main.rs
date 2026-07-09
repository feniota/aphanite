use crate::config::AppConfig;
use crate::kv_cache::KVCache;
use crate::storage::AssetStorage;
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
mod utils;

#[derive(Clone)]
struct AppState {
    da: data::DatabaseAccessor,
    cfg: Arc<AppConfig>,
    assets: AssetStorage,
    kv: KVCache,
    rsa_pubkey: RsaPublicKey,
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

        info!("Setting up database");
        let db_path = &config.service.data_path.join("db.sqlite");
        let db_path_str = db_path
            .to_str()
            .expect("FATAL: Database path is not a valid UTF-8 string!");

        let db = toasty::Db::builder()
            .models(toasty::models!(crate::*))
            .connect(&format!("sqlite:{}", db_path_str))
            .await?;
        // !! This pushes the full schema on every run, which means that this function does NOT care about existing data.
        // Change this before releasing
        let _ = db.push_schema().await;

        #[cfg(debug_assertions)]
        {
            if args.with_test_user {
                // Create a test user with fixed informations
                let mut db = db.clone();

                use argon2::{
                    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
                    Argon2,
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

        let storage = AssetStorage::from_config(db.clone(), &config);
        let storage_router = storage.router();

        let listen = args.listen.unwrap_or(config.service.listen.clone());
        let port = args.port.unwrap_or(config.service.port.clone());
        let state = AppState {
            assets: storage,
            da: data::DatabaseAccessor::new(db.clone()),
            kv: KVCache::new(),
            cfg: Arc::new(config),
            rsa_pubkey,
        };

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
