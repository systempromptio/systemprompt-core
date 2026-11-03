//! The session pathway's producer: a `session.turn.closed` brain event per
//! completed gateway turn, written to the brain outbox after the audit
//! record closes.
//!
//! Scaffolding, off by default. [`SessionCapture::is_on`] holds only when
//! the profile's `capture.sources` lists `session` and
//! `capture.session.enabled` is `true`; without both, no event is written
//! and nothing here runs. Internal requests (the brain's own model calls)
//! are never captured. Texts are bounded to `capture.session.max_chars`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::Utc;
use systemprompt_database::DbPool;
use systemprompt_extension::brain::DynBrainSink;
use systemprompt_manifest::profile::BrainCaptureProfile;
use systemprompt_models::brain::{BrainEvent, BrainEventKind, SessionTurnClosed};
use systemprompt_models::origin::ClientKind;

use super::GatewayRequestContext;
use crate::protocol::canonical::{CanonicalContent, CanonicalMessage, Role};

/// The sink and profile the gateway writes session turns with.
#[derive(Clone)]
pub struct SessionCapture {
    sink: DynBrainSink,
    capture: BrainCaptureProfile,
    db: DbPool,
}

impl std::fmt::Debug for SessionCapture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionCapture")
            .field("on", &self.is_on())
            .finish_non_exhaustive()
    }
}

/// The two texts of a closed turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnTexts {
    pub user: String,
    pub assistant: String,
}

impl SessionCapture {
    #[must_use]
    pub const fn new(sink: DynBrainSink, capture: BrainCaptureProfile, db: DbPool) -> Self {
        Self { sink, capture, db }
    }

    /// Whether this capture writes anything at all.
    #[must_use]
    pub fn is_on(&self) -> bool {
        self.capture.session_capture_on() && self.sink.is_available()
    }

    /// The event one turn produces, or `None` when the capture is off, the
    /// request is internal or carries no session.
    #[must_use]
    pub fn event_for(&self, ctx: &GatewayRequestContext, texts: &TurnTexts) -> Option<BrainEvent> {
        if !self.is_on() || ctx.origin.client == ClientKind::Internal {
            return None;
        }
        let session_id = ctx.session_id.clone()?;
        let max_chars = self.capture.session.max_chars;
        let payload = SessionTurnClosed {
            user_id: ctx.user_id.clone(),
            session_id,
            ai_request_id: ctx.ai_request_id.clone(),
            gateway_conversation_id: ctx.gateway_conversation_id.clone(),
            context_id: Some(ctx.context_id.clone()),
            user_text: SessionTurnClosed::bounded(&texts.user, max_chars),
            assistant_text: SessionTurnClosed::bounded(&texts.assistant, max_chars),
            model: ctx.model.clone(),
            client: ctx.origin.client.as_str().to_owned(),
        };
        let payload = serde_json::to_value(payload).ok()?;
        Some(
            BrainEvent::new(
                BrainEventKind::session_turn_closed(),
                ctx.user_id.clone(),
                Utc::now(),
            )
            .with_payload(payload),
        )
    }

    /// Writes the event on its own transaction.
    pub async fn emit(&self, event: &BrainEvent) -> Result<(), String> {
        let mut tx = self
            .db
            .write_pool()
            .begin()
            .await
            .map_err(|e| e.to_string())?;
        self.sink
            .emit(&mut tx, event)
            .await
            .map_err(|e| e.to_string())?;
        tx.commit().await.map_err(|e| e.to_string())
    }
}

/// The text of the last user message, as recall's subject hints read it.
#[must_use]
pub fn last_user_text(messages: &[CanonicalMessage]) -> String {
    messages
        .iter()
        .rev()
        .find(|message| message.role == Role::User)
        .map(|message| {
            message
                .content
                .iter()
                .filter_map(|part| match part {
                    CanonicalContent::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

pub type SharedSessionCapture = Option<Arc<SessionCapture>>;
