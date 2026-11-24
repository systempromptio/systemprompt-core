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

// Why: Anthropic rejects an assistant turn whose thinking blocks do not lead
// and a user turn whose tool_result blocks do not lead, so those parts are
// placed ahead of the message's own text.
fn message_content(message: &AiMessage) -> Vec<CanonicalContent> {
    let (leading, rest): (Vec<&AiContentPart>, Vec<&AiContentPart>) =
        message.parts.iter().partition(|part| {
            matches!(
                part,
                AiContentPart::Thinking { .. } | AiContentPart::ToolResult { .. }
            )
        });
    let mut content: Vec<CanonicalContent> = leading.into_iter().filter_map(part_content).collect();
    if !message.content.is_empty() {
        content.push(CanonicalContent::text(message.content.clone()));
    }
    content.extend(rest.into_iter().filter_map(part_content));
    content
}

fn part_content(part: &AiContentPart) -> Option<CanonicalContent> {
    match part {
        AiContentPart::Text { text } => Some(CanonicalContent::text(text.clone())),
        AiContentPart::Image { mime_type, data } => {
            Some(CanonicalContent::image(ImageSource::Base64 {
                media_type: mime_type.clone(),
                data: data.clone(),
                detail: None,
            }))
        },
        AiContentPart::ToolUse {
            id,
            name,
            input,
            signature,
        } => Some(CanonicalContent::ToolUse {
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
        } => Some(CanonicalContent::ToolResult {
            tool_use_id: tool_use_id.as_str().to_owned(),
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
        } => Some(CanonicalContent::Thinking {
            text: text.clone(),
            signature: signature.clone(),
            id: id.clone(),
            encrypted_content: encrypted_content.clone(),
        }),
        AiContentPart::Audio { .. } | AiContentPart::Video { .. } => None,
    }
}
