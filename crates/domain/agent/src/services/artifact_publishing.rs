//! Artifact persistence for agent and MCP tool output.
//!
//! Persists artifacts, enriches them with skill metadata, verifies MCP
//! execution ids against the owning domain's ledger through
//! [`ToolExecutionLookup`](systemprompt_traits::ToolExecutionLookup), and
//! creates the accompanying conversation messages for direct MCP calls.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::shared::{AgentServiceError, Result};
use serde_json::json;

use std::sync::Arc;

use crate::models::a2a::{Artifact, Message, MessageRole, Part, TextPart};
use crate::repository::A2ARepositories;
use crate::repository::content::ArtifactRepository;
use crate::services::{MessageService, SkillService};
use systemprompt_identifiers::{ContextId, McpExecutionId, MessageId, TaskId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::execution::CallSource;
use systemprompt_traits::DynToolExecutionLookup;

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
    skill_service: Arc<SkillService>,
    message_service: MessageService,
    tool_executions: DynToolExecutionLookup,
}

impl std::fmt::Debug for ArtifactPublishingService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArtifactPublishingService")
            .finish_non_exhaustive()
    }
}

impl ArtifactPublishingService {
    #[must_use]
    pub fn new(repositories: &A2ARepositories, skill_service: Arc<SkillService>) -> Self {
        Self {
            artifact_repo: repositories.artifacts.clone(),
            skill_service,
            message_service: MessageService::new(repositories.tasks.clone()),
            tool_executions: repositories.tool_executions(),
        }
    }

    // Why: an unknown execution id is dropped so the artifact never points at
    // a ledger row that does not exist, but an unreachable ledger is an error
    // — treating it as "unknown" would silently detach every artifact.
    async fn validate_execution_id(&self, artifact: &Artifact) -> Result<Artifact> {
        let mut validated = artifact.clone();

        if let Some(exec_id) = &validated.metadata.mcp_execution_id {
            let exec_id = McpExecutionId::new(exec_id);
            let exists = self
                .tool_executions
                .execution_exists(&exec_id)
                .await
                .map_err(|e| {
                    AgentServiceError::Internal(format!(
                        "Failed to check mcp_execution_id {exec_id}: {e}"
                    ))
                })?;
            if !exists {
                tracing::warn!(
                    mcp_execution_id = %exec_id,
                    artifact_id = %artifact.id,
                    "mcp_execution_id not found in the tool-execution ledger, setting to NULL"
                );
                validated.metadata.mcp_execution_id = None;
            }
        }

        Ok(validated)
    }

    async fn enrich_artifact_with_skill(&self, artifact: &Artifact, owner: &UserId) -> Artifact {
        let mut enriched = artifact.clone();

        if let Some(skill_id) = &enriched.metadata.skill_id
            && enriched.metadata.skill_name.is_none()
        {
            match self
                .skill_service
                .load_skill_metadata(skill_id, owner)
                .await
            {
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
        owner: &UserId,
    ) -> Result<()> {
        let enriched_artifact = self.enrich_artifact_with_skill(artifact, owner).await;
        let validated_artifact = self.validate_execution_id(&enriched_artifact).await?;

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
            .map_err(|e| AgentServiceError::Internal(format!("Failed to persist artifact: {e}")))?;

        tracing::info!(
            artifact_id = %validated_artifact.id,
            "Artifact persisted to database"
        );

        Ok(())
    }

    pub async fn publish_from_mcp(&self, params: PublishFromMcpParams<'_>) -> Result<()> {
        let owner = params.request_context.user_id();
        let enriched_artifact = self
            .enrich_artifact_with_skill(params.artifact, owner)
            .await;
        let validated_artifact = self.validate_execution_id(&enriched_artifact).await?;

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
            .map_err(|e| AgentServiceError::Internal(format!("Failed to persist artifact: {e}")))?;

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
