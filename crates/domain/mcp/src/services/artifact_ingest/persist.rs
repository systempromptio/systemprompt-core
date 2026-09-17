//! Builds the `mcp_artifacts` row for a freshly ingested tool result.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value as JsonValue;
use systemprompt_identifiers::{ArtifactId, McpExecutionId};
use systemprompt_models::artifacts::{ExecutionMetadata, PayloadDigest, ToolResponse};

use super::IngestRequest;
use super::classify::Classified;
use super::resolve::ResolvedExecution;
use super::scan::ScanOutcome;
use crate::error::{McpDomainError, McpDomainResult};
use crate::repository::{ArtifactShape, CreateMcpArtifact};

#[derive(Clone, Copy)]
pub(super) struct NewArtifact<'a> {
    pub request: &'a IngestRequest,
    pub classified: &'a Classified,
    pub resolved: &'a ResolvedExecution,
    pub scanned: &'a ScanOutcome,
    pub artifact_id: &'a ArtifactId,
    pub stored_digest: &'a PayloadDigest,
    pub byte_len: i32,
}

pub(super) fn create_record(new: &NewArtifact<'_>) -> McpDomainResult<CreateMcpArtifact> {
    let NewArtifact {
        request,
        classified,
        resolved,
        scanned,
        artifact_id,
        stored_digest,
        byte_len,
    } = *new;
    let metadata = build_metadata(request, &resolved.mcp_execution_id);
    let envelope = ToolResponse::new(
        artifact_id.clone(),
        resolved.mcp_execution_id.clone(),
        scanned.body.clone(),
        metadata.clone(),
    )
    .to_json()
    .map_err(|e| McpDomainError::Internal(format!("artifact envelope: {e}")))?;

    let mut create = CreateMcpArtifact::new(
        artifact_id.clone(),
        resolved.mcp_execution_id.clone(),
        request
            .server_name
            .clone()
            .unwrap_or_else(|| request.source.to_string()),
        classified.artifact_type.clone(),
        envelope,
    );
    create.context_id = Some(request.ctx.context_id().clone());
    create.user_id = (!request.ctx.is_anonymous()).then(|| request.ctx.user_id().clone());
    create.session_id = Some(request.ctx.session_id().clone());
    create.trace_id = Some(request.ctx.trace_id().clone());
    create.ai_tool_call_id.clone_from(&request.ai_tool_call_id);
    create.tool_name = Some(request.tool_name.clone());
    create.title.clone_from(&classified.title);
    create.source = request.source;
    create.metadata = metadata.to_object().map(JsonValue::Object);
    create.payload_sha256 = Some(stored_digest.sha256.clone());
    create.payload_bytes = Some(byte_len);
    create.shape = ArtifactShape {
        is_structured: classified.is_structured,
        has_ui_resource: classified.has_ui_resource,
        is_error: classified.is_error,
        secret_redactions: i32::try_from(scanned.secret_redactions).unwrap_or(i32::MAX),
    };
    Ok(create)
}

fn build_metadata(request: &IngestRequest, exec_id: &McpExecutionId) -> ExecutionMetadata {
    let mut builder = ExecutionMetadata::builder(&request.ctx)
        .with_tool(request.tool_name.clone())
        .with_execution(exec_id.to_string());
    if let Some((id, name)) = &request.skill {
        builder = builder.with_skill(id.clone(), name.clone());
    }
    builder.build()
}
