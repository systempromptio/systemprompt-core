//! Default provider-registry and route generation for the setup wizard.
//!
//! The model catalog (providers, models, pricing, capabilities) comes from the
//! embedded canonical seed [`ProviderRegistry::default_seed`]; this module only
//! selects the providers whose AI key was actually supplied and emits a
//! matching [`GatewayRoute`] per provider, so the generated profile resolves
//! and passes both [`ProviderRegistry::validate`] and `GatewayConfig::validate`
//! (every route provider must exist in the registry). Operators reshape the
//! result — adding custom providers like `minimax` — by editing
//! `profile.providers` directly.

use std::collections::HashMap;

use systemprompt_identifiers::{ProviderId, RouteId};
use systemprompt_models::profile::{GatewayRoute, ProviderRegistry};

use super::secrets::SecretsData;

struct ProviderDefault {
    name: &'static str,
    route_pattern: &'static str,
    /// Codex sends `openai` aliases (`gpt-5.4-mini`) the real API rejects; the
    /// `openai` default rewrites them to a concrete model rather than passing
    /// through.
    default_upstream: Option<&'static str>,
    present: fn(&SecretsData) -> bool,
}

const PROVIDER_DEFAULTS: &[ProviderDefault] = &[
    ProviderDefault {
        name: "anthropic",
        route_pattern: "claude-*",
        default_upstream: None,
        present: |s| s.anthropic.is_some(),
    },
    ProviderDefault {
        name: "openai",
        route_pattern: "gpt-*",
        default_upstream: Some("gpt-5-mini"),
        present: |s| s.openai.is_some(),
    },
    ProviderDefault {
        name: "gemini",
        route_pattern: "gemini-*",
        default_upstream: None,
        present: |s| s.gemini.is_some(),
    },
];

fn present_defaults(secrets: &SecretsData) -> Vec<&'static ProviderDefault> {
    PROVIDER_DEFAULTS
        .iter()
        .filter(|p| (p.present)(secrets))
        .collect()
}

pub fn build_routes(secrets: &SecretsData) -> Vec<GatewayRoute> {
    present_defaults(secrets)
        .iter()
        .map(|d| {
            let mut route = GatewayRoute {
                id: RouteId::new(""),
                model_pattern: d.route_pattern.to_owned(),
                provider: ProviderId::new(d.name),
                upstream_model: d.default_upstream.map(str::to_owned),
                extra_headers: HashMap::new(),
                pricing: None,
                when: None,
            };
            route.ensure_id();
            route
        })
        .collect()
}

pub fn build_registry(secrets: &SecretsData) -> ProviderRegistry {
    let seed = match ProviderRegistry::default_seed() {
        Ok(seed) => seed,
        Err(e) => {
            tracing::error!(
                error = %e,
                "embedded default provider catalog failed to parse; seeding an empty provider \
                 registry"
            );
            return ProviderRegistry::default();
        },
    };
    ProviderRegistry {
        providers: present_defaults(secrets)
            .iter()
            .filter_map(|d| seed.find_provider(d.name).cloned())
            .collect(),
    }
}
