//! The failure settlements: a request that never produced a completion.
//!
//! A failure is settled with whatever the provider streamed before it broke:
//! that partial usage is priced the same way a completion is, so a truncated
//! stream is not free.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_ai::models::RequestStatus;

use super::{GatewayAudit, journal};
use crate::services::gateway::protocol::canonical_response::CanonicalUsage;

impl GatewayAudit {
    pub async fn accounting_failed(&self, error: &str) -> Result<()> {
        let mut receipt = journal::Receipt::pending(
            self.ctx.ai_request_id.clone(),
            self.ctx.user_id.clone(),
            self.ctx.session_id.clone(),
        );
        receipt.accounting_failure = Some(error.to_owned());
        journal::record_accounting_failure(&self.settlement, receipt).await
    }

    pub async fn fail(&self, error: &str) -> Result<()> {
        self.fail_with_usage(error, None).await
    }

    pub async fn fail_with_usage(&self, error: &str, usage: Option<CanonicalUsage>) -> Result<()> {
        let latency_ms = self.elapsed_ms();
        let mut receipt = journal::Receipt::pending(
            self.ctx.ai_request_id.clone(),
            self.ctx.user_id.clone(),
            self.ctx.session_id.clone(),
        );
        receipt.failure = Some(error.to_owned());
        receipt.partial = usage
            .filter(|usage| usage.billable_total() > 0)
            .map(|mut usage| {
                usage.normalise_reasoning(&self.ctx.provider);
                let cost = self
                    .completion_pricing(&self.effective_model())
                    .map_or(0, |rates| rates.cost_microdollars(&usage));
                journal::PartialUsage {
                    usage: [
                        usage.input_tokens,
                        usage.output_tokens,
                        usage.cache_read_tokens,
                        usage.cache_creation_tokens,
                        usage.reasoning_tokens,
                        usage.billable_total(),
                    ],
                    cost,
                    latency: latency_ms,
                    upstream_latency: self.upstream_elapsed_ms(),
                }
            });
        let tokens_recorded = receipt.partial.is_some();
        if self.journal_lease.get().is_some() {
            journal::record(&self.settlement, receipt).await?;
        } else {
            journal::settle_unadmitted_failure(&self.settlement, &receipt).await?;
        }
        tracing::warn!(
            ai_request_id = %self.ctx.ai_request_id,
            user_id = %self.ctx.user_id,
            provider = %self.served_provider(),
            model = %self.effective_model(),
            requested_model = %self.ctx.model,
            wire_protocol = self.ctx.origin.wire.as_str(),
            client_kind = self.ctx.origin.client.as_str(),
            status = RequestStatus::Failed.as_str(),
            latency_ms,
            tokens_recorded,
            error,
            "Gateway audit: request failed"
        );
        Ok(())
    }
}
