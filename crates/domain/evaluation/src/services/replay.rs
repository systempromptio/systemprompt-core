//! Replay of stored evaluation cases against the live AI provider.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::ai::{AiMessage, AiRequest, AiResponse, DynAiProvider, MessageRole};

use crate::error::{EvaluationError, Result};
use crate::models::{CanonicalMessage, CanonicalPrompt};

const REPLAY_ACTOR_JOB: &str = "evaluation_replay";
const REPLAY_AGENT: &str = "evaluation-replay";
const REPLAY_MAX_OUTPUT_TOKENS: u32 = 8192;

#[derive(Clone)]
pub struct ReplayService {
    ai: DynAiProvider,
    created_by: UserId,
}

impl std::fmt::Debug for ReplayService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReplayService")
            .field("created_by", &self.created_by)
            .finish_non_exhaustive()
    }
}

impl ReplayService {
    pub const fn new(ai: DynAiProvider, created_by: UserId) -> Self {
        Self { ai, created_by }
    }

    /// Re-executes a canonical prompt with the repair hint injected ahead of
    /// the final user turn, so the model sees the corrective instruction in
    /// the position a system hint would occupy at serve time.
    pub async fn replay(&self, prompt: &CanonicalPrompt, repair_hint: &str) -> Result<AiResponse> {
        let messages = build_messages(&prompt.messages, repair_hint)?;
        let context = RequestContext::new(
            SessionId::generate(),
            TraceId::generate(),
            ContextId::generate(),
            AgentName::new(REPLAY_AGENT),
        )
        .with_actor(Actor::job(self.created_by.clone(), REPLAY_ACTOR_JOB));

        let mut builder = AiRequest::builder(
            messages,
            prompt.provider.clone(),
            prompt.model.clone(),
            REPLAY_MAX_OUTPUT_TOKENS,
            context,
        );
        if let Some(system) = &prompt.system_prompt {
            builder = builder.with_system_prompt(system.clone());
        }
        self.ai
            .generate(&builder.build())
            .await
            .map_err(|e| EvaluationError::Ai(e.to_string()))
    }
}

fn build_messages(canonical: &[CanonicalMessage], repair_hint: &str) -> Result<Vec<AiMessage>> {
    if canonical.is_empty() {
        return Err(EvaluationError::ReplaySource(
            "canonical prompt has no messages".to_owned(),
        ));
    }
    let last_user = canonical
        .iter()
        .rposition(|m| m.role == "user")
        .ok_or_else(|| {
            EvaluationError::ReplaySource("canonical prompt has no user turn".to_owned())
        })?;

    let mut messages = Vec::with_capacity(canonical.len() + 1);
    for (idx, message) in canonical.iter().enumerate() {
        if idx == last_user {
            messages.push(AiMessage::system(format!(
                "Apply this correction when answering the next user message: {repair_hint}"
            )));
        }
        messages.push(to_ai_message(message));
    }
    Ok(messages)
}

fn to_ai_message(message: &CanonicalMessage) -> AiMessage {
    let role = match message.role.as_str() {
        "system" => MessageRole::System,
        "assistant" => MessageRole::Assistant,
        _ => MessageRole::User,
    };
    AiMessage {
        role,
        content: message.content.clone(),
        parts: Vec::new(),
    }
}
