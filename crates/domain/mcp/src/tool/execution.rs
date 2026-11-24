//! [`McpToolExecutor`] as the contract's `ToolExecution`, and the per-server
//! factory the MCP member provides.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use systemprompt_extension::mcp::{
    DynToolExecution, McpOutputSchema, ToolExecution, ToolExecutions, ToolOutput, ToolRun,
    VerifiedCaller,
};
use systemprompt_identifiers::{McpExecutionId, McpServerId, McpToolName};
use systemprompt_models::mcp::{ClientProfile, Correlation, ExecutionSource};
use systemprompt_traits::DynToolCallIntentClaims;

use super::McpToolExecutor;
use crate::models::ToolExecutionRequest;
use crate::repository::ToolUsageRepository;
use crate::response::{McpResponseBuilder, ResponseServices, ToolIdentity};
use crate::services::artifact_ingest::ArtifactIngest;
use crate::services::ui_renderer::UiRendererRegistry;

#[async_trait]
impl ToolExecution for McpToolExecutor {
    async fn begin(
        &self,
        tool_name: McpToolName,
        request: &CallToolRequestParams,
        caller: &VerifiedCaller,
    ) -> Result<ToolRun, McpError> {
        let context = caller.context();
        let started_at = Utc::now();
        let input_value = serde_json::to_value(&request.arguments).map_err(|e| {
            tracing::error!(error = %e, "Failed to serialize tool arguments");
            McpError::internal_error("Failed to serialize arguments", None)
        })?;
        let exec_id = McpExecutionId::generate();
        let execution_request = ToolExecutionRequest {
            tool_name: tool_name.clone(),
            server_name: self.server_name.clone(),
            input: input_value,
            started_at,
            context: context.clone(),
            request_method: Some("mcp".to_owned()),
            request_source: Some(self.server_name.to_string()),
            ai_tool_call_id: context.ai_tool_call_id().cloned(),
            source: ExecutionSource::InProcess,
        };
        self.tool_usage_repo
            .start_execution(&exec_id, &execution_request, Correlation::Exact)
            .await
            .map_err(|e| {
                tracing::error!(tool = %tool_name, error = %e, "Failed to start execution tracking");
                McpError::internal_error("Failed to start execution tracking", None)
            })?;
        let context = self
            .with_claimed_intent(&tool_name, context, &exec_id)
            .await;
        tracing::info!(tool = %tool_name, %exec_id, "MCP execution started");
        Ok(ToolRun {
            exec_id,
            tool_name,
            context,
            started_at,
        })
    }

    async fn finish(
        &self,
        run: ToolRun,
        outcome: Result<ToolOutput, McpError>,
        client: &ClientProfile,
    ) -> Result<CallToolResult, McpError> {
        let ToolRun {
            exec_id,
            tool_name,
            context,
            started_at,
        } = run;
        let (response, output_value) = match outcome {
            Ok(output) => {
                let output_value = Some(output.value.clone());
                let summary = output.summary.clone();
                let artifact_type = output.artifact_type.clone();
                let title = output.title.clone();
                let identity = ToolIdentity::new(self.server_name.clone(), tool_name.clone());
                let response = McpResponseBuilder::new(
                    ErasedOutput(output),
                    identity,
                    &context,
                    &exec_id,
                    client,
                )
                .build(
                    summary,
                    ResponseServices {
                        ingest: &self.ingest,
                        renderers: &self.renderers,
                    },
                    &artifact_type,
                    title,
                )
                .await;
                (response, output_value)
            },
            Err(e) => (Err(e), None),
        };
        let execution_result = Self::build_execution_result(&response, output_value, started_at);
        self.record_completion(&tool_name, &exec_id, &context, &execution_result)
            .await;
        response
    }
}

struct ErasedOutput(ToolOutput);

impl serde::Serialize for ErasedOutput {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.value.serialize(serializer)
    }
}

impl schemars::JsonSchema for ErasedOutput {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ToolOutput".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "object" })
    }
}

impl McpOutputSchema for ErasedOutput {
    fn artifact_type() -> &'static str {
        systemprompt_models::artifacts::ToolResultArtifact::ARTIFACT_TYPE_STR
    }

    fn artifact_type_name(&self) -> String {
        self.0.artifact_type.clone()
    }

    fn artifact_title(&self) -> Option<String> {
        self.0.title.clone()
    }

    fn text_body(&self) -> Option<String> {
        self.0.text_body.clone()
    }

    fn terse_wire(&self) -> bool {
        self.0.terse_wire
    }
}

pub struct McpToolExecutions {
    tool_usage_repo: Arc<ToolUsageRepository>,
    intents: Option<DynToolCallIntentClaims>,
    ingest: Arc<ArtifactIngest>,
    renderers: Arc<UiRendererRegistry>,
}

impl McpToolExecutions {
    pub fn new(
        tool_usage_repo: Arc<ToolUsageRepository>,
        intents: Option<DynToolCallIntentClaims>,
        ingest: Arc<ArtifactIngest>,
        renderers: Arc<UiRendererRegistry>,
    ) -> Self {
        Self {
            tool_usage_repo,
            intents,
            ingest,
            renderers,
        }
    }
}

impl std::fmt::Debug for McpToolExecutions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpToolExecutions").finish_non_exhaustive()
    }
}

impl ToolExecutions for McpToolExecutions {
    fn for_server(&self, server: McpServerId) -> DynToolExecution {
        Arc::new(McpToolExecutor::new(
            Arc::clone(&self.tool_usage_repo),
            self.intents.clone(),
            Arc::clone(&self.ingest),
            Arc::clone(&self.renderers),
            server,
        ))
    }
}
