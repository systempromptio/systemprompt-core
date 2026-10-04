//! Model view a host receives from gateway provider health: the models it can
//! use given the API surfaces it speaks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::providers::ApiSurface;

use crate::gateway::types::ProviderHealth;

/// Desktop accepts only advertised million-token Claude context variants.
#[must_use]
pub fn with_context_variants(
    models: &[String],
    limits: &BTreeMap<String, systemprompt_models::bridge::profile::AdvertisedLimits>,
) -> Vec<String> {
    let mut out = Vec::with_capacity(models.len());
    for id in models {
        let base = id.strip_suffix("[1m]").unwrap_or(id);
        if !base.starts_with("claude-") || base.starts_with("claude-fable-") {
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
