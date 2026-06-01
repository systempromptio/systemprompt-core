//! The provider-neutral response and streaming-event model.
//!
//! Outbound adapters parse a buffered upstream reply into a
//! [`CanonicalResponse`] or map upstream SSE bytes to a stream of
//! [`CanonicalEvent`]s. Stop reasons are normalised here, with per-dialect
//! string mappings.

use super::request::CanonicalContent;

#[derive(Debug, Clone, Copy, Default)]
pub struct CanonicalUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_creation_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalStopReason {
    EndTurn,
    MaxTokens,
    StopSequence,
    ToolUse,
    Other,
}

impl CanonicalStopReason {
    pub const fn anthropic_str(self) -> &'static str {
        match self {
            Self::MaxTokens => "max_tokens",
            Self::StopSequence => "stop_sequence",
            Self::ToolUse => "tool_use",
            Self::EndTurn | Self::Other => "end_turn",
        }
    }

    pub const fn openai_str(self) -> &'static str {
        match self {
            Self::MaxTokens => "length",
            Self::ToolUse => "tool_calls",
            Self::EndTurn | Self::StopSequence | Self::Other => "stop",
        }
    }

    pub fn from_anthropic(s: &str) -> Self {
        match s {
            "end_turn" => Self::EndTurn,
            "max_tokens" => Self::MaxTokens,
            "stop_sequence" => Self::StopSequence,
            "tool_use" => Self::ToolUse,
            _ => Self::Other,
        }
    }

    pub fn from_openai(s: &str) -> Self {
        match s {
            "stop" => Self::EndTurn,
            "length" => Self::MaxTokens,
            "tool_calls" | "function_call" => Self::ToolUse,
            _ => Self::Other,
        }
    }
}

/// One grounded source surfaced by a server-side search / citation pass.
#[derive(Debug, Clone, Default)]
pub struct GroundedSource {
    pub uri: String,
    pub title: Option<String>,
    pub snippet: Option<String>,
    pub relevance: Option<f32>,
}

/// Search-grounding evidence attached to a response: the queries the model ran
/// and the sources it cited.
#[derive(Debug, Clone, Default)]
pub struct Grounding {
    pub sources: Vec<GroundedSource>,
    pub queries: Vec<String>,
}

/// Output of a server-side code-execution turn. `outcome` is the dialect's raw
/// status string (e.g. Gemini's `OUTCOME_OK`); consumers derive success from
/// it.
#[derive(Debug, Clone, Default)]
pub struct CodeExecutionOutput {
    pub language: Option<String>,
    pub code: String,
    pub result: Option<String>,
    pub outcome: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CanonicalResponse {
    pub id: String,
    pub model: String,
    pub content: Vec<CanonicalContent>,
    pub stop_reason: Option<CanonicalStopReason>,
    pub usage: CanonicalUsage,
    pub grounding: Option<Grounding>,
    pub code_execution: Option<CodeExecutionOutput>,
    pub raw_finish_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub enum CanonicalEvent {
    MessageStart {
        id: String,
        model: String,
        usage: CanonicalUsage,
    },
    ContentBlockStart {
        index: u32,
        block: ContentBlockKind,
    },
    TextDelta {
        index: u32,
        text: String,
    },
    ThinkingDelta {
        index: u32,
        text: String,
    },
    ToolUseDelta {
        index: u32,
        partial_json: String,
    },
    ContentBlockStop {
        index: u32,
    },
    UsageDelta(CanonicalUsage),
    MessageStop {
        id: String,
        stop_reason: Option<CanonicalStopReason>,
    },
    Error(String),
}

#[derive(Debug, Clone)]
pub enum ContentBlockKind {
    Text,
    Thinking { signature: Option<String> },
    ToolUse { id: String, name: String },
}
