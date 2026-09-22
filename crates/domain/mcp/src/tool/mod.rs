//! Typed MCP tool definition and execution wrapper.
//!
//! [`McpToolHandler`] is the contract a tool implements — typed input and
//! output with derived JSON schemas plus an async `handle` — and
//! [`McpToolExecutor`] runs a handler against a [`CallToolRequestParams`],
//! recording execution start/completion in the tool-usage repository and
//! building the [`CallToolResult`] (including any artifact) from the output.
//!
//! `INTENT_CLAIM_WINDOW_SECONDS` is how far back an execution may reach for
//! an unclaimed intent. It is shared with the gateway's external-server audit
//! so both correlate over one window.
//!
//! A window that matches no intent is logged, and at info: on the 2026-09-22
//! customer instance 303 in-process executions carried no `ai_tool_call_id`
//! while 138 had a claimable intent, and no offline hypothesis explains it —
//! the session the claim reads is written from this same `RequestContext`,
//! the `LIKE` suffix matches, and the pool is shared. The session id and tool
//! are what an instrumented run needs to tell a genuinely empty window from a
//! mismatched key, and a miss is cheap to log because it is meant to be rare.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod handler;

pub use handler::{McpToolHandler, object_input_schema};

use crate::models::{ExecutionStatus, ToolExecutionRequest, ToolExecutionResult};
use crate::repository::ToolUsageRepository;
use crate::response::{McpResponseBuilder, ToolIdentity};
use crate::schema::McpOutputSchema;
use crate::services::artifact_ingest::ArtifactIngest;
use chrono::Utc;
use rmcp::ErrorData as McpError;
use rmcp::model::{CacheScope, CallToolRequestParams, CallToolResult, ListToolsResult, Tool};
use serde::de::DeserializeOwned;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_models::RequestContext;
use systemprompt_models::mcp::{ClientProfile, Correlation, ExecutionSource};

const TOOL_LIST_TTL_MS: u64 = 3_600_000;
pub const INTENT_CLAIM_WINDOW_SECONDS: i64 = 120;

#[must_use]
pub fn build_tool_list_result(tools: Vec<Tool>) -> ListToolsResult {
    ListToolsResult::with_all_items(tools)
        .with_ttl_ms(TOOL_LIST_TTL_MS)
        .with_cache_scope(CacheScope::Public)
}

#[derive(Clone, Debug)]
pub struct McpToolExecutor {
    tool_usage_repo: Arc<ToolUsageRepository>,
    ingest: Arc<ArtifactIngest>,
    server_name: String,
}

impl McpToolExecutor {
    pub fn new(
        tool_usage_repo: Arc<ToolUsageRepository>,
        ingest: Arc<ArtifactIngest>,
        server_name: impl Into<String>,
    ) -> Self {
        Self {
            tool_usage_repo,
            ingest,
            server_name: server_name.into(),
        }
    }

    pub async fn execute<H: McpToolHandler>(
        &self,
        handler: &H,
        request: &CallToolRequestParams,
        ctx: &RequestContext,
        client: &ClientProfile,
    ) -> Result<CallToolResult, McpError> {
        let started_at = Utc::now();

        let input_value = serde_json::to_value(&request.arguments).map_err(|e| {
            tracing::error!(error = %e, "Failed to serialize tool arguments");
            McpError::internal_error(format!("Failed to serialize arguments: {e}"), None)
        })?;

        let exec_id = McpExecutionId::generate();
        let execution_request = ToolExecutionRequest {
            tool_name: handler.tool_name().to_owned(),
            server_name: self.server_name.clone(),
            input: input_value,
            started_at,
            context: ctx.clone(),
            request_method: Some("mcp".to_owned()),
            request_source: Some(self.server_name.clone()),
            ai_tool_call_id: ctx.ai_tool_call_id().cloned(),
            source: ExecutionSource::InProcess,
        };

        self.tool_usage_repo
            .start_execution(&exec_id, &execution_request, Correlation::Exact)
            .await
            .map_err(|e| {
                tracing::error!(
                    tool = handler.tool_name(),
                    error = %e,
                    "Failed to start execution tracking"
                );
                McpError::internal_error(format!("Failed to start execution tracking: {e}"), None)
            })?;
        let ctx = &self
            .with_claimed_intent(handler.tool_name(), ctx, &exec_id)
            .await;

        tracing::info!(tool = handler.tool_name(), %exec_id, "MCP execution started");

        let result = async {
            let input: H::Input = parse_input(request)?;
            handler.handle(input, ctx, &exec_id).await
        }
        .await;

        let (response, output_value) = match result {
            Ok((output, summary)) => {
                let title = output.artifact_title();
                let artifact_type = output.artifact_type_name();
                let output_value = match serde_json::to_value(&output) {
                    Ok(value) => Some(value),
                    Err(e) => {
                        tracing::warn!(
                            tool = handler.tool_name(),
                            execution_id = %exec_id,
                            error = %e,
                            "Tool output could not be serialised for the execution record"
                        );
                        None
                    },
                };
                let identity = ToolIdentity::new(&self.server_name, handler.tool_name());
                let response = McpResponseBuilder::new(output, identity, ctx, &exec_id, client)
                    .build(summary, &self.ingest, &artifact_type, title)
                    .await;
                (response, output_value)
            },
            Err(ref e) => (Err(e.clone()), None),
        };

        let execution_result = Self::build_execution_result(&response, output_value, started_at);
        self.record_completion(handler.tool_name(), &exec_id, &execution_result)
            .await;

        response
    }

