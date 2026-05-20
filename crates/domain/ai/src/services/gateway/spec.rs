//! Declarative gateway-policy specification.
//!
//! A [`GatewayPolicySpec`] is the `spec` payload of a row in
//! `ai_gateway_policies`. It is the inference-model allow-list (a security
//! control) plus per-call token ceilings, quota windows, and safety-scanner
//! configuration. The same type is parsed from the version-controlled
//! `services/ai/gateway-policies.yaml` and from the JSONB `spec` column, so
//! the on-disk config and the persisted row share one schema.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct QuotaWindow {
    pub window_seconds: i32,
    pub max_requests: Option<i64>,
    pub max_input_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct SafetyConfig {
    #[serde(default)]
    pub scanners: Vec<String>,
    #[serde(default)]
    pub block_categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct GatewayPolicySpec {
    #[serde(default)]
    pub allowed_models: Option<Vec<String>>,
    #[serde(default)]
    pub max_input_tokens_per_call: Option<u32>,
    #[serde(default)]
    pub max_tool_depth: Option<u32>,
    #[serde(default)]
    pub quota_windows: Vec<QuotaWindow>,
    #[serde(default)]
    pub safety: SafetyConfig,
}

impl GatewayPolicySpec {
    #[must_use]
    pub fn permissive() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn model_allowed(&self, model: &str) -> bool {
        self.allowed_models
            .as_deref()
            .is_none_or(|list| list.iter().any(|m| m == model))
    }
}
