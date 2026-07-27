//! Declarative gateway-policy specification.
//!
//! Spec payload of `ai_gateway_policies` rows, shared with the YAML schema in
//! `services/gateway/policies.yaml`. Carries quota windows and safety
//! configuration.
//!
//! Model exposure lives on the profile's gateway catalog, not here — see
//! `GatewayConfig::is_model_exposed`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct QuotaWindow {
    pub window_seconds: i32,
    pub max_requests: Option<i64>,
    pub max_input_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
}

/// How far back into a conversation the request-phase scanners look.
///
/// A request carries the whole conversation, so scanning all of it re-reads
/// every earlier turn on every turn: one finding would deny the rest of the
/// conversation, and each turn would persist the same finding again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SafetyHistoryMode {
    /// Judge only the newest user turn.
    #[default]
    Off,
    /// Also scan earlier turns, recording findings at phase `request_history`
    /// without denying the request.
    Audit,
    /// Scan earlier turns and let their findings deny the request.
    Block,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct SafetyConfig {
    #[serde(default)]
    pub scanners: Vec<String>,
    #[serde(default)]
    pub block_categories: Vec<String>,
    #[serde(default)]
    pub history: SafetyHistoryMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct GatewayPolicySpec {
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
}
