//! The retry policy as pure functions. The loop that waits and resends belongs
//! to the caller.
//!
//! [`delay`](crate::llm::retry::delay) takes a 1-based attempt: the server's
//! wait when given (capped at
//! [`MAX_RETRY_AFTER`](crate::llm::retry::MAX_RETRY_AFTER)),
//! else [`BASE_DELAY`](crate::llm::retry::BASE_DELAY) doubled per attempt up to
//! [`MAX_DELAY`](crate::llm::retry::MAX_DELAY), without jitter.
//! [`retry_after`](crate::llm::retry::retry_after) reads `retry-after-ms`
//! (milliseconds) or `retry-after` (seconds or an HTTP date).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, SystemTime};

use crate::llm::error::{LlmError, TransportError};

pub const BASE_DELAY: Duration = Duration::from_secs(2);
pub const MAX_DELAY: Duration = Duration::from_secs(30);
pub const MAX_RETRY_AFTER: Duration = Duration::from_secs(600);
const TRANSIENT_MARKERS: &[&str] = &[
    "overloaded",
    "rate_limit",
    "rate limit",
    "api_error",
    "server_error",
    "timeout",
    "try again",
    "unavailable",
];

/// Whether and how soon a failed call may be sent again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retry {
    No,
    After(Duration),
    Backoff,
}

pub fn classify(error: &LlmError) -> Retry {
    match error {
        LlmError::Status {
            status,
            retry_after,
            ..
        } => match status {
            408 | 409 | 425 | 429 | 500..=599 => retry_after.map_or(Retry::Backoff, |wait| {
                Retry::After(wait.min(MAX_RETRY_AFTER))
            }),
            _ => Retry::No,
        },
        LlmError::Provider { kind, message } => provider(kind.as_deref(), message),
        LlmError::Transport(TransportError::Client(_))
        | LlmError::ContextOverflow { .. }
        | LlmError::Aborted
        | LlmError::Decode(_)
        | LlmError::Encode(_)
        | LlmError::InvalidEndpoint => Retry::No,
        LlmError::Transport(_) | LlmError::Truncated => Retry::Backoff,
    }
}

fn provider(kind: Option<&str>, message: &str) -> Retry {
    let kind = kind.unwrap_or_default().to_ascii_lowercase();
    let message = message.to_ascii_lowercase();
    let transient = TRANSIENT_MARKERS
        .iter()
        .any(|needle| kind.contains(needle) || message.contains(needle));
    if transient { Retry::Backoff } else { Retry::No }
}

pub fn delay(attempt: u32, retry_after: Option<Duration>) -> Duration {
    if let Some(wait) = retry_after {
        return wait.min(MAX_RETRY_AFTER);
    }
    let exponent = attempt.saturating_sub(1).min(16);
    BASE_DELAY.saturating_mul(1u32 << exponent).min(MAX_DELAY)
}

pub fn retry_after(headers: &[(String, String)]) -> Option<Duration> {
    if let Some(millis) = header(headers, "retry-after-ms").and_then(|v| v.parse::<f64>().ok()) {
        return Duration::try_from_secs_f64(millis / 1000.0).ok();
    }
    let value = header(headers, "retry-after")?;
    if let Ok(seconds) = value.parse::<f64>() {
        return Duration::try_from_secs_f64(seconds).ok();
    }
    let at = httpdate::parse_http_date(value).ok()?;
    at.duration_since(SystemTime::now()).ok()
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.trim())
        .filter(|value| !value.is_empty())
}
