//! The audit owns a request's quota reservation: admission stores it here,
//! completion settles it to the audited usage, and failure (including an
//! abandoned request) releases it. The reservation is taken once, so a request
//! is settled at most once whichever terminal path runs first.
//!
//! A request served from the caller's own subscription key still counts its
//! tokens against every window, but settles zero cost: the estimate stays on
//! the audit row for reporting, and the platform's spend never carries it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::atomic::Ordering;

use super::GatewayAudit;
use crate::protocol::canonical::CanonicalUsage;
use crate::quota::{self, AccountingOutcome, QuotaReservation, QuotaUsage};

impl GatewayAudit {
    pub fn set_quota_reservation(&self, reservation: QuotaReservation) {
        match self.quota_reservation.lock() {
            Ok(mut slot) => *slot = Some(reservation),
            Err(e) => tracing::warn!(error = %e, "quota reservation mutex poisoned"),
        }
    }

    fn take_quota_reservation(&self) -> Option<QuotaReservation> {
        match self.quota_reservation.lock() {
            Ok(mut slot) => slot.take(),
            Err(e) => {
                tracing::warn!(error = %e, "quota reservation mutex poisoned");
                None
            },
        }
    }

    pub async fn settle_quota(&self, usage: &CanonicalUsage, cost: i64) -> AccountingOutcome {
        self.settle_quota_usage(QuotaUsage {
            input_tokens: i64::from(usage.input_tokens),
            output_tokens: i64::from(usage.output_tokens),
            cost_microdollars: cost,
        })
        .await
    }

    /// Marks the request billed to the caller's personal key: quota settles no
    /// cost for it. Called before admission so every settlement path sees it.
    pub fn bill_to_user_key(&self) {
        self.user_key_billing.store(true, Ordering::Relaxed);
    }

    #[must_use]
    pub fn billed_to_user_key(&self) -> bool {
        self.user_key_billing.load(Ordering::Relaxed)
    }

    /// Stamps `billing_source = 'user_key'` on the admitted row.
    pub async fn record_billing_source(&self) {
        if !self.billed_to_user_key() {
            return;
        }
        if let Err(e) = self
            .requests
            .update_billing_source(&self.ctx.ai_request_id, "user_key")
            .await
        {
            tracing::warn!(error = %e, "update_billing_source failed");
        }
    }

    pub(super) async fn settle_quota_usage(&self, mut usage: QuotaUsage) -> AccountingOutcome {
        if self.billed_to_user_key() {
            usage.cost_microdollars = 0;
        }
        let Some(reservation) = self.take_quota_reservation() else {
            return AccountingOutcome::Counted;
        };
        quota::settle(&self.quota_buckets, &reservation, usage).await
    }
}
