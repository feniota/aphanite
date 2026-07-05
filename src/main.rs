use crate::config::AppConfig;
use crate::kv_cache::KVCache;
use crate::storage::AssetsStorage;
use clap::Parser;
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
}

#[tokio::main]
async fn main() {
    let res: anyhow::Result<()> = async {
        let args = cli::Args::parse();
        cli::cli(&args);

        info!("Setting up data directory");
        if !std::fs::exists(&args.data)? {
            std::fs::create_dir(&args.data)?;
        }

        info!("Setting up database");
        let db_path = args.data.join("db.sqlite");
        let db_path_str = db_path
            .to_str()
            .expect("FATAL: Database path is not a valid UTF-8 string!");

        // Build a Db handle, registering all models in this crate
        let db = toasty::Db::builder()
            .models(toasty::models!(crate::*))
            .connect(&format!("sqlite:{}", db_path_str))
            .await?;
        let state = AppState {
            cfg: Default::default(),
            assets: AssetsStorage::new(
                db.clone(),
                storage::StorageConfiguration::Local(storage::LocalStorageConfiguration {
                    path: args.data.clone().join("assets"),
                }),
            ),
            da: data::DatabaseAccessor::new(db.clone()),
            kv: KVCache::new(),
        };
        let app = service::router(state);

        info!("Service listening on http://{}:{}", args.listen, args.port);
        eprintln!("Service listening on http://{}:{}", args.listen, args.port);

        let listener = tokio::net::TcpListener::bind((args.listen, args.port)).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
    .await;
    if let Err(e) = res {
        tracing::error!("Error occurred! Details: {}", e);
        std::process::exit(1);
    }
}
