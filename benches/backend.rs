mod support;

use aphanite::{service::yggdrasil::api::Session, types::User};
use async_compat::CompatExt;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use ntex::http::Method;
use serde_json::{Value, json};
use std::{
    net::{IpAddr, Ipv4Addr},
    rc::Rc,
    time::{Duration, Instant},
};
use support::Server;
use uuid::Uuid;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Clone)]
struct Request {
    method: Method,
    path: String,
    body: Vec<u8>,
    content_type: &'static str,
    bearer: Option<String>,
    authenticated: bool,
    status: u16,
    range: bool,
}

impl Request {
    fn new(method: Method, path: impl Into<String>, status: u16) -> Self {
        Self {
            method,
            path: path.into(),
            body: vec![],
            content_type: "application/json",
            bearer: None,
            authenticated: true,
            status,
            range: false,
        }
    }
    fn json(mut self, body: Value) -> Self {
        self.body = serde_json::to_vec(&body).unwrap();
        self
    }
}

#[derive(Clone)]
enum Case {
    Fixed(Request),
    Password { valid: bool, yggdrasil: bool },
    Otp(&'static str),
    Limited,
    Join,
    HasJoined,
    Totp(&'static str),
    Upload(bool),
    DeleteTexture,
    CreateProfile,
    Refresh,
}

async fn send(client: &ntex::client::Client, server: &Server, req: &Request) -> Vec<u8> {
    let mut request = client
        .request(req.method.clone(), server.http.url(&req.path))
        .header("content-type", req.content_type);
    if req.authenticated {
        request = request.header(
            "authorization",
            format!(
                "Bearer {}",
                req.bearer.as_ref().unwrap_or(&server.fixture.token)
            ),
        );
    }
    if req.range {
        request = request.header("range", "bytes=8192-73727");
    }
    let response = request
        .send_body(req.body.clone())
        .await
        .expect("HTTP request failed");
    let status = response.status().as_u16();
    let location = response.headers().get("location").cloned();
    let body = response
        .body()
        .await
        .expect("response body failed")
        .to_vec();
    assert_eq!(
        status,
        req.status,
        "{} {}: {}",
        req.method,
        req.path,
        String::from_utf8_lossy(&body)
    );
    if status == 302 {
        let location = location.unwrap();
        let location = location.to_str().unwrap();
        assert!(location.contains("X-Amz-Signature="));
        assert!(location.contains("X-Amz-Expires=900"));
    } else if req.path.starts_with("/assets/") && (status == 200 || status == 206) {
        assert_eq!(
            body.as_slice(),
            vec![42; if req.range { 65536 } else { 128 * 1024 }]
        );
    } else if status == 200 && req.path.contains("unsigned=false") {
        let profile: Value = serde_json::from_slice(&body).unwrap();
        let properties = profile["properties"].as_array().unwrap();
        assert!(!properties.is_empty());
        assert!(properties.iter().all(|v| v["signature"].is_string()));
    } else if status == 200 && (req.path == "/api/auth/login" || req.path == "/api/auth/refresh") {
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert!(value["payload"]["access_token"].is_string());
    }
    body
}

fn texture_path(server: &Server) -> String {
    format!(
        "/api/yggdrasil/api/user/profile/{}/skin",
        server.fixture.profile.simple()
    )
}

fn upload(server: &Server) -> Request {
    let mut body = b"--bench\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\ndefault\r\n--bench\r\nContent-Disposition: form-data; name=\"file\"; filename=\"skin.png\"\r\nContent-Type: image/png\r\n\r\n".to_vec();
    body.extend(support::png(7));
    body.extend_from_slice(b"\r\n--bench--\r\n");
    Request {
        body,
        content_type: "multipart/form-data; boundary=bench",
        ..Request::new(Method::PUT, texture_path(server), 204)
    }
}

async fn prepare(server: &Server, client: &ntex::client::Client, case: &Case) -> Request {
    let f = &server.fixture;
    match case {
        Case::Fixed(request) => request.clone(),
        Case::Password { .. } | Case::Otp(_) => {
            // Fixture preparation is untimed. Fresh emails avoid accidentally
            // measuring rate-limit rejection instead of password/OTP login.
            let email = format!("{}@bench.invalid", Uuid::now_v7());
            let mut db = f.db.clone();
            let mut user = User::get_by_id(&mut db, &f.login_user).await.unwrap();
            user.update().email(&email).exec(&mut db).await.unwrap();
            match case {
                Case::Password { valid, yggdrasil } => {
                    let password = if *valid {
                        "password123"
                    } else {
                        "wrong-password"
                    };
                    let (path, body) = if *yggdrasil {
                        (
                            "/api/yggdrasil/authserver/authenticate",
                            json!({"username":email,"password":password,"clientToken":"bench-client","requestUser":true,"agent":{"name":"Minecraft","version":1}}),
                        )
                    } else {
                        (
                            "/api/auth/login",
                            json!({"email":email,"password":password}),
                        )
                    };
                    Request::new(Method::POST, path, if *valid { 200 } else { 403 }).json(body)
                }
                Case::Otp(mode) => {
                    let token = if *mode == "missing" {
                        Uuid::nil()
                    } else {
                        f.kv.sign_otp_token(if *mode == "mismatch" {
                            "other@example.com".into()
                        } else {
                            email.clone()
                        })
                        .await
                    };
                    if *mode == "used" {
                        assert!(f.kv.verify_opt_token(&token, &email).await);
                    }
                    Request::new(
                        Method::POST,
                        "/api/auth/login",
                        if *mode == "success" { 200 } else { 403 },
                    )
                    .json(json!({"email":email,"otp_token":token}))
                }
                _ => unreachable!(),
            }
        }
        Case::Limited => {
            while f.kv.try_consume("limited@example.com").await {}
            Request::new(Method::POST, "/api/auth/login", 429)
                .json(json!({"email":"limited@example.com","password":"password123"}))
        }
        Case::Join | Case::HasJoined => {
            // record_session only inserts absent keys; reusing a key does not
            // refresh its TTL. Use a new server ID for every operation/sample.
            let server_id = Uuid::now_v7().simple().to_string();
            let join = Request::new(Method::POST, "/api/yggdrasil/sessionserver/session/minecraft/join", 204)
                .json(json!({"accessToken": f.token, "selectedProfile": f.profile.simple().to_string(), "serverId": server_id}));
            if matches!(case, Case::Join) {
                return join;
            }
            // Establish the live session over HTTP outside the measured interval.
            send(client, server, &join).await;
            Request::new(
                Method::GET,
                format!(
                    "/api/yggdrasil/sessionserver/session/minecraft/hasJoined?username=BenchPlayer&serverId={server_id}"
                ),
                200,
            )
        }
        Case::Totp(mode) => {
            let create = Request::new(Method::POST, "/api/verification", 200)
                .json(json!({"method":"totp","email":"bench@example.com"}));
            if *mode == "create" {
                return create;
            }
            let body = send(client, server, &create).await;
            let value: Value = serde_json::from_slice(&body).unwrap();
            let code = if *mode == "success" {
                totp_rs::Builder::new()
                    .with_secret(
                        totp_rs::Secret::try_from_base32("JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP")
                            .unwrap(),
                    )
                    .with_issuer(Some("Aphanite"))
                    .with_account_name("bench@example.com")
                    .build()
                    .unwrap()
                    .generate_current()
                    .to_string()
            } else {
                "invalid".into()
            };
            Request::new(
                Method::POST,
                format!(
                    "/api/verification/{}",
                    value["payload"]["id"].as_str().unwrap()
                ),
                if *mode == "success" { 200 } else { 401 },
            )
            .json(json!({"code":code}))
        }
        Case::Upload(dedup) => {
            if *dedup {
                send(client, server, &upload(server)).await;
            } else {
                send(
                    client,
                    server,
                    &Request::new(Method::DELETE, texture_path(server), 204),
                )
                .await;
            }
            upload(server)
        }
        Case::DeleteTexture => {
            send(client, server, &upload(server)).await;
            Request::new(Method::DELETE, texture_path(server), 204)
        }
        Case::CreateProfile => Request::new(Method::POST, "/api/profile", 200)
            .json(json!({"name":format!("B{}",&Uuid::now_v7().simple().to_string()[17..])})),
        Case::Refresh => {
            let da = aphanite::data::DatabaseAccessor::new(f.db.clone());
            let token = da
                .create_token(&f.login_user, "bench-client", None)
                .await
                .unwrap();
            Request {
                bearer: Some(token.to_string()),
                ..Request::new(Method::POST, "/api/auth/refresh", 200)
            }
        }
    }
}

async fn batch(
    client: &ntex::client::Client,
    server: Rc<Server>,
    req: &Request,
    concurrency: usize,
) -> Vec<u8> {
    if concurrency == 1 {
        return send(client, &server, req).await;
    }
    let mut tasks = Vec::new();
    for _ in 0..concurrency {
        let client = client.clone();
        let server = server.clone();
        let req = req.clone();
        tasks.push(ntex::rt::spawn(async move {
            send(&client, &server, &req).await
        }));
    }
    for task in tasks {
        std::hint::black_box(task.await.unwrap());
    }
    Vec::new()
}

// Only HTTP round trips (through complete response bodies) contribute to time.
#[ntex::main]
async fn measure(server: Rc<Server>, case: Case, iterations: u64, concurrency: usize) -> Duration {
    async {
        let client = ntex::client::Client::new();
        let warm = Request::new(Method::GET, "/api/yggdrasil/", 200);
        batch(&client, server.clone(), &warm, concurrency).await;
        if matches!(case, Case::Join | Case::HasJoined)
            || matches!(&case,Case::Fixed(req) if req.path.contains("hasJoined"))
        {
            for i in 0..10_000 {
                server
                    .fixture
                    .kv
                    .record_session(Session {
                        profile_id: server.fixture.profile,
                        access_token: server.fixture.token.parse().unwrap(),
                        server_id: format!("resident-{i}"),
                        ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
                    })
                    .await;
            }
        }
        let mut elapsed = Duration::ZERO;
        for _ in 0..iterations {
            let req = prepare(&server, &client, &case).await;
            let start = Instant::now();
            let body = batch(&client, server.clone(), &req, concurrency).await;
            elapsed += start.elapsed();
            if matches!(case, Case::CreateProfile) {
                let value: Value = serde_json::from_slice(&body).unwrap();
                send(
                    &client,
                    &server,
                    &Request::new(
                        Method::DELETE,
                        format!("/api/profiles/{}", value["payload"]["id"].as_str().unwrap()),
                        200,
                    ),
                )
                .await;
            }
        }
        elapsed
    }
    .compat()
    .await
}

#[ntex::main]
async fn seed(server: Rc<Server>) {
    async {
        let mut db = server.fixture.db.clone();
        let mut user = User::get_by_id(&mut db, &server.fixture.user)
            .await
            .unwrap();
        user.update()
            .totp_secret("JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP")
            .exec(&mut db)
            .await
            .unwrap();
    }
    .compat()
    .await;
}

fn benchmarks(c: &mut Criterion) {
    let local = Rc::new(support::start(false));
    seed(local.clone());
    let f = &local.fixture;
    let fixed = |method, path, status| Case::Fixed(Request::new(method, path, status));
    let mut cases: Vec<(String,Case)> = vec![
        ("ntex/meta".into(),fixed(Method::GET,"/api/yggdrasil/".into(),200)),
        ("auth/limited".into(),Case::Limited),
        ("auth/refresh".into(),Case::Refresh),
        ("token/valid".into(),fixed(Method::GET,"/api/auth/validate".into(),204)),
        ("token/missing".into(),Case::Fixed(Request {authenticated:false,..Request::new(Method::GET,"/api/auth/validate",401)})),
        ("token/unknown".into(),Case::Fixed(Request::new(Method::POST,"/api/yggdrasil/authserver/validate",403).json(json!({"accessToken":Uuid::nil().simple().to_string()})))),
        ("user/get".into(),fixed(Method::GET,"/api/users/me".into(),200)),
        ("profiles/list".into(),fixed(Method::GET,"/api/users/me/profiles?with_skin=true".into(),200)),
        ("profiles/create".into(),Case::CreateProfile),
        ("session/join".into(),Case::Join),
        ("session/hit_signed".into(),Case::HasJoined),
        ("session/miss".into(),fixed(Method::GET,"/api/yggdrasil/sessionserver/session/minecraft/hasJoined?username=BenchPlayer&serverId=missing".into(),204)),
        ("profile/unsigned".into(),fixed(Method::GET,format!("/api/yggdrasil/sessionserver/session/minecraft/profile/{}",f.profile.simple()),200)),
        ("profile/signed".into(),fixed(Method::GET,format!("/api/yggdrasil/sessionserver/session/minecraft/profile/{}?unsigned=false",f.profile.simple()),200)),
        ("profile/missing".into(),fixed(Method::GET,format!("/api/yggdrasil/sessionserver/session/minecraft/profile/{}",Uuid::nil().simple()),204)),
        ("totp/session/create".into(),Case::Totp("create")),
        ("totp/session/complete".into(),Case::Totp("success")),
        ("totp/session/wrong_code".into(),Case::Totp("wrong")),
        ("totp/session/missing".into(),Case::Fixed(Request::new(Method::POST,format!("/api/verification/{}",Uuid::nil()),404).json(json!({"code":"123456"})))),
        ("storage/upload/new".into(),Case::Upload(false)),
        ("storage/upload/dedup".into(),Case::Upload(true)),
        ("storage/delete".into(),Case::DeleteTexture),
        ("storage/download".into(),fixed(Method::GET,format!("/assets/{}",f.asset),200)),
        ("storage/range".into(),Case::Fixed(Request {range:true,..Request::new(Method::GET,format!("/assets/{}",f.asset),206)})),
        ("storage/missing".into(),fixed(Method::GET,format!("/assets/{}",Uuid::nil()),404)),
        ("json/malformed".into(),Case::Fixed(Request {body:b"{".to_vec(),..Request::new(Method::POST,"/api/auth/login",400)})),
    ];
    for yggdrasil in [false, true] {
        for valid in [true, false] {
            cases.push((
                format!(
                    "auth/{}/{valid}",
                    if yggdrasil {
                        "yggdrasil_password"
                    } else {
                        "password"
                    }
                ),
                Case::Password { valid, yggdrasil },
            ));
        }
    }
    for mode in ["success", "missing", "mismatch", "used"] {
        cases.push((format!("auth/otp/{mode}"), Case::Otp(mode)));
    }
    for (name, input, status) in [
        ("min", "Ab_", 200),
        ("max", "Player_123456789", 200),
        ("short", "ab", 400),
        ("long", "12345678901234567", 400),
        ("character", "Player!", 400),
    ] {
        cases.push((
            format!("validation/profile_name/{name}"),
            Case::Fixed(
                Request::new(
                    Method::PATCH,
                    format!("/api/profiles/{}", f.profile),
                    status,
                )
                .json(json!({"name":input})),
            ),
        ));
    }
    for (name, input, status) in [
        ("min", "A", 200),
        ("max", "12345678901234567890", 200),
        ("unicode", "\u{73a9}\u{5bb6}", 200),
        ("empty", "", 418),
        ("long", "123456789012345678901", 418),
    ] {
        cases.push((
            format!("validation/nickname/{name}"),
            Case::Fixed(
                Request::new(Method::PATCH, "/api/users/me", status).json(json!({"name":input})),
            ),
        ));
    }
    for (name, input, status) in [
        ("min", "12345678".to_owned(), 204),
        ("max", "a".repeat(128), 204),
        ("short", "short".to_owned(), 422),
        ("long", "a".repeat(129), 422),
    ] {
        cases.push((
            format!("validation/password/{name}"),
            Case::Fixed(
                Request::new(Method::PATCH, "/api/users/me/credentials/password", status)
                    .json(json!({"new_password":input})),
            ),
        ));
    }
    let mut group = c.benchmark_group("http");
    group
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    group.throughput(Throughput::Elements(1));
    for (name, case) in cases {
        group.bench_function(name, |b| {
            b.iter_custom(|n| measure(local.clone(), case.clone(), n, 1))
        });
    }
    for count in [8, 32] {
        group.throughput(Throughput::Elements(count as u64));
        for (name, case) in [
            ("meta", fixed(Method::GET, "/api/yggdrasil/".into(), 200)),
            ("database", fixed(Method::GET, "/api/users/me".into(), 200)),
            (
                "signed_profile",
                fixed(
                    Method::GET,
                    format!(
                        "/api/yggdrasil/sessionserver/session/minecraft/profile/{}?unsigned=false",
                        f.profile.simple()
                    ),
                    200,
                ),
            ),
        ] {
            group.bench_function(format!("concurrent/{name}/{count}"), |b| {
                b.iter_custom(|n| measure(local.clone(), case.clone(), n, count))
            });
        }
    }
    let s3 = Rc::new(support::start(true));
    let redirect = fixed(Method::GET, format!("/assets/{}", s3.fixture.asset), 302);
    group.throughput(Throughput::Elements(1));
    group.bench_function("storage/s3_signed_redirect", |b| {
        b.iter_custom(|n| measure(s3.clone(), redirect.clone(), n, 1))
    });
    group.finish();
    shutdown(s3);
    shutdown(local);
}

#[ntex::main]
async fn shutdown(server: Rc<Server>) {
    drop(server);
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
