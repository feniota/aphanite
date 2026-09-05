use crate::config::{AppConfig, DatabaseBackend};
use crate::kv_cache::KVCache;
use crate::storage::AssetStorage;
use crate::types::RegisterToken;
use clap::Parser;
use rsa::RsaPublicKey;
use std::sync::Arc;
use tracing::info;

mod cli;
pub mod config;
pub mod data;
pub mod kv_cache;
pub mod service;
pub mod storage;
pub mod types;

#[derive(Clone)]
pub struct AppState {
    pub da: data::DatabaseAccessor,
    pub cfg: Arc<AppConfig>,
    pub assets: AssetStorage,
    pub kv: KVCache,
    pub rsa_pubkey: RsaPublicKey,
    pub http_client: reqwest::Client,
    /// Clean subdirectory prefix (e.g. "aphanite"), empty if serving at root.
    pub base_path: String,
}

pub async fn start() -> anyhow::Result<()> {
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

    let db = if matches!(config.database.backend, DatabaseBackend::Turso) {
        let db_path = &config.service.data_path.join("db.sqlite");
        let db_path_str = db_path
            .to_str()
            .expect("FATAL: Database path is not a valid UTF-8 string!");
        tracing::debug!("Using db at: turso:{}", db_path_str);
        let driver = toasty_driver_turso::Turso::file(db_path_str).concurrent_writes();
        toasty::Db::builder()
            .models(toasty::models!(crate::*))
            .build(driver)
            .await?
    } else {
        let db_url = config.database.postgres_url.clone();
        let redacted_db_url = db_url
            .split_once('@')
            .map(|(_, rest)| format!("<redacted>@{rest}"))
            .unwrap_or_else(|| db_url.clone());
        tracing::debug!("Using db at: {redacted_db_url}");
        toasty::Db::builder()
            .models(toasty::models!(crate::*))
            .connect(&db_url)
            .await?
    };

    #[cfg(debug_assertions)]
    {
        if args.with_test_user {
            // Create a test user with fixed informations
            let mut db = db.clone();

            use argon2::{Argon2, password_hash::PasswordHasher};
            let uuid = uuid::uuid!("11451419-1981-8011-8451-419198101145");
            let email = "test@aphanite.example.com";
            let password = b"01234567890";
            let name = "Aphanite_Test";
            let argon2 = Argon2::default();
            let hashed_password = argon2.hash_password(password)?.to_string();

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

    if let Some(cli::Command::CreateAdmin {
        email,
        nickname,
        password,
    }) = &args.command
    {
        use argon2::password_hash::PasswordHasher;
        use types::User;

        let nickname = nickname.clone().unwrap_or_else(|| email.clone());
        let argon2 = argon2::Argon2::default();
        let hashed_password = argon2
            .hash_password(password.as_bytes())
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
            .exec(&mut db)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create admin user: {e}"))?;

        info!("Admin user created: {email} (nickname: {nickname})");
        return Ok(());
    }

    let base_path = config.service.path_prefix().to_string();

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
        base_path,
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

    // Build the inner app (API + frontend) with its state already set
    let inner_app: axum::Router = service::router(state.clone())
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()));

    let app = if state.base_path.is_empty() {
        inner_app.nest("/assets", storage_router)
    } else {
        let prefix = format!("/{}", state.base_path);
        // Build a new outer router that handles both /assets directly and
        // /prefix/{*rest} by stripping the prefix and forwarding.
        let mut outer = axum::Router::new();
        // Mount assets directly
        outer = outer.nest("/assets", storage_router.clone());

        // Handle /prefix/ (root) and /prefix/{*rest} (all sub-paths)
        let root_route = format!("/{}/", state.base_path);
        let catch_route = format!("/{}/{{*rest}}", state.base_path);
        let prefix_captured = prefix.clone();

        let prefix_handler = move |req: axum::http::Request<axum::body::Body>| {
            let app = inner_app.clone();
            let s_router = storage_router.clone();
            let prefix = prefix_captured.clone();
            async move {
                use tower::Service;
                let path = req.uri().path().to_string();
                if let Some(rest) = path.strip_prefix(&prefix) {
                    if rest.is_empty() || rest == "/" {
                        // Root of the subdirectory → serve index.html
                        let (mut parts, _body) = req.into_parts();
                        parts.uri = axum::http::Uri::try_from("/").unwrap();
                        return Service::call(
                            &mut app.clone(),
                            axum::http::Request::from_parts(parts, axum::body::Body::empty()),
                        )
                        .await;
                    }
                    if rest.starts_with("/assets") || rest.starts_with("assets") {
                        let asset_path = rest.trim_start_matches('/').trim_start_matches("assets/");
                        let (mut parts, body) = req.into_parts();
                        parts.uri = axum::http::Uri::try_from(&format!("/{}", asset_path))
                            .unwrap_or(parts.uri);
                        return Service::call(
                            &mut s_router.clone(),
                            axum::http::Request::from_parts(parts, body),
                        )
                        .await;
                    }
                    let new_path = rest;
                    let (mut parts, body) = req.into_parts();
                    parts.uri = axum::http::Uri::try_from(new_path).unwrap_or(parts.uri);
                    return Service::call(
                        &mut app.clone(),
                        axum::http::Request::from_parts(parts, body),
                    )
                    .await;
                }
                Service::call(&mut app.clone(), req).await
            }
        };

        outer = outer.route(
            &root_route,
            axum::routing::any_service(tower::service_fn(prefix_handler.clone())),
        );
        outer = outer.route(
            &catch_route,
            axum::routing::any_service(tower::service_fn(prefix_handler)),
        );
        outer
    };

    info!("Service listening on http://{}:{}", listen, port);
    if !(args.debug || args.verbose) {
        eprintln!("Service listening on http://{}:{}", listen, port);
    }

    let listener = tokio::net::TcpListener::bind((listen, port)).await?;
    axum::serve(listener, app).await?;
    Ok(())
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
        info!("Cleaned up {count} expired registration token(s)");
    }
}
