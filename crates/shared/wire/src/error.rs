//! Failures raised while reading a buffered upstream body or an upstream SSE
//! stream.
//!
//! The per-wire buffered parsers are lenient about *missing optional fields*
//! and strict about the body's overall shape: a reply whose top-level
//! deserialization fails is an upstream failure, never an empty success, so it
//! reaches the client as an error and the audit row as a failed request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

/// Failure to read a buffered upstream body into a canonical response.
///
/// Each variant names the wire dialect whose contract the body broke, so the
/// relayed message identifies the adapter without the caller adding context.
#[derive(Debug, Error)]
pub enum WireParseError {
    #[error("Malformed Anthropic response body: {0}")]
    Anthropic(#[source] serde_json::Error),
    #[error("Malformed Gemini response body: {0}")]
    Gemini(#[source] serde_json::Error),
    #[error("Malformed OpenAI chat completion body: {0}")]
    OpenAiChat(#[source] serde_json::Error),
    #[error("Malformed OpenAI responses body: {0}")]
    OpenAiResponses(#[source] serde_json::Error),
    // Why: the Responses API rejects a replayed function call whose id is
    // missing, so a call that arrives without `call_id` and `id` cannot be
    // represented as a canonical tool use.
    #[error("OpenAI responses body: function_call `{name}` carries neither call_id nor id")]
    OpenAiResponsesMissingToolCallId { name: String },
    // Why: a well-formed body whose only candidate finished on a refusal or an
    // unclassified reason without a single part is the provider ending the
    // turn, and it must reach the client as an error rather than an empty
    // success — the message names the raw reason and whatever the provider
    // said about it.
    #[error("upstream finished with {finish}{}", detail_suffix(.detail.as_deref()))]
    EmptyTerminal {
        finish: String,
        detail: Option<String>,
    },
    #[error("upstream blocked the prompt: {reason}{}", detail_suffix(.detail.as_deref()))]
    PromptBlocked {
        reason: String,
        detail: Option<String>,
    },
}

impl WireParseError {
    #[must_use]
    pub fn empty_terminal(finish: &str, detail: Option<&str>) -> Self {
        Self::EmptyTerminal {
            finish: finish.to_owned(),
            detail: provider_detail(detail),
        }
    }

    #[must_use]
    pub fn prompt_blocked(reason: &str, detail: Option<&str>) -> Self {
        Self::PromptBlocked {
            reason: reason.to_owned(),
            detail: provider_detail(detail),
        }
    }
}

fn provider_detail(detail: Option<&str>) -> Option<String> {
    detail
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(ToOwned::to_owned)
}

fn detail_suffix(detail: Option<&str>) -> String {
    detail.map_or_else(String::new, |detail| format!(": {detail}"))
}

/// Failure of an upstream SSE stream while translating it to canonical events.
#[derive(Debug, Error)]
pub enum WireStreamError {
    #[error("{0}")]
    Transport(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error(transparent)]
    FrameTooLarge(#[from] crate::sse::SseFrameTooLarge),
    #[error("upstream stream carried no decodable frame ({dropped} dropped)")]
    NoDecodableFrame { dropped: u32 },
}

impl WireStreamError {
    pub fn transport(source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Transport(source.into())
    }
}
