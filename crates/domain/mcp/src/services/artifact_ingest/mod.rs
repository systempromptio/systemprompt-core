//! The narrow waist every tool result passes through.
//!
//! An in-process server, the external-MCP proxy tap, a gateway `tool_result`
//! block and a client hook all hand their result to [`ArtifactIngest::ingest`],
//! which normalises it into the typed artifact model, resolves the execution
//! it belongs to (by exact key wherever one exists, and visibly as inferred
//! where none does), scans and redacts it before any row is written, stores
//! the body once by content, and links the artifact to its execution. The
//! same call seen from several vantage points ends as one execution and one
//! artifact, enriched with every key each vantage point knew.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod classify;
mod normalize;
mod resolve;
mod scan;

use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use rmcp::model::CallToolResult;
use serde_json::Value as JsonValue;
use systemprompt_identifiers::{AiToolCallId, ArtifactId, McpExecutionId, SkillId};
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::{ExecutionMetadata, ToolResponse, payload_digest};
use systemprompt_models::mcp::{Correlation, ExecutionSource};
use systemprompt_security::policy::secrets::SecretScanner;

use crate::error::{McpDomainError, McpDomainResult};
use crate::repository::{
    ArtifactFindingRepository, ArtifactPayloadRepository, ArtifactShape, CreateMcpArtifact,
    McpArtifactRepository, ToolUsageRepository,
};

pub use classify::Classified;
pub use normalize::{
    from_canonical_tool_result, from_hook_failure, from_hook_response, from_wire_value,
};
pub use scan::{ArtifactScanner, ScanOutcome};

/// Largest body the platform keeps. Beyond this only the digest survives.
pub const MAX_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;

/// How far back the last-resort fingerprint join looks.
pub const FINGERPRINT_WINDOW_SECONDS: i64 = 30;

#[derive(Debug)]
pub struct IngestRequest {
    pub result: CallToolResult,
    pub tool_name: String,
    pub server_name: Option<String>,
    pub ai_tool_call_id: Option<AiToolCallId>,
    pub mcp_execution_id: Option<McpExecutionId>,
    pub ctx: RequestContext,
    pub skill: Option<(SkillId, String)>,
    pub source: ExecutionSource,
    pub started_at: Option<DateTime<Utc>>,
    // JSON: the tool's own arguments, when the vantage point had them.
    pub input: Option<JsonValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestOutcome {
    pub mcp_execution_id: McpExecutionId,
    pub artifact_id: ArtifactId,
    pub created: bool,
    pub is_structured: bool,
    pub correlation: Correlation,
    pub findings: usize,
}

pub struct ArtifactIngest {
    artifacts: Arc<McpArtifactRepository>,
    payloads: Arc<ArtifactPayloadRepository>,
    findings: Arc<ArtifactFindingRepository>,
    executions: Arc<ToolUsageRepository>,
    secrets: Option<Arc<SecretScanner>>,
    scanners: RwLock<Vec<Arc<dyn ArtifactScanner>>>,
}

impl std::fmt::Debug for ArtifactIngest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArtifactIngest")
            .field("secret_scanner", &self.secrets.is_some())
            .field(
                "scanners",
                &self.scanners.read().map(|s| s.len()).unwrap_or_default(),
            )
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct ArtifactIngestRepositories {
    pub artifacts: Arc<McpArtifactRepository>,
    pub payloads: Arc<ArtifactPayloadRepository>,
    pub findings: Arc<ArtifactFindingRepository>,
    pub executions: Arc<ToolUsageRepository>,
}

impl ArtifactIngest {
    #[must_use]
    pub fn new(repos: ArtifactIngestRepositories, secrets: Option<Arc<SecretScanner>>) -> Self {
        Self {
            artifacts: repos.artifacts,
            payloads: repos.payloads,
            findings: repos.findings,
            executions: repos.executions,
            secrets,
            scanners: RwLock::new(Vec::new()),
        }
    }

