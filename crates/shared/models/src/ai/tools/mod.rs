//! Tool-call models — MCP tool descriptors, provider tool-call requests,
//! and their execution results.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "mcp")]
pub mod call_result;
pub mod mcp_tool;
pub mod tool_call;

#[cfg(feature = "mcp")]
pub use call_result::CallToolResult;
pub use mcp_tool::McpTool;
pub use tool_call::ToolCall;
