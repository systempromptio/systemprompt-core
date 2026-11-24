//! Typed error returned by [`crate::tool::ToolProvider`] implementations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::McpServerId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolProviderError {
    #[error("Tool execution failed: circuit breaker open for MCP server {server}; failing fast")]
    CircuitOpen { server: McpServerId },

    #[error("Tool execution failed: MCP server {server} unavailable: concurrency limit reached")]
    ConcurrencyLimit { server: McpServerId },

    #[error("Tool execution failed: MCP server {server} timed out after {after:?}")]
    TimedOut {
        server: McpServerId,
        after: Duration,
    },

    #[error("Tool execution failed: {0}")]
    Execution(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error(
        "Authorization failed: Missing or invalid x-user-id header - the caller identity must be \
         propagated from the parent request"
    )]
    MissingCallerIdentity,

    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },

    #[error("Internal error: {0}")]
    Internal(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("no tool provider is composed; tool '{tool}' cannot run")]
    NoTools { tool: String },
}

pub type ToolProviderResult<T> = Result<T, ToolProviderError>;
