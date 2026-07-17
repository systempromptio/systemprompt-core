//! Tool-call models — MCP tool descriptors, provider tool-call requests,
//! and their execution results.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod mcp_tool;
pub mod tool_call;

pub use mcp_tool::McpTool;
pub use tool_call::{CallToolResult, ToolCall, ToolExecution};
