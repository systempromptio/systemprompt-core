use anyhow::{anyhow, Result};
use serde_json::json;

use crate::models::a2a::{Artifact, Message, Part, TextPart};
use crate::repository::content::{ArtifactRepository, SkillRepository};
use crate::services::MessageService;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::execution::CallSource;
use systemprompt_models::RequestContext;

pub struct ArtifactPublishingService {
    db_pool: DbPool,
    artifact_repo: ArtifactRepository,
    skill_repo: SkillRepository,
    message_service: MessageService,
}

impl std::fmt::Debug for ArtifactPublishingService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArtifactPublishingService")
            .finish_non_exhaustive()
    }
}

impl ArtifactPublishingService {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool: db_pool.clone(),
            artifact_repo: ArtifactRepository::new(db_pool.clone()),
            skill_repo: SkillRepository::new(db_pool.clone()),
            message_service: MessageService::new(db_pool),
        }
    }

    async fn execution_id_exists(&self, mcp_execution_id: &str) -> bool {
        let Some(pool) = self.db_pool.as_ref().get_postgres_pool() else {
            tracing::warn!("PostgreSQL pool not available for FK validation");
            return false;
        };

        match sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM mcp_tool_executions WHERE mcp_execution_id = $1) as "exists!""#,
            mcp_execution_id
        )
        .fetch_one(pool.as_ref())
        .await
        {
            Ok(exists) => exists,
            Err(e) => {
                tracing::warn!(
                    mcp_execution_id = %mcp_execution_id,
                    error = %e,
                    "Failed to check mcp_execution_id existence"
                );
                false
            }
        }
    }

    async fn validate_execution_id(&self, artifact: &Artifact) -> Artifact {
        let mut validated = artifact.clone();

        if let Some(exec_id) = &validated.metadata.mcp_execution_id {
            if !self.execution_id_exists(exec_id).await {
                tracing::warn!(
                    mcp_execution_id = %exec_id,
                    artifact_id = %artifact.id,
                    "mcp_execution_id not found in mcp_tool_executions, setting to NULL"
                );
                validated.metadata.mcp_execution_id = None;
            }
        }

        validated
    }

    async fn enrich_artifact_with_skill(&self, artifact: &Artifact) -> Artifact {
        let mut enriched = artifact.clone();

        if let Some(skill_id) = &enriched.metadata.skill_id {
            if enriched.metadata.skill_name.is_none() {
                let skill_id_typed = systemprompt_identifiers::SkillId::new(skill_id);
                if let Ok(Some(skill)) = self.skill_repo.get_by_skill_id(&skill_id_typed).await {
                    enriched.metadata.skill_name = Some(skill.name);
                }
            }
        }

        enriched
    }

    pub async fn publish_from_a2a(
        &self,
        artifact: &Artifact,
        task_id: &TaskId,
        context_id: &ContextId,
    ) -> Result<()> {
        let enriched_artifact = self.enrich_artifact_with_skill(artifact).await;
        let validated_artifact = self.validate_execution_id(&enriched_artifact).await;

        tracing::info!(
            artifact_id = %validated_artifact.id,
            artifact_type = %validated_artifact.metadata.artifact_type,
            task_id = %task_id,
            context_id = %context_id,
            source = "a2a_agent",
            "Publishing artifact from A2A agent"
        );

        self.artifact_repo
            .create_artifact(task_id, context_id, &validated_artifact)
            .await
            .map_err(|e| anyhow!("Failed to persist artifact: {}", e))?;

        tracing::info!(
            artifact_id = %validated_artifact.id,
            "Artifact persisted to database"
        );

        Ok(())
    }

    pub async fn publish_from_mcp(
        &self,
        artifact: &Artifact,
        task_id: &TaskId,
        context_id: &ContextId,
        tool_name: &str,
        tool_args: &serde_json::Value,
        request_context: &RequestContext,
        call_source: CallSource,
    ) -> Result<()> {
        let enriched_artifact = self.enrich_artifact_with_skill(artifact).await;
        let validated_artifact = self.validate_execution_id(&enriched_artifact).await;

        tracing::info!(
            artifact_id = %validated_artifact.id,
            artifact_type = %validated_artifact.metadata.artifact_type,
            tool_name = %tool_name,
            task_id = %task_id,
            context_id = %context_id,
            source = "mcp_direct_call",
            "Publishing artifact from direct MCP tool execution"
        );

        self.artifact_repo
            .create_artifact(task_id, context_id, &validated_artifact)
            .await
            .map_err(|e| anyhow!("Failed to persist artifact: {}", e))?;

        tracing::info!(
            artifact_id = %validated_artifact.id,
            "Artifact persisted to database"
        );

        if call_source == CallSource::Direct {
            tracing::info!("Creating technical messages for direct MCP call");

            let (user_message_id, _seq) = self
                .message_service
                .create_tool_execution_message(
                    task_id,
                    context_id,
                    tool_name,
                    tool_args,
                    request_context,
                )
                .await?;

            tracing::info!(
                message_id = %user_message_id,
                tool_name = %tool_name,
                "Created synthetic user message for MCP tool"
            );

            let agent_message = Message {
                role: "agent".to_string(),
                id: MessageId::generate(),
                task_id: Some(task_id.clone()),
                context_id: context_id.clone(),
                kind: "message".to_string(),
                parts: vec![Part::Text(TextPart {
                    text: format!(
                        "Tool execution completed successfully.\n\nCreated artifact: {} (type: {})",
                        validated_artifact.id, validated_artifact.metadata.artifact_type
                    ),
                })],
                metadata: Some(json!({
                    "source": "mcp_direct_call_response",
                    "tool_name": tool_name,
                    "artifact_id": validated_artifact.id,
                    "artifact_type": validated_artifact.metadata.artifact_type,
                })),
                extensions: None,
                reference_task_ids: None,
            };

            self.message_service
                .persist_messages(
                    task_id,
                    context_id,
                    vec![agent_message],
                    Some(request_context.user_id()),
                    request_context.session_id(),
                    request_context.trace_id(),
                )
                .await?;

            tracing::info!("Created agent response message with artifact reference");
        } else {
            tracing::info!(
                "Skipping message creation for agentic tool call (AI will synthesize response)"
            );
        }

        Ok(())
    }
}
