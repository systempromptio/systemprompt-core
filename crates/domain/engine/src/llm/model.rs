//! The neutral request model both wire protocols are shaped from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The wire protocol a model is served on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    Messages,
    ChatCompletions,
}

/// What a model accepts beyond plain text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Supports {
    #[serde(default)]
    pub images: bool,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub tool_calls: bool,
    #[serde(default)]
    pub caching: bool,
}

/// A model on offer, as the governing instance describes it. Token counts of 0
/// mean unknown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSpec {
    pub id: String,
    pub name: String,
    pub protocol: Protocol,
    #[serde(default)]
    pub context_window: u32,
    #[serde(default)]
    pub max_output: u32,
    #[serde(default)]
    pub supports: Supports,
}

/// A named reasoning level, for models that take an effort rather than a token
/// budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
    Max,
}

impl ReasoningEffort {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Max => "max",
        }
    }
}

/// How much the model may think before answering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[expect(
    variant_size_differences,
    reason = "the larger variant is a plain u32; boxing four bytes to save three costs an allocation"
)]
pub enum Reasoning {
    Effort(ReasoningEffort),
    Budget(u32),
}

/// Whether, and which, tool the model must call.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    #[default]
    Auto,
    None,
    Required,
    Tool(String),
}

/// Sampling and output parameters of one call.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Params {
    #[serde(default)]
    pub max_output: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub reasoning: Option<Reasoning>,
    #[serde(default)]
    pub tool_choice: ToolChoice,
}

/// A tool offered to the model; `input_schema` is a JSON Schema object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Who wrote a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
}

/// A file carried inside a tool result; `data` is base64.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub mime: String,
    pub data: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// One block of a message. Binary payloads (`data`) are base64.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    Text {
        text: String,
    },
    Image {
        mime: String,
        data: String,
    },
    Document {
        mime: String,
        data: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    ToolCall {
        id: String,
        name: String,
        // JSON: tool arguments as the model wrote them for the tool's input schema.
        input: Value,
    },
    ToolResult {
        id: String,
        output: String,
        #[serde(default)]
        is_error: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        attachments: Vec<Attachment>,
    },
    Reasoning {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    RedactedReasoning {
        data: String,
    },
}

impl Content {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }
}

/// One turn of the conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<Content>,
}

impl Message {
    pub const fn user(content: Vec<Content>) -> Self {
        Self {
            role: Role::User,
            content,
        }
    }

    pub const fn assistant(content: Vec<Content>) -> Self {
        Self {
            role: Role::Assistant,
            content,
        }
    }

    pub fn user_text(text: impl Into<String>) -> Self {
        Self::user(vec![Content::text(text)])
    }
}

/// One model call in protocol-neutral form. `metadata` is sent as the body's
/// `metadata` object when not empty.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub model: ModelSpec,
    pub system: Vec<String>,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDef>,
    pub params: Params,
    // JSON: caller-owned key/values sent as the provider body's `metadata` object.
    pub metadata: Map<String, Value>,
}

impl Request {
    pub fn new(model: ModelSpec) -> Self {
        Self {
            model,
            system: Vec::new(),
            messages: Vec::new(),
            tools: Vec::new(),
            params: Params::default(),
            metadata: Map::new(),
        }
    }
}
