//! Turning the parts of an A2A message into model-ready content: the prompt
//! text plus decoded file parts the model can consume.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;

use crate::models::a2a::{Message, Part, PartContent};
use systemprompt_models::{
    AiContentPart, is_supported_audio, is_supported_image, is_supported_text, is_supported_video,
};

pub fn extract_message_content(message: &Message) -> (String, Vec<AiContentPart>) {
    let mut text_content = String::new();
    let mut content_parts = Vec::new();

    for part in &message.parts {
        match &part.content {
            PartContent::Text(text) => {
                if text_content.is_empty() {
                    text_content.clone_from(text);
                }
                content_parts.push(AiContentPart::text(text));
            },
            PartContent::Raw(bytes) => {
                if let Some(content_part) = file_to_content_part(part, bytes) {
                    content_parts.push(content_part);
                }
            },
            PartContent::Url(_) | PartContent::Data(_) => {},
        }
    }

    (text_content, content_parts)
}

fn file_to_content_part(part: &Part, bytes: &str) -> Option<AiContentPart> {
    let mime_type = part.media_type.as_deref()?;
    let file_name = part.filename.as_deref().unwrap_or("unnamed");

    if is_supported_image(mime_type) {
        return Some(AiContentPart::image(mime_type, bytes));
    }

    if is_supported_audio(mime_type) {
        return Some(AiContentPart::audio(mime_type, bytes));
    }

    if is_supported_video(mime_type) {
        return Some(AiContentPart::video(mime_type, bytes));
    }

    if is_supported_text(mime_type) {
        return decode_text_file(bytes, file_name, mime_type);
    }

    tracing::warn!(
        file_name = %file_name,
        mime_type = %mime_type,
        "Unsupported file type - file will not be sent to AI"
    );
    None
}

fn decode_text_file(bytes: &str, file_name: &str, mime_type: &str) -> Option<AiContentPart> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(bytes)
        .map_err(|e| {
            tracing::warn!(
                file_name = %file_name,
                mime_type = %mime_type,
                error = %e,
                "Failed to decode base64 text file"
            );
            e
        })
        .ok()?;

    let text_content = String::from_utf8(decoded)
        .map_err(|e| {
            tracing::warn!(
                file_name = %file_name,
                mime_type = %mime_type,
                error = %e,
                "Failed to decode text file as UTF-8"
            );
            e
        })
        .ok()?;

    let formatted = format!("[File: {file_name} ({mime_type})]\n{text_content}");
    Some(AiContentPart::text(formatted))
}
