//! Opening a gateway audit record: describe the request row, its payload and
//! the canonical request messages for the admission transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{GatewayAuditError, GatewayAuditResult as Result, ensure};
use bytes::Bytes;
use systemprompt_ai::models::{AiRequestRecord, RequestKind};

use super::GatewayAudit;
use super::admission::PendingAdmission;
use super::message_text::flatten_message_content;
use super::payload::{excerpt_payload, slice_payload, tools_array};
use crate::protocol::canonical::{CanonicalRequest, Role};

impl GatewayAudit {
    fn build_record(&self) -> AiRequestRecord {
        let mut record = AiRequestRecord::builder(
            self.ctx.ai_request_id.clone(),
            self.ctx.user_id.clone(),
            self.ctx.context_id.clone(),
            self.ctx.origin,
        )
        .provider(self.ctx.provider.clone())
        .model(self.ctx.model.clone())
        .streaming(self.ctx.is_streaming)
        .request_kind(RequestKind::classify(self.ctx.max_tokens))
        .attribution(self.ctx.attribution.clone());
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
        ensure(
            self.ctx
                .session_id
                .as_ref()
                .is_some_and(|id| !id.as_str().is_empty())
                && self
                    .ctx
                    .trace_id
                    .as_ref()
                    .is_some_and(|id| !id.as_str().is_empty()),
            "Gateway identity requires an authenticated session and trace",
        )?;
        let record = self.build_record();

        if !self.ctx.context_bound {
            self.context_materializer
                .ensure_context(systemprompt_traits::EnsureContextParams {
                    context_id: &self.ctx.context_id,
                    user_id: &self.ctx.user_id,
                    session_id: self.ctx.session_id.as_ref(),
                    name: "Gateway conversation",
                    kind: "derived",
                })
                .await?;
        }

        let capture = if record.request_kind == RequestKind::Probe {
            excerpt_payload(request_body)
        } else {
            slice_payload(request_body, self.payload_cap_bytes())
        };
        let offered = capture
            .json
            .as_ref()
            .and_then(|body| body.get("tools").filter(|t| t.is_array()).cloned())
            .or_else(|| tools_array(request_body));
        let pending = PendingAdmission::new(record, capture, offered, request_messages(request));
        let mut slot = self
            .admission
            .lock()
            .map_err(|_poisoned| GatewayAuditError::Invariant("admission slot poisoned"))?;
        ensure(slot.is_none(), "Audit already opened")?;
        *slot = Some(pending);
        drop(slot);
        self.ingest_tool_results(request);
        Ok(())
    }
}

fn request_messages(request: &CanonicalRequest) -> Vec<(&'static str, String)> {
    let system = request.system_text().map(|text| ("system", text));
    let turns = request.messages.iter().map(|msg| {
        let role = match msg.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        };
        (role, flatten_message_content(&msg.content))
    });
    system.into_iter().chain(turns).collect()
}
