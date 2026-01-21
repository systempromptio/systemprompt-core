mod helpers;
mod processing;

use anyhow::{anyhow, Result};
use base64::Engine;
use std::sync::Arc;

use crate::models::a2a::{FilePart, Message, Part};
use crate::models::AgentRuntimeInfo;
use crate::repository::execution::ExecutionStepRepository;
use crate::services::{ContextService, SkillService};
use systemprompt_models::{
    is_supported_audio, is_supported_image, is_supported_text, is_supported_video, AiContentPart,
    AiProvider,
};

pub use helpers::{build_artifacts_from_results, synthesize_final_response};

#[allow(missing_debug_implementations)]
pub struct StreamProcessor {
    pub ai_service: Arc<dyn AiProvider>,
    pub context_service: ContextService,
    pub skill_service: Arc<SkillService>,
    pub execution_step_repo: Arc<ExecutionStepRepository>,
}

impl StreamProcessor {
    pub fn extract_message_text(message: &Message) -> Result<String> {
        for part in &message.parts {
            if let Part::Text(text_part) = part {
                return Ok(text_part.text.clone());
            }
        }
        Err(anyhow!("No text content found in message"))
    }

    pub fn extract_message_content(message: &Message) -> (String, Vec<AiContentPart>) {
        let mut text_content = String::new();
        let mut content_parts = Vec::new();

        for part in &message.parts {
            match part {
                Part::Text(text_part) => {
                    if text_content.is_empty() {
                        text_content.clone_from(&text_part.text);
                    }
                    content_parts.push(AiContentPart::text(&text_part.text));
                },
                Part::File(file_part) => {
                    if let Some(content_part) = Self::file_to_content_part(file_part) {
                        content_parts.push(content_part);
                    }
                },
                Part::Data(_) => {},
            }
        }

        (text_content, content_parts)
    }

    fn file_to_content_part(file_part: &FilePart) -> Option<AiContentPart> {
        let mime_type = file_part.file.mime_type.as_deref()?;
        let file_name = file_part.file.name.as_deref().unwrap_or("unnamed");

        if is_supported_image(mime_type) {
            return Some(AiContentPart::image(mime_type, &file_part.file.bytes));
        }

        if is_supported_audio(mime_type) {
            return Some(AiContentPart::audio(mime_type, &file_part.file.bytes));
        }

        if is_supported_video(mime_type) {
            return Some(AiContentPart::video(mime_type, &file_part.file.bytes));
        }

        if is_supported_text(mime_type) {
            return Self::decode_text_file(file_part, file_name, mime_type);
        }

        tracing::warn!(
            file_name = %file_name,
            mime_type = %mime_type,
            "Unsupported file type - file will not be sent to AI"
        );
        None
    }

    fn decode_text_file(
        file_part: &FilePart,
        file_name: &str,
        mime_type: &str,
    ) -> Option<AiContentPart> {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&file_part.file.bytes)
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
}
