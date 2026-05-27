// JSON: protocol boundary — OpenAI Responses wire format is dynamic JSON.
use serde_json::{Map, Value};

use super::super::super::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalTool, CanonicalToolChoice,
    ImageSource, Role, ThinkingConfig,
};
use super::super::InboundParseError;

const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 4096;

#[cfg_attr(not(feature = "test-api"), allow(unreachable_pub))]
pub fn parse(value: &Value) -> Result<CanonicalRequest, InboundParseError> {
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .ok_or(InboundParseError::MissingField("model"))?
        .to_owned();

    let max_tokens = value
        .get("max_output_tokens")
        .and_then(Value::as_u64)
        .map_or(DEFAULT_MAX_OUTPUT_TOKENS, |v| v as u32);
    let system = value
        .get("instructions")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let messages = value
        .get("input")
        .map(parse_input)
        .transpose()?
        .unwrap_or_else(Vec::new);
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
    let thinking = value.get("reasoning").map(parse_reasoning);
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
        thinking,
        metadata,
    })
}

fn parse_input(value: &Value) -> Result<Vec<CanonicalMessage>, InboundParseError> {
    let arr = match value {
        Value::String(s) => {
            return Ok(vec![CanonicalMessage {
                role: Role::User,
                content: vec![CanonicalContent::Text(s.clone())],
            }]);
        },
        Value::Array(a) => a,
        other => {
            return Err(InboundParseError::Unsupported {
                field: "input",
                detail: format!("expected string or array, got {other}"),
            });
        },
    };

    let mut messages: Vec<CanonicalMessage> = Vec::new();
    for item in arr {
        let kind = item
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("message");
        match kind {
            "message" => messages.push(parse_message_item(item)?),
            "function_call" => messages.push(parse_function_call(item)),
            "function_call_output" => messages.push(parse_function_call_output(item)),
            "reasoning" => {
                if let Some(msg) = parse_reasoning_item(item) {
                    messages.push(msg);
                }
            },
            other => {
                return Err(InboundParseError::Unsupported {
                    field: "input[].type",
                    detail: other.to_owned(),
                });
            },
        }
    }
    Ok(messages)
}

fn parse_function_call(item: &Value) -> CanonicalMessage {
    let id = item
        .get("call_id")
        .and_then(Value::as_str)
        .or_else(|| item.get("id").and_then(Value::as_str))
        .unwrap_or("")
        .to_owned();
    let name = item
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let arguments = item
        .get("arguments")
        .and_then(Value::as_str)
        .unwrap_or("{}");
    let input: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    CanonicalMessage {
        role: Role::Assistant,
        content: vec![CanonicalContent::ToolUse { id, name, input }],
    }
}

fn parse_function_call_output(item: &Value) -> CanonicalMessage {
    let tool_use_id = item
        .get("call_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let output_text = item
        .get("output")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    CanonicalMessage {
        role: Role::Tool,
        content: vec![CanonicalContent::ToolResult {
            tool_use_id,
            content: vec![CanonicalContent::Text(output_text)],
            is_error: false,
        }],
    }
}

fn parse_reasoning_item(item: &Value) -> Option<CanonicalMessage> {
    let text = item
        .get("summary")
        .and_then(Value::as_array)
        .map_or_else(String::new, |arr| {
            arr.iter()
                .filter_map(|v| v.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        });
    if text.is_empty() {
        return None;
    }
    Some(CanonicalMessage {
        role: Role::Assistant,
        content: vec![CanonicalContent::Thinking {
            text,
            signature: None,
        }],
    })
}

fn parse_message_item(value: &Value) -> Result<CanonicalMessage, InboundParseError> {
    let role_str = value.get("role").and_then(Value::as_str).unwrap_or("user");
    let role = match role_str {
        "user" => Role::User,
        "assistant" => Role::Assistant,
        "system" | "developer" => Role::System,
        other => {
            return Err(InboundParseError::Unsupported {
                field: "input[].role",
                detail: other.to_owned(),
            });
        },
    };
    let content_value = value.get("content").unwrap_or(&Value::Null);
    let content = match content_value {
        Value::String(s) => vec![CanonicalContent::Text(s.clone())],
        Value::Array(parts) => parts.iter().filter_map(parse_content_part).collect(),
        Value::Null => Vec::new(),
        other => {
            return Err(InboundParseError::Unsupported {
                field: "input[].content",
                detail: format!("unexpected: {other}"),
            });
        },
    };
    Ok(CanonicalMessage { role, content })
}

fn parse_content_part(value: &Value) -> Option<CanonicalContent> {
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        "input_text" | "output_text" | "text" => Some(CanonicalContent::Text(
            value
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
        )),
        "input_image" => {
            let url = value.get("image_url").and_then(Value::as_str)?;
            Some(CanonicalContent::Image(ImageSource::Url(url.to_owned())))
        },
        _ => None,
    }
}

fn parse_tool(value: &Value) -> Option<CanonicalTool> {
    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("function");
    if kind != "function" {
        return None;
    }
    let name = value.get("name").and_then(Value::as_str)?;
    let description = value
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let parameters = value
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
    let kind = value.get("type").and_then(Value::as_str)?;
    if kind == "function" {
        return value
            .get("name")
            .and_then(Value::as_str)
            .map(|n| CanonicalToolChoice::Tool(n.to_owned()));
    }
    None
}

fn parse_reasoning(value: &Value) -> ThinkingConfig {
    let effort = value.get("effort").and_then(Value::as_str).unwrap_or("");
    let enabled = !effort.is_empty();
    let budget_tokens = match effort {
        "low" => Some(1024),
        "medium" => Some(4096),
        "high" => Some(16384),
        _ => None,
    };
    ThinkingConfig {
        enabled,
        budget_tokens,
    }
}
