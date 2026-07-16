//! "Aphanite General" API endpoints

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::validation::{validate_nickname, validate_password};
use crate::service::yggdrasil::types::GameProfile;
use crate::service::yggdrasil::types::SkinModel;
use crate::{
    AppState,
    service::{ApiResponse, ApiResult, Error, types::ProfilePayload, types::UserPayload},
    types::{Permission, ToPermission as _, Token, User},
};

/// Extract the Bearer token from headers and verify it, returning the user.
pub async fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<User, Error> {
    let bearer = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| Error::error(401, "Unauthorized"))?;

    let token: Uuid = bearer
        .parse()
        .map_err(|_| Error::error(401, "Invalid token"))?;

    state
        .da
        .verify_token(&token, &None)
        .await
        .map_err(|_| Error::error(401, "Invalid or expired token"))
}

// ---- POST /auth/login ----

#[derive(Deserialize)]
struct LoginRequest {
    email: String,
    otp_token: Option<Uuid>,
    password: Option<String>,
}

#[derive(Serialize)]
struct LoginPayload {
    access_token: Uuid,
    client_token: String,
    user: UserPayload,
}

async fn auth_login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> ApiResult<LoginPayload> {
    if !state.kv.try_consume(&body.email).await {
        return Err(Error::error(429, "Too many requests"));
    }

    let user = if let Some(password) = body.password {
        state
            .da
            .verify_user(&body.email, &password)
            .await
            .map_err(|_| Error::error(403, "Invalid credentials"))?
    } else if let Some(otp_token) = body.otp_token {
        if state.kv.verify_opt_token(&otp_token, &body.email).await {
            let mut db = state.da.db().clone();
            User::get_by_email(&mut db, &body.email).await?
        } else {
            return Err(Error::error(403, "Invalid credentials"));
        }
    } else {
        return Err(Error::error(400, "password or otp is required"));
    };

    let client_token = Uuid::now_v7().simple().to_string();
    let access_token = state
        .da
        .create_token(&user.id, &client_token, None)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create token: {e}");
            Error::error(500, "Internal server error")
        })?;

    Ok(ApiResponse::from(LoginPayload {
        access_token,
        client_token,
        user: UserPayload::from(user),
    }))
}

// ---- POST /auth/refresh ----

#[derive(Serialize)]
struct RefreshPayload {
    access_token: Uuid,
    user: UserPayload,
}

async fn auth_refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<RefreshPayload> {
    let bearer = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| Error::error(401, "Unauthorized"))?;

    let access_token: Uuid = bearer
        .parse()
        .map_err(|_| Error::error(401, "Invalid token"))?;

    let mut db = state.da.db().clone();
    let token = Token::get_by_access_token(&mut db, &access_token)
        .await
        .map_err(|_| Error::error(401, "Invalid or expired token"))?;

    let user = token.user().exec(&mut db).await.map_err(|e| {
        tracing::error!("Failed to load user: {e}");
        Error::error(500, "Internal server error")
    })?;

    let client_token = token.client_token.clone();
    let profile_id = token.profile_id;

    // Drop old token, issue a new one with the same client_token
    Token::delete_by_access_token(&mut db, &access_token)
        .await
        .ok();
    let new_access_token = state
        .da
        .create_token(&user.id, &client_token, profile_id.as_ref())
        .await
        .map_err(|e| {
            tracing::error!("Failed to create refreshed token: {e}");
            Error::error(500, "Internal server error")
        })?;

    Ok(ApiResponse::from(RefreshPayload {
        access_token: new_access_token,
        user: UserPayload::from(user),
    }))
}

// ---- GET /auth/validate ----

