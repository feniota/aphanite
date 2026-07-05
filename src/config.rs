//! Configuration TOML file parsing and validating

use rsa::{
    RsaPrivateKey,
    pkcs8::{EncodePrivateKey, LineEnding},
};
use serde::{Deserialize, Serialize};
use std::{
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
};

#[derive(Serialize, Deserialize)]
pub struct AppConfig {
    pub service: ServiceConfig,
    pub storage: StorageConfig,
    pub database: DatabaseConfig,
    pub yggdrasil: YggdrasilConfig,
}

static EXAMPLE_CONFIG: &'static str = include_str!("./assets/config.example.toml");

impl AppConfig {
    /// Parse the TOML configuration file at the given path
    /// Panics if the file does not satisfy requirements
    pub fn read(args: &crate::cli::Args) -> Self {
        let file = std::fs::read_to_string(&args.config);
        let config_str = match file {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!(
                    "Configuration file not found, generating one using default parameters..."
                );
                tracing::warn!("A NEW RSA private key is being generated!");
                tracing::warn!("If you want to persist configuration, run `aphanite init`.");
                Self::generate(args).unwrap()
            }
            Err(e) => {
                tracing::error!("Failed to open the configuration file: {}", e);
                std::process::exit(1);
            }
            Ok(f) => f,
        };
        let conf = toml::from_str(&config_str);
        if let Err(e) = conf {
            tracing::debug!("{:?}", e);
            tracing::error!(
                "Failed to parse configuration file (set RUST_LOG to debug for details): {}",
                e.message()
            );
            std::process::exit(1);
        }
        let conf: Self = conf.unwrap();
        if !conf.service.tls {
            tracing::warn!(
                "`tls=false` detected! This should only be used in testing and development. Minecraft would NOT trust a server without TLS!"
            );
        }
        conf
    }

    /// Get the bundled "default" configuration file
    pub fn generate(args: &crate::cli::Args) -> anyhow::Result<String> {
        let replaced = EXAMPLE_CONFIG
            .replace("{APHANITE_VERSION}", env!("CARGO_PKG_VERSION"))
            .replace(
                "{APHANITE_CONFIG_LISTEN}",
                &args
                    .listen
                    .unwrap_or(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))
                    .to_string(),
            )
            .replace(
                "{APHANITE_CONFIG_PORT}",
                &args.port.unwrap_or(3000).to_string(),
            )
            .replace(
                "{APHANITE_CONFIG_PRIVATE_KEY}",
                &RsaPrivateKey::new(&mut rand::rng(), 4096)?.to_pkcs8_pem(LineEnding::default())?,
            );

        Ok(replaced)
    }
    pub fn init(args: &crate::cli::Args) -> anyhow::Result<()> {
        let c = Self::generate(args)?;
        if let Some(parent) = args.config.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&args.config, c)?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct ServiceConfig {
    pub listen: IpAddr,
    pub port: u16,
    pub path: Option<String>,
    pub domain: String,
    pub data_path: PathBuf,
    pub tls: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageType {
    Local,
    S3,
}
#[derive(Serialize, Deserialize)]
pub struct StorageConfig {
    #[serde(rename = "type")]
    pub storage_type: StorageType,
    pub local: LocalStorageConfig,
    pub s3: S3StorageConfig,
}

#[derive(Serialize, Deserialize)]
pub struct LocalStorageConfig {
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize)]
pub struct S3StorageConfig {
    pub bucket_name: String,
    #[serde(rename = "endpoint")]
    pub endpoint: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    pub domains: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseBackend {
    Sqlite,
}

#[derive(Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub backend: DatabaseBackend,
}

#[derive(Serialize, Deserialize)]
pub struct YggdrasilConfig {
    #[serde(deserialize_with = "crate::config::deserialize_rsa_privkey_from_pkcs8_pem")]
    pub private_key: RsaPrivateKey,

    pub server_name: Option<String>,
    pub homepage: Option<String>,
    pub register_page: Option<String>,
}
fn deserialize_rsa_privkey_from_pkcs8_pem<'de, D>(
    deserializer: D,
) -> Result<RsaPrivateKey, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use rsa::pkcs8::DecodePrivateKey;
    use serde::de::Error;
    use serde::de::Expected;
    use serde::de::Unexpected;
    use std::fmt;
    struct ExpectPkcs8PemPrivKey;

    impl Expected for ExpectPkcs8PemPrivKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a PKCS#8 PEM-encoded RSA private key string")
        }
    }
    let s = String::deserialize(deserializer)?;
    RsaPrivateKey::from_pkcs8_pem(&s)
        .map_err(|_e| D::Error::invalid_value(Unexpected::Str(&s), &ExpectPkcs8PemPrivKey))
}
