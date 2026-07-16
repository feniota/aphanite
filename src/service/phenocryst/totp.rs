use crate::AppState;
use crate::service::api::authenticate;
use crate::service::{ApiResult as Result, Error};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use totp_rs::{Algorithm, Rfc6238, Secret, TOTP};
use uuid::Uuid;

// 创建的 TOTP 会话有效期（不是OTP Token有效期）
const TOTP_SESSION_TTL: Duration = Duration::from_mins(10);

// POST /user/me/credentials/totp
#[derive(Serialize)]
pub struct ResponseTotp {
    secret: String,
    otpauth_url: String,
}

async fn create_totp(State(state): State<AppState>, headers: HeaderMap) -> Result<ResponseTotp> {
    let mut current_user = authenticate(&state, &headers).await?;
    let mut db = state.da.db().clone();
    let new_secret = Secret::generate_secret();
    current_user
        .update()
        .totp_secret(new_secret.to_encoded().to_string())
        .exec(&mut db)
        .await?;

    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        new_secret.to_bytes().unwrap(),
        Some("Aphanite".to_string()),
        current_user.email,
    )?;

    Ok(ResponseTotp {
        secret: new_secret.to_encoded().to_string(),
        otpauth_url: totp.get_url(),
    }
    .into())
}

// DELETE /user/me/credentials/totp

async fn delete_totp(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> std::result::Result<StatusCode, Error> {
    let mut current_user = authenticate(&state, &headers).await?;
    let mut db = state.da.db().clone();
    current_user
        .update()
        .totp_secret(None)
        .exec(&mut db)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

// POST /verification
#[derive(Clone)]
pub struct OtpSession {
    method: VerificationMethod,
    user_email: String,
    secret: String,
    pub expired_at: Instant,
}

#[derive(Deserialize)]
struct RequestVerification {
    method: VerificationMethod,
    email: String,
}
#[derive(Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMethod {
    Totp,
}

#[derive(Serialize)]
struct ResponseVerification {
    id: String,
}

async fn create_verification(
    State(state): State<AppState>,
    Json(body): Json<RequestVerification>,
) -> Result<ResponseVerification> {
    match body.method {
        VerificationMethod::Totp => {
            let secret = match state.da.query_totp(&body.email).await {
                None => {
                    return Err(Error::new(
                        StatusCode::BAD_REQUEST,
                        "No TOTP secret available.",
                    ));
                }
                Some(v) => v,
            };
            let id = state
                .kv
                .insert_otp_session(OtpSession {
                    method: VerificationMethod::Totp,
                    user_email: body.email,
                    secret,
                    expired_at: Instant::now() + TOTP_SESSION_TTL,
                })
                .await;
            Ok(ResponseVerification { id: id.to_string() }.into())
        }
    }
}

// POST /verification/{id}
#[derive(Deserialize)]
struct CompleteVerification {
    code: String,
}
#[derive(Serialize)]
struct SignVerification {
    otp_token: Uuid,
}

async fn complete_verification(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<CompleteVerification>,
) -> Result<SignVerification> {
    let session = match state.kv.query_otp_session(&id).await {
        None => {
            return Err(Error::new(
                StatusCode::NOT_FOUND,
                "Failed to find an OTP session with this ID.",
            ));
        }
        Some(v) => v,
    };
    match session.method {
        VerificationMethod::Totp => {
            let mut rfc = Rfc6238::with_defaults(
                Secret::Encoded(session.secret)
                    .to_bytes()
                    .expect("Failed to parse in-database Base32 TOTP secret"),
            )
            .expect("The Secret does not comply with the RFC6238 standard.");
            rfc.issuer("Aphanite".into());
            rfc.account_name(session.user_email.clone());
            let totp = TOTP::from_rfc6238(rfc).unwrap();
            if totp
                .check_current(&body.code)
                .expect("TOTP verification failed")
            {
                Ok(SignVerification {
                    otp_token: state.kv.sign_otp_token(session.user_email).await,
                }
                .into())
            } else {
                tracing::debug!(
                    "TOTP Failed!!, right PIN: {}",
                    totp.generate_current().unwrap()
                );
                Err(Error::new(
                    StatusCode::UNAUTHORIZED,
                    "TOTP verification code error",
                ))
            }
        }
    }
}

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{delete, post};
    axum::Router::new()
        .route("/users/me/credentials/totp", post(create_totp))
        .route("/users/me/credentials/totp", delete(delete_totp))
        .route("/verification", post(create_verification))
        .route("/verification/{id}", post(complete_verification))
}
