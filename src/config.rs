//! Configuration TOML file parsing and validating

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub api: ApiConfig,
    pub database: DatabaseConfig,
    pub yggdrasil: YggdrasilConfig,
}

impl AppConfig {
    /// Parse the TOML configuration file at the given path
    /// Panics if the file does not satisfy requirements
    pub fn read(path: PathBuf) -> Self {
        Default::default()
    }

    /// Write the bundled "default" configuration file to the given path
    pub fn generate(path: PathBuf) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct ApiConfig {
    pub listen: SocketAddr,
    pub path: Option<String>,
    pub domain: String,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            listen: "127.0.0.1:3000".parse().unwrap(),
            path: Default::default(),
            domain: String::from("example.com"),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct DatabaseConfig;

#[derive(Serialize, Deserialize, Default)]
pub struct YggdrasilConfig {
    pub public_key: String,
    pub private_key: String,

    pub server_name: Option<String>,
    pub homepage: Option<String>,
    pub register: Option<String>,
}
