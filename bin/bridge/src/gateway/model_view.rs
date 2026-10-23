//! Model view a host receives from gateway provider health: the models it
//! lists, given the client family it is.
//!
//! A model's `clients` is its whole visibility rule (the gateway resolves it
//! from the catalog); the bridge only maps each host to its family and keeps
//! the providers that advertise to it. `clients: []` is hidden everywhere.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::providers::{ClientFamily, LONG_CONTEXT_WINDOW, with_context_variant};

use crate::gateway::types::ProviderHealth;

#[must_use]
pub const fn host_client(host: HostKind) -> ClientFamily {
    match host {
        HostKind::ClaudeCode | HostKind::ClaudeDesktop => ClientFamily::Claude,
        HostKind::Codex
        | HostKind::CodexCli
        | HostKind::OpenCode
        | HostKind::OpenCodeCli
        | HostKind::Hermes => ClientFamily::OpenAi,
    }
}

/// The client families a host lists models for: the user's server-stored
/// filter when one is set for it, else the host's own family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostClients {
    pub families: Vec<ClientFamily>,
    pub filtered: bool,
}

impl HostClients {
    #[must_use]
    pub fn for_host(host: HostKind, server_filters: &BTreeMap<HostKind, Vec<String>>) -> Self {
        server_filters.get(&host).map_or_else(
            || Self {
                families: vec![host_client(host)],
                filtered: false,
            },
            |tags| Self {
                families: tags
                    .iter()
                    .filter_map(|t| ClientFamily::from_tag(t))
                    .collect(),
                filtered: true,
            },
        )
    }

    #[must_use]
    pub fn tags(&self) -> Vec<String> {
        self.families
            .iter()
            .map(|f| f.as_tag().to_owned())
            .collect()
    }
}

fn advertises_to(provider: &ProviderHealth, families: &[ClientFamily]) -> bool {
    provider.clients.iter().any(|c| families.contains(c))
}

/// One provider of the catalog as the Status card groups it: its distinct
/// models, the client families it advertises to, and why it serves none when
/// it is not configured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogGroup {
    pub name: String,
    pub clients: Vec<ClientFamily>,
    pub configured: bool,
    pub models: Vec<String>,
    pub config_issue: Option<String>,
}

#[must_use]
pub fn catalog_models(providers: &[ProviderHealth]) -> Vec<CatalogGroup> {
    let mut seen = std::collections::HashSet::new();
    providers
        .iter()
        .filter(|p| !p.clients.is_empty())
        .map(|p| CatalogGroup {
            name: p.name.clone(),
            clients: p.clients.clone(),
            configured: p.configured,
            models: p
                .models
                .iter()
                .filter(|m| seen.insert(m.as_str()))
                .cloned()
                .collect(),
            config_issue: p.config_issue.clone(),
        })
        .collect()
}

#[must_use]
pub fn with_context_variants(
    models: &[String],
    limits: &BTreeMap<String, systemprompt_models::bridge::profile::AdvertisedLimits>,
) -> Vec<String> {
    let mut out = Vec::with_capacity(models.len());
    for id in models {
        let base = systemprompt_models::providers::without_context_variant(id);
        let is_million = limits
            .get(base)
            .or_else(|| limits.get(id))
            .is_some_and(|limit| limit.context_window >= LONG_CONTEXT_WINDOW);
        if !is_million {
            continue;
        }
        let listed = with_context_variant(base);
        if !out.contains(&listed) {
            out.push(listed);
        }
    }
    out
}

/// What one host can list from the catalog.
///
/// `checked` is false when there was no provider health to evaluate
/// (distinguishes "nothing usable" from "not yet checked").
/// `missing_personal_keys` names each personal credential once, however many
/// endpoints it serves.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct HostModelView {
    pub compatible_models: Vec<String>,
    pub checked: bool,
    pub available: bool,
    pub unconfigured_providers: Vec<String>,
    pub missing_personal_keys: Vec<String>,
}

#[must_use]
pub fn host_model_view(health: &[ProviderHealth], families: &[ClientFamily]) -> HostModelView {
    let mut seen = std::collections::HashSet::new();
    let mut missing_credentials = std::collections::HashSet::new();
    let mut view = HostModelView {
        checked: !health.is_empty(),
        ..HostModelView::default()
    };
    for provider in health.iter().filter(|p| advertises_to(p, families)) {
        if !provider.configured {
            view.unconfigured_providers.push(provider.name.clone());
            if provider.personal_key && missing_credentials.insert(provider.credential.as_str()) {
                view.missing_personal_keys.push(
                    provider
                        .display_name
                        .clone()
                        .unwrap_or_else(|| provider.credential.clone()),
                );
            }
            continue;
        }
        if !provider.models.is_empty() {
            view.available = true;
        }
        for model in &provider.models {
            if seen.insert(model.clone()) {
                view.compatible_models.push(model.clone());
            }
        }
    }
    view
}

#[must_use]
pub fn claude_models(groups: &[CatalogGroup]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in groups
        .iter()
        .filter(|g| g.configured && g.clients.contains(&ClientFamily::Claude))
        .flat_map(|g| g.models.iter())
    {
        if !out.contains(id) {
            out.push(id.clone());
        }
    }
    out
}
