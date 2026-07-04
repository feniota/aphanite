//! The general data models and types used across the whole Aphanite service

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
    pub created_at: jiff::Timestamp,

    #[belongs_to(key=user_id, references=id)]
    pub profile: Option<crate::service::yggdrasil::types::GameProfile>,
}
