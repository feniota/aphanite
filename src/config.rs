//! Configuration TOML file parsing and validating

use crate::config::parser::RawConfig;
pub use parser::ClientIpType;
use rsa::PrivateKey;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::{net::IpAddr, path::PathBuf};

pub struct AppConfig {
    pub service: ServiceConfig,
    pub storage: StorageConfig,
    pub database: DatabaseConfig,
    pub yggdrasil: YggdrasilConfig,
}

pub struct ServiceConfig {
    pub listen: IpAddr,
    pub port: u16,
    pub path: String,
    pub domain: String,
    pub name: String,
    pub data_path: PathBuf,
    pub client_ip: ClientIpType,
    pub public: bool,
    pub turnstile: TurnstileSettings,
    pub tls: TlsConfig,

    pub base_url: url::Url,
}

pub struct TlsCertificate {
    pub certificate: CertificateDer<'static>,
    pub private_key: PrivateKeyDer<'static>,
}

pub enum TlsConfig {
    Disabled,
    External,
    Manual(TlsCertificate),
    AcmeTlsAlpn(Option<rustls_acme::AcmeConfig<std::io::Error>>),
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
    Sqlite,
    Postgres(String),
}

pub struct YggdrasilConfig {
    pub private_key: PrivateKey,
    pub homepage: Option<String>,
    pub register_page: Option<String>,
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
                RawConfig::generate(args.listen, args.port)
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
                    concat!(
                        "Failed to parse configuration file ",
                        "(set `RUST_LOG=debug` for more details): {}"
                    ),
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
        if matches!(conf.service.tls, TlsConfig::Disabled) {
            tracing::warn!(concat!(
                "`tls=disabled` detected! This should only be used in testing and development. ",
                "Minecraft would NOT trust a server without TLS!"
            ));
        }
        conf
    }

    pub fn init(args: &crate::cli::Args) -> anyhow::Result<()> {
        let c = RawConfig::generate(args.listen, args.port);
        if let Some(parent) = args.config.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&args.config, c)?;
        Ok(())
    }
}

mod parser {
    const EXAMPLE_CONFIG: &str = include_str!("./assets/config.example.toml");

    use anyhow::Context;
    use rsa::PrivateKey;
    use rsa::pkcs1::LineEnding;
    use rsa::pkcs8::EncodePrivateKey;
    use rustls::pki_types::{
        CertificateDer, PrivateKeyDer,
        pem::{PemObject, SectionKind},
    };
    use serde::Deserialize;
    use std::net::{IpAddr, Ipv4Addr};
    use std::path::PathBuf;

    #[derive(Deserialize)]
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

    fn default_name() -> String {
        "Aphanite".into()
    }

    #[derive(Deserialize)]
    pub(super) struct ServiceConfig {
        pub listen: IpAddr,
        pub port: u16,
        pub path: Option<String>,
        pub domain: String,
        #[serde(default = "default_name")]
        pub name: String,
        #[serde(default = "default_data_dir")]
        pub data_path: PathBuf,
        #[serde(default)]
        pub client_ip: ClientIpType,
        #[serde(default)]
        pub public: bool,
        #[serde(default)]
        pub turnstile: TurnstileSettings,

        #[serde(default)]
        pub tls: TlsConfig,
    }

    #[derive(Deserialize, Default)]
    #[serde(rename_all = "kebab-case")]
    pub(super) enum TlsMode {
        /// Disable any TLS feature
        #[default]
        Disabled,

        /// Disable build-in TLS
        External,

        /// Enable TLS with the provided certificate
        Manual,

        /// Enable TLS with ACME certificate
        AcmeTlsAlpn,
    }

    #[derive(Deserialize, Default)]
    pub(super) struct TlsConfig {
        #[serde(default)]
        pub mode: TlsMode,
        pub cert: Option<KeyConfig>,
        pub private_key: Option<KeyConfig>,
        pub acme_email: Option<String>,
    }

    impl ServiceConfig {
        /// Get the clean subdirectory prefix (no leading/trailing slash), or empty string if none.
        fn path_prefix(&self) -> &str {
            const EMPTY: &str = "";
            self.path
                .as_deref()
                .map(|p| p.trim_start_matches('/').trim_end_matches('/'))
                .unwrap_or(EMPTY)
        }

