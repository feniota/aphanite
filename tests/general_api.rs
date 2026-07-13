//! Integration tests for the General API (auth, user, profile CRUD).

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::{create_test_user, login, new_test_state};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

async fn setup() -> (Router, aphanite::AppState, TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let state = new_test_state(tmp.path())
        .await
        .expect("failed to build test AppState");

    let router = Router::new()
        .nest("/api", aphanite::service::api::router())
        .nest("/api", aphanite::service::phenocryst::totp::router())
        .with_state(state.clone());

    (router, state, tmp)
}

async fn do_post(app: &Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, v)
}

async fn do_post_auth(app: &Router, uri: &str, token: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, v)
}

async fn do_get_auth(app: &Router, uri: &str, token: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, v)
}

async fn do_patch_auth(app: &Router, uri: &str, token: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(uri)
                .header("content-type", "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, v)
}

async fn do_delete_auth(app: &Router, uri: &str, token: &str) -> StatusCode {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(uri)
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    response.status()
}

// ── Auth: Login ─────────────────────────────────────────────────────────

#[tokio::test]
async fn general_auth_login_password() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;

    let (status, v) = do_post(
        &app,
        "/api/auth/login",
        json!({"email": "test@aphanite.example.com", "password": "pass"}),
    )
    .await;

    // 正确密码 → 200 OK
    assert_eq!(status, StatusCode::OK, "{:?}", v);
    // 响应格式: { success: true, payload: { ... } }
    assert!(v["success"].as_bool().unwrap_or(false));
    // payload 包含 access_token
    assert!(v["payload"]["access_token"].is_string());
    // payload.user 包含正确的 email
    assert!(v["payload"]["user"]["email"] == "test@aphanite.example.com");
}

#[tokio::test]
async fn general_auth_login_wrong_password() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;

    let (status, v) = do_post(
        &app,
        "/api/auth/login",
        json!({"email": "test@aphanite.example.com", "password": "wrong"}),
    )
    .await;

    // 错误密码 → 403 Forbidden
    assert_eq!(status, StatusCode::FORBIDDEN, "{:?}", v);
    // 响应格式: { success: false, reason: "..." }
    assert!(!v["success"].as_bool().unwrap_or(true));
}

#[tokio::test]
async fn general_auth_login_missing_credentials() {
    let (app, _state, _tmp) = setup().await;

    let (status, v) = do_post(
        &app,
        "/api/auth/login",
        json!({"email": "test@aphanite.example.com"}),
    )
    .await;

    // 缺少 password 和 otp_token → 400 Bad Request
    assert_eq!(status, StatusCode::BAD_REQUEST, "{:?}", v);
}

// ── Auth: Refresh ───────────────────────────────────────────────────────

#[tokio::test]
async fn general_auth_refresh() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;

    let token = login(&app, "test@aphanite.example.com", "pass").await;

    // Spec: POST /auth/refresh, body empty, Auth required → 200 + 新 token
    let (status, v) = do_post_auth(&app, "/api/auth/refresh", &token, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{:?}", v);
    let new_token = v["payload"]["access_token"].as_str().unwrap();
    // 刷新后的 access_token 应不同于原 token
    assert_ne!(new_token, token);
}

// ── Auth: Validate ──────────────────────────────────────────────────────

#[tokio::test]
async fn general_auth_validate() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;

    let token = login(&app, "test@aphanite.example.com", "pass").await;

    // Spec: GET /auth/validate, Auth required → 204 No Content
    let (status, _) = do_get_auth(&app, "/api/auth/validate", &token).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn general_auth_validate_invalid_token() {
    let (app, _state, _tmp) = setup().await;

    // 无效 token → 401 Unauthorized
    let (status, _) = do_get_auth(&app, "/api/auth/validate", "garbage-token").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ── User: Get /users/me ─────────────────────────────────────────────────

#[tokio::test]
async fn general_get_current_user() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;

    let token = login(&app, "test@aphanite.example.com", "pass").await;

    // Spec: GET /users/me, Auth required → 200 + User payload
    let (status, v) = do_get_auth(&app, "/api/users/me", &token).await;
    assert_eq!(status, StatusCode::OK, "{:?}", v);
    // 返回当前认证用户的 email
    assert_eq!(v["payload"]["email"], "test@aphanite.example.com");
}

