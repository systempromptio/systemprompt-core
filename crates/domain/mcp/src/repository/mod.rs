//! Postgres persistence for MCP: tool-execution records and aggregate stats,
//! session state, proxy session bindings, tool-output artifacts, and the
//! tool-usage reports.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod artifact;
mod artifact_finding;
mod artifact_ingest;
mod artifact_payload;
mod external_session;
mod ingest_mark;
mod proxy_session;
mod session;
mod tool_usage;
mod usage_report;

pub use artifact::{
    ArtifactCorrelation, ArtifactSave, ArtifactShape, CreateMcpArtifact, McpArtifactRecord,
    McpArtifactRepository,
};
pub use artifact_finding::{ArtifactFindingRecord, ArtifactFindingRepository};
pub use artifact_ingest::ArtifactIngestRepositories;
pub use artifact_payload::{ArtifactPayloadRecord, ArtifactPayloadRepository};
pub use external_session::ExternalSessionBinding;
pub use ingest_mark::ToolResultIngestMarkRepository;
pub use proxy_session::McpProxySessionRepository;
pub use session::{McpSessionRecord, McpSessionRepository, McpSessionStatus};
pub use tool_usage::{
    ExecutionInsert, IntentStamp, ProximityProbe, ToolCallSubject, ToolUsageRepository,
};
pub use usage_report::ToolUsageReportRepository;

pub mod prelude {
    pub use super::{CreateMcpArtifact, McpArtifactRecord, McpArtifactRepository};
}

// Why: two vantage points can insert the same call at the same instant; the
// loser's write is refused by a named unique index, which is an outcome the
// caller branches on rather than a failure.
pub(crate) fn is_unique_violation_on(err: &crate::error::McpDomainError, index: &str) -> bool {
    matches!(
        err,
        crate::error::McpDomainError::Repository(systemprompt_traits::RepositoryError::Constraint {
            kind: systemprompt_traits::ConstraintKind::Unique,
            constraint,
            ..
        }) if constraint == index
    )
}
