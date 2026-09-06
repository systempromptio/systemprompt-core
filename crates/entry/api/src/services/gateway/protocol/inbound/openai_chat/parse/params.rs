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
use super::super::super::InboundParseError;

// Why: rejection detail for a `tool_choice` outside the Chat Completions
// grammar.
const TOOL_CHOICE_EXPECTED: &str =
    "expected \"none\", \"auto\", \"required\", or an object with type function";

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

// Why: Chat Completions accepts three strings or a `function` object; anything
// else is a client bug that the upstream API rejects, so it must not reach
// dispatch as a silently dropped field.
pub(super) fn parse_tool_choice(
    request: &Value,
) -> Result<Option<CanonicalToolChoice>, InboundParseError> {
    request
        .get("tool_choice")
        .map(parse_present_tool_choice)
        .transpose()
}

fn parse_present_tool_choice(value: &Value) -> Result<CanonicalToolChoice, InboundParseError> {
    let unsupported = || InboundParseError::Unsupported {
        field: "tool_choice",
        detail: TOOL_CHOICE_EXPECTED.to_owned(),
    };
    if let Some(s) = value.as_str() {
        return match s {
            "auto" => Ok(CanonicalToolChoice::Auto),
            "none" => Ok(CanonicalToolChoice::None),
            "required" => Ok(CanonicalToolChoice::Required),
            _ => Err(unsupported()),
        };
    }
    if value.get("type").and_then(Value::as_str) != Some("function") {
        return Err(unsupported());
    }
    value
        .get("function")
        .and_then(|f| f.get("name"))
        .and_then(Value::as_str)
        .map(|n| CanonicalToolChoice::Tool(n.to_owned()))
        .ok_or_else(|| InboundParseError::Unsupported {
            field: "tool_choice",
            detail: "expected a `function.name` for tool_choice type function".to_owned(),
        })
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
