//! Configuration TOML file parsing and validating

use crate::config::parser::RawConfig;
pub use parser::ReverseProxyType;
use rsa::PrivateKey;
use std::{net::IpAddr, path::PathBuf};

pub struct AppConfig {
    pub service: ServiceConfig,
    pub storage: StorageConfig,
    pub database: DatabaseConfig,
    pub yggdrasil: YggdrasilConfig,
}

impl AppConfig {
    /// Parse the TOML configuration file specified by the given cmdline argument
    ///
    /// Exits with status 1 if the file cannot be read, parsed, or validated.
    pub fn read(args: &crate::cli::Args) -> Self {
        let file = std::fs::read_to_string(&args.config);
        let config_str = match file {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if std::env::var("APHANITE_AUTO_INIT").is_ok() {
                    tracing::info!(
                        "APHANITE_AUTO_INIT detected, initializing configuration file..."
                    );
                    Self::init(args).expect("Initializing configuration file failed!");
                    return Self::read(args);
                }

                tracing::warn!(
                    "Configuration file not found, generating one using default parameters..."
                );
                tracing::warn!("A NEW RSA private key is being generated!");
                tracing::warn!("If you want to persist configuration, run `aphanite init`.");
                RawConfig::generate(args)
            }
            Err(e) => {
                tracing::error!("Failed to open the configuration file: {}", e);
                std::process::exit(1);
            }
            Ok(f) => f,
        };
        let raw_conf = match toml::from_str::<RawConfig>(&config_str) {
            Ok(conf) => conf,
            Err(e) => {
                tracing::debug!("{:?}", e);
                tracing::error!(
                    "Failed to parse configuration file (set `RUST_LOG=debug` for more details): {}",
                    e.message()
                );
                std::process::exit(1);
            }
        };
        let conf = match Self::try_from(raw_conf) {
            Ok(conf) => conf,
            Err(e) => {
                tracing::error!("Invalid configuration: {:#}", e);
                std::process::exit(1);
            }
        };
        if !conf.service.tls {
            tracing::warn!(
                "`tls=false` detected! This should only be used in testing and development. Minecraft would NOT trust a server without TLS!"
            );
        }
        conf
    }

    pub fn init(args: &crate::cli::Args) -> anyhow::Result<()> {
        let c = RawConfig::generate(args);
        if let Some(parent) = args.config.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&args.config, c)?;
        Ok(())
    }
}

pub struct ServiceConfig {
    pub listen: IpAddr,
    pub port: u16,
    pub path: String,
    pub domain: String,
    pub data_path: PathBuf,
    pub tls: bool,
    pub client_ip: ReverseProxyType,
    pub public: bool,
    pub turnstile: TurnstileSettings,

    pub base_url: url::Url,
}

pub struct TurnstileSettings {
    pub enabled: bool,
    pub site_key: String,
    pub secret_key: String,
}

pub enum StorageConfig {
    Local(LocalStorageConfig),
    S3(S3StorageConfig),
}

pub struct LocalStorageConfig {
    pub path: PathBuf,
}

pub struct S3StorageConfig {
    pub bucket_name: String,
    pub endpoint: url::Url,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    pub domains: Vec<String>,
}

pub enum DatabaseConfig {
    Turso,
    Postgres(String),
}

pub struct YggdrasilConfig {
    pub private_key: PrivateKey,
    pub server_name: Option<String>,
    pub homepage: Option<String>,
    pub register_page: Option<String>,
}

mod parser {
    const EXAMPLE_CONFIG: &str = include_str!("./assets/config.example.toml");

    use crate::config::AppConfig;
    use anyhow::Context;
    use rsa::PrivateKey;
    use rsa::pkcs1::LineEnding;
    use rsa::pkcs8::EncodePrivateKey;
    use serde::{Deserialize, Serialize};
    use std::net::{IpAddr, Ipv4Addr};
    use std::path::PathBuf;

    #[derive(Serialize, Deserialize)]
    pub(super) struct RawConfig {
        pub service: ServiceConfig,
        #[serde(default)]
        pub storage: StorageConfig,
        #[serde(default)]
        pub database: DatabaseConfig,
        pub yggdrasil: YggdrasilConfig,
    }

    fn default_data_dir() -> PathBuf {
        PathBuf::from("./data")
    }

