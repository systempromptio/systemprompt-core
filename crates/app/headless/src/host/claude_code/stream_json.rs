//! Decodes Claude Code `--output-format stream-json` lines into
//! [`HostEvent`]s.
//!
//! One assistant message carries several content blocks, so one line can
//! yield several payloads. Lines the contract does not record (rate limits,
//! hook starts) yield none; a line that is not JSON at all yields a
//! [`RunEventPayload::Runner`] warning, never a panic. Free text is capped at
//! [`TEXT_CAP_BYTES`] so one verbose tool result cannot bloat the events
//! table.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::HostSessionId;

use super::blocks::{Block, Line, Message, MessageContent, ResultLine, SystemLine};
use crate::event::{HookDecision, RunEventPayload, RunnerLevel};
use crate::host::HostEvent;

pub const TEXT_CAP_BYTES: usize = 64 * 1024;

const DENY_OUTCOMES: &[&str] = &["blocked", "blocking", "deny", "denied"];

#[must_use]
pub fn cap_text(text: &str) -> String {
    if text.len() <= TEXT_CAP_BYTES {
        return text.to_owned();
    }
    let mut end = TEXT_CAP_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…[truncated {} bytes]", &text[..end], text.len() - end)
}

fn session(raw: Option<&str>) -> Option<HostSessionId> {
    raw.and_then(|id| HostSessionId::try_new(id).ok())
}

const fn warn(message: String) -> RunEventPayload {
    RunEventPayload::Runner {
        message,
        level: RunnerLevel::Warn,
    }
}

#[must_use]
pub fn decode_line(line: &str) -> Vec<HostEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let (payloads, session_id) = match serde_json::from_str::<Line>(trimmed) {
        Ok(Line::System(system)) => {
            let id = session(system.raw_session.as_deref());
            (system_payloads(system, id.clone()), id)
        },
        Ok(
            Line::Assistant {
                message,
                raw_session,
            }
            | Line::User {
                message,
                raw_session,
            },
        ) => (block_payloads(message), session(raw_session.as_deref())),
        Ok(Line::Result(result)) => {
            let id = session(result.raw_session.as_deref());
            (vec![result_payload(result)], id)
        },
        Ok(Line::Other) => (Vec::new(), None),
        Err(error) => (
            vec![warn(format!("undecodable stream-json line: {error}"))],
            None,
        ),
    };
    if payloads.is_empty() && session_id.is_none() {
        return Vec::new();
    }
    vec![HostEvent {
        payloads,
        session_id,
    }]
}

fn system_payloads(system: SystemLine, session_id: Option<HostSessionId>) -> Vec<RunEventPayload> {
    match system.subtype.as_str() {
        "init" => match session_id {
            Some(session_id) => vec![RunEventPayload::Init {
                session_id,
                model: system.model.unwrap_or_default(),
                tools: system.tools,
                cwd: system.cwd,
            }],
            None => vec![warn("system/init line without a session id".to_owned())],
        },
        "hook_response" => vec![hook_payload(system)],
        _ => Vec::new(),
    }
}

fn hook_payload(system: SystemLine) -> RunEventPayload {
    let output = system.output.unwrap_or_default();
    let denied_by_output = output.contains("\"permissionDecision\":\"deny\"")
        || output.contains("\"permissionDecision\": \"deny\"");
    let decision = match system.outcome.as_deref() {
        _ if denied_by_output => HookDecision::Deny,
        Some(outcome) if DENY_OUTCOMES.contains(&outcome) => HookDecision::Deny,
        Some("success") => HookDecision::Allow,
        _ => HookDecision::Pending,
    };
    RunEventPayload::Hook {
        name: system.hook_name.or(system.hook_event).unwrap_or_default(),
        decision,
        policy: None,
    }
}

fn block_payloads(message: Message) -> Vec<RunEventPayload> {
    let MessageContent::Blocks(blocks) = message.content else {
        return Vec::new();
    };
    blocks
        .into_iter()
        .filter_map(|block| match block {
            Block::Text { text } if !text.trim().is_empty() => {
                Some(RunEventPayload::AssistantText {
                    text: cap_text(&text),
                })
            },
            Block::ToolUse { id, name, input } => Some(RunEventPayload::ToolUse {
                tool_use_id: id,
                name,
                input,
            }),
            Block::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => Some(RunEventPayload::ToolResult {
                tool_use_id,
                content: cap_text(&content.flatten()),
                is_error: is_error.unwrap_or(false),
            }),
            Block::Text { .. } | Block::Other => None,
        })
        .collect()
}

#[must_use]
pub fn usd_to_microdollars(usd: f64) -> i64 {
    if !usd.is_finite() || usd <= 0.0 {
        return 0;
    }
    (usd * 1_000_000.0).round() as i64
}

fn result_payload(result: ResultLine) -> RunEventPayload {
    let usage = result.usage;
    let error = result
        .is_error
        .then(|| {
            result
                .result
                .as_deref()
                .map(cap_text)
                .or_else(|| (!result.errors.is_empty()).then(|| result.errors.join("; ")))
        })
        .flatten();
    RunEventPayload::Result {
        subtype: result.subtype,
        is_error: result.is_error,
        num_turns: result.num_turns,
        duration_ms: result.duration_ms,
        input_tokens: usage.input_tokens
            + usage.cache_read_input_tokens
            + usage.cache_creation_input_tokens,
        output_tokens: usage.output_tokens,
        cost_microdollars: usd_to_microdollars(result.total_cost_usd),
        structured_output: result.structured_output,
        error,
    }
}
