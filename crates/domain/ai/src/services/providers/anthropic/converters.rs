use serde_json::json;

use crate::models::ai::{AiContentPart, AiMessage, MessageRole};
use crate::models::providers::anthropic::{
    AnthropicContent, AnthropicContentBlock, AnthropicImageSource, AnthropicMessage, AnthropicTool,
};
use crate::models::tools::McpTool;

pub fn convert_messages(messages: &[AiMessage]) -> (Option<String>, Vec<AnthropicMessage>) {
    let mut system_prompt = None;
    let mut anthropic_messages = Vec::new();

    for message in messages {
        match message.role {
            MessageRole::System => {
                system_prompt = Some(message.content.clone());
            },
            MessageRole::User | MessageRole::Assistant => {
                let role = match message.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => unreachable!(),
                }
                .to_string();

                let content = if message.parts.is_empty() {
                    AnthropicContent::Text(message.content.clone())
                } else {
                    AnthropicContent::Blocks(convert_to_blocks(message))
                };

                anthropic_messages.push(AnthropicMessage { role, content });
            },
        }
    }

    (system_prompt, anthropic_messages)
}

fn convert_to_blocks(message: &AiMessage) -> Vec<AnthropicContentBlock> {
    let mut blocks = Vec::new();

    if !message.content.is_empty() {
        blocks.push(AnthropicContentBlock::Text {
            text: message.content.clone(),
        });
    }

    for part in &message.parts {
        match part {
            AiContentPart::Text { text } => {
                blocks.push(AnthropicContentBlock::Text { text: text.clone() });
            },
            AiContentPart::Image { mime_type, data } => {
                blocks.push(AnthropicContentBlock::Image {
                    source: AnthropicImageSource::Base64 {
                        media_type: mime_type.clone(),
                        data: data.clone(),
                    },
                });
            },
            AiContentPart::Audio { .. } | AiContentPart::Video { .. } => {},
        }
    }

    blocks
}

pub fn convert_tools(tools: Vec<McpTool>) -> Vec<AnthropicTool> {
    tools
        .into_iter()
        .map(|tool| AnthropicTool {
            name: tool.name,
            description: tool.description,
            input_schema: tool.input_schema.unwrap_or(json!({
                "type": "object",
                "properties": {}
            })),
        })
        .collect()
}
