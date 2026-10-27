//! Builder for the `GET /v1/bridge/profile` payload.
//!
//! The desktop bridge (`bin/bridge`) fetches this to render host configuration
//! and to decide which models each host advertises. The server
//! (`crates/entry/api`) produces it and the bridge consumes it through these
//! exact types, so the two sides cannot drift.
//!
//! Every field is derived from the [`ResolvedCatalog`]. The flat `models`
//! list is every advertised model (`clients` not empty) that is servable for
//! the caller: the model resolves, and some hop of its chain has a configured
//! credential (the platform secret is present, or the caller stored a key for
//! the personal credential). The gateway transcodes every inbound wire to every
//! upstream wire, so an advertised model is reachable from every host; the
//! per-provider `providers` split, with each provider's `clients`, is what the
//! bridge narrows per host with.
//!
//! `default_model` is `gateway.default_model`, presented as `<id>[1m]` when
//! the model is advertised to Claude clients and its context window is at
//! least 1M tokens: the marker is derived here, never written in config.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use systemprompt_models::bridge::profile::{
    AdvertisedLimits, BridgeProfileResponse, ProviderHealth,
};
use systemprompt_models::providers::{ClientFamily, LONG_CONTEXT_WINDOW, with_context_variant};

use crate::services::catalog::{
    ModelRequest, ResolvedCatalog, ResolvedCredential, ResolvedModel, ResolvedProvider,
};

/// The caller's credential facts: which platform secrets the deployment
/// holds, and which personal credentials this user stored a key for.
#[derive(Clone, Copy)]
pub struct CredentialFacts<'a> {
    pub secret_present: &'a dyn Fn(&str) -> bool,
    pub personal_keys: &'a BTreeSet<String>,
}

impl std::fmt::Debug for CredentialFacts<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CredentialFacts")
            .field("personal_keys", &self.personal_keys)
            .finish_non_exhaustive()
    }
}

impl CredentialFacts<'_> {
    #[must_use]
    pub fn configured(&self, credential: &ResolvedCredential) -> bool {
        if credential.is_personal() {
            return self.personal_keys.contains(credential.id.as_str());
        }
        credential
            .platform_secret()
            .is_some_and(|secret| (self.secret_present)(secret.as_str()))
    }
}

#[must_use]
pub fn is_model_servable(
    catalog: &ResolvedCatalog,
    model: &str,
    facts: &CredentialFacts<'_>,
) -> bool {
    catalog
        .resolve(&ModelRequest::new(model))
        .is_ok_and(|target| {
            target
                .chain
                .iter()
                .any(|hop| facts.configured(&hop.credential))
        })
}

fn provider_models<'a>(
    catalog: &'a ResolvedCatalog,
    provider: &'a ResolvedProvider,
) -> impl Iterator<Item = &'a ResolvedModel> {
    catalog.advertised_models(None).filter(move |model| {
        catalog
            .enabled_hops(model.id.as_str())
            .any(|hop| hop.provider == provider.id)
    })
}

#[must_use]
pub fn provider_health(
    catalog: &ResolvedCatalog,
    facts: &CredentialFacts<'_>,
) -> Vec<ProviderHealth> {
    catalog
        .providers()
        .filter(|provider| provider.enabled)
        .filter_map(|provider| {
            let models: Vec<&ResolvedModel> = provider_models(catalog, provider).collect();
            if models.is_empty() {
                return None;
            }
            let credential = catalog.credential(provider.credential.as_str())?;
            let configured = facts.configured(credential);
            let config_issue = (!configured).then(|| {
                credential.platform_secret().map_or_else(
                    || "personal key not added — add your own key in Settings".to_owned(),
                    |secret| format!("API key secret '{}' is not configured", secret.as_str()),
                )
            });
            let clients: BTreeSet<ClientFamily> = models
                .iter()
                .flat_map(|m| m.clients.iter().copied())
                .collect();
            Some(ProviderHealth {
                name: provider.id.as_str().to_owned(),
                credential: credential.id.as_str().to_owned(),
                clients: clients.into_iter().collect(),
                configured,
                models: models.iter().map(|m| m.id.as_str().to_owned()).collect(),
                config_issue,
                display_name: provider
                    .display_name
                    .clone()
                    .or_else(|| credential.label.clone()),
                personal_key: credential.is_personal(),
            })
        })
        .collect()
}

#[must_use]
pub fn presented_default_model(catalog: &ResolvedCatalog) -> Option<String> {
    let id = catalog.default_model()?;
    let model = catalog.model(id.as_str())?;
    let long_context = model.limits.context_window >= LONG_CONTEXT_WINDOW
        && model.is_advertised_to(ClientFamily::Claude);
    Some(if long_context {
        with_context_variant(id.as_str())
    } else {
        id.as_str().to_owned()
    })
}

#[derive(Debug, Clone)]
pub struct BridgeProfileParams<'a> {
    pub inference_gateway_base_url: String,
    pub auth_scheme: String,
    pub organization_uuid: Option<String>,
    pub catalog: &'a ResolvedCatalog,
}

#[must_use]
pub fn build(
    params: BridgeProfileParams<'_>,
    facts: &CredentialFacts<'_>,
) -> BridgeProfileResponse {
    let BridgeProfileParams {
        inference_gateway_base_url,
        auth_scheme,
        organization_uuid,
        catalog,
    } = params;
    let servable: Vec<&ResolvedModel> = catalog
        .advertised_models(None)
        .filter(|model| is_model_servable(catalog, model.id.as_str(), facts))
        .collect();
    let model_limits: BTreeMap<String, AdvertisedLimits> = servable
        .iter()
        .filter(|model| model.limits.context_window > 0)
        .map(|model| {
            (
                model.id.as_str().to_owned(),
                AdvertisedLimits {
                    context_window: model.limits.context_window,
                    max_output_tokens: model.limits.max_output_tokens,
                },
            )
        })
        .collect();
    let model_names: BTreeMap<String, String> = servable
        .iter()
        .filter_map(|model| {
            model
                .display_name
                .as_ref()
                .map(|name| (model.id.as_str().to_owned(), name.clone()))
        })
        .collect();
    BridgeProfileResponse {
        inference_gateway_base_url,
        auth_scheme,
        models: servable.iter().map(|m| m.id.as_str().to_owned()).collect(),
        default_model: presented_default_model(catalog),
        organization_uuid,
        model_limits,
        model_names,
        providers: provider_health(catalog, facts),
    }
}
