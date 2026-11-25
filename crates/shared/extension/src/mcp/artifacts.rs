//! The artifact ingest contract: the narrow waist every tool result passes
//! through, as another capability hands one to it.
//!
//! The `mcp` member owns the ingest (normalise, scan and redact, store once
//! by content, link to the execution) and provides it **sealed** as
//! [`DynToolResultIngest`]: an ingest writes executions and artifacts on any
//! caller's behalf. The `gateway` member reads it in its provide fns, hands
//! it the `tool_result` blocks a request history replays, keyed by the
//! high-water marks ([`IngestMarkKey`], [`IngestMark`]) it reads and
//! advances through it, and registers its safety scanners
//! ([`ArtifactScanner`]) on it. A composition without the `mcp` member
//! provides no ingest, and the gateway then ingests nothing and registers no
//! scanner.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use systemprompt_identifiers::{
    AiToolCallId, ArtifactId, McpExecutionId, McpServerId, McpToolName, SkillId, SkillName, UserId,
};
use systemprompt_models::mcp::{Correlation, ExecutionSource};
use systemprompt_models::{CallToolResult, RequestContext};
use systemprompt_traits::BoxedSource;

pub const PHASE_TOOL_RESULT: &str = "tool_result";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactFinding {
    pub phase: &'static str,
    pub severity: String,
    pub category: String,
    pub scanner: String,
    pub path: Option<String>,
    pub excerpt: Option<String>,
    pub redacted: bool,
}

/// A content scanner run over the text surfaces of an artifact body.
///
/// `#[async_trait]` because scanners are registered as `Arc<dyn
/// ArtifactScanner>`.
#[async_trait]
pub trait ArtifactScanner: Send + Sync {
    fn name(&self) -> &'static str;

    async fn scan(
        &self,
        surfaces: &[(String, String)],
    ) -> Result<Vec<ArtifactFinding>, BoxedSource>;
}

/// Which replayed conversation a mark belongs to.
#[derive(Debug, Clone)]
pub struct IngestMarkKey {
    pub user: UserId,
    pub scope: String,
    pub conversation: String,
}

/// The boundary a later turn resumes after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestMark {
    pub last_message_index: usize,
    pub boundary_tool_use_id: AiToolCallId,
}

#[derive(Debug)]
pub struct IngestRequest {
    pub result: CallToolResult,
    pub tool_name: McpToolName,
    pub server_name: Option<McpServerId>,
    pub ai_tool_call_id: Option<AiToolCallId>,
    pub mcp_execution_id: Option<McpExecutionId>,
    pub ctx: RequestContext,
    pub skill: Option<(SkillId, SkillName)>,
    pub source: ExecutionSource,
    pub started_at: Option<DateTime<Utc>>,
    // JSON: the tool's own arguments, when the vantage point had them.
    pub input: Option<JsonValue>,
}

/// What the ingest stored and, when the scanner rewrote the body, what the
/// caller must put on the wire in place of its own copy.
///
/// `stored_body` is the artifact body as persisted (the header alone for a
/// result over the ingestion ceiling); `redacted_body` is `Some` only when
/// `secret_redactions > 0` and then carries the whole scanned body, so a
/// caller never has to re-read the row to learn whether its unredacted copy
/// is safe to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestOutcome {
    pub mcp_execution_id: McpExecutionId,
    pub artifact_id: ArtifactId,
    pub created: bool,
    pub is_structured: bool,
    pub correlation: Correlation,
    pub findings: usize,
    pub secret_redactions: usize,
    // JSON: the persisted artifact body.
    pub stored_body: JsonValue,
    // JSON: the scanned body when the scanner rewrote it.
    pub redacted_body: Option<JsonValue>,
}

#[derive(Debug, thiserror::Error)]
#[error("the artifact ingest failed: {0}")]
pub struct ToolResultIngestError(#[source] pub BoxedSource);

/// The artifact ingest as another capability reaches it.
///
/// `#[async_trait]` because the ingest is held as `Arc<dyn
/// ToolResultIngest>`.
#[async_trait]
pub trait ToolResultIngest: Send + Sync {
    fn register_scanner(&self, scanner: Arc<dyn ArtifactScanner>);

    async fn ingest(&self, request: IngestRequest) -> Result<IngestOutcome, ToolResultIngestError>;

    async fn find_mark(
        &self,
        key: &IngestMarkKey,
    ) -> Result<Option<IngestMark>, ToolResultIngestError>;

    async fn set_mark(
        &self,
        key: &IngestMarkKey,
        mark: &IngestMark,
    ) -> Result<(), ToolResultIngestError>;

    async fn list_settled_gateway_call_ids(
        &self,
        ids: &[String],
    ) -> Result<Vec<String>, ToolResultIngestError>;
}

pub type DynToolResultIngest = Arc<dyn ToolResultIngest>;
