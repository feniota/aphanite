//! Integration tests for the Yggdrasil authentication API.

use super::common;

use common::{TestApp, create_test_profile, create_test_user, new_test_state, request};
use ntex::http::{Method, StatusCode};
use ntex::web::{self, test};
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

async fn setup() -> (TestApp, aphanite::AppState, TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let state = new_test_state(tmp.path())
        .await
        .expect("failed to build test AppState");
    let router = test::init_service_st(
        web::AppState::new(state.clone()),
        web::App::new().configure(aphanite::service::yggdrasil::configure),
    )
    .await;
    (router, state, tmp)
}

async fn post_json(app: &TestApp, uri: &str, body: Value) -> (StatusCode, String) {
    let response = request(app, Method::POST, uri, None, Some(body)).await;
    let status = response.status();
    let bytes = test::read_body(response).await;
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn get(app: &TestApp, uri: &str) -> (StatusCode, String) {
    let response = request(app, Method::GET, uri, None, None).await;
    let status = response.status();
    let bytes = test::read_body(response).await;
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn do_authenticate(app: &TestApp, username: &str, password: &str) -> Value {
    let (status, body) = post_json(
        app,
        "/authserver/authenticate",
        json!({
            "username": username,
            "password": password,
            "clientToken": "client-abc",
            "requestUser": false,
            "agent": {"name": "Minecraft", "version": 1}
        }),
    )
    .await;
    // 正确凭证应返回 200
    assert_eq!(status, StatusCode::OK, "authenticate failed: {}", body);
    serde_json::from_str(&body).unwrap()
}

fn error_type(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .unwrap()
        .get("error")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

// ── Meta ────────────────────────────────────────────────────────────────

#[ntex::test]
async fn yggdrasil_meta() {
    let (app, _state, _tmp) = setup().await;
    let (status, body) = get(&app, "/").await;

    // GET / → 200
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_str(&body).unwrap();
    // 响应体包含服务器元信息
    assert!(v.get("meta").is_some(), "{}", body);
    // 响应体包含 RSA 签名公钥
    assert!(v.get("signaturePublickey").is_some(), "{}", body);
}

// ── Authenticate ────────────────────────────────────────────────────────

#[ntex::test]
async fn yggdrasil_authenticate_success() {
    let (app, state, _tmp) = setup().await;

    let user = create_test_user(&state, "test@aphanite.example.com").await;
    let _profile = create_test_profile(&state, user.id, "TestPlayer").await;

    let (status, body) = post_json(
        &app,
        "/authserver/authenticate",
        json!({
            "username": "test@aphanite.example.com",
            "password": "password123",
            "clientToken": "client-abc",
            "requestUser": true,
            "agent": {"name": "Minecraft", "version": 1}
        }),
    )
    .await;

    // 正确凭证 → 200 OK
    assert_eq!(status, StatusCode::OK, "{}", body);
    let v: Value = serde_json::from_str(&body).unwrap();
    // 返回的 clientToken 与请求一致
    assert_eq!(v["clientToken"], "client-abc");
    // availableProfiles 包含用户拥有的角色
    let profiles = v["availableProfiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0]["name"], "TestPlayer");
    // 只有一个角色时自动选中
    assert!(v["selectedProfile"].is_object());
    assert_eq!(v["selectedProfile"]["name"], "TestPlayer");
    // requestUser=true → 返回 user 对象
    assert!(v["user"].is_object());
}

#[ntex::test]
async fn yggdrasil_authenticate_wrong_password() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;

    let (status, body) = post_json(
        &app,
        "/authserver/authenticate",
        json!({
            "username": "test@aphanite.example.com",
            "password": "wrongpass",
            "requestUser": false,
            "agent": {"name": "Minecraft", "version": 1}
        }),
    )
    .await;

    // 错误密码 → 403 Forbidden
    assert_eq!(status, StatusCode::FORBIDDEN, "{}", body);
    // Yggdrasil 错误类型为 ForbiddenOperationException
    assert_eq!(error_type(&body), "ForbiddenOperationException");
}

#[ntex::test]
async fn yggdrasil_authenticate_nonexistent_user() {
    let (app, _state, _tmp) = setup().await;

    let (status, body) = post_json(
        &app,
        "/authserver/authenticate",
        json!({
            "username": "nobody@nowhere.com",
            "password": "password123",
            "requestUser": false,
            "agent": {"name": "Minecraft", "version": 1}
        }),
    )
    .await;

    // 不存在的用户 → 403（不泄露用户是否存在）
    assert_eq!(status, StatusCode::FORBIDDEN, "{}", body);
    assert_eq!(error_type(&body), "ForbiddenOperationException");
}

// ── Refresh ─────────────────────────────────────────────────────────────

#[ntex::test]
async fn yggdrasil_refresh_success() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;

    let auth = do_authenticate(&app, "test@aphanite.example.com", "password123").await;
    let old_token = auth["accessToken"].as_str().unwrap();

    let (status, body) = post_json(
        &app,
        "/authserver/refresh",
        json!({
            "accessToken": old_token,
            "clientToken": "client-abc",
            "requestUser": false
        }),
    )
    .await;

    // 有效 token → 200 OK
    assert_eq!(status, StatusCode::OK, "{}", body);
    let v: Value = serde_json::from_str(&body).unwrap();
    let new_token = v["accessToken"].as_str().unwrap();
    // 刷新后的 accessToken 应不同于原 token
    assert_ne!(new_token, old_token);
}

#[ntex::test]
async fn yggdrasil_refresh_invalid_token() {
    let (app, _state, _tmp) = setup().await;

    let fake_token = Uuid::now_v7().simple().to_string();
    let (status, body) = post_json(
        &app,
        "/authserver/refresh",
        json!({
            "accessToken": fake_token,
            "clientToken": "client-abc",
            "requestUser": false
        }),
    )
    .await;

    // 无效 token → 403 Forbidden
    assert_eq!(status, StatusCode::FORBIDDEN, "{}", body);
    assert_eq!(error_type(&body), "ForbiddenOperationException");
}

// ── Validate ────────────────────────────────────────────────────────────

#[ntex::test]
async fn yggdrasil_validate_success() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;

    let auth = do_authenticate(&app, "test@aphanite.example.com", "password123").await;
    let token = auth["accessToken"].as_str().unwrap();

    let (status, _) = post_json(
        &app,
        "/authserver/validate",
        json!({ "accessToken": token, "clientToken": "client-abc" }),
    )
    .await;

    // 有效 token → 204 No Content
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[ntex::test]
async fn yggdrasil_validate_invalid_token() {
    let (app, _state, _tmp) = setup().await;

    let fake_token = Uuid::now_v7().simple().to_string();
    let (status, body) = post_json(
        &app,
        "/authserver/validate",
        json!({ "accessToken": fake_token }),
    )
    .await;

    // 无效 token → 403 Forbidden
    assert_eq!(status, StatusCode::FORBIDDEN, "{}", body);
}

// ── Invalidate ──────────────────────────────────────────────────────────

#[ntex::test]
async fn yggdrasil_invalidate() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;

    let auth = do_authenticate(&app, "test@aphanite.example.com", "password123").await;
    let token = auth["accessToken"].as_str().unwrap();

    // Invalidate → 204 No Content
    let (status, _) = post_json(
        &app,
        "/authserver/invalidate",
        json!({ "accessToken": token }),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // 作废后 validate 应失败 → 403
    let (status, _) = post_json(
        &app,
        "/authserver/validate",
        json!({ "accessToken": token }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ── Signout ─────────────────────────────────────────────────────────────

#[ntex::test]
async fn yggdrasil_signout() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;

    let auth = do_authenticate(&app, "test@aphanite.example.com", "password123").await;
    let token = auth["accessToken"].as_str().unwrap();

    // Signout → 204 No Content
    let (status, _) = post_json(
        &app,
        "/authserver/signout",
        json!({ "username": "test@aphanite.example.com", "password": "password123" }),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // 登出后该用户所有 token 均失效 → 403
    let (status, _) = post_json(
        &app,
        "/authserver/validate",
        json!({ "accessToken": token }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ── Join + HasJoined ────────────────────────────────────────────────────

#[ntex::test]
async fn yggdrasil_join_and_has_joined() {
    let (app, state, _tmp) = setup().await;

    let user = create_test_user(&state, "test@aphanite.example.com").await;
    let profile = create_test_profile(&state, user.id, "TestPlayer").await;

    let auth = do_authenticate(&app, "test@aphanite.example.com", "password123").await;
    let access_token = auth["accessToken"].as_str().unwrap();
    let server_id = Uuid::now_v7().to_string();

    // Join → 204 No Content
    let (status, _) = post_json(
        &app,
        "/sessionserver/session/minecraft/join",
        json!({
            "accessToken": access_token,
            "selectedProfile": profile.id,
            "serverId": server_id
        }),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // hasJoined → 200，返回完整的 GameProfile
    let (status, body) = get(
        &app,
        &format!(
            "/sessionserver/session/minecraft/hasJoined?username={}&serverId={}",
            "TestPlayer", server_id
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", body);

    let v: Value = serde_json::from_str(&body).unwrap();
    // 返回的角色名与 join 时一致
    assert_eq!(v["name"], "TestPlayer");
}

// ── Profile ─────────────────────────────────────────────────────────────

#[ntex::test]
async fn yggdrasil_profile() {
    let (app, state, _tmp) = setup().await;

    let user = create_test_user(&state, "test@aphanite.example.com").await;
    let profile = create_test_profile(&state, user.id, "TestPlayer").await;

    let (status, body) = get(
        &app,
        &format!("/sessionserver/session/minecraft/profile/{}", profile.id),
    )
    .await;

    // 查询指定 UUID 的角色 → 200 OK
    assert_eq!(status, StatusCode::OK, "{}", body);
    let v: Value = serde_json::from_str(&body).unwrap();
    // 角色名匹配
    assert_eq!(v["name"], "TestPlayer");
    // properties 中包含 textures 属性（含 skin/cape URL）
    let props = v["properties"].as_array().unwrap();
    let textures = props.iter().find(|p| p["name"] == "textures");
    assert!(textures.is_some(), "missing textures property in {}", body);
}
