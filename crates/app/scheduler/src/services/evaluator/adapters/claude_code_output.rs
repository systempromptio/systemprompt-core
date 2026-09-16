//! Bounded Claude stream-json normalization preserves terminal and metering
//! uncertainty.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::{NativeCompletion, NormalizedClientOutput, invalid, malformed};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use systemprompt_evaluation::Result;

pub(super) fn normalize(bytes: &[u8]) -> Result<NormalizedClientOutput> {
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(invalid("Claude evidence exceeds 16 MiB"));
    }
    let body = std::str::from_utf8(bytes)
        .map_err(|error| malformed("Claude evidence is not UTF-8", error))?;
    let mut terminal: Option<Value> = None;
    let mut tools = BTreeMap::new();
    let mut texts = BTreeSet::new();
    let mut response = String::new();
    for (index, line) in body.lines().enumerate() {
        if index >= 8192 || line.len() > 1024 * 1024 {
            return Err(invalid("Claude event stream exceeds event bounds"));
        }
        if line.trim().is_empty() {
            continue;
        }
        // JSON: Claude stream-json emits one protocol event per line, including
        // client-version-specific fields.
        let event: Value = serde_json::from_str(line)
            .map_err(|error| malformed("Malformed Claude stream-json event", error))?;
        let kind = event
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("Claude event type is missing"))?;
        match kind {
            "result" => {
                if let Some(existing) = &terminal {
                    if existing != &event {
                        return Err(invalid("Conflicting Claude terminal evidence"));
                    }
                } else {
                    terminal = Some(event);
                }
            },
            "assistant" => collect_assistant(&event, &mut tools, &mut texts, &mut response)?,
            _ => {},
        }
    }
    let mut normalized = NormalizedClientOutput {
        text: response,
        completion: NativeCompletion::Incomplete,
        reported_input_tokens: None,
        reported_output_tokens: None,
        tool_calls: tools
            .into_values()
            .map(|tool: Value| tool["name"].as_str().unwrap_or_default().to_owned())
            .collect(),
    };
    if let Some(event) = terminal {
        normalize_terminal(&event, &mut normalized)?;
    }
    normalized.validate()?;
    Ok(normalized)
}

fn collect_assistant(
    event: &Value,
    tools: &mut BTreeMap<String, Value>,
    texts: &mut BTreeSet<(String, String)>,
    response: &mut String,
) -> Result<()> {
    let message = event
        .get("message")
        .ok_or_else(|| invalid("Claude assistant message is missing"))?;
    let message_id = message
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if message_id.len() > 256 {
        return Err(invalid("Claude message identity exceeds its bound"));
    }
    let blocks = message
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("Claude assistant content is malformed"))?;
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                let text = block
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("Claude text block is malformed"))?;
                if texts.insert((message_id.to_owned(), text.to_owned())) {
                    if !response.is_empty() {
                        response.push('\n');
                    }
                    response.push_str(text);
                }
            },
            Some("tool_use") => {
                let id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("Claude tool identity is missing"))?;
                let name = block
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("Claude tool name is missing"))?;
                if id.is_empty()
                    || id.len() > 256
                    || name.is_empty()
                    || name.len() > 256
                    || name.chars().any(char::is_control)
                {
                    return Err(invalid("Claude tool evidence exceeds identity bounds"));
                }
                if let Some(existing) = tools.get(id) {
                    if existing != block {
                        return Err(invalid("Conflicting Claude tool-use evidence"));
                    }
                } else {
                    if tools.len() >= 1000 {
                        return Err(invalid("Claude tool evidence exceeds 1000 calls"));
                    }
                    tools.insert(id.to_owned(), block.clone());
                }
            },
            Some(_) => {},
            None => return Err(invalid("Claude content block type is missing")),
        }
    }
    Ok(())
}

fn normalize_terminal(event: &Value, normalized: &mut NormalizedClientOutput) -> Result<()> {
    let failed = event
        .get("is_error")
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| invalid("Claude terminal error flag is malformed"))
        })
        .transpose()?
        .unwrap_or(false);
    let subtype = event
        .get("subtype")
        .and_then(Value::as_str)
        .unwrap_or_default();
    normalized.completion = if failed || subtype.starts_with("error_") {
        NativeCompletion::Failed
    } else if subtype == "success" && event.get("result").is_some_and(Value::is_string) {
        NativeCompletion::Completed
    } else {
        NativeCompletion::Incomplete
    };
    if let Some(result) = event.get("result") {
        result
            .as_str()
            .ok_or_else(|| invalid("Claude terminal result is malformed"))?
            .clone_into(&mut normalized.text);
    } else if normalized.completion == NativeCompletion::Failed
        && let Some(errors) = event.get("errors").and_then(Value::as_array)
    {
        let errors = errors
            .iter()
            .map(|error| {
                error
                    .as_str()
                    .ok_or_else(|| invalid("Claude terminal errors are malformed"))
            })
            .collect::<Result<Vec<_>>>()?;
        normalized.text = errors.join("\n");
    }
    if let Some(usage) = event.get("usage") {
        if !usage.is_object() {
            return Err(invalid("Claude terminal usage is malformed"));
        }
        normalized.reported_input_tokens = reported_tokens(usage, "input_tokens")?;
        normalized.reported_output_tokens = reported_tokens(usage, "output_tokens")?;
    }
    Ok(())
}

fn reported_tokens(usage: &Value, key: &str) -> Result<Option<u64>> {
    match usage.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| invalid("Claude usage must be an observed nonnegative token count")),
    }
}
