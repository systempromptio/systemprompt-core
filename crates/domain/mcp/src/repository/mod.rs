//! Postgres persistence for MCP: tool-execution records and aggregate stats,
//! session state, proxy session identities, and tool-output artifacts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod artifact;
mod artifact_finding;
mod artifact_payload;
mod external_session;
mod ownership;
mod proxy_identity;
mod session;
mod tool_usage;

pub use artifact::{
    ArtifactCorrelation, ArtifactShape, CreateMcpArtifact, McpArtifactRecord, McpArtifactRepository,
};
pub use artifact_finding::{
    ArtifactFinding, ArtifactFindingRecord, ArtifactFindingRepository, PHASE_TOOL_RESULT,
};
pub use artifact_payload::{ArtifactPayloadRecord, ArtifactPayloadRepository};
pub use external_session::ExternalSessionBinding;
pub use ownership::McpOwnerReassignment;
pub use proxy_identity::{McpProxyIdentityRepository, ProxyIdentityRow};
pub use session::{McpSessionRecord, McpSessionRepository};
pub use tool_usage::ToolUsageRepository;

pub mod prelude {
    pub use super::{CreateMcpArtifact, McpArtifactRecord, McpArtifactRepository};
}
