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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotaWindow {
    pub window_seconds: i32,
    /// Which subject the bucket is keyed by: `user` (the default), or any
    /// subject-attribute dimension registered by an extension (for example
    /// `organization`). A window whose subject cannot be resolved for the
    /// requesting user is skipped.
    #[serde(default = "default_subject")]
    pub subject: String,
    pub max_requests: Option<i64>,
    pub max_input_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    /// Spend ceiling for the window. Enforced one request late: cost is known
    /// only after the response, so the request that crosses the ceiling
    /// completes and subsequent requests are denied.
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

/// Phrase-list tuning for the builtin `heuristic` scanner. Ignored when an
/// extension registers its own scanner under the name `heuristic`, which
/// shadows the builtin wholesale.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct HeuristicConfig {
    /// Replaces the builtin phrase list entirely when set.
    #[serde(default)]
    pub phrases: Option<Vec<String>>,
    /// Appended to whichever base list is in effect.
    #[serde(default)]
    pub extra_phrases: Vec<String>,
    /// Drops the builtin list, leaving only `phrases`/`extra_phrases`.
    #[serde(default)]
    pub disable_builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct SafetyConfig {
    #[serde(default)]
    pub scanners: Vec<String>,
    #[serde(default)]
    pub heuristic: HeuristicConfig,
    #[serde(default)]
    pub block_categories: Vec<String>,
    /// Response-phase categories that deny the reply instead of merely
    /// recording it. Buffered replies only: a streamed reply is already on the
    /// wire by the time the scan can run, so listing a category here has no
    /// effect on streaming and the gateway does not pretend otherwise.
    #[serde(default)]
    pub block_response_categories: Vec<String>,
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
