// JSON: protocol boundary — OpenAI Responses wire format is dynamic JSON.
use serde_json::{Map, Value};

use super::super::super::canonical::{
    CanonicalRequest, CanonicalTool, CanonicalToolChoice, ThinkingConfig,
};
use super::super::InboundParseError;
use super::input::parse_input;

const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 4096;

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
        response_format: None,
        reasoning_effort: None,
        search: None,
        code_execution: false,
        presence_penalty: None,
        frequency_penalty: None,
    })
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
