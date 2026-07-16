//! Integration tests for the TOTP (Phenocryst) API — full flow.

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::{create_test_user, login, new_test_state};
use serde_json::{Value, json};
use tempfile::TempDir;
use totp_rs::{Algorithm, Secret, TOTP};
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
    let token = login(&app, "test@aphanite.example.com", "password123").await;

    let (status, v) =
        do_post_auth(&app, "/api/users/me/credentials/totp", &token, Value::Null).await;

    // Spec: POST /users/me/credentials/totp, Auth required → 200 + { secret, otpauth_url }
    assert_eq!(status, StatusCode::OK, "{:?}", v);
    assert!(v["success"].as_bool().unwrap_or(false));
    assert!(v["payload"]["secret"].is_string());
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
    let token = login(&app, "test@aphanite.example.com", "password123").await;

    // Create TOTP first
    let (_, v) = do_post_auth(&app, "/api/users/me/credentials/totp", &token, Value::Null).await;
    assert!(v["success"].as_bool().unwrap_or(false));

    // Spec: DELETE /users/me/credentials/totp, Auth required → 204 No Content
    let status = do_delete_auth(&app, "/api/users/me/credentials/totp", &token).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

// ── Full TOTP flow: create → verify → login with OTP ────────────────────

#[tokio::test]
async fn totp_full_flow_create_verify_login() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;
    let token = login(&app, "test@aphanite.example.com", "password123").await;

    // Step 1: Create TOTP
    let (status, v) =
        do_post_auth(&app, "/api/users/me/credentials/totp", &token, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{:?}", v);
    let secret = v["payload"]["secret"].as_str().unwrap().to_string();

    // Step 2: Generate a valid TOTP code from the secret
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        Secret::Raw(secret.clone().into_bytes()).to_bytes().unwrap(),
        Some("Aphanite".to_string()),
        "test@aphanite.example.com".to_string(),
    )
    .unwrap();
    let code = totp.generate_current().unwrap();

    // Step 3: Create verification session
    let (status, v) = do_post(
        &app,
        "/api/verification",
        json!({"method": "totp", "email": "test@aphanite.example.com"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{:?}", v);
    assert!(v["success"].as_bool().unwrap_or(false));
    let session_id = v["payload"]["id"].as_str().unwrap().to_string();

    // Step 4: Complete verification with the TOTP code
    let (status, v) = do_post(
        &app,
        &format!("/api/verification/{}", session_id),
        json!({"code": code}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{:?}", v);
    assert!(v["success"].as_bool().unwrap_or(false));
    let otp_token = v["payload"]["otp_token"].as_str().unwrap().to_string();
    assert!(!otp_token.is_empty());

    // Step 5: Login using the OTP token (no password)
    let (status, v) = do_post(
        &app,
        "/api/auth/login",
        json!({"email": "test@aphanite.example.com", "otp_token": otp_token}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{:?}", v);
    assert!(v["success"].as_bool().unwrap_or(false));
    assert!(v["payload"]["access_token"].is_string());
    assert_eq!(v["payload"]["user"]["email"], "test@aphanite.example.com");
}

// ── Verification fails with wrong TOTP code ─────────────────────────────

#[tokio::test]
async fn totp_verification_wrong_code() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;
    let token = login(&app, "test@aphanite.example.com", "password123").await;

    // Create TOTP
    let (_, v) = do_post_auth(&app, "/api/users/me/credentials/totp", &token, Value::Null).await;
    assert!(v["success"].as_bool().unwrap_or(false));

    // Create verification session
    let (status, v) = do_post(
        &app,
        "/api/verification",
        json!({"method": "totp", "email": "test@aphanite.example.com"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let session_id = v["payload"]["id"].as_str().unwrap().to_string();

    // Submit a wrong code
    let (status, v) = do_post(
        &app,
        &format!("/api/verification/{}", session_id),
        json!({"code": "000000"}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{:?}", v);
    assert!(!v["success"].as_bool().unwrap_or(true));
}

// ── Verification fails after TOTP is deleted ────────────────────────────

#[tokio::test]
async fn totp_verification_fails_after_delete() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;
    let token = login(&app, "test@aphanite.example.com", "password123").await;

    // Create TOTP
    let (_, v) = do_post_auth(&app, "/api/users/me/credentials/totp", &token, Value::Null).await;
    assert!(v["success"].as_bool().unwrap_or(false));

    // Delete TOTP
    let status = do_delete_auth(&app, "/api/users/me/credentials/totp", &token).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verification now fails because no TOTP secret is available
    let (status, v) = do_post(
        &app,
        "/api/verification",
        json!({"method": "totp", "email": "test@aphanite.example.com"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{:?}", v);
    assert!(!v["success"].as_bool().unwrap_or(true));
}

// ── Login with invalid OTP token fails ──────────────────────────────────

#[tokio::test]
async fn totp_login_invalid_otp_token() {
    let (app, _state, _tmp) = setup().await;

    // Try to login with a random UUID as OTP token
    let (status, v) = do_post(
        &app,
        "/api/auth/login",
        json!({"email": "nobody@example.com", "otp_token": "00000000-0000-0000-0000-000000000000"}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{:?}", v);
    assert!(!v["success"].as_bool().unwrap_or(true));
}

// ── No TOTP secret: verification returns error ──────────────────────────

#[tokio::test]
async fn totp_verification_no_secret() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "no-totp@aphanite.example.com").await;

    // User has no TOTP secret → verification should fail
    let (status, v) = do_post(
        &app,
        "/api/verification",
        json!({"method": "totp", "email": "no-totp@aphanite.example.com"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{:?}", v);
    assert!(!v["success"].as_bool().unwrap_or(true));
}

// ── Requires auth ───────────────────────────────────────────────────────

#[tokio::test]
async fn totp_requires_auth() {
    let (app, _state, _tmp) = setup().await;

    // No Authorization header → 401 Unauthorized
    let (status, _) = do_post(&app, "/api/users/me/credentials/totp", Value::Null).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
