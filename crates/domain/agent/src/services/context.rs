//! Reconstructing conversation history for a context into AI-ready messages,
//! including decoding file parts and serializing artifacts as context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::a2a_server::processing::message::content::extract_message_content;
use crate::services::shared::{AgentServiceError, Result};
use systemprompt_models::text::truncate_with_ellipsis;
use systemprompt_models::{AiMessage, MessageRole};

use crate::models::a2a::Artifact;
use crate::repository::task::TaskRepository;

#[derive(Debug, Clone)]
pub struct ContextService {
    task_repo: TaskRepository,
}

impl ContextService {
    #[must_use]
    pub const fn new(task_repo: TaskRepository) -> Self {
        Self { task_repo }
    }

    pub async fn load_conversation_history(
        &self,
        context_id: &systemprompt_identifiers::ContextId,
    ) -> Result<Vec<AiMessage>> {
        let tasks = self
            .task_repo
            .list_tasks_by_context(context_id)
            .await
            .map_err(|e| {
                AgentServiceError::Internal(format!("Failed to load conversation history: {}", e))
            })?;

        let mut history_messages = Vec::new();

        for task in tasks {
            if let Some(task_history) = task.history {
                for msg in task_history {
                    let (text, parts) = extract_message_content(&msg);
                    if text.is_empty() && parts.is_empty() {
                        continue;
                    }

                    let role = match msg.role {
                        crate::models::a2a::MessageRole::User => MessageRole::User,
                        crate::models::a2a::MessageRole::Agent => MessageRole::Assistant,
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
                    let artifact_content = Self::serialize_artifact_for_context(&artifact);
                    history_messages.push(AiMessage {
                        role: MessageRole::Assistant,
                        content: artifact_content,
                        parts: Vec::new(),
                    });
                }
            }
        }

        Ok(history_messages)
    }

    fn serialize_artifact_for_context(artifact: &Artifact) -> String {
        let artifact_name = artifact.title.as_deref().unwrap_or("unnamed");

        let mut content = format!(
            "[Artifact: {} (type: {}, id: {})]",
            artifact_name, artifact.metadata.artifact_type, artifact.id
        );

        if let Some(description) = &artifact.description
            && !description.is_empty()
        {
            let truncated = truncate_with_ellipsis(description, 300);
            content.push_str(&format!("\n{truncated}"));
        }

        content
    }
}
