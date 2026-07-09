use crate::service::{ApiResult as Result, Error};
use crate::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use totp_rs::{Rfc6238, TOTP};
use uuid::Uuid;

// 创建的 TOTP 会话有效期（不是OTP Token有效期）
const TOTP_SESSION_TTL: Duration = Duration::from_secs(10);

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
                        StatusCode::NOT_FOUND,
                        "No TOTP secret available.",
                    ));
                }
                Some(v) => v,
            };
            let id = state.kv.insert_otp_session(OtpSession {
                method: VerificationMethod::Totp,
                user_email: body.email,
                secret,
                expired_at: Instant::now() + TOTP_SESSION_TTL,
            });
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
    let session = match state.kv.query_otp_session(&id) {
        None => {
            return Err(Error::new(
                StatusCode::NOT_FOUND,
                "No TOTP session available.",
            ));
        }
        Some(v) => v,
    };
    match session.method {
        VerificationMethod::Totp => {
            let rfc = Rfc6238::with_defaults(session.secret.into_bytes())
                .expect("The Secret does not comply with the RFC6238 standard.");
            let totp = TOTP::from_rfc6238(rfc).unwrap();
            if totp
                .check_current(&body.code)
                .expect("TOTP verification failed")
            {
                Ok(SignVerification {
                    otp_token: state.kv.sign_otp_token(session.user_email),
                }
                .into())
            } else {
                Err(Error::new(
                    StatusCode::UNAUTHORIZED,
                    "TOTP verification code error",
                ))
            }
        }
    }
}
