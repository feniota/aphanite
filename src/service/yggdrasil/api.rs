use super::types::GameProfile;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

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

impl std::error::Error for YggdrasilError {}

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
                StatusCode::FORBIDDEN,
                ErrorResponse {
                    error: "ForbiddenOperationException".into(),
                    error_message: format!("An error has occurred: {}", &msg),
                    cause: Some(msg),
                },
            ),
        };

        (status, axum::Json(body)).into_response()
    }
}

// POST /authserver/authenticate

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestAuthenticate {
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
struct ResponseAuthenticate {
    access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_token: Option<String>,
    available_profiles: Vec<GameProfile>,
    selected_profile: Vec<GameProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<GameProfile>,
}

// POST /authserver/refresh

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestRefresh {
    access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_token: Option<String>,
    request_user: bool,
    selected_profile: Vec<GameProfile>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResponseRefresh {
    access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_token: Option<String>,
    selected_profile: Vec<GameProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<GameProfile>,
}

// POST /authserver/validate

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestValidate {
    access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_token: Option<String>,
}

// POST /authserver/invalidate

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestInvalidate {
    access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_token: Option<String>,
}

// POST /authserver/signout

#[derive(Deserialize)]
struct RequestSignout {
    username: String,
    password: String,
}

#[cfg(test)]
mod test {
    use super::*;
    use axum::Router;
    use axum::routing::get;
    #[test]
    fn type_test() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let app = Router::new().route("/test", get(test_route));
            let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
            axum::serve(listener, app).await.unwrap();
        })
    }
    async fn test_route() -> Result<()> {
        Ok(())
    }
}
