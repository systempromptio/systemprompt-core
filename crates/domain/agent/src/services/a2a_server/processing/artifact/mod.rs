use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::{AgentName, AiToolCallId, ArtifactId, ContextId, TaskId};
use systemprompt_models::a2a::ArtifactMetadata;
use systemprompt_models::{AiProvider, CallToolResult, McpTool, RequestContext, ToolCall};

use crate::models::a2a::{Artifact, Part, TextPart};
use crate::services::mcp::{extract_artifact_id, extract_skill_id};

#[async_trait]
pub trait ToolProvider: Send + Sync {
    async fn list_available_tools_for_agent(
        &self,
        agent_name: &AgentName,
        context: &RequestContext,
    ) -> Result<Vec<McpTool>>;
}

#[async_trait]
pub trait ExecutionIdLookup: Send + Sync {
    async fn get_mcp_execution_id(&self, ai_tool_call_id: &AiToolCallId) -> Result<Option<String>>;
}

pub struct AiServiceToolProvider {
    ai_service: Arc<dyn AiProvider>,
}

impl std::fmt::Debug for AiServiceToolProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiServiceToolProvider")
            .field("ai_service", &"<AiProvider>")
            .finish()
    }
}

impl AiServiceToolProvider {
    pub fn new(ai_service: Arc<dyn AiProvider>) -> Self {
        Self { ai_service }
    }
}

#[async_trait]
impl ToolProvider for AiServiceToolProvider {
    async fn list_available_tools_for_agent(
        &self,
        agent_name: &AgentName,
        context: &RequestContext,
    ) -> Result<Vec<McpTool>> {
        self.ai_service
            .list_available_tools_for_agent(agent_name, context)
            .await
    }
}

#[derive(Debug)]
pub struct DatabaseExecutionIdLookup {
    db_pool: DbPool,
}

impl DatabaseExecutionIdLookup {
    pub const fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl ExecutionIdLookup for DatabaseExecutionIdLookup {
    async fn get_mcp_execution_id(&self, ai_tool_call_id: &AiToolCallId) -> Result<Option<String>> {
        use systemprompt_core_mcp::repository::ToolUsageRepository;

        let tool_usage_repo = ToolUsageRepository::new(&self.db_pool)?;
        match tool_usage_repo.find_by_ai_call_id(ai_tool_call_id).await {
            Ok(Some(exec_id)) => Ok(Some(exec_id.to_string())),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[derive(Debug)]
pub struct ArtifactBuilder {
    tool_calls: Vec<ToolCall>,
    tool_results: Vec<CallToolResult>,
    execution_lookup: Arc<DatabaseExecutionIdLookup>,
    context_id: String,
    task_id: String,
}

impl ArtifactBuilder {
    pub const fn new(
        tool_calls: Vec<ToolCall>,
        tool_results: Vec<CallToolResult>,
        execution_lookup: Arc<DatabaseExecutionIdLookup>,
        context_id: String,
        task_id: String,
    ) -> Self {
        Self {
            tool_calls,
            tool_results,
            execution_lookup,
            context_id,
            task_id,
        }
    }

    pub async fn build_artifacts(&self) -> Result<Vec<Artifact>> {
        let mut artifacts = Vec::new();

        for (index, result) in self.tool_results.iter().enumerate() {
            if let Some(structured_content) = &result.structured_content {
                if let Some(tool_call) = self.tool_calls.get(index) {
                    let mcp_execution_id = self
                        .execution_lookup
                        .get_mcp_execution_id(&tool_call.ai_tool_call_id)
                        .await
                        .unwrap_or(None);

                    let skill_id = extract_skill_id(structured_content);

                    let mut metadata = ArtifactMetadata {
                        artifact_type: "mcp_tool_result".to_string(),
                        context_id: ContextId::new(self.context_id.clone()),
                        created_at: Utc::now().to_rfc3339(),
                        task_id: TaskId::new(self.task_id.clone()),
                        rendering_hints: None,
                        source: Some("mcp_tool".to_string()),
                        mcp_execution_id,
                        mcp_schema: None,
                        is_internal: None,
                        fingerprint: None,
                        tool_name: Some(tool_call.name.clone()),
                        execution_index: Some(index),
                        skill_id: None,
                        skill_name: None,
                    };

                    if let Some(sid) = skill_id {
                        metadata = metadata.with_skill_id(sid);
                    }

                    let artifact_id = extract_artifact_id(structured_content)
                        .map(ArtifactId::new)
                        .unwrap_or_else(ArtifactId::generate);

                    let artifact = Artifact {
                        id: artifact_id,
                        name: Some(tool_call.name.clone()),
                        description: None,
                        parts: vec![Part::Text(TextPart {
                            text: structured_content.to_string(),
                        })],
                        extensions: Vec::new(),
                        metadata,
                    };
                    artifacts.push(artifact);
                }
            }
        }

        Ok(artifacts)
    }
}
