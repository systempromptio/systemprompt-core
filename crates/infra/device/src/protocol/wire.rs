//! Answer bodies that carry a credential.
//!
//! Each holds its secret in a redacting, zeroizing newtype of
//! [`crate::protocol::secret`], so these stay beside those types; every other
//! body the device exchanges with an instance is a `systemprompt-models` type.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use systemprompt_identifiers::{ClientId, DeviceId, SessionId, UserId};

use crate::protocol::secret::{ClientSecret, DeviceCredential, HookToken, Jwt, Pat};

/// Answer of `POST /v1/auth/bridge/session-pat`: a durable PAT.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionPatResponse {
    pub pat: Pat,
}

/// Answer of `POST /v1/auth/bridge/pat` (PAT as bearer, no body) and of `POST
/// /v1/auth/bridge/session`.
#[derive(Debug, Clone, Deserialize)]
pub struct PatSession {
    pub token: Jwt,
    pub ttl: u64,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub device_token: Option<Jwt>,
}

impl PatSession {
    #[must_use]
    pub fn session_id(&self) -> Option<SessionId> {
        self.headers
            .iter()
            .find(|(name, _)| {
                name.eq_ignore_ascii_case(systemprompt_identifiers::headers::SESSION_ID)
            })
            .map(|(_, value)| value.clone())
            .or_else(|| self.token.session_id())
            .and_then(|raw| SessionId::try_new(raw).ok())
    }
}

/// Answer of `POST /v1/auth/bridge/oauth-client` (the user's JWT, body `{}`):
/// the per-user hook client.
#[derive(Debug, Clone, Deserialize)]
pub struct OAuthClientResponse {
    pub client_id: ClientId,
    pub client_secret: ClientSecret,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub token_endpoint: Option<String>,
}

/// Answer of the hook token request.
#[derive(Debug, Clone, Deserialize)]
pub struct HookTokenResponse {
    pub access_token: HookToken,
    #[serde(default)]
    pub token_type: Option<String>,
    pub expires_in: i64,
    #[serde(default)]
    pub scope: Option<String>,
}

/// Answer of `POST /v1/bridge/device`. The same fingerprint is the same
/// `device_id`; each call issues a new credential.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceEnrolment {
    pub device_id: DeviceId,
    pub consumer_id: UserId,
    pub credential: DeviceCredential,
}

/// The sync token the device token was traded for, and when it expires.
#[derive(Debug, Clone, Deserialize)]
pub struct SyncTokenAnswer {
    pub token: Jwt,
    pub expires_at: DateTime<Utc>,
}
