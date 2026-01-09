use crate::models::ai::{AiContentPart, AiMessage, MessageRole};
use crate::models::providers::gemini::{GeminiContent, GeminiInlineData, GeminiPart};
use crate::models::tools::CallToolResult;
use rmcp::model::RawContent;
use serde_json::json;

pub fn convert_messages(messages: &[AiMessage]) -> Vec<GeminiContent> {
    let mut contents = Vec::new();
    let mut system_content = Vec::new();

    for message in messages {
        let role = match message.role {
            MessageRole::System => {
                system_content.push(message.content.clone());
                continue;
            },
            MessageRole::User => "user",
            MessageRole::Assistant => "model",
        }
        .to_string();

        let parts = convert_message_parts(message);
        contents.push(GeminiContent { role, parts });
    }

    if !system_content.is_empty() {
        contents.insert(
            0,
            GeminiContent {
                role: "user".to_string(),
                parts: vec![GeminiPart::Text {
                    text: system_content.join("\n"),
                }],
            },
        );
    }

    contents
}

fn convert_message_parts(message: &AiMessage) -> Vec<GeminiPart> {
    if message.parts.is_empty() {
        return vec![GeminiPart::Text {
            text: message.content.clone(),
        }];
    }

    message
        .parts
        .iter()
        .map(|part| match part {
            AiContentPart::Text { text } => GeminiPart::Text { text: text.clone() },
            AiContentPart::Image { mime_type, data }
            | AiContentPart::Audio { mime_type, data }
            | AiContentPart::Video { mime_type, data } => GeminiPart::InlineData {
                inline_data: GeminiInlineData {
                    mime_type: mime_type.clone(),
                    data: data.clone(),
                },
            },
        })
        .collect()
}

pub fn convert_tool_result_to_json(tool_result: &CallToolResult) -> serde_json::Value {
    if tool_result.is_error.unwrap_or(false) {
        let error_text = tool_result
            .content
            .iter()
            .filter_map(|c| match &c.raw {
                RawContent::Text(text_content) => Some(text_content.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        return json!({"error": error_text});
    }

    if let Some(structured) = &tool_result.structured_content {
        return structured.clone();
    }

    let content_json: Vec<serde_json::Value> = tool_result
        .content
        .iter()
        .map(|c| match &c.raw {
            RawContent::Text(text_content) => json!({"type": "text", "text": text_content.text}),
            RawContent::Image(image_content) => {
                json!({"type": "image", "data": image_content.data, "mimeType": image_content.mime_type})
            },
            RawContent::ResourceLink(resource) => {
                json!({"type": "resource", "uri": resource.uri, "mimeType": resource.mime_type})
            },
            _ => json!({"type": "unknown"}),
        })
        .collect();

    json!({"content": content_json})
}
