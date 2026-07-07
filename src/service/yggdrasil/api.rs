//! Specific API endpoints implementation

use super::types::{ExchangeableGameProfile, ProfileTextures, SkinModel, UnhyphenatedUuid};
use crate::AppState;
use crate::service::yggdrasil::types::AphaniteClientIp;
use crate::types::User;
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use rsa::pkcs8::{EncodePublicKey, LineEnding};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::net::IpAddr;
use tokio_stream::StreamExt;
use tracing::{debug, info, warn};
use uuid::Uuid;

const QUERY_PROFILE_LIMIT: usize = 50;
const MAX_TEXTURE_UPLOAD_SIZE: usize = 8 * 1024 * 1024; // 8 MB

/// Truncate a UUID for logging: show first 8 hex chars
fn trunc_uuid(u: &Uuid) -> String {
    let s = u.simple().to_string();
    format!("{}…", &s[..8])
}

pub type Result<T> = std::result::Result<T, YggdrasilError>;

/// Error type defined by the authlib-injector Yggdrasil doc
#[derive(Debug)]
pub enum YggdrasilError {
    /// General HTTP error
    HttpError(StatusCode),

    /// Invalid token or trying to join a server with a wrong profile
    InvalidToken,

    /// Invalid username or password
    InvalidCredentials,

    // /// Attempt to assign profile to a profile-assigned token
    // IllegalArgument,
    /// Attempt to assign unrelated profile or other general forbidden operations
    ForbiddenOperation,

    /// General error that is not covered by the doc
    ///
    /// One should ALWAYS prefer the try (`?`) operator where available rather than directly construct this variant.
    Other(String),
}

/// "Error Message" of the error
impl std::fmt::Display for YggdrasilError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            YggdrasilError::HttpError(s) => {
                write!(f, "{}", s.canonical_reason().unwrap_or("Error"))
            }
            YggdrasilError::InvalidToken => write!(f, "Invalid token."),
            YggdrasilError::InvalidCredentials => {
                write!(f, "Invalid credentials. Invalid username or password.")
            }
            // YggdrasilError::IllegalArgument => {
            //     write!(f, "Access token already has a profile assigned.")
            // }
            YggdrasilError::ForbiddenOperation => write!(f, "An error has occurred."),
            YggdrasilError::Other(msg) => write!(f, "An error has occurred: {}", msg),
        }
    }
}

impl<E> From<E> for YggdrasilError
where
    E: Error,
{
    fn from(error: E) -> Self {
        let str = error.to_string();
        tracing::warn!("Unexpected error occurred: {}", str);
        Self::Other(str)
    }
}

impl YggdrasilError {
    pub(super) fn http(status_code: u16) -> Self {
        Self::HttpError(StatusCode::from_u16(status_code).unwrap())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub error: String,
    pub error_message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

impl ErrorResponse {
    fn new(error: &YggdrasilError, error_id: impl Into<String>) -> Self {
        Self {
            error: error_id.into(),
            error_message: format!("{}", error),
            cause: None,
        }
    }
}

impl IntoResponse for YggdrasilError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            YggdrasilError::HttpError(status) => {
                if matches!(status, StatusCode::NO_CONTENT) {
                    return (status).into_response();
                } else {
                    (
                        status,
                        ErrorResponse::new(&self, status.canonical_reason().unwrap_or("Error")),
                    )
                }
            }

            YggdrasilError::InvalidToken => (
                StatusCode::FORBIDDEN,
                ErrorResponse::new(&self, "ForbiddenOperationException"),
            ),

            YggdrasilError::InvalidCredentials => (
                StatusCode::FORBIDDEN,
                ErrorResponse::new(&self, "ForbiddenOperationException"),
            ),

            // YggdrasilError::IllegalArgument => (
            //     StatusCode::BAD_REQUEST,
            //     ErrorResponse::new(&self, "IllegalArgumentException"),
            // ),
            YggdrasilError::ForbiddenOperation => (
                StatusCode::FORBIDDEN,
                ErrorResponse::new(&self, "ForbiddenOperationException"),
            ),

            YggdrasilError::Other(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorResponse {
                    error: "Internal Server Error".into(),
                    error_message: format!("An error has occurred: {}", &msg),
                    cause: Some(msg),
                },
            ),
        };

