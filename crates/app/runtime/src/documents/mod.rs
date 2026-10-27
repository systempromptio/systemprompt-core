//! The shared document store, composed for the runtime.
//!
//! [`DocumentsRuntime`] is built once by the [`AppContext`] builder from the
//! context's database, file storage, authz hook, subject source and event
//! router, and reached through [`AppContext::documents`]. [`mcp`] holds the
//! MCP tool handlers an extension's MCP server binary composes through
//! [`mcp::DocumentsMcpTools`].
//!
//! [`AppContext`]: crate::AppContext
//! [`AppContext::documents`]: crate::AppContext::documents
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// MCP tool handlers over the document service.
pub mod mcp;
mod wiring;

pub use wiring::{DocumentsRuntime, DocumentsRuntimeParts};