    #[derive(Serialize, Deserialize)]
    pub(super) struct ServiceConfig {
        pub listen: IpAddr,
        pub port: u16,
        pub path: Option<String>,
        pub domain: String,
        #[serde(default = "default_data_dir")]
        pub data_path: PathBuf,
        #[serde(default)]
        pub tls: bool,
        #[serde(default)]
        pub client_ip: ReverseProxyType,
        #[serde(default)]
        pub public: bool,
        #[serde(default)]
        pub turnstile: TurnstileSettings,
    }

    impl ServiceConfig {
        /// Get the clean subdirectory prefix (no leading/trailing slash), or empty string if none.
        pub fn path_prefix(&self) -> &str {
            const EMPTY: &str = "";
            self.path
                .as_deref()
                .map(|p| p.trim_start_matches('/').trim_end_matches('/'))
                .unwrap_or(EMPTY)
        }

        /// Get the full base URL (scheme + domain + optional subpath).
        ///
        /// Example: `"https://example.com/aphanite/"`
        pub fn base_url(&self) -> String {
            let prefix = self.path_prefix();
            let base = format!(
                "{}://{}{}",
                if self.tls { "https" } else { "http" },
                self.domain,
                if prefix.is_empty() {
                    String::new()
                } else {
                    format!("/{}", prefix)
                }
            );
            if base.ends_with('/') {
                base
            } else {
                format!("{}/", base)
            }
        }
    }

    /// Reverse-proxy header used to determine the client IP.
    #[derive(Serialize, Deserialize, Default)]
    pub enum ReverseProxyType {
        #[serde(rename = "CF-Connecting-IP")]
        CfConnectingIp,

        #[serde(rename = "CloudFront-Viewer-Address")]
        CloudFrontViewerAddress,

        #[serde(rename = "Fly-Client-IP")]
        FlyClientIp,

        Forwarded,

        #[serde(rename = "X-Forwarded-For")]
        XForwardedFor,

        #[serde(rename = "True-Client-IP")]
        TrueClientIp,

        #[serde(rename = "X-Envoy-External-Address")]
        XEnvoyExternalAddress,

        #[serde(rename = "X-Real-Ip")]
        XRealIp,

        /// Disables IP address examine completely
        #[default]
        #[serde(rename = "disabled")]
        Disabled,
    }

    impl ReverseProxyType {
        pub fn is_disabled(&self) -> bool {
            matches!(self, Self::Disabled)
        }
    }

    #[derive(Serialize, Deserialize, Default)]
    pub(super) struct TurnstileSettings {
        #[serde(default)]
        pub enabled: bool,
        #[serde(default)]
        pub site_key: String,
        #[serde(default)]
        pub secret_key: String,
    }

    #[derive(Serialize, Deserialize, strum::Display, Default)]
    #[serde(rename_all = "lowercase")]
    pub(super) enum StorageType {
        #[default]
        #[strum(serialize = "local")]
        Local,
        #[strum(serialize = "s3")]
        S3,
    }
    #[derive(Serialize, Deserialize, Default)]
    pub(super) struct StorageConfig {
        #[serde(rename = "type")]
        #[serde(default)]
        pub storage_type: StorageType,
        pub local: Option<LocalStorageConfig>,
        pub s3: Option<S3StorageConfig>,
    }

    fn default_storage_dir() -> PathBuf {
        PathBuf::from("./data/assets")
    }
    #[derive(Serialize, Deserialize, Default)]
    pub(super) struct LocalStorageConfig {
        #[serde(default = "default_storage_dir")]
        pub path: PathBuf,
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub(super) struct S3StorageConfig {
        pub bucket_name: String,
        pub endpoint: url::Url,
        pub region: String,
        pub access_key: String,
        pub secret_key: String,
        pub domains: Vec<String>,
    }

    #[derive(Serialize, Deserialize, Default)]
    #[serde(rename_all = "lowercase")]
    pub(super) enum DatabaseBackend {
        #[default]
        Turso,
        Postgres,
    }

    #[derive(Serialize, Deserialize, Default)]
    pub(super) struct DatabaseConfig {
        #[serde(default)]
        pub backend: DatabaseBackend,
        pub postgres_url: Option<String>,
    }

    #[derive(Serialize, Deserialize)]
    pub(super) struct YggdrasilConfig {
        #[serde(deserialize_with = "deserialize_rsa_privkey_from_pkcs8_pem")]
        pub private_key: PrivateKey,

