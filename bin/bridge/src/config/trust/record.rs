//! The trust record shape: a normalized gateway identity, a validated Ed25519
//! key and the channel that supplied it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ValidatedUrl;

use super::TrustError;
use crate::ids::PinnedPubKey;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GatewayIdentity(String);

impl TryFrom<String> for GatewayIdentity {
    type Error = TrustError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let mut url = url::Url::parse(&value)?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(TrustError::GatewayShape);
        }
        let path = url.path().trim_end_matches('/').to_owned();
        url.set_path(&path);
        Ok(Self(url.to_string().trim_end_matches('/').to_owned()))
    }
}

impl From<GatewayIdentity> for String {
    fn from(value: GatewayIdentity) -> Self {
        value.0
    }
}

impl std::fmt::Display for GatewayIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl GatewayIdentity {
    pub fn new(url: &ValidatedUrl) -> Result<Self, TrustError> {
        Self::try_from(url.as_str().to_owned())
    }

    pub fn parse(url: &str) -> Result<Self, TrustError> {
        Self::try_from(url.to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinSource {
    Policy,
    Operator,
}

impl PinSource {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Policy => "policy (env or managed policy)",
            Self::Operator => "config file",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustRecord {
    pub gateway: GatewayIdentity,
    pub key: PinnedPubKey,
    pub source: PinSource,
}

impl TrustRecord {
    pub fn new(gateway: &ValidatedUrl, key: &str, source: PinSource) -> Result<Self, TrustError> {
        let bytes = base64::engine::general_purpose::STANDARD.decode(key)?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|bytes: Vec<u8>| TrustError::KeyLength {
                actual: bytes.len(),
            })?;
        ed25519_dalek::VerifyingKey::from_bytes(&bytes)?;
        Ok(Self {
            gateway: GatewayIdentity::new(gateway)?,
            key: PinnedPubKey::new(key),
            source,
        })
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SyncConfig {
    #[serde(default)]
    pub trust: Option<TrustRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinnedPubkeyState {
    Pinned {
        key: PinnedPubKey,
        source: PinSource,
    },
    StaleForGateway {
        pinned_for: GatewayIdentity,
        current: GatewayIdentity,
    },
    Unpinned,
}
