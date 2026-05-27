// JSON: protocol boundary — Anthropic Messages outbound wire format is dynamic
// JSON.
use serde_json::{Map, Value, json};

use super::super::super::canonical::{
    CanonicalMessage, CanonicalRequest, CanonicalToolChoice, Role,
};
use super::super::super::inbound::anthropic_messages::content_to_anthropic_block;

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
    if !request.tools.is_empty() {
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|t| {
                let mut tobj = Map::new();
                tobj.insert("name".into(), Value::String(t.name.clone()));
                if let Some(d) = &t.description {
                    tobj.insert("description".into(), Value::String(d.clone()));
                }
                tobj.insert("input_schema".into(), t.input_schema.clone());
                Value::Object(tobj)
            })
            .collect();
        obj.insert("tools".into(), Value::Array(tools));
    }
    if let Some(tc) = &request.tool_choice {
        obj.insert("tool_choice".into(), tool_choice_to_anthropic(tc));
    }
    if request.stream {
        obj.insert("stream".into(), Value::Bool(true));
    }
    if let Some(thinking) = &request.thinking {
        if thinking.enabled {
            let mut t = Map::new();
            t.insert("type".into(), Value::String("enabled".into()));
            if let Some(b) = thinking.budget_tokens {
                t.insert("budget_tokens".into(), Value::from(b));
            }
            obj.insert("thinking".into(), Value::Object(t));
        }
    }
    if let Some(meta) = &request.metadata {
        obj.insert("metadata".into(), meta.clone());
    }
    Value::Object(obj)
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
