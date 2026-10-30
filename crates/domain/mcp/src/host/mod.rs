//! The generic MCP host: one rmcp server for any set of declared toolsets.
//!
//! [`ToolsetServer`] absorbs what every hand-written MCP server binary
//! repeated — server info and icons, RBAC and the governance hook on every
//! request, an access-audit row per outcome, one [`crate::McpToolExecutor`] —
//! and adds per-caller tool filtering from the YAML's `access:` blocks. The
//! runtime crate wraps it in `serve(ctx, server_id)`, which builds the
//! deployment's toolsets from the factory inventory and binds the listener.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod catalog;
mod gate;
mod recorder;
mod server;

pub use catalog::{HostedCatalog, HostedTool, ToolsetHostError, resolve_tool_access};
pub use recorder::{
    McpAccessEvent, McpAccessOutcome, McpAccessRecorder, McpAccessRecorderRegistration,
    SharedMcpAccessRecorder, discover_mcp_access_recorders,
};
pub use server::{ToolsetAuthz, ToolsetServer, ToolsetServerParts};

pub async fn serve_router(
    listener: tokio::net::TcpListener,
    router: axum::Router,
) -> std::io::Result<()> {
    axum::serve(listener, router).await
}
