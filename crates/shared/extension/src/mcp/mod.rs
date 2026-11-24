//! The MCP tool contract.
//!
//! Typed tool handlers, their artifact output schema, declarative toolsets,
//! the execution seam a toolset runs a handler through (for a hop-verified
//! caller only), the access-recorder
//! slot and the session store the toolset host reads. Compiled with the
//! `mcp` feature; the MCP member provides the
//! execution implementation (`ToolExecutions`), and the toolset host and the
//! members that contribute toolsets name only this module.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod caller;
pub mod capabilities;
mod execution;
mod handler;
mod recorder;
mod recorders;
mod schema;
mod session_store;
mod toolset;

pub use caller::VerifiedCaller;
pub use execution::{
    DynToolExecution, DynToolExecutions, ToolExecution, ToolExecutions, ToolOutput, ToolRun,
    execute,
};
pub use handler::{McpToolHandler, object_input_schema};
pub use recorder::{McpAccessEvent, McpAccessOutcome, McpAccessRecorder, SharedMcpAccessRecorder};
pub use recorders::{AccessRecorderSlot, AccessRecorderSlotBuilder, AccessRecorders};
pub use schema::McpOutputSchema;
pub use session_store::{
    DynMcpSessionStore, McpSessionStore, McpSessionStoreError, McpSessionStoreResult,
};
pub use toolset::{McpToolset, ToolsetCall};
