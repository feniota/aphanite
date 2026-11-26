//! Shared types for the General API

use serde::Serialize;
use uuid::Uuid;

use crate::types::Permission;

/// Serializable user representation for API responses
#[derive(Serialize)]
pub struct UserPayload {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub permissions: Vec<Permission>,
}

impl From<crate::data::entity::users::Model> for UserPayload {
    fn from(user: crate::data::entity::users::Model) -> Self {
        Self {
            id: user.id,
            name: user.nickname,
            email: user.email,
            permissions: Permission::parse_flags(user.permission.0),
        }
    }
}

/// Serializable profile (GameProfile) representation for General API responses
#[derive(Serialize)]
pub struct ProfilePayload {
    pub id: Uuid,
    pub name: String,
    pub owner: Uuid,
}
