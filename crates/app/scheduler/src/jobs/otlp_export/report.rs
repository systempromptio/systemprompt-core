//! What an OTLP export run reports per signal, and the `batch_seconds` pacing
//! check that decides whether a signal is due.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{Duration, Utc};
use serde::Serialize;
use systemprompt_manifest::profile::OtlpSignal;

/// What one run did for one signal.
#[derive(Debug, Clone, Serialize)]
pub struct SignalReport {
    pub signal: OtlpSignal,
    pub rows: u64,
    pub skipped: u64,
    pub paced: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ExportReport {
    pub signals: Vec<SignalReport>,
}

impl ExportReport {
    #[must_use]
    pub fn rows(&self) -> u64 {
        self.signals.iter().map(|s| s.rows).sum()
    }

    #[must_use]
    pub fn skipped(&self) -> u64 {
        self.signals.iter().map(|s| s.skipped).sum()
    }

    #[must_use]
    pub fn failed(&self) -> u64 {
        self.signals.iter().filter(|s| s.error.is_some()).count() as u64
    }
}

#[must_use]
pub fn pacing_elapsed(
    last_attempt_at: Option<chrono::DateTime<Utc>>,
    now: chrono::DateTime<Utc>,
    batch_seconds: u64,
) -> bool {
    let pace = i64::try_from(batch_seconds)
        .ok()
        .and_then(Duration::try_seconds)
        .unwrap_or(Duration::MAX);
    last_attempt_at.is_none_or(|last| now - last >= pace)
}
