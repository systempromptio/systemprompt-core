//! Per-model descriptors.
//!
//! [`ModelDefinition`] and its [`ModelCapabilities`], `ModelLimits`, and
//! [`ModelPricing`] are the per-model descriptors shared with the services
//! catalog (`services/ai/models.yaml`). Connectivity is never modelled here —
//! it lives in the catalog's providers and endpoints.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use systemprompt_wire::ModelLimits;

#[expect(
    clippy::struct_excessive_bools,
    reason = "model capability matrix: each bool is an independent provider feature flag, not \
              state"
)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
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

    #[serde(default)]
    pub reasoning: bool,

    #[serde(default = "default_true")]
    pub prompt_caching: bool,
}

const fn default_true() -> bool {
    true
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self {
            vision: false,
            audio_input: false,
            video_input: false,
            image_generation: false,
            audio_generation: false,
            streaming: false,
            tools: false,
            structured_output: false,
            system_prompts: false,
            image_resolution_config: false,
            reasoning: false,
            prompt_caching: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModelPricing {
    #[serde(default)]
    pub input_per_million: f64,

    #[serde(default)]
    pub output_per_million: f64,

    #[serde(default)]
    pub cache_read_per_million: Option<f64>,

    #[serde(default)]
    pub cache_write_per_million: Option<f64>,

    #[serde(default)]
    pub per_image_cents: Option<f64>,
}

/// Contractual/data-governance posture of a provider or model.
///
/// Used by gateway routes that require residency (`european`) or a
/// no-train/no-retain clause (`no_retain`) before dispatching classified
/// traffic. A model declares its own posture only to override the provider's;
/// a model absent from the registry inherits the provider's, so adding a model
/// never silently weakens what its provider already guarantees.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
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

    #[must_use]
    pub const fn declares_cache_rate(&self) -> bool {
        self.cache_read_per_million.is_some()
    }

    #[must_use]
    pub fn cache_read_rate(&self) -> f64 {
        self.cache_read_per_million.unwrap_or(0.0)
    }

    #[must_use]
    pub fn cache_write_rate(&self) -> f64 {
        self.cache_write_per_million.unwrap_or(0.0)
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
