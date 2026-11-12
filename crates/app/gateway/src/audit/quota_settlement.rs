//! The audit owns a request's quota reservation: admission stores it here,
//! completion settles it to the audited usage, and failure (including an
//! abandoned request) releases it. The reservation is taken once, so a request
//! is settled at most once whichever terminal path runs first.
//!
//! A request billed to the member — served from their stored key or from the
//! credential they presented — still counts its tokens against every window,
//! but settles zero cost: the estimate stays on the audit row for reporting,
//! and the platform's spend never carries it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_ai::models::BillingSource;

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

    pub fn bill_to(&self, source: BillingSource) {
        if self.billing.set(source).is_err() {
            tracing::warn!(
                ai_request_id = %self.ctx.ai_request_id,
                "billing source already set for this request"
            );
        }
    }

    #[must_use]
    pub fn billing_source(&self) -> BillingSource {
        self.billing
            .get()
            .copied()
            .unwrap_or(BillingSource::Platform)
    }

    pub async fn record_billing_source(&self) {
        let source = self.billing_source();
        if source.is_platform() {
            return;
        }
        if let Err(e) = self
            .requests
            .update_billing_source(&self.ctx.ai_request_id, source.as_str())
            .await
        {
            tracing::warn!(error = %e, "update_billing_source failed");
        }
    }

    pub async fn settle_quota_unmetered(
        &self,
        usage: &CanonicalUsage,
        cost: i64,
    ) -> AccountingOutcome {
        let observed = QuotaUsage {
            input_tokens: i64::from(usage.input_tokens),
            output_tokens: i64::from(usage.output_tokens),
            cost_microdollars: cost,
        };
        let cost_floor = if self.billing_source().is_platform() {
            quota::CostFloor::Reserved
        } else {
            quota::CostFloor::Waived
        };
        let Some(reservation) = self.take_quota_reservation() else {
            return AccountingOutcome::Counted;
        };
        quota::settle_unmetered(&self.quota_buckets, &reservation, observed, cost_floor).await
    }

    pub(super) async fn settle_quota_usage(&self, mut usage: QuotaUsage) -> AccountingOutcome {
        if !self.billing_source().is_platform() {
            usage.cost_microdollars = 0;
        }
        let Some(reservation) = self.take_quota_reservation() else {
            return AccountingOutcome::Counted;
        };
        quota::settle(&self.quota_buckets, &reservation, usage).await
    }
}
