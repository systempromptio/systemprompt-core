//! Opening a gateway audit record: insert the request row, its payload, and the
//! canonical request messages.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use bytes::Bytes;
use systemprompt_ai::models::{AiRequestRecord, RequestKind};
use systemprompt_ai::repository::UpsertPayloadParams;

use super::GatewayAudit;
use super::message_text::flatten_message_content;
use super::payload::slice_payload;
use crate::services::gateway::protocol::canonical::{CanonicalRequest, Role};

impl GatewayAudit {
    fn build_record(&self) -> AiRequestRecord {
        let mut record = AiRequestRecord::builder(
            self.ctx.ai_request_id.clone(),
            self.ctx.user_id.clone(),
            self.ctx.context_id.clone(),
        )
        .provider(self.ctx.provider.clone())
        .model(self.ctx.model.clone())
        .streaming(self.ctx.is_streaming)
        .request_kind(RequestKind::classify(self.ctx.max_tokens));
        if let Some(instance_id) = systemprompt_logging::instance_id() {
            record = record.instance_id(instance_id.clone());
        }
        if let Some(s) = &self.ctx.session_id {
            record = record.session_id(s.clone());
        }
        if let Some(rm) = &self.ctx.requested_model {
            record = record.requested_model(rm.clone());
        }
        if let Some(g) = &self.ctx.gateway_conversation_id {
            record = record.gateway_conversation_id(g.clone());
        }
        if let Some(cs) = &self.ctx.client_session_id {
            record = record.client_session_id(cs.clone());
        }
        if let Some(t) = &self.ctx.trace_id {
            record = record.trace_id(t.clone());
        }
        if let Some(mt) = self.ctx.max_tokens {
            record = record.max_tokens(mt);
        }
        record.build()
    }

    pub async fn open(&self, request: &CanonicalRequest, request_body: &Bytes) -> Result<()> {
        anyhow::ensure!(
            self.ctx
                .session_id
                .as_ref()
                .is_some_and(|id| !id.as_str().is_empty())
                && self
                    .ctx
                    .trace_id
                    .as_ref()
                    .is_some_and(|id| !id.as_str().is_empty()),
            "Gateway identity requires an authenticated session and trace"
        );
        let mut record = self.build_record();
        if let Some(session) = &self.ctx.session_id
            && let Some(actor) = self
                .evaluations
                .execution_actor(&self.ctx.user_id, session)
                .await?
        {
            record.actor = actor;
        }

        self.context_materializer
            .ensure_context(systemprompt_traits::EnsureContextParams {
                context_id: &self.ctx.context_id,
                user_id: &self.ctx.user_id,
                session_id: self.ctx.session_id.as_ref(),
                name: "Gateway conversation",
                kind: "derived",
            })
            .await?;

        self.requests
            .insert_with_id(&self.ctx.ai_request_id, &record)
            .await?;

        let capture = slice_payload(request_body);
        self.payloads
            .upsert_request(
                &self.ctx.ai_request_id,
                UpsertPayloadParams {
                    body: capture.json.as_ref(),
                    excerpt: capture.excerpt.as_deref(),
                    truncated: capture.truncated,
                    bytes: Some(capture.byte_len),
                    sha256: Some(&capture.sha256),
                },
            )
            .await?;
        if let Some(tools) = capture.json.as_ref().and_then(|body| body.get("tools")) {
            self.payloads
                .upsert_offered_tools(&self.ctx.ai_request_id, tools)
                .await?;
        }
        self.persist_request_messages(request).await?;
        let lease = super::journal::reserve(
            super::journal::Receipt::pending(
                self.ctx.ai_request_id.clone(),
                self.ctx.user_id.clone(),
            ),
            &self.audit_pool,
        )
        .await?;
        self.journal_lease
            .set(lease)
            .map_err(|_existing_lease| anyhow::anyhow!("Audit already admitted"))?;
        Ok(())
    }

    async fn persist_request_messages(&self, request: &CanonicalRequest) -> Result<()> {
        let mut seq = 0i32;
        if let Some(system) = &request.system
            && !system.is_empty()
        {
            self.requests
                .insert_message(&self.ctx.ai_request_id, "system", system, seq)
                .await?;
            seq += 1;
        }
        for msg in &request.messages {
            let role = match msg.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::Tool => "tool",
            };
            self.requests
                .insert_message(
                    &self.ctx.ai_request_id,
                    role,
                    &flatten_message_content(&msg.content),
                    seq,
                )
                .await?;
            seq += 1;
        }
        Ok(())
    }
}
