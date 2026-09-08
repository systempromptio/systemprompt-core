//! Synchronize canonical text views with repaired provider-bound content.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::wire::canonical::{CanonicalContent, CanonicalRequest};
// JSON: canonical tool arguments and metadata retain provider-defined JSON
// values.
use serde_json::Value;

fn replace_text(text: &mut String, replacements: &[(String, String)]) {
    for (old, new) in replacements {
        *text = text.replace(old, new);
    }
}

fn replace_json(value: &mut Value, replacements: &[(String, String)]) {
    match value {
        Value::String(text) => replace_text(text, replacements),
        Value::Array(values) => {
            for value in values {
                replace_json(value, replacements);
            }
        },
        Value::Object(values) => {
            for value in values.values_mut() {
                replace_json(value, replacements);
            }
        },
        Value::Null | Value::Bool(_) | Value::Number(_) => {},
    }
}

fn replace_content(content: &mut [CanonicalContent], replacements: &[(String, String)]) {
    for part in content {
        match part {
            CanonicalContent::Text(text) | CanonicalContent::Thinking { text, .. } => {
                replace_text(text, replacements);
            },
            CanonicalContent::ToolUse { input, .. } => replace_json(input, replacements),
            CanonicalContent::ToolResult {
                content,
                structured_content,
                meta,
                ..
            } => {
                replace_content(content, replacements);
                for value in structured_content.iter_mut().chain(meta.iter_mut()) {
                    replace_json(value, replacements);
                }
            },
            CanonicalContent::Image(_) => {},
        }
    }
}

pub(super) fn replace_canonical(request: &mut CanonicalRequest, replacements: &[(String, String)]) {
    if let Some(system) = &mut request.system {
        replace_text(system, replacements);
    }
    for message in &mut request.messages {
        replace_content(&mut message.content, replacements);
    }
    for tool in &mut request.tools {
        if let Some(description) = &mut tool.description {
            replace_text(description, replacements);
        }
        replace_json(&mut tool.input_schema, replacements);
    }
    if let Some(metadata) = &mut request.metadata {
        replace_json(metadata, replacements);
    }
}
