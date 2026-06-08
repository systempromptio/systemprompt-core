//! Closing a gateway audit record: completion metrics, tool calls, and the
//! response payload.

use anyhow::Result;
use bytes::Bytes;
use systemprompt_ai::repository::ai_requests::UpdateCompletionParams;
use systemprompt_ai::repository::{InsertToolCallParams, UpsertPayloadParams};

use super::GatewayAudit;
use super::payload::{slice_payload, truncate_for_tool_input};
use crate::services::gateway::captures::{CapturedToolUse, CapturedUsage};
use crate::services::gateway::pricing;
use crate::services::gateway::protocol::canonical_response::CanonicalResponse;

impl GatewayAudit {
    fn effective_model(&self) -> String {
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
    ) -> Result<()> {
        let latency_ms = self.started_at.elapsed().as_millis().min(i32::MAX as u128) as i32;
        let effective_model = self.effective_model();
        let profile = systemprompt_config::ProfileBootstrap::get().ok();
        let gateway = profile
            .as_ref()
            .and_then(|p| p.gateway.as_ref())
            .and_then(systemprompt_models::profile::GatewayState::resolved);
        let empty_registry = systemprompt_models::profile::ProviderRegistry::default();
        let registry = profile.as_ref().map_or(&empty_registry, |p| &p.providers);
        let candidates = [
            effective_model.as_str(),
            self.ctx.model.as_str(),
            self.ctx.requested_model.as_deref().unwrap_or(""),
        ];
        let pricing_rates = pricing::resolve(&self.ctx.provider, &candidates, gateway, registry);
        let cost =
            pricing::cost_microdollars(pricing_rates, usage.input_tokens, usage.output_tokens);

        self.requests
            .update_completion(UpdateCompletionParams {
                id: self.ctx.ai_request_id.clone(),
                tokens_used: (usage.input_tokens + usage.output_tokens) as i32,
                input_tokens: usage.input_tokens as i32,
                output_tokens: usage.output_tokens as i32,
                cost_microdollars: cost,
                latency_ms,
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
            cost_microdollars = cost,
            latency_ms,
            tool_calls = tool_calls.len(),
            "Gateway audit: request completed"
        );
        Ok(())
    }

    async fn persist_response(&self, response: &CanonicalResponse, response_body: &Bytes) {
        let (body_json, excerpt, truncated, bytes) = slice_payload(response_body);
        if let Err(e) = self
            .payloads
            .upsert_response(
                &self.ctx.ai_request_id,
                UpsertPayloadParams {
                    body: body_json.as_ref(),
                    excerpt: excerpt.as_deref(),
                    truncated,
                    bytes: Some(bytes),
                },
            )
            .await
        {
            tracing::warn!(error = %e, ai_request_id = %self.ctx.ai_request_id, "payload insert (response) failed");
        }

        if let Some(assistant_text) = super::super::parse::extract_assistant_text(response) {
            if let Err(e) = self
                .requests
                .add_response_message(&self.ctx.ai_request_id, &assistant_text)
                .await
            {
                tracing::warn!(error = %e, "assistant response message insert failed");
            }
        }
    }

    async fn persist_tool_calls(&self, tool_calls: &[CapturedToolUse]) {
        for (idx, tool) in tool_calls.iter().enumerate() {
            let seq = idx as i32 + 1;
            let trimmed = truncate_for_tool_input(&tool.tool_input);
            if let Err(e) = self
                .requests
                .insert_tool_call(InsertToolCallParams {
                    request_id: &self.ctx.ai_request_id,
                    ai_tool_call_id: tool.ai_tool_call_id.as_str(),
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
