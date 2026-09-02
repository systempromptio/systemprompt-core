//! Postgres persistence for MCP: tool-execution records and aggregate stats,
//! session state, proxy session identities, and tool-output artifacts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod artifact;
mod proxy_identity;
mod session;
mod tool_usage;

pub use artifact::{CreateMcpArtifact, McpArtifactRecord, McpArtifactRepository};
pub use proxy_identity::{McpProxyIdentityRepository, ProxyIdentityRow};
pub use session::{McpSessionRecord, McpSessionRepository};
pub use tool_usage::ToolUsageRepository;

pub mod prelude {
    pub use super::{CreateMcpArtifact, McpArtifactRecord, McpArtifactRepository};
}
