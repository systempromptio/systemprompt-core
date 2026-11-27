//! Sign-in and credential bodies.
//!
//! Capabilities, health, the one-time-code exchanges, PAT to JWT, the manifest
//! key, the per-user hook OAuth client and its `client_credentials` hook token
//! (PROTOCOL sections 6, 7 and 9).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::{ClientId, PluginId, SessionId};
use crate::secret::{ClientSecret, HookToken, Jwt, Pat};

/// `GET /v1/auth/bridge/capabilities`. An instance before 0.76 answers `404`,
/// which means no modes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(default)]
    pub modes: Vec<String>,
}

impl Capabilities {
    pub const MODE_PAT: &'static str = "pat";
    pub const MODE_SESSION: &'static str = "session";
    pub const MODE_OAUTH_CLIENT: &'static str = "oauth-client";

    #[must_use]
    pub fn offers(&self, mode: &str) -> bool {
        self.modes.iter().any(|offered| offered == mode)
    }
}

/// `GET /api/v1/health`. A failed or undecodable answer only leaves the version
/// unknown.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Health {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

/// Body of `POST /v1/auth/bridge/session-pat`: the one-time code and the name
/// the PAT gets in the user's token list.
#[derive(Serialize)]
pub struct SessionPatRequest<'a> {
    pub code: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<&'a str>,
}

impl fmt::Debug for SessionPatRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionPatRequest")
            .field("code", &"<redacted>")
            .field("device_name", &self.device_name)
            .finish()
    }
}

/// Answer of `POST /v1/auth/bridge/session-pat`: a durable PAT.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionPatResponse {
    pub pat: Pat,
}

/// Body of `POST /v1/auth/bridge/session`: the one-time code, exchanged for a
/// JWT and no durable credential.
#[derive(Serialize)]
pub struct SessionExchangeRequest<'a> {
    pub code: &'a str,
}

impl fmt::Debug for SessionExchangeRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionExchangeRequest")
            .field("code", &"<redacted>")
            .finish()
    }
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
            .find(|(name, _)| name.eq_ignore_ascii_case(crate::wire::headers::SESSION_ID))
            .map(|(_, value)| value.clone())
            .or_else(|| self.token.session_id())
            .and_then(|raw| SessionId::try_new(raw).ok())
    }
}

/// `GET /v1/bridge/pubkey`: base64 of the 32 raw bytes of the instance's
/// ed25519 manifest key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PubkeyResponse {
    pub pubkey: String,
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

pub const HOOK_TOKEN_SCOPE: &str = "hook:govern hook:track";
pub const HOOK_TOKEN_AUDIENCE: &str = "hook";
pub const CLIENT_CREDENTIALS_GRANT: &str = "client_credentials";

/// Form body (`application/x-www-form-urlencoded`) of the hook token request to
/// the client's token endpoint.
#[derive(Serialize)]
pub struct HookTokenRequest<'a> {
    pub grant_type: &'static str,
    pub client_id: &'a ClientId,
    pub client_secret: &'a str,
    pub scope: &'static str,
    pub audience: &'static str,
    pub plugin_id: &'a PluginId,
}

impl<'a> HookTokenRequest<'a> {
    #[must_use]
    pub const fn new(
        client_id: &'a ClientId,
        client_secret: &'a str,
        plugin_id: &'a PluginId,
    ) -> Self {
        Self {
            grant_type: CLIENT_CREDENTIALS_GRANT,
            client_id,
            client_secret,
            scope: HOOK_TOKEN_SCOPE,
            audience: HOOK_TOKEN_AUDIENCE,
            plugin_id,
        }
    }

    #[must_use]
    pub fn fields(&self) -> [(&'static str, &'a str); 6] {
        [
            ("grant_type", self.grant_type),
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret),
            ("scope", self.scope),
            ("audience", self.audience),
            ("plugin_id", self.plugin_id.as_str()),
        ]
    }
}

impl fmt::Debug for HookTokenRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HookTokenRequest")
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .field("plugin_id", &self.plugin_id)
            .finish_non_exhaustive()
    }
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
