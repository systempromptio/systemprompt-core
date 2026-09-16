//! Wire types exchanged with the systemprompt gateway: provisioned OAuth client
//! credentials, plugin hook tokens, and the `whoami` identity envelope.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod auth;
pub use auth::*;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use systemprompt_identifiers::{ClientId, DeviceId, TenantId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeOAuthClientResponse {
    pub client_id: ClientId,
    pub client_secret: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub token_endpoint: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HookTokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub token_type: Option<String>,
    pub expires_in: i64,
    #[serde(default)]
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoamiResponse {
    #[serde(default)]
    pub user_id: Option<UserId>,
    #[serde(default)]
    pub tenant_id: Option<TenantId>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// Body of `POST /v1/bridge/device`: the self-issued device fingerprint the
/// bridge wants enrolled under the authenticated user, plus a display label.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfEnrollRequest {
    pub fingerprint: String,
    pub label: String,
}

/// Reply to `POST /v1/bridge/device`: the enrolled device and the
/// `sp_device_` credential that attributes installation feedback to it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfEnrollResponse {
    pub device_id: DeviceId,
    pub consumer_id: UserId,
    pub credential: String,
}

/// One platform's newest published build, as advertised by
/// `GET /v1/bridge/latest`. `sha256` is the digest the updater must reproduce
/// over the downloaded bytes before it will install them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseManifest {
    pub version: String,
    pub sha256: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub notes_url: Option<String>,
}
