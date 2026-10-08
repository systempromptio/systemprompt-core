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

mod safety;

use serde::{Deserialize, Serialize};

pub use safety::{
    DEFAULT_SCANNER_TIMEOUT_MS, HeuristicConfig, SafetyConfig, SafetyHistoryMode, SafetyMode,
    ScannerFailMode, ScannerSettings,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotaWindow {
    pub window_seconds: i32,
    #[serde(default = "default_subject")]
    pub subject: String,
    pub max_requests: Option<i64>,
    pub max_input_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    #[serde(default)]
    pub max_cost_microdollars: Option<i64>,
}

impl Default for QuotaWindow {
    fn default() -> Self {
        Self {
            window_seconds: 0,
            subject: default_subject(),
            max_requests: None,
            max_input_tokens: None,
            max_output_tokens: None,
            max_cost_microdollars: None,
        }
    }
}

fn default_subject() -> String {
    "user".to_owned()
}

pub const USER_QUOTA_SUBJECT: &str = "user";
pub const API_KEY_QUOTA_SUBJECT: &str = "api_key";

/// Whether an exhausted quota window refuses the request or only records
/// that it would have.
///
/// The quota windows are the third enforcement plane on an inference request,
/// beside the governance chain and the safety scanners, and warn mode has to
/// cover it too or "nothing blocks" is not true. Under `warn` every window is
/// still reserved against and every ceiling still evaluated; a breach is
/// written to `governance_decisions` as a `warn` under policy `quota`, and
/// the request proceeds.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum QuotaMode {
    #[default]
    Enforce,
    Warn,
}

impl QuotaMode {
    #[must_use]
    pub const fn is_warn(self) -> bool {
        matches!(self, Self::Warn)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct GatewayPolicySpec {
    #[serde(default)]
    pub quota_mode: QuotaMode,
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
