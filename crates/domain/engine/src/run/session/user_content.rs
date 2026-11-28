//! Mapping a stored user message to the model's content blocks: text, marked
//! as a steer when it arrived mid-run, and attached files as images, inline
//! text or documents.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::llm::Content;
use crate::prompt::notices;
use crate::store::{MessageWithParts, PartBody};

use super::attachments::{decode, parse_data_url};

pub(super) fn user_content(message: &MessageWithParts, steered: bool) -> Vec<Content> {
    let mut content = Vec::new();
    for part in &message.parts {
        match &part.body {
            PartBody::Text { text, synthetic } if !text.trim().is_empty() => {
                let text = if steered && !synthetic {
                    notices::steering(text)
                } else {
                    text.clone()
                };
                content.push(Content::Text { text });
            },
            PartBody::File { mime, name, url } => {
                if let Some(block) = file_content(mime, name.as_deref(), url) {
                    content.push(block);
                }
            },
            _ => {},
        }
    }
    content
}

fn file_content(mime: &str, name: Option<&str>, url: &str) -> Option<Content> {
    let (declared, data) = parse_data_url(url)?;
    let mime = if mime.is_empty() {
        declared
    } else {
        mime.to_owned()
    };
    if mime.starts_with("image/") {
        return Some(Content::Image { mime, data });
    }
    if mime.starts_with("text/") {
        let text = String::from_utf8(decode(&data)?).ok()?;
        let label = name.unwrap_or("attachment");
        return Some(Content::Text {
            text: format!("Attached file {label}:\n\n{text}"),
        });
    }
    Some(Content::Document {
        mime,
        data,
        name: name.map(str::to_owned),
    })
}
