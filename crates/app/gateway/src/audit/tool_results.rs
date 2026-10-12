//! Replayed `tool_result` blocks become linked artifacts.
//!
//! Every turn a client sends carries the results of the tools it ran since
//! the previous turn, inside the request history. Those blocks are the one
//! place the gateway sees what the model was actually given, so each is
//! handed to the artifact ingest keyed by its `tool_use_id`. History is
//! replayed on every later turn too, so the cost of a turn is kept to what is
//! new in it: a per-conversation high-water mark skips the replayed prefix
//! outright, and one batched lookup skips any block whose ingest has already
//! settled. A block that is ingested again resolves the existing execution by
//! its key and enriches it, so a result is stored once however many turns
//! repeat it. The work runs off the request path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::{Actor, AgentName, AiToolCallId, McpToolName};
use systemprompt_mcp::{
    ArtifactIngest, CallToolResult, IngestMark, IngestMarkKey, IngestRequest,
    from_canonical_tool_result,
};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;
use systemprompt_models::mcp::ExecutionSource;

use super::{GatewayAudit, GatewayRequestContext};
use crate::protocol::canonical::{CanonicalContent, CanonicalRequest};

impl GatewayAudit {
    pub(super) fn ingest_tool_results(&self, request: &CanonicalRequest) {
        let Some(ingest) = self.artifact_ingest.clone() else {
            return;
        };
        let results: Vec<ReplayedResult> = request
            .messages
            .iter()
            .enumerate()
            .flat_map(|(index, message)| message.content.iter().map(move |c| (index, c)))
            .filter_map(|(index, content)| ReplayedResult::from_content(index, content))
            .collect();
        if results.is_empty() {
            return;
        }
        let Some(ctx) = request_context(&self.ctx) else {
            return;
        };
        let key = mark_key(&self.ctx);
        let requests = Arc::clone(&self.requests);
        self.background
            .spawn("gateway_tool_result_ingest", async move {
                ingest_new(&ingest, &requests, &ctx, &key, results).await;
            });
    }
}

async fn ingest_new(
    ingest: &ArtifactIngest,
    requests: &systemprompt_ai::repository::AiRequestRepository,
    ctx: &RequestContext,
    key: &IngestMarkKey,
    results: Vec<ReplayedResult>,
) {
    let marks = ingest.ingest_marks();
    let mark = match marks.find_mark(key).await {
        Ok(mark) => mark,
        Err(e) => {
            tracing::warn!(error = %e, "tool_result ingest mark lookup failed");
            None
        },
    };
    let start = resume_index(mark.as_ref(), &results);
    let fresh: Vec<ReplayedResult> = results
        .into_iter()
        .filter(|r| r.message_index >= start)
        .collect();
    let Some(boundary) = fresh.last().map(|r| IngestMark {
        last_message_index: r.message_index,
        boundary_tool_use_id: r.tool_use_id.clone(),
    }) else {
        return;
    };
    let ids: Vec<String> = fresh.iter().map(|r| r.tool_use_id.to_string()).collect();
    let settled = marks
        .list_settled_gateway_call_ids(&ids)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "settled tool_result lookup failed");
            Vec::new()
        });
    let mut all_ingested = true;
    for replayed in fresh {
        if settled.iter().any(|id| id == replayed.tool_use_id.as_str()) {
            continue;
        }
        all_ingested &= ingest_one(ingest, requests, ctx, replayed).await;
    }
    if !all_ingested || mark.as_ref() == Some(&boundary) {
        return;
    }
    if let Err(e) = marks.set_mark(key, &boundary).await {
        tracing::warn!(error = %e, "tool_result ingest mark could not be advanced");
    }
}

/// The first message index this turn has to look at: just past the mark when
/// the block that pinned it is still where it was, else the whole history.
fn resume_index(mark: Option<&IngestMark>, results: &[ReplayedResult]) -> usize {
    mark.filter(|mark| {
        results.iter().any(|r| {
            r.message_index == mark.last_message_index
                && r.tool_use_id == mark.boundary_tool_use_id
        })
    })
    .map_or(0, |mark| mark.last_message_index + 1)
}

fn mark_key(ctx: &GatewayRequestContext) -> IngestMarkKey {
    IngestMarkKey {
        user_id: ctx.user_id.clone(),
        scope_id: ctx
            .client_session_id
            .as_ref()
            .map_or_else(|| ctx.context_id.to_string(), ToString::to_string),
        gateway_conversation_id: ctx
            .gateway_conversation_id
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default(),
    }
}

struct ReplayedResult {
    message_index: usize,
    tool_use_id: AiToolCallId,
    result: CallToolResult,
}

impl ReplayedResult {
    fn from_content(message_index: usize, content: &CanonicalContent) -> Option<Self> {
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
            message_index,
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
            AgentName::system(),
            Actor::user(ctx.user_id.clone()),
        )
        .with_user_type(UserType::User),
    )
}

async fn ingest_one(
    ingest: &ArtifactIngest,
    requests: &systemprompt_ai::repository::AiRequestRepository,
    ctx: &RequestContext,
    replayed: ReplayedResult,
) -> bool {
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
        || (McpToolName::new("unknown"), None),
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
        return false;
    }
    true
}
