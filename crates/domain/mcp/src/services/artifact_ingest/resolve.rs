//! Finding the execution a result belongs to.
//!
//! Resolution is exact wherever a key exists, in this order: the server's
//! `mcp_execution_id` carried in `_meta`, the execution id the vantage point
//! itself minted, then the client `tool_use_id`. With none of those, a
//! server-observed result is a new execution; a client-reported one is
//! matched by session, tool, digest and time as a last resort and recorded as
//! inferred, or becomes a new execution the client alone attested.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_models::mcp::Correlation;

use super::classify::Classified;
use super::{ArtifactIngest, FINGERPRINT_WINDOW_SECONDS, IngestRequest};
use crate::error::McpDomainResult;
use crate::models::{ExecutionStatus, ToolExecutionRequest, ToolExecutionResult};
use crate::repository::{ArtifactCorrelation, McpArtifactRecord};

#[derive(Debug, Clone)]
pub(super) struct ResolvedExecution {
    pub mcp_execution_id: McpExecutionId,
    pub correlation: Correlation,
}

pub(super) async fn resolve_execution(
    ingest: &ArtifactIngest,
    request: &IngestRequest,
    classified: &Classified,
    raw_sha256: &str,
) -> McpDomainResult<ResolvedExecution> {
    if let Some(id) = &classified.meta_execution_id
        && ingest.executions.find_by_id(id).await?.is_some()
    {
        return Ok(exact(id.clone()));
    }
    if let Some(id) = &request.mcp_execution_id {
        return Ok(exact(id.clone()));
    }
    if let Some(call_id) = &request.ai_tool_call_id
        && let Some(id) = ingest.executions.find_by_ai_call_id(call_id).await?
    {
        return Ok(exact(id));
    }

    if !request.source.is_server_observed()
        && let Some(id) = ingest
            .executions
            .find_by_fingerprint(
                request.ctx.session_id(),
                &request.tool_name,
                raw_sha256,
                FINGERPRINT_WINDOW_SECONDS,
            )
            .await?
    {
        tracing::info!(
            mcp_execution_id = %id,
            tool = %request.tool_name,
            source = %request.source,
            "Tool result joined to its execution by fingerprint"
        );
        return Ok(ResolvedExecution {
            mcp_execution_id: id,
            correlation: Correlation::Inferred,
        });
    }

    let correlation = if request.ai_tool_call_id.is_some() || request.source.is_server_observed() {
        Correlation::Exact
    } else {
        Correlation::Inferred
    };
    let id = McpExecutionId::new(uuid::Uuid::new_v4().to_string());
    let (execution, result) = new_execution(request, classified);
    ingest
        .executions
        .log_execution_sync_with_id(&id, &execution, &result, correlation)
        .await?;
    Ok(ResolvedExecution {
        mcp_execution_id: id,
        correlation,
    })
}

fn new_execution(
    request: &IngestRequest,
    classified: &Classified,
) -> (ToolExecutionRequest, ToolExecutionResult) {
    let started_at = request.started_at.unwrap_or_else(Utc::now);
    let error_message = classified
        .is_error
        .then(|| "tool result reported isError".to_owned());
    let execution = ToolExecutionRequest {
        tool_name: request.tool_name.clone(),
        server_name: request
            .server_name
            .clone()
            .unwrap_or_else(|| request.source.to_string()),
        input: request.input.clone().unwrap_or(serde_json::Value::Null),
        started_at,
        context: request.ctx.clone(),
        request_method: Some("mcp".to_owned()),
        request_source: Some(request.source.to_string()),
        ai_tool_call_id: request.ai_tool_call_id.clone(),
        source: request.source,
    };
    let result = ToolExecutionResult {
        output: None,
        output_schema: None,
        status: ExecutionStatus::from_error(error_message.is_some()).to_string(),
        error_message,
        started_at,
        completed_at: Utc::now(),
    };
    (execution, result)
}

const fn exact(id: McpExecutionId) -> ResolvedExecution {
    ResolvedExecution {
        mcp_execution_id: id,
        correlation: Correlation::Exact,
    }
}

pub(super) async fn enrich_existing(
    ingest: &ArtifactIngest,
    request: &IngestRequest,
    resolved: &ResolvedExecution,
    existing: &McpArtifactRecord,
) -> McpDomainResult<()> {
    let keys = ArtifactCorrelation {
        session_id: Some(request.ctx.session_id().clone()),
        trace_id: Some(request.ctx.trace_id().clone()),
        ai_tool_call_id: request.ai_tool_call_id.clone(),
        last_seen_source: Some(request.source),
    };
    ingest
        .artifacts
        .enrich_correlation(&existing.artifact_id, &keys)
        .await?;
    ingest
        .executions
        .mark_correlated(
            &resolved.mcp_execution_id,
            request.ai_tool_call_id.as_ref(),
            resolved.correlation,
            existing.payload_sha256.as_deref(),
        )
        .await?;
    tracing::debug!(
        artifact_id = %existing.artifact_id,
        source = %request.source,
        "Artifact seen again from another vantage point; correlation enriched"
    );
    Ok(())
}
