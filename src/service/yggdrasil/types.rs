//! Core data model used in Yggdrasil

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Profile / User

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameProfile {
    pub id: String,
    pub name: String,
    pub properties: Vec<ProfileProperty>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileProperty {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

// Textures

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TexturesPayload {
    pub timestamp: i64,
    pub profile_id: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkinModel {
    Slim,
}
