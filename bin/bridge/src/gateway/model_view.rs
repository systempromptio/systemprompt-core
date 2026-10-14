//! Model view a host receives from gateway provider health: the models it can
//! use given the API surfaces it speaks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::providers::ApiSurface;

use crate::gateway::types::ProviderHealth;

// Why: Claude Desktop breaks when `inferenceModels` names a non-Anthropic
// family, and Fable ids are not served to it; this predicate is the one
// definition of "a model Desktop may list", shared by every Desktop writer
// and the Status card's Desktop subset.
#[must_use]
pub fn is_desktop_model(id: &str) -> bool {
    let lower = id.to_ascii_lowercase();
    lower.starts_with("claude-") && !lower.starts_with("claude-fable-")
}

/// The Claude-only subset of `models` Claude Desktop is given, deduplicated
/// in first-seen order.
#[must_use]
pub fn desktop_models(models: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in models.iter().filter(|id| is_desktop_model(id)) {
        if !out.contains(id) {
            out.push(id.clone());
        }
    }
    out
}

/// One provider of the catalog as the Status card groups it: its distinct
/// models, and why it serves none when it is not configured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogGroup {
    pub name: String,
    pub surface: ApiSurface,
    pub configured: bool,
    pub models: Vec<String>,
    pub config_issue: Option<String>,
}

/// Every advertised provider in `providers`, each with its distinct models.
/// A model two providers serve is listed under the first.
#[must_use]
pub fn catalog_models(providers: &[ProviderHealth]) -> Vec<CatalogGroup> {
    let mut seen = std::collections::HashSet::new();
    providers
        .iter()
        .filter(|p| p.surface.is_advertised())
        .map(|p| CatalogGroup {
            name: p.name.clone(),
            surface: p.surface,
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
        let base = id.strip_suffix("[1m]").unwrap_or(id);
        if !is_desktop_model(base) {
            continue;
        }
        let is_million = limits
            .get(base)
            .or_else(|| limits.get(id))
            .is_some_and(|limit| limit.context_window >= 1_000_000);
        if !is_million {
            continue;
        }
        let listed = format!("{base}[1m]");
        if !out.contains(&listed) {
            out.push(listed);
        }
    }
    out
}

/// `checked` is false when there was no provider health to evaluate
/// (distinguishes "nothing usable" from "not yet checked").
#[derive(Debug, Default, PartialEq, Eq)]
pub struct HostModelView {
    pub compatible_models: Vec<String>,
    pub checked: bool,
    pub available: bool,
    pub unconfigured_providers: Vec<String>,
}

#[must_use]
pub fn host_model_view(health: &[ProviderHealth], accepted: &[ApiSurface]) -> HostModelView {
    let mut seen = std::collections::HashSet::new();
    let mut view = HostModelView {
        checked: !health.is_empty(),
        ..HostModelView::default()
    };
    for provider in health {
        let speaks = accepted.is_empty() || accepted.contains(&provider.surface);
        if !speaks {
            continue;
        }
        if !provider.configured {
            view.unconfigured_providers.push(provider.name.clone());
        } else if !provider.models.is_empty() {
            view.available = true;
        }
        if !provider.configured {
            continue;
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
pub fn effective_surfaces(
    host_id: HostKind,
    default: &[ApiSurface],
    overrides: &BTreeMap<String, Vec<String>>,
) -> Vec<ApiSurface> {
    overrides.get(host_id.as_str()).map_or_else(
        || default.to_vec(),
        |tags| {
            tags.iter()
                .filter_map(|t| ApiSurface::from_tag(t))
                .collect()
        },
    )
}

#[must_use]
pub fn has_surface_override(host_id: HostKind, overrides: &BTreeMap<String, Vec<String>>) -> bool {
    overrides.contains_key(host_id.as_str())
}