        (status, Json(body)).into_response()
    }
}

// POST /authserver/authenticate

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestAuthenticate {
    username: String,
    password: String,
    client_token: Option<String>,
    request_user: bool,
    agent: AuthenticateAgent,
}

#[derive(Deserialize)]
struct AuthenticateAgent {
    name: String,
    version: isize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseAuthenticate {
    access_token: UnhyphenatedUuid,
    client_token: String,
    available_profiles: Vec<ExchangeableGameProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    selected_profile: Option<ExchangeableGameProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<UserProfile>,
}

#[derive(Serialize)]
struct UserProfile {
    id: UnhyphenatedUuid,
    properties: Vec<UserProperty>,
}

#[derive(Serialize)]
struct UserProperty {
    name: &'static str,
    value: String,
}

async fn create_authenticate(
    user: User,
    client_token: Option<String>,
    state: &AppState,
    request_user: bool,
    selected_profile: Option<ExchangeableGameProfile>,
) -> Result<ResponseAuthenticate> {
    let client_token = client_token.unwrap_or_else(|| Uuid::now_v7().simple().to_string());

    let db = state.da.db().clone();
    let available_profiles = tokio_stream::iter(
        state
            .da
            .query_profile_by_user(&user.id)
            .await
            .map_err(|e| YggdrasilError::Other(e.to_string()))?,
    )
    .then(|x| {
        let assets = state.assets.clone();
        let mut db = db.clone();
        async move { ExchangeableGameProfile::new(&mut db, assets, &x, false, None).await }
    })
    .collect::<Vec<_>>()
    .await;

    let selected_profile = if let Some(v) = selected_profile {
        Some(v)
    } else if available_profiles.len() > 1 {
        None
    } else {
        available_profiles.first().map(|t| t.clone())
    };

    let access_token = state
        .da
        .create_token(
            &user.id,
            &client_token,
            selected_profile
                .as_ref()
                .map(|t| Uuid::from(t.id.clone()))
                .as_ref(),
        )
        .await
        .map_err(|e| YggdrasilError::Other(e.to_string()))?
        .into();

    let user = if request_user {
        Some(UserProfile {
            id: user.id.into(),
            properties: vec![UserProperty {
                name: "preferredLanguage",
                value: user.preferred_language,
            }],
        })
    } else {
        None
    };

    Ok(ResponseAuthenticate {
        access_token,
        client_token,
        available_profiles,
        selected_profile,
        user,
    })
}

pub async fn authenticate(
    State(state): State<AppState>,
    Json(body): Json<RequestAuthenticate>,
) -> Result<(StatusCode, Json<ResponseAuthenticate>)> {
    info!(
        "authenticate: user: {}, client: {} v{}",
        body.username, body.agent.name, body.agent.version
    );

    if !state.kv.try_consume(body.username.clone()) {
        warn!("authenticate: rate-limited: user={}", body.username);
        return Err(YggdrasilError::InvalidCredentials);
    }

    let user = state
        .da
        .verify_user(&body.username, &body.password)
        .await
        .map_err(|_| {
            warn!("authenticate: invalid credentials: user={}", body.username);
            YggdrasilError::InvalidCredentials
        })?;

    info!("authenticate: success: user={}", body.username);
    Ok((
        StatusCode::OK,
        create_authenticate(user, body.client_token, &state, body.request_user, None)
            .await?
            .into(),
    ))
}

// POST /authserver/refresh

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestRefresh {
    access_token: UnhyphenatedUuid,
    client_token: Option<String>,
    request_user: bool,
    selected_profile: Option<ExchangeableGameProfile>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseRefresh {
    access_token: UnhyphenatedUuid,
    client_token: String,
    selected_profile: Option<ExchangeableGameProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<UserProfile>,
}

pub async fn refresh(
    State(state): State<AppState>,
    Json(body): Json<RequestRefresh>,
) -> Result<(StatusCode, Json<ResponseRefresh>)> {
    let access_token: Uuid = body.access_token.into();
    info!("refresh: access_token={}", trunc_uuid(&access_token));

    let user = state
        .da
        .verify_token(&access_token, &body.client_token)
        .await
        .map_err(|_| {
            warn!(
                "refresh: invalid token: access_token={}",
                trunc_uuid(&access_token)
            );
            YggdrasilError::InvalidToken
        })?;

    state
        .da
        .delete_token(&access_token)
        .await
        .map_err(|e| YggdrasilError::Other(e.to_string()))?;

    let new_authenticate = create_authenticate(
        user,
        body.client_token,
        &state,
        body.request_user,
        body.selected_profile,
    )
    .await?;

    info!(
        "refresh: success: access_token={}",
        trunc_uuid(&access_token)
    );
    Ok((
        StatusCode::OK,
        ResponseRefresh {
            access_token: new_authenticate.access_token,
            client_token: new_authenticate.client_token,
            selected_profile: new_authenticate.selected_profile,
            user: new_authenticate.user,
        }
        .into(),
    ))
}

// POST /authserver/validate

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestValidate {
    access_token: UnhyphenatedUuid,
    client_token: Option<String>,
}

pub async fn validate(
    State(state): State<AppState>,
    Json(body): Json<RequestValidate>,
) -> Result<StatusCode> {
    let access_token: Uuid = body.access_token.into();
    info!("validate: access_token={}", trunc_uuid(&access_token));

    state
        .da
        .verify_token(&access_token, &body.client_token)
        .await
        .map_err(|_| {
            warn!(
                "validate: invalid token: access_token={}",
                trunc_uuid(&access_token)
            );
            YggdrasilError::InvalidToken
        })?;
    info!(
        "validate: success: access_token={}",
        trunc_uuid(&access_token)
    );
    Ok(StatusCode::NO_CONTENT)
}

// POST /authserver/invalidate

pub async fn invalidate(
    State(state): State<AppState>,
    Json(body): Json<RequestValidate>,
) -> Result<StatusCode> {
    let access_token: Uuid = body.access_token.into();
    info!("invalidate: access_token={}", trunc_uuid(&access_token));

    if let Err(e) = state.da.delete_token(&access_token).await {
        warn!(
            "invalidate: delete failed (token may already be gone): access_token={}: {e}",
            trunc_uuid(&access_token)
        );
    }
    info!(
        "invalidate: success: access_token={}",
        trunc_uuid(&access_token)
    );
    Ok(StatusCode::NO_CONTENT)
}

// POST /authserver/signout

#[derive(Deserialize)]
pub struct RequestSignout {
    username: String,
    password: String,
}

pub async fn signout(
    State(state): State<AppState>,
    Json(body): Json<RequestSignout>,
) -> Result<StatusCode> {
    info!("signout: user={}", body.username);

    if !state.kv.try_consume(body.username.clone()) {
        warn!("signout: rate-limited: user={}", body.username);
        return Err(YggdrasilError::InvalidCredentials);
    }

    let user = state
        .da
        .verify_user(&body.username, &body.password)
        .await
        .map_err(|_| {
            warn!("signout: invalid credentials: user={}", body.username);
            YggdrasilError::InvalidCredentials
        })?;

    state
        .da
        .clear_token(&user.id)
        .await
        .map_err(|e| YggdrasilError::Other(e.to_string()))?;

    info!("signout: success: user={}", body.username);
    Ok(StatusCode::NO_CONTENT)
}

// POST /sessionserver/session/minecraft/join

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestJoin {
    pub access_token: UnhyphenatedUuid,
    pub selected_profile: UnhyphenatedUuid,
    pub server_id: String,
}

pub struct Session {
    pub profile_id: Uuid,
    pub server_id: String,
    pub access_token: Uuid,
    pub ip: IpAddr,
}

pub async fn join(
    State(state): State<AppState>,
    AphaniteClientIp(ip): AphaniteClientIp,
    Json(body): Json<RequestJoin>,
) -> Result<StatusCode> {
    let access_token: Uuid = body.access_token.into();
    let profile_id: Uuid = body.selected_profile.into();
    info!(
        "join: access_token={}, profile_id={}, server_id={}, ip={}",
        trunc_uuid(&access_token),
        trunc_uuid(&profile_id),
        body.server_id,
        ip,
    );

    let _ = state
        .da
        .match_profile(&access_token, &profile_id)
        .await
        .map_err(|_| {
            warn!(
                "join: forbidden: access_token={}, profile_id={}",
                trunc_uuid(&access_token),
                trunc_uuid(&profile_id)
            );
            YggdrasilError::ForbiddenOperation
        })?;

    state.kv.record_session(Session {
        profile_id,
        server_id: body.server_id,
        access_token,
        ip,
    });

    info!("join: success: profile_id={}", trunc_uuid(&profile_id));
    Ok(StatusCode::NO_CONTENT)
}

// GET /sessionserver/session/minecraft/hasJoined?username={username}&serverId={serverId}&ip={ip}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HasJoinedParams {
    username: String,
    server_id: String,
    ip: Option<String>,
}

pub async fn has_joined(
    State(state): State<AppState>,
    Query(params): Query<HasJoinedParams>,
) -> Result<(StatusCode, Json<ExchangeableGameProfile>)> {
    info!(
        "has_joined: username={}, server_id={}, ip={:?}",
        params.username, params.server_id, params.ip
    );
    if let Some(session) = state.kv.query_session(&params.server_id) {
        state
            .da
            .verify_token(&session.access_token, &None)
            .await
            .map_err(|_| YggdrasilError::http(204))?;

        if let Some(ip) = params.ip
            && !state.cfg.service.client_ip.is_disabled()
        {
            let ip: IpAddr = ip.parse().map_err(|_| YggdrasilError::http(400))?;
            if session.ip != ip {
                info!(
                    "has_joined: ip mismatch: username={}, expected={}, got={}",
                    params.username, session.ip, ip
                );
                Err(YggdrasilError::http(204))?;
            }
        }

        let profile = state
            .da
            .query_profile(&session.profile_id)
            .await
            .map_err(|_| YggdrasilError::http(204))?;
        let mut db = state.da.db().clone();

        if profile.name == params.username {
            info!("has_joined: success: username={}", params.username);
            Ok((
                StatusCode::OK,
                Json(
                    ExchangeableGameProfile::new(
                        &mut db,
                        state.assets,
                        &profile,
                        true,
                        Some(&state.cfg.yggdrasil.private_key),
                    )
                    .await,
                ),
            ))
        } else {
            info!("has_joined: profile name mismatch");
            Err(YggdrasilError::http(204))
        }
    } else {
        Err(YggdrasilError::http(204))
    }
}

// GET /sessionserver/session/minecraft/profile/{uuid}?unsigned={unsigned}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileParams {
    unsigned: Option<bool>,
}