        /// Get the full base URL (scheme + domain + optional subpath).
        ///
        /// Example: `"https://example.com/aphanite/"`
        fn base_url(&self) -> String {
            let prefix = self.path_prefix();
            let base = format!(
                "{}://{}{}",
                if matches!(self.tls.mode, TlsMode::Disabled) {
                    "http"
                } else {
                    "https"
                },
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
    #[derive(Deserialize, Default)]
    pub enum ClientIpType {
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

        /// Acquires the client IP from the TCP connection
        #[serde(rename = "direct")]
        Direct,

        /// Disables IP address examine completely
        #[default]
        #[serde(rename = "disabled")]
        Disabled,
    }

    impl ClientIpType {
        pub fn is_disabled(&self) -> bool {
            matches!(self, Self::Disabled)
        }
    }

    #[derive(Deserialize, Default)]
    pub(super) struct TurnstileSettings {
        #[serde(default)]
        pub enabled: bool,
        #[serde(default)]
        pub site_key: String,
        #[serde(default)]
        pub secret_key: String,
    }

    #[derive(Deserialize, strum::Display, Default)]
    #[serde(rename_all = "lowercase")]
    pub(super) enum StorageType {
        #[default]
        #[strum(serialize = "local")]
        Local,
        #[strum(serialize = "s3")]
        S3,
    }
    #[derive(Deserialize, Default)]
    pub(super) struct StorageConfig {
        #[serde(rename = "type")]
        #[serde(default)]
        pub storage_type: StorageType,
        pub local: Option<LocalStorageConfig>,
        pub s3: Option<S3StorageConfig>,
    }

    #[derive(Deserialize, Default)]
    pub(super) struct LocalStorageConfig {
        pub path: Option<PathBuf>,
    }

    #[derive(Deserialize)]
    pub(super) struct S3StorageConfig {
        pub bucket_name: String,
        pub endpoint: url::Url,
        pub region: String,
        pub access_key: String,
        pub secret_key: String,
        pub domains: Vec<String>,
    }

    #[derive(Deserialize, Default)]
    #[serde(rename_all = "lowercase")]
    pub(super) enum DatabaseBackend {
        #[default]
        Sqlite,
        Postgres,
    }

    #[derive(Deserialize, Default)]
    pub(super) struct DatabaseConfig {
        #[serde(default)]
        pub backend: DatabaseBackend,
        pub postgres_url: Option<String>,
    }

    #[derive(Deserialize)]
    pub(super) struct YggdrasilConfig {
        pub private_key: KeyConfig,

        pub homepage: Option<String>,
        pub register_page: Option<String>,
    }

    pub(super) enum KeyConfig {
        Path(PathBuf),
        Certificate(CertificateDer<'static>),
        PrivateKey(PrivateKeyDer<'static>),
    }

    impl PemObject for KeyConfig {
        fn from_pem(kind: SectionKind, der: Vec<u8>) -> Option<Self> {
            match kind {
                SectionKind::Certificate => Some(Self::Certificate(der.into())),
                _ => PrivateKeyDer::from_pem(kind, der).map(Self::PrivateKey),
            }
        }
    }

    impl<'de> Deserialize<'de> for KeyConfig {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            use serde::de::Error;

            let value = String::deserialize(deserializer)?;
            let path = PathBuf::from(&value);
            if path.exists() {
                return Ok(Self::Path(path));
            }
            Self::from_pem_slice(value.trim().as_bytes()).map_err(|error| {
                D::Error::custom(format!(
                    "expected an existing path or a PEM certificate or private key: {error}"
                ))
            })
        }
    }

    impl KeyConfig {
        fn into_certificate(self) -> anyhow::Result<CertificateDer<'static>> {
            match self {
                Self::Certificate(certificate) => Ok(certificate),
                Self::Path(path) => CertificateDer::from_pem_file(&path)
                    .with_context(|| format!("Failed to load certificate from {}", path.display())),
                Self::PrivateKey(_) => {
                    anyhow::bail!("Expected a certificate, received a private key")
                }
            }
        }

        fn into_private_key(self) -> anyhow::Result<PrivateKeyDer<'static>> {
            match self {
                Self::PrivateKey(key) => Ok(key),
                Self::Path(path) => PrivateKeyDer::from_pem_file(&path)
                    .with_context(|| format!("Failed to load private key from {}", path.display())),
                Self::Certificate(_) => {
                    anyhow::bail!("Expected a private key, received a certificate")
                }
            }
        }

        fn into_rsa_private_key(self) -> anyhow::Result<PrivateKey> {
            use rsa::pkcs1::DecodeRsaPrivateKey;
            use rsa::pkcs8::DecodePrivateKey;

            let key = self
                .into_private_key()
                .context("Invalid Yggdrasil private key")?;
            match key {
                PrivateKeyDer::Pkcs8(key) => PrivateKey::from_pkcs8_der(key.secret_pkcs8_der())
                    .context("Yggdrasil requires a valid PKCS#8 RSA private key"),
                PrivateKeyDer::Pkcs1(key) => PrivateKey::from_pkcs1_der(key.secret_pkcs1_der())
                    .context("Yggdrasil requires a valid PKCS#1 RSA private key"),
                _ => anyhow::bail!("Yggdrasil requires an RSA private key"),
            }
        }
    }

    impl RawConfig {
        /// Get the bundled "default" configuration file
        pub(super) fn generate(listen: Option<IpAddr>, port: Option<u16>) -> String {
            #[cfg_attr(not(debug_assertions), allow(unused_mut))]
            let mut replaced = EXAMPLE_CONFIG
                .replace("{APHANITE_VERSION}", env!("CARGO_PKG_VERSION"))
                .replace(
                    "{APHANITE_CONFIG_LISTEN}",
                    &listen
                        .unwrap_or(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))
                        .to_string(),
                )
                .replace(
                    "\"{APHANITE_CONFIG_PORT}\"",
                    &port.unwrap_or(3000).to_string(),
                )
                .replace(
                    "{APHANITE_CONFIG_PRIVATE_KEY}",
                    &PrivateKey::new(&mut rand::rng(), 4096)
                        .expect("Failed to generate private key")
                        .to_pkcs8_pem(LineEnding::default())
                        .expect("Failed to generate private key"),
                )
                .replace(
                    "{APHANITE_CONFIG_TLS_MODE}",
                    if cfg!(debug_assertions) {
                        "disabled"
                    } else {
                        "external"
                    },
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
                            listen.unwrap_or(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
                            port.unwrap_or(3000)
                        ),
                    );
            }

            replaced
        }
    }

