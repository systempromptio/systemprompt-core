//! The gateway-facing recall seam: the request a gateway stage builds, the
//! outcome it acts on and the receipt it records after admission.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{
    AiRequestId, BrainRecallId, ClientId, ContextId, GatewayConversationId, SessionId, UserId,
    WorkflowId,
};
use systemprompt_models::auth::RequestActor;
use systemprompt_models::origin::ClientKind;

use super::errors::RecallError;
use super::render::RecallBlock;

pub const MAX_SUBJECT_HINTS: usize = 2000;

/// Whether the request opens a new user turn or continues one (a tool
/// result round trip), which reuses the turn's stored block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReuseHint {
    NewTurn,
    SameTurn,
}

/// Who the recall is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecallPrincipal {
    pub user_id: UserId,
    pub client: ClientKind,
    pub actor: RequestActor,
    pub project: Option<String>,
    pub client_id: Option<ClientId>,
}

/// The conversation the request belongs to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_field_names,
    reason = "each field is named for the identifier type it carries"
)]
pub struct RecallConversation {
    pub ai_request_id: Option<AiRequestId>,
    pub gateway_conversation_id: Option<GatewayConversationId>,
    pub session_id: Option<SessionId>,
    pub context_id: Option<ContextId>,
}

/// One recall for one gateway request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecallRequest {
    pub principal: RecallPrincipal,
    pub conversation: RecallConversation,
    pub subject_hints: String,
    pub budget_tokens: u32,
    pub deadline: Duration,
    pub reuse: ReuseHint,
}

impl RecallRequest {
    #[must_use]
    pub const fn new(principal: RecallPrincipal, conversation: RecallConversation) -> Self {
        Self {
            principal,
            conversation,
            subject_hints: String::new(),
            budget_tokens: 0,
            deadline: Duration::ZERO,
            reuse: ReuseHint::NewTurn,
        }
    }

    #[must_use]
    pub fn with_subject_hints(mut self, hints: &str) -> Self {
        self.subject_hints = hints.chars().take(MAX_SUBJECT_HINTS).collect();
        self
    }

    #[must_use]
    pub const fn with_budget_tokens(mut self, budget_tokens: u32) -> Self {
        self.budget_tokens = budget_tokens;
        self
    }

    #[must_use]
    pub const fn with_deadline(mut self, deadline: Duration) -> Self {
        self.deadline = deadline;
        self
    }

    #[must_use]
    pub const fn with_reuse(mut self, reuse: ReuseHint) -> Self {
        self.reuse = reuse;
        self
    }
}

/// Why nothing was injected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkipReason {
    Disabled,
    ClientNotEnabled,
    HeaderOff,
    NotNewTurn,
    NoFacts,
    Deadline,
    Unavailable,
}

impl SkipReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::ClientNotEnabled => "client_not_enabled",
            Self::HeaderOff => "header_off",
            Self::NotNewTurn => "not_new_turn",
            Self::NoFacts => "no_facts",
            Self::Deadline => "deadline",
            Self::Unavailable => "unavailable",
        }
    }

    #[must_use]
    pub fn descriptor(self) -> String {
        format!("skipped={}", self.as_str())
    }
}

/// What a recall produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecallOutcome {
    Injected(RecallBlock),
    Skipped { reason: SkipReason },
}

impl RecallOutcome {
    #[must_use]
    pub fn descriptor(&self) -> String {
        match self {
            Self::Injected(block) => block.descriptor(),
            Self::Skipped { reason } => reason.descriptor(),
        }
    }
}

/// The record of one recall, written after the request is admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecallReceipt {
    pub recall_id: BrainRecallId,
    pub ai_request_id: AiRequestId,
    pub principal: RecallPrincipal,
    pub conversation: RecallConversation,
    pub outcome: RecallOutcome,
    pub workflow: Option<WorkflowId>,
    pub recorded_at: DateTime<Utc>,
}

impl RecallReceipt {
    #[must_use]
    pub fn new(
        request: &RecallRequest,
        ai_request_id: AiRequestId,
        outcome: RecallOutcome,
    ) -> Self {
        Self {
            recall_id: BrainRecallId::generate(),
            ai_request_id,
            principal: request.principal.clone(),
            conversation: request.conversation.clone(),
            outcome,
            workflow: None,
            recorded_at: Utc::now(),
        }
    }
}

/// What the gateway saw come back for a request whose recall injected
/// facts: the assistant's text, buffered or assembled from the stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecallObservation {
    pub ai_request_id: AiRequestId,
    pub assistant_text: String,
}

/// Recalls team memory for a gateway request and records what it did;
/// attached to the gateway engines as `Arc<dyn MemoryRecall>`.
///
/// `recall` runs under the stage's deadline, before the upstream body is
/// built; `record` runs after admission, when the request row exists;
/// `observe` runs after the response completes and by default observes
/// nothing.
///
/// `#[async_trait]` keeps the trait dyn-compatible.
#[async_trait::async_trait]
pub trait MemoryRecall: Send + Sync {
    fn name(&self) -> &'static str;

    async fn recall(&self, request: &RecallRequest) -> Result<RecallOutcome, RecallError>;

    async fn record(&self, receipt: &RecallReceipt) -> Result<(), RecallError>;

    async fn observe(&self, _observation: &RecallObservation) -> Result<(), RecallError> {
        Ok(())
    }
}
