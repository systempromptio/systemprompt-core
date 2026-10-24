//! The pre-0.73 catalog shape, privately: the one parser of it left in the
//! code base.
//!
//! These structs mirror what 0.72.1 read from `services/ai/providers.yaml`
//! (a `providers:` list of entries), `gateway.yaml` (routes keyed by
//! `model_pattern`/`provider`, `default_provider`) and `ai/config.yaml`
//! (`ai.default_provider`, `ai.providers`). Nothing outside `config migrate`
//! may name them; the loader refuses the shape outright.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Deserialize;
use systemprompt_identifiers::SecretName;
use systemprompt_manifest::services::gateway::{
    ModelGlobs, RouteMatch, RouteRequirements, SelectionStrategy,
};
use systemprompt_manifest::services::{
    BridgeReleasesSpec, ModelCapabilities, ModelGovernance, ModelPricing, QuotaFaultMode,
    ResilienceSettings, SystemPromptRule,
};
use systemprompt_wire::anthropic::AnthropicBeta;
use systemprompt_wire::{ModelLimits, WireProtocol};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyModel {
    pub id: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub upstream_model: Option<String>,
    #[serde(default)]
    pub pricing: ModelPricing,
    #[serde(default)]
    pub capabilities: ModelCapabilities,
    #[serde(default)]
    pub limits: ModelLimits,
    #[serde(default)]
    pub governance: Option<ModelGovernance>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyCredentialSource {
    #[default]
    Platform,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LegacySurface {
    Anthropic,
    #[serde(rename = "openai")]
    OpenAi,
    Gemini,
    Backend,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyProvider {
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub wire: WireProtocol,
    pub surface: LegacySurface,
    pub endpoint: String,
    #[serde(default)]
    pub api_key_secret: Option<SecretName>,
    #[serde(default)]
    pub credential: LegacyCredentialSource,
    #[serde(default)]
    pub credential_from: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub forward_headers: Vec<String>,
    #[serde(default)]
    pub extra_headers: HashMap<String, String>,
    #[serde(default)]
    pub accepted_betas: Option<BTreeSet<AnthropicBeta>>,
    #[serde(default)]
    pub models: Vec<LegacyModel>,
    #[serde(default)]
    pub governance: ModelGovernance,
}

impl LegacyProvider {
    pub fn is_personal(&self) -> bool {
        self.credential == LegacyCredentialSource::User
    }

    pub fn credential_owner(&self) -> &str {
        self.credential_from.as_deref().unwrap_or(&self.name)
    }

    pub fn find_model(&self, requested: &str) -> Option<&LegacyModel> {
        let bare = systemprompt_models::providers::without_context_variant(requested);
        self.models
            .iter()
            .find(|m| m.id == bare || m.aliases.iter().any(|a| a == bare))
    }

    pub fn find_served_model(&self, requested: &str) -> Option<&LegacyModel> {
        let bare = systemprompt_models::providers::without_context_variant(requested);
        self.find_model(requested).or_else(|| {
            self.models
                .iter()
                .find(|m| m.upstream_model.as_deref() == Some(bare))
        })
    }

    pub fn upstream_model_for(&self, route_override: Option<&str>, requested: &str) -> String {
        let name = route_override.unwrap_or(requested);
        let bare = systemprompt_models::providers::without_context_variant(name);
        self.find_model(name)
            .and_then(|m| m.upstream_model.clone())
            .unwrap_or_else(|| bare.to_owned())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyDeployment {
    pub provider: String,
    #[serde(default)]
    pub upstream_model: Option<String>,
    #[serde(default)]
    pub weight: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyScopeChain {
    pub provider: String,
    #[serde(default)]
    pub upstream_model: Option<String>,
    #[serde(default)]
    pub fallbacks: Vec<LegacyDeployment>,
    #[serde(default)]
    pub strategy: SelectionStrategy,
    #[serde(default)]
    pub weight: Option<u32>,
    #[serde(default)]
    pub context_fallbacks: Vec<LegacyDeployment>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LegacyUnmapped {
    #[default]
    Deny,
    Shared,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyScopeChains {
    pub dimension: String,
    #[serde(default)]
    pub chains: BTreeMap<String, LegacyScopeChain>,
    #[serde(default)]
    pub unmapped: LegacyUnmapped,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyRoute {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub model_pattern: String,
    pub provider: String,
    #[serde(default)]
    pub upstream_model: Option<String>,
    #[serde(default)]
    pub extra_headers: HashMap<String, String>,
    #[serde(default)]
    pub pricing: Option<ModelPricing>,
    #[serde(default)]
    pub when: Option<RouteMatch>,
    #[serde(default)]
    pub requires: Option<RouteRequirements>,
    #[serde(default)]
    pub fallbacks: Vec<LegacyDeployment>,
    #[serde(default)]
    pub by_scope: Option<LegacyScopeChains>,
    #[serde(default)]
    pub strategy: SelectionStrategy,
    #[serde(default)]
    pub weight: Option<u32>,
    #[serde(default)]
    pub context_fallbacks: Vec<LegacyDeployment>,
}

impl LegacyRoute {
    pub fn effective_id(&self) -> String {
        self.id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map_or_else(
                || {
                    systemprompt_manifest::services::gateway::synthesize_route_id(
                        &self.model_pattern,
                        &self.provider,
                    )
                    .as_str()
                    .to_owned()
                },
                str::to_owned,
            )
    }

    pub fn matches(&self, model: &str) -> bool {
        ModelGlobs(vec![self.model_pattern.clone()]).matches(
            systemprompt_models::providers::without_context_variant(model),
        )
    }

    pub fn is_pure_routing(&self) -> bool {
        self.when.is_none()
            && self.requires.is_none()
            && self.by_scope.is_none()
            && self.strategy.is_ordered()
            && self.context_fallbacks.is_empty()
            && self.extra_headers.is_empty()
    }
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyGateway {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub routes: Vec<LegacyRoute>,
    #[serde(default)]
    pub default_provider: Option<String>,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default)]
    pub allow_unlisted_models: bool,
    #[serde(default = "default_true")]
    pub hidden_requires_route: bool,
    #[serde(default)]
    pub catalog_route_id: Option<String>,
    #[serde(default)]
    pub quota_fault_mode: QuotaFaultMode,
    #[serde(default)]
    pub require_scopes: Vec<systemprompt_identifiers::ScopeDimension>,
    #[serde(default = "default_true")]
    pub automatic_prompt_caching: bool,
    #[serde(default)]
    pub auth_scheme: Option<String>,
    #[serde(default)]
    pub inference_path_prefix: Option<String>,
    #[serde(default)]
    pub system_prompt_overrides: Vec<SystemPromptRule>,
    #[serde(default)]
    pub bridge_releases: Option<BridgeReleasesSpec>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LegacyAiProvider {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub default_model: String,
    #[serde(default)]
    pub default_image_model: String,
    #[serde(default)]
    pub google_search_enabled: bool,
    #[serde(default)]
    pub resilience: Option<ResilienceSettings>,
}

/// The legacy keys of `ai:`; every other key is kept verbatim.
#[derive(Debug, Clone, Default)]
pub struct LegacyAi {
    pub default_provider: Option<String>,
    pub providers: BTreeMap<String, LegacyAiProvider>,
}
