//! Specific API endpoints implementation

use super::types::{ExchangeableGameProfile, UnhyphenatedUuid};
use crate::AppState;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::error::Error;
use uuid::Uuid;

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

    /// Attempt to assign profile to a profile-assigned token
    IllegalArgument,

    /// Attempt to assign unrelated profile or other general forbidden operations
    ForbiddenOperation,

    /// General error that is not covered by the doc
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
            YggdrasilError::IllegalArgument => {
                write!(f, "Access token already has a profile assigned.")
            }
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
        Self::Other(error.to_string())
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
    fn into_response(self) -> axum::response::Response {
        let (status, body) = match self {
            YggdrasilError::HttpError(status) => (
                status,
                ErrorResponse::new(&self, status.canonical_reason().unwrap_or("Error")),
            ),

            YggdrasilError::InvalidToken => (
                StatusCode::FORBIDDEN,
                ErrorResponse::new(&self, "ForbiddenOperationException"),
            ),

            YggdrasilError::InvalidCredentials => (
                StatusCode::FORBIDDEN,
                ErrorResponse::new(&self, "ForbiddenOperationException"),
            ),

            YggdrasilError::IllegalArgument => (
                StatusCode::BAD_REQUEST,
                ErrorResponse::new(&self, "IllegalArgumentException"),
            ),

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
    #[serde(skip_serializing_if = "Option::is_none")]
    client_token: Option<String>,
    request_user: bool,
    agent: AuthenticateAgent,
}

#[derive(Deserialize)]
struct AuthenticateAgent {
    name: String,
    version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseAuthenticate {
    access_token: UnhyphenatedUuid,
    client_token: String,
    available_profiles: Vec<ExchangeableGameProfile>,
    selected_profile: Vec<ExchangeableGameProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<ExchangeableGameProfile>,
}

pub async fn authenticate(
    State(state): State<AppState>,
    Json(body): Json<RequestAuthenticate>,
) -> Result<(StatusCode, Json<ResponseAuthenticate>)> {
    let user = state
        .da
        .verify_user(&body.username, &body.password)
        .await
        .map_err(|_| YggdrasilError::InvalidCredentials)?;
    let client_token = body
        .client_token
        .unwrap_or_else(|| Uuid::now_v7().simple().to_string());
    let access_token = state
        .da
        .create_token(&user.id, &client_token, None)
        .await
        .map_err(|e| YggdrasilError::Other(e.to_string()))?
        .into();
    let mut available_profiles = Vec::new();
    for i in state
        .da
        .query_profile_by_user(&user.id)
        .await
        .map_err(|e| YggdrasilError::Other(e.to_string()))?
    {
        available_profiles
            .push(ExchangeableGameProfile::new(state.assets.clone(), &i, true, true).await)
    }

    Ok((
        StatusCode::OK,
        ResponseAuthenticate {
            access_token,
            client_token,
            available_profiles,
            selected_profile: vec![],
            user: None,
        }
        .into(),
    ))
}

// POST /authserver/refresh

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestRefresh {
    access_token: UnhyphenatedUuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_token: Option<String>,
    request_user: bool,
    selected_profile: Vec<ExchangeableGameProfile>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseRefresh {
    access_token: UnhyphenatedUuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_token: Option<String>,
    selected_profile: Vec<ExchangeableGameProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<ExchangeableGameProfile>,
}

pub async fn refresh(
    State(state): State<AppState>,
    body: Json<RequestRefresh>,
) -> Result<(StatusCode, Json<ResponseRefresh>)> {
    todo!()
}

// POST /authserver/validate

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestValidate {
    access_token: UnhyphenatedUuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_token: Option<String>,
}

pub async fn validate(
    State(state): State<AppState>,
    body: Json<RequestValidate>,
) -> Result<StatusCode> {
    todo!()
}

// POST /authserver/invalidate

pub async fn invalidate(
    State(state): State<AppState>,
    body: Json<RequestValidate>,
) -> Result<StatusCode> {
    todo!()
}

// POST /authserver/signout

#[derive(Deserialize)]
struct RequestSignout {
    username: String,
    password: String,
}

pub async fn signout(
    State(state): State<AppState>,
    body: Json<RequestValidate>,
) -> Result<StatusCode> {
    todo!()
}

// POST /sessionserver/session/minecraft/join

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestJoin {
    #[serde(rename = "accessToken")]
    pub access_token: UnhyphenatedUuid,
    #[serde(rename = "selectedProfile")]
    pub selected_profile: String,
    #[serde(rename = "serverId")]
    pub server_id: String,
}

pub async fn join(
    State(state): State<AppState>,
    body: Json<RequestValidate>,
) -> Result<StatusCode> {
    todo!()
}

// GET /sessionserver/session/minecraft/hasJoined?username={username}&serverId={serverId}&ip={ip}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HasJoinedParams {
    username: String,
    server_id: String,
    ip: Option<String>,
}

pub async fn has_joined(
    State(state): State<AppState>,
    body: Json<RequestValidate>,
) -> Result<StatusCode> {
    todo!()
}

// GET /sessionserver/session/minecraft/profile/{uuid}?unsigned={unsigned}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileParams {
    uuid: UnhyphenatedUuid,
    unsigned: Option<bool>,
}

pub struct ResponseProfile(Option<ExchangeableGameProfile>);

#[axum::debug_handler]
pub async fn profile(
    State(state): State<AppState>,
    body: Query<ProfileParams>,
) -> Result<ResponseProfile> {
    todo!()
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
    body: Json<Vec<String>>,
) -> Result<Json<Vec<ExchangeableGameProfile>>> {
    todo!()
}

// PUT /api/user/profile/{uuid}/{textureType}

#[inline]
fn bearer_token(header_map: &HeaderMap, token: &str) -> bool {
    token
        == header_map
            .get("Authorization")
            .and_then(|t| Some(t.to_str().unwrap_or("")))
            .unwrap_or("")
            .trim()
            .strip_prefix("Bearer")
            .unwrap_or("")
            .trim()
}

pub async fn put_texture(
    header_map: HeaderMap,
    Path(uuid): Path<UnhyphenatedUuid>,
    Path(textureType): Path<String>,
    multipart: Multipart,
) -> Result<StatusCode> {
    todo!()
}

// DELETE /api/user/profile/{uuid}/{textureType}

pub async fn delete_texture(
    header_map: HeaderMap,
    Path(uuid): Path<UnhyphenatedUuid>,
    Path(textureType): Path<String>,
) -> Result<StatusCode> {
    todo!()
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
    let links = LinksInfo {
        homepage: state.cfg.yggdrasil.homepage.clone(),
        register: state.cfg.yggdrasil.register.clone(),
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
                server_name: state.cfg.yggdrasil.server_name.clone(),
                implementation_name: "Aphanite",
                implementation_version: env!("CARGO_PKG_VERSION"),
                links,
            },
            skin_domains: match state.assets.whitelist_domain() {
                None => vec![state.cfg.api.domain.to_string()],
                Some(v) => vec![state.cfg.api.domain.to_string(), v],
            },
            signature_publickey: state.cfg.yggdrasil.public_key.to_string(),
        }),
    ))
}
