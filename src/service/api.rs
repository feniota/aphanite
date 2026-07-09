//! "Aphanite General" API endpoints

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::service::yggdrasil::types::GameProfile;
use crate::service::yggdrasil::types::SkinModel;
use crate::{
    AppState,
    service::{ApiResponse, ApiResult, types::ProfilePayload, types::UserPayload},
    types::{Permission, ToPermission as _, Token, User},
};

/// Extract the Bearer token from headers and verify it, returning the user.
pub async fn authenticate(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<User, crate::service::Error> {
    let bearer = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| crate::service::Error::error(401, "Unauthorized"))?;

    let token: Uuid = bearer
        .parse()
        .map_err(|_| crate::service::Error::error(401, "Invalid token"))?;

    state
        .da
        .verify_token(&token, &None)
        .await
        .map_err(|_| crate::service::Error::error(401, "Invalid or expired token"))
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
    if !state.kv.try_consume(&body.email) {
        return Err(crate::service::Error::error(429, "Too many requests"));
    }

    let user = if let Some(password) = body.password {
        state
            .da
            .verify_user(&body.email, &password)
            .await
            .map_err(|_| crate::service::Error::error(403, "Invalid credentials"))?
    } else if let Some(otp_token) = body.otp_token {
        if state.kv.verify_opt_token(&otp_token, &body.email) {
            let mut db = state.da.db().clone();
            User::get_by_email(&mut db, &body.email).await?
        } else {
            return Err(crate::service::Error::error(403, "Invalid credentials"));
        }
    } else {
        return Err(crate::service::Error::error(
            400,
            "password or otp is required",
        ));
    };

    let client_token = Uuid::now_v7().simple().to_string();
    let access_token = state
        .da
        .create_token(&user.id, &client_token, None)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create token: {e}");
            crate::service::Error::error(500, "Internal server error")
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
        .ok_or_else(|| crate::service::Error::error(401, "Unauthorized"))?;

    let access_token: Uuid = bearer
        .parse()
        .map_err(|_| crate::service::Error::error(401, "Invalid token"))?;

    let mut db = state.da.db().clone();
    let token = Token::get_by_access_token(&mut db, &access_token)
        .await
        .map_err(|_| crate::service::Error::error(401, "Invalid or expired token"))?;

    let user = token.user().exec(&mut db).await.map_err(|e| {
        tracing::error!("Failed to load user: {e}");
        crate::service::Error::error(500, "Internal server error")
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
            crate::service::Error::error(500, "Internal server error")
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
) -> Result<StatusCode, crate::service::Error> {
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
) -> Result<Uuid, crate::service::Error> {
    let current_user = authenticate(state, headers).await?;
    match id {
        Some(target)
            if target != current_user.id
                && !current_user.permission.contains(Permission::Management) =>
        {
            Err(crate::service::Error::error(403, "Forbidden"))
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
) -> Result<UserPayload, crate::service::Error> {
    let target_id = resolve_target_id(state, headers, id).await?;

    let mut db = state.da.db().clone();
    let user = User::get_by_id(&mut db, &target_id)
        .await
        .map_err(|_| crate::service::Error::error(404, "User not found"))?;

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
) -> Result<UserPayload, crate::service::Error> {
    let target_id = resolve_target_id(state, headers, id).await?;

    let mut db = state.da.db().clone();
    let mut user = User::get_by_id(&mut db, &target_id)
        .await
        .map_err(|_| crate::service::Error::error(404, "User not found"))?;

    if let Some(new_name) = body.name {
        user.update()
            .nickname(&new_name)
            .exec(&mut db)
            .await
            .map_err(|e| {
                tracing::error!("Failed to update nickname: {e}");
                crate::service::Error::error(500, "Internal server error")
            })?;
    }
    if let Some(new_email) = body.email {
        if User::get_by_email(&mut db, &new_email).await.is_ok() && new_email != user.email {
            return Err(crate::service::Error::error(409, "Email already in use"));
        }
        user.update()
            .email(&new_email)
            .exec(&mut db)
            .await
            .map_err(|e| {
                tracing::error!("Failed to update email: {e}");
                crate::service::Error::error(500, "Internal server error")
            })?;
    }

    let user = User::get_by_id(&mut db, &target_id)
        .await
        .map_err(|_| crate::service::Error::error(500, "Internal server error"))?;

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
) -> Result<StatusCode, crate::service::Error> {
    let target_id = match authenticate(state, headers).await {
        Ok(u) => match id {
            Some(target) => {
                if u.id != target && !u.permission.contains(Permission::Management) {
                    return Err(crate::service::Error::error(403, "Forbidden"));
                }
                target
            }
            None => u.id,
        },
        Err(_) => {
            let target = id.ok_or_else(|| {
                crate::service::Error::error(
                    401,
                    "Unauthorized: authenticate or provide old_password",
                )
            })?;
            let mut db = state.da.db().clone();
            let user = User::get_by_id(&mut db, &target)
                .await
                .map_err(|_| crate::service::Error::error(404, "User not found"))?;
            let target = if let Some(password) = body.old_password {
                state
                    .da
                    .verify_user(&user.email, &password)
                    .await
                    .map_err(|_| crate::service::Error::error(403, "Invalid old password"))?
            } else if let Some(otp_token) = body.otp_token {
                if state.kv.verify_opt_token(&otp_token, &user.email) {
                    let mut db = state.da.db().clone();
                    User::get_by_email(&mut db, &user.email).await?
                } else {
                    return Err(crate::service::Error::error(403, "Invalid otp token"));
                }
            } else {
                return Err(crate::service::Error::error(
                    401,
                    "Unauthorized: provide old_password or otp_token",
                ));
            };
            target.id
        }
    };

    state
        .da
        .update_user_password(&target_id, &body.new_password)
        .await
        .map_err(|e| {
            tracing::error!("Failed to update password: {e}");
            crate::service::Error::error(500, "Internal server error")
        })?;

    Ok(StatusCode::NO_CONTENT)
}

async fn patch_user_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchPasswordRequest>,
) -> Result<StatusCode, crate::service::Error> {
    patch_user_password_inner(&state, &headers, Some(id), body).await
}

async fn patch_current_user_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PatchPasswordRequest>,
) -> Result<StatusCode, crate::service::Error> {
    patch_user_password_inner(&state, &headers, None, body).await
}

// ---- POST /user ----

#[derive(Deserialize)]
struct CreateUserRequest {
    email: String,
    name: Option<String>,
    password: String,
    permissions: Vec<Permission>,
}

async fn create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateUserRequest>,
) -> ApiResult<UserPayload> {
    let current_user = authenticate(&state, &headers).await?;
    if !current_user.permission.contains(Permission::Management) {
        return Err(crate::service::Error::error(403, "Forbidden"));
    }

    let mut db = state.da.db().clone();

    // Check email uniqueness
    if User::get_by_email(&mut db, &body.email).await.is_ok() {
        return Err(crate::service::Error::error(409, "Email already in use"));
    }

    use argon2::password_hash::{PasswordHasher as _, SaltString, rand_core::OsRng};
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = argon2::Argon2::default();
    let hashed_password = argon2
        .hash_password(body.password.as_bytes(), &salt)
        .map_err(|e| {
            tracing::error!("Password hashing failed: {e}");
            crate::service::Error::error(500, "Internal server error")
        })?
        .to_string();

    let nickname = body.name.unwrap_or_else(|| body.email.clone());
    let perm_bits = Permission::to_u32(&body.permissions);

    let user = User::create()
        .email(&body.email)
        .nickname(&nickname)
        .password(&hashed_password)
        .preferred_language("zh_CN")
        .permission(perm_bits)
        .totp_active(false)
        .exec(&mut db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create user: {e}");
            crate::service::Error::error(500, "Internal server error")
        })?;

    Ok(ApiResponse::from(UserPayload::from(user)))
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
            crate::service::Error::error(500, "Internal server error")
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
        .map_err(|_| crate::service::Error::error(404, "Profile not found"))?;

    if profile.owner_id != current_user.id
        && !current_user.permission.contains(Permission::Management)
    {
        return Err(crate::service::Error::error(403, "Forbidden"));
    }

    let payload = ProfilePayload {
        id: profile.id,
        name: profile.name.clone(),
        owner: profile.owner_id,
    };

    GameProfile::delete_by_id(&mut db, &id).await.map_err(|e| {
        tracing::error!("Failed to delete profile: {e}");
        crate::service::Error::error(500, "Internal server error")
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
        .map_err(|_| crate::service::Error::error(404, "Profile not found"))?;

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
        .map_err(|_| crate::service::Error::error(404, "Profile not found"))?;

    if profile.owner_id != current_user.id
        && !current_user.permission.contains(Permission::Management)
    {
        return Err(crate::service::Error::error(403, "Forbidden"));
    }

    if let Some(new_name) = body.name {
        profile
            .update()
            .name(&new_name)
            .exec(&mut db)
            .await
            .map_err(|e| {
                tracing::error!("Failed to update profile name: {e}");
                crate::service::Error::error(500, "Internal server error")
            })?;
    }

    let profile = GameProfile::get_by_id(&mut db, &id)
        .await
        .map_err(|_| crate::service::Error::error(500, "Internal server error"))?;

    Ok(ApiResponse::from(ProfilePayload {
        id: profile.id,
        name: profile.name,
        owner: profile.owner_id,
    }))
}

pub fn router(state: AppState) -> axum::Router {
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
        .route("/profile", post(create_profile))
        .route("/profiles/{id}", get(get_profile))
        .route("/profiles/{id}", delete(delete_profile))
        .route("/profiles/{id}", patch(patch_profile))
        .with_state(state)
}
