use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::config::ResilienceSettings;

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolModelSettings {
    pub model: String,

    #[serde(default)]
    pub max_output_tokens: Option<u32>,
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "model capability matrix: each bool is an independent provider feature flag, not state"
)]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ModelLimits {
    #[serde(default)]
    pub context_window: u32,

    #[serde(default)]
    pub max_output_tokens: u32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModelPricing {
    #[serde(default)]
    pub input_per_million: f64,

    #[serde(default)]
    pub output_per_million: f64,

    #[serde(default)]
    pub per_image_cents: Option<f64>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default)]
    pub api_key: String,

    #[serde(default)]
    pub endpoint: Option<String>,

    #[serde(default)]
    pub default_model: String,

    #[serde(default)]
    pub default_image_model: String,

    #[serde(default)]
    pub default_image_resolution: String,

    #[serde(default)]
    pub google_search_enabled: bool,

    #[serde(default)]
    pub models: HashMap<String, ModelDefinition>,

    /// Resilience policy applied to outbound AI provider calls (timeouts,
    /// retry, circuit breaker, bulkhead).
    #[serde(default)]
    pub resilience: ResilienceSettings,
}

impl Default for AiProviderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            api_key: String::new(),
            endpoint: None,
            default_model: String::new(),
            default_image_model: String::new(),
            default_image_resolution: String::new(),
            google_search_enabled: false,
            models: HashMap::new(),
            resilience: ResilienceSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolModelConfig {
    pub provider: String,
    pub model: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<String>,
}
