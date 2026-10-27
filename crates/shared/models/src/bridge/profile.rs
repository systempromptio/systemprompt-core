//! Wire contract for `GET /v1/bridge/profile`.
//!
//! The desktop bridge (`bin/bridge`) fetches this to render host configuration
//! and to decide which provider models each host advertises. The server
//! (`crates/entry/api`) produces it and the bridge consumes it through these
//! exact types, so the two sides cannot drift. The builder that derives every
//! field from the resolved catalog lives with the services manifest
//! (`systemprompt_manifest::bridge_profile`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ModelId;

use crate::providers::ClientFamily;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeProfileResponse {
    pub inference_gateway_base_url: String,
    pub auth_scheme: String,
    #[serde(default)]
    pub models: Vec<ModelId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    #[serde(default)]
    pub organization_uuid: Option<String>,
    #[serde(default)]
    pub providers: Vec<ProviderHealth>,
    // Why: hosts that take per-model limits (OpenCode) otherwise size every
    // gateway model with their own default and cut a 1M model short. Absent
    // from an older server, so it defaults to empty.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub model_limits: BTreeMap<ModelId, AdvertisedLimits>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub model_names: BTreeMap<ModelId, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdvertisedLimits {
    pub context_window: u32,
    pub max_output_tokens: u32,
}

/// One provider with advertised models, and whether it can serve requests.
///
/// A provider whose credential secret is absent is flagged
/// (`configured = false`) rather than dropped silently. A `personal_key`
/// provider is served only from a key the user stores for themselves under
/// `credential` (a personal subscription); `configured` then means this user
/// has stored one. `clients` is the union of the client families its
/// advertised models are listed to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub name: String,
    pub credential: String,
    #[serde(default)]
    pub clients: Vec<ClientFamily>,
    pub configured: bool,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_issue: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub personal_key: bool,
}

/// One row of `GET /v1/me/provider-credentials`: a personal credential, and
/// what is known about the caller's stored key without opening it. The key
/// itself is never returned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCredentialStatus {
    pub credential: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub configured: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Body of `PUT /v1/me/provider-credentials/{credential}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetProviderCredentialRequest {
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}
