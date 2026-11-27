//! Lenient reading of streamed tool-call arguments.
//!
//! Models occasionally send arguments as a JSON string that itself holds JSON,
//! wrap them in prose or fences, or append stray text after the object. What
//! can be recovered is recovered; otherwise the raw text is kept so the caller
//! can show the model exactly what it sent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Map, Value};

/// The outcome of reading one tool call's arguments. `malformed` holds the raw
/// text when nothing could be read; `value` is then an empty object.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolInput {
    // JSON: tool arguments as the model wrote them for the tool's input schema.
    pub value: Value,
    pub malformed: Option<String>,
}

pub fn repair_tool_input(raw: &str) -> ToolInput {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return recovered(Value::Object(Map::new()));
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return recovered(unwrap_encoded(value));
    }
    if let Some(slice) = first_balanced(trimmed)
        && let Ok(value) = serde_json::from_str::<Value>(slice)
    {
        return recovered(unwrap_encoded(value));
    }
    ToolInput {
        value: Value::Object(Map::new()),
        malformed: Some(raw.to_owned()),
    }
}

// JSON: a parsed tool-argument value, any shape the model produced.
const fn recovered(value: Value) -> ToolInput {
    ToolInput {
        value,
        malformed: None,
    }
}

// JSON: a parsed tool-argument value, any shape the model produced.
fn unwrap_encoded(value: Value) -> Value {
    match value {
        Value::Null => Value::Object(Map::new()),
        Value::String(text) => match serde_json::from_str::<Value>(text.trim()) {
            Ok(inner @ (Value::Object(_) | Value::Array(_))) => inner,
            _ => Value::String(text),
        },
        other => other,
    }
}

fn first_balanced(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let start = bytes.iter().position(|b| *b == b'{' || *b == b'[')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in bytes[start..].iter().enumerate() {
        if in_string {
            match byte {
                _ if escaped => escaped = false,
                b'\\' => escaped = true,
                b'"' => in_string = false,
                _ => {},
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return text.get(start..start + offset + 1);
                }
            },
            _ => {},
        }
    }
    None
}
