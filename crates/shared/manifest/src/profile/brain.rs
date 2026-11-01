//! Brain settings: the master switch, gateway recall, capture sources and
//! auto-confirmation, the extraction model and its budget, fact lifecycle
//! retention, and the outbox consumer's batch and cadence.
//!
//! `enabled: false` (the default) starts no consumer, wires no recall and
//! serves no brain routes. Booleans are YAML `true`/`false`; `on`/`off`
//! parse as strings and are refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ModelId, ProviderId};
use systemprompt_models::brain::ScopeLevel;
use systemprompt_models::origin::ClientKind;

use super::brain_defaults::{
    default_batch, default_budget_tokens, default_clients, default_correction_window_days,
    default_daily_token_budget, default_deadline_ms, default_document_confirm,
    default_extraction_timeout_ms, default_max_output_tokens, default_others_confirm,
    default_poll_secs, default_recalls_retention_days, default_results_retention_days,
    default_retired_retention_days, default_review_after_days, default_scopes,
    default_source_timeout_ms, default_sources, default_sweep_secs,
};

/// The `brain:` profile block; every field has a default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BrainProfile {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub recall: BrainRecallProfile,
    #[serde(default)]
    pub capture: BrainCaptureProfile,
    #[serde(default)]
    pub extraction: BrainExtractionProfile,
    #[serde(default)]
    pub lifecycle: BrainLifecycleProfile,
    #[serde(default)]
    pub consumer: BrainConsumerProfile,
}

/// Gateway recall: the token budget and deadline, the scopes searched, the
/// clients recall is on for (keyed by `x-systemprompt-client` name; an
/// unlisted client is off) and what happens to stale facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BrainRecallProfile {
    #[serde(default = "default_budget_tokens")]
    pub budget_tokens: u32,
    #[serde(default = "default_deadline_ms")]
    pub deadline_ms: u64,
    #[serde(default = "default_scopes")]
    pub scopes: Vec<ScopeLevel>,
    #[serde(default = "default_clients")]
    pub clients: BTreeMap<String, bool>,
    #[serde(default)]
    pub stale: StalePolicy,
}

impl BrainRecallProfile {
    #[must_use]
    pub fn client_enabled(&self, client: ClientKind) -> bool {
        self.clients.get(client.as_str()).copied().unwrap_or(false)
    }
}

/// Whether stale facts are withheld or injected with a marker.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum StalePolicy {
    #[default]
    Withhold,
    Marker,
}

/// A pathway the brain captures from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CaptureSource {
    Document,
    Comment,
    Session,
    Entered,
}

/// When a new fact is confirmed without a person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AutoConfirm {
    Never,
    ProjectPolicy,
    Always,
}

/// Auto-confirmation for document-sourced facts and for every other source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutoConfirmPolicy {
    #[serde(default = "default_document_confirm")]
    pub document: AutoConfirm,
    #[serde(default = "default_others_confirm")]
    pub others: AutoConfirm,
}

/// What the brain captures and how it confirms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BrainCaptureProfile {
    #[serde(default = "default_sources")]
    pub sources: Vec<CaptureSource>,
    #[serde(default)]
    pub auto_confirm: AutoConfirmPolicy,
}

/// The extraction model call: an optional pinned provider and model, its
/// output cap and timeout, and the daily token budget of the brain's
/// service principal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BrainExtractionProfile {
    #[serde(default)]
    pub provider: Option<ProviderId>,
    #[serde(default)]
    pub model: Option<ModelId>,
    #[serde(default = "default_max_output_tokens")]
    pub max_output_tokens: u32,
    #[serde(default = "default_extraction_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_daily_token_budget")]
    pub daily_token_budget: u64,
}

/// Review and retention windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "field names are the profile YAML keys, where the unit suffix is the contract"
)]
pub struct BrainLifecycleProfile {
    #[serde(default = "default_review_after_days")]
    pub review_after_days: u32,
    #[serde(default = "default_retired_retention_days")]
    pub retired_retention_days: u32,
    #[serde(default = "default_recalls_retention_days")]
    pub recalls_retention_days: u32,
    #[serde(default = "default_results_retention_days")]
    pub results_retention_days: u32,
    #[serde(default = "default_correction_window_days")]
    pub correction_window_days: u32,
}

/// How the brain outbox consumer drains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BrainConsumerProfile {
    #[serde(default = "default_batch")]
    pub batch: u32,
    #[serde(default = "default_poll_secs")]
    pub poll_secs: u32,
    #[serde(default = "default_sweep_secs")]
    pub sweep_secs: u32,
    #[serde(default = "default_source_timeout_ms")]
    pub source_timeout_ms: u64,
}