// ── User: Get /users/{id} ───────────────────────────────────────────────

#[tokio::test]
async fn general_get_user_by_id() {
    let (app, state, _tmp) = setup().await;
    let user = create_test_user(&state, "test@aphanite.example.com").await;

    let token = login(&app, "test@aphanite.example.com", "pass").await;

    // Spec: GET /users/{id}, Auth required → 200 + User payload
    let (status, v) = do_get_auth(&app, &format!("/api/users/{}", user.id), &token).await;
    assert_eq!(status, StatusCode::OK, "{:?}", v);
    // 返回指定 UUID 用户的 email
    assert_eq!(v["payload"]["email"], "test@aphanite.example.com");
}

// ── User: Patch password ────────────────────────────────────────────────

#[tokio::test]
async fn general_patch_current_user_password() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;

    let token = login(&app, "test@aphanite.example.com", "pass").await;

    // Spec: PATCH /users/me/credentials/password → 204 No Content
    let (status, _) = do_patch_auth(
        &app,
        "/api/users/me/credentials/password",
        &token,
        json!({"old_password": "pass", "new_password": "newpass"}),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // 旧密码登录 → 403（密码已变更）
    let (status, _) = do_post(
        &app,
        "/api/auth/login",
        json!({"email": "test@aphanite.example.com", "password": "pass"}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // 新密码登录 → 200（修改成功）
    let (status, _) = do_post(
        &app,
        "/api/auth/login",
        json!({"email": "test@aphanite.example.com", "password": "newpass"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

// ── Profile: Create / Get / Patch / Delete ──────────────────────────────

#[tokio::test]
async fn general_profile_crud() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;

    let token = login(&app, "test@aphanite.example.com", "pass").await;

    // Create profile: POST /profile → 200 + Profile payload
    let (status, v) =
        do_post_auth(&app, "/api/profile", &token, json!({"name": "MyProfile"})).await;
    assert_eq!(status, StatusCode::OK, "{:?}", v);
    let profile_id = v["payload"]["id"].as_str().unwrap();
    // 返回的角色名与请求一致
    assert!(v["payload"]["name"] == "MyProfile");

    // Get profile: GET /profiles/{id} → 200 + { metadata: Profile, skin? }
    let (status, v) = do_get_auth(&app, &format!("/api/profiles/{}", profile_id), &token).await;
    assert_eq!(status, StatusCode::OK, "{:?}", v);
    // metadata.name 匹配创建时的名称
    assert_eq!(v["payload"]["metadata"]["name"], "MyProfile");

    // Patch profile: PATCH /profiles/{id} → 200 + Profile payload
    let (status, v) = do_patch_auth(
        &app,
        &format!("/api/profiles/{}", profile_id),
        &token,
        json!({"name": "RenamedProfile"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{:?}", v);

    // Delete profile: DELETE /profiles/{id} → 200 + 被删除的 Profile payload
    let status = do_delete_auth(&app, &format!("/api/profiles/{}", profile_id), &token).await;
    assert_eq!(status, StatusCode::OK);

    // 删除后查询 → 404 Not Found
    let (status, _) = do_get_auth(&app, &format!("/api/profiles/{}", profile_id), &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── 401 without auth ────────────────────────────────────────────────────

#[tokio::test]
async fn general_requires_auth() {
    let (app, _state, _tmp) = setup().await;

    // 不带 Authorization header → 401 Unauthorized
    let (status, _) = do_get_auth(&app, "/api/users/me", "").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
