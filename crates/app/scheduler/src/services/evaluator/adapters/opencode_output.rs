//! `OpenCode` JSON events retain completion uncertainty and advisory step
//! usage.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::{NativeCompletion, NormalizedClientOutput, invalid, malformed};
use serde_json::Value;
use std::collections::BTreeMap;
use systemprompt_evaluation::Result;

struct OpenCodeStream {
    parts: BTreeMap<String, (String, Value)>,
    session: Option<String>,
    text: String,
    tools: Vec<String>,
    completion: NativeCompletion,
    failed: bool,
    runner_closed: bool,
    steps: u32,
    input: Option<u64>,
    output: Option<u64>,
}

pub(super) fn normalize(bytes: &[u8]) -> Result<NormalizedClientOutput> {
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(invalid("OpenCode evidence exceeds 16 MiB"));
    }
    let body = std::str::from_utf8(bytes)
        .map_err(|error| malformed("OpenCode evidence is not UTF-8", error))?;
    let mut stream = OpenCodeStream {
        parts: BTreeMap::new(),
        session: None,
        text: String::new(),
        tools: Vec::new(),
        completion: NativeCompletion::Incomplete,
        failed: false,
        runner_closed: false,
        steps: 0,
        input: Some(0u64),
        output: Some(0u64),
    };
    for (index, line) in body.lines().enumerate() {
        if index >= 8192 || line.len() > 1024 * 1024 {
            return Err(invalid("OpenCode event stream exceeds event bounds"));
        }
        if line.trim().is_empty() {
            continue;
        }
        // JSON: OpenCode run --format json emits version-specific parts inside each
        // event.
        let event: Value = serde_json::from_str(line)
            .map_err(|error| malformed("Malformed OpenCode JSON event", error))?;
        stream.record(&event)?;
    }
    stream.finish()
}

impl OpenCodeStream {
    fn record(&mut self, event: &Value) -> Result<()> {
        let kind = required(event, "type")?;
        if self.runner_closed {
            return Err(invalid(
                "OpenCode emitted evidence after runner termination",
            ));
        }
        if kind == "opencode.runner_error" {
            if required(event, "diagnostic")?.len() > 4096 {
                return Err(invalid("OpenCode runner diagnostic exceeds bound"));
            }
            self.failed = true;
            self.runner_closed = true;
            return Ok(());
        }
        if kind == "opencode.runner_result" {
            self.failed |= runner_failed(event)?;
            self.runner_closed = true;
            return Ok(());
        }
        let event_session = required(event, "sessionID")?;
        if event_session.len() > 256 || event_session.chars().any(char::is_control) {
            return Err(invalid("Invalid OpenCode session identity"));
        }
        if let Some(existing) = &self.session {
            if existing != event_session {
                return Err(invalid("Mixed OpenCode session evidence"));
            }
        } else {
            self.session = Some(event_session.to_owned());
        }
        if kind == "error" {
            if !event.get("error").is_some_and(Value::is_object) {
                return Err(invalid("Malformed OpenCode error event"));
            }
            self.failed = true;
            return Ok(());
        }
        if !matches!(
            kind,
            "step_start" | "step_finish" | "text" | "tool_use" | "reasoning"
        ) {
            return Ok(());
        }
        let part = event
            .get("part")
            .ok_or_else(|| invalid("OpenCode event has no part"))?;
        self.record_part(kind, event_session, part)
    }

    fn record_part(&mut self, kind: &str, event_session: &str, part: &Value) -> Result<()> {
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
        if let Some((existing_kind, existing)) = self.parts.get(id) {
            if existing_kind != kind || existing != part {
                return Err(invalid("Conflicting OpenCode part evidence"));
            }
            return Ok(());
        }
        self.parts
            .insert(id.to_owned(), (kind.to_owned(), part.clone()));
        match kind {
            "step_start" => self.completion = NativeCompletion::Incomplete,
            "text" => {
                if !self.text.is_empty() {
                    self.text.push('\n');
                }
                self.text.push_str(required(part, "text")?);
            },
            "tool_use" => {
                let status = part
                    .pointer("/state/status")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("OpenCode tool has no terminal state"))?;
                if !matches!(status, "completed" | "error") {
                    return Err(invalid("Nonterminal OpenCode tool event"));
                }
                self.tools.push(required(part, "tool")?.to_owned());
            },
            "step_finish" => {
                self.steps += 1;
                self.input = add_usage(self.input, part, "input")?;
                self.output = add_usage(self.output, part, "output")?;
                self.completion = match required(part, "reason")? {
                    "stop" => NativeCompletion::Completed,
                    "length" | "content-filter" | "error" => {
                        self.failed = true;
                        NativeCompletion::Failed
                    },
                    _ => NativeCompletion::Incomplete,
                };
            },
            _ => {},
        }
        Ok(())
    }

    fn finish(self) -> Result<NormalizedClientOutput> {
        let normalized = NormalizedClientOutput {
            text: self.text,
            completion: if self.failed {
                NativeCompletion::Failed
            } else if self.runner_closed {
                self.completion
            } else {
                NativeCompletion::Incomplete
            },
            reported_input_tokens: if self.steps > 0 { self.input } else { None },
            reported_output_tokens: if self.steps > 0 { self.output } else { None },
            tool_calls: self.tools,
        };
        normalized.validate()?;
        Ok(normalized)
    }
}

fn runner_failed(event: &Value) -> Result<bool> {
    let attempts = event
        .get("provider_attempts")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("Missing OpenCode provider attempt count"))?;
    let maximum = event
        .get("max_requests")
        .and_then(Value::as_u64)
        .filter(|value| (1..=100).contains(value))
        .ok_or_else(|| invalid("Missing OpenCode request bound"))?;
    let limited = event
        .get("output_limit_reached")
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid("Missing OpenCode output bound result"))?;
    let code = event
        .get("process_exit_code")
        .ok_or_else(|| invalid("Missing OpenCode process exit"))?;
    let signal = event
        .get("signal")
        .ok_or_else(|| invalid("Missing OpenCode process signal"))?;
    let code_valid = code.is_null() || code.as_u64().is_some_and(|value| value <= 255);
    let signal_valid = signal.is_null()
        || signal
            .as_str()
            .is_some_and(|value| !value.is_empty() && value.len() <= 32);
    if !code_valid || !signal_valid || (code.is_null() && signal.is_null()) {
        return Err(invalid("Malformed OpenCode runner process result"));
    }
    Ok(code.as_u64() != Some(0) || !signal.is_null() || limited || attempts > maximum)
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
