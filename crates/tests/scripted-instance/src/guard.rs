//! The guard of a turn: whether a model request is the one the turn was
//! written for. It reads both request shapes, Messages and Chat
//! Completions.

use serde_json::Value;

use crate::script::Expect;

pub(crate) fn check(expect: &Expect, request: &Value) -> Result<(), String> {
    let tools = advertised_tools(request);
    let results = tool_results(request);
    let satisfied = expect
        .tools_include
        .iter()
        .all(|name| tools.iter().any(|tool| tool == name))
        && expect
            .tool_results
            .iter()
            .all(|id| results.iter().any(|result| result == id));
    if satisfied {
        return Ok(());
    }
    Err(format!(
        "the turn expects tools {:?} and tool results {:?}; the request advertises tools {tools:?} \
         and carries tool results {results:?}",
        expect.tools_include, expect.tool_results
    ))
}

fn advertised_tools(request: &Value) -> Vec<String> {
    list(request, "tools")
        .iter()
        .filter_map(|tool| {
            tool.get("name")
                .or_else(|| {
                    tool.get("function")
                        .and_then(|function| function.get("name"))
                })
                .and_then(Value::as_str)
        })
        .map(str::to_owned)
        .collect()
}

fn tool_results(request: &Value) -> Vec<String> {
    let mut ids = Vec::new();
    for message in list(request, "messages") {
        if message.get("role").and_then(Value::as_str) == Some("tool")
            && let Some(id) = message.get("tool_call_id").and_then(Value::as_str)
        {
            ids.push(id.to_owned());
        }
        for block in list(message, "content") {
            if block.get("type").and_then(Value::as_str) == Some("tool_result")
                && let Some(id) = block.get("tool_use_id").and_then(Value::as_str)
            {
                ids.push(id.to_owned());
            }
        }
    }
    ids
}

fn list<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}
