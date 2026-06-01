//! Tool, tool-choice, and thinking-config parsing for the Anthropic Messages
//! wire format.

// JSON: protocol boundary — Anthropic Messages wire format is dynamic JSON.
use serde_json::{Map, Value};

use crate::services::gateway::protocol::canonical::{
    CanonicalTool, CanonicalToolChoice, ThinkingConfig,
};

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

pub(super) fn parse_tool_choice(value: &Value) -> Option<CanonicalToolChoice> {
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
