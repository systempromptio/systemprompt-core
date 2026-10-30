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
pub struct BrainLifecycleProfile {
    #[serde(default = "default_review_after_days")]
    pub review_after_days: u32,
    #[serde(default = "default_retired_retention_days")]
    pub retired_retention_days: u32,
    #[serde(default = "default_recalls_retention_days")]
    pub recalls_retention_days: u32,
    #[serde(default = "default_results_retention_days")]
    pub results_retention_days: u32,
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

const fn default_budget_tokens() -> u32 {
    800
}

const fn default_deadline_ms() -> u64 {
    50
}

fn default_scopes() -> Vec<ScopeLevel> {
    ScopeLevel::ALL.to_vec()
}

fn default_clients() -> BTreeMap<String, bool> {
    [ClientKind::ClaudeCode, ClientKind::ClaudeDesktop]
        .into_iter()
        .map(|client| (client.as_str().to_owned(), true))
        .collect()
}

fn default_sources() -> Vec<CaptureSource> {
    vec![
        CaptureSource::Document,
        CaptureSource::Comment,
        CaptureSource::Session,
        CaptureSource::Entered,
    ]
}

const fn default_document_confirm() -> AutoConfirm {
    AutoConfirm::ProjectPolicy
}

const fn default_others_confirm() -> AutoConfirm {
    AutoConfirm::Never
}

const fn default_max_output_tokens() -> u32 {
    2048
}

const fn default_extraction_timeout_ms() -> u64 {
    60_000
}

const fn default_daily_token_budget() -> u64 {
    1_000_000
}

const fn default_review_after_days() -> u32 {
    90
}

const fn default_retired_retention_days() -> u32 {
    365
}

const fn default_recalls_retention_days() -> u32 {
    90
}

const fn default_results_retention_days() -> u32 {
    30
}

const fn default_batch() -> u32 {
    32
}

const fn default_poll_secs() -> u32 {
    5
}

const fn default_sweep_secs() -> u32 {
    300
}

const fn default_source_timeout_ms() -> u64 {
    5000
}

impl Default for BrainRecallProfile {
    fn default() -> Self {
        Self {
            budget_tokens: default_budget_tokens(),
            deadline_ms: default_deadline_ms(),
            scopes: default_scopes(),
            clients: default_clients(),
            stale: StalePolicy::default(),
        }
    }
}

impl Default for AutoConfirmPolicy {
    fn default() -> Self {
        Self {
            document: default_document_confirm(),
            others: default_others_confirm(),
        }
    }
}

impl Default for BrainCaptureProfile {
    fn default() -> Self {
        Self {
            sources: default_sources(),
            auto_confirm: AutoConfirmPolicy::default(),
        }
    }
}

impl Default for BrainExtractionProfile {
    fn default() -> Self {
        Self {
            provider: None,
            model: None,
            max_output_tokens: default_max_output_tokens(),
            timeout_ms: default_extraction_timeout_ms(),
            daily_token_budget: default_daily_token_budget(),
        }
    }
}

impl Default for BrainLifecycleProfile {
    fn default() -> Self {
        Self {
            review_after_days: default_review_after_days(),
            retired_retention_days: default_retired_retention_days(),
            recalls_retention_days: default_recalls_retention_days(),
            results_retention_days: default_results_retention_days(),
        }
    }
}

impl Default for BrainConsumerProfile {
    fn default() -> Self {
        Self {
            batch: default_batch(),
            poll_secs: default_poll_secs(),
            sweep_secs: default_sweep_secs(),
            source_timeout_ms: default_source_timeout_ms(),
        }
    }
}
