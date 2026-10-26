//! Charging a governed call's rate windows ahead of the pure chain walk.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{Evaluation, GovernanceEngine};
use crate::policy::rate_window::{RateCharge, RateCounts};
use crate::policy::types::{PolicyContext, PolicyRequest};

/// A [`PolicyRequest`] whose rate windows the engine has charged; the only
/// production source of a [`PolicyContext`] with real rate counts.
#[derive(Debug)]
pub struct ChargedCall<'a> {
    request: PolicyRequest<'a>,
    rate_counts: RateCounts,
}

impl ChargedCall<'_> {
    #[must_use]
    pub fn context(&self) -> PolicyContext<'_> {
        self.request.with_counts(&self.rate_counts)
    }
}

impl GovernanceEngine {
    pub async fn charge_and_evaluate(&self, request: PolicyRequest<'_>) -> Evaluation {
        let charged = self.charge(request).await;
        self.evaluate(&charged.context())
    }

    pub async fn charge<'a>(&self, request: PolicyRequest<'a>) -> ChargedCall<'a> {
        let rate_counts = self.charge_rate_windows(&request).await;
        ChargedCall {
            request,
            rate_counts,
        }
    }

    async fn charge_rate_windows(&self, request: &PolicyRequest<'_>) -> RateCounts {
        let mut counts = RateCounts::empty();
        if !self.enabled || self.unapplied.is_some() {
            return counts;
        }
        let Some(repository) = self.rate_windows.as_ref() else {
            return counts;
        };
        let subject = format!(
            "{}:{}",
            request.session_id.as_str(),
            request.user_id.as_str()
        );
        for entry in self.entries.iter().filter(|e| e.config.enabled) {
            let Some(window) = entry.instance.rate_window() else {
                continue;
            };
            let charge = RateCharge {
                policy_id: &entry.config.id,
                window_secs: window.seconds,
                subject: &subject,
                call_id: request.call_id,
            };
            match repository.charge(charge).await {
                Ok(position) => counts.record(entry.config.id.clone(), position),
                Err(error) => tracing::error!(
                    %error,
                    policy_id = %entry.config.id,
                    "rate window charge failed; the policy fails closed for this call"
                ),
            }
        }
        counts
    }
}
