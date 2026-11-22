//! The MCP tool contract: typed tool handlers, their artifact output schema,
//! declarative toolsets and the execution seam a toolset runs a handler
//! through. Compiled with the `mcp` feature; the MCP member provides the
//! execution implementation (`ToolExecutions`), and the toolset host and the
//! members that contribute toolsets name only this module.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod capabilities;
mod execution;
mod handler;
mod schema;
mod toolset;

pub use execution::{
    DynToolExecution, DynToolExecutions, ToolExecution, ToolExecutions, ToolOutput, ToolRun,
    execute,
};
pub use handler::{McpToolHandler, object_input_schema};
pub use schema::McpOutputSchema;
pub use toolset::{McpToolset, ToolsetCall};
