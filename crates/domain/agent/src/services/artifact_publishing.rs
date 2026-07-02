//! Artifact persistence for agent and MCP tool output.
//!
//! Persists artifacts, enriches them with skill metadata, validates
//! execution-id foreign keys, and creates the accompanying conversation
//! messages for direct MCP calls.

use crate::services::shared::{AgentServiceError, Result};
use serde_json::json;
use std::sync::Arc;

use crate::models::a2a::{Artifact, Message, MessageRole, Part, TextPart};
use crate::repository::content::ArtifactRepository;
use crate::repository::execution::ExecutionStepRepository;
use crate::services::{MessageService, SkillService};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, McpExecutionId, MessageId, TaskId};
use systemprompt_models::RequestContext;
use systemprompt_models::execution::CallSource;

#[derive(Debug)]
pub struct PublishFromMcpParams<'a> {
    pub artifact: &'a Artifact,
    pub task_id: &'a TaskId,
    pub context_id: &'a ContextId,
    pub tool_name: &'a str,
    pub tool_args: &'a serde_json::Value,
    pub request_context: &'a RequestContext,
    pub call_source: CallSource,
}

pub struct ArtifactPublishingService {
    artifact_repo: ArtifactRepository,
    skill_service: SkillService,
    message_service: MessageService,
    execution_repo: Option<Arc<ExecutionStepRepository>>,
}

impl std::fmt::Debug for ArtifactPublishingService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArtifactPublishingService")
            .finish_non_exhaustive()
    }
}

impl ArtifactPublishingService {
    pub fn new(db_pool: &DbPool) -> Result<Self> {
        let execution_repo = ExecutionStepRepository::new(db_pool)
            .map(Arc::new)
            .map_err(|e| {
                tracing::debug!(error = %e, "ExecutionStepRepository not available, FK validation disabled");
                e
            })
            .ok();

        Ok(Self {
            artifact_repo: ArtifactRepository::new(db_pool)?,
            skill_service: SkillService::new()?,
            message_service: MessageService::new(db_pool)?,
            execution_repo,
        })
    }

    async fn execution_id_exists(&self, mcp_execution_id: &McpExecutionId) -> bool {
        let Some(repo) = &self.execution_repo else {
            tracing::warn!("ExecutionStepRepository not available for FK validation");
            return false;
        };

        match repo.mcp_execution_id_exists(mcp_execution_id).await {
            Ok(exists) => exists,
            Err(e) => {
                tracing::warn!(
                    mcp_execution_id = %mcp_execution_id,
                    error = %e,
                    "Failed to check mcp_execution_id existence"
                );
                false
            },
        }
    }

    async fn validate_execution_id(&self, artifact: &Artifact) -> Artifact {
        let mut validated = artifact.clone();

        if let Some(exec_id) = &validated.metadata.mcp_execution_id {
            let exec_id = McpExecutionId::new(exec_id);
            if !self.execution_id_exists(&exec_id).await {
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

        if let Some(skill_id) = &enriched.metadata.skill_id
            && enriched.metadata.skill_name.is_none()
        {
            match self.skill_service.load_skill_metadata(skill_id).await {
                Ok(meta) => enriched.metadata.skill_name = Some(meta.name),
                Err(e) => tracing::debug!(
                    skill_id = %skill_id,
                    error = %e,
                    "skill metadata not available; leaving skill_name empty"
                ),
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
            .map_err(|e| {
                AgentServiceError::Internal(format!("Failed to persist artifact: {}", e))
            })?;

        tracing::info!(
            artifact_id = %validated_artifact.id,
            "Artifact persisted to database"
        );

        Ok(())
    }

    pub async fn publish_from_mcp(&self, params: PublishFromMcpParams<'_>) -> Result<()> {
        let enriched_artifact = self.enrich_artifact_with_skill(params.artifact).await;
        let validated_artifact = self.validate_execution_id(&enriched_artifact).await;

        tracing::info!(
            artifact_id = %validated_artifact.id,
            artifact_type = %validated_artifact.metadata.artifact_type,
            tool_name = %params.tool_name,
            task_id = %params.task_id,
            context_id = %params.context_id,
            source = "mcp_direct_call",
            "Publishing artifact from direct MCP tool execution"
        );

        self.artifact_repo
            .create_artifact(params.task_id, params.context_id, &validated_artifact)
            .await
            .map_err(|e| {
                AgentServiceError::Internal(format!("Failed to persist artifact: {}", e))
            })?;

        tracing::info!(
            artifact_id = %validated_artifact.id,
            "Artifact persisted to database"
        );

        if params.call_source == CallSource::Direct {
            self.create_direct_call_messages(&params, &validated_artifact)
                .await?;
        } else {
            tracing::info!(
                "Skipping message creation for agentic tool call (AI will synthesize response)"
            );
        }

        Ok(())
    }

    async fn create_direct_call_messages(
        &self,
        params: &PublishFromMcpParams<'_>,
        artifact: &Artifact,
    ) -> Result<()> {
        tracing::info!("Creating technical messages for direct MCP call");

        let (user_message_id, _seq) = self
            .message_service
            .create_tool_execution_message(super::CreateToolExecutionMessageParams {
                task_id: params.task_id,
                context_id: params.context_id,
                tool_name: params.tool_name,
                tool_args: params.tool_args,
                request_context: params.request_context,
            })
            .await?;

        tracing::info!(
            message_id = %user_message_id,
            tool_name = %params.tool_name,
            "Created synthetic user message for MCP tool"
        );

        let agent_message = Message {
            role: MessageRole::Agent,
            message_id: MessageId::generate(),
            task_id: Some(params.task_id.clone()),
            context_id: params.context_id.clone(),
            parts: vec![Part::Text(TextPart {
                text: format!(
                    "Tool execution completed successfully.\n\nCreated artifact: {} (type: {})",
                    artifact.id, artifact.metadata.artifact_type
                ),
            })],
            metadata: Some(json!({
                "source": "mcp_direct_call_response",
                "tool_name": params.tool_name,
                "artifact_id": artifact.id,
                "artifact_type": artifact.metadata.artifact_type,
            })),
            extensions: None,
            reference_task_ids: None,
        };

        self.message_service
            .persist_messages(super::PersistMessagesParams {
                task_id: params.task_id,
                context_id: params.context_id,
                messages: vec![agent_message],
                user_id: Some(params.request_context.user_id()),
                session_id: params.request_context.session_id(),
                trace_id: params.request_context.trace_id(),
            })
            .await?;

        tracing::info!("Created agent response message with artifact reference");

        Ok(())
    }
}
