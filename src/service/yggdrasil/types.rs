//! Core data model used only in Yggdrasil service

use rsa::PrivateKey;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::HashMap, net::IpAddr};
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
    /// Get a ExchangeableGameProfile from GameProfile
    pub async fn new(
        db: &mut toasty::Db,
        storage: crate::storage::AssetStorage,
        profile: &GameProfile,
        properties_included: bool,
        signature_required: Option<&PrivateKey>,
    ) -> Self {
        let profile = profile.clone();
        let textures_value = TexturesPayload::new(db, storage, &profile)
            .await
            .to_payload_string();

        let (textures_signature, uploadable_signature) =
            if let Some(private_key) = signature_required {
                use base64::prelude::{BASE64_STANDARD, Engine as _};
                use rsa::pkcs1v15::Pkcs1v15Sign;
                use sha1::Digest;

                let sha1_digest = |data: &[u8]| -> Vec<u8> {
                    let mut hasher = sha1::Sha1::new();
                    hasher.update(data);
                    hasher.finalize().to_vec()
                };

                let textures_sig = BASE64_STANDARD.encode(
                    private_key
                        .sign(
                            Pkcs1v15Sign::new::<sha1::Sha1>(),
                            &sha1_digest(textures_value.as_bytes()),
                        )
                        .unwrap(),
                );

                let uploadable_sig = BASE64_STANDARD.encode(
                    private_key
                        .sign(
                            Pkcs1v15Sign::new::<sha1::Sha1>(),
                            &sha1_digest(b"skin,cape"),
                        )
                        .unwrap(),
                );

                (Some(textures_sig), Some(uploadable_sig))
            } else {
                (None, None)
            };

        Self {
            id: profile.id.into(),
            name: profile.name.clone(),
            properties: if properties_included {
                Some(vec![
                    ProfileProperty {
                        name: "uploadableTextures".into(),
                        value: "skin,cape".into(),
                        signature: uploadable_signature,
                    },
                    ProfileProperty {
                        name: "textures".into(),
                        value: textures_value,
                        signature: textures_signature,
                    },
                ])
            } else {
                None
            },
        }
    }
}

/// A Minecraft player profile
/// This is NOT intended to be used in data exchange with Yggdrasil clients for some database-specific fields.
/// Please use [`ExchangeableGameProfile`] instead.
#[derive(Debug, Clone, Model)]
pub struct GameProfile {
    /// UUID of this player profile
    #[key]
    #[auto]
    pub id: Uuid,

    /// Player name of this profile
    #[index]
    pub name: String,

    /// Internal field for database relationship
    #[index]
    pub owner_id: Uuid,

    #[has_one(pair=profile)]
    pub textures: Deferred<Option<ProfileTextures>>,

    /// Associated user account of this profile
    #[belongs_to(key=owner_id, references=id)]
    pub owner: Deferred<crate::types::User>,
}

/// A property of a game profile
/// This "property" is basically a KV pair with an optional signature.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProfileProperty {
    /// The key of the property
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

    #[index]
    profile_id: Uuid,

    #[belongs_to(key=profile_id, references=id)]
    pub profile: Deferred<GameProfile>,

    pub skin_model: SkinModel,
    pub skin_file: Option<Uuid>,
    pub cape_file: Option<Uuid>,
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

impl TexturesPayload {
    async fn new(
        db: &mut toasty::Db,
        storage: crate::storage::AssetStorage,
        profile: &GameProfile,
    ) -> Self {
        let maybe_textures = profile.textures().exec(db).await.ok().flatten();
        let mut textures_hashmap = HashMap::new();
        let timestamp = if let Some(ref textures) = maybe_textures {
            if let Some(f) = textures.skin_file
                && let Some(skin_url) = storage.get_url(f).await
            {
                textures_hashmap.insert(
                    TextureType::Skin,
                    Texture {
                        url: skin_url,
                        metadata: Some(SkinMetadata {
                            model: textures.skin_model,
                        }),
                    },
                );
            }
            if let Some(f) = textures.cape_file
                && let Some(skin_url) = storage.get_url(f).await
            {
                textures_hashmap.insert(
                    TextureType::Cape,
                    Texture {
                        url: skin_url,
                        metadata: None,
                    },
                );
            }
            textures.created_at.as_millisecond()
        } else {
            0
        };
        Self {
            timestamp,
            profile_id: profile.id.into(),
            profile_name: profile.name.clone(),
            textures: TextureMap {
                textures: textures_hashmap,
            },
        }
    }
    fn to_payload_string(&self) -> String {
        use base64::prelude::{BASE64_STANDARD, Engine as _};
        BASE64_STANDARD.encode(serde_json::to_string(&self).unwrap())
    }
    // fn from_payload_string(payload: String) -> anyhow::Result<Self> {
    //     use base64::prelude::{BASE64_STANDARD, Engine as _};

