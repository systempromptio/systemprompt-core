//! Opening a gateway audit record: insert the request row, its payload, and the
//! canonical request messages.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use bytes::Bytes;
use systemprompt_ai::models::ai_request_record::AiRequestRecord;
use systemprompt_ai::repository::UpsertPayloadParams;

use super::GatewayAudit;
use super::message_text::flatten_message_content;
use super::payload::slice_payload;
use crate::services::gateway::protocol::canonical::{CanonicalRequest, Role};

impl GatewayAudit {
    fn build_record(&self) -> Result<AiRequestRecord> {
        let mut record =
            AiRequestRecord::builder(self.ctx.ai_request_id.clone(), self.ctx.user_id.clone())
                .provider(self.ctx.provider.clone())
                .model(self.ctx.model.clone())
                .streaming(self.ctx.is_streaming);
        if let Some(s) = &self.ctx.session_id {
            record = record.session_id(s.clone());
        }
        if let Some(rm) = &self.ctx.requested_model {
            record = record.requested_model(rm.clone());
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
        if let Some(system) = &request.system
            && !system.is_empty()
        {
            if let Err(e) = self
                .requests
                .insert_message(&self.ctx.ai_request_id, "system", system, seq)
                .await
            {
                tracing::warn!(error = %e, "insert system message failed");
            }
            seq += 1;
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
}
