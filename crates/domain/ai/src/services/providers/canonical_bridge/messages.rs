//! Agent messages mapped onto canonical system blocks and turns, including
//! the structured tool-use, tool-result and thinking parts of a replayed turn.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_wire::canonical::{
    CanonicalContent, CanonicalMessage, ImageSource, Role, SystemBlock,
};

use crate::models::ai::{AiContentPart, AiMessage, MessageRole};

pub(super) fn messages_to_canonical(
    messages: &[AiMessage],
) -> (Vec<SystemBlock>, Vec<CanonicalMessage>) {
    let mut system: Option<String> = None;
    let mut out: Vec<CanonicalMessage> = Vec::new();
    for message in messages {
        match message.role {
            MessageRole::System => match &mut system {
                Some(existing) => {
                    existing.push('\n');
                    existing.push_str(&message.content);
                },
                None => system = Some(message.content.clone()),
            },
            MessageRole::User | MessageRole::Assistant => {
                let role = if matches!(message.role, MessageRole::Assistant) {
                    Role::Assistant
                } else {
                    Role::User
                };
                let content = message_content(message);
                if !content.is_empty() {
                    out.push(CanonicalMessage { role, content });
                }
            },
        }
    }
    (system.map(SystemBlock::text).into_iter().collect(), out)
}

fn message_content(message: &AiMessage) -> Vec<CanonicalContent> {
    let mut content: Vec<CanonicalContent> = Vec::new();
    if !message.content.is_empty() {
        content.push(CanonicalContent::text(message.content.clone()));
    }
    for part in &message.parts {
        match part {
            AiContentPart::Text { text } => content.push(CanonicalContent::text(text.clone())),
            AiContentPart::Image { mime_type, data } => {
                content.push(CanonicalContent::image(ImageSource::Base64 {
                    media_type: mime_type.clone(),
                    data: data.clone(),
                    detail: None,
                }));
            },
            AiContentPart::ToolUse {
                id,
                name,
                input,
                signature,
            } => content.push(CanonicalContent::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
                signature: signature.clone(),
                cache_control: None,
            }),
            AiContentPart::ToolResult {
                tool_use_id,
                content: result,
                is_error,
            } => content.push(CanonicalContent::ToolResult {
                tool_use_id: tool_use_id.clone(),
                content: vec![CanonicalContent::text(result.clone())],
                is_error: *is_error,
                structured_content: None,
                meta: None,
                cache_control: None,
            }),
            AiContentPart::Thinking {
                text,
                signature,
                id,
                encrypted_content,
            } => content.push(CanonicalContent::Thinking {
                text: text.clone(),
                signature: signature.clone(),
                id: id.clone(),
                encrypted_content: encrypted_content.clone(),
            }),
            AiContentPart::Audio { .. } | AiContentPart::Video { .. } => {},
        }
    }
    content
}
