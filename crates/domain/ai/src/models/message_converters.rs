use crate::models::ai::{AiMessage, MessageRole};
use crate::models::providers::anthropic::AnthropicMessage;
use crate::models::providers::gemini::GeminiContent;
use crate::models::providers::openai::OpenAiMessage;

impl From<&AiMessage> for OpenAiMessage {
    fn from(message: &AiMessage) -> Self {
        Self {
            role: match message.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
            }
            .to_string(),
            content: message.content.clone(),
        }
    }
}

impl From<&AiMessage> for AnthropicMessage {
    fn from(message: &AiMessage) -> Self {
        use crate::models::providers::anthropic::AnthropicContent;

        Self {
            role: match message.role {
                MessageRole::System | MessageRole::Assistant => "assistant",
                MessageRole::User => "user",
            }
            .to_string(),
            content: AnthropicContent::Text(message.content.clone()),
        }
    }
}

impl From<&AiMessage> for GeminiContent {
    fn from(message: &AiMessage) -> Self {
        use crate::models::providers::gemini::GeminiPart;

        Self {
            role: match message.role {
                MessageRole::System | MessageRole::User => "user",
                MessageRole::Assistant => "model",
            }
            .to_string(),
            parts: vec![GeminiPart::Text {
                text: message.content.clone(),
            }],
        }
    }
}
