mod message_text;
pub mod payload;

use message_text::flatten_message_content;
use payload::{slice_payload, truncate_for_tool_input};

use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use bytes::Bytes;
use systemprompt_ai::models::ai_request_record::AiRequestRecord;
use systemprompt_ai::repository::ai_requests::UpdateCompletionParams;
use systemprompt_ai::repository::{
    AiRequestPayloadRepository, AiRequestRepository, InsertToolCallParams, UpsertPayloadParams,
};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    AiRequestId, ContextId, GatewayConversationId, SessionId, TraceId, UserId,
};

use super::captures::{CapturedToolUse, CapturedUsage};
use super::pricing;
use super::protocol::canonical::{CanonicalRequest, Role};
use super::protocol::canonical_response::CanonicalResponse;
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct GatewayRequestContext {
    pub ai_request_id: AiRequestId,
    pub user_id: UserId,
    pub session_id: Option<SessionId>,
    pub context_id: ContextId,
    pub gateway_conversation_id: Option<GatewayConversationId>,
    pub trace_id: Option<TraceId>,
    pub provider: String,
    pub model: String,
    pub max_tokens: Option<u32>,
    pub is_streaming: bool,
    pub wire_protocol: String,
}

#[expect(missing_debug_implementations, reason = "holds borrowed pool/transaction/service handles that do not implement Debug")]
pub struct GatewayAudit {
    requests: Arc<AiRequestRepository>,
    payloads: Arc<AiRequestPayloadRepository>,
    pub ctx: GatewayRequestContext,
    served_model: Mutex<Option<String>>,
    started_at: Instant,
}

impl GatewayAudit {
    pub fn new(
        db: &DbPool,
        ctx: GatewayRequestContext,
    ) -> Result<Self, systemprompt_ai::error::RepositoryError> {
        let requests = Arc::new(AiRequestRepository::new(db)?);
        let payloads = Arc::new(AiRequestPayloadRepository::new(db)?);
        Ok(Self {
            requests,
            payloads,
            ctx,
            served_model: Mutex::new(None),
            started_at: Instant::now(),
        })
    }

    pub async fn set_served_model(&self, model: &str) {
        if model.is_empty() || model == self.ctx.model {
            return;
        }
        if let Ok(mut slot) = self.served_model.lock() {
            *slot = Some(model.to_owned());
        }
        if let Err(e) = self
            .requests
            .update_model(&self.ctx.ai_request_id, model)
            .await
        {
            tracing::warn!(error = %e, "update_model failed");
        }
    }

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

    fn build_record(&self) -> Result<AiRequestRecord> {
        let mut record =
            AiRequestRecord::builder(self.ctx.ai_request_id.clone(), self.ctx.user_id.clone())
                .provider(self.ctx.provider.clone())
                .model(self.ctx.model.clone())
                .streaming(self.ctx.is_streaming);
        if let Some(s) = &self.ctx.session_id {
            record = record.session_id(s.clone());
        }
        record = record.context_id(self.ctx.context_id.clone());
        if let Some(g) = &self.ctx.gateway_conversation_id {
            record = record.gateway_conversation_id(g.clone());
        }
        if let Some(t) = &self.ctx.trace_id {
            record = record.trace_id(t.clone());
        }
        if let Some(mt) = self.ctx.max_tokens {
            record = record.max_tokens(mt);
        }
        record.build().map_err(anyhow::Error::from)
    }

    pub async fn open(&self, request: &CanonicalRequest, request_body: &Bytes) -> Result<()> {
        let record = self.build_record()?;

        self.requests
            .insert_with_id(&self.ctx.ai_request_id, &record)
            .await?;

        let (body_json, excerpt, truncated, bytes) = slice_payload(request_body);
        if let Err(e) = self
            .payloads
            .upsert_request(
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
            tracing::warn!(error = %e, ai_request_id = %self.ctx.ai_request_id, "payload insert (request) failed");
        }

        self.persist_request_messages(request).await;
        Ok(())
    }

    async fn persist_request_messages(&self, request: &CanonicalRequest) {
        let mut seq = 0i32;
        if let Some(system) = &request.system {
            if !system.is_empty() {
                if let Err(e) = self
                    .requests
                    .insert_message(&self.ctx.ai_request_id, "system", system, seq)
                    .await
                {
                    tracing::warn!(error = %e, "insert system message failed");
                }
                seq += 1;
            }
        }
        for msg in &request.messages {
            let role = match msg.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::Tool => "tool",
            };
            let text = flatten_message_content(&msg.content);
            if let Err(e) = self
                .requests
                .insert_message(&self.ctx.ai_request_id, role, &text, seq)
                .await
            {
                tracing::warn!(error = %e, seq, "insert message failed");
            }
            seq += 1;
        }
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
        let gateway = profile.as_ref().and_then(|p| p.gateway.as_ref());
        let pricing_rates = pricing::resolve(&self.ctx.provider, &effective_model, gateway);
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

        if let Some(assistant_text) = super::parse::extract_assistant_text(response) {
            if let Err(e) = self
                .requests
                .add_response_message(&self.ctx.ai_request_id, &assistant_text)
                .await
            {
                tracing::warn!(error = %e, "assistant response message insert failed");
            }
        }

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

    async fn persist_tool_calls(&self, tool_calls: &[CapturedToolUse]) {
        for (idx, tool) in tool_calls.iter().enumerate() {
            let seq = idx as i32 + 1;
            let trimmed = truncate_for_tool_input(&tool.tool_input);
            if let Err(e) = self
                .requests
                .insert_tool_call(InsertToolCallParams {
                    request_id: &self.ctx.ai_request_id,
                    ai_tool_call_id: &tool.ai_tool_call_id,
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

    pub async fn fail(&self, error: &str) -> Result<()> {
        if let Err(e) = self
            .requests
            .update_error(&self.ctx.ai_request_id, error)
            .await
        {
            tracing::warn!(error = %e, "audit fail update failed");
        }
        tracing::info!(
            ai_request_id = %self.ctx.ai_request_id,
            user_id = %self.ctx.user_id,
            provider = %self.ctx.provider,
            model = %self.ctx.model,
            error,
            "Gateway audit: request failed"
        );
        Ok(())
    }
}
