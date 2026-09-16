//! Hermes wrapper events preserve native streams, process status and unverified
//! usage.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::{NativeCompletion, NormalizedClientOutput, invalid, malformed};
use serde_json::Value;
use std::collections::BTreeMap;
use systemprompt_evaluation::Result;

pub(super) fn normalize(bytes: &[u8]) -> Result<NormalizedClientOutput> {
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(invalid("Hermes evidence exceeds 16 MiB"));
    }
    let body = std::str::from_utf8(bytes)
        .map_err(|error| malformed("Hermes evidence is not UTF-8", error))?;
    let mut events = BTreeMap::new();
    let mut terminal: Option<Value> = None;
    let mut text = String::new();
    let mut failed = false;
    for (index, line) in body.lines().enumerate() {
        if index >= 8192 || line.len() > 1024 * 1024 {
            return Err(invalid("Hermes event stream exceeds event bounds"));
        }
        if line.trim().is_empty() {
            continue;
        }
        // JSON: the pinned wrapper retains native stdout/stderr and the native
        // usage-file payload.
        let event: Value = serde_json::from_str(line)
            .map_err(|error| malformed("Malformed Hermes wrapper event", error))?;
        let kind = event
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("Hermes event has no type"))?;
        if kind == "hermes.launch_error" {
            failed = true;
            continue;
        }
        let sequence = event
            .get("sequence")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("Hermes event sequence is missing"))?;
        if let Some(existing) = events.get(&sequence) {
            if existing != &event {
                return Err(invalid("Conflicting Hermes stream evidence"));
            }
            continue;
        }
        if sequence != events.len() as u64 {
            return Err(invalid("Hermes stream evidence has a sequence gap"));
        }
        if terminal.is_some() {
            return Err(invalid("Hermes stream continues after terminal evidence"));
        }
        match kind {
            "hermes.stdout" => text.push_str(
                event
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("Invalid Hermes stdout event"))?,
            ),
            "hermes.stderr" | "hermes.diagnostic" => {},
            "hermes.result" => terminal = Some(event.clone()),
            _ => return Err(invalid("Unknown Hermes wrapper event")),
        }
        events.insert(sequence, event);
    }
    let mut output = NormalizedClientOutput {
        text,
        completion: NativeCompletion::Incomplete,
        reported_input_tokens: None,
        reported_output_tokens: None,
        tool_calls: Vec::new(),
    };
    if let Some(result) = terminal {
        failed |= apply_result(&result, &mut output)?;
    }
    if failed {
        output.completion = NativeCompletion::Failed;
    }
    output.validate()?;
    Ok(output)
}

fn apply_result(result: &Value, output: &mut NormalizedClientOutput) -> Result<bool> {
    let exit = result.get("process_exit_code").and_then(Value::as_i64);
    let mut failed = exit.is_some_and(|code| code != 0)
        || !result.get("signal").is_some_and(Value::is_null)
        || result.get("output_limit_reached").and_then(Value::as_bool) == Some(true);
    if result
        .get("usage_independently_verified")
        .and_then(Value::as_bool)
        != Some(false)
    {
        return Err(invalid(
            "Hermes usage must not claim independent verification",
        ));
    }
    if let Some(usage) = result.get("usage").filter(|value| !value.is_null()) {
        if !usage.is_object() {
            return Err(invalid("Invalid Hermes usage evidence"));
        }
        output.reported_input_tokens = usage_field(usage, "input_tokens")?;
        output.reported_output_tokens = usage_field(usage, "output_tokens")?;
        if usage.get("failed").and_then(Value::as_bool) == Some(true) {
            failed = true;
        }
        if !failed
            && exit == Some(0)
            && usage.get("completed").and_then(Value::as_bool) == Some(true)
            && usage.get("failed").and_then(Value::as_bool) == Some(false)
            && result
                .get("usage_identity_matches")
                .and_then(Value::as_bool)
                == Some(true)
            && result.get("output_limit_reached").and_then(Value::as_bool) == Some(false)
        {
            output.completion = NativeCompletion::Completed;
        }
    }
    if let Some(tools) = result.get("tool_calls") {
        collect_tools(tools, &mut output.tool_calls)?;
    }
    Ok(failed)
}

fn collect_tools(tools: &Value, tool_calls: &mut Vec<String>) -> Result<()> {
    let tools = tools
        .as_array()
        .ok_or_else(|| invalid("Invalid Hermes tool evidence"))?;
    for tool in tools {
        let name = tool
            .as_str()
            .ok_or_else(|| invalid("Invalid Hermes tool name"))?;
        if ![
            "read_file",
            "write_file",
            "patch",
            "search_files",
            "mcp__evaluation_fixture__evaluation_fixture",
        ]
        .contains(&name)
        {
            return Err(invalid("Hermes emitted forbidden tool evidence"));
        }
        tool_calls.push(name.to_owned());
    }
    Ok(())
}
fn usage_field(usage: &Value, key: &str) -> Result<Option<u64>> {
    usage
        .get(key)
        .filter(|value| !value.is_null())
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| invalid("Hermes usage must be nonnegative integers"))
        })
        .transpose()
}
