//! Parses `OpenAI` Chat Completions requests into the canonical request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — OpenAI Chat Completions wire format is dynamic
// JSON.
use serde_json::{Map, Value};
use systemprompt_models::wire::inspect::ForwardedSurface;

use super::super::super::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalTool, CanonicalToolChoice,
    ImageDetail, ImageSource, ReasoningEffort, ResponseFormat, Role,
};
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
    let tool_choice = value.get("tool_choice").and_then(parse_tool_choice);
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

fn parse_user_content(value: Option<&Value>) -> Vec<CanonicalContent> {
    match value {
        Some(Value::String(s)) => vec![CanonicalContent::Text(s.clone())],
        Some(Value::Array(parts)) => parts.iter().filter_map(parse_user_part).collect(),
        _ => Vec::new(),
    }
}

fn parse_user_part(part: &Value) -> Option<CanonicalContent> {
    match part.get("type").and_then(Value::as_str)? {
        "text" => part
            .get("text")
            .and_then(Value::as_str)
            .map(|t| CanonicalContent::Text(t.to_owned())),
        "image_url" => {
            let image = part.get("image_url")?;
            let url = image.get("url").and_then(Value::as_str)?.to_owned();
            let detail = image
                .get("detail")
                .and_then(Value::as_str)
                .and_then(parse_image_detail);
            Some(CanonicalContent::Image(parse_image_source(url, detail)))
        },
        _ => None,
    }
}

fn parse_image_source(url: String, detail: Option<ImageDetail>) -> ImageSource {
    // Why: data URIs must round-trip to providers (Anthropic) that only accept
    // base64 source blocks, so split them back apart here.
    if let Some(rest) = url.strip_prefix("data:")
        && let Some((media_type, data)) = rest.split_once(";base64,")
    {
        return ImageSource::Base64 {
            media_type: media_type.to_owned(),
            data: data.to_owned(),
            detail,
        };
    }
    ImageSource::Url { url, detail }
}

fn parse_image_detail(s: &str) -> Option<ImageDetail> {
    match s {
        "auto" => Some(ImageDetail::Auto),
        "low" => Some(ImageDetail::Low),
        "high" => Some(ImageDetail::High),
        _ => None,
    }
}

fn parse_assistant_message(msg: &Value) -> CanonicalMessage {
    let mut content: Vec<CanonicalContent> = Vec::new();
    let text = flatten_content_text(msg.get("content"));
    if !text.is_empty() {
        content.push(CanonicalContent::Text(text));
    }
    if let Some(calls) = msg.get("tool_calls").and_then(Value::as_array) {
        for call in calls {
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let function = call.get("function");
            let name = function
                .and_then(|f| f.get("name"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let args = function
                .and_then(|f| f.get("arguments"))
                .and_then(Value::as_str)
                .unwrap_or("{}");
            // JSON: tool-call arguments are a user-defined schema instance; the
            // canonical model carries them as an opaque JSON value.
            let input: Value =
                serde_json::from_str(args).unwrap_or_else(|_| Value::Object(Map::new()));
            content.push(CanonicalContent::ToolUse {
                id,
                name,
                input,
                signature: None,
            });
        }
    }
    CanonicalMessage {
        role: Role::Assistant,
        content,
    }
}

fn flatten_content_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn parse_tool(value: &Value) -> Option<CanonicalTool> {
    if value.get("type").and_then(Value::as_str).unwrap_or("function") != "function" {
        return None;
    }
    let function = value.get("function")?;
    let name = function.get("name").and_then(Value::as_str)?;
    let description = function
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let parameters = function
        .get("parameters")
        .cloned()
        .unwrap_or(Value::Object(Map::new()));
    Some(CanonicalTool {
        name: name.to_owned(),
        description,
        input_schema: parameters,
    })
}

fn parse_tool_choice(value: &Value) -> Option<CanonicalToolChoice> {
    if let Some(s) = value.as_str() {
        return match s {
            "auto" => Some(CanonicalToolChoice::Auto),
            "none" => Some(CanonicalToolChoice::None),
            "required" => Some(CanonicalToolChoice::Required),
            _ => None,
        };
    }
    if value.get("type").and_then(Value::as_str)? == "function" {
        return value
            .get("function")
            .and_then(|f| f.get("name"))
            .and_then(Value::as_str)
            .map(|n| CanonicalToolChoice::Tool(n.to_owned()));
    }
    None
}

fn parse_reasoning_effort(s: &str) -> Option<ReasoningEffort> {
    match s {
        // Why: `minimal` is a valid OpenAI value with no canonical tier; folding
        // it into Low keeps the caller's intent instead of dropping the field.
        "minimal" | "low" => Some(ReasoningEffort::Low),
        "medium" => Some(ReasoningEffort::Medium),
        "high" => Some(ReasoningEffort::High),
        _ => None,
    }
}

fn parse_response_format(value: &Value) -> Option<ResponseFormat> {
    match value.get("type").and_then(Value::as_str)? {
        "json_object" => Some(ResponseFormat::JsonObject),
        "json_schema" => {
            let spec = value.get("json_schema")?;
            Some(ResponseFormat::JsonSchema {
                name: spec
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("response")
                    .to_owned(),
                schema: spec.get("schema").cloned().unwrap_or(Value::Object(Map::new())),
                strict: spec.get("strict").and_then(Value::as_bool).unwrap_or(false),
            })
        },
        _ => None,
    }
}
