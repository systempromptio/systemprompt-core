//! Persistence of gateway request lifecycle to the AI-request audit trail.
//!
//! [`GatewayAudit`] opens a record when a request arrives (see the `open`
//! submodule), records the canonical messages and request payload, then closes
//! it on completion with token usage, resolved cost, latency, captured tool
//! calls, and the response payload (see the `complete` submodule) — or marks it
//! failed. [`GatewayRequestContext`] carries the identifiers and routing
//! metadata bound to a single request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod complete;
mod message_text;
mod open;
pub mod payload;

#[cfg(feature = "test-api")]
pub mod test_api {
    pub use super::message_text::flatten_message_content;
}

use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::Result;
use systemprompt_ai::repository::{AiRequestPayloadRepository, AiRequestRepository};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    AiRequestId, ContextId, GatewayConversationId, SessionId, TraceId, UserId,
};

#[derive(Debug, Clone)]
pub struct GatewayRequestContext {
    pub ai_request_id: AiRequestId,
    pub user_id: UserId,
    pub session_id: Option<SessionId>,
    pub context_id: ContextId,
    pub gateway_conversation_id: Option<GatewayConversationId>,
    pub trace_id: Option<TraceId>,
    pub provider: String,
    /// The upstream model the request dispatches to (after route rewrite). The
    /// audit `model` column is opened from this, then overwritten by
    /// `set_served_model` with the model the provider echoes back.
    pub model: String,
    /// The model the client requested on the wire, before route rewrite.
    /// Persisted to `ai_requests.requested_model` so an audit retains both.
    pub requested_model: Option<String>,
    pub max_tokens: Option<u32>,
    pub is_streaming: bool,
    pub wire_protocol: String,
}

#[expect(
    missing_debug_implementations,
    reason = "service type holds repository clients that intentionally do not implement Debug"
)]
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

    pub async fn set_system_prompt_override(&self, descriptor: &str) {
        if let Err(e) = self
            .requests
            .update_system_prompt_override(&self.ctx.ai_request_id, descriptor)
            .await
        {
            tracing::warn!(error = %e, "update_system_prompt_override failed");
        }
    }

    pub async fn set_route_match(&self, descriptor: &str) {
        if let Err(e) = self
            .requests
            .update_route_match(&self.ctx.ai_request_id, descriptor)
            .await
        {
            tracing::warn!(error = %e, "update_route_match failed");
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
