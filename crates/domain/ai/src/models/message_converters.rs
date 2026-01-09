use crate::models::ai::{AiContentPart, AiMessage, MessageRole};
use crate::models::providers::anthropic::AnthropicMessage;
use crate::models::providers::gemini::GeminiContent;
use crate::models::providers::openai::{
    OpenAiContentPart, OpenAiImageUrl, OpenAiMessage, OpenAiMessageContent,
};

impl From<&AiMessage> for OpenAiMessage {
    fn from(message: &AiMessage) -> Self {
        let role = match message.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
        }
        .to_string();

        let content = if message.parts.is_empty() {
            OpenAiMessageContent::Text(message.content.clone())
        } else {
            OpenAiMessageContent::Parts(convert_to_openai_parts(message))
        };

        Self { role, content }
    }
}

fn convert_to_openai_parts(message: &AiMessage) -> Vec<OpenAiContentPart> {
    let mut parts = Vec::new();

    if !message.content.is_empty() {
        parts.push(OpenAiContentPart::Text {
            text: message.content.clone(),
        });
    }

    for part in &message.parts {
        match part {
            AiContentPart::Text { text } => {
                parts.push(OpenAiContentPart::Text { text: text.clone() });
            },
            AiContentPart::Image { mime_type, data } => {
                let data_uri = format!("data:{mime_type};base64,{data}");
                parts.push(OpenAiContentPart::ImageUrl {
                    image_url: OpenAiImageUrl {
                        url: data_uri,
                        detail: None,
                    },
                });
            },
            AiContentPart::Audio { .. } => {
                tracing::warn!("Audio content not supported by OpenAI vision, skipping");
            },
            AiContentPart::Video { .. } => {
                tracing::warn!("Video content not supported by OpenAI vision, skipping");
            },
        }
    }

    parts
}

impl From<&AiMessage> for AnthropicMessage {
    fn from(message: &AiMessage) -> Self {
        use crate::models::providers::anthropic::AnthropicContent;

        let role = match message.role {
            MessageRole::System | MessageRole::Assistant => "assistant",
            MessageRole::User => "user",
        }
        .to_string();

        let content = if message.parts.is_empty() {
            AnthropicContent::Text(message.content.clone())
        } else {
            AnthropicContent::Blocks(convert_to_anthropic_blocks(message))
        };

        Self { role, content }
    }
}

fn convert_to_anthropic_blocks(
    message: &AiMessage,
) -> Vec<crate::models::providers::anthropic::AnthropicContentBlock> {
    use crate::models::providers::anthropic::{AnthropicContentBlock, AnthropicImageSource};

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
            AiContentPart::Audio { .. } => {
                tracing::warn!("Audio content not supported by Anthropic, skipping");
            },
            AiContentPart::Video { .. } => {
                tracing::warn!("Video content not supported by Anthropic, skipping");
            },
        }
    }

    blocks
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