    /// All four repositories over one pool, for hosts that have no app
    /// context: in-process MCP server binaries.
    pub fn from_db(
        db: &systemprompt_database::DbPool,
        secrets: Option<Arc<SecretScanner>>,
    ) -> McpDomainResult<Self> {
        Ok(Self::new(
            ArtifactIngestRepositories {
                artifacts: Arc::new(McpArtifactRepository::new(db)?),
                payloads: Arc::new(ArtifactPayloadRepository::new(db)?),
                findings: Arc::new(ArtifactFindingRepository::new(db)?),
                executions: Arc::new(ToolUsageRepository::new(db)?),
            },
            secrets,
        ))
    }

    /// Adds a content scanner. The composition root registers the gateway's
    /// safety scanners here once the ingest is shared.
    pub fn register_scanner(&self, scanner: Arc<dyn ArtifactScanner>) {
        match self.scanners.write() {
            Ok(mut scanners) => scanners.push(scanner),
            Err(poisoned) => poisoned.into_inner().push(scanner),
        }
    }

    pub(super) fn scanners(&self) -> Vec<Arc<dyn ArtifactScanner>> {
        match self.scanners.read() {
            Ok(scanners) => scanners.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    #[must_use]
    pub fn artifacts(&self) -> &McpArtifactRepository {
        &self.artifacts
    }

    #[must_use]
    pub fn payloads(&self) -> &ArtifactPayloadRepository {
        &self.payloads
    }

    #[must_use]
    pub fn findings_repository(&self) -> &ArtifactFindingRepository {
        &self.findings
    }

    pub async fn ingest(&self, request: IngestRequest) -> McpDomainResult<IngestOutcome> {
        let classified = classify::classify(&request);
        let raw_digest = payload_digest(&classified.body);

        let resolved =
            resolve::resolve_execution(self, &request, &classified, &raw_digest.sha256).await?;

        if let Some(existing) = self
            .artifacts
            .find_by_execution_id(&resolved.mcp_execution_id)
            .await?
        {
            resolve::enrich_existing(self, &request, &resolved, &existing).await?;
            return Ok(IngestOutcome {
                mcp_execution_id: resolved.mcp_execution_id,
                artifact_id: existing.artifact_id,
                created: false,
                is_structured: existing.is_structured,
                correlation: resolved.correlation,
                findings: 0,
            });
        }

        let scanned = if raw_digest.byte_len > MAX_PAYLOAD_BYTES {
            ScanOutcome::truncated(classified.header_only(&request), raw_digest)
        } else {
            scan::scan_body(self, &request, classified.body.clone(), &raw_digest).await?
        };

        let artifact_id = classified
            .meta_artifact_id
            .clone()
            .unwrap_or_else(ArtifactId::generate);
        let stored_digest = payload_digest(&scanned.body);
        let byte_len = i32::try_from(stored_digest.byte_len).unwrap_or(i32::MAX);
        self.payloads
            .upsert_payload(&stored_digest.sha256, byte_len, &scanned.body)
            .await?;

        let metadata = build_metadata(&request, &resolved.mcp_execution_id);
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
        create.ai_tool_call_id = request.ai_tool_call_id.clone();
        create.tool_name = Some(request.tool_name.clone());
        create.title = classified.title.clone();
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
        self.artifacts.save(&create).await?;
        if !scanned.findings.is_empty() {
            self.findings
                .insert_findings(&artifact_id, &scanned.findings)
                .await?;
        }
        self.executions
            .mark_correlated(
                &resolved.mcp_execution_id,
                request.ai_tool_call_id.as_ref(),
                resolved.correlation,
                Some(&stored_digest.sha256),
            )
            .await?;

        tracing::info!(
            artifact_id = %artifact_id,
            mcp_execution_id = %resolved.mcp_execution_id,
            source = %request.source,
            correlation = %resolved.correlation,
            is_structured = classified.is_structured,
            findings = scanned.findings.len(),
            "Artifact ingested"
        );

        Ok(IngestOutcome {
            mcp_execution_id: resolved.mcp_execution_id,
            artifact_id,
            created: true,
            is_structured: classified.is_structured,
            correlation: resolved.correlation,
            findings: scanned.findings.len(),
        })
    }
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
