//! Bounded Codex exec JSONL normalization keeps terminal and usage evidence
//! distinct.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::{NativeCompletion, NormalizedClientOutput};
use super::invalid;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use systemprompt_evaluation::Result;

pub(super) fn normalize(bytes: &[u8]) -> Result<NormalizedClientOutput> {
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(invalid("Codex evidence exceeds 16 MiB"));
    }
    let body = std::str::from_utf8(bytes).map_err(|_| invalid("Codex evidence is not UTF-8"))?;
    let mut thread: Option<String> = None;
    let mut terminal: Option<Value> = None;
    let mut completed = BTreeMap::new();
    let mut pending = BTreeSet::new();
    let mut text = String::new();
    let mut tools = Vec::new();
    let mut failed = false;
    for (index, line) in body.lines().enumerate() {
        if index >= 8192 || line.len() > 1024 * 1024 {
            return Err(invalid("Codex event stream exceeds event bounds"));
        }
        if line.trim().is_empty() {
            continue;
        }
        // JSON: Codex exec emits version-specific item records and one terminal turn
        // event.
        let event: Value =
            serde_json::from_str(line).map_err(|_| invalid("Malformed Codex JSON event"))?;
        match required(&event, "type")? {
            "thread.started" => {
                let id = identity(&event, "thread_id")?;
                if let Some(existing) = &thread {
                    if existing != id {
                        return Err(invalid("Mixed Codex thread evidence"));
                    }
                } else {
                    thread = Some(id.to_owned());
                }
            },
            "turn.completed" | "turn.failed" => {
                if let Some(existing) = &terminal {
                    if existing != &event {
                        return Err(invalid("Conflicting Codex terminal evidence"));
                    }
                } else {
                    terminal = Some(event);
                }
            },
            "turn.started" => {
                if terminal.is_some() {
                    return Err(invalid(
                        "Codex started another turn after terminal evidence",
                    ));
                }
            },
            "error" => failed = true,
            "item.started" | "item.updated" => {
                let item = event
                    .get("item")
                    .ok_or_else(|| invalid("Codex event has no item"))?;
                let id = identity(item, "id")?;
                if !completed.contains_key(id) {
                    pending.insert(id.to_owned());
                }
            },
            "item.completed" => {
                let item = event
                    .get("item")
                    .ok_or_else(|| invalid("Codex event has no item"))?;
                let id = identity(item, "id")?;
                if let Some(existing) = completed.get(id) {
                    if existing != item {
                        return Err(invalid("Conflicting Codex item evidence"));
                    }
                    continue;
                }
                if terminal.is_some() {
                    return Err(invalid(
                        "Codex emitted new item evidence after the terminal turn",
                    ));
                }
                pending.remove(id);
                match required(item, "type")? {
                    "agent_message" => text = required(item, "text")?.to_owned(),
                    "command_execution" => tools.push("exec_command".to_owned()),
                    "file_change" => tools.push("apply_patch".to_owned()),
                    "mcp_tool_call" => tools.push(format!(
                        "mcp__{}__{}",
                        identity(item, "server")?,
                        identity(item, "tool")?
                    )),
                    "web_search" | "web_search_call" | "collab_tool_call" => {
                        return Err(invalid("Codex emitted a forbidden tool"));
                    },
                    _ => {},
                }
                completed.insert(id.to_owned(), item.clone());
            },
            _ => {},
        }
    }
    let mut normalized = NormalizedClientOutput {
        text,
        completion: NativeCompletion::Incomplete,
        reported_input_tokens: None,
        reported_output_tokens: None,
        tool_calls: tools,
    };
    if let Some(terminal) = terminal {
        if terminal["type"] == "turn.failed" {
            failed = true;
        }
        if !failed && pending.is_empty() {
            normalized.completion = NativeCompletion::Completed;
        }
        if let Some(usage) = terminal.get("usage") {
            if !usage.is_object() {
                return Err(invalid("Codex usage must be an object"));
            }
            normalized.reported_input_tokens = usage_field(usage, "input_tokens")?;
            normalized.reported_output_tokens = usage_field(usage, "output_tokens")?;
            usage_field(usage, "cached_input_tokens")?;
            usage_field(usage, "reasoning_output_tokens")?;
        }
    }
    if failed {
        normalized.completion = NativeCompletion::Failed;
    }
    normalized.validate()?;
    Ok(normalized)
}
fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("Codex event has an invalid required field"))
}
fn identity<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    let id = required(value, key)?;
    if id.is_empty() || id.len() > 256 || id.chars().any(char::is_control) {
        return Err(invalid("Invalid Codex evidence identity"));
    }
    Ok(id)
}
fn usage_field(usage: &Value, field: &str) -> Result<Option<u64>> {
    usage
        .get(field)
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| invalid("Codex usage must be nonnegative integers"))
        })
        .transpose()
}
