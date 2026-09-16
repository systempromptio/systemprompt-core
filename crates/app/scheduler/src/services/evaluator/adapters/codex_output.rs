//! Bounded Codex exec JSONL normalization keeps terminal and usage evidence
//! distinct.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::{NativeCompletion, NormalizedClientOutput, invalid, malformed};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use systemprompt_evaluation::Result;

struct CodexStream {
    thread: Option<String>,
    terminal: Option<Value>,
    completed: BTreeMap<String, Value>,
    pending: BTreeSet<String>,
    text: String,
    tools: Vec<String>,
    failed: bool,
}

pub(super) fn normalize(bytes: &[u8]) -> Result<NormalizedClientOutput> {
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(invalid("Codex evidence exceeds 16 MiB"));
    }
    let body = std::str::from_utf8(bytes)
        .map_err(|error| malformed("Codex evidence is not UTF-8", error))?;
    let mut stream = CodexStream {
        thread: None,
        terminal: None,
        completed: BTreeMap::new(),
        pending: BTreeSet::new(),
        text: String::new(),
        tools: Vec::new(),
        failed: false,
    };
    for (index, line) in body.lines().enumerate() {
        if index >= 8192 || line.len() > 1024 * 1024 {
            return Err(invalid("Codex event stream exceeds event bounds"));
        }
        if line.trim().is_empty() {
            continue;
        }
        // JSON: Codex exec emits version-specific item records and one terminal turn
        // event.
        let event: Value = serde_json::from_str(line)
            .map_err(|error| malformed("Malformed Codex JSON event", error))?;
        stream.record(event)?;
    }
    stream.finish()
}

impl CodexStream {
    fn record(&mut self, event: Value) -> Result<()> {
        match required(&event, "type")? {
            "thread.started" => {
                let id = identity(&event, "thread_id")?;
                if let Some(existing) = &self.thread {
                    if existing != id {
                        return Err(invalid("Mixed Codex thread evidence"));
                    }
                } else {
                    self.thread = Some(id.to_owned());
                }
            },
            "turn.completed" | "turn.failed" => {
                if let Some(existing) = &self.terminal {
                    if existing != &event {
                        return Err(invalid("Conflicting Codex terminal evidence"));
                    }
                } else {
                    self.terminal = Some(event);
                }
            },
            "turn.started" => {
                if self.terminal.is_some() {
                    return Err(invalid(
                        "Codex started another turn after terminal evidence",
                    ));
                }
            },
            "error" => self.failed = true,
            "item.started" | "item.updated" => {
                let item = event
                    .get("item")
                    .ok_or_else(|| invalid("Codex event has no item"))?;
                let id = identity(item, "id")?;
                if !self.completed.contains_key(id) {
                    self.pending.insert(id.to_owned());
                }
            },
            "item.completed" => {
                let item = event
                    .get("item")
                    .ok_or_else(|| invalid("Codex event has no item"))?;
                self.complete_item(item)?;
            },
            _ => {},
        }
        Ok(())
    }

    fn complete_item(&mut self, item: &Value) -> Result<()> {
        let id = identity(item, "id")?;
        if let Some(existing) = self.completed.get(id) {
            if existing != item {
                return Err(invalid("Conflicting Codex item evidence"));
            }
            return Ok(());
        }
        if self.terminal.is_some() {
            return Err(invalid(
                "Codex emitted new item evidence after the terminal turn",
            ));
        }
        self.pending.remove(id);
        match required(item, "type")? {
            "agent_message" => required(item, "text")?.clone_into(&mut self.text),
            "command_execution" => self.tools.push("exec_command".to_owned()),
            "file_change" => self.tools.push("apply_patch".to_owned()),
            "mcp_tool_call" => self.tools.push(format!(
                "mcp__{}__{}",
                identity(item, "server")?,
                identity(item, "tool")?
            )),
            "web_search" | "web_search_call" | "collab_tool_call" => {
                return Err(invalid("Codex emitted a forbidden tool"));
            },
            _ => {},
        }
        self.completed.insert(id.to_owned(), item.clone());
        Ok(())
    }

    fn finish(mut self) -> Result<NormalizedClientOutput> {
        let mut normalized = NormalizedClientOutput {
            text: self.text,
            completion: NativeCompletion::Incomplete,
            reported_input_tokens: None,
            reported_output_tokens: None,
            tool_calls: self.tools,
        };
        if let Some(terminal) = self.terminal {
            if terminal["type"] == "turn.failed" {
                self.failed = true;
            }
            if !self.failed && self.pending.is_empty() {
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
        if self.failed {
            normalized.completion = NativeCompletion::Failed;
        }
        normalized.validate()?;
        Ok(normalized)
    }
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
