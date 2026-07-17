//! Minimal JSON-RPC / MCP frame parsing for the tool-call audit tap.
//!
//! The gateway forwards MCP frames verbatim; to audit a `tools/call` it parses
//! the tool name and arguments from the request and the result from the
//! response, matching them by JSON-RPC id. The `arguments`, `result`, and
//! `content` payloads are `serde_json::Value` because MCP defines them as
//! open-shaped at the wire boundary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use serde_json::Value;

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

pub(crate) struct ToolCallInvocation {
    pub(super) id: Value,
    pub(super) tool_name: String,
    pub(super) arguments: Value,
}

pub(crate) fn parse_tool_call(body: &[u8]) -> Option<ToolCallInvocation> {
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
    id: Option<Value>,
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

pub(super) struct ToolCallOutcome {
    pub(super) output: Option<Value>,
    pub(super) error_message: Option<String>,
}

pub(super) fn parse_response_frame(data: &str, request_id: &Value) -> Option<ToolCallOutcome> {
    let frame: ResponseFrame = serde_json::from_str(data).ok()?;
    if frame.id.as_ref() != Some(request_id) {
        return None;
    }
    if let Some(error) = frame.error {
        return Some(ToolCallOutcome {
            error_message: Some(error.to_string()),
            output: Some(error),
        });
    }
    let result = frame.result?;
    let output = result.structured_content.or(result.content);
    let error_message = result
        .is_error
        .then(|| "MCP tool call returned isError".to_owned());
    Some(ToolCallOutcome {
        output,
        error_message,
    })
}

pub(super) fn extract_sse_data(frame: &str) -> Option<String> {
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
