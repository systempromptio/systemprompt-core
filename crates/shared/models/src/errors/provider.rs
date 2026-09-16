//! Typed errors returned across the dyn-dispatched provider seams
//! ([`crate::ai::AiProvider`], [`crate::mcp::McpRegistry`] and friends).
//!
//! Each variant names the failure class a caller can act on; the concrete
//! provider maps its own error hierarchy onto these before crossing the seam,
//! so consumers never see a boxed, opaque error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AiInferenceError {
    #[error("provider {provider} rejected the request: {message}")]
    Provider { provider: String, message: String },

    #[error("no provider available for model {model}")]
    NoProviderForModel { model: String },

    #[error("provider {provider} rate limited the request: {details}")]
    RateLimited { provider: String, details: String },

    #[error("provider {provider} rejected the credentials")]
    AuthenticationFailed { provider: String },

    #[error("provider {provider} unavailable: {message}")]
    Unavailable { provider: String, message: String },

    #[error("inference request invalid: {0}")]
    InvalidRequest(String),

    #[error("tool execution failed: {0}")]
    Tool(String),

    #[error("configuration error: {0}")]
    Configuration(String),

    #[error("persistence failed: {0}")]
    Storage(String),

    #[error("{0}")]
    Internal(String),
}

pub type AiInferenceResult<T> = Result<T, AiInferenceError>;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum McpRegistryError {
    #[error("MCP server not found: {0}")]
    NotFound(String),

    #[error("MCP registry configuration error: {0}")]
    Configuration(String),

    #[error("MCP transport failure for server {server}: {message}")]
    Transport { server: String, message: String },

    #[error("{0}")]
    Internal(String),
}

pub type McpRegistryResult<T> = Result<T, McpRegistryError>;
