//! Anthropic Messages request parsing into the canonical request shape.
//!
//! [`parse`] reads the top-level request fields; message and content-block
//! parsing lives in [`content`], and tool/tool-choice/thinking parsing in
//! [`tools`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod content;
mod tools;

// JSON: protocol boundary — Anthropic Messages wire format is dynamic JSON.
use serde_json::Value;

use super::super::super::canonical::CanonicalRequest;
use super::super::InboundParseError;
use content::{parse_message, parse_system};
use tools::{parse_thinking, parse_tool, parse_tool_choice};

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn parse(value: &Value) -> Result<CanonicalRequest, InboundParseError> {
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .ok_or(InboundParseError::MissingField("model"))?
        .to_owned();
    let max_tokens = value
        .get("max_tokens")
        .and_then(Value::as_u64)
        .ok_or(InboundParseError::MissingField("max_tokens"))? as u32;

    let system = value.get("system").map(parse_system).transpose()?.flatten();

    let messages = value
        .get("messages")
        .and_then(Value::as_array)
        .ok_or(InboundParseError::MissingField("messages"))?
        .iter()
        .map(parse_message)
        .collect::<Result<Vec<_>, _>>()?;

    let temperature = value
        .get("temperature")
        .and_then(Value::as_f64)
        .map(|v| v as f32);
    let top_p = value.get("top_p").and_then(Value::as_f64).map(|v| v as f32);
    let top_k = value.get("top_k").and_then(Value::as_i64).map(|v| v as i32);

    let stop_sequences = value
        .get("stop_sequences")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        });

    let tools = value
        .get("tools")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |arr| {
            arr.iter().map(parse_tool).collect::<Vec<_>>()
        });

    let tool_choice = value.get("tool_choice").and_then(parse_tool_choice);

    let stream = value
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let thinking = value.get("thinking").map(parse_thinking);

    let metadata = value.get("metadata").cloned();

    Ok(CanonicalRequest {
        model,
        system,
        messages,
        max_tokens,
        temperature,
        top_p,
        top_k,
        stop_sequences,
        tools,
        tool_choice,
        stream,
        thinking,
        metadata,
        response_format: None,
        reasoning_effort: None,
        search: None,
        code_execution: false,
        presence_penalty: None,
        frequency_penalty: None,
    })
}
