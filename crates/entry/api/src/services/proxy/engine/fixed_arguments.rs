//! Instance-fixed tool arguments for external MCP servers.
//!
//! A `tools/call` bound for an external server is rewritten so that every
//! argument the deployment fixes for that tool (`tools.<name>.arguments` in
//! the server's yaml) carries the configured value, whatever the client sent.
//! It runs before governance and audit, so the call that is judged and
//! recorded is the call that reaches the upstream.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_models::mcp::deployment::ToolMetadata;

/// Rewrites `body` in place when it is a `tools/call` whose tool has fixed
/// arguments. Anything that is not such a call, including an unparsable body,
/// is left untouched for the upstream to reject on its own terms.
pub fn apply(tools: &HashMap<String, ToolMetadata>, body: &mut Vec<u8>) {
    if tools.values().all(|tool| tool.arguments.is_empty()) || body.is_empty() {
        return;
    }
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return;
    };
    if value.get("method").and_then(serde_json::Value::as_str) != Some("tools/call") {
        return;
    }
    let Some(params) = value.get_mut("params").and_then(serde_json::Value::as_object_mut) else {
        return;
    };
    let Some(fixed) = params
        .get("name")
        .and_then(serde_json::Value::as_str)
        .and_then(|name| tools.get(name))
        .filter(|tool| !tool.arguments.is_empty())
        .map(|tool| tool.arguments.clone())
    else {
        return;
    };
    let arguments = params
        .entry("arguments")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    if !arguments.is_object() {
        *arguments = serde_json::Value::Object(serde_json::Map::new());
    }
    if let Some(arguments) = arguments.as_object_mut() {
        for (key, fixed_value) in fixed {
            arguments.insert(key, fixed_value);
        }
    }
    if let Ok(rewritten) = serde_json::to_vec(&value) {
        *body = rewritten;
    }
}
