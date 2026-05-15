//! Tool-call models — MCP tool descriptors, provider tool-call requests,
//! and their execution results.

pub mod mcp_tool;
pub mod tool_call;

pub use mcp_tool::McpTool;
pub use tool_call::{CallToolResult, ToolCall, ToolExecution};
