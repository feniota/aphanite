use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub api: ApiConfig,
    pub database: DatabaseConfig,
    pub yggdrasil: YggdrasilConfig,
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
            domain: Default::default(),
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
