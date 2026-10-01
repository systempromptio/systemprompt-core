//! Per-tool audit for external MCP servers served over the HTTP gateway.
//!
//! A client-mediated `tools/call` to an external provider has no backend
//! process to record it, so the gateway taps the forwarded request/response,
//! writes one `mcp_tool_executions` row under the calling user, and hands the
//! result to the artifact ingest. The execution id is minted before the
//! response leaves, and stamped into its `_meta`, so the client's own later
//! report of the same result carries the exact server key. `record` composes
//! the tap over the upstream body; the tap owns an [`McpAudit`] and finalizes
//! it (once) on stream EOF or drop.
//!
//! The execution row is written first, then paired with the model's intent
//! through [`IntentClaimService`], mirroring
//! [`systemprompt_mcp::McpToolExecutor`]: a client-supplied tool-call id is
//! claimed as an exact pairing, otherwise the newest unclaimed intent for the
//! tool in the calling session is claimed as an inferred one. Failing to claim
//! leaves the execution unpaired rather than failing the call, which has
//! already returned to the client.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod jsonrpc;
pub mod tap;

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde_json::Value;
use systemprompt_identifiers::{AiToolCallId, McpExecutionId, McpServerId};
use systemprompt_mcp::models::{ExecutionStatus, ToolExecutionRequest, ToolExecutionResult};
use systemprompt_mcp::{
    ArtifactIngest, INTENT_CLAIM_WINDOW_SECONDS, IngestRequest, IntentClaimService, from_wire_value,
};
use systemprompt_models::RequestContext;
use systemprompt_models::mcp::{Correlation, ExecutionSource};

pub(crate) use jsonrpc::parse_tool_call;
pub(crate) use tap::record;

use jsonrpc::{ToolCallInvocation, ToolCallOutcome};

#[derive(Debug)]
pub struct McpAudit {
    intent_claims: IntentClaimService,
    ingest: Option<Arc<ArtifactIngest>>,
    context: RequestContext,
    server_name: McpServerId,
    invocation: ToolCallInvocation,
    started_at: DateTime<Utc>,
    mcp_execution_id: McpExecutionId,
}

impl McpAudit {
    pub fn new(
        intent_claims: IntentClaimService,
        ingest: Option<Arc<ArtifactIngest>>,
        context: RequestContext,
        server_name: McpServerId,
        invocation: ToolCallInvocation,
    ) -> Self {
        Self {
            intent_claims,
            ingest,
            context,
            server_name,
            invocation,
            started_at: Utc::now(),
            mcp_execution_id: McpExecutionId::generate(),
        }
    }

    const fn request_id(&self) -> &Value {
        &self.invocation.id
    }

    pub const fn mcp_execution_id(&self) -> &McpExecutionId {
        &self.mcp_execution_id
    }

    fn finalize(self, outcome: Option<ToolCallOutcome>) {
        let (output, error_message, result) = match outcome {
            Some(o) => (o.output, o.error_message, o.result),
            None => (
                None,
                Some("external MCP tool call produced no parseable result".to_owned()),
                None,
            ),
        };

        let request = ToolExecutionRequest {
            tool_name: self.invocation.tool_name,
            server_name: self.server_name.clone(),
            input: self.invocation.arguments,
            started_at: self.started_at,
            context: self.context,
            request_method: Some("mcp".to_owned()),
            request_source: Some(String::from(self.server_name)),
            ai_tool_call_id: None,
            source: ExecutionSource::Proxy,
        };
        let result_row = ToolExecutionResult {
            status: ExecutionStatus::from_error(error_message.is_some()).to_string(),
            error_message,
            output,
            output_schema: None,
            started_at: self.started_at,
            completed_at: Some(Utc::now()),
        };

        let intent_claims = self.intent_claims;
        let ingest = self.ingest;
        let mcp_execution_id = self.mcp_execution_id;
        tokio::spawn(async move {
            let mut request = request;
            request.ai_tool_call_id = request.context.ai_tool_call_id().cloned();
            let exact = request.ai_tool_call_id.clone();
            let correlation = if exact.is_some() {
                Correlation::Exact
            } else {
                Correlation::Inferred
            };
            if let Err(e) = intent_claims
                .executions()
                .log_execution_sync_with_id(&mcp_execution_id, &request, &result_row, correlation)
                .await
            {
                tracing::warn!(
                    tool = %request.tool_name,
                    server = %request.server_name,
                    error = %e,
                    "Failed to record external MCP tool execution"
                );
                return;
            }
            // Why: no client sends `x-ai-tool-call-id`, so an external-server
            // execution arrives with nothing to join it to the inference turn
            // that asked for it. The in-process executor claims the newest
            // unclaimed intent for the tool in this session; without the same
            // claim here, every proxied call stayed unpaired — and eight of
            // nine configured servers are external.
            match exact {
                Some(call_id) => {
                    claim_exact(&intent_claims, &request, &call_id, &mcp_execution_id).await;
                },
                None => {
                    request.ai_tool_call_id =
                        claim_inferred(&intent_claims, &request, &mcp_execution_id).await;
                },
            }
            if let Some(ingest) = ingest {
                ingest_proxied_result(&ingest, &request, result, mcp_execution_id).await;
            }
        });
    }
}

async fn claim_exact(
    intent_claims: &IntentClaimService,
    request: &ToolExecutionRequest,
    call_id: &AiToolCallId,
    mcp_execution_id: &McpExecutionId,
) {
    if let Err(e) = intent_claims.claim_exact(call_id, mcp_execution_id).await {
        tracing::warn!(
            tool = %request.tool_name,
            server = %request.server_name,
            %mcp_execution_id,
            %call_id,
            error = %e,
            "Proxy exact intent claim failed"
        );
    }
}

async fn claim_inferred(
    intent_claims: &IntentClaimService,
    request: &ToolExecutionRequest,
    mcp_execution_id: &McpExecutionId,
) -> Option<AiToolCallId> {
    match intent_claims
        .claim_inferred(
            request.context.session_id(),
            &request.tool_name,
            mcp_execution_id,
            INTENT_CLAIM_WINDOW_SECONDS,
        )
        .await
    {
        Ok(claimed) => claimed,
        Err(e) => {
            tracing::warn!(
                tool = %request.tool_name,
                server = %request.server_name,
                %mcp_execution_id,
                error = %e,
                "Proxy intent claim failed"
            );
            None
        },
    }
}

// JSON: MCP `tools/call` result — open-shaped per the MCP spec, ingested as
// artifacts.
async fn ingest_proxied_result(
    ingest: &ArtifactIngest,
    request: &ToolExecutionRequest,
    result: Option<Value>,
    mcp_execution_id: McpExecutionId,
) {
    let Some(wire) = result.as_ref().and_then(from_wire_value) else {
        return;
    };
    let ingest_request = IngestRequest {
        result: wire,
        tool_name: request.tool_name.clone(),
        server_name: Some(request.server_name.clone()),
        ai_tool_call_id: request.ai_tool_call_id.clone(),
        mcp_execution_id: Some(mcp_execution_id),
        ctx: request.context.clone(),
        skill: None,
        source: ExecutionSource::Proxy,
        started_at: Some(request.started_at),
        input: Some(request.input.clone()),
    };
    if let Err(e) = ingest.ingest(ingest_request).await {
        tracing::warn!(
            tool = %request.tool_name,
            server = %request.server_name,
            error = %e,
            "Failed to ingest external MCP tool result as an artifact"
        );
    }
}
