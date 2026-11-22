//! The MCP member's implementation of the tool-execution seam.
//!
//! [`McpToolExecutor`] implements `systemprompt_extension::mcp::ToolExecution`
//! for one server: `begin` records the execution start in the tool-usage
//! repository and claims the AI tool-call intent, `finish` persists the output
//! as an artifact, builds the [`CallToolResult`] for the negotiated client and
//! records the completion. [`McpToolExecutions`] is the per-server factory the
//! member provides.
//!
//! `INTENT_CLAIM_WINDOW_SECONDS` is how far back an execution may reach for
//! an unclaimed intent. It is shared with the gateway's external-server audit
//! so both correlate over one window.
//!
//! A window that matches no intent is logged, and at info. The session id and
//! tool
//! are what an instrumented run needs to tell a genuinely empty window from a
//! mismatched key, and a miss is cheap to log because it is meant to be rare.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod execution;

pub use execution::McpToolExecutions;

use crate::models::{McpToolExecutionStatus, ToolExecutionResult};
use crate::repository::ToolUsageRepository;

use crate::services::artifact_ingest::ArtifactIngest;
use crate::services::intent_claim::IntentClaimService;
use crate::services::ui_renderer::UiRendererRegistry;
use chrono::Utc;
use rmcp::ErrorData as McpError;
use rmcp::model::{CacheScope, CallToolResult, ListToolsResult, Tool};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use systemprompt_identifiers::{McpExecutionId, McpServerId, McpToolName};
use systemprompt_models::RequestContext;
use systemprompt_traits::DynToolCallIntentClaims;

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
    intent_claims: IntentClaimService,
    ingest: Arc<ArtifactIngest>,
    renderers: Arc<UiRendererRegistry>,
    server_name: McpServerId,
}

impl McpToolExecutor {
    pub fn new(
        tool_usage_repo: Arc<ToolUsageRepository>,
        intents: DynToolCallIntentClaims,
        ingest: Arc<ArtifactIngest>,
        renderers: Arc<UiRendererRegistry>,
        server_name: McpServerId,
    ) -> Self {
        Self {
            intent_claims: IntentClaimService::new(intents, Arc::clone(&tool_usage_repo)),
            tool_usage_repo,
            ingest,
            renderers,
            server_name,
        }
    }

    // Why: a client-supplied `tool_use_id` is an exact join and is claimed as
    // such; with none, the newest unclaimed intent for this tool in the
    // session is claimed atomically and recorded as inferred, never exact.
    async fn with_claimed_intent(
        &self,
        tool_name: &McpToolName,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
    ) -> RequestContext {
        if let Some(call_id) = ctx.ai_tool_call_id() {
            if let Err(e) = self.intent_claims.claim_exact(call_id, exec_id).await {
                tracing::warn!(tool = %tool_name, %exec_id, error = %e, "Intent not claimed");
            }
            return ctx.clone();
        }
        match self
            .intent_claims
            .claim_inferred(
                ctx.session_id(),
                tool_name,
                exec_id,
                INTENT_CLAIM_WINDOW_SECONDS,
            )
            .await
        {
            Ok(Some(call_id)) => {
                tracing::debug!(
                    tool = %tool_name,
                    %exec_id,
                    session_id = %ctx.session_id(),
                    %call_id,
                    "Intent claimed"
                );
                ctx.clone().with_ai_tool_call_id(call_id)
            },
            Ok(None) => {
                tracing::info!(
                    tool = %tool_name,
                    %exec_id,
                    session_id = %ctx.session_id(),
                    window_seconds = INTENT_CLAIM_WINDOW_SECONDS,
                    "No unclaimed intent matched this execution"
                );
                ctx.clone()
            },
            Err(e) => {
                tracing::warn!(tool = %tool_name, %exec_id, error = %e, "Intent claim failed");
                ctx.clone()
            },
        }
    }

    // JSON: MCP tool result — structured content is schema-less per the spec.
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
                McpToolExecutionStatus::Success.as_str().to_owned()
            } else {
                McpToolExecutionStatus::Failed.as_str().to_owned()
            },
            error_message: response.as_ref().err().map(|e| e.message.to_string()),
            started_at,
            completed_at: Some(completed_at),
        }
    }

    async fn record_completion(
        &self,
        tool_name: &McpToolName,
        exec_id: &McpExecutionId,
        result: &ToolExecutionResult,
    ) {
        match self
            .tool_usage_repo
            .complete_execution(exec_id, result)
            .await
        {
            Ok(()) => {
                tracing::info!(tool = %tool_name, %exec_id, "MCP execution completed");
            },
            Err(e) => {
                tracing::error!(
                    tool = %tool_name,
                    %exec_id,
                    error = %e,
                    "Failed to complete execution tracking"
                );
            },
        }
    }
}
