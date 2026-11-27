//! A rough token estimate (about four characters per token) for compaction
//! budgeting, never for billing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::llm::model::{Content, Message, Request};

pub const CHARS_PER_TOKEN: u64 = 4;
const IMAGE_TOKENS: u64 = 1_000;
const MESSAGE_OVERHEAD: u64 = 4;

pub fn estimate(text: &str) -> u64 {
    (text.chars().count() as u64).div_ceil(CHARS_PER_TOKEN)
}

pub fn estimate_content(content: &Content) -> u64 {
    match content {
        Content::Text { text } | Content::Reasoning { text, .. } => estimate(text),
        Content::Image { .. } => IMAGE_TOKENS,
        Content::Document { data, .. } => base64_tokens(data),
        Content::ToolCall { name, input, .. } => estimate(name) + estimate(&input.to_string()),
        Content::ToolResult {
            output,
            attachments,
            ..
        } => {
            let files: u64 = attachments
                .iter()
                .map(|a| {
                    if a.mime.starts_with("image/") {
                        IMAGE_TOKENS
                    } else {
                        base64_tokens(&a.data)
                    }
                })
                .sum();
            estimate(output) + files
        },
        Content::RedactedReasoning { data } => estimate(data),
    }
}

pub fn estimate_message(message: &Message) -> u64 {
    MESSAGE_OVERHEAD + message.content.iter().map(estimate_content).sum::<u64>()
}

pub fn estimate_request(request: &Request) -> u64 {
    let system: u64 = request.system.iter().map(|s| estimate(s)).sum();
    let messages: u64 = request.messages.iter().map(estimate_message).sum();
    let tools: u64 = request
        .tools
        .iter()
        .map(|tool| {
            estimate(&tool.name)
                + estimate(&tool.description)
                + estimate(&tool.input_schema.to_string())
        })
        .sum();
    system + messages + tools
}

const fn base64_tokens(data: &str) -> u64 {
    (data.len() as u64 * 3 / 4).div_ceil(CHARS_PER_TOKEN)
}