async fn auth_validate(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, Error> {
    authenticate(&state, &headers).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct PatchUserRequest {
    name: Option<String>,
    email: Option<String>,
}

#[derive(Deserialize)]
struct PatchPasswordRequest {
    otp_token: Option<Uuid>,
    old_password: Option<String>,
    new_password: String,
}

/// Helper: resolve which user ID to act on.
///
/// - `Some(id)` → access-checked (self or Management)
/// - `None`     → current authenticated user
async fn resolve_target_id(
    state: &AppState,
    headers: &HeaderMap,
    id: Option<Uuid>,
) -> Result<Uuid, Error> {
    let current_user = authenticate(state, headers).await?;
    match id {
        Some(target)
            if target != current_user.id
                && !current_user.permission.contains(Permission::Management) =>
        {
            Err(Error::error(403, "Forbidden"))
        }
        Some(target) => Ok(target),
        None => Ok(current_user.id),
    }
}

// ---- GET /users/{id} / GET /users/me ----

async fn get_user_inner(
    state: &AppState,
    headers: &HeaderMap,
    id: Option<Uuid>,
) -> Result<UserPayload, Error> {
    let target_id = resolve_target_id(state, headers, id).await?;

    let mut db = state.da.db().clone();
    let user = User::get_by_id(&mut db, &target_id)
        .await
        .map_err(|_| Error::error(404, "User not found"))?;

    Ok(UserPayload::from(user))
}

async fn get_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<UserPayload> {
    let payload = get_user_inner(&state, &headers, Some(id)).await?;
    Ok(ApiResponse::from(payload))
}

async fn get_current_user(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<UserPayload> {
    let payload = get_user_inner(&state, &headers, None).await?;
    Ok(ApiResponse::from(payload))
}

// ---- PATCH /users/{id} / PATCH /users/me ----

async fn patch_user_inner(
    state: &AppState,
    headers: &HeaderMap,
    id: Option<Uuid>,
    body: PatchUserRequest,
) -> Result<UserPayload, Error> {
    let target_id = resolve_target_id(state, headers, id).await?;

    let mut db = state.da.db().clone();
    let mut user = User::get_by_id(&mut db, &target_id)
        .await
        .map_err(|_| Error::error(404, "User not found"))?;

    if let Some(ref new_name) = body.name {
        validate_nickname(new_name)?;
        user.update()
            .nickname(new_name)
            .exec(&mut db)
            .await
            .map_err(|e| {
                tracing::error!("Failed to update nickname: {e}");
                Error::error(500, "Internal server error")
            })?;
    }
    if let Some(new_email) = body.email {
        if User::get_by_email(&mut db, &new_email).await.is_ok() && new_email != user.email {
            return Err(Error::error(409, "Email already in use"));
        }
        user.update()
            .email(&new_email)
            .exec(&mut db)
            .await
            .map_err(|e| {
                tracing::error!("Failed to update email: {e}");
                Error::error(500, "Internal server error")
            })?;
    }

    let user = User::get_by_id(&mut db, &target_id)
        .await
        .map_err(|_| Error::error(500, "Internal server error"))?;

    Ok(UserPayload::from(user))
}

async fn patch_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchUserRequest>,
) -> ApiResult<UserPayload> {
    let payload = patch_user_inner(&state, &headers, Some(id), body).await?;
    Ok(ApiResponse::from(payload))
}

async fn patch_current_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PatchUserRequest>,
) -> ApiResult<UserPayload> {
    let payload = patch_user_inner(&state, &headers, None, body).await?;
    Ok(ApiResponse::from(payload))
}

// ---- PATCH /users/{id}/credentials/password / PATCH /users/me/credentials/password ----

async fn patch_user_password_inner(
    state: &AppState,
    headers: &HeaderMap,
    id: Option<Uuid>,
    body: PatchPasswordRequest,
) -> Result<StatusCode, Error> {
    let target_id = match authenticate(state, headers).await {
        Ok(u) => match id {
            Some(target) => {
                if u.id != target && !u.permission.contains(Permission::Management) {
                    return Err(Error::error(403, "Forbidden"));
                }
                target
            }
            None => u.id,
        },
        Err(_) => {
            let target = id.ok_or_else(|| {
                Error::error(401, "Unauthorized: authenticate or provide old_password")
            })?;
            let mut db = state.da.db().clone();
            let user = User::get_by_id(&mut db, &target)
                .await
                .map_err(|_| Error::error(404, "User not found"))?;
            let target = if let Some(password) = body.old_password {
                state
                    .da
                    .verify_user(&user.email, &password)
                    .await
                    .map_err(|_| Error::error(403, "Invalid old password"))?
            } else if let Some(otp_token) = body.otp_token {
                if state.kv.verify_opt_token(&otp_token, &user.email).await {
                    let mut db = state.da.db().clone();
                    User::get_by_email(&mut db, &user.email).await?
                } else {
                    return Err(Error::error(403, "Invalid otp token"));
                }
            } else {
                return Err(Error::error(
                    401,
                    "Unauthorized: provide old_password or otp_token",
                ));
            };
            target.id
        }
    };

    validate_password(&body.new_password)?;
    state
        .da
        .update_user_password(&target_id, &body.new_password)
        .await
        .map_err(|e| {
            tracing::error!("Failed to update password: {e}");
            Error::error(500, "Internal server error")
        })?;

    Ok(StatusCode::NO_CONTENT)
}

async fn patch_user_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchPasswordRequest>,
) -> Result<StatusCode, Error> {
    patch_user_password_inner(&state, &headers, Some(id), body).await
}

