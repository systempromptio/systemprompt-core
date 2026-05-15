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

    #[serde(default = "default_mcp_resilience")]
    pub resilience: ResilienceSettings,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            auto_discover: false,
            resilience: default_mcp_resilience(),
        }
    }
}

/// MCP defaults: tool RPCs are bounded at 30s rather than the 60s AI default.
fn default_mcp_resilience() -> ResilienceSettings {
    ResilienceSettings {
        request_timeout_ms: 30_000,
        connect_timeout_ms: 5_000,
        ..ResilienceSettings::default()
    }
}

/// Per-dependency resilience policy: timeouts, retry, circuit breaker,
/// bulkhead.
///
/// Plain serde data loaded from profile config (all values in milliseconds or
/// counts). Domain crates translate this into the runtime form consumed by
/// `systemprompt-resilience`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ResilienceSettings {
    /// Timeout for a single (non-streaming) attempt.
    #[serde(default = "default_request_timeout")]
    pub request_timeout_ms: u64,

    /// Connection-establishment timeout.
    #[serde(default = "default_resilience_connect_timeout")]
    pub connect_timeout_ms: u64,

    /// Maximum gap between two chunks of a streaming response.
    #[serde(default = "default_stream_idle_timeout")]
    pub stream_idle_timeout_ms: u64,

    /// Maximum attempts including the first. `1` disables retries.
    #[serde(default = "default_retry_attempts")]
    pub retry_attempts: u32,

    /// Backoff before the first retry; doubles each subsequent attempt.
    #[serde(default = "default_retry_base_delay")]
    pub retry_base_delay_ms: u64,

    /// Upper bound on a single backoff delay.
    #[serde(default = "default_retry_max_delay")]
    pub retry_max_delay_ms: u64,

    /// Consecutive failures that trip the circuit breaker open.
    #[serde(default = "default_breaker_threshold")]
    pub breaker_failure_threshold: u32,

    /// How long the breaker stays open before allowing a half-open probe.
    #[serde(default = "default_breaker_cooldown")]
    pub breaker_open_cooldown_ms: u64,

    /// Concurrent probes admitted while the breaker is half-open.
    #[serde(default = "default_half_open_probes")]
    pub breaker_half_open_probes: u32,

    /// Maximum in-flight calls to the dependency; further calls fast-fail.
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
}

impl Default for ResilienceSettings {
    fn default() -> Self {
        Self {
            request_timeout_ms: default_request_timeout(),
            connect_timeout_ms: default_resilience_connect_timeout(),
            stream_idle_timeout_ms: default_stream_idle_timeout(),
            retry_attempts: default_retry_attempts(),
            retry_base_delay_ms: default_retry_base_delay(),
            retry_max_delay_ms: default_retry_max_delay(),
            breaker_failure_threshold: default_breaker_threshold(),
            breaker_open_cooldown_ms: default_breaker_cooldown(),
            breaker_half_open_probes: default_half_open_probes(),
            max_concurrent: default_max_concurrent(),
        }
    }
}

const fn default_request_timeout() -> u64 {
    60_000
}

const fn default_resilience_connect_timeout() -> u64 {
    10_000
}

const fn default_stream_idle_timeout() -> u64 {
    60_000
}

const fn default_retry_attempts() -> u32 {
    3
}

const fn default_retry_base_delay() -> u64 {
    200
}

const fn default_retry_max_delay() -> u64 {
    10_000
}

const fn default_breaker_threshold() -> u32 {
    5
}

const fn default_breaker_cooldown() -> u64 {
    30_000
}

const fn default_half_open_probes() -> u32 {
    1
}

const fn default_max_concurrent() -> usize {
    16
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
    pub default_image_model: String,

    #[serde(default)]
    pub default_image_resolution: String,

    #[serde(default)]
    pub google_search_enabled: bool,

    #[serde(default)]
    pub models: HashMap<String, ModelDefinition>,

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
