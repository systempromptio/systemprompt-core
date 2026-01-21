use anyhow::{anyhow, Result};
use base64::Engine;
use systemprompt_database::DbPool;
use systemprompt_models::{
    is_supported_audio, is_supported_image, is_supported_text, is_supported_video, AiContentPart,
    AiMessage, MessageRole,
};

use crate::models::a2a::{Artifact, FilePart, Message, Part};
use crate::repository::task::TaskRepository;

#[derive(Debug, Clone)]
pub struct ContextService {
    task_repo: TaskRepository,
}

impl ContextService {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            task_repo: TaskRepository::new(db_pool),
        }
    }

    pub async fn load_conversation_history(&self, context_id: &str) -> Result<Vec<AiMessage>> {
        let context_id_typed = systemprompt_identifiers::ContextId::new(context_id);
        let tasks = self
            .task_repo
            .list_tasks_by_context(&context_id_typed)
            .await
            .map_err(|e| anyhow!("Failed to load conversation history: {}", e))?;

        let mut history_messages = Vec::new();

        for task in tasks {
            if let Some(task_history) = task.history {
                for msg in task_history {
                    let (text, parts) = match Self::extract_message_content(&msg) {
                        Ok((t, p)) if !t.is_empty() || !p.is_empty() => (t, p),
                        Ok(_) => continue,
                        Err(e) => {
                            tracing::warn!(error = %e, "Failed to extract message content");
                            continue;
                        },
                    };

                    let role = match msg.role.as_str() {
                        "user" => MessageRole::User,
                        "agent" => MessageRole::Assistant,
                        _ => continue,
                    };

                    history_messages.push(AiMessage {
                        role,
                        content: text,
                        parts,
                    });
                }
            }

            if let Some(artifacts) = task.artifacts {
                for artifact in artifacts {
                    if let Ok(artifact_content) = Self::serialize_artifact_for_context(&artifact) {
                        history_messages.push(AiMessage {
                            role: MessageRole::Assistant,
                            content: artifact_content,
                            parts: Vec::new(),
                        });
                    }
                }
            }
        }

        Ok(history_messages)
    }

    fn extract_message_content(message: &Message) -> Result<(String, Vec<AiContentPart>)> {
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

        Ok((text_content, content_parts))
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

    fn serialize_artifact_for_context(artifact: &Artifact) -> Result<String> {
        let artifact_name = artifact
            .name
            .as_ref()
            .map(String::as_str)
            .unwrap_or("unnamed");

        let mut content = format!(
            "[Artifact: {} (type: {}, id: {})]",
            artifact_name, artifact.metadata.artifact_type, artifact.id
        );

        if let Some(description) = &artifact.description {
            if !description.is_empty() {
                let truncated = if description.len() > 300 {
                    format!("{}...", &description[..300])
                } else {
                    description.clone()
                };
                content.push_str(&format!("\n{truncated}"));
            }
        }

        Ok(content)
    }
}
