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
    #[error("{0}")]
    EmptyTerminal(String),
}

/// Failure of an upstream SSE stream while translating it to canonical events.
#[derive(Debug, Error)]
pub enum WireStreamError {
    #[error("{0}")]
    Transport(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("malformed Gemini SSE frame: {0}")]
    MalformedGeminiFrame(#[source] serde_json::Error),
    #[error("malformed Responses SSE frame: {0}")]
    MalformedResponsesFrame(#[source] serde_json::Error),
}

impl WireStreamError {
    pub fn transport(source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Transport(source.into())
    }
}
