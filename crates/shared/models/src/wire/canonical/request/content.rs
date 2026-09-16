//! Message roles and content parts of a canonical request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;

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
        // JSON: MCP tool-call arguments are the tool's own JSON object.
        input: Value,
        // Why: Gemini requires function-call `thoughtSignature` values replayed verbatim.
        signature: Option<String>,
    },
    ToolResult {
        tool_use_id: String,
        content: Vec<Self>,
        is_error: bool,
        structured_content: Option<Value>,
        // JSON: MCP `_meta` is an open map of vendor-prefixed keys.
        meta: Option<Value>,
    },
    Thinking {
        text: String,
        signature: Option<String>,
        // Why: OpenAI Responses requires the reasoning item ID and encrypted content
        // replayed verbatim for stateless reasoning continuity.
        id: Option<String>,
        encrypted_content: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct CanonicalMessage {
    pub role: Role,
    pub content: Vec<CanonicalContent>,
}
