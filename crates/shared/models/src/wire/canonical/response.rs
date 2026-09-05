//! The provider-neutral response and streaming-event model.
//!
//! Outbound adapters parse a buffered upstream reply into a
//! [`CanonicalResponse`] or map upstream SSE bytes to a stream of
//! [`CanonicalEvent`]s. Stop reasons are normalised here, with per-dialect
//! string mappings.
//!
//! # Reasoning tokens
//!
//! `CanonicalUsage::reasoning_tokens` is a **breakdown of** `output_tokens`,
//! never an addition to it. Providers disagree on the wire, so every adapter
//! normalises to that one rule before the count reaches billing:
//!
//! * OpenAI (chat and responses) already folds
//!   `*_tokens_details.reasoning_tokens` into `completion_tokens` /
//!   `output_tokens`, so the adapter copies it across untouched.
//! * Gemini reports `thoughtsTokenCount` *beside* `candidatesTokenCount` (and
//!   inside `totalTokenCount`), so its adapter adds it into `output_tokens` on
//!   the way in.
//! * Anthropic bills extended thinking as ordinary output tokens and reports no
//!   separate count, so it stays 0 there.
//!
//! Holding that invariant here is what makes reasoning billable: cost is
//! computed from `output_tokens`, so a reasoning-only turn is charged at the
//! output rate with no per-provider arithmetic downstream, and no count is
//! charged twice. It is also why `reasoning_tokens` is absent from the
//! `total_tokens` sum in [`CanonicalUsageUpdate::apply_to`].
//!
//! Third-party `OpenAI`-compatible upstreams (Cerebras, Moonshot, Qwen) are not
//! probed, so `CanonicalUsage::normalise_reasoning` enforces the rule at
//! runtime rather than trusting it: a breakdown cannot exceed its parent, and a
//! wire `total_tokens` that overshoots `input + output` by exactly the
//! reasoning count is the same signal. Either one means the provider reported
//! reasoning *additionally*, so the count is folded into `output_tokens` and
//! warned about. The total-based half only fires on the buffered path — the
//! streaming accumulator recomputes `total_tokens` itself in
//! [`CanonicalUsageUpdate::apply_to`], discarding the wire's own figure, so a
//! stream is covered by the `reasoning > output` half alone -- a recomputed
//! total is explicitly excluded, because it coincides with the additive shape
//! whenever the cache counts happen to equal the reasoning count.
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

    // Why: a breakdown of output_tokens, not an addition -- see the module
    // head for the per-provider normalisation and why billing depends on it.
    pub reasoning_tokens: u32,

    pub total_tokens: u32,
}

impl CanonicalUsage {
    // Why: enforces the module head's one rule for providers we have never
    // probed. Returns whether the count had to be folded in, so callers can
    // assert on it; the warning is emitted here so no call site can forget it.
    pub fn normalise_reasoning(&mut self, provider: &str) -> bool {
        let stated_sum = self
            .input_tokens
            .saturating_add(self.output_tokens)
            .saturating_add(self.reasoning_tokens);
        // Why: the second clause needs the wire's own total. The streaming
        // accumulator recomputes it as the cache-inclusive sum, and that sum
        // coincides with `stated_sum` whenever the cache counts happen to
        // equal the reasoning count -- so a recomputed total is excluded
        // rather than read as a signal.
        let cache_inclusive = self
            .input_tokens
            .saturating_add(self.output_tokens)
            .saturating_add(self.cache_read_tokens)
            .saturating_add(self.cache_creation_tokens);
        let additive = self.reasoning_tokens > self.output_tokens
            || (self.reasoning_tokens > 0
                && self.total_tokens == stated_sum
                && self.total_tokens != cache_inclusive);
        if !additive {
            return false;
        }
        let folded = self.output_tokens.saturating_add(self.reasoning_tokens);
        tracing::warn!(
            provider,
            reasoning_tokens = self.reasoning_tokens,
            reported_output_tokens = self.output_tokens,
            folded_output_tokens = folded,
            "provider reports reasoning tokens in addition to output tokens; folding them in so \
             the thinking share is billed"
        );
        self.output_tokens = folded;
        self.total_tokens = self
            .input_tokens
            .saturating_add(self.output_tokens)
            .saturating_add(self.cache_read_tokens)
            .saturating_add(self.cache_creation_tokens);
        true
    }
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
    pub reasoning_tokens: Option<u32>,
}

impl CanonicalUsageUpdate {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.cache_read_tokens.is_none()
            && self.cache_creation_tokens.is_none()
            && self.reasoning_tokens.is_none()
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
        if let Some(v) = self.reasoning_tokens {
            usage.reasoning_tokens = v;
        }
        // Why: reasoning_tokens is a subset of output_tokens, so it is
        // deliberately absent from this sum -- adding it would double-count
        // every thinking turn in `total_tokens` and in the cost derived from it.
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

    // Why: providers routinely report a generic "stop" beside a fully-formed
    // tool call -- Gemini sends `finishReason: STOP` on a functionCall
    // candidate, several OpenAI-compatible upstreams send `finish_reason:
    // "stop"` beside a tool_calls array. Relayed verbatim, every client ends
    // the turn and the call is silently never run. Truncation still wins: a
    // call cut mid-arguments carries unparseable JSON, so declaring tool use
    // there hands the client a call it cannot run.
    #[must_use]
    pub const fn with_tool_use(self, has_tool_use: bool) -> Self {
        match self {
            Self::EndTurn | Self::Other if has_tool_use => Self::ToolUse,
            other => other,
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
