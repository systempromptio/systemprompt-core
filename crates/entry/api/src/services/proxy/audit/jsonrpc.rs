//! Minimal JSON-RPC / MCP frame parsing for the tool-call audit tap.
//!
//! The gateway forwards MCP frames verbatim; to audit a `tools/call` it parses
//! the tool name and arguments from the request and the result from the
//! response, matching them by JSON-RPC id. The `arguments`, `result`, and
//! `content` payloads are `serde_json::Value` because MCP defines them as
//! open-shaped at the wire boundary. The matching response frame is also
//! stamped with the execution id the tap minted, under the systemprompt
//! `_meta` key, so a client that reports the result later carries the exact
//! server key.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use serde_json::{Value, json};
use systemprompt_models::artifacts::EXECUTION_META_KEY;

const TOOLS_CALL_METHOD: &str = "tools/call";

#[derive(Deserialize)]
struct RequestFrame {
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<ToolCallParams>,
}

#[derive(Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Option<Value>,
}

#[derive(Debug)]
pub struct ToolCallInvocation {
    pub id: Value,
    pub tool_name: String,
    pub arguments: Value,
}

pub fn parse_tool_call(body: &[u8]) -> Option<ToolCallInvocation> {
    let frame: RequestFrame = serde_json::from_slice(body).ok()?;
    if frame.method != TOOLS_CALL_METHOD {
        return None;
    }
    let params = frame.params?;
    Some(ToolCallInvocation {
        id: frame.id.unwrap_or(Value::Null),
        tool_name: params.name,
        arguments: params.arguments.unwrap_or(Value::Null),
    })
}

#[derive(Deserialize)]
struct ResponseFrame {
    #[serde(default)]
    result: Option<ToolCallResult>,
    #[serde(default)]
    error: Option<Value>,
}

#[derive(Deserialize)]
struct ToolCallResult {
    #[serde(default, rename = "isError")]
    is_error: bool,
    #[serde(default, rename = "structuredContent")]
    structured_content: Option<Value>,
    #[serde(default)]
    content: Option<Value>,
}

#[derive(Debug)]
pub struct ToolCallOutcome {
    pub output: Option<Value>,
    pub error_message: Option<String>,
    pub result: Option<Value>,
}

pub fn parse_response_frame(data: &str, request_id: &Value) -> Option<ToolCallOutcome> {
    let frame: Value = serde_json::from_str(data).ok()?;
    if frame.get("id") != Some(request_id) {
        return None;
    }
    let parsed: ResponseFrame = serde_json::from_value(frame.clone()).ok()?;
    if let Some(error) = parsed.error {
        return Some(ToolCallOutcome {
            error_message: Some(error.to_string()),
            output: Some(error),
            result: None,
        });
    }
    let result = parsed.result?;
    let output = result.structured_content.or(result.content);
    let error_message = result
        .is_error
        .then(|| "MCP tool call returned isError".to_owned());
    Some(ToolCallOutcome {
        output,
        error_message,
        result: frame.get("result").cloned(),
    })
}

pub fn frame_matches(data: &str, request_id: &Value) -> bool {
    serde_json::from_str::<Value>(data)
        .ok()
        .is_some_and(|frame| frame.get("id") == Some(request_id))
}

pub fn stamp_execution(data: &str, mcp_execution_id: &str) -> Option<String> {
    let mut frame: Value = serde_json::from_str(data).ok()?;
    let result = frame.get_mut("result")?.as_object_mut()?;
    let meta = result
        .entry("_meta")
        .or_insert_with(|| json!({}))
        .as_object_mut()?;
    let execution = meta
        .entry(EXECUTION_META_KEY)
        .or_insert_with(|| json!({}))
        .as_object_mut()?;
    execution
        .entry("mcp_execution_id")
        .or_insert_with(|| Value::String(mcp_execution_id.to_owned()));
    serde_json::to_string(&frame).ok()
}

pub fn extract_sse_data(frame: &str) -> Option<String> {
    let mut data = String::new();
    for line in frame.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.trim_start());
        }
    }
    (!data.is_empty()).then_some(data)
}

pub fn replace_sse_data(frame: &str, data: &str) -> String {
    let mut out = String::with_capacity(frame.len() + data.len());
    let mut wrote = false;
    for line in frame.lines() {
        if line.starts_with("data:") {
            if !wrote {
                out.push_str("data: ");
                out.push_str(data);
                out.push('\n');
                wrote = true;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !wrote {
        out.push_str("data: ");
        out.push_str(data);
        out.push('\n');
    }
    out.push('\n');
    out
}
