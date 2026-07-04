//! Core data model used only in Yggdrasil service

use base64::Engine;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use toasty::{Deferred, Embed, Model};
use uuid::Uuid;

// UUID
#[derive(Debug, Clone)]
pub struct UnhyphenatedUuid(Uuid);

impl From<Uuid> for UnhyphenatedUuid {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<UnhyphenatedUuid> for Uuid {
    fn from(value: UnhyphenatedUuid) -> Self {
        value.0
    }
}

impl Serialize for UnhyphenatedUuid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = self.0.simple().to_string();
        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for UnhyphenatedUuid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&s).map_err(serde::de::Error::custom)?;
        Ok(UnhyphenatedUuid(uuid))
    }
}

// Profile / User

/// [`GameProfile`] without database annotations for API exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeableGameProfile {
    pub id: UnhyphenatedUuid,
    pub name: String,
    #[serde(skip_deserializing)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Vec<ProfileProperty>>,
}

impl ExchangeableGameProfile {
    fn from(profile: &GameProfile, properties_included: bool) -> Self {
        let profile = profile.clone();
        Self {
            id: profile.id.into(),
            name: profile.name,
            properties: if properties_included {
                Some(profile.properties)
            } else {
                None
            },
        }
    }
}

/// A Minecraft player profile
/// This is NOT intended to be used in data exchange with Yggdrasil clients for some database-specific fields.
/// Please use [`ExchangeableGameProfile`] instead.
#[derive(Debug, Clone, Serialize, Model)]
pub struct GameProfile {
    /// UUID of this player profile
    #[key]
    #[auto]
    pub id: Uuid,

    /// Player name of this profile
    pub name: String,

    /// Properties of this profile
    #[has_many(pair=profile)]
    pub properties: Vec<ProfileProperty>,

    /// Internal field for database relationship
    #[serde(skip_serializing)]
    owner_id: Uuid,

    /// Associated user account of this profile
    #[belongs_to(key=owner_id, references=id)]
    #[serde(skip_serializing)]
    owner: crate::types::User,
}

/// A property of player profile
/// This "property" is basically a KV pair with an optional signature.
#[derive(Debug, Clone, Serialize, Model)]
#[table = "game_profile_property"]
pub struct ProfileProperty {
    /// Internal ID of this property item
    #[key]
    #[auto]
    #[serde(skip_serializing)]
    id: Uuid,

    /// Internal field for database relationship
    #[serde(skip_serializing)]
    profile_id: Uuid,

    /// The profile this property belongs to
    #[belongs_to(key=profile_id, references=id)]
    #[serde(skip_serializing)]
    pub profile: Deferred<GameProfile>,

    /// The key of the property
    #[index]
    pub name: String,

    // The value of the property
    pub value: String,

    /// The signature of the property
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

// Textures

/// Texture of a player profile
/// This type is designed especially for database storage and guaranteed NOT to be compatible with the API payload type.
/// For API usage use [`TexturesPayload`] instead.
#[derive(Debug, Clone, Model)]
pub struct ProfileTextures {
    #[key]
    #[auto]
    pub id: Uuid,

    #[auto]
    pub created_at: jiff::Timestamp,

    profile_id: Uuid,

    #[belongs_to(key=profile_id, references=id)]
    pub profile: GameProfile,

    pub skin_model: SkinModel,
    pub skin_file: Uuid,
    pub cape_file: Uuid,
}

/// Texture (i.e. skin and cape) of a player profile
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TexturesPayload {
    pub timestamp: i64,
    pub profile_id: UnhyphenatedUuid,
    pub profile_name: String,
    pub textures: TextureMap,
}

// Texture map

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureMap {
    #[serde(flatten)]
    pub textures: HashMap<TextureType, Texture>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TextureType {
    Skin,
    Cape,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Texture {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<SkinMetadata>,
}

// Skin metadata

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkinMetadata {
    pub model: SkinModel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Embed)]
#[serde(rename_all = "lowercase")]
pub enum SkinModel {
    #[column(variant = 0)]
    Default,
    #[column(variant = 1)]
    Slim,
}

#[derive(Debug, Clone)]
pub struct TexturesBase64(TexturesPayload);

impl Serialize for TexturesBase64 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let json = serde_json::to_string(&self.0).map_err(serde::ser::Error::custom)?;

        let encoded = base64::engine::general_purpose::STANDARD.encode(json);

        serializer.serialize_str(&encoded)
    }
}

impl<'de> Deserialize<'de> for TexturesBase64 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        let decoded = base64::engine::general_purpose::STANDARD
            .decode(s)
            .map_err(serde::de::Error::custom)?;

        let payload: TexturesPayload =
            serde_json::from_slice(&decoded).map_err(serde::de::Error::custom)?;

        Ok(TexturesBase64(payload))
    }
}
