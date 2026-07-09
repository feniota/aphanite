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

impl From<crate::types::User> for UserPayload {
    fn from(user: crate::types::User) -> Self {
        Self {
            id: user.id,
            name: user.nickname,
            email: user.email,
            permissions: Permission::from_u32(user.permission),
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