    impl TryFrom<RawConfig> for super::AppConfig {
        type Error = anyhow::Error;

        fn try_from(value: RawConfig) -> Result<Self, Self::Error> {
            anyhow::ensure!(
                value.service.name.chars().count() <= 50,
                "service.name must not exceed 50 characters"
            );
            let base_url = url::Url::parse(&value.service.base_url())
                .context("Failed to parse URL generated from service.domain and service.path")?;
            let path = value.service.path_prefix().to_owned();

            let storage = match value.storage.storage_type {
                StorageType::Local => {
                    crate::config::StorageConfig::Local(crate::config::LocalStorageConfig {
                        path: value
                            .storage
                            .local
                            .and_then(|t| t.path)
                            .unwrap_or(value.service.data_path.join("assets")),
                    })
                }
                StorageType::S3 => {
                    let s3 = value
                        .storage
                        .s3
                        .context("Storage type is set to s3, but [storage.s3] is missing")?;
                    super::StorageConfig::S3(super::S3StorageConfig {
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
                DatabaseBackend::Sqlite => super::DatabaseConfig::Sqlite,
                DatabaseBackend::Postgres => super::DatabaseConfig::Postgres(
                    value
                        .database
                        .postgres_url
                        .filter(|url| !url.trim().is_empty())
                        .context(concat!(
                            "Database backend is set to postgres, but database.postgres_url ",
                            "is missing or empty",
                        ))?,
                ),
            };
            let tls = match value.service.tls.mode {
                TlsMode::Disabled => super::TlsConfig::Disabled,
                TlsMode::External => super::TlsConfig::External,
                TlsMode::Manual => {
                    let certificate = value
                        .service
                        .tls
                        .cert
                        .context("service.tls.cert is required in manual TLS mode")?
                        .into_certificate()
                        .context("Invalid service.tls.cert")?;
                    let private_key = value
                        .service
                        .tls
                        .private_key
                        .context("service.tls.private_key is required in manual TLS mode")?
                        .into_private_key()
                        .context("Invalid service.tls.private_key")?;
                    super::TlsConfig::Manual(super::TlsCertificate {
                        certificate,
                        private_key,
                    })
                }
                TlsMode::AcmeTlsAlpn => {
                    let acme_cfg =
                        rustls_acme::AcmeConfig::new([value.service.domain.clone()])
                            .contact_push(value.service.tls.acme_email.expect(
                                "service.tls.acme_email is required in acme-tls-alpn TLS mode",
                            ))
                            .cache(rustls_acme::caches::DirCache::new(
                                value.service.data_path.join("acme"),
                            ));
                    super::TlsConfig::AcmeTlsAlpn(Some(acme_cfg))
                }
            };

            Ok(Self {
                service: super::ServiceConfig {
                    listen: value.service.listen,
                    port: value.service.port,
                    path,
                    domain: value.service.domain,
                    name: value.service.name,
                    data_path: value.service.data_path,
                    tls,
                    client_ip: value.service.client_ip,
                    public: value.service.public,
                    turnstile: super::TurnstileSettings {
                        enabled: value.service.turnstile.enabled,
                        site_key: value.service.turnstile.site_key,
                        secret_key: value.service.turnstile.secret_key,
                    },
                    base_url,
                },
                storage,
                database,
                yggdrasil: super::YggdrasilConfig {
                    private_key: value.yggdrasil.private_key.into_rsa_private_key()?,
                    homepage: value.yggdrasil.homepage,
                    register_page: value.yggdrasil.register_page,
                },
            })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use rsa::pkcs8::EncodePrivateKey;
        use serde::de::value::{Error, StringDeserializer};

        fn parse_key(value: &str) -> Result<KeyConfig, Error> {
            KeyConfig::deserialize(StringDeserializer::<Error>::new(value.into()))
        }

        #[test]
        fn private_key_accepts_file_paths() {
            let path = std::env::current_dir().unwrap();
            let KeyConfig::Path(parsed) = parse_key(path.to_str().unwrap()).unwrap() else {
                panic!("expected a file path");
            };
            assert_eq!(parsed, path);
        }

        #[test]
        fn private_key_accepts_inline_pkcs8_pem() {
            let key = PrivateKey::new(&mut rand::rng(), 2048).unwrap();
            let pem = key.to_pkcs8_pem(LineEnding::LF).unwrap();
            let parsed = parse_key(&pem).unwrap().into_rsa_private_key().unwrap();
            assert_eq!(parsed.as_public_key(), key.as_public_key());
        }

        #[test]
        fn service_name_rejects_more_than_50_chars() {
            let key = PrivateKey::new(&mut rand::rng(), 2048).unwrap();
            let pem = key.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
            let raw: RawConfig = toml::from_str(&format!(
                r#"
[service]
listen = "127.0.0.1"
port = 3000
domain = "localhost"
name = "{}"

[yggdrasil]
private_key = '''
{pem}'''
"#,
                "a".repeat(51)
            ))
            .unwrap();
            let error = match crate::config::AppConfig::try_from(raw) {
                Ok(_) => panic!("expected service.name validation to fail"),
                Err(error) => error,
            };
            assert_eq!(
                error.to_string(),
                "service.name must not exceed 50 characters"
            );
        }

        #[test]
        fn private_key_rejects_empty_and_malformed_pem() {
            for value in [
                "",
                "  ",
                "-----BEGIN PRIVATE KEY-----\ninvalid\n-----END PRIVATE KEY-----",
            ] {
                assert!(
                    parse_key(value)
                        .and_then(|key| {
                            key.into_rsa_private_key().map_err(serde::de::Error::custom)
                        })
                        .is_err()
                );
            }
        }
    }
}
