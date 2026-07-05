use crate::config::AppConfig;
use crate::kv_cache::KVCache;
use crate::storage::AssetsStorage;
use clap::Parser;
use rsa::RsaPublicKey;
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

        let storage = AssetsStorage::from_config(db.clone(), &config);

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
        let app = service::router(state).nest("/assets", storage_router);

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