        pub server_name: Option<String>,
        pub homepage: Option<String>,
        pub register_page: Option<String>,
    }
    fn deserialize_rsa_privkey_from_pkcs8_pem<'de, D>(
        deserializer: D,
    ) -> Result<PrivateKey, D::Error>
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
        PrivateKey::from_pkcs8_pem(&s)
            .map_err(|_e| D::Error::invalid_value(Unexpected::Str(&s), &ExpectPkcs8PemPrivKey))
    }

    impl RawConfig {
        /// Get the bundled "default" configuration file
        pub(super) fn generate(args: &crate::cli::Args) -> String {
            #[cfg_attr(not(debug_assertions), allow(unused_mut))]
            let mut replaced = EXAMPLE_CONFIG
                .replace("{APHANITE_VERSION}", env!("CARGO_PKG_VERSION"))
                .replace(
                    "{APHANITE_CONFIG_LISTEN}",
                    &args
                        .listen
                        .unwrap_or(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))
                        .to_string(),
                )
                .replace(
                    "\"{APHANITE_CONFIG_PORT}\"",
                    &args.port.unwrap_or(3000).to_string(),
                )
                .replace(
                    "{APHANITE_CONFIG_PRIVATE_KEY}",
                    &PrivateKey::new(&mut rand::rng(), 4096)
                        .expect("Failed to generate private key")
                        .to_pkcs8_pem(LineEnding::default())
                        .expect("Failed to generate private key"),
                )
                .replace(
                    "\"{APHANITE_CONFIG_TLS_ENABLED}\"",
                    &(!cfg!(debug_assertions)).to_string(),
                );

            #[cfg(debug_assertions)]
            {
                replaced = replaced
                    .replace(
                        "client_ip = \"X-Forwarded-For\"",
                        "client_ip = \"disabled\"",
                    )
                    .replace(
                        r#"domain = "aphanite.example.com""#,
                        &format!(
                            "domain = \"{}:{}\"",
                            args.listen
                                .unwrap_or(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
                            args.port.unwrap_or(3000)
                        ),
                    );
            }

            replaced
        }
    }

    impl TryFrom<RawConfig> for AppConfig {
        type Error = anyhow::Error;

        fn try_from(value: RawConfig) -> Result<Self, Self::Error> {
            let base_url = url::Url::parse(&value.service.base_url())
                .context("Failed to parse URL generated from service.domain and service.path")?;
            let path = value.service.path_prefix().to_owned();

            let storage = match value.storage.storage_type {
                StorageType::Local => {
                    let local = value
                        .storage
                        .local
                        .context("Storage type is set to local, but [storage.local] is missing")?;
                    crate::config::StorageConfig::Local(crate::config::LocalStorageConfig {
                        path: local.path,
                    })
                }
                StorageType::S3 => {
                    let s3 = value
                        .storage
                        .s3
                        .context("Storage type is set to s3, but [storage.s3] is missing")?;
                    crate::config::StorageConfig::S3(crate::config::S3StorageConfig {
                        bucket_name: s3.bucket_name,
                        endpoint: s3.endpoint,
                        region: s3.region,
                        access_key: s3.access_key,
                        secret_key: s3.secret_key,
                        domains: s3.domains,
                    })
                }
            };
            let database = match value.database.backend {
                DatabaseBackend::Turso => crate::config::DatabaseConfig::Turso,
                DatabaseBackend::Postgres => crate::config::DatabaseConfig::Postgres(
                    value
                        .database
                        .postgres_url
                        .filter(|url| !url.trim().is_empty())
                        .context(
                            "Database backend is set to postgres, but database.postgres_url is missing or empty",
                        )?,
                ),
            };

            Ok(Self {
                service: crate::config::ServiceConfig {
                    listen: value.service.listen,
                    port: value.service.port,
                    path,
                    domain: value.service.domain,
                    data_path: value.service.data_path,
                    tls: value.service.tls,
                    client_ip: value.service.client_ip,
                    public: value.service.public,
                    turnstile: crate::config::TurnstileSettings {
                        enabled: value.service.turnstile.enabled,
                        site_key: value.service.turnstile.site_key,
                        secret_key: value.service.turnstile.secret_key,
                    },
                    base_url,
                },
                storage,
                database,
                yggdrasil: crate::config::YggdrasilConfig {
                    private_key: value.yggdrasil.private_key,
                    server_name: value.yggdrasil.server_name,
                    homepage: value.yggdrasil.homepage,
                    register_page: value.yggdrasil.register_page,
                },
            })
        }
    }
}
