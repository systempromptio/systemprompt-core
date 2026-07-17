//! Wire contract for `GET /v1/bridge/profile`.
//!
//! The desktop bridge (`bin/bridge`) fetches this to render host configuration
//! and to decide which provider models each host advertises. The server
//! (`crates/entry/api`) produces it and the bridge consumes it through these
//! exact types, so the two sides cannot drift.
//!
//! Every field is derived in [`build`] from
//! [`ProviderRegistry::advertised_providers`], the single bearer of the
//! advertisement rule ([`ApiSurface::is_advertised`]). A `surface: backend`
//! provider is therefore structurally absent from both `providers` and the
//! flat `models` front door — the flat list is a projection of the same
//! advertised set, so it can never disagree with `providers`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use crate::profile::{ApiSurface, ProviderRegistry};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeProfileResponse {
    pub inference_gateway_base_url: String,
    pub auth_scheme: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub organization_uuid: Option<String>,
    #[serde(default)]
    pub providers: Vec<ProviderHealth>,
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

pub fn provider_health(
    registry: &ProviderRegistry,
    secret_present: impl Fn(&str) -> bool,
) -> Vec<ProviderHealth> {
    registry
        .advertised_providers()
        .map(|entry| {
            let secret = entry.api_key_secret.as_str();
            let configured = secret_present(secret);
            ProviderHealth {
                name: entry.name.as_str().to_owned(),
                surface: entry.surface,
                configured,
                models: entry
                    .models
                    .iter()
                    .flat_map(|m| {
                        std::iter::once(m.id.as_str().to_owned())
                            .chain(m.aliases.iter().map(|a| a.as_str().to_owned()))
                    })
                    .collect(),
                config_issue: (!configured)
                    .then(|| format!("API key secret '{secret}' is not configured")),
            }
        })
        .collect()
}

#[must_use]
pub fn build(
    inference_gateway_base_url: String,
    auth_scheme: String,
    organization_uuid: Option<String>,
    registry: &ProviderRegistry,
    secret_present: impl Fn(&str) -> bool,
) -> BridgeProfileResponse {
    BridgeProfileResponse {
        inference_gateway_base_url,
        auth_scheme,
        models: registry.advertised_model_ids(&[ApiSurface::Anthropic]),
        organization_uuid,
        providers: provider_health(registry, secret_present),
    }
}
