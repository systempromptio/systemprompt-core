//! Anthropic Messages wire codec.
//!
//! Pure, transport-free translation between the canonical model and the
//! Anthropic Messages dialect. HTTP transport and SSE framing live in the
//! gateway adapter; everything here operates on already-decoded values so it
//! is shared by both the outbound adapter and the inbound renderer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod parse;
mod sse;

pub use parse::parse_response;
pub use sse::event_from_sse;

// JSON: protocol boundary — the Anthropic Messages wire format is dynamic JSON.
use serde_json::{Map, Value, json};

use crate::profile::WireProtocol;
use crate::schema::SchemaSanitizer;
use crate::services::ai::ModelLimits;
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

// Why: Anthropic's contract wants `anthropic-*` forwarded verbatim, not
// allowlisted — each beta body field pairs with a header, and forwarding one
// half of the pair is a hard 400.
const FORWARD_PREFIXES: &[&str] = &["anthropic-"];

// Why: the contract classifies these as consumable — recorded on the audit row
// and dropped before the upstream send, never relayed to a third party.
const IDENTITY_PREFIXES: &[&str] = &["x-claude-code-", "x-stainless-", "x-systemprompt-"];

// Why: the gateway substitutes its own provider credential — relaying the
// caller's `authorization`/`x-api-key` would leak a systemprompt credential.
const IDENTITY_NAMES: &[&str] = &[
    "user-agent",
    "cookie",
    "set-cookie",
    "authorization",
    "x-api-key",
    "x-forwarded-for",
    "x-real-ip",
];

#[must_use]
pub fn is_forwardable_request_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    !identity_lower(&lower) && FORWARD_PREFIXES.iter().any(|p| lower.starts_with(p))
}

#[must_use]
pub fn is_identity_request_header(name: &str) -> bool {
    identity_lower(&name.to_ascii_lowercase())
}

fn identity_lower(lower: &str) -> bool {
    IDENTITY_NAMES.contains(&lower) || IDENTITY_PREFIXES.iter().any(|p| lower.starts_with(p))
}

/// Both the canonical lane and the byte-passthrough lane strip
/// `metadata.user_id` through this one function, so which lane a request takes
/// never changes what identity leaves the gateway.
pub fn strip_user_id(obj: &mut Map<String, Value>) {
    let Some(metadata) = obj.get_mut("metadata") else {
        return;
    };
    let Some(map) = metadata.as_object_mut() else {
        return;
    };
    map.remove("user_id");
    if map.is_empty() {
        obj.remove("metadata");
    }
}

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

fn canonical_message_to_anthropic(
    msg: &CanonicalMessage,
    audience: BlockAudience,
) -> Option<Value> {
    let role = match msg.role {
        Role::Assistant => "assistant",
        Role::User | Role::Tool | Role::System => "user",
    };
    let content: Vec<Value> = msg
        .content
        .iter()
        .filter(|part| {
            // Why: Anthropic 400s on a replayed thinking block without its
            // signature; history without the block is valid and merely loses
            // continuity.
            audience == BlockAudience::Client
                || !matches!(
                    part,
                    CanonicalContent::Thinking {
                        signature: None,
                        ..
                    }
                )
        })
        .map(|part| block_for_audience(part, audience))
        .collect();
    if content.is_empty() {
        return None;
    }
    Some(json!({ "role": role, "content": content }))
}

fn tool_choice_to_anthropic(tc: &CanonicalToolChoice) -> Value {
    match tc {
        CanonicalToolChoice::Auto => json!({ "type": "auto" }),
        CanonicalToolChoice::Any | CanonicalToolChoice::Required => json!({ "type": "any" }),
        CanonicalToolChoice::None => json!({ "type": "none" }),
        CanonicalToolChoice::Tool(name) => json!({ "type": "tool", "name": name }),
    }
}

// Why: the real Anthropic API rejects unknown keys in content blocks, while
// the gateway's own client relies on its vendor-extension fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockAudience {
    Client,
    Upstream,
}

#[must_use]
pub fn content_to_anthropic_block(part: &CanonicalContent) -> Value {
    block_for_audience(part, BlockAudience::Client)
}

fn block_for_audience(part: &CanonicalContent, audience: BlockAudience) -> Value {
    match part {
        CanonicalContent::Text(t) => json!({ "type": "text", "text": t }),
        CanonicalContent::Thinking {
            text, signature, ..
        } => {
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
            if audience == BlockAudience::Client
                && let Some(sig) = signature
            {
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
            let inner: Vec<Value> = content
                .iter()
                .map(|p| block_for_audience(p, audience))
                .collect();
            let mut obj = Map::new();
            obj.insert("type".into(), Value::String("tool_result".into()));
            obj.insert("tool_use_id".into(), Value::String(tool_use_id.clone()));
            obj.insert("is_error".into(), Value::Bool(*is_error));
            obj.insert("content".into(), Value::Array(inner));
            if audience == BlockAudience::Client {
                if let Some(sc) = structured_content {
                    obj.insert("structuredContent".into(), sc.clone());
                }
                if let Some(m) = meta {
                    obj.insert("_meta".into(), m.clone());
                }
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
