//! Default provider-registry and route generation for the setup wizard.
//!
//! Emits a `profile.providers` [`ProviderEntry`] plus a matching
//! [`GatewayRoute`] for each AI key actually supplied, so the generated
//! profile resolves and passes both [`ProviderRegistry::validate`] and
//! `GatewayConfig::validate` (every route provider must exist in the
//! registry). Operators reshape the result — adding custom providers like
//! `minimax` — by editing `profile.providers` directly.

use std::collections::HashMap;

use systemprompt_identifiers::{ModelId, ProviderId, RouteId, SecretName};
use systemprompt_models::profile::{
    GatewayRoute, ProviderEntry, ProviderModel, ProviderRegistry, WireProtocol,
};
use systemprompt_models::services::ai::{ModelCapabilities, ModelLimits, ModelPricing};

use super::secrets::SecretsData;

struct ProviderDefault {
    name: &'static str,
    protocol: WireProtocol,
    endpoint: &'static str,
    secret: &'static str,
    route_pattern: &'static str,
    model: &'static str,
    present: fn(&SecretsData) -> bool,
}

const PROVIDER_DEFAULTS: &[ProviderDefault] = &[
    ProviderDefault {
        name: "anthropic",
        protocol: WireProtocol::Anthropic,
        endpoint: "https://api.anthropic.com/v1",
        secret: "anthropic",
        route_pattern: "claude-*",
        model: "claude-sonnet-4-20250514",
        present: |s| s.anthropic.is_some(),
    },
    ProviderDefault {
        name: "openai",
        protocol: WireProtocol::OpenAiChat,
        endpoint: "https://api.openai.com/v1",
        secret: "openai",
        route_pattern: "gpt-*",
        model: "gpt-4-turbo",
        present: |s| s.openai.is_some(),
    },
    ProviderDefault {
        name: "gemini",
        protocol: WireProtocol::Gemini,
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

pub(super) fn build_registry(secrets: &SecretsData) -> ProviderRegistry {
    ProviderRegistry {
        providers: present_defaults(secrets)
            .iter()
            .map(|d| ProviderEntry {
                name: ProviderId::new(d.name),
                protocol: d.protocol,
                endpoint: d.endpoint.to_owned(),
                api_key_secret: SecretName::new(d.secret),
                extra_headers: HashMap::new(),
                models: vec![ProviderModel {
                    id: ModelId::new(d.model),
                    aliases: Vec::new(),
                    upstream_model: None,
                    pricing: ModelPricing::default(),
                    capabilities: ModelCapabilities::default(),
                    limits: ModelLimits::default(),
                }],
            })
            .collect(),
    }
}
