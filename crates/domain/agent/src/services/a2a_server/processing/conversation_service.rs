use anyhow::{anyhow, Result};
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::ContextId;
use systemprompt_models::{AiContentPart, AiMessage, MessageRole};

use crate::models::a2a::{FilePart, Part};
use crate::models::{Artifact, Message};
use crate::repository::task::TaskRepository;

const SUPPORTED_IMAGE_TYPES: &[&str] = &["image/jpeg", "image/png", "image/gif", "image/webp"];
const SUPPORTED_AUDIO_TYPES: &[&str] = &[
    "audio/wav",
    "audio/mp3",
    "audio/mpeg",
    "audio/aiff",
    "audio/aac",
    "audio/ogg",
    "audio/flac",
];

#[derive(Debug)]
pub struct ConversationService {
    db_pool: DbPool,
}

impl ConversationService {
    pub const fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    pub async fn load_conversation_history(
        &self,
        context_id: &ContextId,
    ) -> Result<Vec<AiMessage>> {
        let task_repo = TaskRepository::new(self.db_pool.clone());
        let tasks = task_repo
            .list_tasks_by_context(context_id)
            .await
            .map_err(|e| anyhow!("Failed to load conversation history: {}", e))?;

        let mut history_messages = Vec::new();

        for task in tasks {
            if let Some(task_history) = task.history {
                for msg in task_history {
                    let (text, parts) = match self.extract_message_content(&msg) {
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
                    if let Ok(artifact_content) = self.serialize_artifact_for_context(&artifact) {
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

    fn extract_message_content(&self, message: &Message) -> Result<(String, Vec<AiContentPart>)> {
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
                    if let Some(content_part) = self.file_to_content_part(file_part) {
                        content_parts.push(content_part);
                    }
                },
                Part::Data(_) => {},
            }
        }

        Ok((text_content, content_parts))
    }

    fn file_to_content_part(&self, file_part: &FilePart) -> Option<AiContentPart> {
        let mime_type = file_part.file.mime_type.as_deref()?;

        if Self::is_supported_image(mime_type) {
            return Some(AiContentPart::image(mime_type, &file_part.file.bytes));
        }

        if Self::is_supported_audio(mime_type) {
            return Some(AiContentPart::audio(mime_type, &file_part.file.bytes));
        }

        None
    }

    fn is_supported_image(mime_type: &str) -> bool {
        SUPPORTED_IMAGE_TYPES
            .iter()
            .any(|&t| mime_type.starts_with(t))
    }

    fn is_supported_audio(mime_type: &str) -> bool {
        SUPPORTED_AUDIO_TYPES
            .iter()
            .any(|&t| mime_type.starts_with(t))
    }

    fn serialize_artifact_for_context(&self, artifact: &Artifact) -> Result<String> {
        let artifact_name = artifact
            .name
            .clone()
            .unwrap_or_else(|| "unnamed".to_string());

        let mut content = format!(
            "[Artifact: {} (type: {})]\n",
            artifact_name, artifact.metadata.artifact_type
        );

        for part in &artifact.parts {
            match part {
                Part::Text(text_part) => {
                    content.push_str(&text_part.text);
                    content.push('\n');
                },
                Part::Data(data_part) => {
                    let json_str = serde_json::to_string_pretty(&data_part.data)
                        .unwrap_or_else(|_| "{}".to_string());
                    content.push_str(&json_str);
                    content.push('\n');
                },
                Part::File(file_part) => {
                    if let Some(name) = &file_part.file.name {
                        content.push_str(&format!("[File: {}]\n", name));
                    }
                },
            }
        }

        Ok(content)
    }
}
