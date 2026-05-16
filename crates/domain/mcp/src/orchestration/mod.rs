//! Multi-server lifecycle and state orchestration: tool discovery/loading
//! across MCP servers and runtime service-state lookups backed by the database.

mod loader;
mod models;
mod state;

pub use loader::McpToolLoader;
pub use models::{McpServerConnectionInfo, McpServiceState, ServerStatus, SkillLoadingResult};
pub use state::ServiceStateManager;
pub use systemprompt_models::a2a::McpServerMetadata;
