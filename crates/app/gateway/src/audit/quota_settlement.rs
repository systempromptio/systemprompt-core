//! The audit owns a request's quota reservation: admission stores it here,
//! completion settles it to the audited usage, and failure (including an
//! abandoned request) releases it. The reservation is taken once, so a request
//! is settled at most once whichever terminal path runs first.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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

    pub(super) async fn settle_quota_usage(&self, usage: QuotaUsage) -> AccountingOutcome {
        let Some(reservation) = self.take_quota_reservation() else {
            return AccountingOutcome::Counted;
        };
        quota::settle(&self.quota_buckets, &reservation, usage).await
    }
}
