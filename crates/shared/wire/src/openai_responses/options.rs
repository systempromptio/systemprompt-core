//! Request-option mappers of the Responses wire: reasoning effort, response
//! format, web search and tool choice.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Map, Value, json};

use crate::canonical::{CanonicalRequest, CanonicalToolChoice, ResponseFormat, SearchConfig};

pub(super) fn reasoning_effort(request: &CanonicalRequest) -> Option<&'static str> {
    if let Some(effort) = request.reasoning.reasoning_effort {
        return Some(effort.as_str());
    }
    let thinking = request.reasoning.thinking?;
    if !thinking.enabled {
        return None;
    }
    Some(match thinking.budget_tokens {
        Some(b) if b >= 16384 => "high",
        Some(b) if b >= 4096 => "medium",
        Some(_) => "low",
        None => "medium",
    })
}

// JSON: OpenAI Responses API request body; upstream JSON is the contract.
pub(super) fn response_format_to_responses(format: &ResponseFormat) -> Value {
    match format {
        ResponseFormat::JsonObject => json!({ "type": "json_object" }),
        ResponseFormat::JsonSchema {
            name,
            schema,
            strict,
        } => json!({
            "type": "json_schema",
            "name": name,
            "strict": strict,
            "schema": schema,
        }),
    }
}

// JSON: OpenAI Responses API request body; upstream JSON is the contract.
pub(super) fn web_search_tool(search: &SearchConfig) -> Value {
    let mut t = Map::new();
    t.insert("type".into(), Value::String("web_search".into()));
    if let Some(size) = &search.context_size {
        t.insert("search_context_size".into(), Value::String(size.clone()));
    }
    Value::Object(t)
}

// JSON: OpenAI Responses API request body; upstream JSON is the contract.
pub(super) fn tool_choice_to_responses(tc: &CanonicalToolChoice) -> Value {
    match tc {
        CanonicalToolChoice::Auto => Value::String("auto".into()),
        CanonicalToolChoice::None => Value::String("none".into()),
        CanonicalToolChoice::Any | CanonicalToolChoice::Required => {
            Value::String("required".into())
        },
        CanonicalToolChoice::Tool(name) => json!({
            "type": "function",
            "name": name,
        }),
    }
}
