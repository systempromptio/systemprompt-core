//! Tool results and rendered prompts, flattened from MCP content blocks to text
//! plus image attachments.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;

/// Binary content returned by a server, base64 encoded as received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub mime_type: String,
    pub data: String,
}

/// The outcome of `tools/call`, flattened to text plus image attachments.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CallResult {
    pub text: String,
    pub is_error: bool,
    pub attachments: Vec<Attachment>,
    // JSON: the tool's `structuredContent`, shaped by its own output schema.
    pub structured: Option<Value>,
}

impl CallResult {
    // JSON: the `tools/call` result as the server sent it.
    pub(crate) fn from_value(result: &Value) -> Self {
        let blocks = result
            .get("content")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        let (mut text, attachments) = flatten(blocks);
        let structured = result
            .get("structuredContent")
            .filter(|value| !value.is_null())
            .cloned();
        if text.is_empty()
            && attachments.is_empty()
            && let Some(value) = &structured
        {
            text = value.to_string();
        }
        Self {
            text,
            is_error: result
                .get("isError")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            attachments,
            structured,
        }
    }
}

/// A rendered prompt from `prompts/get`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PromptResult {
    pub description: Option<String>,
    pub messages: Vec<PromptMessage>,
}

/// One message of a rendered prompt, flattened like a tool result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptMessage {
    pub role: String,
    pub text: String,
    pub attachments: Vec<Attachment>,
}

impl PromptResult {
    // JSON: the `prompts/get` result as the server sent it.
    pub(crate) fn from_value(result: &Value) -> Self {
        let messages = result
            .get("messages")
            .and_then(Value::as_array)
            .map(|messages| {
                messages
                    .iter()
                    .map(|message| {
                        let blocks: Vec<Value> =
                            message.get("content").cloned().into_iter().collect();
                        let (text, attachments) = flatten(&blocks);
                        PromptMessage {
                            role: message
                                .get("role")
                                .and_then(Value::as_str)
                                .unwrap_or("user")
                                .to_owned(),
                            text,
                            attachments,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            description: result
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_owned),
            messages,
        }
    }
}

// JSON: MCP content blocks, each tagged by its `type` member.
fn flatten(blocks: &[Value]) -> (String, Vec<Attachment>) {
    let mut texts: Vec<String> = Vec::new();
    let mut attachments = Vec::new();
    for block in blocks {
        let text_of = |key: &str| block.get(key).and_then(Value::as_str);
        match text_of("type").unwrap_or_default() {
            "text" => {
                if let Some(text) = text_of("text") {
                    texts.push(text.to_owned());
                }
            },
            "image" => {
                if let (Some(data), Some(mime)) = (text_of("data"), text_of("mimeType")) {
                    attachments.push(Attachment {
                        mime_type: mime.to_owned(),
                        data: data.to_owned(),
                    });
                }
            },
            "audio" => texts.push(format!(
                "[audio {}]",
                text_of("mimeType").unwrap_or("unknown")
            )),
            "resource" => {
                let Some(resource) = block.get("resource") else {
                    continue;
                };
                let field = |key: &str| resource.get(key).and_then(Value::as_str);
                let uri = field("uri").unwrap_or_default();
                if let Some(text) = field("text") {
                    texts.push(text.to_owned());
                } else if let (Some(blob), Some(mime)) = (field("blob"), field("mimeType"))
                    && mime.starts_with("image/")
                {
                    attachments.push(Attachment {
                        mime_type: mime.to_owned(),
                        data: blob.to_owned(),
                    });
                } else {
                    texts.push(format!("[resource {uri}]"));
                }
            },
            "resource_link" => {
                let uri = text_of("uri").unwrap_or_default();
                match text_of("name") {
                    Some(name) if !name.is_empty() => {
                        texts.push(format!("[resource {name}: {uri}]"));
                    },
                    _ => texts.push(format!("[resource {uri}]")),
                }
            },
            _ => {},
        }
    }
    (texts.join("\n"), attachments)
}
