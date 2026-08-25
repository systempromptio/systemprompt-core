//! Per-provider AI policy and per-model descriptors.
//!
//! [`AiProviderConfig`] is the deployment policy layered on a registry provider
//! (enable flag, default-model overrides, resilience). [`ModelDefinition`] and
//! its [`ModelCapabilities`], [`ModelLimits`], and [`ModelPricing`] are the
//! per-model descriptors shared with `profile.providers`. Connectivity itself
//! is never modelled here — it lives in the provider registry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use super::config::ResilienceSettings;

const fn default_true() -> bool {
    true
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "model capability matrix: each bool is an independent provider feature flag, not \
              state"
)]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModelCapabilities {
    #[serde(default)]
    pub vision: bool,

    #[serde(default)]
    pub audio_input: bool,

    #[serde(default)]
    pub video_input: bool,

    #[serde(default)]
    pub image_generation: bool,

    #[serde(default)]
    pub audio_generation: bool,

    #[serde(default)]
    pub streaming: bool,

    #[serde(default)]
    pub tools: bool,

    #[serde(default)]
    pub structured_output: bool,

    #[serde(default)]
    pub system_prompts: bool,

    #[serde(default)]
    pub image_resolution_config: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModelLimits {
    #[serde(default)]
    pub context_window: u32,

    #[serde(default)]
    pub max_output_tokens: u32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_thinking_budget: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModelPricing {
    #[serde(default)]
    pub input_per_million: f64,

    #[serde(default)]
    pub output_per_million: f64,

    #[serde(default)]
    pub cache_read_per_million: f64,

    #[serde(default)]
    pub cache_write_per_million: f64,

    #[serde(default)]
    pub per_image_cents: Option<f64>,
}

/// Contractual/data-governance posture of a provider or model.
///
/// Used by gateway routes that require residency (`european`) or a
/// no-train/no-retain clause (`no_retain`) before dispatching classified
/// traffic.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModelGovernance {
    #[serde(default)]
    pub european: bool,

    #[serde(default)]
    pub no_retain: bool,
}

impl ModelPricing {
    #[must_use]
    pub fn is_billable(&self) -> bool {
        if self.per_image_cents.is_some_and(|c| c > 0.0) {
            return true;
        }
        self.input_per_million > 0.0 && self.output_per_million > 0.0
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ModelDefinition {
    #[serde(default)]
    pub capabilities: ModelCapabilities,

    #[serde(default)]
    pub limits: ModelLimits,

    #[serde(default)]
    pub pricing: ModelPricing,
}

/// Per-provider AI *policy*, keyed by registry provider name.
///
/// Connectivity (endpoint, credential, model catalog) lives in the profile
/// `providers` registry; this struct carries only the policy a deployment
/// layers on top of an entry: whether the provider is enabled, its agent-side
/// default-model override, image defaults, web-search toggle, and resilience.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default)]
    pub default_model: String,

    #[serde(default)]
    pub default_image_model: String,

    #[serde(default)]
    pub google_search_enabled: bool,

    #[serde(default)]
    pub resilience: ResilienceSettings,
}

impl Default for AiProviderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_model: String::new(),
            default_image_model: String::new(),
            google_search_enabled: false,
            resilience: ResilienceSettings::default(),
        }
    }
}
