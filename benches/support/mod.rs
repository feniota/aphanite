//! Fixtures only: all measured work goes through real TCP HTTP endpoints.
#![allow(dead_code)]
#[path = "../../tests/rust/common/mod.rs"]
mod common;

use aphanite::{kv_cache::KVCache, storage::AssetStorage};

use ntex::web::{self, middleware, test};
use std::sync::{Arc, mpsc};
use uuid::Uuid;

pub struct Fixture {
    pub db: sea_orm::DatabaseConnection,
    pub kv: KVCache,
    pub user: Uuid,
    pub login_user: Uuid,
    pub profile: Uuid,
    pub token: String,
    pub asset: String,
}

pub struct Server {
    pub http: test::TestServer,
    pub fixture: Fixture,
    _dir: Arc<tempfile::TempDir>,
}

#[ntex::main]
pub async fn start(s3: bool) -> Server {
    let dir = Arc::new(tempfile::tempdir().unwrap());
    let root = dir.clone();
    let (tx, rx) = mpsc::sync_channel(1);
    let http = test::server_with(test::config().h1(), async move |_| {
        async {
            let mut state = common::new_test_state(root.path()).await.unwrap();
            let user = common::create_test_user(&state, "bench@example.com").await;
            let login_user = common::create_test_user(&state, "login@example.com").await;
            let profile = common::create_test_profile(&state, user.id, "BenchPlayer").await;
            let token = state
                .da
                .create_token(&user.id, "bench-client", Some(&profile.id))
                .await
                .unwrap();
            let asset = async {
                state
                    .assets
                    .create_file(futures_lite::io::Cursor::new(vec![42; 128 * 1024]))
                    .await
                    .unwrap()
            }
            .await;
            if s3 {
                let cfg = Arc::get_mut(&mut state.cfg).unwrap();
                cfg.storage =
                    aphanite::config::StorageConfig::S3(aphanite::config::S3StorageConfig {
                        bucket_name: "bench".into(),
                        endpoint: "https://s3.amazonaws.com".parse().unwrap(),
                        region: "us-east-1".into(),
                        access_key: "bench-access".into(),
                        secret_key: "bench-secret".into(),
                        domains: vec!["bench.s3.amazonaws.com".into()],
                    });
                state.assets = AssetStorage::from_config(state.da.db().clone(), cfg)
                    .install_client(state.http_client.clone());
            }
            tx.send(Fixture {
                db: state.da.db().clone(),
                kv: state.kv.clone(),
                user: user.id,
                login_user: login_user.id,
                profile: profile.id,
                token: token.to_string(),
                asset: asset.hash,
            })
            .unwrap_or_else(|_| panic!("fixture receiver closed"));
            // Match production's compatibility middleware and API response headers.
            web::App::new()
                .middleware(middleware::from_fn(async |req, next| next.call(req).await))
                .middleware(middleware::DefaultHeaders::new().header(
                    "X-Authlib-Injector-API-Location",
                    "http://localhost/api/yggdrasil/",
                ))
                .configure(aphanite::service::configure)
                .service(web::scope("/assets").configure(AssetStorage::configure))
                .build_with::<()>(web::AppState::new(state))
        }
        .await
    });
    // Force lazy application initialization before waiting for its fixtures.
    let response = http.get("/api/yggdrasil/").send().await.unwrap();
    assert!(response.status().is_success());
    response.body().await.unwrap();
    Server {
        http,
        fixture: rx.recv().unwrap(),
        _dir: dir,
    }
}

pub fn png(seed: u8) -> Vec<u8> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    let mut pixels = vec![0_u8; 64 * 64 * 4];
    for (i, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        pixel.copy_from_slice(&[seed, (i % 251) as u8, (i / 251) as u8, 255]);
    }
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&pixels, 64, 64, image::ExtendedColorType::Rgba8)
        .unwrap();
    out
}
