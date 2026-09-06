//! Parses `OpenAI` Chat Completions requests into the canonical request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — OpenAI Chat Completions wire format is dynamic
// JSON.
use serde_json::Value;
use systemprompt_models::wire::inspect::ForwardedSurface;

mod content;
mod params;

use self::content::{flatten_content_text, parse_assistant_message, parse_user_content};
use self::params::{parse_reasoning_effort, parse_response_format, parse_tool, parse_tool_choice};

use super::super::super::canonical::{CanonicalContent, CanonicalMessage, CanonicalRequest, Role};
use super::super::InboundParseError;

const DEFAULT_MAX_TOKENS: u32 = 4096;

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

    // Why: gpt-5/o-series clients send `max_completion_tokens`; the legacy
    // field is still what most OpenAI-compatible tools emit, so accept both.
    let max_tokens = value
        .get("max_completion_tokens")
        .or_else(|| value.get("max_tokens"))
        .and_then(Value::as_u64)
        .map_or(DEFAULT_MAX_TOKENS, |v| v as u32);

    let (system, messages) = parse_messages(value.get("messages"))?;
    let temperature = value
        .get("temperature")
        .and_then(Value::as_f64)
        .map(|v| v as f32);
    let top_p = value.get("top_p").and_then(Value::as_f64).map(|v| v as f32);
    let stop_sequences = value
        .get("stop")
        .and_then(|v| match v {
            Value::String(s) => Some(vec![s.clone()]),
            Value::Array(arr) => Some(
                arr.iter()
                    .filter_map(|x| x.as_str().map(str::to_owned))
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_else(Vec::new);
    let tools = value
        .get("tools")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |arr| {
            arr.iter().filter_map(parse_tool).collect::<Vec<_>>()
        });
    let tool_choice = parse_tool_choice(value)?;
    let stream = value
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let reasoning_effort = value
        .get("reasoning_effort")
        .and_then(Value::as_str)
        .and_then(parse_reasoning_effort);
    let response_format = value.get("response_format").and_then(parse_response_format);
    let presence_penalty = value
        .get("presence_penalty")
        .and_then(Value::as_f64)
        .map(|v| v as f32);
    let frequency_penalty = value
        .get("frequency_penalty")
        .and_then(Value::as_f64)
        .map(|v| v as f32);
    let metadata = value.get("metadata").cloned();

    Ok(CanonicalRequest {
        model,
        system,
        messages,
        max_tokens,
        temperature,
        top_p,
        top_k: None,
        stop_sequences,
        tools,
        tool_choice,
        stream,
        thinking: None,
        metadata,
        response_format,
        reasoning_effort,
        search: None,
        code_execution: false,
        presence_penalty,
        frequency_penalty,
        forwarded_surface: ForwardedSurface::default(),
    })
}

fn parse_messages(
    value: Option<&Value>,
) -> Result<(Option<String>, Vec<CanonicalMessage>), InboundParseError> {
    let Some(arr) = value.and_then(Value::as_array) else {
        return Ok((None, Vec::new()));
    };
    let mut system_parts: Vec<String> = Vec::new();
    let mut messages: Vec<CanonicalMessage> = Vec::new();
    for msg in arr {
        let role = msg.get("role").and_then(Value::as_str).unwrap_or("");
        match role {
            // Why: `developer` is the o-series successor to `system`; both
            // carry instruction text, and canonical has one system slot.
            "system" | "developer" => {
                let text = flatten_content_text(msg.get("content"));
                if !text.is_empty() {
                    system_parts.push(text);
                }
            },
            "user" => messages.push(CanonicalMessage {
                role: Role::User,
                content: parse_user_content(msg.get("content")),
            }),
            "assistant" => messages.push(parse_assistant_message(msg)),
            "tool" => {
                let tool_use_id = msg
                    .get("tool_call_id")
                    .and_then(Value::as_str)
                    .ok_or(InboundParseError::MissingField("tool_call_id"))?
                    .to_owned();
                let text = flatten_content_text(msg.get("content"));
                messages.push(CanonicalMessage {
                    role: Role::Tool,
                    content: vec![CanonicalContent::ToolResult {
                        tool_use_id,
                        content: vec![CanonicalContent::Text(text)],
                        is_error: false,
                        structured_content: None,
                        meta: None,
                    }],
                });
            },
            other => {
                return Err(InboundParseError::Unsupported {
                    field: "messages.role",
                    detail: other.to_owned(),
                });
            },
        }
    }
    let system = if system_parts.is_empty() {
        None
    } else {
        Some(system_parts.join("\n"))
    };
    Ok((system, messages))
}
