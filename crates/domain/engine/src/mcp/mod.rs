//! Model Context Protocol client.
//!
//! [`McpManager`] owns a set of named servers, remote (streamable HTTP with a
//! legacy SSE fallback) or local (a child process on stdio). It connects them
//! concurrently, reports each one's [`ServerStatus`], keeps a single catalogue
//! of their tools under collision-free qualified names and routes tool calls.
//! [`McpClient`] is the session with one server and can be used on its own.
//!
//! The crate knows nothing about who supplies credentials: a remote server's
//! headers come from a [`HeaderSource`] the caller implements, read again
//! before every request. A server that answers 401 is reported as
//! [`ServerStatus::NeedsAuth`]; this crate runs no authorization flow. Header
//! values are never logged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod catalog;
mod client;
mod error;
mod headers;
mod jsonrpc;
mod manager;
mod reconcile;
mod transport;
mod types;

pub use catalog::{MAX_QUALIFIED_NAME_LEN, McpTool, qualify};
pub use client::{
    DEFAULT_REQUEST_TIMEOUT, LATEST_PROTOCOL_VERSION, McpClient, RequestOptions,
    SUPPORTED_PROTOCOL_VERSIONS, ServerNotification,
};
pub use error::{BoxError, McpError};
pub use headers::{HeaderSource, RemoteHeaders};
pub use manager::{ManagerConfig, McpManager, ServerConfig, ServerSpec, ServerStatus, StatusMap};
pub use tokio_util::sync::CancellationToken;
pub use types::{
    Attachment, CallResult, ClientInfo, Prompt, PromptArgument, PromptMessage, PromptResult,
    Resource, ResourceContent, ServerInfo, ToolInfo,
};
