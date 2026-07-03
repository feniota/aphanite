use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    HttpError(StatusCode),
    InvalidToken,
    InvalidCredentials,
    IllegalArgumentException,
    Undefined(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::HttpError(s) => write!(f, "HTTP error: {}", s),
            Error::InvalidToken => write!(f, "Invalid token"),
            Error::InvalidCredentials => write!(f, "Invalid credentials"),
            Error::IllegalArgumentException => write!(f, "Illegal argument"),
            Error::Undefined(msg) => write!(f, "Undefined error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub error: String,
    pub error_message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

impl ErrorResponse {
    fn new(error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            error_message: message.into(),
            cause: None,
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let (status, body) = match self {
            Error::HttpError(s) => (
                s,
                ErrorResponse::new(s.canonical_reason().unwrap_or("Error"), ""),
            ),

            Error::InvalidToken => (
                StatusCode::FORBIDDEN,
                ErrorResponse::new("ForbiddenOperationException", "Invalid token."),
            ),

            Error::InvalidCredentials => (
                StatusCode::FORBIDDEN,
                ErrorResponse::new("ForbiddenOperationException", "Invalid credentials."),
            ),

            Error::IllegalArgumentException => (
                StatusCode::BAD_REQUEST,
                ErrorResponse::new("IllegalArgumentException", ""),
            ),

            Error::Undefined(msg) => (
                StatusCode::FORBIDDEN,
                ErrorResponse {
                    error: "ForbiddenOperationException".into(),
                    error_message: "".into(),
                    cause: Some(msg),
                },
            ),
        };

        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use axum::routing::get;
    use axum::Router;
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
