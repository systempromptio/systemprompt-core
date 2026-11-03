//! The two clocks a request runs: whole-request latency and the upstream
//! bracket around the provider call (see the module head of `audit`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

use super::GatewayAudit;

#[derive(Debug, Default)]
pub(super) struct UpstreamClock {
    pub(super) started: Option<Instant>,
    pub(super) elapsed_ms: Option<i32>,
}

impl GatewayAudit {
    pub fn mark_upstream_start(&self) {
        match self.upstream.lock() {
            Ok(mut clock) => {
                if clock.elapsed_ms.is_none() {
                    clock.started = Some(Instant::now());
                }
            },
            Err(e) => tracing::warn!(error = %e, "upstream clock mutex poisoned"),
        }
    }

    pub fn mark_upstream_end(&self) {
        match self.upstream.lock() {
            Ok(mut clock) => {
                if clock.elapsed_ms.is_some() {
                    return;
                }
                let Some(started) = clock.started else {
                    return;
                };
                clock.elapsed_ms = Some(millis_i32(started));
            },
            Err(e) => tracing::warn!(error = %e, "upstream clock mutex poisoned"),
        }
    }

    pub(crate) fn upstream_elapsed_ms(&self) -> Option<i32> {
        match self.upstream.lock() {
            Ok(clock) => clock.elapsed_ms,
            Err(e) => {
                tracing::warn!(error = %e, "upstream clock mutex poisoned");
                None
            },
        }
    }

    pub(crate) fn elapsed_ms(&self) -> i32 {
        millis_i32(self.started_at)
    }
}

fn millis_i32(since: Instant) -> i32 {
    since.elapsed().as_millis().min(i32::MAX as u128) as i32
}
