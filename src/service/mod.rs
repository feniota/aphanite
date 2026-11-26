//! The core Web service

use crate::AppState;
use ntex::http::StatusCode;
use ntex::web;

pub mod api;
pub mod frontend;
pub mod phenocryst;
pub mod types;
pub mod validation;
pub mod yggdrasil;

pub(crate) type HttpState = web::AppState<AppState>;

impl web::FromRequest<HttpState> for AppState {
    type Error = std::convert::Infallible;

    async fn from_request(
        state: &HttpState,
        _: &web::HttpRequest,
        _: &mut ntex::http::Payload,
    ) -> std::result::Result<Self, Self::Error> {
        Ok(state.st().clone())
    }
}

/// Register API routes on an ntex application.
pub fn configure(cfg: &mut web::ServiceConfig<web::AppState<AppState>>) {
    cfg.service(web::scope("/api").configure(|cfg| {
        api::configure(cfg);
        phenocryst::totp::configure(cfg);
        cfg.service(
            web::scope("/yggdrasil")
                .configure(yggdrasil::configure)
                .default_service(web::to(yggdrasil::fallback)),
        );
    }));
}

/// The generic Error type used across all the *Web functions* in Aphanite
///
/// This implements `From<impl Error>` and [`web::Responder`].
///
/// This is intended to be used in ntex routes to simplify error handling.
///
/// - For general error handling (outside ntex) use [`anyhow::Error`] instead.
/// - For Yggdrasil APIs use [`YggdrasilError`](crate::service::yggdrasil::types::YggdrasilError) instead.
#[derive(Debug, Clone)]
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

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
}

impl std::error::Error for Error {}

macro_rules! impl_error_from {
    ($($error:ty),+ $(,)?) => {$(
        impl From<$error> for Error {
            fn from(error: $error) -> Self {
                tracing::error!("Unexpected error occurred: {}", error);
                Self::error(500, error.to_string())
            }
        }
    )+};
}

impl_error_from!(
    anyhow::Error,
    sea_orm::DbErr,
    std::io::Error,
    totp_rs::TotpError
);

impl<St: web::State> web::WebResponseError<St, St::Error> for Error {
    fn error_response(&self, _: &St) -> web::HttpResponse {
        web::HttpResponse::builder(self.status).json(&serde_json::json!({
            "success": false,
            "reason": self.reason,
        }))
    }
}

/// Type alias for [`Result<T,E>`](std::result::Result) where `E` is always [`Error`]
pub type Result<T> = std::result::Result<T, Error>;

/// The general success response type used across the Web functions in Aphanite.
///
/// This implements [`web::Responder`].
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

impl<St: web::State, T: serde::Serialize> web::Responder<St> for ApiResponse<T> {
    async fn respond_to(self, _: &St, _: &web::HttpRequest) -> web::HttpResponse {
        #[derive(serde::Serialize)]
        struct Response<T> {
            success: bool,
            payload: T,
        }
        web::HttpResponse::Ok().json(&Response {
            success: true,
            payload: self.payload,
        })
    }
}

pub type ApiResult<T> = std::result::Result<ApiResponse<T>, Error>;

pub(crate) trait IntoResponse {
    fn into_response(self) -> web::HttpResponse;
}
impl IntoResponse for StatusCode {
    fn into_response(self) -> web::HttpResponse {
        web::HttpResponse::builder(self).build()
    }
}
impl<T: serde::Serialize> IntoResponse for (StatusCode, web::types::Json<T>) {
    fn into_response(self) -> web::HttpResponse {
        web::HttpResponse::builder(self.0).json(&self.1.0)
    }
}
impl IntoResponse for (StatusCode, String) {
    fn into_response(self) -> web::HttpResponse {
        web::HttpResponse::builder(self.0)
            .content_type("text/plain; charset=utf-8")
            .body(self.1)
    }
}
impl IntoResponse for (StatusCode, &'static str) {
    fn into_response(self) -> web::HttpResponse {
        (self.0, self.1.to_string()).into_response()
    }
}
