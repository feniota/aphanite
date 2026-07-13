//! Integration tests for the Yggdrasil authentication API.

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{create_test_profile, create_test_user, new_test_state};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

/// Build a test Yggdrasil router with an in-memory SQLite database.
/// Returns the router, the AppState (for seeding data), and the temp dir guard.
async fn setup() -> (Router, aphanite::AppState, TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let state = new_test_state(tmp.path())
        .await
        .expect("failed to build test AppState");
    let router = aphanite::service::yggdrasil::router().with_state(state.clone());
    (router, state, tmp)
}

/// Helper: POST JSON to the router, return (status, body).
async fn post_json(app: &Router, uri: &str, body: Value) -> (StatusCode, String) {
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
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&body).to_string())
}

/// Helper: GET from the router, return (status, body).
async fn get(app: &Router, uri: &str) -> (StatusCode, String) {
    let response = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&body).to_string())
}

/// Authenticate and return the parsed JSON body as a `Value`.
async fn do_authenticate(app: &Router, username: &str, password: &str) -> Value {
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
    assert_eq!(status, StatusCode::OK, "authenticate failed: {}", body);
    serde_json::from_str(&body).unwrap()
}

/// Extract the `error` field from a Yggdrasil error JSON body.
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

#[tokio::test]
async fn yggdrasil_meta() {
    let (app, _state, _tmp) = setup().await;
    let (status, body) = get(&app, "/").await;

    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert!(v.get("meta").is_some(), "{}", body);
    assert!(v.get("signaturePublickey").is_some(), "{}", body);
}

// ── Authenticate ────────────────────────────────────────────────────────

#[tokio::test]
async fn yggdrasil_authenticate_success() {
    let (app, state, _tmp) = setup().await;

    let user = create_test_user(&state, "test@aphanite.example.com").await;
    let _profile = create_test_profile(&state, user.id, "TestPlayer").await;

    let (status, body) = post_json(
        &app,
        "/authserver/authenticate",
        json!({
            "username": "test@aphanite.example.com",
            "password": "pass",
            "clientToken": "client-abc",
            "requestUser": true,
            "agent": {"name": "Minecraft", "version": 1}
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{}", body);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["clientToken"], "client-abc");

    let profiles = v["availableProfiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0]["name"], "TestPlayer");

    assert!(v["selectedProfile"].is_object());
    assert_eq!(v["selectedProfile"]["name"], "TestPlayer");

    assert!(v["user"].is_object());
}

#[tokio::test]
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

    assert_eq!(status, StatusCode::FORBIDDEN, "{}", body);
    assert_eq!(error_type(&body), "ForbiddenOperationException");
}

#[tokio::test]
async fn yggdrasil_authenticate_nonexistent_user() {
    let (app, _state, _tmp) = setup().await;

    let (status, body) = post_json(
        &app,
        "/authserver/authenticate",
        json!({
            "username": "nobody@nowhere.com",
            "password": "pass",
            "requestUser": false,
            "agent": {"name": "Minecraft", "version": 1}
        }),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN, "{}", body);
    assert_eq!(error_type(&body), "ForbiddenOperationException");
}

// ── Refresh ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn yggdrasil_refresh_success() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;

    let auth = do_authenticate(&app, "test@aphanite.example.com", "pass").await;
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

    assert_eq!(status, StatusCode::OK, "{}", body);
    let v: Value = serde_json::from_str(&body).unwrap();
    let new_token = v["accessToken"].as_str().unwrap();
    assert_ne!(new_token, old_token);
}

#[tokio::test]
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

    assert_eq!(status, StatusCode::FORBIDDEN, "{}", body);
    assert_eq!(error_type(&body), "ForbiddenOperationException");
}

// ── Validate ────────────────────────────────────────────────────────────

#[tokio::test]
async fn yggdrasil_validate_success() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;

    let auth = do_authenticate(&app, "test@aphanite.example.com", "pass").await;
    let token = auth["accessToken"].as_str().unwrap();

    let (status, _) = post_json(
        &app,
        "/authserver/validate",
        json!({ "accessToken": token, "clientToken": "client-abc" }),
    )
    .await;

    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn yggdrasil_validate_invalid_token() {
    let (app, _state, _tmp) = setup().await;

    let fake_token = Uuid::now_v7().simple().to_string();
    let (status, body) = post_json(
        &app,
        "/authserver/validate",
        json!({ "accessToken": fake_token }),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN, "{}", body);
}

// ── Invalidate ──────────────────────────────────────────────────────────

#[tokio::test]
async fn yggdrasil_invalidate() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;

    let auth = do_authenticate(&app, "test@aphanite.example.com", "pass").await;
    let token = auth["accessToken"].as_str().unwrap();

    // Invalidate
    let (status, _) = post_json(
        &app,
        "/authserver/invalidate",
        json!({ "accessToken": token }),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Token should no longer validate
    let (status, _) = post_json(
        &app,
        "/authserver/validate",
        json!({ "accessToken": token }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ── Signout ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn yggdrasil_signout() {
    let (app, state, _tmp) = setup().await;
    create_test_user(&state, "test@aphanite.example.com").await;

    let auth = do_authenticate(&app, "test@aphanite.example.com", "pass").await;
    let token = auth["accessToken"].as_str().unwrap();

    // Signout
    let (status, _) = post_json(
        &app,
        "/authserver/signout",
        json!({ "username": "test@aphanite.example.com", "password": "pass" }),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Token should no longer validate
    let (status, _) = post_json(
        &app,
        "/authserver/validate",
        json!({ "accessToken": token }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ── Join + HasJoined ────────────────────────────────────────────────────

#[tokio::test]
async fn yggdrasil_join_and_has_joined() {
    let (app, state, _tmp) = setup().await;

    let user = create_test_user(&state, "test@aphanite.example.com").await;
    let profile = create_test_profile(&state, user.id, "TestPlayer").await;

    let auth = do_authenticate(&app, "test@aphanite.example.com", "pass").await;
    let access_token = auth["accessToken"].as_str().unwrap();
    let server_id = Uuid::now_v7().to_string();

    // Join
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

    // HasJoined
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
    assert_eq!(v["name"], "TestPlayer");
}

// ── Profile ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn yggdrasil_profile() {
    let (app, state, _tmp) = setup().await;

    let user = create_test_user(&state, "test@aphanite.example.com").await;
    let profile = create_test_profile(&state, user.id, "TestPlayer").await;

    let (status, body) = get(
        &app,
        &format!("/sessionserver/session/minecraft/profile/{}", profile.id),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{}", body);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["name"], "TestPlayer");

    let props = v["properties"].as_array().unwrap();
    let textures = props.iter().find(|p| p["name"] == "textures");
    assert!(textures.is_some(), "missing textures property in {}", body);
}
