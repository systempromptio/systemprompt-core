//! The Chat Completions request body: system and conversation messages, tools,
//! output ceiling, and either a reasoning effort or sampling parameters. Tool
//! results become `tool` messages and their attachments follow in one `user`
//! message.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine as _;
use serde_json::{Map, Value, json};

use crate::model::{
    Content, Message, Params, Reasoning, ReasoningEffort, Request, Role, ToolChoice,
};

// JSON: the Chat Completions request body, built field by field in the
// provider's wire shape.
pub(crate) fn body(request: &Request) -> Value {
    let model = &request.model;
    let mut body = Map::new();
    body.insert("model".into(), model.id.clone().into());
    body.insert("messages".into(), Value::Array(messages(request)));
    if !request.tools.is_empty() && model.supports.tool_calls {
        insert_tools(request, &mut body);
    }
    body.insert("stream".into(), true.into());
    body.insert("stream_options".into(), json!({"include_usage": true}));
    let max_output = match (request.params.max_output, model.max_output) {
        (Some(wanted), 0) => Some(wanted),
        (Some(wanted), ceiling) => Some(wanted.min(ceiling)),
        (None, 0) => None,
        (None, ceiling) => Some(ceiling),
    };
    if let Some(max_output) = max_output.filter(|n| *n > 0) {
        body.insert("max_tokens".into(), max_output.into());
    }
    match request
        .params
        .reasoning
        .filter(|_| model.supports.reasoning)
    {
        Some(reasoning) => {
            body.insert("reasoning_effort".into(), effort(reasoning).into());
        },
        None => insert_sampling(&request.params, &mut body),
    }
    if !request.metadata.is_empty() {
        body.insert("metadata".into(), Value::Object(request.metadata.clone()));
    }
    Value::Object(body)
}

// JSON: the Chat Completions request body under construction.
fn insert_tools(request: &Request, body: &mut Map<String, Value>) {
    let tools: Vec<Value> = request
        .tools
        .iter()
        .map(|tool| {
            json!({"type": "function", "function": {
                "name": tool.name, "description": tool.description, "parameters": tool.input_schema
            }})
        })
        .collect();
    body.insert("tools".into(), Value::Array(tools));
    let choice = match &request.params.tool_choice {
        ToolChoice::Auto => return,
        ToolChoice::None => Value::from("none"),
        ToolChoice::Required => Value::from("required"),
        ToolChoice::Tool(name) => json!({"type": "function", "function": {"name": name}}),
    };
    body.insert("tool_choice".into(), choice);
}

// JSON: the Chat Completions request body under construction.
fn insert_sampling(params: &Params, body: &mut Map<String, Value>) {
    if let Some(temperature) = params.temperature {
        body.insert("temperature".into(), json!(temperature));
    }
    if let Some(top_p) = params.top_p {
        body.insert("top_p".into(), json!(top_p));
    }
}

const fn effort(reasoning: Reasoning) -> &'static str {
    match reasoning {
        Reasoning::Effort(ReasoningEffort::Low) | Reasoning::Budget(0..=4_096) => "low",
        Reasoning::Effort(ReasoningEffort::Medium) | Reasoning::Budget(4_097..=16_384) => "medium",
        Reasoning::Effort(ReasoningEffort::High | ReasoningEffort::Max) | Reasoning::Budget(_) => {
            "high"
        },
    }
}

// JSON: the Chat Completions `messages` array.
fn messages(request: &Request) -> Vec<Value> {
    let images = request.model.supports.images;
    let mut out: Vec<Value> = request
        .system
        .iter()
        .filter(|text| !text.trim().is_empty())
        .map(|text| json!({"role": "system", "content": text}))
        .collect();
    for message in &request.messages {
        match message.role {
            Role::User => user(message, images, &mut out),
            Role::Assistant => assistant(message, &mut out),
        }
    }
    out
}

// JSON: appends a user turn's Chat Completions messages.
fn user(message: &Message, images: bool, out: &mut Vec<Value>) {
    let mut carried = Vec::new();
    let mut parts = Vec::new();
    for content in &message.content {
        match content {
            Content::ToolResult {
                id,
                output,
                attachments,
                ..
            } => {
                out.push(json!({"role": "tool", "tool_call_id": id, "content": output}));
                for attachment in attachments {
                    let name = attachment.name.as_deref();
                    carried.push(part(&attachment.mime, &attachment.data, name, images));
                }
            },
            Content::Text { text } if !text.trim().is_empty() => {
                parts.push(json!({"type": "text", "text": text}));
            },
            Content::Image { mime, data } => parts.push(part(mime, data, None, images)),
            Content::Document { mime, data, name } => {
                parts.push(part(mime, data, name.as_deref(), images));
            },
            _ => {},
        }
    }
    carried.extend(parts);
    if carried.is_empty() {
        return;
    }
    let content = match carried.as_slice() {
        [single] if single.get("type").and_then(Value::as_str) == Some("text") => {
            single["text"].clone()
        },
        _ => Value::Array(carried),
    };
    out.push(json!({"role": "user", "content": content}));
}

// JSON: appends an assistant turn's Chat Completions message.
fn assistant(message: &Message, out: &mut Vec<Value>) {
    let mut text = String::new();
    let mut calls = Vec::new();
    for content in &message.content {
        match content {
            Content::Text { text: chunk } => text.push_str(chunk),
            Content::ToolCall { id, name, input } => {
                let arguments = if input.is_null() {
                    "{}".to_owned()
                } else {
                    input.to_string()
                };
                calls.push(json!({
                    "id": id,
                    "type": "function",
                    "function": {"name": name, "arguments": arguments}
                }));
            },
            _ => {},
        }
    }
    if text.trim().is_empty() && calls.is_empty() {
        return;
    }
    let mut entry = Map::new();
    entry.insert("role".into(), "assistant".into());
    entry.insert(
        "content".into(),
        if text.is_empty() {
            Value::Null
        } else {
            text.into()
        },
    );
    if !calls.is_empty() {
        entry.insert("tool_calls".into(), Value::Array(calls));
    }
    out.push(Value::Object(entry));
}

// JSON: one Chat Completions content part for an attachment, or the note that
// it was left out.
fn part(mime: &str, data: &str, name: Option<&str>, images: bool) -> Value {
    if mime.starts_with("image/") && images {
        let url = format!("data:{mime};base64,{data}");
        return json!({"type": "image_url", "image_url": {"url": url}});
    }
    if mime == "application/pdf" {
        let filename = name.unwrap_or("document.pdf");
        let file_data = format!("data:{mime};base64,{data}");
        return json!({"type": "file", "file": {"filename": filename, "file_data": file_data}});
    }
    if mime.starts_with("text/")
        && let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data)
    {
        return json!({"type": "text", "text": String::from_utf8_lossy(&bytes)});
    }
    let label = name.map_or_else(|| mime.to_owned(), |n| format!("{n} ({mime})"));
    let note = format!("[attachment {label} omitted: the model does not accept this type]");
    json!({"type": "text", "text": note})
}
