//! The general data models and types used across the whole Aphanite service

use axum::http::StatusCode;
use serde::Serialize;
use toasty::{Deferred, Model};
use uuid::Uuid;

/// A player account.
/// This is used across Yggdrasil and Phenocryst
#[derive(Debug, Clone, Model)]
pub struct User {
    /// The UUID of the User
    #[key]
    #[auto]
    pub id: Uuid,

    /// The email of the User
    #[unique]
    pub email: String,

    /// PHC representation of Argon2 password hash & salt
    /// See [`PasswordHash`](argon2::PasswordHash) for details
    pub password: String,

    pub prefer_language: String,

    /// Instances that this user is allowed to play (Phenocryst only)
    #[has_many(via=instances_relation.instance)]
    pub instances: Deferred<Vec<Instance>>,

    /// Internal relationship constraint; should NOT be used
    #[has_many]
    instances_relation: Deferred<Vec<UserInstance>>,

    #[has_many(pair=owner)]
    pub profiles: Deferred<Vec<crate::service::yggdrasil::types::GameProfile>>,

    #[has_many]
    tokens: Deferred<Vec<Token>>,
}

/// User to Instance relationship maps.
/// This is used to implement multiple-multiple relationships between two models and should NOT be used directly
#[derive(Clone, Debug, Model)]
// `pub` here is not required but without it rustc would produce a warning :(
pub struct UserInstance {
    #[key]
    #[auto]
    id: Uuid,

    #[index]
    user_id: Uuid,

    #[belongs_to(key=user_id, references=id)]
    user: Deferred<User>,

    #[index]
    instance_id: Uuid,

    #[belongs_to(key=instance_id, references=id)]
    instance: Instance,
}

/// An modpack instance.
/// This is used only in Phenocryst
#[derive(Clone, Debug, Model)]
pub struct Instance {
    /// The ID of the instance
    #[key]
    #[auto]
    id: Uuid,

    /// Name of this instance
    name: String,

    /// Description of this instance
    description: String,

    /// Hash of the modpack file.
    /// This should be a BLAKE3 hash in lowercase hex format
    hash: String,

    /// File of the modpack
    file: Uuid,
}

#[derive(Clone, Debug, Model)]
pub struct Token {
    #[key]
    #[auto]
    pub access_token: Uuid,

    #[index]
    user_id: Uuid,

    #[belongs_to(key=user_id, references=id)]
    user: Deferred<User>,

    pub client_token: String,

    #[auto]
    pub created_at: jiff::Timestamp,

    #[index]
    profile_id: Option<Uuid>,

    #[belongs_to(key=profile_id, references=id)]
    pub profile: Option<crate::service::yggdrasil::types::GameProfile>,
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
    status: axum::http::StatusCode,
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
    /// This function performs no checks on the `u16 -> StatusCode` conversion; The caller MUST guarantee that the status code is valid.
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

/// Type alias for [`Result<T,E>`](std::result::Result) where `E` is always [`aphanite::types::Error`]
pub type Result<T> = std::result::Result<T, Error>;
