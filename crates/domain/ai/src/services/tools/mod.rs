//! Tool dispatch helpers.
//!
//! Includes adapters between MCP and the trait-level tool representation,
//! tool discovery for an agent, and the [`crate::NoopToolProvider`] used as a
//! default when no MCP services are configured.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod adapter;
pub mod discovery;
pub mod noop_provider;

pub use adapter::{
    definition_to_mcp_tool, mcp_tool_to_definition, request_context_to_tool_context,
    request_to_tool_call, rmcp_result_to_trait_result, tool_call_to_request,
    trait_result_to_rmcp_result,
};
pub use discovery::ToolDiscovery;
pub use noop_provider::NoopToolProvider;
