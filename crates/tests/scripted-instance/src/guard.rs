//! The guard of a turn: whether a model request is the one the turn was
//! written for, the system prompt an aside is recognised by, and whether a
//! request is a session-title call. It reads both request shapes, Messages
//! and Chat Completions.

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

/// The corpus's rule for a session-title call: no tool advertised and
/// exactly one message besides system ones, from the user.
pub(crate) fn is_title_call(request: &Value) -> bool {
    let mut conversation = list(request, "messages").iter().filter(|message| {
        !matches!(
            message.get("role").and_then(Value::as_str),
            Some("system" | "developer")
        )
    });
    let only_user = conversation
        .next()
        .is_some_and(|first| first.get("role").and_then(Value::as_str) == Some("user"));
    list(request, "tools").is_empty() && only_user && conversation.next().is_none()
}

pub(crate) fn system_text(request: &Value) -> String {
    let mut parts = Vec::new();
    collect_text(request.get("system"), &mut parts);
    for message in list(request, "messages") {
        if matches!(
            message.get("role").and_then(Value::as_str),
            Some("system" | "developer")
        ) {
            collect_text(message.get("content"), &mut parts);
        }
    }
    parts.join("\n")
}

fn collect_text<'a>(content: Option<&'a Value>, parts: &mut Vec<&'a str>) {
    match content {
        Some(Value::String(text)) => parts.push(text),
        Some(Value::Array(blocks)) => parts.extend(
            blocks
                .iter()
                .filter_map(|block| block.get("text").and_then(Value::as_str)),
        ),
        _ => {},
    }
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