#[derive(Serialize)]
pub struct ResponseProfile(Option<ExchangeableGameProfile>);

pub async fn profile(
    State(state): State<AppState>,
    Path(uuid): Path<UnhyphenatedUuid>,
    Query(params): Query<ProfileParams>,
) -> ResponseProfile {
    let uuid: Uuid = uuid.clone().into();
    info!(
        "profile: uuid={}, unsigned={:?}",
        trunc_uuid(&uuid),
        params.unsigned
    );

    if let Ok(profile) = state
        .da
        .query_profile(&uuid)
        .await
        .map_err(|_| YggdrasilError::http(204))
    {
        let mut db = state.da.db().clone();
        let rsa_priv_key = if params.unsigned.is_some_and(|x| !x) {
            Some(state.cfg.yggdrasil.private_key.clone())
        } else {
            None
        };
        info!("profile: success: uuid={}", trunc_uuid(&uuid));
        ResponseProfile(Some(
            ExchangeableGameProfile::new(
                &mut db,
                state.assets,
                &profile,
                true,
                rsa_priv_key.as_ref(),
            )
            .await,
        ))
    } else {
        info!("profile: no content: uuid={}", trunc_uuid(&uuid));
        ResponseProfile(None)
    }
}

impl IntoResponse for ResponseProfile {
    fn into_response(self) -> Response {
        match self.0 {
            None => StatusCode::NO_CONTENT.into_response(),
            Some(v) => (StatusCode::OK, Json(v)).into_response(),
        }
    }
}

