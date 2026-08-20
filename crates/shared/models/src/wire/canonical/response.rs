//! The provider-neutral response and streaming-event model.
//!
//! Outbound adapters parse a buffered upstream reply into a
//! [`CanonicalResponse`] or map upstream SSE bytes to a stream of
//! [`CanonicalEvent`]s. Stop reasons are normalised here, with per-dialect
//! string mappings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::request::{CanonicalContent, flatten_part};
use crate::wire::inspect::ForwardedSurface;

#[derive(Debug, Clone, Copy, Default)]
#[expect(
    clippy::struct_field_names,
    reason = "every field is a token count; the `_tokens` suffix is the domain vocabulary shared \
              with the provider usage wire formats"
)]
pub struct CanonicalUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_creation_tokens: u32,
    pub total_tokens: u32,
}

/// A streaming usage report, carrying only the counts its frame actually
/// stated.
///
/// [`CanonicalUsage`] cannot express this: an unreported count and a reported
/// zero are both `0`. Providers differ in what a mid-stream usage frame
/// includes — an Anthropic `message_delta` may carry `output_tokens` alone —
/// so folding one in as though it were complete zeroes the input and cache
/// counts an earlier frame established, and billing loses them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_field_names,
    reason = "every field is a token count; the `_tokens` suffix is the domain vocabulary shared \
              with the provider usage wire formats"
)]
pub struct CanonicalUsageUpdate {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,
    pub cache_creation_tokens: Option<u32>,
}

impl CanonicalUsageUpdate {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.cache_read_tokens.is_none()
            && self.cache_creation_tokens.is_none()
    }

    pub const fn apply_to(&self, usage: &mut CanonicalUsage) {
        if let Some(v) = self.input_tokens {
            usage.input_tokens = v;
        }
        if let Some(v) = self.output_tokens {
            usage.output_tokens = v;
        }
        if let Some(v) = self.cache_read_tokens {
            usage.cache_read_tokens = v;
        }
        if let Some(v) = self.cache_creation_tokens {
            usage.cache_creation_tokens = v;
        }
        usage.total_tokens = usage.input_tokens
            + usage.output_tokens
            + usage.cache_read_tokens
            + usage.cache_creation_tokens;
    }
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

#[derive(Debug, Clone, Default)]
pub struct GroundedSource {
    pub uri: String,
    pub title: Option<String>,
    pub snippet: Option<String>,
    pub relevance: Option<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct Grounding {
    pub sources: Vec<GroundedSource>,
    pub queries: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CodeExecutionOutput {
    pub language: Option<String>,
    pub code: String,
    pub result: Option<String>,
    pub outcome: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CanonicalResponse {
    pub id: String,
    pub model: String,
    pub content: Vec<CanonicalContent>,
    pub stop_reason: Option<CanonicalStopReason>,
    pub usage: CanonicalUsage,
    pub grounding: Option<Grounding>,
    pub code_execution: Option<CodeExecutionOutput>,
    pub raw_finish_reason: Option<String>,
    pub received_surface: ForwardedSurface,
}

impl CanonicalResponse {
    pub fn content_units(&self) -> Vec<String> {
        let mut units = Vec::with_capacity(self.content.len() + self.received_surface.len());
        for part in &self.content {
            let mut out = String::new();
            flatten_part(&mut out, part);
            if !out.is_empty() {
                units.push(out);
            }
        }
        for leaf in self.received_surface.leaves() {
            units.push(leaf.value.clone());
        }
        units
    }
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
    SignatureDelta {
        index: u32,
        signature: String,
    },
    EncryptedContentDelta {
        index: u32,
        data: String,
    },
    ToolUseDelta {
        index: u32,
        partial_json: String,
    },
    ContentBlockStop {
        index: u32,
    },
    UsageDelta(CanonicalUsageUpdate),
    MessageStop {
        id: String,
        stop_reason: Option<CanonicalStopReason>,
    },
    Error(String),
}

#[derive(Debug, Clone)]
pub enum ContentBlockKind {
    Text,
    Thinking {
        id: Option<String>,
        signature: Option<String>,
    },
    ToolUse {
        id: String,
        name: String,
        signature: Option<String>,
    },
}
