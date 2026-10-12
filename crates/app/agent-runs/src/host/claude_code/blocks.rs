//! The wire shapes of one stream-json line: the line envelope, message
//! content blocks and tool-result content.
//!
//! Every enum ends in `#[serde(other)]` or an untagged fallback so a shape a
//! newer Claude Code adds decodes as "ignored" instead of failing the line.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Line {
    System(SystemLine),
    Assistant {
        #[serde(default)]
        message: Message,
        #[serde(default, rename = "session_id")]
        raw_session: Option<String>,
    },
    User {
        #[serde(default)]
        message: Message,
        #[serde(default, rename = "session_id")]
        raw_session: Option<String>,
    },
    Result(ResultLine),
    #[serde(other)]
    Other,
}

#[derive(Debug, Default, Deserialize)]
pub struct SystemLine {
    #[serde(default)]
    pub subtype: String,
    #[serde(default, rename = "session_id")]
    pub raw_session: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub hook_name: Option<String>,
    #[serde(default)]
    pub hook_event: Option<String>,
    #[serde(default)]
    pub outcome: Option<String>,
    #[serde(default)]
    pub output: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[expect(
    clippy::struct_field_names,
    reason = "field names mirror the Anthropic usage wire object"
)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: i64,
    #[serde(default)]
    pub output_tokens: i64,
    #[serde(default)]
    pub cache_read_input_tokens: i64,
    #[serde(default)]
    pub cache_creation_input_tokens: i64,
}

// JSON: protocol boundary — `structured_output` is shaped by the caller's
// `RunRequest::output_schema`.
#[derive(Debug, Default, Deserialize)]
pub struct ResultLine {
    #[serde(default)]
    pub subtype: String,
    #[serde(default)]
    pub is_error: bool,
    #[serde(default)]
    pub num_turns: i64,
    #[serde(default)]
    pub duration_ms: i64,
    #[serde(default)]
    pub total_cost_usd: f64,
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default, rename = "session_id")]
    pub raw_session: Option<String>,
    #[serde(default)]
    pub usage: Usage,
    // JSON: the caller's output contract.
    #[serde(default)]
    pub structured_output: Option<serde_json::Value>,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Message {
    #[serde(default)]
    pub content: MessageContent,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Blocks(Vec<Block>),
    Text(serde::de::IgnoredAny),
}

impl Default for MessageContent {
    fn default() -> Self {
        Self::Blocks(Vec::new())
    }
}

// JSON: protocol boundary — `tool_use.input` is tool-defined.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Text {
        #[serde(default)]
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        // JSON: the tool's own input schema.
        #[serde(default)]
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        #[serde(default)]
        content: ToolResultContent,
        #[serde(default)]
        is_error: Option<bool>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Default, Deserialize)]
#[serde(untagged)]
pub enum ToolResultContent {
    Text(String),
    Parts(Vec<TextPart>),
    #[default]
    Missing,
}

#[derive(Debug, Deserialize)]
pub struct TextPart {
    #[serde(default)]
    pub text: Option<String>,
}

impl ToolResultContent {
    #[must_use]
    pub fn flatten(self) -> String {
        match self {
            Self::Text(text) => text,
            Self::Parts(parts) => parts
                .into_iter()
                .filter_map(|part| part.text)
                .collect::<Vec<_>>()
                .join("\n"),
            Self::Missing => String::new(),
        }
    }
}