// POST /api/profiles/minecraft

pub async fn minecraft(
    State(state): State<AppState>,
    Json(body): Json<Vec<String>>,
) -> Result<Json<Vec<ExchangeableGameProfile>>> {
    let names: Vec<&str> = body.iter().map(|s| s.as_str()).collect();
    info!("minecraft: names={:?}", names);

    if body.len() > QUERY_PROFILE_LIMIT {
        warn!(
            "minecraft: exceeded query limit: {} > {}",
            body.len(),
            QUERY_PROFILE_LIMIT
        );
        return Err(YggdrasilError::ForbiddenOperation);
    }

    let mut out = Vec::new();

    for name in &body {
        let profiles = state
            .da
            .query_profile_by_name(name)
            .await
            .map_err(|e| YggdrasilError::Other(e.to_string()))?;

        for profile in &profiles {
            let mut db = state.da.db().clone();
            let converted =
                ExchangeableGameProfile::new(&mut db, state.assets.clone(), profile, false, None)
                    .await;

            out.push(converted);
        }
    }

    Ok(out.into())
}

// PUT /api/user/profile/{uuid}/{textureType}

#[inline]
fn bearer_token(header_map: &HeaderMap) -> &str {
    header_map
        .get("Authorization")
        .and_then(|t| Some(t.to_str().unwrap_or("")))
        .unwrap_or("")
        .trim()
        .strip_prefix("Bearer")
        .unwrap_or("")
        .trim()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LowercaseTexture {
    Skin,
    Cape,
}
pub async fn put_texture(
    State(state): State<AppState>,
    header_map: HeaderMap,
    Path((uuid, texture_type)): Path<(UnhyphenatedUuid, LowercaseTexture)>,
    mut multipart: Multipart,
) -> Result<axum::response::Response> {
    use image::codecs::png::PngDecoder;
    use image::error::UnsupportedErrorKind;
    use image::{ExtendedColorType, ImageDecoder, ImageEncoder, ImageFormat, Limits};
    use std::io::Cursor;

    let uuid: Uuid = uuid.into();
    info!(
        "put_texture: uuid={}, texture_type={:?}",
        trunc_uuid(&uuid),
        texture_type
    );
    debug!(
        "put_texture: parsed uuid={}, texture_type={:?}",
        uuid, texture_type
    );

    let bearer_str = bearer_token(&header_map);
    debug!(
        "put_texture: bearer_token raw={:?}",
        bearer_str.chars().take(16).collect::<String>()
    );
    let access_token = bearer_str.parse().map_err(|_| {
        warn!("put_texture: unauthorized: uuid={}", trunc_uuid(&uuid));
        YggdrasilError::http(401)
    })?;
    debug!("put_texture: access_token={}", trunc_uuid(&access_token));

    let profile = state
        .da
        .query_profile(&uuid.into())
        .await
        .map_err(|_| YggdrasilError::ForbiddenOperation)?;
    debug!(
        "put_texture: profile queried: profile.id={}",
        trunc_uuid(&profile.id)
    );

    state
        .da
        .match_profile(&access_token, &profile.id)
        .await
        .map_err(|_| YggdrasilError::ForbiddenOperation)?;
    debug!("put_texture: profile matched successfully");

    let mut skin_model: Option<String> = None;
    let mut png_file: Option<Bytes> = None;
    debug!("put_texture: entering multipart parse loop");

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| YggdrasilError::http(400))?
    {
        let name = field.name();
        if name.is_none() {
            continue;
        }
        let name = field.name().unwrap();
        match name {
            "model" => {
                let text = field.text().await.map_err(|_| YggdrasilError::http(400))?;
                if !text.is_empty() {
                    skin_model = Some(text);
                }
                debug!("put_texture: multipart field 'model' = {:?}", skin_model);
            }
            "file" => {
                png_file = Some(field.bytes().await.map_err(|_| YggdrasilError::http(400))?);
                if let Some(ref data) = png_file {
                    debug!(
                        "put_texture: multipart field 'file' size={} bytes",
                        data.len()
                    );
                    if data.len() > MAX_TEXTURE_UPLOAD_SIZE {
                        debug!(
                            "put_texture: file too large: {} > {}",
                            data.len(),
                            MAX_TEXTURE_UPLOAD_SIZE
                        );
                        return Ok((
                            StatusCode::PAYLOAD_TOO_LARGE,
                            format!(
                                "File size exceeds the maximum of {} bytes.",
                                MAX_TEXTURE_UPLOAD_SIZE
                            ),
                        )
                            .into_response());
                    }
                }
            }
            _ => {
                debug!("put_texture: multipart unknown field name={:?}", name);
                continue;
            }
        }
    }

    if png_file.is_none() {
        debug!("put_texture: no 'file' field in multipart, returning 400");
        return Err(YggdrasilError::http(400));
    }
    let png_file = png_file.unwrap();
    debug!(
        "put_texture: multipart parsed, png_file size={}",
        png_file.len()
    );

    let mut limits = Limits::default();
    limits.max_alloc = Some(16 * 1024 * 1024);
    debug!(
        "put_texture: decoding PNG with max_alloc={}",
        limits.max_alloc.unwrap_or(0)
    );
    let png_decoder = match PngDecoder::with_limits(Cursor::new(png_file.as_ref()), limits) {
        Ok(d) => d,
        Err(image::ImageError::Unsupported(e))
            if matches!(
                e.kind(),
                UnsupportedErrorKind::Color(image::ExtendedColorType::Unknown(_))
            ) =>
        {
            debug!("put_texture: unsupported color encoding error={:?}", e);
            return Ok((
                StatusCode::BAD_REQUEST,
                "Provided PNG file has an unsupported color encoding.",
            )
                .into_response());
        }
        Err(e) => {
            debug!("put_texture: PNG decode error={:?}", e);
            return Ok((
                StatusCode::BAD_REQUEST,
                format!("Error decoding PNG data: {}.", e),
            )
                .into_response());
        }
    };

    let (origin_width, origin_height) = png_decoder.dimensions();
    let is_cape = matches!(&texture_type, LowercaseTexture::Cape);
    debug!(
        "put_texture: origin dimensions={}x{}, is_cape={}",
        origin_width, origin_height, is_cape
    );

    // max file size is 512*512
    if origin_width > 64 * 8 || origin_height > 64 * 8 {
        debug!(
            "put_texture: dimensions exceed 512x512: {}x{}",
            origin_width, origin_height
        );
        return Ok((
            StatusCode::PAYLOAD_TOO_LARGE,
            "Picture size exceed the max of 512x512 pixels.",
        )
            .into_response());
    }

    let is_standard = origin_width % 64 == 0 && origin_height % 32 == 0;
    let is_cape_unstandard = is_cape && origin_width % 22 == 0 && origin_height % 17 == 0;
    debug!(
        "put_texture: size check: is_standard={}, is_cape_unstandard={}",
        is_standard, is_cape_unstandard
    );
    if origin_width == 0 || origin_height == 0 || !(is_standard || is_cape_unstandard) {
        debug!(
            "put_texture: invalid dimensions: {}x{}",
            origin_width, origin_height
        );
        return Ok((StatusCode::BAD_REQUEST, "Picture does not match size requirements. The size is only allowed to be multiples of 64x32 (for skins and capes) or 22x17 (for capes only).").into_response());
    }

    let (width, height) = if is_cape_unstandard {
        debug!(
            "put_texture: upscaling unstandard cape from {}x{} to {}x{}",
            origin_width,
            origin_height,
            64 * (origin_width / 22),
            32 * (origin_height / 17)
        );
        (64 * (origin_width / 22), 32 * (origin_height / 17))
    } else {
        (origin_width, origin_height)
    };
    debug!("put_texture: target dimensions={}x{}", width, height);

    let source_rgba = match image::load_from_memory_with_format(png_file.as_ref(), ImageFormat::Png)
    {
        Ok(image) => image.to_rgba8(),
        Err(e) => {
            debug!("put_texture: load_from_memory error={:?}", e);
            return Ok((
                StatusCode::BAD_REQUEST,
                format!("Error decoding PNG data: {}.", e),
            )
                .into_response());
        }
    };
    debug!(
        "put_texture: loaded RGBA image: {}x{}",
        source_rgba.width(),
        source_rgba.height()
    );

    let mut washed_rgba = vec![0_u8; width as usize * height as usize * 4];
    let source_raw = source_rgba.as_raw();
    debug!(
        "put_texture: washed_rgba allocated: {} bytes ({}x{}x4)",
        washed_rgba.len(),
        width,
        height
    );
    // Copy the image data; for unstandard capes, put the original image on the upper-left corner of new image and fill the remaining pixels with transparent
    let mut y = 0;
    loop {
        if y >= origin_height {
            break;
        }
        let mut x = 0;
        loop {
            if x >= origin_width {
                break;
            }
            let src_offset = ((y * origin_width + x) * 4) as usize;
            let dst_offset = ((y * width + x) * 4) as usize;
            washed_rgba[dst_offset..dst_offset + 4]
                .copy_from_slice(&source_raw[src_offset..src_offset + 4]);
            x += 1;
        }
        y += 1;
    }
    debug!(
        "put_texture: pixel copy done: {} rows copied",
        origin_height
    );

    let parsed_skin_model = match (&texture_type, skin_model.as_deref()) {
        (LowercaseTexture::Skin, None | Some("default") | Some("classic") | Some("")) => {
            SkinModel::Default
        }
        (LowercaseTexture::Skin, Some("slim")) => SkinModel::Slim,
        (LowercaseTexture::Skin, Some(_)) => {
            debug!("put_texture: invalid skin model value={:?}", skin_model);
            return Ok((
                StatusCode::BAD_REQUEST,
                "Invalid skin model. Allowed values are: default, classic, slim.",
            )
                .into_response());
        }
        (LowercaseTexture::Cape, _) => SkinModel::Default,
    };
    debug!("put_texture: parsed_skin_model={:?}", parsed_skin_model);

    let mut washed_png = vec![];
    image::codecs::png::PngEncoder::new(&mut washed_png)
        .write_image(&washed_rgba, width, height, ExtendedColorType::Rgba8)
        .map_err(|e| YggdrasilError::Other(format!("Error encoding PNG data: {}.", e)))?;
    debug!(
        "put_texture: PNG re-encoded: output size={} bytes",
        washed_png.len()
    );

    let new_file = state
        .assets
        .create_file(Cursor::new(washed_png))
        .await
        .map_err(|e| YggdrasilError::Other(e.to_string()))?;
    debug!(
        "put_texture: asset file created: file.id={}",
        trunc_uuid(&new_file.id)
    );

    let mut db = state.da.db().clone();
    let existing_textures = profile.textures().exec(&mut db).await?;
    debug!(
        "put_texture: existing_textures={:?}",
        existing_textures.as_ref().map(|_| "Some")
    );

    let mut old_file: Option<Uuid> = None;
    if let Some(mut textures) = existing_textures {
        match texture_type {
            LowercaseTexture::Skin => {
                debug!(
                    "put_texture: updating existing textures: old.skin_file={:?}",
                    textures.skin_file.map(|u| trunc_uuid(&u))
                );
                old_file = textures.skin_file;
                textures
                    .update()
                    .skin_model(parsed_skin_model)
                    .skin_file(Some(new_file.id))
                    .exec(&mut db)
                    .await?;
                debug!(
                    "put_texture: textures skin updated to new_file.id={}",
                    trunc_uuid(&new_file.id)
                );
            }
            LowercaseTexture::Cape => {
                debug!(
                    "put_texture: updating existing textures: old.cape_file={:?}",
                    textures.cape_file.map(|u| trunc_uuid(&u))
                );
                old_file = textures.cape_file;
                textures
                    .update()
                    .cape_file(Some(new_file.id))
                    .exec(&mut db)
                    .await?;
                debug!(
                    "put_texture: textures cape updated to new_file.id={}",
                    trunc_uuid(&new_file.id)
                );
            }
        }
    } else {
        debug!("put_texture: no existing textures, creating new ProfileTextures");
        match texture_type {
            LowercaseTexture::Skin => {
                ProfileTextures::create()
                    .profile_id(profile.id)
                    .skin_model(parsed_skin_model)
                    .skin_file(Some(new_file.id))
                    .cape_file(None)
                    .exec(&mut db)
                    .await?;
                debug!("put_texture: new ProfileTextures created for skin");
            }
            LowercaseTexture::Cape => {
                ProfileTextures::create()
                    .profile_id(profile.id)
                    .skin_model(SkinModel::Default)
                    .skin_file(None)
                    .cape_file(Some(new_file.id))
                    .exec(&mut db)
                    .await?;
                debug!("put_texture: new ProfileTextures created for cape");
            }
        }
    }

    if let Some(old_file) = old_file {
        debug!(
            "put_texture: deleting old file: old_file.id={}",
            trunc_uuid(&old_file)
        );
        let _ = state.assets.delete_file(old_file).await;
        debug!("put_texture: old file deletion completed");
    }

    info!("put_texture: success: uuid={}", trunc_uuid(&uuid));
    Ok(StatusCode::NO_CONTENT.into_response())
}

