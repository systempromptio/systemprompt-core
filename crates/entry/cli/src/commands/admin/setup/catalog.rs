//! Default AI-provider catalog and route generation for the setup wizard.
//!
//! Emits a `GatewayProvider` / `GatewayModel` / `GatewayRoute` triple only for
//! the AI keys actually supplied, so the generated gateway resolves and passes
//! `GatewayConfig::validate` (every catalog model must be reachable by a route,
//! and every route provider must exist in the catalog). Operators reshape the
//! result — adding custom providers like `minimax` — via `admin config
//! catalog`.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use systemprompt_identifiers::{ModelId, ProviderId, RouteId, SecretName};
use systemprompt_models::profile::{GatewayCatalog, GatewayModel, GatewayProvider, GatewayRoute};

use super::secrets::SecretsData;

struct ProviderDefault {
    name: &'static str,
    endpoint: &'static str,
    secret: &'static str,
    route_pattern: &'static str,
    model: &'static str,
    present: fn(&SecretsData) -> bool,
}

const PROVIDER_DEFAULTS: &[ProviderDefault] = &[
    ProviderDefault {
        name: "anthropic",
        endpoint: "https://api.anthropic.com/v1",
        secret: "anthropic",
        route_pattern: "claude-*",
        model: "claude-sonnet-4-20250514",
        present: |s| s.anthropic.is_some(),
    },
    ProviderDefault {
        name: "openai",
        endpoint: "https://api.openai.com/v1",
        secret: "openai",
        route_pattern: "gpt-*",
        model: "gpt-4-turbo",
        present: |s| s.openai.is_some(),
    },
    ProviderDefault {
        name: "gemini",
        endpoint: "https://generativelanguage.googleapis.com/v1beta",
        secret: "gemini",
        route_pattern: "gemini-*",
        model: "gemini-2.5-flash",
        present: |s| s.gemini.is_some(),
    },
];

fn present_defaults(secrets: &SecretsData) -> Vec<&'static ProviderDefault> {
    PROVIDER_DEFAULTS
        .iter()
        .filter(|p| (p.present)(secrets))
        .collect()
}

pub(super) fn build_routes(secrets: &SecretsData) -> Vec<GatewayRoute> {
    present_defaults(secrets)
        .iter()
        .map(|d| {
            let mut route = GatewayRoute {
                id: RouteId::new(""),
                model_pattern: d.route_pattern.to_owned(),
                provider: ProviderId::new(d.name),
                upstream_model: None,
                extra_headers: HashMap::new(),
                pricing: None,
            };
            route.ensure_id();
            route
        })
        .collect()
}

pub(super) fn build_catalog(secrets: &SecretsData) -> GatewayCatalog {
    let defaults = present_defaults(secrets);
    GatewayCatalog {
        providers: defaults
            .iter()
            .map(|d| GatewayProvider {
                name: ProviderId::new(d.name),
                endpoint: d.endpoint.to_owned(),
                api_key_secret: SecretName::new(d.secret),
                extra_headers: HashMap::new(),
            })
            .collect(),
        models: defaults
            .iter()
            .map(|d| GatewayModel {
                id: ModelId::new(d.model),
                provider: ProviderId::new(d.name),
                aliases: Vec::new(),
                display_name: None,
                upstream_model: None,
                pricing: None,
            })
            .collect(),
    }
}

pub(super) fn save_catalog(catalog: &GatewayCatalog, path: &Path) -> Result<()> {
    let yaml = serde_yaml::to_string(catalog).context("Failed to serialize gateway catalog")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    std::fs::write(path, yaml).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}
