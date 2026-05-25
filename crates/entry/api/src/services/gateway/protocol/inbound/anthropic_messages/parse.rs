// JSON: protocol boundary — Anthropic Messages wire format is dynamic JSON.
use serde_json::{Map, Value};

use super::super::super::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalTool, CanonicalToolChoice,
    ImageSource, Role, ThinkingConfig,
};
use super::super::InboundParseError;

pub(super) fn parse(value: &Value) -> Result<CanonicalRequest, InboundParseError> {
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
    })
}

fn parse_system(value: &Value) -> Result<Option<String>, InboundParseError> {
    match value {
        Value::Null => Ok(None),
        Value::String(s) if s.is_empty() => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        Value::Array(arr) => {
            let joined = arr
                .iter()
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n");
            Ok(if joined.is_empty() {
                None
            } else {
                Some(joined)
            })
        },
        other => Err(InboundParseError::Unsupported {
            field: "system",
            detail: format!("expected string or array, got {other}"),
        }),
    }
}

fn parse_message(value: &Value) -> Result<CanonicalMessage, InboundParseError> {
    let role_str = value
        .get("role")
        .and_then(Value::as_str)
        .ok_or(InboundParseError::MissingField("messages[].role"))?;
    let role = match role_str {
        "user" => Role::User,
        "assistant" => Role::Assistant,
        "system" => Role::System,
        "tool" => Role::Tool,
        other => {
            return Err(InboundParseError::Unsupported {
                field: "messages[].role",
                detail: other.to_owned(),
            });
        },
    };
    let content_value = value
        .get("content")
        .ok_or(InboundParseError::MissingField("messages[].content"))?;
    let content = parse_content(content_value)?;
    Ok(CanonicalMessage { role, content })
}

fn parse_content(value: &Value) -> Result<Vec<CanonicalContent>, InboundParseError> {
    match value {
        Value::String(s) => Ok(vec![CanonicalContent::Text(s.clone())]),
        Value::Array(blocks) => blocks.iter().map(parse_content_block).collect(),
        other => Err(InboundParseError::Unsupported {
            field: "messages[].content",
            detail: format!("unexpected shape: {other}"),
        }),
    }
}

fn parse_content_block(value: &Value) -> Result<CanonicalContent, InboundParseError> {
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("text");
    match kind {
        "text" => Ok(CanonicalContent::Text(
            value
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
        )),
        "image" => parse_image(value),
        "tool_use" => Ok(CanonicalContent::ToolUse {
            id: value
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            name: value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            input: value.get("input").cloned().unwrap_or(Value::Null),
        }),
        "tool_result" => {
            let inner = value
                .get("content")
                .map_or_else(Vec::new, parse_tool_result_content);
            Ok(CanonicalContent::ToolResult {
                tool_use_id: value
                    .get("tool_use_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                content: inner,
                is_error: value
                    .get("is_error")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            })
        },
        "thinking" => Ok(CanonicalContent::Thinking {
            text: value
                .get("thinking")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            signature: value
                .get("signature")
                .and_then(Value::as_str)
                .map(str::to_owned),
        }),
        other => Err(InboundParseError::Unsupported {
            field: "messages[].content[].type",
            detail: other.to_owned(),
        }),
    }
}

fn parse_tool_result_content(value: &Value) -> Vec<CanonicalContent> {
    match value {
        Value::String(s) => vec![CanonicalContent::Text(s.clone())],
        Value::Array(arr) => arr
            .iter()
            .filter_map(|v| parse_content_block(v).ok())
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_image(value: &Value) -> Result<CanonicalContent, InboundParseError> {
    let source = value
        .get("source")
        .ok_or(InboundParseError::MissingField("image.source"))?;
    let kind = source
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("base64");
    match kind {
        "base64" => Ok(CanonicalContent::Image(ImageSource::Base64 {
            media_type: source
                .get("media_type")
                .and_then(Value::as_str)
                .unwrap_or("image/png")
                .to_owned(),
            data: source
                .get("data")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
        })),
        "url" => Ok(CanonicalContent::Image(ImageSource::Url(
            source
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
        ))),
        other => Err(InboundParseError::Unsupported {
            field: "image.source.type",
            detail: other.to_owned(),
        }),
    }
}

fn parse_tool(value: &Value) -> CanonicalTool {
    CanonicalTool {
        name: value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        description: value
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_owned),
        input_schema: value
            .get("input_schema")
            .cloned()
            .unwrap_or(Value::Object(Map::new())),
    }
}

fn parse_tool_choice(value: &Value) -> Option<CanonicalToolChoice> {
    if let Some(s) = value.as_str() {
        return match s {
            "auto" => Some(CanonicalToolChoice::Auto),
            "any" => Some(CanonicalToolChoice::Any),
            "none" => Some(CanonicalToolChoice::None),
            "required" => Some(CanonicalToolChoice::Required),
            _ => None,
        };
    }
    let kind = value.get("type").and_then(Value::as_str)?;
    match kind {
        "auto" => Some(CanonicalToolChoice::Auto),
        "any" => Some(CanonicalToolChoice::Any),
        "none" => Some(CanonicalToolChoice::None),
        "required" => Some(CanonicalToolChoice::Required),
        "tool" => value
            .get("name")
            .and_then(Value::as_str)
            .map(|n| CanonicalToolChoice::Tool(n.to_owned())),
        _ => None,
    }
}

fn parse_thinking(value: &Value) -> ThinkingConfig {
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    let enabled = kind == "enabled";
    let budget_tokens = value
        .get("budget_tokens")
        .and_then(Value::as_u64)
        .map(|v| v as u32);
    ThinkingConfig {
        enabled,
        budget_tokens,
    }
}