async fn patch_current_user_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PatchPasswordRequest>,
) -> Result<StatusCode, Error> {
    patch_user_password_inner(&state, &headers, None, body).await
}

// ---- POST /user (admin) ----

#[derive(Deserialize)]
struct CreateUserRequest {
    email: String,
    name: Option<String>,
    permissions: Vec<Permission>,
}

#[derive(Serialize)]
struct CreateUserResponse {
    id: Uuid,
    name: String,
    email: String,
    permissions: Vec<Permission>,
    password: String,
}

async fn create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateUserRequest>,
) -> ApiResult<CreateUserResponse> {
    let current_user = authenticate(&state, &headers).await?;
    if !current_user.permission.contains(Permission::Management) {
        return Err(Error::error(403, "Forbidden"));
    }

    let mut db = state.da.db().clone();

    // Check email uniqueness
    if User::get_by_email(&mut db, &body.email).await.is_ok() {
        return Err(Error::error(409, "Email already in use"));
    }

    use argon2::password_hash::{PasswordHasher as _, SaltString, rand_core::OsRng};

    // Generate a random 24-char hex password
    let plain_password: String = (0..24)
        .map(|_| {
            let hex = rand::random::<u8>();
            format!("{:02x}", hex)
        })
        .collect();

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = argon2::Argon2::default();
    let hashed_password = argon2
        .hash_password(plain_password.as_bytes(), &salt)
        .map_err(|e| {
            tracing::error!("Password hashing failed: {e}");
            Error::error(500, "Internal server error")
        })?
        .to_string();

    let nickname = body.name.unwrap_or_else(|| body.email.clone());
    validate_nickname(&nickname)?;
    validate_password(&plain_password)?;
    let perm_bits = Permission::construct_number(&body.permissions);

    let user = User::create()
        .email(&body.email)
        .nickname(&nickname)
        .password(&hashed_password)
        .preferred_language("zh_CN")
        .permission(perm_bits)
        .exec(&mut db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create user: {e}");
            Error::error(500, "Internal server error")
        })?;

    use crate::types::Permission as P;
    let perms = P::parse_flags(user.permission);
    Ok(ApiResponse::from(CreateUserResponse {
        id: user.id,
        name: user.nickname,
        email: user.email,
        permissions: perms,
        password: plain_password,
    }))
}

// ---- Turnstile verification helper ----

async fn verify_turnstile(client: &reqwest::Client, secret_key: &str, token: &str) -> bool {
    let params = serde_json::json!({"secret": secret_key, "response": token});
    match client
        .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
        .json(&params)
        .send()
        .await
    {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(json) => json
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            Err(_) => false,
        },
        Err(_) => false,
    }
}

// ---- GET /turnstile ----

#[derive(Serialize)]
struct TurnstilePayload {
    site_key: String,
}

async fn get_turnstile(State(state): State<AppState>) -> ApiResult<TurnstilePayload> {
    if !state.cfg.service.public {
        return Err(Error::error(403, "This server is not public"));
    }
    if !state.cfg.service.turnstile.enabled {
        return Err(Error::error(404, "Turnstile is not enabled"));
    }
    Ok(ApiResponse::from(TurnstilePayload {
        site_key: state.cfg.service.turnstile.site_key.clone(),
    }))
}

// ---- POST /register/session (admin creates registration token) ----

#[derive(Deserialize)]
struct CreateRegisterSessionRequest {
    expires_after: u64, // minutes, max 10080 (7 days)
}

#[derive(Serialize)]
struct RegisterSessionPayload {
    token: Uuid,
}

async fn create_register_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateRegisterSessionRequest>,
) -> ApiResult<RegisterSessionPayload> {
    let current_user = authenticate(&state, &headers).await?;
    if !current_user.permission.contains(Permission::Management) {
        return Err(Error::error(403, "Forbidden"));
    }

    let max_minutes: u64 = 10080;
    let expires_after = body.expires_after.min(max_minutes);
    use jiff::ToSpan;
    let expires_at = jiff::Timestamp::now() + (expires_after as i64).minutes();

    let mut db = state.da.db().clone();
    let token = crate::types::RegisterToken::create()
        .expires_at(expires_at)
        .exec(&mut db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create register token: {e}");
            Error::error(500, "Internal server error")
        })?;

    Ok(ApiResponse::from(RegisterSessionPayload {
        token: token.id,
    }))
}

