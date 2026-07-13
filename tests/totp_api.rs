//! Integration tests for the TOTP (Phenocryst) API.

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

// ── Create TOTP ─────────────────────────────────────────────────────────

#[tokio::test]
async fn totp_create() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;
    let token = login(&app, "test@aphanite.example.com", "pass").await;

    let (status, v) =
        do_post_auth(&app, "/api/users/me/credentials/totp", &token, Value::Null).await;

    // Spec: POST /users/me/credentials/totp, Auth required → 200 + { secret, otpauth_url }
    assert_eq!(status, StatusCode::OK, "{:?}", v);
    // 响应格式: { success: true, payload: { ... } }
    assert!(v["success"].as_bool().unwrap_or(false));
    // payload 包含 TOTP secret 密钥
    assert!(v["payload"]["secret"].is_string());
    // payload 包含 otpauth:// 格式的 URL（供客户端生成二维码）
    assert!(v["payload"]["otpauth_url"].is_string());
    assert!(
        v["payload"]["otpauth_url"]
            .as_str()
            .unwrap()
            .starts_with("otpauth://")
    );
}

// ── Delete TOTP ─────────────────────────────────────────────────────────

#[tokio::test]
async fn totp_delete() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;
    let token = login(&app, "test@aphanite.example.com", "pass").await;

    // 先创建 TOTP
    let (_, v) = do_post_auth(&app, "/api/users/me/credentials/totp", &token, Value::Null).await;
    assert!(v["success"].as_bool().unwrap_or(false));

    // Spec: DELETE /users/me/credentials/totp, Auth required → 204 No Content
    let status = do_delete_auth(&app, "/api/users/me/credentials/totp", &token).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

// ── Verification requires activated TOTP ────────────────────────────────

#[tokio::test]
async fn totp_verification_requires_activation() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;
    let token = login(&app, "test@aphanite.example.com", "pass").await;

    // 创建 TOTP (totp_active 保持 false)
    let (_, v) = do_post_auth(&app, "/api/users/me/credentials/totp", &token, Value::Null).await;
    assert!(v["success"].as_bool().unwrap_or(false));

    // Spec: POST /verification → 400 Bad Request（TOTP 未激活）
    let (status, v) = do_post(
        &app,
        "/api/verification",
        json!({"method": "totp", "email": "test@aphanite.example.com"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{:?}", v);
    // 响应格式: { success: false, reason: "..." }
    assert!(!v["success"].as_bool().unwrap_or(true));
}

// ── Requires auth ───────────────────────────────────────────────────────

#[tokio::test]
async fn totp_requires_auth() {
    let (app, _state, _tmp) = setup().await;

    // 不带 Authorization header → 401 Unauthorized
    let (status, _) = do_post(&app, "/api/users/me/credentials/totp", Value::Null).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
