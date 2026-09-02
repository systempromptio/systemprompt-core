//! Closing a gateway audit record: completion metrics, tool calls, and the
//! response payload.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use bytes::Bytes;
use systemprompt_ai::repository::ai_requests::UpdateCompletionParams;
use systemprompt_ai::repository::{InsertToolCallParams, UpsertPayloadParams};
use systemprompt_identifiers::AiToolCallId;

use super::GatewayAudit;
use super::payload::{slice_payload, truncate_for_tool_input};
use crate::services::gateway::captures::{CapturedToolUse, CapturedUsage};
use crate::services::gateway::pricing;
use crate::services::gateway::protocol::canonical_response::CanonicalResponse;

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
        usage: CapturedUsage,
        tool_calls: Vec<CapturedToolUse>,
        response: &CanonicalResponse,
        response_body: &Bytes,
    ) -> Result<i64> {
        let latency_ms = self.elapsed_ms();
        let effective_model = self.effective_model();
        let services = systemprompt_loader::ServicesBootstrap::get().ok();
        let gateway =
            services.and_then(systemprompt_models::services::ServicesConfig::gateway_config);
        let empty_registry = systemprompt_models::services::ProviderRegistry::default();
        let registry = services.map_or(&empty_registry, |s| &s.providers);
        let candidates = [
            effective_model.as_str(),
            self.ctx.model.as_str(),
            self.ctx.requested_model.as_deref().unwrap_or(""),
        ];
        let pricing_rates = pricing::resolve(&self.ctx.provider, &candidates, gateway, registry);
        let cost = pricing::cost_microdollars(
            pricing_rates,
            pricing::CostTokens {
                input: usage.input_tokens,
                output: usage.output_tokens,
                cache_read: usage.cache_read_tokens,
                cache_creation: usage.cache_creation_tokens,
            },
        );
        let tokens_used = usage.input_tokens
            + usage.output_tokens
            + usage.cache_read_tokens
            + usage.cache_creation_tokens;

        self.requests
            .update_completion(UpdateCompletionParams {
                id: self.ctx.ai_request_id.clone(),
                tokens_used: tokens_used as i32,
                input_tokens: usage.input_tokens as i32,
                output_tokens: usage.output_tokens as i32,
                cost_microdollars: cost,
                latency_ms,
                cache_hit: usage.cache_read_tokens > 0,
                cache_read_tokens: usage.cache_read_tokens as i32,
                cache_creation_tokens: usage.cache_creation_tokens as i32,
            })
            .await?;

        self.persist_tool_calls(&tool_calls).await;
        self.persist_response(response, response_body).await;

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
            tokens_used,
            cost_microdollars = cost,
            latency_ms,
            tool_calls = tool_calls.len(),
            "Gateway audit: request completed"
        );
        Ok(cost)
    }

    async fn persist_response(&self, response: &CanonicalResponse, response_body: &Bytes) {
        let capture = slice_payload(response_body);
        if let Err(e) = self
            .payloads
            .upsert_response(
                &self.ctx.ai_request_id,
                UpsertPayloadParams {
                    body: capture.json.as_ref(),
                    excerpt: capture.excerpt.as_deref(),
                    truncated: capture.truncated,
                    bytes: Some(capture.byte_len),
                    sha256: Some(capture.sha256.as_str()),
                },
            )
            .await
        {
            tracing::warn!(error = %e, ai_request_id = %self.ctx.ai_request_id, "payload insert (response) failed");
        }

        if let Some(assistant_text) = super::super::parse::extract_assistant_text(response)
            && let Err(e) = self
                .requests
                .add_response_message(&self.ctx.ai_request_id, &assistant_text)
                .await
        {
            tracing::warn!(error = %e, "assistant response message insert failed");
        }
    }

    async fn persist_tool_calls(&self, tool_calls: &[CapturedToolUse]) {
        for (idx, tool) in tool_calls.iter().enumerate() {
            let seq = idx as i32 + 1;
            let trimmed = truncate_for_tool_input(&tool.tool_input);
            let ai_tool_call_id = AiToolCallId::new(tool.ai_tool_call_id.clone());
            if let Err(e) = self
                .requests
                .insert_tool_call(InsertToolCallParams {
                    request_id: &self.ctx.ai_request_id,
                    ai_tool_call_id: &ai_tool_call_id,
                    tool_name: &tool.tool_name,
                    tool_input: &trimmed,
                    sequence_number: seq,
                })
                .await
            {
                tracing::warn!(error = %e, seq, "tool_call insert failed");
            }
        }
    }
}
