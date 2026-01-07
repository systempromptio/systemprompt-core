use anyhow::{anyhow, Result};
use systemprompt_core_database::DbPool;
use systemprompt_models::{AiMessage, MessageRole};

use crate::models::a2a::{Artifact, Message, Part};
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
                    let text = match Self::extract_message_text(&msg) {
                        Ok(t) if !t.is_empty() => t,
                        Ok(_) => continue,
                        Err(e) => {
                            tracing::warn!(error = %e, "Failed to extract message text");
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
                    });
                }
            }

            if let Some(artifacts) = task.artifacts {
                for artifact in artifacts {
                    if let Ok(artifact_content) = Self::serialize_artifact_for_context(&artifact) {
                        history_messages.push(AiMessage {
                            role: MessageRole::Assistant,
                            content: artifact_content,
                        });
                    }
                }
            }
        }

        Ok(history_messages)
    }

    fn extract_message_text(message: &Message) -> Result<String> {
        for part in &message.parts {
            if let Part::Text(text_part) = part {
                return Ok(text_part.text.clone());
            }
        }
        Err(anyhow!("No text content found in message"))
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
