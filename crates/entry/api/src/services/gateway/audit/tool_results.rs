//! Replayed `tool_result` blocks become linked artifacts.
//!
//! Every turn a client sends carries the results of the tools it ran since
//! the previous turn, inside the request history. Those blocks are the one
//! place the gateway sees what the model was actually given, so each is
//! handed to the artifact ingest keyed by its `tool_use_id`. History is
//! replayed on every later turn too; the ingest resolves the existing
//! execution by that key and enriches it, so a result is stored once however
//! many turns repeat it. The work runs off the request path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::{Actor, AgentName, AiToolCallId};
use systemprompt_mcp::{ArtifactIngest, CallToolResult, IngestRequest, from_canonical_tool_result};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;
use systemprompt_models::mcp::ExecutionSource;

use super::{GatewayAudit, GatewayRequestContext};
use crate::services::gateway::protocol::canonical::{CanonicalContent, CanonicalRequest};

impl GatewayAudit {
    pub(super) fn ingest_tool_results(&self, request: &CanonicalRequest) {
        let Some(ingest) = self.artifact_ingest.clone() else {
            return;
        };
        let results: Vec<ReplayedResult> = request
            .messages
            .iter()
            .flat_map(|message| message.content.iter())
            .filter_map(ReplayedResult::from_content)
            .collect();
        if results.is_empty() {
            return;
        }
        let Some(ctx) = request_context(&self.ctx) else {
            return;
        };
        let requests = Arc::clone(&self.requests);
        tokio::spawn(async move {
            for replayed in results {
                ingest_one(&ingest, &requests, &ctx, replayed).await;
            }
        });
    }
}

struct ReplayedResult {
    tool_use_id: AiToolCallId,
    result: CallToolResult,
}

impl ReplayedResult {
    fn from_content(content: &CanonicalContent) -> Option<Self> {
        let CanonicalContent::ToolResult {
            tool_use_id,
            content,
            is_error,
            structured_content,
            meta,
            ..
        } = content
        else {
            return None;
        };
        if tool_use_id.is_empty() {
            return None;
        }
        Some(Self {
            tool_use_id: AiToolCallId::new(tool_use_id.clone()),
            result: from_canonical_tool_result(
                content,
                structured_content.as_ref(),
                meta.as_ref(),
                *is_error,
            ),
        })
    }
}

fn request_context(ctx: &GatewayRequestContext) -> Option<RequestContext> {
    let session_id = ctx.session_id.clone()?;
    let trace_id = ctx.trace_id.clone()?;
    Some(
        RequestContext::new(
            session_id,
            trace_id,
            ctx.context_id.clone(),
            AgentName::unset(),
        )
        .with_actor(Actor::user(ctx.user_id.clone()))
        .with_user_type(UserType::User),
    )
}

async fn ingest_one(
    ingest: &ArtifactIngest,
    requests: &systemprompt_ai::repository::AiRequestRepository,
    ctx: &RequestContext,
    replayed: ReplayedResult,
) {
    // Why: the history block carries no tool name; the intent row the same
    // id created on the previous turn does.
    let intent = match requests
        .find_tool_call_by_ai_id(&replayed.tool_use_id)
        .await
    {
        Ok(intent) => intent,
        Err(e) => {
            tracing::warn!(error = %e, tool_use_id = %replayed.tool_use_id, "tool intent lookup failed");
            None
        },
    };
    let (tool_name, input) = intent.map_or_else(
        || ("unknown".to_owned(), None),
        |row| (row.tool_name, serde_json::from_str(&row.tool_input).ok()),
    );
    let request = IngestRequest {
        result: replayed.result,
        tool_name,
        server_name: None,
        ai_tool_call_id: Some(replayed.tool_use_id.clone()),
        mcp_execution_id: None,
        ctx: ctx.clone(),
        skill: None,
        source: ExecutionSource::Gateway,
        started_at: None,
        input,
    };
    if let Err(e) = ingest.ingest(request).await {
        tracing::warn!(
            error = %e,
            tool_use_id = %replayed.tool_use_id,
            "gateway tool_result could not be ingested as an artifact"
        );
    }
}
