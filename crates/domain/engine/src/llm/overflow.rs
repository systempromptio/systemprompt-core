//! Recognises "the prompt does not fit" answers from either protocol.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

const OVERFLOW_PHRASES: &[&str] = &[
    "prompt is too long",
    "input is too long",
    "request_too_large",
    "request entity too large",
    "context_length_exceeded",
    "context length exceeded",
    "maximum context length",
    "model_context_window_exceeded",
    "exceeds the context window",
    "exceeds the model's context",
    "context window exceeded",
    "reduce the length of the messages",
    "token limit exceeded",
    "too many tokens",
    "exceeds the available context size",
];

const NOT_OVERFLOW_PHRASES: &[&str] = &[
    "rate limit",
    "rate_limit",
    "too many requests",
    "overloaded",
];

pub fn is_context_overflow(status: Option<u16>, text: &str) -> bool {
    match status {
        Some(413) => return true,
        Some(429) => return false,
        _ => {},
    }
    let lower = text.to_lowercase();
    if NOT_OVERFLOW_PHRASES
        .iter()
        .any(|phrase| lower.contains(phrase))
    {
        return false;
    }
    OVERFLOW_PHRASES.iter().any(|phrase| lower.contains(phrase))
}
