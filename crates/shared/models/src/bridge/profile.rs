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
//! flat `models` front door. The flat `models` list is the *whole* advertised
//! set, not one family's projection: the gateway transcodes every inbound wire
//! to every provider wire, so every advertised model is reachable from every
//! host, provided it is servable ([`is_model_servable`]): a model whose
//! serving provider has no credential is left out of `models` and
//! `model_limits`, though its provider still appears in `providers` flagged
//! `configured = false`. `providers` carries the per-provider split the bridge uses to build
//! the narrower per-host views (Claude Desktop being the only host that
//! narrows).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::services::{ApiSurface, GatewayConfig, ProviderRegistry};

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

fn advertised_limits(
    registry: &ProviderRegistry,
    servable: impl Fn(&str) -> bool,
) -> BTreeMap<String, AdvertisedLimits> {
    registry
        .advertised_providers()
        .flat_map(|entry| entry.models.iter())
        .filter(|model| !model.hidden && model.limits.context_window > 0)
        .filter(|model| servable(model.id.as_str()))
        .map(|model| {
            (
                model.id.as_str().to_owned(),
                AdvertisedLimits {
                    context_window: model.limits.context_window,
                    max_output_tokens: model.limits.max_output_tokens,
                },
            )
        })
        .collect()
}

pub const KNOWN_HOSTS: &[&str] = &[
    "claude-code",
    "claude-desktop",
    "codex-cli",
    "hermes",
    "opencode",
];

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
                    .filter(|model| !model.hidden)
                    .map(|model| model.id.as_str().to_owned())
                    .collect(),
                config_issue: (!configured)
                    .then(|| format!("API key secret '{secret}' is not configured")),
            }
        })
        .collect()
}

/// Whether the gateway can dispatch `model` to a provider whose credential is
/// present.
///
/// Why: model ids are unique to one catalog provider, but a gateway route may
/// send that id elsewhere (Claude ids routed to Vertex AI), so the route's
/// provider, or its fallback, decides; with no matching route the
/// `default_provider` does, and without a gateway the declaring provider.
pub fn is_model_servable(
    registry: &ProviderRegistry,
    gateway: Option<&GatewayConfig>,
    model: &str,
    secret_present: &impl Fn(&str) -> bool,
) -> bool {
    let configured = |provider: &str| {
        registry
            .find_provider(provider)
            .is_some_and(|entry| secret_present(entry.api_key_secret.as_str()))
    };
    if let Some(gw) = gateway {
        if let Some(route) = gw.find_route(model) {
            return configured(route.provider.as_str())
                || route
                    .fallback_provider
                    .as_ref()
                    .is_some_and(|fallback| configured(fallback.as_str()));
        }
        if let Some(default) = gw.default_provider.as_ref() {
            return configured(default.as_str());
        }
    }
    registry
        .providers
        .iter()
        .filter(|entry| entry.find_model(model).is_some())
        .any(|entry| secret_present(entry.api_key_secret.as_str()))
}

#[derive(Debug, Clone)]
pub struct BridgeProfileParams<'a> {
    pub inference_gateway_base_url: String,
    pub auth_scheme: String,
    pub organization_uuid: Option<String>,
    pub default_model: Option<String>,
    pub registry: &'a ProviderRegistry,
    pub gateway: Option<&'a GatewayConfig>,
}

#[must_use]
pub fn build(
    params: BridgeProfileParams<'_>,
    secret_present: impl Fn(&str) -> bool,
) -> BridgeProfileResponse {
    let BridgeProfileParams {
        inference_gateway_base_url,
        auth_scheme,
        organization_uuid,
        default_model,
        registry,
        gateway,
    } = params;
    let servable = |model: &str| is_model_servable(registry, gateway, model, &secret_present);
    BridgeProfileResponse {
        inference_gateway_base_url,
        auth_scheme,
        models: registry
            .advertised_model_ids(&[])
            .into_iter()
            .filter(|model| servable(model))
            .collect(),
        default_model,
        organization_uuid,
        model_limits: advertised_limits(registry, servable),
        providers: provider_health(registry, &secret_present),
    }
}