// ---- POST /register (user self-registration) ----

#[derive(Deserialize)]
struct RegisterRequest {
    register_token: Option<Uuid>,
    turnstile_token: Option<String>,
    email: String,
    name: Option<String>,
    password: String,
}

async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> ApiResult<UserPayload> {
    let is_public = state.cfg.service.public;
    let turnstile_enabled = state.cfg.service.turnstile.enabled;

    // --- Auth checks ---
    if is_public {
        if turnstile_enabled {
            match (&body.register_token, &body.turnstile_token) {
                (None, None) => {
                    return Err(Error::error(
                        400,
                        "Either register_token or turnstile_token is required",
                    ));
                }
                (_, Some(ts))
                    if !verify_turnstile(
                        &state.http_client,
                        &state.cfg.service.turnstile.secret_key,
                        ts,
                    )
                    .await =>
                {
                    return Err(Error::error(422, "Turnstile verification failed"));
                }
                _ => {}
            }
            if let Some(ref reg_token) = body.register_token {
                let mut db = state.da.db().clone();
                if !verify_register_token(&mut db, reg_token).await {
                    return Err(Error::error(403, "Invalid or expired register token"));
                }
            }
        }
        // else: no auth check when public & turnstile disabled
    } else {
        // Private instance — register_token is mandatory
        let reg_token = body
            .register_token
            .ok_or_else(|| Error::error(400, "register_token is required on private instances"))?;
        let mut db = state.da.db().clone();
        if !verify_register_token(&mut db, &reg_token).await {
            return Err(Error::error(403, "Invalid or expired register token"));
        }
    }

    // --- Validation ---
    validate_password(&body.password)?;
    let nickname = body.name.as_deref().unwrap_or(&body.email);
    validate_nickname(nickname)?;

    // --- Create user ---
    let mut db = state.da.db().clone();

    if User::get_by_email(&mut db, &body.email).await.is_ok() {
        return Err(Error::error(409, "Email already in use"));
    }

    use argon2::password_hash::{PasswordHasher as _, SaltString, rand_core::OsRng};
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = argon2::Argon2::default();
    let hashed_password = argon2
        .hash_password(body.password.as_bytes(), &salt)
        .map_err(|e| {
            tracing::error!("Password hashing failed: {e}");
            Error::error(500, "Internal server error")
        })?
        .to_string();

    let user = User::create()
        .email(&body.email)
        .nickname(nickname)
        .password(&hashed_password)
        .preferred_language("zh_CN")
        .permission(0)
        .exec(&mut db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create user: {e}");
            Error::error(500, "Internal server error")
        })?;

    Ok(ApiResponse::from(UserPayload::from(user)))
}

/// Check whether a register token exists and is still valid.
/// Does NOT consume the token (allows reuse within its lifetime).
async fn verify_register_token(db: &mut toasty::Db, token_id: &Uuid) -> bool {
    match crate::types::RegisterToken::get_by_id(db, token_id).await {
        Ok(tok) if tok.expires_at > jiff::Timestamp::now() => {
            // Consume the token — one-time use
            let _ = crate::types::RegisterToken::delete_by_id(db, token_id).await;
            true
        }
        _ => false,
    }
}

// ---- POST /profile ----

#[derive(Deserialize)]
struct CreateProfileRequest {
    name: String,
}

async fn create_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateProfileRequest>,
) -> ApiResult<ProfilePayload> {
    let current_user = authenticate(&state, &headers).await?;

    let mut db = state.da.db().clone();
    let profile = GameProfile::create()
        .name(&body.name)
        .owner_id(current_user.id)
        .exec(&mut db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create profile: {e}");
            Error::error(500, "Internal server error")
        })?;

    Ok(ApiResponse::from(ProfilePayload {
        id: profile.id,
        name: profile.name,
        owner: profile.owner_id,
    }))
}

// ---- DELETE /profiles/{id} ----