    //     let decoded = BASE64_STANDARD.decode(payload)?;
    //     Ok(serde_json::from_slice(&decoded)?)
    // }
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

pub struct AphaniteClientIp(pub IpAddr);

impl ntex::web::FromRequest<ntex::web::AppState<crate::AppState>> for AphaniteClientIp {
    type Error = crate::service::Error;

    async fn from_request(
        state: &ntex::web::AppState<crate::AppState>,
        request: &ntex::web::HttpRequest,
        _: &mut ntex::http::Payload,
    ) -> Result<Self, Self::Error> {
        extract_client_ip(&state.cfg.service.client_ip, request.headers()).map(Self)
    }
}

fn extract_client_ip(
    source: &crate::config::ReverseProxyType,
    headers: &ntex::http::HeaderMap,
) -> Result<IpAddr, crate::service::Error> {
    use crate::config::ReverseProxyType;
    use forwarded_header_value::ForwardedHeaderValue;

    let header_name = match source {
        ReverseProxyType::Disabled => return Ok(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)),
        ReverseProxyType::CfConnectingIp => "cf-connecting-ip",
        ReverseProxyType::CloudFrontViewerAddress => "cloudfront-viewer-address",
        ReverseProxyType::FlyClientIp => "fly-client-ip",
        ReverseProxyType::Forwarded => "forwarded",
        ReverseProxyType::XForwardedFor => "x-forwarded-for",
        ReverseProxyType::TrueClientIp => "true-client-ip",
        ReverseProxyType::XEnvoyExternalAddress => "x-envoy-external-address",
        ReverseProxyType::XRealIp => "x-real-ip",
    };
    let invalid =
        || crate::service::Error::error(400, format!("Cannot extract IP from {header_name}"));
    let values = headers
        .get_all(header_name)
        .map(|value| value.to_str())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid())?;
    let value = values.join(",");
    let ip =
        match source {
            ReverseProxyType::Forwarded => ForwardedHeaderValue::from_forwarded(&value)
                .ok()
                .and_then(|value| {
                    value
                        .proximate()
                        .forwarded_for
                        .as_ref()
                        .and_then(|id| id.ip())
                }),
            ReverseProxyType::XForwardedFor => value
                .rsplit(',')
                .next()
                .and_then(|ip| ip.trim().parse().ok()),
            ReverseProxyType::CloudFrontViewerAddress => value
                .parse::<std::net::SocketAddr>()
                .ok()
                .map(|addr| addr.ip()),
            _ => value.parse().ok(),
        };
    ip.ok_or_else(invalid)
}

// Enita: Serialize 和 Deserialize 指的是和 serde 内部的数据结构交换，而不是和最终的成品 JSON；
// 用这两个 trait 将它转换成字符串会不会有点不太妥当？我觉得用自己定义的函数可能更合适一点。
//
// #[derive(Debug, Clone)]
// pub struct TexturesBase64(TexturesPayload);

// impl Serialize for TexturesBase64 {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: Serializer,
//     {
//         let json = serde_json::to_string(&self.0).map_err(serde::ser::Error::custom)?;

//         let encoded = base64::engine::general_purpose::STANDARD.encode(json);

//         serializer.serialize_str(&encoded)
//     }
// }

// impl<'de> Deserialize<'de> for TexturesBase64 {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         let s = String::deserialize(deserializer)?;

//         let decoded = base64::engine::general_purpose::STANDARD
//             .decode(s)
//             .map_err(serde::de::Error::custom)?;

//         let payload: TexturesPayload =
//             serde_json::from_slice(&decoded).map_err(serde::de::Error::custom)?;

//         Ok(TexturesBase64(payload))
//     }
// }
