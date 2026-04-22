use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use bytes::Bytes;
use serde_json::Value;
use systemprompt_ai::models::ai_request_record::AiRequestRecord;
use systemprompt_ai::repository::ai_requests::UpdateCompletionParams;
use systemprompt_ai::repository::{
    AiRequestPayloadRepository, AiRequestRepository, InsertToolCallParams, UpsertPayloadParams,
};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiRequestId, SessionId, TenantId, TraceId, UserId};

use super::pricing::{self, ModelPricing};

const PAYLOAD_CAP_BYTES: usize = 256 * 1024;
const EXCERPT_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone)]
pub struct GatewayRequestContext {
    pub ai_request_id: AiRequestId,
    pub user_id: UserId,
    pub tenant_id: Option<TenantId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub provider: String,
    pub model: String,
    pub max_tokens: Option<u32>,
    pub is_streaming: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CapturedUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct CapturedToolUse {
    pub ai_tool_call_id: String,
    pub tool_name: String,
    pub tool_input: String,
}

#[derive(Clone, Debug)]
pub struct GatewayAudit {
    requests: Arc<AiRequestRepository>,
    payloads: Arc<AiRequestPayloadRepository>,
    pub ctx: GatewayRequestContext,
    pricing: ModelPricing,
    started_at: Instant,
}

impl GatewayAudit {
    pub fn new(
        db: &DbPool,
        ctx: GatewayRequestContext,
    ) -> Result<Self, systemprompt_ai::error::RepositoryError> {
        let requests = Arc::new(AiRequestRepository::new(db)?);
        let payloads = Arc::new(AiRequestPayloadRepository::new(db)?);
        let pricing = pricing::lookup(&ctx.provider, &ctx.model);
        Ok(Self {
            requests,
            payloads,
            ctx,
            pricing,
            started_at: Instant::now(),
        })
    }

    pub async fn open(&self, request_body: &Bytes) -> Result<()> {
        let record =
            AiRequestRecord::builder(self.ctx.ai_request_id.as_str(), self.ctx.user_id.clone())
                .provider(self.ctx.provider.clone())
                .model(self.ctx.model.clone())
                .streaming(self.ctx.is_streaming);
        let record = if let Some(t) = &self.ctx.tenant_id {
            record.tenant_id(t.clone())
        } else {
            record
        };
        let record = if let Some(s) = &self.ctx.session_id {
            record.session_id(s.clone())
        } else {
            record
        };
        let record = if let Some(t) = &self.ctx.trace_id {
            record.trace_id(t.clone())
        } else {
            record
        };
        let record = if let Some(mt) = self.ctx.max_tokens {
            record.max_tokens(mt)
        } else {
            record
        };
        let record = record.build()?;

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
        Ok(())
    }

    pub async fn complete(
        &self,
        usage: CapturedUsage,
        tool_calls: Vec<CapturedToolUse>,
        response_body: &Bytes,
    ) -> Result<()> {
        let latency_ms = self.started_at.elapsed().as_millis().min(i32::MAX as u128) as i32;
        let cost =
            pricing::cost_microdollars(self.pricing, usage.input_tokens, usage.output_tokens);

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

        tracing::info!(
            ai_request_id = %self.ctx.ai_request_id,
            user_id = %self.ctx.user_id,
            provider = %self.ctx.provider,
            model = %self.ctx.model,
            input_tokens = usage.input_tokens,
            output_tokens = usage.output_tokens,
            cost_microdollars = cost,
            latency_ms,
            tool_calls = tool_calls.len(),
            "Gateway audit: request completed"
        );
        Ok(())
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

fn slice_payload(bytes: &Bytes) -> (Option<Value>, Option<String>, bool, i32) {
    let len = bytes.len();
    let len_i32 = len.min(i32::MAX as usize) as i32;
    if len <= PAYLOAD_CAP_BYTES {
        serde_json::from_slice::<Value>(bytes).map_or_else(
            |_| {
                let excerpt = String::from_utf8_lossy(bytes).to_string();
                (None, Some(excerpt), false, len_i32)
            },
            |v| (Some(v), None, false, len_i32),
        )
    } else {
        let head_len = EXCERPT_BYTES.min(len);
        let head = String::from_utf8_lossy(&bytes[..head_len]).to_string();
        let tail_start = len.saturating_sub(EXCERPT_BYTES);
        let tail = String::from_utf8_lossy(&bytes[tail_start..]).to_string();
        let excerpt = format!("{head}\n...<truncated {} bytes>...\n{tail}", len - head_len);
        (None, Some(excerpt), true, len_i32)
    }
}

fn truncate_for_tool_input(input: &str) -> String {
    const TOOL_INPUT_CAP: usize = 64 * 1024;
    if input.len() <= TOOL_INPUT_CAP {
        input.to_string()
    } else {
        let head = &input[..TOOL_INPUT_CAP];
        format!(
            "{head}...<truncated {} bytes>",
            input.len() - TOOL_INPUT_CAP
        )
    }
}
