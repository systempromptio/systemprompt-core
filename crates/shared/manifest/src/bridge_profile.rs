//! Builder for the `GET /v1/bridge/profile` payload.
//!
//! The desktop bridge (`bin/bridge`) fetches this to render host configuration
//! and to decide which provider models each host advertises. The server
//! (`crates/entry/api`) produces it and the bridge consumes it through these
//! exact types, so the two sides cannot drift.
//!
//! Every field is derived in [`build`] from
//! [`ProviderRegistry::advertised_providers`], the single bearer of the
//! advertisement rule
//! ([`ApiSurface::is_advertised`](systemprompt_models::providers::ApiSurface::is_advertised)).
//! A `surface: backend` provider is therefore structurally absent from both
//! `providers` and the flat `models` front door. The flat `models` list is the
//! *whole* advertised set, not one family's projection: the gateway transcodes
//! every inbound wire to every provider wire, so every advertised model is
//! reachable from every host, provided it is servable ([`is_model_servable`]):
//! a model whose serving provider has no credential is left out of `models` and
//! `model_limits`, though its provider still appears in `providers` flagged
//! `configured = false`. `providers` carries the per-provider split the bridge
//! uses to build the narrower per-host views (Claude Desktop being the only
//! host that narrows).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use systemprompt_models::bridge::profile::{
    AdvertisedLimits, BridgeProfileResponse, ProviderHealth,
};

use crate::services::{GatewayConfig, ProviderEntry, ProviderRegistry};

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

fn entry_configured(
    entry: &ProviderEntry,
    secret_present: &impl Fn(&str) -> bool,
    personal_keys: &BTreeSet<String>,
) -> bool {
    if entry.is_user_credential() {
        return personal_keys.contains(entry.name.as_str());
    }
    entry
        .platform_secret()
        .is_some_and(|secret| secret_present(secret.as_str()))
}

pub fn provider_health(
    registry: &ProviderRegistry,
    secret_present: impl Fn(&str) -> bool,
) -> Vec<ProviderHealth> {
    provider_health_for_user(registry, &secret_present, &BTreeSet::new())
}

pub fn provider_health_for_user(
    registry: &ProviderRegistry,
    secret_present: &impl Fn(&str) -> bool,
    personal_keys: &BTreeSet<String>,
) -> Vec<ProviderHealth> {
    registry
        .advertised_providers()
        .map(|entry| {
            let configured = entry_configured(entry, secret_present, personal_keys);
            let config_issue = (!configured).then(|| {
                entry.platform_secret().map_or_else(
                    || "personal key not added — add your own key in Settings".to_owned(),
                    |secret| format!("API key secret '{}' is not configured", secret.as_str()),
                )
            });
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
                config_issue,
                display_name: entry.display_name.clone(),
                personal_key: entry.is_user_credential(),
            }
        })
        .collect()
}

pub fn is_model_servable(
    registry: &ProviderRegistry,
    gateway: Option<&GatewayConfig>,
    model: &str,
    secret_present: &impl Fn(&str) -> bool,
) -> bool {
    is_model_servable_for_user(registry, gateway, model, secret_present, &BTreeSet::new())
}

pub fn is_model_servable_for_user(
    registry: &ProviderRegistry,
    gateway: Option<&GatewayConfig>,
    model: &str,
    secret_present: &impl Fn(&str) -> bool,
    personal_keys: &BTreeSet<String>,
) -> bool {
    let configured = |provider: &str| {
        registry
            .find_provider(provider)
            .is_some_and(|entry| entry_configured(entry, secret_present, personal_keys))
    };
    if let Some(gw) = gateway {
        if let Some(route) = gw.find_route(model) {
            return route
                .all_chains()
                .iter()
                .flat_map(|(_, views)| views)
                .any(|view| configured(view.provider.as_str()));
        }
        if let Some(default) = gw.default_provider.as_ref() {
            return configured(default.as_str());
        }
    }
    registry
        .providers
        .iter()
        .filter(|entry| entry.find_model(model).is_some())
        .any(|entry| entry_configured(entry, secret_present, personal_keys))
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
    build_for_user(params, secret_present, &BTreeSet::new())
}

#[must_use]
pub fn build_for_user(
    params: BridgeProfileParams<'_>,
    secret_present: impl Fn(&str) -> bool,
    personal_keys: &BTreeSet<String>,
) -> BridgeProfileResponse {
    let BridgeProfileParams {
        inference_gateway_base_url,
        auth_scheme,
        organization_uuid,
        default_model,
        registry,
        gateway,
    } = params;
    let servable = |model: &str| {
        is_model_servable_for_user(registry, gateway, model, &secret_present, personal_keys)
    };
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
        providers: provider_health_for_user(registry, &secret_present, personal_keys),
    }
}
