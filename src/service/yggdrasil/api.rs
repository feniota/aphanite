//! Specific API endpoints implementation

use super::types::{ExchangeableGameProfile, UnhyphenatedUuid};
use crate::types::User;
use crate::AppState;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::net::IpAddr;
use tokio_stream::StreamExt;
use tracing::{debug, error};
use uuid::Uuid;

const QUERY_PROFILE_LIMIT: usize = 50;

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
    fn into_response(self) -> Response {
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
    state: AppState,
    request_user: bool,
    selected_profile: Option<ExchangeableGameProfile>,
) -> Result<ResponseAuthenticate> {
    let client_token = client_token.unwrap_or_else(|| Uuid::now_v7().simple().to_string());

    let available_profiles = tokio_stream::iter(
        state
            .da
            .query_profile_by_user(&user.id)
            .await
            .map_err(|e| YggdrasilError::Other(e.to_string()))?,
    )
    .then(|x| {
        let assets = state.assets.clone();
        async move { ExchangeableGameProfile::new(assets, &x, false, false).await }
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
                value: user.prefer_language,
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
    if !state.kv.try_consume(body.username.clone()) {
        debug!(
            "User {} has an excessively high login frequency.",
            body.username
        );
        return Err(YggdrasilError::InvalidCredentials);
    }

    let user = state
        .da
        .verify_user(&body.username, &body.password)
        .await
        .map_err(|_| YggdrasilError::InvalidCredentials)?;

    Ok((
        StatusCode::OK,
        create_authenticate(user, body.client_token, state, body.request_user, None)
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
    let user = state
        .da
        .verify_token(&body.access_token.into(), &body.client_token)
        .await
        .map_err(|_| YggdrasilError::InvalidToken)?;

    let new_authenticate = create_authenticate(
        user,
        body.client_token,
        state,
        body.request_user,
        body.selected_profile,
    )
    .await?;

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
    state
        .da
        .verify_token(&body.access_token.into(), &body.client_token)
        .await
        .map_err(|_| YggdrasilError::InvalidToken)?;
    Ok(StatusCode::NO_CONTENT)
}

// POST /authserver/invalidate

pub async fn invalidate(
    State(state): State<AppState>,
    Json(body): Json<RequestValidate>,
) -> Result<StatusCode> {
    if let Err(e) = state.da.delete_token(&body.access_token.into()).await {
        error!("{e}")
    };
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
    if !state.kv.try_consume(body.username.clone()) {
        debug!(
            "User {} has an excessively high login frequency.",
            body.username
        );
        return Err(YggdrasilError::InvalidCredentials);
    }

    let user = state
        .da
        .verify_user(&body.username, &body.password)
        .await
        .map_err(|_| YggdrasilError::InvalidCredentials)?;

    state
        .da
        .clear_token(&user.id)
        .await
        .map_err(|e| YggdrasilError::Other(e.to_string()))?;

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
    Json(body): Json<RequestJoin>,
) -> Result<StatusCode> {
    let access_token = body.access_token.into();
    let profile_id = body.selected_profile.into();
    let user = state
        .da
        .match_profile(&access_token, &profile_id)
        .await
        .map_err(|_| YggdrasilError::ForbiddenOperation)?;

    state.kv.record_session(Session {
        profile_id,
        server_id: body.server_id,
        access_token,
        ip: todo!("IP"),
    });

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
    if let Some(session) = state.kv.query_session(&params.server_id) {
        let user = state
            .da
            .verify_token(&session.access_token, &None)
            .await
            .map_err(|_| YggdrasilError::HttpError(StatusCode::NO_CONTENT))?;
        // TODO: 验证 IP

        let profile = state
            .da
            .query_profile(&session.profile_id)
            .await
            .map_err(|_| YggdrasilError::HttpError(StatusCode::NO_CONTENT))?;

        if user.email == params.username {
            Ok((
                StatusCode::OK,
                ExchangeableGameProfile {
                    id: profile.id.into(),
                    name: "".to_string(),
                    properties: None,
                }
                .into(),
            ))
        } else {
            Err(YggdrasilError::HttpError(StatusCode::NO_CONTENT))
        }
    } else {
        Err(YggdrasilError::HttpError(StatusCode::NO_CONTENT))
    }
}

// GET /sessionserver/session/minecraft/profile/{uuid}?unsigned={unsigned}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileParams {
    uuid: UnhyphenatedUuid,
    unsigned: Option<bool>,
}

#[derive(Serialize)]
pub struct ResponseProfile(Option<ExchangeableGameProfile>);

#[axum::debug_handler]
pub async fn profile(
    State(state): State<AppState>,
    Query(params): Query<ProfileParams>,
) -> ResponseProfile {
    if let Ok(profile) = state
        .da
        .query_profile(&params.uuid.into())
        .await
        .map_err(|_| YggdrasilError::HttpError(StatusCode::NO_CONTENT))
    {
        ResponseProfile(Some(
            ExchangeableGameProfile::new(
                state.assets,
                &profile,
                true,
                params.unsigned.unwrap_or(true),
            )
            .await,
        ))
    } else {
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
    if body.len() > QUERY_PROFILE_LIMIT {
        return Err(YggdrasilError::ForbiddenOperation);
    }

    let mut out = Vec::new();

    for name in body {
        let profiles = state
            .da
            .query_profile_by_name(&name)
            .await
            .map_err(|e| YggdrasilError::Other(e.to_string()))?;

        for profile in profiles {
            let converted =
                ExchangeableGameProfile::new(state.assets.clone(), &profile, false, false).await;

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

pub async fn put_texture(
    State(state): State<AppState>,
    header_map: HeaderMap,
    Path(uuid): Path<UnhyphenatedUuid>,
    Path(textureType): Path<String>,
    mut multipart: Multipart,
) -> Result<StatusCode> {
    let access_token = bearer_token(&header_map)
        .parse()
        .map_err(|_| YggdrasilError::HttpError(StatusCode::UNAUTHORIZED))?;

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

    // 请开始吧（（（

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
