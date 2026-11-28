//! Rendering a scripted turn as one model response, in the protocol of the
//! route that was called, and the faults a turn can carry in place of it.

pub(crate) mod chat;
pub(crate) mod messages;

use std::io;
use std::time::Duration;

use axum::body::Body;
use futures_util::stream;
use serde_json::Value;
use systemprompt_wire::WireProtocol;

use crate::script::{Respond, StopReason, ToolCall, Turn, TurnFault, Usage};

const MALFORMED_FRAME: &str = "data: {\"type\": \n\n";
const MALFORMED_BODY: &str = "{\"id\": ";

// Why: hyper writes buffered frames once the body is pending; a close in the
// same poll as the last frame would drop the frames the turn keeps.
const FLUSH_BEFORE_CLOSE: Duration = Duration::from_millis(25);

#[derive(Debug)]
pub(crate) enum Step {
    Frame(String),
    Pause(Duration),
    Abort,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Call<'a> {
    pub(crate) protocol: WireProtocol,
    pub(crate) model: &'a str,
    pub(crate) sequence: u64,
}

pub(crate) fn stream_steps(turn: &Turn, call: Call<'_>) -> Vec<Step> {
    let frames = match call.protocol {
        WireProtocol::OpenAiChat => chat::frames(turn, call),
        _ => messages::frames(turn, call),
    };
    match turn.fault {
        Some(TurnFault::CloseAfterEvents(count)) => {
            let kept = usize::try_from(count).unwrap_or(usize::MAX);
            let mut steps: Vec<Step> = frames.into_iter().take(kept).map(Step::Frame).collect();
            steps.push(Step::Pause(FLUSH_BEFORE_CLOSE));
            steps.push(Step::Abort);
            steps
        },
        Some(TurnFault::StallMs(millis)) => {
            let terminal = match call.protocol {
                WireProtocol::OpenAiChat => chat::TERMINAL_FRAMES,
                _ => messages::TERMINAL_FRAMES,
            };
            let answered = frames.len().saturating_sub(terminal);
            let mut steps: Vec<Step> = frames.into_iter().map(Step::Frame).collect();
            steps.insert(answered, Step::Pause(Duration::from_millis(millis)));
            steps
        },
        Some(TurnFault::MalformedEvent) => frames
            .into_iter()
            .take(1)
            .chain(std::iter::once(MALFORMED_FRAME.to_owned()))
            .map(Step::Frame)
            .collect(),
        Some(TurnFault::Status(_) | TurnFault::ContextOverflow | TurnFault::HeadersAfterMs(_))
        | None => frames.into_iter().map(Step::Frame).collect(),
    }
}

pub(crate) fn buffered_steps(turn: &Turn, call: Call<'_>) -> Vec<Step> {
    let body = match call.protocol {
        WireProtocol::OpenAiChat => chat::body(turn, call),
        _ => messages::body(turn, call),
    }
    .to_string();
    match turn.fault {
        Some(TurnFault::CloseAfterEvents(_)) => vec![Step::Abort],
        Some(TurnFault::StallMs(millis)) => {
            vec![
                Step::Pause(Duration::from_millis(millis)),
                Step::Frame(body),
            ]
        },
        Some(TurnFault::MalformedEvent) => vec![Step::Frame(MALFORMED_BODY.to_owned())],
        Some(TurnFault::Status(_) | TurnFault::ContextOverflow | TurnFault::HeadersAfterMs(_))
        | None => vec![Step::Frame(body)],
    }
}

pub(crate) fn body(steps: Vec<Step>) -> Body {
    Body::from_stream(stream::unfold(steps.into_iter(), |mut steps| async move {
        loop {
            match steps.next()? {
                Step::Frame(frame) => return Some((Ok(frame), steps)),
                Step::Pause(pause) => tokio::time::sleep(pause).await,
                Step::Abort => {
                    let closed = io::Error::new(
                        io::ErrorKind::ConnectionAborted,
                        "the scripted turn closes the stream here",
                    );
                    return Some((Err(closed), steps));
                },
            }
        }
    }))
}

pub(crate) fn stop_of(turn: &Turn) -> StopReason {
    turn.stop.unwrap_or_else(|| {
        if tool_calls(turn).next().is_some() {
            StopReason::ToolUse
        } else {
            StopReason::End
        }
    })
}

pub(crate) fn usage_of(turn: &Turn) -> Usage {
    turn.usage.unwrap_or_default()
}

pub(crate) fn tool_calls(turn: &Turn) -> impl Iterator<Item = &ToolCall> {
    turn.respond.iter().filter_map(|part| match part {
        Respond::ToolCall(call) => Some(call),
        Respond::Text(_) => None,
    })
}

pub(crate) fn text_of(turn: &Turn) -> String {
    turn.respond
        .iter()
        .filter_map(|part| match part {
            Respond::Text(text) => Some(text.as_str()),
            Respond::ToolCall(_) => None,
        })
        .collect()
}

pub(crate) fn argument_chunks(call: &ToolCall) -> Vec<String> {
    if call.argument_chunks.is_empty() {
        vec![call.input.to_string()]
    } else {
        call.argument_chunks.clone()
    }
}

/// The 400 body each provider sends when the prompt does not fit: the
/// phrasing the Messages API and the Chat Completions API use.
pub(crate) fn overflow_body(protocol: WireProtocol, window: u64) -> Value {
    let asked = window.saturating_add(1);
    match protocol {
        WireProtocol::OpenAiChat => serde_json::json!({
            "error": {
                "message": format!(
                    "This model's maximum context length is {window} tokens. However, your \
                     messages resulted in {asked} tokens. Please reduce the length of the messages."
                ),
                "type": "invalid_request_error",
                "param": "messages",
                "code": "context_length_exceeded",
            },
        }),
        _ => serde_json::json!({
            "type": "error",
            "error": {
                "type": "invalid_request_error",
                "message": format!("prompt is too long: {asked} tokens > {window} maximum"),
            },
        }),
    }
}

pub(crate) fn error_body(protocol: WireProtocol, status: u16, message: &str) -> Value {
    let kind = match status {
        401 => "authentication_error",
        429 => "rate_limit_error",
        529 => "overloaded_error",
        400..=499 => "invalid_request_error",
        _ => "api_error",
    };
    match protocol {
        WireProtocol::OpenAiChat => serde_json::json!({
            "error": { "message": message, "type": kind, "param": null, "code": null },
        }),
        _ => serde_json::json!({
            "type": "error",
            "error": { "type": kind, "message": message },
        }),
    }
}