// DELETE /api/user/profile/{uuid}/{textureType}

pub async fn delete_texture(
    State(state): State<AppState>,
    header_map: HeaderMap,
    Path((uuid, texture_type)): Path<(UnhyphenatedUuid, LowercaseTexture)>,
) -> Result<StatusCode> {
    let uuid: Uuid = uuid.into();
    info!(
        "delete_texture: uuid={}, texture_type={:?}",
        trunc_uuid(&uuid),
        texture_type
    );

    let access_token = bearer_token(&header_map)
        .parse()
        .map_err(|_| YggdrasilError::http(401))?;

    let profile = state
        .da
        .query_profile(&uuid.into())
        .await
        .map_err(|_| YggdrasilError::ForbiddenOperation)?;

    state
        .da
        .match_profile(&access_token, &profile.id)
        .await
        .map_err(|_| YggdrasilError::ForbiddenOperation)?;

    let mut db = state.da.db().clone();
    let existing_textures = profile.textures().exec(&mut db).await.unwrap_or(None);

    if let Some(mut textures) = existing_textures {
        let old_file = match texture_type {
            LowercaseTexture::Skin => {
                let file = textures.skin_file.take();
                textures.update().skin_file(None).exec(&mut db).await?;
                file
            }
            LowercaseTexture::Cape => {
                let file = textures.cape_file.take();
                textures.update().cape_file(None).exec(&mut db).await?;
                file
            }
        };
        if let Some(file_id) = old_file {
            let _ = state.assets.delete_file(file_id).await;
        }
    }

    info!("delete_texture: success: uuid={}", trunc_uuid(&uuid));
    Ok(StatusCode::NO_CONTENT)
}

// GET /

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseMeta {
    meta: MetaInfo,
    skin_domains: Vec<String>,
    signature_publickey: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MetaInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    server_name: Option<String>,
    implementation_name: &'static str,
    implementation_version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    links: Option<LinksInfo>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LinksInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    homepage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    register: Option<String>,
}

pub async fn meta(State(state): State<AppState>) -> Result<(StatusCode, Json<ResponseMeta>)> {
    info!("meta");

    let links = LinksInfo {
        homepage: state.cfg.yggdrasil.homepage.clone(),
        register: state.cfg.yggdrasil.register_page.clone(),
    };
    let links = if let None = links.homepage
        && let None = links.homepage
    {
        None
    } else {
        Some(links)
    };

    Ok((
        StatusCode::OK,
        Json(ResponseMeta {
            meta: MetaInfo {
                server_name: state
                    .cfg
                    .yggdrasil
                    .server_name
                    .clone()
                    .or(Some("Aphanite".into())),
                implementation_name: "Aphanite",
                implementation_version: env!("CARGO_PKG_VERSION"),
                links,
            },
            skin_domains: state.assets.whitelisted_domains(),
            signature_publickey: state.rsa_pubkey.to_public_key_pem(LineEnding::LF)?,
        }),
    ))
}
