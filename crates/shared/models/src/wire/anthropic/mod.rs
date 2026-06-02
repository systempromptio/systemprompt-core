//! Anthropic Messages wire codec.
//!
//! Pure, transport-free translation between the canonical model and the
//! Anthropic Messages dialect. This module ([`mod`](self)) owns the request
//! side — auth headers, request-body build, and canonical-content rendering —
//! while the `parse` submodule owns the buffered-response and per-SSE-frame
//! parse side. The HTTP transport and SSE framing live in the gateway adapter;
//! everything here operates on already-decoded values so it is shared by both
//! the outbound adapter and the inbound renderer.

mod parse;
mod sse;

pub use parse::parse_response;
pub use sse::event_from_sse;

// JSON: protocol boundary — the Anthropic Messages wire format is dynamic JSON.
use serde_json::{Map, Value, json};

use crate::profile::WireProtocol;
use crate::schema::SchemaSanitizer;
use crate::wire::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalTool, CanonicalToolChoice,
    ImageSource, ResponseFormat, Role, SearchConfig,
};

pub const ANTHROPIC_VERSION: &str = "2023-06-01";

#[must_use]
pub fn auth_headers(api_key: &str) -> [(&'static str, String); 3] {
    [
        ("x-api-key", api_key.to_owned()),
        ("anthropic-version", ANTHROPIC_VERSION.to_owned()),
        ("content-type", "application/json".to_owned()),
    ]
}

#[must_use]
pub fn build_request_body(request: &CanonicalRequest, upstream_model: &str) -> Value {
    let messages: Vec<Value> = request
        .messages
        .iter()
        .filter(|m| !matches!(m.role, Role::System))
        .map(canonical_message_to_anthropic)
        .collect();

    let mut obj = Map::new();
    obj.insert("model".into(), Value::String(upstream_model.to_owned()));
    obj.insert("max_tokens".into(), Value::from(request.max_tokens));
    obj.insert("messages".into(), Value::Array(messages));
    if let Some(sys) = &request.system {
        obj.insert("system".into(), Value::String(sys.clone()));
    }
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
    let mut tools: Vec<Value> = request.tools.iter().map(tool_to_anthropic).collect();
    let forced_tool: Option<&str> =
        if let Some(ResponseFormat::JsonSchema { name, schema, .. }) = &request.response_format {
            tools.push(structured_output_tool(name, schema));
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
    // A server-tool search turn must not pin tool_choice or stream — Anthropic
    // rejects the web_search tool combined with either.
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

fn structured_output_tool(name: &str, schema: &Value) -> Value {
    json!({
        "name": name,
        "description": "Respond by calling this tool with arguments matching the schema.",
        "input_schema": schema,
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

fn canonical_message_to_anthropic(msg: &CanonicalMessage) -> Value {
    let role = match msg.role {
        Role::Assistant => "assistant",
        Role::User | Role::Tool | Role::System => "user",
    };
    let content: Vec<Value> = msg.content.iter().map(content_to_anthropic_block).collect();
    json!({ "role": role, "content": content })
}

fn tool_choice_to_anthropic(tc: &CanonicalToolChoice) -> Value {
    match tc {
        CanonicalToolChoice::Auto => json!({ "type": "auto" }),
        CanonicalToolChoice::Any | CanonicalToolChoice::Required => json!({ "type": "any" }),
        CanonicalToolChoice::None => json!({ "type": "none" }),
        CanonicalToolChoice::Tool(name) => json!({ "type": "tool", "name": name }),
    }
}

#[must_use]
pub fn content_to_anthropic_block(part: &CanonicalContent) -> Value {
    match part {
        CanonicalContent::Text(t) => json!({ "type": "text", "text": t }),
        CanonicalContent::Thinking { text, signature } => {
            let mut obj = Map::new();
            obj.insert("type".into(), Value::String("thinking".into()));
            obj.insert("thinking".into(), Value::String(text.clone()));
            if let Some(sig) = signature {
                obj.insert("signature".into(), Value::String(sig.clone()));
            }
            Value::Object(obj)
        },
        CanonicalContent::ToolUse {
            id,
            name,
            input,
            signature,
        } => {
            let mut obj = Map::new();
            obj.insert("type".into(), Value::String("tool_use".into()));
            obj.insert("id".into(), Value::String(id.clone()));
            obj.insert("name".into(), Value::String(name.clone()));
            obj.insert("input".into(), input.clone());
            if let Some(sig) = signature {
                obj.insert("signature".into(), Value::String(sig.clone()));
            }
            Value::Object(obj)
        },
        CanonicalContent::ToolResult {
            tool_use_id,
            content,
            is_error,
            structured_content,
            meta,
        } => {
            let inner: Vec<Value> = content.iter().map(content_to_anthropic_block).collect();
            let mut obj = Map::new();
            obj.insert("type".into(), Value::String("tool_result".into()));
            obj.insert("tool_use_id".into(), Value::String(tool_use_id.clone()));
            obj.insert("is_error".into(), Value::Bool(*is_error));
            obj.insert("content".into(), Value::Array(inner));
            if let Some(sc) = structured_content {
                obj.insert("structuredContent".into(), sc.clone());
            }
            if let Some(m) = meta {
                obj.insert("_meta".into(), m.clone());
            }
            Value::Object(obj)
        },
        CanonicalContent::Image(src) => match src {
            ImageSource::Base64 {
                media_type, data, ..
            } => json!({
                "type": "image",
                "source": { "type": "base64", "media_type": media_type, "data": data },
            }),
            ImageSource::Url { url, .. } => json!({
                "type": "image",
                "source": { "type": "url", "url": url },
            }),
        },
    }
}
