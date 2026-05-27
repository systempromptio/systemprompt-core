// JSON: protocol boundary — OpenAI Responses outbound wire format is dynamic
// JSON.
use serde_json::{Map, Value, json};

use super::super::super::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalToolChoice, ImageSource, Role,
};

pub fn build_request_body(request: &CanonicalRequest, upstream_model: &str) -> Value {
    let mut input: Vec<Value> = Vec::new();
    for msg in &request.messages {
        match msg.role {
            Role::Tool => render_tool_message(msg, &mut input),
            Role::Assistant => render_assistant_message(msg, &mut input),
            Role::User | Role::System => render_user_or_system(msg, &mut input),
        }
    }

    let mut obj = Map::new();
    obj.insert("model".into(), Value::String(upstream_model.to_owned()));
    obj.insert("input".into(), Value::Array(input));
    obj.insert("max_output_tokens".into(), Value::from(request.max_tokens));
    if let Some(sys) = &request.system {
        obj.insert("instructions".into(), Value::String(sys.clone()));
    }
    if let Some(t) = request.temperature {
        obj.insert("temperature".into(), json!(t));
    }
    if let Some(p) = request.top_p {
        obj.insert("top_p".into(), json!(p));
    }
    if request.stream {
        obj.insert("stream".into(), Value::Bool(true));
    }
    if !request.tools.is_empty() {
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema,
                })
            })
            .collect();
        obj.insert("tools".into(), Value::Array(tools));
    }
    if let Some(tc) = &request.tool_choice {
        obj.insert("tool_choice".into(), tool_choice_to_responses(tc));
    }
    if let Some(thinking) = &request.thinking {
        if thinking.enabled {
            let effort = match thinking.budget_tokens {
                Some(b) if b >= 16384 => "high",
                Some(b) if b >= 4096 => "medium",
                Some(_) => "low",
                None => "medium",
            };
            obj.insert("reasoning".into(), json!({ "effort": effort }));
        }
    }
    Value::Object(obj)
}

fn render_tool_message(msg: &CanonicalMessage, input: &mut Vec<Value>) {
    for part in &msg.content {
        if let CanonicalContent::ToolResult {
            tool_use_id,
            content,
            ..
        } = part
        {
            let output = flatten_text_parts(content);
            input.push(json!({
                "type": "function_call_output",
                "call_id": tool_use_id,
                "output": output,
            }));
        }
    }
}

fn render_assistant_message(msg: &CanonicalMessage, input: &mut Vec<Value>) {
    let mut text = String::new();
    let mut tool_calls: Vec<Value> = Vec::new();
    let mut reasoning: Option<String> = None;
    for part in &msg.content {
        match part {
            CanonicalContent::Text(t) => text.push_str(t),
            CanonicalContent::ToolUse {
                id,
                name,
                input: arg,
            } => {
                tool_calls.push(json!({
                    "type": "function_call",
                    "call_id": id,
                    "name": name,
                    "arguments": serde_json::to_string(arg)
                        .unwrap_or_else(|_| "{}".into()),
                }));
            },
            CanonicalContent::Thinking { text: t, .. } => {
                reasoning = Some(t.clone());
            },
            _ => {},
        }
    }
    if let Some(r) = reasoning {
        input.push(json!({
            "type": "reasoning",
            "summary": [{ "type": "summary_text", "text": r }],
        }));
    }
    input.extend(tool_calls);
    if !text.is_empty() {
        input.push(json!({
            "type": "message",
            "role": "assistant",
            "content": [{ "type": "output_text", "text": text }],
        }));
    }
}

fn render_user_or_system(msg: &CanonicalMessage, input: &mut Vec<Value>) {
    let parts: Vec<Value> = msg
        .content
        .iter()
        .filter_map(content_to_input_part)
        .collect();
    if parts.is_empty() {
        return;
    }
    input.push(json!({
        "type": "message",
        "role": match msg.role {
            Role::System => "developer",
            _ => "user",
        },
        "content": parts,
    }));
}

fn content_to_input_part(part: &CanonicalContent) -> Option<Value> {
    match part {
        CanonicalContent::Text(t) => Some(json!({ "type": "input_text", "text": t })),
        CanonicalContent::Image(src) => match src {
            ImageSource::Url(u) => Some(json!({ "type": "input_image", "image_url": u })),
            ImageSource::Base64 { media_type, data } => Some(json!({
                "type": "input_image",
                "image_url": format!("data:{media_type};base64,{data}"),
            })),
        },
        _ => None,
    }
}

fn flatten_text_parts(parts: &[CanonicalContent]) -> String {
    let mut out = String::new();
    for p in parts {
        if let CanonicalContent::Text(t) = p {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(t);
        }
    }
    out
}

fn tool_choice_to_responses(tc: &CanonicalToolChoice) -> Value {
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
