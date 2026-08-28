//! Request-parameter parsing: tools, tool choice, reasoning effort, and
//! response format.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — OpenAI Chat Completions wire format is dynamic
// JSON.
use serde_json::{Map, Value};

use super::super::super::super::canonical::{
    CanonicalTool, CanonicalToolChoice, ReasoningEffort, ResponseFormat,
};

pub(super) fn parse_tool(value: &Value) -> Option<CanonicalTool> {
    if value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("function")
        != "function"
    {
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

pub(super) fn parse_tool_choice(value: &Value) -> Option<CanonicalToolChoice> {
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

pub(super) fn parse_reasoning_effort(s: &str) -> Option<ReasoningEffort> {
    match s {
        // Why: `minimal` is a valid OpenAI value with no canonical tier; folding
        // it into Low keeps the caller's intent instead of dropping the field.
        "minimal" | "low" => Some(ReasoningEffort::Low),
        "medium" => Some(ReasoningEffort::Medium),
        "high" => Some(ReasoningEffort::High),
        _ => None,
    }
}

pub(super) fn parse_response_format(value: &Value) -> Option<ResponseFormat> {
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
                schema: spec
                    .get("schema")
                    .cloned()
                    .unwrap_or(Value::Object(Map::new())),
                strict: spec.get("strict").and_then(Value::as_bool).unwrap_or(false),
            })
        },
        _ => None,
    }
}
