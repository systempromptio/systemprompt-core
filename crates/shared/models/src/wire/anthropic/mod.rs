//! Anthropic Messages wire codec.
//!
//! Pure, transport-free translation between the canonical model and the
//! Anthropic Messages dialect. HTTP transport and SSE framing live in the
//! gateway adapter; everything here operates on already-decoded values so it
//! is shared by both the outbound adapter and the inbound renderer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod blocks;
mod headers;
mod parse;
mod sse;
mod strict;

pub use blocks::content_to_anthropic_block;
pub use headers::{
    ANTHROPIC_VERSION, auth_headers, is_forwardable_request_header, is_identity_request_header,
    strip_user_id,
};
pub use parse::parse_response;
pub use sse::events_from_sse;

// JSON: protocol boundary — the Anthropic Messages wire format is dynamic JSON.
use serde_json::{Map, Value, json};

use blocks::{BlockAudience, canonical_message_to_anthropic};

use crate::schema::SchemaSanitizer;
use crate::services::WireProtocol;
use crate::services::ai::ModelLimits;
use crate::wire::canonical::{
    CanonicalRequest, CanonicalTool, CanonicalToolChoice, ResponseFormat, Role, SearchConfig,
};

#[must_use]
pub fn build_request_body(
    request: &CanonicalRequest,
    upstream_model: &str,
    limits: Option<ModelLimits>,
) -> Value {
    let messages: Vec<Value> = request
        .messages
        .iter()
        .filter(|m| !matches!(m.role, Role::System))
        .filter_map(|m| canonical_message_to_anthropic(m, BlockAudience::Upstream))
        .collect();

    let mut obj = Map::new();
    obj.insert("model".into(), Value::String(upstream_model.to_owned()));
    obj.insert(
        "max_tokens".into(),
        Value::from(crate::wire::clamp_output_tokens(
            request.max_tokens,
            limits.map(|l| l.max_output_tokens),
        )),
    );
    obj.insert("messages".into(), Value::Array(messages));
    if let Some(sys) = &request.system {
        obj.insert("system".into(), Value::String(sys.clone()));
    }
    insert_sampling_params(&mut obj, request);
    let mut tools: Vec<Value> = request.tools.iter().map(tool_to_anthropic).collect();
    let forced_tool: Option<&str> = if let Some(ResponseFormat::JsonSchema {
        name,
        schema,
        strict,
    }) = &request.response_format
    {
        tools.push(structured_output_tool(name, schema, *strict));
        Some(name.as_str())
    } else {
        None
    };
    let searching = request.search.is_some();
    if let Some(search) = &request.search {
        tools.push(web_search_tool(search));
    }
    if !tools.is_empty() {
        obj.insert("tools".into(), Value::Array(tools));
    }
    if searching {
        if let Some(thinking) = &request.thinking {
            insert_thinking(&mut obj, thinking);
        }
        if let Some(meta) = &request.metadata {
            obj.insert("metadata".into(), meta.clone());
        }
        return Value::Object(obj);
    }
    if let Some(name) = forced_tool {
        obj.insert(
            "tool_choice".into(),
            json!({ "type": "tool", "name": name }),
        );
    } else if let Some(tc) = &request.tool_choice {
        obj.insert("tool_choice".into(), tool_choice_to_anthropic(tc));
    }
    if request.stream {
        obj.insert("stream".into(), Value::Bool(true));
    }
    if let Some(thinking) = &request.thinking {
        insert_thinking(&mut obj, thinking);
    }
    if let Some(meta) = &request.metadata {
        obj.insert("metadata".into(), meta.clone());
    }
    Value::Object(obj)
}

fn insert_sampling_params(obj: &mut Map<String, Value>, request: &CanonicalRequest) {
    if let Some(t) = request.temperature {
        obj.insert("temperature".into(), json!(t));
    }
    if let Some(p) = request.top_p {
        obj.insert("top_p".into(), json!(p));
    }
    if let Some(k) = request.top_k {
        obj.insert("top_k".into(), json!(k));
    }
    if !request.stop_sequences.is_empty() {
        obj.insert("stop_sequences".into(), json!(request.stop_sequences));
    }
}

fn insert_thinking(
    obj: &mut Map<String, Value>,
    thinking: &crate::wire::canonical::ThinkingConfig,
) {
    if !thinking.enabled {
        return;
    }
    let mut t = Map::new();
    t.insert("type".into(), Value::String("enabled".into()));
    if let Some(b) = thinking.budget_tokens {
        t.insert("budget_tokens".into(), Value::from(b));
    }
    obj.insert("thinking".into(), Value::Object(t));
}

// Why: `strict` makes Anthropic compile the schema into a grammar and sample
// only conforming output; without it the schema is advisory and the model can
// wrap, truncate or free-text a field. The schema is shaped first because the
// grammar compiler accepts a narrower dialect than JSON Schema.
fn structured_output_tool(name: &str, schema: &Value, strict: bool) -> Value {
    let input_schema = if strict {
        strict::strict_input_schema(schema)
    } else {
        schema.clone()
    };
    json!({
        "name": name,
        "description": "Respond by calling this tool with arguments matching the schema.",
        "strict": strict,
        "input_schema": input_schema,
    })
}

fn web_search_tool(search: &SearchConfig) -> Value {
    let mut t = Map::new();
    t.insert("type".into(), Value::String("web_search_20250305".into()));
    t.insert("name".into(), Value::String("web_search".into()));
    if let Some(max) = search.max_uses {
        t.insert("max_uses".into(), Value::from(max));
    }
    Value::Object(t)
}

fn tool_to_anthropic(tool: &CanonicalTool) -> Value {
    let sanitizer = SchemaSanitizer::new(WireProtocol::Anthropic.schema_capabilities());
    let mut tobj = Map::new();
    tobj.insert("name".into(), Value::String(tool.name.clone()));
    if let Some(d) = &tool.description {
        tobj.insert("description".into(), Value::String(d.clone()));
    }
    tobj.insert(
        "input_schema".into(),
        sanitizer.sanitize(tool.input_schema.clone()),
    );
    Value::Object(tobj)
}

fn tool_choice_to_anthropic(tc: &CanonicalToolChoice) -> Value {
    match tc {
        CanonicalToolChoice::Auto => json!({ "type": "auto" }),
        CanonicalToolChoice::Any | CanonicalToolChoice::Required => json!({ "type": "any" }),
        CanonicalToolChoice::None => json!({ "type": "none" }),
        CanonicalToolChoice::Tool(name) => json!({ "type": "tool", "name": name }),
    }
}
