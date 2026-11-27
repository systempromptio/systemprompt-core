//! The usage counters the heartbeat reports: calls the gateway answered, step
//! tokens, and the last successful call.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use systemprompt_engine::llm::Usage;

/// What the instance's gateway carried since the instance was applied.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UsageTotals {
    pub forwarded_total: u64,
    pub tokens_in_total: u64,
    pub tokens_out_total: u64,
    pub last_activity_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Default)]
pub(crate) struct Counters(Mutex<UsageTotals>);

impl Counters {
    pub(crate) fn snapshot(&self) -> UsageTotals {
        *self.0.lock()
    }

    pub(crate) fn reset(&self) {
        *self.0.lock() = UsageTotals::default();
    }

    pub(crate) fn answered(&self, status: u16, now: DateTime<Utc>) {
        let mut totals = self.0.lock();
        totals.forwarded_total = totals.forwarded_total.saturating_add(1);
        if (200..300).contains(&status) {
            totals.last_activity_at = Some(now);
        }
    }

    // Why: harness-llm's `input` already counts cache reads and writes, and its
    // `output` already counts reasoning, which is how the gateway's own
    // accounting counts prompt and completion tokens.
    pub(crate) fn step(&self, usage: &Usage) {
        let mut totals = self.0.lock();
        totals.tokens_in_total = totals.tokens_in_total.saturating_add(usage.input);
        totals.tokens_out_total = totals.tokens_out_total.saturating_add(usage.output);
    }
}
