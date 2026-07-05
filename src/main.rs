use crate::config::AppConfig;
use crate::kv_cache::KVCache;
use crate::storage::AssetsStorage;
use clap::Parser;
use rsa::RsaPublicKey;
use std::net::IpAddr;
use std::sync::Arc;
use tracing::info;

mod cli;
mod config;
mod data;
pub mod kv_cache;
mod service;
mod storage;
mod types;

pub use types::{Error, Result};

#[derive(Clone)]
struct AppState {
    da: data::DatabaseAccessor,
    cfg: Arc<AppConfig>,
    assets: AssetsStorage,
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

        let actual_listen = if args.listen == IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))
            && config.service.listen != IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))
        {
            config.service.listen.clone()
        } else {
            args.listen.clone()
        };
        let actual_port = if args.port == 3000 && config.service.port != 3000 {
            config.service.port
        } else {
            args.port
        };

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

        let storage = AssetsStorage::new(
            db.clone(),
            storage::StorageConfiguration::Local(storage::LocalStorageConfiguration {
                path: config.storage.local.path.clone(),
            }),
            config.service.data_path.join("tmp"),
        );

        let storage_router = storage.router();

        let state = AppState {
            assets: storage,
            da: data::DatabaseAccessor::new(db.clone()),
            kv: KVCache::new(),
            cfg: Arc::new(config),
            rsa_pubkey,
        };
        let mut app = service::router(state);
        if let Some(router) = storage_router {
            app = app.nest("/assets", router);
        }

        info!(
            "Service listening on http://{}:{}",
            actual_listen, actual_port
        );
        if !(args.debug || args.verbose) {
            eprintln!(
                "Service listening on http://{}:{}",
                actual_listen, actual_port
            );
        }
        let salt = argon2::password_hash::SaltString::generate(
            &mut argon2::password_hash::rand_core::OsRng,
        );
        let argon2 = argon2::Argon2::default();

        use argon2::PasswordHasher;
        let hash = argon2.hash_password(b"123", &salt).unwrap();
        // crate::types::User::create()
        //     .password(hash.to_string())
        //     .email("Tuxium")
        //     .prefer_language("zh_CN")
        //     .exec(&mut db.clone())
        //     .await
        //     .unwrap();
        let listener = tokio::net::TcpListener::bind((actual_listen, actual_port)).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
    .await;
    if let Err(e) = res {
        tracing::error!("Error occurred! Details: {}", e);
        std::process::exit(1);
    }
}
