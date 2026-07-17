//! MCP-to-A2A bridging for tool execution within agent tasks.
//!
//! Covers transforming MCP tool results into A2A artifacts
//! ([`artifact_transformer`]), task-construction helpers, and the
//! [`ToolResultHandler`] that routes tool outputs into task state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod artifact_transformer;
pub mod task_helper;
pub mod tool_result_handler;

pub use artifact_transformer::{McpToA2aTransformer, infer_type, parse_tool_response};
pub use tool_result_handler::ToolResultHandler;
