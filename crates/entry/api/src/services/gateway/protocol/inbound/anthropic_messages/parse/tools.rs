//! Tool, tool-choice, and thinking-config parsing for the Anthropic Messages
//! wire format.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — Anthropic Messages wire format is dynamic JSON.
use serde_json::{Map, Value};

use crate::services::gateway::protocol::canonical::{
    CanonicalTool, CanonicalToolChoice, ThinkingConfig,
};
use crate::services::gateway::protocol::inbound::InboundParseError;

// Why: rejection detail for a `tool_choice` that is not an object of a known
// type.
const TOOL_CHOICE_EXPECTED: &str = "expected an object with type auto|any|tool";

pub(super) fn parse_tool(value: &Value) -> CanonicalTool {
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

// Why: the Anthropic Messages contract defines `tool_choice` as an object; a
// bare string (the OpenAI form) or an unknown `type` is a client bug, and the
// upstream API answers it with a 400 rather than silently ignoring the field.
pub(super) fn parse_tool_choice(value: &Value) -> Result<CanonicalToolChoice, InboundParseError> {
    let unsupported = || InboundParseError::Unsupported {
        field: "tool_choice",
        detail: TOOL_CHOICE_EXPECTED.to_owned(),
    };
    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(unsupported)?;
    match kind {
        "auto" => Ok(CanonicalToolChoice::Auto),
        "any" => Ok(CanonicalToolChoice::Any),
        "none" => Ok(CanonicalToolChoice::None),
        "required" => Ok(CanonicalToolChoice::Required),
        "tool" => value
            .get("name")
            .and_then(Value::as_str)
            .map(|n| CanonicalToolChoice::Tool(n.to_owned()))
            .ok_or_else(|| InboundParseError::Unsupported {
                field: "tool_choice",
                detail: "expected a `name` for tool_choice type tool".to_owned(),
            }),
        _ => Err(unsupported()),
    }
}

pub(super) fn parse_thinking(value: &Value) -> ThinkingConfig {
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
