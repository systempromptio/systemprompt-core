//! Wire contract for `GET /v1/bridge/profile`.
//!
//! The desktop bridge (`bin/bridge`) fetches this to render host configuration
//! and to decide which provider models each host advertises. The server
//! (`crates/entry/api`) produces it and the bridge consumes it through these
//! exact types, so the two sides cannot drift. The builder that derives every
//! field from the provider registry lives with the services manifest
//! (`systemprompt_manifest::bridge_profile`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::providers::ApiSurface;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeProfileResponse {
    pub inference_gateway_base_url: String,
    pub auth_scheme: String,
    #[serde(default)]
    pub models: Vec<String>,
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
    pub model_limits: BTreeMap<String, AdvertisedLimits>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdvertisedLimits {
    pub context_window: u32,
    pub max_output_tokens: u32,
}

/// A provider whose credential secret is absent is flagged
/// (`configured = false`) rather than dropped silently.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub name: String,
    pub surface: ApiSurface,
    pub configured: bool,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_issue: Option<String>,
}
