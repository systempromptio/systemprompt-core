//! The Messages request body: system blocks, merged turns, tools, thinking and
//! sampling, with prompt-cache markers on the system prompt and the last turns
//! when the model supports caching.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Map, Value, json};

use super::content::block;
use crate::model::{Params, Reasoning, Request, Role, ToolChoice};

const DEFAULT_MAX_OUTPUT: u32 = 8_192;
const MIN_THINKING_BUDGET: u32 = 1_024;
const CACHED_TURNS: usize = 2;

// JSON: the Messages request body, built field by field in the provider's wire
// shape.
pub(crate) fn body(request: &Request) -> Value {
    let model = &request.model;
    let mut body = Map::new();
    body.insert("model".into(), model.id.clone().into());
    if let Some(system) = system(request) {
        body.insert("system".into(), Value::Array(system));
    }
    body.insert("messages".into(), Value::Array(messages(request)));
    if !request.tools.is_empty() && model.supports.tool_calls {
        insert_tools(request, &mut body);
    }
    let ceiling = if model.max_output == 0 {
        DEFAULT_MAX_OUTPUT
    } else {
        model.max_output
    };
    let max_tokens = request
        .params
        .max_output
        .unwrap_or(ceiling)
        .clamp(1, ceiling);
    let thinking = insert_thinking(request, max_tokens, &mut body);
    body.insert("max_tokens".into(), max_tokens.into());
    if !thinking {
        insert_sampling(&request.params, &mut body);
    }
    body.insert("stream".into(), true.into());
    if !request.metadata.is_empty() {
        body.insert("metadata".into(), Value::Object(request.metadata.clone()));
    }
    Value::Object(body)
}

// JSON: the system prompt as Messages text blocks.
fn system(request: &Request) -> Option<Vec<Value>> {
    let mut system: Vec<Value> = request
        .system
        .iter()
        .filter(|text| !text.trim().is_empty())
        .map(|text| json!({"type": "text", "text": text}))
        .collect();
    if system.is_empty() {
        return None;
    }
    if request.model.supports.caching {
        mark_cache(system.last_mut());
    }
    Some(system)
}

// JSON: the Messages request body under construction.
fn insert_tools(request: &Request, body: &mut Map<String, Value>) {
    let tools: Vec<Value> = request
        .tools
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.input_schema
            })
        })
        .collect();
    body.insert("tools".into(), Value::Array(tools));
    let choice = match &request.params.tool_choice {
        ToolChoice::Auto => return,
        ToolChoice::None => json!({"type": "none"}),
        ToolChoice::Required => json!({"type": "any"}),
        ToolChoice::Tool(name) => json!({"type": "tool", "name": name}),
    };
    body.insert("tool_choice".into(), choice);
}

// JSON: the Messages request body under construction.
fn insert_thinking(request: &Request, max_tokens: u32, body: &mut Map<String, Value>) -> bool {
    match request
        .params
        .reasoning
        .filter(|_| request.model.supports.reasoning)
    {
        Some(Reasoning::Budget(budget)) => {
            let budget = budget
                .max(MIN_THINKING_BUDGET)
                .min(max_tokens.saturating_sub(1));
            if budget < MIN_THINKING_BUDGET {
                return false;
            }
            body.insert(
                "thinking".into(),
                json!({"type": "enabled", "budget_tokens": budget}),
            );
            true
        },
        Some(Reasoning::Effort(effort)) => {
            body.insert("thinking".into(), json!({"type": "adaptive"}));
            body.insert("output_config".into(), json!({"effort": effort.as_str()}));
            true
        },
        None => false,
    }
}

// JSON: the Messages request body under construction.
fn insert_sampling(params: &Params, body: &mut Map<String, Value>) {
    if let Some(temperature) = params.temperature {
        body.insert("temperature".into(), json!(temperature.clamp(0.0, 1.0)));
    } else if let Some(top_p) = params.top_p {
        body.insert("top_p".into(), json!(top_p));
    }
}

// JSON: the Messages `messages` array: alternating turns of content blocks.
fn messages(request: &Request) -> Vec<Value> {
    let images = request.model.supports.images;
    let mut turns: Vec<(Role, Vec<Value>)> = Vec::new();
    for message in &request.messages {
        let blocks: Vec<Value> = message
            .content
            .iter()
            .filter_map(|content| block(content, images))
            .collect();
        if blocks.is_empty() {
            continue;
        }
        match turns.last_mut() {
            Some((role, existing)) if *role == message.role => existing.extend(blocks),
            _ => turns.push((message.role, blocks)),
        }
    }
    for (role, blocks) in &mut turns {
        if *role == Role::User {
            results_first(blocks);
        }
    }
    if let Some((Role::Assistant, blocks)) = turns.last_mut() {
        trim_final_text(blocks);
    }
    if request.model.supports.caching {
        let skip = turns.len().saturating_sub(CACHED_TURNS);
        for (_, blocks) in turns.iter_mut().skip(skip) {
            let last = blocks
                .iter_mut()
                .rev()
                .find(|b| !matches!(kind(b), "thinking" | "redacted_thinking"));
            mark_cache(last);
        }
    }
    turns
        .into_iter()
        .map(|(role, blocks)| json!({"role": role_name(role), "content": blocks}))
        .collect()
}

const fn role_name(role: Role) -> &'static str {
    match role {
        Role::User => "user",
        Role::Assistant => "assistant",
    }
}

// JSON: one turn's Messages content blocks.
fn results_first(blocks: &mut Vec<Value>) {
    let (mut results, rest): (Vec<Value>, Vec<Value>) = std::mem::take(blocks)
        .into_iter()
        .partition(|b| kind(b) == "tool_result");
    results.extend(rest);
    *blocks = results;
}

// JSON: a Messages content block, read for its `type` tag.
fn kind(block: &Value) -> &str {
    block
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

// JSON: a Messages content block that takes a prompt-cache marker.
fn mark_cache(block: Option<&mut Value>) {
    if let Some(map) = block.and_then(Value::as_object_mut) {
        map.insert("cache_control".into(), json!({"type": "ephemeral"}));
    }
}

// JSON: the final assistant turn's Messages content blocks.
fn trim_final_text(blocks: &mut [Value]) {
    if let Some(Value::Object(map)) = blocks.last_mut()
        && map.get("type").and_then(Value::as_str) == Some("text")
        && let Some(Value::String(text)) = map.get_mut("text")
    {
        let trimmed = text.trim_end().len();
        text.truncate(trimmed);
    }
}
