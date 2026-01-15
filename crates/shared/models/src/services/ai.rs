use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiConfig {
    #[serde(default)]
    pub default_provider: String,

    #[serde(default)]
    pub default_max_output_tokens: Option<u32>,

    #[serde(default)]
    pub sampling: SamplingConfig,

    #[serde(default)]
    pub providers: HashMap<String, AiProviderConfig>,

    #[serde(default)]
    pub tool_models: HashMap<String, ToolModelSettings>,

    #[serde(default)]
    pub mcp: McpConfig,

    #[serde(default)]
    pub history: HistoryConfig,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct SamplingConfig {
    #[serde(default)]
    pub enable_smart_routing: bool,

    #[serde(default)]
    pub fallback_enabled: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default)]
    pub auto_discover: bool,

    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_ms: u64,

    #[serde(default = "default_execution_timeout")]
    pub execution_timeout_ms: u64,

    #[serde(default = "default_retry_attempts")]
    pub retry_attempts: u32,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            auto_discover: false,
            connect_timeout_ms: default_connect_timeout(),
            execution_timeout_ms: default_execution_timeout(),
            retry_attempts: default_retry_attempts(),
        }
    }
}

const fn default_connect_timeout() -> u64 {
    5000
}

const fn default_execution_timeout() -> u64 {
    30000
}

const fn default_retry_attempts() -> u32 {
    3
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HistoryConfig {
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,

    #[serde(default)]
    pub log_tool_executions: bool,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            retention_days: default_retention_days(),
            log_tool_executions: false,
        }
    }
}

const fn default_retention_days() -> u32 {
    30
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolModelSettings {
    pub model: String,

    #[serde(default)]
    pub max_output_tokens: Option<u32>,
}

#[allow(clippy::struct_excessive_bools)]
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
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ModelLimits {
    #[serde(default)]
    pub context_window: u32,

    #[serde(default)]
    pub max_output_tokens: u32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
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
    pub google_search_enabled: bool,

    #[serde(default)]
    pub models: HashMap<String, ModelDefinition>,
}

impl Default for AiProviderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            api_key: String::new(),
            endpoint: None,
            default_model: String::new(),
            google_search_enabled: false,
            models: HashMap::new(),
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
