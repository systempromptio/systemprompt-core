//! Finding the execution a result belongs to.
//!
//! Resolution is exact wherever a key exists, in this order: the server's
//! `mcp_execution_id` carried in `_meta`, the execution id the vantage point
//! itself minted, then the client `tool_use_id`. With none of those, a
//! server-observed result is a new execution; a client-reported one is
//! paired with the server-observed execution of the same tool the same user
//! ran moments before, then matched by session, tool, digest and time, and
//! recorded as inferred either way — or becomes a new execution the client
//! alone attested, which carries no duration because the client never
//! measured one. A key a client supplied only resolves to an execution that
//! client's user owns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_identifiers::{McpExecutionId, McpServerId};
use systemprompt_models::mcp::Correlation;

use super::classify::Classified;
use super::pair::pair_client_attestation;
use super::{ArtifactIngest, IngestRequest};
use crate::error::McpDomainResult;
use crate::models::{
    McpToolExecutionStatus, ToolExecution, ToolExecutionRequest, ToolExecutionResult,
};
use crate::repository::{ArtifactCorrelation, ExecutionInsert, McpArtifactRecord};
use systemprompt_traits::RepositoryError;

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
        && let Some(execution) = ingest.executions.find_by_id(id).await?
        && attributable(request, &execution)
    {
        return Ok(exact(id.clone()));
    }
    if let Some(id) = &request.mcp_execution_id {
        if ingest.executions.find_by_id(id).await?.is_none() {
            let (execution, result) = new_execution(request, classified);
            let inserted = ingest
                .executions
                .insert_execution_with_id(id, &execution, &result, Correlation::Exact)
                .await?;
            if inserted == ExecutionInsert::CallIdHeld {
                return Ok(exact(held_call_id_execution(ingest, request).await?));
            }
        }
        return Ok(exact(id.clone()));
    }
    if let Some(call_id) = &request.ai_tool_call_id
        && let Some(id) = ingest.executions.find_by_ai_call_id(call_id).await?
    {
        return Ok(exact(id));
    }

    if let Some(resolved) = pair_client_attestation(ingest, request, raw_sha256).await? {
        return Ok(resolved);
    }

    let correlation = if request.ai_tool_call_id.is_some() || request.source.is_server_observed() {
        Correlation::Exact
    } else {
        Correlation::Inferred
    };
    let id = McpExecutionId::generate();
    let (execution, result) = new_execution(request, classified);
    match ingest
        .executions
        .insert_execution_with_id(&id, &execution, &result, correlation)
        .await?
    {
        ExecutionInsert::Inserted => Ok(ResolvedExecution {
            mcp_execution_id: id,
            correlation,
        }),
        ExecutionInsert::CallIdHeld => Ok(exact(held_call_id_execution(ingest, request).await?)),
        ExecutionInsert::ExecutionIdHeld => Err(RepositoryError::conflict(
            "mcp_tool_execution",
            &id,
            "a freshly generated execution id is already taken",
        )
        .into()),
    }
}

// Why: another vantage point inserted the same call between this one's
// lookup and its insert. The call id names that execution exactly, so this
// sighting joins it and ends in enrichment rather than a dropped write.
async fn held_call_id_execution(
    ingest: &ArtifactIngest,
    request: &IngestRequest,
) -> McpDomainResult<McpExecutionId> {
    let Some(call_id) = request.ai_tool_call_id.as_ref() else {
        return Err(RepositoryError::conflict(
            "mcp_tool_execution",
            "<no call id>",
            "a call id conflict was reported for a request without one",
        )
        .into());
    };
    let held = ingest.executions.find_by_ai_call_id(call_id).await?;
    let Some(id) = held else {
        return Err(RepositoryError::conflict(
            "mcp_tool_execution",
            call_id,
            "the execution holding this call id is gone",
        )
        .into());
    };
    tracing::debug!(
        mcp_execution_id = %id,
        source = %request.source,
        "Concurrent sighting joined the execution another vantage point inserted"
    );
    Ok(id)
}

// Why: a client-reported result carries keys the client chose. Joining it to
// an execution the platform observed is only safe when the caller owns that
// execution; an anonymous caller owns none.
fn attributable(request: &IngestRequest, execution: &ToolExecution) -> bool {
    if request.source.is_server_observed() {
        return true;
    }
    let owned = !request.ctx.is_anonymous() && execution.user_id == *request.ctx.user_id();
    if !owned {
        tracing::debug!(
            mcp_execution_id = %execution.mcp_execution_id,
            execution_user_id = %execution.user_id,
            caller_user_id = %request.ctx.user_id(),
            source = %request.source,
            "Client-supplied execution id belongs to another user; ignored"
        );
    }
    owned
}

// Why: `mcp_tool_executions.ai_tool_call_id` is unique, so a client-supplied
// `tool_use_id` that already names another user's execution can neither
// join it nor be recorded on a new one — the result is ingested as if the
// client had sent no key.
pub(super) async fn disown_foreign_call_id(
    ingest: &ArtifactIngest,
    mut request: IngestRequest,
) -> McpDomainResult<IngestRequest> {
    if request.source.is_server_observed() {
        return Ok(request);
    }
    let Some(call_id) = request.ai_tool_call_id.clone() else {
        return Ok(request);
    };
    let Some(execution_id) = ingest.executions.find_by_ai_call_id(&call_id).await? else {
        return Ok(request);
    };
    let Some(execution) = ingest.executions.find_by_id(&execution_id).await? else {
        return Ok(request);
    };
    if !attributable(&request, &execution) {
        request.ai_tool_call_id = None;
    }
    Ok(request)
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
            .unwrap_or_else(|| McpServerId::new(request.source.to_string())),
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
        status: McpToolExecutionStatus::from_error(error_message.is_some()),
        error_message,
        started_at,
        completed_at: request.source.is_server_observed().then(Utc::now),
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
