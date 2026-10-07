//! Quota decisions and the machine-readable detail a `429` carries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use systemprompt_ai::repository::QuotaBucketState;

use crate::policies::QuotaWindow;

/// The ceiling of a quota window that a request breached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaDimension {
    Requests,
    InputTokens,
    OutputTokens,
    CostMicrodollars,
}

impl QuotaDimension {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Requests => "requests",
            Self::InputTokens => "input_tokens",
            Self::OutputTokens => "output_tokens",
            Self::CostMicrodollars => "cost_microdollars",
        }
    }
}

/// The `error.quota` object of a quota `429`.
///
/// `dimension`, `limit` and `used` are absent when the window could not be
/// evaluated (a subject-resolution fault under `QuotaFaultMode::Closed`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QuotaDetail {
    pub window_seconds: i32,
    pub subject: String,
    pub dimension: Option<QuotaDimension>,
    pub limit: Option<i64>,
    pub used: Option<i64>,
    pub resets_at: DateTime<Utc>,
    pub retry_after_seconds: i64,
}

impl QuotaDetail {
    fn new(window: &QuotaWindow, window_start: DateTime<Utc>, now: DateTime<Utc>) -> Self {
        let resets_at = window_start + Duration::seconds(i64::from(window.window_seconds.max(1)));
        Self {
            window_seconds: window.window_seconds,
            subject: window.subject.clone(),
            dimension: None,
            limit: None,
            used: None,
            resets_at,
            retry_after_seconds: (resets_at - now).num_seconds().max(1),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuotaDecision {
    pub allow: bool,
    pub window_seconds: i32,
    pub message: String,
    pub state: QuotaBucketState,
    pub detail: QuotaDetail,
}

const EMPTY_STATE: QuotaBucketState = QuotaBucketState {
    requests: 0,
    input_tokens: 0,
    output_tokens: 0,
    cost_microdollars: 0,
};

pub(super) fn fault_decision(
    window: &QuotaWindow,
    fault: &str,
    window_start: DateTime<Utc>,
    now: DateTime<Utc>,
) -> QuotaDecision {
    QuotaDecision {
        allow: false,
        window_seconds: window.window_seconds,
        message: format!(
            "quota window {}s for subject '{}' could not be evaluated: {fault}",
            window.window_seconds, window.subject
        ),
        state: EMPTY_STATE,
        detail: QuotaDetail::new(window, window_start, now),
    }
}

pub(super) fn ceiling_decision(
    window: &QuotaWindow,
    state: QuotaBucketState,
    window_start: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Option<QuotaDecision> {
    let (dimension, max, used, label, verb, unit) = [
        (
            QuotaDimension::Requests,
            window.max_requests,
            state.requests,
            "request",
            "used",
            "",
        ),
        (
            QuotaDimension::InputTokens,
            window.max_input_tokens,
            state.input_tokens,
            "input token",
            "consumed",
            " tokens",
        ),
        (
            QuotaDimension::OutputTokens,
            window.max_output_tokens,
            state.output_tokens,
            "output token",
            "consumed",
            " tokens",
        ),
        (
            QuotaDimension::CostMicrodollars,
            window.max_cost_microdollars,
            state.cost_microdollars,
            "cost",
            "spent",
            " microdollars",
        ),
    ]
    .into_iter()
    .find_map(|(dimension, max, used, label, verb, unit)| match max {
        Some(max) if used > max => Some((dimension, max, used, label, verb, unit)),
        _ => None,
    })?;
    let mut detail = QuotaDetail::new(window, window_start, now);
    detail.dimension = Some(dimension);
    detail.limit = Some(max);
    detail.used = Some(used);
    Some(QuotaDecision {
        allow: false,
        window_seconds: window.window_seconds,
        message: format!(
            "{label} ceiling exceeded for {} window {}s ({verb} {used}/{max}{unit})",
            window.subject, window.window_seconds
        ),
        state,
        detail,
    })
}
