//! Defaults for the brain profile sections, as serde and `Default` use
//! them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_models::brain::ScopeLevel;
use systemprompt_models::origin::ClientKind;

use super::brain::{
    AutoConfirm, AutoConfirmPolicy, BrainCaptureProfile, BrainConsumerProfile,
    BrainExtractionProfile, BrainLifecycleProfile, BrainRecallProfile, CaptureSource, StalePolicy,
};

pub(super) const fn default_budget_tokens() -> u32 {
    800
}

pub(super) const fn default_deadline_ms() -> u64 {
    50
}

pub(super) fn default_scopes() -> Vec<ScopeLevel> {
    ScopeLevel::ALL.to_vec()
}

pub(super) fn default_clients() -> BTreeMap<String, bool> {
    [ClientKind::ClaudeCode, ClientKind::ClaudeDesktop]
        .into_iter()
        .map(|client| (client.as_str().to_owned(), true))
        .collect()
}

pub(super) fn default_sources() -> Vec<CaptureSource> {
    vec![
        CaptureSource::Document,
        CaptureSource::Comment,
        CaptureSource::Session,
        CaptureSource::Entered,
    ]
}

pub(super) const fn default_document_confirm() -> AutoConfirm {
    AutoConfirm::ProjectPolicy
}

pub(super) const fn default_others_confirm() -> AutoConfirm {
    AutoConfirm::Never
}

pub(super) const fn default_max_output_tokens() -> u32 {
    2048
}

pub(super) const fn default_extraction_timeout_ms() -> u64 {
    60_000
}

pub(super) const fn default_daily_token_budget() -> u64 {
    1_000_000
}

pub(super) const fn default_correction_window_days() -> u32 {
    14
}

pub(super) const fn default_review_after_days() -> u32 {
    90
}

pub(super) const fn default_retired_retention_days() -> u32 {
    365
}

pub(super) const fn default_recalls_retention_days() -> u32 {
    90
}

pub(super) const fn default_results_retention_days() -> u32 {
    30
}

pub(super) const fn default_batch() -> u32 {
    32
}

pub(super) const fn default_poll_secs() -> u32 {
    5
}

pub(super) const fn default_sweep_secs() -> u32 {
    300
}

pub(super) const fn default_source_timeout_ms() -> u64 {
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
            correction_window_days: default_correction_window_days(),
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
