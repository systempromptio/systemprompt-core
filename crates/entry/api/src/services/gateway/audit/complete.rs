//! Closing a gateway audit record: completion metrics, tool calls, and the
//! response payload.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use bytes::Bytes;

use super::GatewayAudit;
use super::payload::{slice_payload, truncate_for_tool_input};
use crate::services::gateway::captures::CapturedToolUse;
use crate::services::gateway::pricing;
use crate::services::gateway::protocol::canonical_response::{CanonicalResponse, CanonicalUsage};

impl GatewayAudit {
    pub(super) fn effective_model(&self) -> String {
        self.served_model
            .lock()
            .map_err(|e| {
                tracing::warn!(error = %e, "served_model mutex poisoned");
                e
            })
            .ok()
            .and_then(|s| s.clone())
            .unwrap_or_else(|| self.ctx.model.clone())
    }

    pub async fn complete(
        &self,
        mut usage: CanonicalUsage,
        tool_calls: Vec<CapturedToolUse>,
        response: &CanonicalResponse,
        response_body: &Bytes,
    ) -> Result<i64> {
        let latency_ms = self.elapsed_ms();
        let upstream_latency_ms = self.upstream_elapsed_ms();
        let effective_model = self.effective_model();
        usage.normalise_reasoning(&self.ctx.provider);
        let pricing_rates = self.completion_pricing(&effective_model)?;
        let cost = pricing_rates.cost_microdollars(&usage);
        let tokens_used = usage.billable_total();

        let completion = super::journal::Completion {
            usage: [
                usage.input_tokens,
                usage.output_tokens,
                usage.cache_read_tokens,
                usage.cache_creation_tokens,
                usage.reasoning_tokens,
                tokens_used,
            ],
            cost,
            latency: latency_ms,
            upstream_latency: upstream_latency_ms,
            payload: slice_payload(response_body),
            assistant: super::super::parse::extract_assistant_text(response)
                .map(|text| truncate_for_tool_input(&text)),
            tools: tool_calls
                .iter()
                .map(|tool| {
                    (
                        tool.ai_tool_call_id.to_string(),
                        tool.tool_name.clone(),
                        truncate_for_tool_input(&tool.tool_input),
                    )
                })
                .collect(),
        };
        let mut receipt = super::journal::Receipt::pending(
            self.ctx.ai_request_id.clone(),
            self.ctx.user_id.clone(),
        );
        receipt.completion = Some(completion);
        super::journal::record(receipt, &self.audit_pool).await?;

        tracing::info!(
            ai_request_id = %self.ctx.ai_request_id,
            user_id = %self.ctx.user_id,
            provider = %self.ctx.provider,
            model = %effective_model,
            wire_protocol = %self.ctx.wire_protocol,
            input_tokens = usage.input_tokens,
            output_tokens = usage.output_tokens,
            cache_read_tokens = usage.cache_read_tokens,
            cache_creation_tokens = usage.cache_creation_tokens,
            reasoning_tokens = usage.reasoning_tokens,
            tokens_used,
            cost_microdollars = cost,
            latency_ms,
            upstream_latency_ms,
            gateway_overhead_ms = upstream_latency_ms.map(|u| latency_ms.saturating_sub(u)),
            tool_calls = tool_calls.len(),
            "Gateway audit: request completed"
        );
        Ok(cost)
    }

    pub fn pin_pricing(&self, pricing: systemprompt_models::services::ModelPricing) -> Result<()> {
        self.pricing_snapshot
            .set(pricing)
            .map_err(|_rejected_pricing| anyhow::anyhow!("Gateway pricing already pinned"))
    }

    fn completion_pricing(
        &self,
        effective_model: &str,
    ) -> Result<systemprompt_models::services::ModelPricing> {
        if let Some(pricing) = self.pricing_snapshot.get() {
            return Ok(*pricing);
        }
        let services = systemprompt_loader::ServicesBootstrap::get()?;
        let candidates = [effective_model, self.ctx.model.as_str()];
        Ok(pricing::resolve(
            &self.ctx.provider,
            &candidates,
            services.gateway_config(),
            &services.providers,
        )?)
    }
}