async fn delete_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<ProfilePayload> {
    let current_user = authenticate(&state, &headers).await?;

    let mut db = state.da.db().clone();
    let profile = GameProfile::get_by_id(&mut db, &id)
        .await
        .map_err(|_| Error::error(404, "Profile not found"))?;

    if profile.owner_id != current_user.id
        && !current_user.permission.contains(Permission::Management)
    {
        return Err(Error::error(403, "Forbidden"));
    }

    let payload = ProfilePayload {
        id: profile.id,
        name: profile.name.clone(),
        owner: profile.owner_id,
    };

    GameProfile::delete_by_id(&mut db, &id).await.map_err(|e| {
        tracing::error!("Failed to delete profile: {e}");
        Error::error(500, "Internal server error")
    })?;

    Ok(ApiResponse::from(payload))
}

// ---- GET /profiles/{id}?with_skin ----

#[derive(Deserialize)]
struct GetProfileParams {
    with_skin: Option<bool>,
}

#[derive(Serialize)]
struct ProfileDetail {
    metadata: ProfilePayload,
    #[serde(skip_serializing_if = "Option::is_none")]
    skin: Option<ProfileSkinPayload>,
}

#[derive(Serialize)]
struct ProfileSkinPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    skin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<SkinModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cape: Option<String>,
}

async fn get_profile(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(params): Query<GetProfileParams>,
) -> ApiResult<ProfileDetail> {
    let mut db = state.da.db().clone();
    let profile = GameProfile::get_by_id(&mut db, &id)
        .await
        .map_err(|_| Error::error(404, "Profile not found"))?;

    let skin = if params.with_skin.unwrap_or(false) {
        if let Ok(Some(textures)) = profile.textures().exec(&mut db).await {
            let skin_url = match textures.skin_file {
                Some(f) => state.assets.get_url(f).await,
                None => None,
            };
            let cape_url = match textures.cape_file {
                Some(f) => state.assets.get_url(f).await,
                None => None,
            };
            Some(ProfileSkinPayload {
                skin: skin_url,
                model: Some(textures.skin_model),
                cape: cape_url,
            })
        } else {
            None
        }
    } else {
        None
    };

    Ok(ApiResponse::from(ProfileDetail {
        metadata: ProfilePayload {
            id: profile.id,
            name: profile.name,
            owner: profile.owner_id,
        },
        skin,
    }))
}

// ---- PATCH /profiles/{id} ----

#[derive(Deserialize)]
struct PatchProfileRequest {
    name: Option<String>,
}

async fn patch_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchProfileRequest>,
) -> ApiResult<ProfilePayload> {
    let current_user = authenticate(&state, &headers).await?;

    let mut db = state.da.db().clone();
    let mut profile = GameProfile::get_by_id(&mut db, &id)
        .await
        .map_err(|_| Error::error(404, "Profile not found"))?;

    if profile.owner_id != current_user.id
        && !current_user.permission.contains(Permission::Management)
    {
        return Err(Error::error(403, "Forbidden"));
    }

    if let Some(new_name) = body.name {
        profile
            .update()
            .name(&new_name)
            .exec(&mut db)
            .await
            .map_err(|e| {
                tracing::error!("Failed to update profile name: {e}");
                Error::error(500, "Internal server error")
            })?;
    }

    let profile = GameProfile::get_by_id(&mut db, &id)
        .await
        .map_err(|_| Error::error(500, "Internal server error"))?;

    Ok(ApiResponse::from(ProfilePayload {
        id: profile.id,
        name: profile.name,
        owner: profile.owner_id,
    }))
}

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{delete, get, patch, post};
    axum::Router::new()
        .route("/auth/login", post(auth_login))
        .route("/auth/refresh", post(auth_refresh))
        .route("/auth/validate", get(auth_validate))
        .route("/users/{id}", get(get_user))
        .route("/users/{id}", patch(patch_user))
        .route(
            "/users/{id}/credentials/password",
            patch(patch_user_password),
        )
        .route("/users/me", get(get_current_user))
        .route("/users/me", patch(patch_current_user))
        .route(
            "/users/me/credentials/password",
            patch(patch_current_user_password),
        )
        .route("/user", post(create_user))
        .route("/turnstile", get(get_turnstile))
        .route("/register/session", post(create_register_session))
        .route("/register", post(register))
        .route("/profile", post(create_profile))
        .route("/profiles/{id}", get(get_profile))
        .route("/profiles/{id}", delete(delete_profile))
        .route("/profiles/{id}", patch(patch_profile))
}
