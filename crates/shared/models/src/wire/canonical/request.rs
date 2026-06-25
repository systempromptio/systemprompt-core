//! The provider-neutral request model the gateway translates to and from.
//!
//! The flattening helpers derive plain-text views and a stable
//! [`GatewayConversationId`] from the leading message.

use crate::gateway_hash::conversation_prefix_hash;
use serde_json::Value;
use systemprompt_identifiers::GatewayConversationId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl Role {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageDetail {
    Auto,
    Low,
    High,
}

impl ImageDetail {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Low => "low",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone)]
pub enum ImageSource {
    Base64 {
        media_type: String,
        data: String,
        detail: Option<ImageDetail>,
    },
    Url {
        url: String,
        detail: Option<ImageDetail>,
    },
}

#[derive(Debug, Clone)]
pub enum CanonicalContent {
    Text(String),
    Image(ImageSource),
    ToolUse {
        id: String,
        name: String,
        input: Value,
        // Gemini attaches an opaque `thoughtSignature` to function-call parts that
        // must be echoed back verbatim on the next turn; this carries it through.
        signature: Option<String>,
    },
    ToolResult {
        tool_use_id: String,
        content: Vec<Self>,
        is_error: bool,
        structured_content: Option<Value>,
        meta: Option<Value>,
    },
    Thinking {
        text: String,
        signature: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct CanonicalMessage {
    pub role: Role,
    pub content: Vec<CanonicalContent>,
}

#[derive(Debug, Clone)]
pub struct CanonicalTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
}

#[derive(Debug, Clone)]
pub enum CanonicalToolChoice {
    Auto,
    Any,
    None,
    Required,
    Tool(String),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ThinkingConfig {
    pub enabled: bool,
    pub budget_tokens: Option<u32>,
}

#[derive(Debug, Clone)]
pub enum ResponseFormat {
    JsonObject,
    JsonSchema {
        name: String,
        schema: Value,
        strict: bool,
    },
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
}

impl ReasoningEffort {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SearchConfig {
    pub max_uses: Option<u32>,
    pub context_size: Option<String>,
    pub urls: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CanonicalRequest {
    pub model: String,
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
    pub metadata: Option<Value>,
    pub response_format: Option<ResponseFormat>,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub search: Option<SearchConfig>,
    pub code_execution: bool,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
}

impl CanonicalRequest {
    pub fn flatten_text(&self) -> String {
        let mut out = String::new();
        if let Some(sys) = &self.system {
            push_with_sep(&mut out, sys);
        }
        for msg in &self.messages {
            for part in &msg.content {
                flatten_part(&mut out, part);
            }
        }
        out
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
}

fn flatten_part(out: &mut String, part: &CanonicalContent) {
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
