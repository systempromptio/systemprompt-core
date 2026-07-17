//! Multi-server lifecycle and state orchestration: tool discovery/loading
//! across MCP servers and runtime service-state lookups backed by the database.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod loader;
mod models;
mod state;

pub use loader::{McpToolLoader, has_server_permission};
pub use models::{McpServerConnectionInfo, McpServiceState, ServerStatus, SkillLoadingResult};
pub use state::ServiceStateService;
pub use systemprompt_models::a2a::McpServerMetadata;
