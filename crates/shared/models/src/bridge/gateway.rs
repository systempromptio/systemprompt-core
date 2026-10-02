//! Request and response bodies of the gateway's bridge identity, enrolment,
//! PAT and release-feed endpoints.
//!
//! The gateway serialises these and the bridge deserialises them, so a field
//! renamed here changes both sides at once; a shipped bridge still reads the
//! old shape, which is why fields are only ever added as optional.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{DeviceId, TenantId, UserId};

/// Body of `GET /v1/bridge/whoami`.
///
/// The stock gateway fills `user_id`, `email`, `display_name` and `roles`. A
/// white-label identity endpoint may also answer `tenant_id`, `provider` and
/// keys of its own, which the bridge carries through `extra`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WhoamiResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<UserId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<TenantId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    // JSON: identity endpoint response — unknown claims kept via `flatten`.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// Body of `POST /v1/bridge/device`: the self-issued device fingerprint the
/// bridge wants enrolled under the authenticated user, plus a display label.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelfEnrollRequest {
    pub fingerprint: String,
    pub label: String,
}

/// Reply to `POST /v1/bridge/device`: the enrolled device and the
/// `sp_device_` credential that attributes installation feedback to it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SelfEnrollResponse {
    pub device_id: DeviceId,
    pub consumer_id: UserId,
    pub credential: String,
}

/// Reply to `POST /v1/auth/bridge/session-pat`: the durable PAT minted from a
/// one-time session exchange code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevicePatResponse {
    pub pat: String,
}

/// One platform's newest published build, as advertised by
/// `GET /v1/bridge/latest`. `sha256` is the digest the updater must reproduce
/// over the downloaded bytes before it will install them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseManifest {
    pub version: String,
    pub sha256: String,
    pub size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes_url: Option<String>,
}
