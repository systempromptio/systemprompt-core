//! The provider-neutral request model the gateway translates to and from.
//!
//! The flattening helpers derive plain-text views and a stable
//! [`GatewayConversationId`] from the leading message.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod content;
mod options;

pub use content::{CanonicalContent, CanonicalMessage, ImageDetail, ImageSource, Role};
pub use options::{
    CanonicalTool, CanonicalToolChoice, ReasoningEffort, ResponseFormat, SearchConfig,
    ThinkingConfig,
};

use crate::gateway_hash::conversation_prefix_hash;
use crate::wire::inspect::ForwardedSurface;
use serde_json::Value;
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_identifiers::{ClientSessionId, GatewayConversationId, ModelId};

#[derive(Debug, Clone)]
pub struct CanonicalRequest {
    pub model: ModelId,
    pub system: Option<String>,
    pub messages: Vec<CanonicalMessage>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub stop_sequences: Vec<String>,
    pub tools: Vec<CanonicalTool>,
    pub tool_choice: Option<CanonicalToolChoice>,
    pub stream: bool,
    pub thinking: Option<ThinkingConfig>,
    // JSON: Free-form request metadata mirrored to `ai_requests.metadata` (JSONB).
    pub metadata: Option<Value>,
    pub response_format: Option<ResponseFormat>,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub search: Option<SearchConfig>,
    pub code_execution: bool,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub forwarded_surface: ForwardedSurface,
}

impl CanonicalRequest {
    #[must_use]
    pub fn new(model: ModelId, messages: Vec<CanonicalMessage>, max_tokens: u32) -> Self {
        Self {
            model,
            system: None,
            messages,
            max_tokens,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: Vec::new(),
            tools: Vec::new(),
            tool_choice: None,
            stream: false,
            thinking: None,
            metadata: None,
            response_format: None,
            reasoning_effort: None,
            search: None,
            code_execution: false,
            presence_penalty: None,
            frequency_penalty: None,
            forwarded_surface: ForwardedSurface::default(),
        }
    }

    pub fn flatten_parts(&self) -> Vec<(String, String)> {
        let mut parts = Vec::with_capacity(self.messages.len() + self.forwarded_surface.len() + 1);
        if let Some(sys) = &self.system
            && !sys.is_empty()
        {
            parts.push(("system".to_owned(), sys.clone()));
        }
        for (index, msg) in self.messages.iter().enumerate() {
            let mut out = String::new();
            for part in &msg.content {
                flatten_part(&mut out, part);
            }
            if !out.is_empty() {
                parts.push((format!("messages[{index}].{}", msg.role.as_str()), out));
            }
        }
        for leaf in self.forwarded_surface.leaves() {
            parts.push((format!("forwarded.{}", leaf.path), leaf.value.clone()));
        }
        parts
    }

    pub fn derived_gateway_conversation_id(&self) -> Option<GatewayConversationId> {
        let first = self.messages.first()?;
        let mut content = String::new();
        for part in &first.content {
            flatten_part(&mut content, part);
        }
        let hash = conversation_prefix_hash(self.system.as_deref(), first.role.as_str(), &content);
        Some(GatewayConversationId::from_prefix_hash(hash))
    }

    // Why: the caller's own session travels inside `metadata.user_id`; it is
    // read here, before the identity is stripped for the upstream.
    pub fn client_session_id(&self) -> Result<Option<ClientSessionId>, IdValidationError> {
        let Some(value) = self.metadata.as_ref().and_then(|m| m.get("user_id")) else {
            return Ok(None);
        };
        let value = value.as_str().ok_or_else(|| IdValidationError::Invalid {
            id_type: "ClientSessionId",
            message: "metadata.user_id must be a string".to_owned(),
        })?;
        ClientSessionId::from_metadata_user_id(value)
    }

    pub fn flatten_message_text(&self, role: Role) -> Option<String> {
        let mut out = String::new();
        for msg in &self.messages {
            if msg.role != role {
                continue;
            }
            for part in &msg.content {
                flatten_part(&mut out, part);
            }
        }
        if out.is_empty() { None } else { Some(out) }
    }

    pub fn latest_message_text(&self, role: Role) -> Option<String> {
        let msg = self.messages.iter().rev().find(|m| m.role == role)?;
        let mut out = String::new();
        for part in &msg.content {
            flatten_part(&mut out, part);
        }
        if out.is_empty() { None } else { Some(out) }
    }

    pub fn message_units(&self) -> Vec<String> {
        let mut units = Vec::with_capacity(self.messages.len() + self.forwarded_surface.len() + 1);
        if let Some(sys) = &self.system {
            units.push(sys.clone());
        }
        for msg in &self.messages {
            let mut out = String::new();
            for part in &msg.content {
                flatten_part(&mut out, part);
            }
            if !out.is_empty() {
                units.push(out);
            }
        }
        for leaf in self.forwarded_surface.leaves() {
            units.push(leaf.value.clone());
        }
        units
    }
}

pub(super) fn flatten_part(out: &mut String, part: &CanonicalContent) {
    match part {
        CanonicalContent::Text(t) => push_with_sep(out, t),
        CanonicalContent::Thinking { text, .. } => push_with_sep(out, text),
        CanonicalContent::ToolUse { name, input, .. } => {
            push_with_sep(out, &format!("[tool_use:{name} {input}]"));
        },
        CanonicalContent::ToolResult { content, .. } => {
            for inner in content {
                flatten_part(out, inner);
            }
        },
        CanonicalContent::Image(_) => {},
    }
}

fn push_with_sep(out: &mut String, fragment: &str) {
    if fragment.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(fragment);
}
