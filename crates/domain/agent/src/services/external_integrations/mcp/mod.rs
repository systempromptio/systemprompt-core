//! MCP integration for agent module.
//!
//! This module re-exports MCP orchestration types from the MCP module.
//! The implementations have been moved to systemprompt-core-mcp for
//! proper module boundaries.

// Re-export orchestration types from MCP module
pub use systemprompt_mcp::orchestration::{
    McpServerConnectionInfo, McpServerMetadata, McpServiceState, McpToolLoader, ServerStatus,
    ServiceStateManager, SkillLoadingResult,
};

// Re-export MCP client types
pub use systemprompt_mcp::services::client::McpClient;
pub use systemprompt_models::ai::tools::McpTool;
