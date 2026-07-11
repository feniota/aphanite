//! The core Web service

use crate::AppState;
use crate::service::frontend::make_frontend_router;
use axum::http::StatusCode;
use axum::{Extension, Router};

pub mod api;
pub mod frontend;
pub mod phenocryst;
pub mod types;
pub mod yggdrasil;

pub fn router(state: AppState) -> Router {
    use axum::routing::get;
    make_frontend_router()
        .route("/api/yggdrasil/", get(yggdrasil::api::meta))
        .nest("/api/yggdrasil", yggdrasil::router())
        .nest("/api", api::router())
        .nest("/api", phenocryst::totp::router())
        .layer(Extension(state.cfg.service.client_ip.clone()))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            api_location_indication,
        ))
        .with_state(state)
}

// https://yushijinhun.github.io/authlib-injector/zh/Yggdrasil-%E6%9C%8D%E5%8A%A1%E7%AB%AF%E6%8A%80%E6%9C%AF%E8%A7%84%E8%8C%83.html#api-%E5%9C%B0%E5%9D%80%E6%8C%87%E7%A4%BAali
async fn api_location_indication(
    axum::extract::State(state): axum::extract::State<AppState>,
    request: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut response = next.run(request).await;

    let location = format!(
        "{}{}/api/yggdrasil/",
        if state.cfg.service.tls {
            "https://"
        } else {
            "http://"
        },
        state.cfg.service.domain,
    );

    response.headers_mut().insert(
        "X-Authlib-Injector-API-Location",
        axum::http::HeaderValue::from_str(&location)
            .expect("Illegal characters were found in X-Authlib-Injector-API-Location"),
    );

    response
}

/// The generic Error type used across all the *Web functions* in Aphanite
///
/// This implements `From<impl Error>` and [`IntoResponse`](axum::response::IntoResponse).
///
/// This is intended to be used in axum routes to simplify error handling.
///
/// - For general error handling (outside axum) use [`anyhow::Error`] instead.
/// - For Yggdrasil APIs use [`YggdrasilError`](crate::service::yggdrasil::types::YggdrasilError) instead.
#[derive(Clone)]
pub struct Error {
    status: StatusCode,
    reason: String,
}

impl Error {
    /// Construct a new Error
    pub fn new<S>(status: StatusCode, reason: S) -> Self
    where
        S: AsRef<str>,
    {
        Self {
            status,
            reason: reason.as_ref().to_string(),
        }
    }

    /// Construct a new Error with the status code being a number literal
    ///
    /// This function performs no checks on the `u16 -> StatusCode` conversion; The caller MUST guarantee that the status code is valid.(>=100 && <=999)
    #[allow(clippy::self_named_constructors)]
    pub fn error<S>(status: u16, reason: S) -> Self
    where
        S: AsRef<str>,
    {
        Self {
            status: StatusCode::from_u16(status).unwrap(),
            reason: reason.as_ref().to_string(),
        }
    }
}

impl<E> From<E> for Error
where
    E: std::error::Error,
{
    fn from(e: E) -> Self {
        tracing::error!("Unexpected error occurred: {}", e);
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            reason: e.to_string(),
        }
    }
}

impl axum::response::IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        use axum::Json;
        use serde::Serialize;
        #[derive(Serialize)]
        struct R {
            success: bool,
            reason: String,
        }
        let resp = R {
            success: false,
            reason: self.reason,
        };
        (self.status, Json(resp)).into_response()
    }
}

/// Type alias for [`Result<T,E>`](std::result::Result) where `E` is always [`Error`]
pub type Result<T> = std::result::Result<T, Error>;

/// The general success response type used across the Web functions in Aphanite.
///
/// This implements [`IntoResponse`](axum::response::IntoResponse).
pub struct ApiResponse<T> {
    payload: T,
}

impl<T> From<T> for ApiResponse<T>
where
    T: serde::Serialize,
{
    fn from(value: T) -> Self {
        Self { payload: value }
    }
}

impl<T> axum::response::IntoResponse for ApiResponse<T>
where
    T: serde::Serialize,
{
    fn into_response(self) -> axum::response::Response {
        use axum::Json;

        #[derive(serde::Serialize)]
        struct Resp<T> {
            success: bool,
            payload: T,
        }

        let resp = Resp {
            success: true,
            payload: self.payload,
        };
        Json(resp).into_response()
    }
}

pub type ApiResult<T> = std::result::Result<ApiResponse<T>, Error>;