    // Why: a client-supplied `tool_use_id` is an exact join and is claimed as
    // such; with none, the newest unclaimed intent for this tool in the
    // session is claimed atomically and recorded as inferred, never exact.
    async fn with_claimed_intent(
        &self,
        tool_name: &str,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
    ) -> RequestContext {
        if let Some(call_id) = ctx.ai_tool_call_id() {
            if let Err(e) = self.tool_usage_repo.claim_intent(call_id, exec_id).await {
                tracing::warn!(tool = tool_name, %exec_id, error = %e, "Intent not claimed");
            }
            return ctx.clone();
        }
        match self
            .tool_usage_repo
            .claim_unclaimed_intent(
                ctx.session_id(),
                tool_name,
                exec_id,
                INTENT_CLAIM_WINDOW_SECONDS,
            )
            .await
        {
            Ok(Some(call_id)) => {
                tracing::debug!(
                    tool = tool_name,
                    %exec_id,
                    session_id = %ctx.session_id(),
                    %call_id,
                    "Intent claimed"
                );
                ctx.clone().with_ai_tool_call_id(call_id)
            },
            Ok(None) => {
                tracing::info!(
                    tool = tool_name,
                    %exec_id,
                    session_id = %ctx.session_id(),
                    window_seconds = INTENT_CLAIM_WINDOW_SECONDS,
                    "No unclaimed intent matched this execution"
                );
                ctx.clone()
            },
            Err(e) => {
                tracing::warn!(tool = tool_name, %exec_id, error = %e, "Intent claim failed");
                ctx.clone()
            },
        }
    }

    fn build_execution_result(
        response: &Result<CallToolResult, McpError>,
        output_value: Option<JsonValue>,
        started_at: chrono::DateTime<Utc>,
    ) -> ToolExecutionResult {
        let completed_at = Utc::now();
        ToolExecutionResult {
            output: response.as_ref().ok().and(output_value),
            output_schema: None,
            status: if response.is_ok() {
                ExecutionStatus::Success.as_str().to_owned()
            } else {
                ExecutionStatus::Failed.as_str().to_owned()
            },
            error_message: response.as_ref().err().map(|e| e.message.to_string()),
            started_at,
            completed_at: Some(completed_at),
        }
    }

    async fn record_completion(
        &self,
        tool_name: &str,
        exec_id: &McpExecutionId,
        result: &ToolExecutionResult,
    ) {
        match self
            .tool_usage_repo
            .complete_execution(exec_id, result)
            .await
        {
            Ok(()) => {
                tracing::info!(tool = tool_name, %exec_id, "MCP execution completed");
            },
            Err(e) => {
                tracing::error!(
                    tool = tool_name,
                    %exec_id,
                    error = %e,
                    "Failed to complete execution tracking"
                );
            },
        }
    }
}

fn parse_input<T: DeserializeOwned>(request: &CallToolRequestParams) -> Result<T, McpError> {
    let args_value = request
        .arguments
        .as_ref()
        .map_or(JsonValue::Object(serde_json::Map::new()), |m| {
            JsonValue::Object(m.clone())
        });

    serde_json::from_value(args_value).map_err(|e| {
        tracing::warn!(
            error = %e,
            tool = %request.name,
            "Failed to parse tool input"
        );
        McpError::invalid_params(format!("Invalid tool input: {e}"), None)
    })
}
