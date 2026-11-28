//! Joining a client-reported result to the execution the server observed.
//!
//! A client-reported result carries no key the server shares. It is paired
//! with the server-observed execution of the same tool the same user ran
//! moments before, then matched by session, tool, digest and time. Either
//! join is recorded as inferred.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_models::mcp::Correlation;

use super::resolve::ResolvedExecution;
use super::{ArtifactIngest, FINGERPRINT_WINDOW_SECONDS, IngestRequest, PROXIMITY_WINDOW_SECONDS};
use crate::error::McpDomainResult;
use crate::repository::ProximityProbe;

// Why: a client-reported result carries no key the server shares, so it is
// joined to the execution the server observed — first by proximity, then by
// payload fingerprint. A server-observed row is already its own record and
// never pairs with another.
pub(super) async fn pair_client_attestation(
    ingest: &ArtifactIngest,
    request: &IngestRequest,
    raw_sha256: &str,
) -> McpDomainResult<Option<ResolvedExecution>> {
    if request.source.is_server_observed() {
        return Ok(None);
    }

    if let Some(id) = find_by_proximity(ingest, request).await? {
        ingest
            .executions
            .mark_correlated(
                &id,
                request.ai_tool_call_id.as_ref(),
                Correlation::Inferred,
                None,
            )
            .await?;
        tracing::info!(
            mcp_execution_id = %id,
            tool = %request.tool_name,
            source = %request.source,
            "Client attestation paired with the execution the server observed"
        );
        return Ok(Some(ResolvedExecution {
            mcp_execution_id: id,
            correlation: Correlation::Inferred,
        }));
    }

    if let Some(id) = ingest
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
        return Ok(Some(ResolvedExecution {
            mcp_execution_id: id,
            correlation: Correlation::Inferred,
        }));
    }

    Ok(None)
}

async fn find_by_proximity(
    ingest: &ArtifactIngest,
    request: &IngestRequest,
) -> McpDomainResult<Option<McpExecutionId>> {
    if request.ctx.is_anonymous() {
        return Ok(None);
    }
    let Some(server_name) = request.server_name.as_ref() else {
        return Ok(None);
    };
    ingest
        .executions
        .find_unattested_by_proximity(&ProximityProbe {
            user_id: request.ctx.user_id(),
            server_name,
            tool_name: &request.tool_name,
            at: request.started_at.unwrap_or_else(Utc::now),
            window_seconds: PROXIMITY_WINDOW_SECONDS,
        })
        .await
}
