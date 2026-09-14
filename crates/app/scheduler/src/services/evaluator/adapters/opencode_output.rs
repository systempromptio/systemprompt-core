//! OpenCode JSON events retain completion uncertainty and advisory step usage.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::{NativeCompletion, NormalizedClientOutput};
use super::invalid;
use serde_json::Value;
use std::collections::BTreeMap;
use systemprompt_evaluation::Result;

pub(super) fn normalize(bytes: &[u8]) -> Result<NormalizedClientOutput> {
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(invalid("OpenCode evidence exceeds 16 MiB"));
    }
    let body = std::str::from_utf8(bytes).map_err(|_| invalid("OpenCode evidence is not UTF-8"))?;
    let mut parts = BTreeMap::new();
    let mut session: Option<String> = None;
    let mut text = String::new();
    let mut tools = Vec::new();
    let mut completion = NativeCompletion::Incomplete;
    let mut failed = false;
    let mut steps = 0;
    let mut input = Some(0_u64);
    let mut output = Some(0_u64);
    for (index, line) in body.lines().enumerate() {
        if index >= 8192 || line.len() > 1024 * 1024 {
            return Err(invalid("OpenCode event stream exceeds event bounds"));
        }
        if line.trim().is_empty() {
            continue;
        }
        // JSON: OpenCode run --format json emits version-specific parts inside each
        // event.
        let event: Value =
            serde_json::from_str(line).map_err(|_| invalid("Malformed OpenCode JSON event"))?;
        let kind = required(&event, "type")?;
        let event_session = required(&event, "sessionID")?;
        if event_session.len() > 256 || event_session.chars().any(char::is_control) {
            return Err(invalid("Invalid OpenCode session identity"));
        }
        if let Some(existing) = &session {
            if existing != event_session {
                return Err(invalid("Mixed OpenCode session evidence"));
            }
        } else {
            session = Some(event_session.to_owned());
        }
        if kind == "error" {
            if !event.get("error").is_some_and(Value::is_object) {
                return Err(invalid("Malformed OpenCode error event"));
            }
            failed = true;
            continue;
        }
        if !matches!(
            kind,
            "step_start" | "step_finish" | "text" | "tool_use" | "reasoning"
        ) {
            continue;
        }
        let part = event
            .get("part")
            .ok_or_else(|| invalid("OpenCode event has no part"))?;
        let id = required(part, "id")?;
        if id.len() > 256 || id.chars().any(char::is_control) {
            return Err(invalid("Invalid OpenCode part identity"));
        }
        if required(part, "sessionID")? != event_session {
            return Err(invalid("OpenCode part session mismatch"));
        }
        let expected = match kind {
            "step_start" => "step-start",
            "step_finish" => "step-finish",
            "tool_use" => "tool",
            other => other,
        };
        if required(part, "type")? != expected {
            return Err(invalid("OpenCode event and part types disagree"));
        }
        if let Some((existing_kind, existing)) = parts.get(id) {
            if existing_kind != kind || existing != part {
                return Err(invalid("Conflicting OpenCode part evidence"));
            }
            continue;
        }
        parts.insert(id.to_owned(), (kind.to_owned(), part.clone()));
        match kind {
            "step_start" => completion = NativeCompletion::Incomplete,
            "text" => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(required(part, "text")?);
            },
            "tool_use" => {
                let status = part
                    .pointer("/state/status")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("OpenCode tool has no terminal state"))?;
                if !matches!(status, "completed" | "error") {
                    return Err(invalid("Nonterminal OpenCode tool event"));
                }
                tools.push(required(part, "tool")?.to_owned());
            },
            "step_finish" => {
                steps += 1;
                input = add_usage(input, part, "input")?;
                output = add_usage(output, part, "output")?;
                completion = match required(part, "reason")? {
                    "stop" => NativeCompletion::Completed,
                    "tool-calls" => NativeCompletion::Incomplete,
                    "length" | "content-filter" | "error" => {
                        failed = true;
                        NativeCompletion::Failed
                    },
                    _ => NativeCompletion::Incomplete,
                };
            },
            _ => {},
        }
    }
    let normalized = NormalizedClientOutput {
        text,
        completion: if failed {
            NativeCompletion::Failed
        } else {
            completion
        },
        reported_input_tokens: if steps > 0 { input } else { None },
        reported_output_tokens: if steps > 0 { output } else { None },
        tool_calls: tools,
    };
    normalized.validate()?;
    Ok(normalized)
}
fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| invalid("OpenCode event has a missing or invalid required field"))
}
fn add_usage(total: Option<u64>, part: &Value, field: &str) -> Result<Option<u64>> {
    let Some(tokens) = part.get("tokens") else {
        return Ok(None);
    };
    if !tokens.is_object() {
        return Err(invalid("OpenCode token usage must be an object"));
    }
    let Some(value) = tokens.get(field) else {
        return Ok(None);
    };
    let amount = value
        .as_u64()
        .ok_or_else(|| invalid("OpenCode token usage must be nonnegative integers"))?;
    total
        .map(|total| {
            total
                .checked_add(amount)
                .ok_or_else(|| invalid("OpenCode token usage overflow"))
        })
        .transpose()
}
