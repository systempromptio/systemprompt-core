use serde_json::json;

use crate::models::ai::{AiMessage, MessageRole};
use crate::models::providers::anthropic::{AnthropicContent, AnthropicMessage, AnthropicTool};
use crate::models::tools::McpTool;

pub fn convert_messages(messages: &[AiMessage]) -> (Option<String>, Vec<AnthropicMessage>) {
    let mut system_prompt = None;
    let mut anthropic_messages = Vec::new();

    for message in messages {
        match message.role {
            MessageRole::System => {
                system_prompt = Some(message.content.clone());
            },
            MessageRole::User => {
                anthropic_messages.push(AnthropicMessage {
                    role: "user".to_string(),
                    content: AnthropicContent::Text(message.content.clone()),
                });
            },
            MessageRole::Assistant => {
                anthropic_messages.push(AnthropicMessage {
                    role: "assistant".to_string(),
                    content: AnthropicContent::Text(message.content.clone()),
                });
            },
        }
    }

    (system_prompt, anthropic_messages)
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
